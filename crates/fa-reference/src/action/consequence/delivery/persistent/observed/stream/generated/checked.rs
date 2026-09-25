//! Compose native monitored text and final-publication witnesses in ONE image.
//! These are bootstrap pins, not new evaluators, permissions or saved approvals.
use super::{BaseEvent, Event, FileOversight, JournalError, Machine};
use super::super::{FileHumanReviewer, FileOversightProfile, StreamProfile};
use super::super::super::{journal, storage};
use super::super::super::decoder::{DecoderEvent, FileDecoderConfig,
    text::{MAX_FILE_TOKENIZER_BYTES, replay_text_events}};
use super::super::super::publication::witness_gate::{WitnessEvent, freshness::FreshnessEvent};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::action::consequence::delivery::publication_gate::PublicationLimits;
use crate::action::consequence::delivery::publication_gate::changes::{PublicationChangePolicy,
    freshness::PublicationFreshnessPolicy};
use crate::Error;
use std::path::Path;
use std::rc::Rc;

/// Independently retained native stream and publication contract. No source
/// capture, helper result, current time or authority is accepted here. Absence
/// of a feed/reserve is an exact choice, not permission to ignore a stored one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedPublicationProfile {
    pub stream: StreamProfile,
    pub reserve: Option<RecoveryReserve>,
    pub limits: PublicationLimits,
    pub feed: Option<GeneratedPublicationFeed>,
}

/// Reuse the original change-tail and producer-lease semantics. Snapshot fallback
/// is explicitly selected; it never treats an incomplete exact snapshot as valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedPublicationFeed {
    pub changes: PublicationChangePolicy,
    pub freshness: PublicationFreshnessPolicy,
    pub snapshot_fallback: bool,
}

impl GeneratedPublicationProfile {
    fn witnesses(self) -> Vec<WitnessEvent> {
        let mut events = vec![WitnessEvent::Enable(self.limits)];
        if let Some(feed) = self.feed {
            events.push(WitnessEvent::ChangeProfile(feed.changes));
            events.push(WitnessEvent::Freshness(FreshnessEvent::Enable(feed.freshness)));
            if feed.snapshot_fallback {
                events.push(WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback));
            }
        }
        events
    }

    /// Compare the entire selected bootstrap, including duplicates/extra modes,
    /// before numerical replay or any store confirmation/cleanup/recovery write.
    fn check(self, events: &[Event]) -> Result<(), Error> {
        if !matches!(events.first(), Some(Event::GeneratedStreamBootstrap(actual)) if *actual == self.stream) {
            return Err(Error::Binding);
        }
        let mut reserves = events.iter().filter_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(actual)) => Some(*actual), _ => None,
        });
        if reserves.next() != self.reserve || reserves.next().is_some() { return Err(Error::Binding); }
        let mut actual = events.iter().filter_map(|event| match event {
            Event::PublicationWitness(witness) if witness.bootstrap() => Some(witness), _ => None,
        });
        for expected in self.witnesses() {
            if !actual.next().is_some_and(|actual| same_bootstrap(actual, &expected)) {
                return Err(Error::Binding);
            }
        }
        if actual.next().is_some() { return Err(Error::Binding); }
        Ok(())
    }
}

fn same_bootstrap(actual: &WitnessEvent, expected: &WitnessEvent) -> bool {
    match (actual, expected) {
        (WitnessEvent::Enable(a), WitnessEvent::Enable(b)) => a == b,
        (WitnessEvent::ChangeProfile(a), WitnessEvent::ChangeProfile(b)) => a == b,
        (WitnessEvent::Freshness(FreshnessEvent::Enable(a)),
            WitnessEvent::Freshness(FreshnessEvent::Enable(b))) => a == b,
        (WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback),
            WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback)) => true,
        _ => false,
    }
}

impl FileOversight {
    /// Atomically install the native-text-only stream, optional original recovery
    /// reserve, exact witness/change/freshness contracts, model and tokenizer.
    /// Invalid combinations fail native replay before a store is created. No
    /// successfully initialized image can lack one of these mandatory guards.
    /// No token is computed and no caller-provided text gains native provenance.
    ///
    /// Actual source capture, complete helper review, the independent human key
    /// and fresh final publication validation remain separate requirements.
    /// This local reference constructor is not remote delivery or OS isolation.
    pub fn create_generated_text_stream_checked(directory: impl AsRef<Path>,
        profile: FileOversightProfile, decoder: FileDecoderConfig, tokenizer: ByteBpe,
        selected: GeneratedPublicationProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let mut events = vec![Event::GeneratedStreamBootstrap(selected.stream)];
        if let Some(reserve) = selected.reserve { events.push(Event::Core(BaseEvent::ReserveRecovery(reserve))); }
        events.extend(selected.witnesses().into_iter().map(Event::PublicationWitness));
        events.extend([Event::Decoder(DecoderEvent::Enable(Rc::new(decoder))),
            Event::Decoder(DecoderEvent::Tokenizer(canonical.into()))]);
        if events.len() > profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&journal::encode(&profile, store.identity(), &events)?)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    /// Pin native provenance, exact stream/reserve and ALL selected witness
    /// bootstrap fields against ONE locked canonical image. Then pin actual
    /// model/monitor/sampler/tokenizer bytes and replay through the original
    /// reducers. Mismatches do not clean staging, fence or create a live role.
    ///
    /// Successful recovery uses the original fence: acknowledged progress is
    /// retained, but no clock, source lease, sendable key or numerical resume is
    /// restored. Recorded requests remain eligible only for original receipt
    /// reconciliation. Pending inference needs fresh source/time and explicit
    /// resume; new publication still needs current witnesses and both keys.
    ///
    /// This does not pin an independently authenticated latest head or compose
    /// additional anchored/joint-governance roles. Those retain their own APIs.
    pub fn open_generated_text_stream_checked(directory: impl AsRef<Path>,
        profile: FileOversightProfile, decoder: &FileDecoderConfig, tokenizer: &ByteBpe,
        selected: GeneratedPublicationProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        selected.check(&events)?;
        let machine = replay_text_events(&profile, decoder, &canonical, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }
}

#[cfg(test)]
mod tests;
