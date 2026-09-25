//! A native-text-only stream is an immutable bootstrap contract, not a flag
//! that an actor or an existing generation can turn on after the fact.
use super::{BaseEvent, Event, FileOversight, JournalError, Machine};
use super::super::{FileHumanReviewer, FileOversightProfile, ReleaseFrame, StreamProfile, storage};
use super::super::super::decoder::{DecoderEvent, FileDecoderConfig, text::MAX_FILE_TOKENIZER_BYTES};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::Error;
use std::path::Path;
use std::rc::Rc;

impl FileOversight {
    /// Create a complete-message stream whose messages MUST come through
    /// submit_decoder_text_message. Install the original decoder and immutable
    /// tokenizer with that requirement in ONE first canonical image. No token
    /// computes during construction and no partial owner/reviewer is returned.
    /// Additional optional guards may be configured before their original cutoff.
    ///
    /// Direct proposals, ordinary request submission and actor-wire message
    /// submission cannot supply replacement message bytes, even when those bytes
    /// equal a native result. Native source checks, full-input congress, two-key
    /// approval and final publication checks all remain separate prerequisites.
    ///
    /// A finish frame is still permitted through the ordinary proposal path:
    /// it adds NO message and must satisfy the original exact cumulative-prefix
    /// checks, congress, human key and dispatch law. Finishing cannot disclose
    /// a partial generation or label its output as a completed native result.
    ///
    /// There is no enable/disable or retrofit API. Existing create_stream owners
    /// keep their original caller-text contract. This local reference sink is
    /// not OS isolation, remote delivery or a claim of model correctness.
    pub fn create_generated_text_stream(directory: impl AsRef<Path>, profile: FileOversightProfile,
        stream: StreamProfile, decoder: FileDecoderConfig, tokenizer: ByteBpe)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        Self::create_generated_stream_bootstrap(directory.as_ref(), profile, stream, decoder, tokenizer, None)
    }

    /// Install the original logical recovery reserve BEFORE tokenizer work in
    /// the same first image. The default constructor has no reserve; installing
    /// one afterward would be too late under the original admission contract.
    /// No allowance is created by recovery, and physical disk space is not
    /// reserved. Only the existing Fence/Stop/StopProgress lane can use its tail.
    pub fn create_generated_text_stream_with_reserve(directory: impl AsRef<Path>,
        profile: FileOversightProfile, stream: StreamProfile, decoder: FileDecoderConfig,
        tokenizer: ByteBpe, reserve: RecoveryReserve)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        Self::create_generated_stream_bootstrap(directory.as_ref(), profile, stream, decoder, tokenizer, Some(reserve))
    }

    fn create_generated_stream_bootstrap(directory: &Path, profile: FileOversightProfile,
        stream: StreamProfile, decoder: FileDecoderConfig, tokenizer: ByteBpe,
        reserve: Option<RecoveryReserve>) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        let count = 3 + usize::from(reserve.is_some());
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES || profile.delivery.limits.events < count {
            return Err(Error::Limit.into());
        }
        let mut events = Vec::with_capacity(count);
        events.push(Event::GeneratedStreamBootstrap(stream));
        if let Some(reserve) = reserve { events.push(Event::Core(BaseEvent::ReserveRecovery(reserve))); }
        events.extend([Event::Decoder(DecoderEvent::Enable(Rc::new(decoder))),
            Event::Decoder(DecoderEvent::Tokenizer(canonical.into()))]);
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory)?;
        store.replace(&super::super::super::journal::encode(&profile, store.identity(), &events)?)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    /// Recover an originally native-text-only stream with NO recovery reserve.
    /// Pin the stream, model/monitor/sampler and tokenizer from the SAME locked
    /// canonical image before numerical replay, cleanup, role creation or fence.
    /// An ordinary caller-text stream is not an acceptable substitute, even when
    /// its current decoder, tokenizer and output bytes happen to match.
    ///
    /// Recovery uses the original full numerical replay and authority fence. It
    /// preserves acknowledged work, RNG draws, text intents and message history;
    /// it returns a paused decoder, not fresh time, evidence or old sendable keys.
    /// Resume and publication still require their original separate admissions.
    /// Optional anchored/guarded-role deployments must use their composed opener;
    /// this local bootstrap pin is not an independent anti-rollback guarantee.
    pub fn open_generated_text_stream(directory: impl AsRef<Path>, profile: FileOversightProfile,
        stream: StreamProfile, decoder: &FileDecoderConfig, tokenizer: &ByteBpe)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        Self::open_generated_stream_bootstrap(directory.as_ref(), profile, stream, decoder, tokenizer, None)
    }

    /// Recover with exactly the independently supplied logical reserve. Presence
    /// and absence are distinct contracts; recovery never installs, enlarges or
    /// removes a reserve to make an otherwise incompatible image writable.
    pub fn open_generated_text_stream_with_reserve(directory: impl AsRef<Path>,
        profile: FileOversightProfile, stream: StreamProfile, decoder: &FileDecoderConfig,
        tokenizer: &ByteBpe, reserve: RecoveryReserve)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        Self::open_generated_stream_bootstrap(directory.as_ref(), profile, stream, decoder, tokenizer, Some(reserve))
    }

    fn open_generated_stream_bootstrap(directory: &Path, profile: FileOversightProfile,
        stream: StreamProfile, decoder: &FileDecoderConfig, tokenizer: &ByteBpe,
        reserve: Option<RecoveryReserve>) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let store = storage::Store::open(directory)?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = super::super::super::journal::decode(&profile, store.identity(), &bytes)?;
        check_generated_contract(&events, stream, reserve.as_ref())?;
        let machine = super::super::super::decoder::text::replay_text_events(
            &profile, decoder, &canonical, &events)?;
        if !machine.generated_text_only { return Err(Error::Binding.into()); }
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }

    /// Mode at the acknowledged cut, not an independent assurance certificate.
    /// A faulted owner cannot report its old mode as the current writable state.
    pub fn generated_text_stream_required(&self) -> Result<bool, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.generated_text_only)
    }
}

fn check_generated_contract(events: &[Event], stream: StreamProfile,
    reserve: Option<&RecoveryReserve>) -> Result<(), Error>
{
    if !matches!(events.first(), Some(Event::GeneratedStreamBootstrap(actual)) if *actual == stream) {
        return Err(Error::Binding);
    }
    let mut reserves = events.iter().filter_map(|event| match event {
        Event::Core(BaseEvent::ReserveRecovery(actual)) => Some(actual),
        _ => None,
    });
    if reserves.next() != reserve || reserves.next().is_some() { return Err(Error::Binding); }
    Ok(())
}

impl Machine {
    pub(in super::super::super) fn check_generated_text_origin(&self, event: &Event)
        -> Result<(), Error>
    {
        if !self.generated_text_only { return Ok(()); }
        match event {
            Event::Core(BaseEvent::Propose(_, spec, _) | BaseEvent::SubmitRequest(_, spec, _)) => {
                // Original bounded frame parsing and original prefix validation
                // still apply. Only a finish is non-disclosing; arbitrary bytes
                // cannot borrow the provenance of a nearby native generation.
                if ReleaseFrame::decode(&spec.payload)?.message().is_some() {
                    return Err(Error::Binding);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
