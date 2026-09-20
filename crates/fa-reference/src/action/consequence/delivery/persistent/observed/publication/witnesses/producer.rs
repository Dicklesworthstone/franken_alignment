//! Derive complete change windows from the same immutable images they describe.
//! This is producer observation state, never an effect ledger or a truth oracle.
mod codec;
mod publisher;
pub use publisher::{FilePublicationProducer, ProducerPublication, ProducerPublicationKind};
#[cfg(test)]
mod tests;

use super::{FilePublicationInputs, FileWitnessInput, MAX_PUBLICATION_PACKET_BYTES};
use super::super::capture::{FileCaptureIdentity, FilePublicationCapture};
use super::super::capture::heartbeat::feed::{PublicationFeedBatch, MAX_FEED_BYTES, MAX_FEED_RECORDS};
use crate::action::{ElapsedTick, FrozenAction, Purpose, Scope};
use crate::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationInputCut};
use crate::action::consequence::delivery::publication_gate::changes::freshness::PublicationHeartbeat;
use crate::witness::refinement::index::routing::WitnessChange;
use crate::Error;

pub const MAX_PRODUCER_BYTES: usize = MAX_PUBLICATION_PACKET_BYTES + MAX_FEED_BYTES + 256;

/// Independently configured identities and bootstrap position. `source` names
/// the snapshot producer, `feed` its notifications. Neither is authentication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationProducerProfile {
    pub source: u64,
    pub scope: Scope,
    pub feed: u64,
    pub clock_domain: u64,
    pub after: u64,
}
impl PublicationProducerProfile {
    pub fn check(self) -> Result<(), Error> {
        self.scope.validate()?;
        if self.scope.purpose != Purpose::Effect { return Err(Error::Binding); }
        if self.source == 0 || self.feed == 0 || self.clock_domain == 0 {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}

/// A single bounded source image and its exact retained notification suffix.
/// Callers cannot edit the suffix or stamp a supplied coverage claim onto it.
/// Clones are historical data; only the locked publisher owns persistent writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationProducerImage {
    profile: PublicationProducerProfile,
    input_generation: u64,
    floor: Option<(u64, u64, u64)>,
    inputs: FilePublicationInputs,
    batch: PublicationFeedBatch,
}
impl PublicationProducerImage {
    pub fn new(profile: PublicationProducerProfile, inputs: FilePublicationInputs,
        observed_at: ElapsedTick) -> Result<Self, Error>
    {
        profile.check()?;
        let heartbeat = PublicationHeartbeat { source: profile.feed, clock_domain: profile.clock_domain,
            generation: 1, through: profile.after, produced_at: observed_at };
        let image = Self { profile, input_generation: 1, floor: input_floor(&inputs), inputs,
            batch: PublicationFeedBatch::new(heartbeat, profile.after, Vec::new())? };
        image.to_bytes()?;
        Ok(image)
    }

    pub fn profile(&self) -> PublicationProducerProfile { self.profile }
    pub fn generation(&self) -> u64 { self.batch.heartbeat().generation }
    pub fn input_generation(&self) -> u64 { self.input_generation }
    pub fn inputs(&self) -> &FilePublicationInputs { &self.inputs }
    pub fn batch(&self) -> &PublicationFeedBatch { &self.batch }

    /// Bind actual producer data to an independently supplied action. The source
    /// does not review that action; the consumer still owns exact review/authority.
    pub fn capture(&self, attempt: u64, action: &FrozenAction) -> Result<FilePublicationCapture, Error> {
        if action.spec().scope != self.profile.scope { return Err(Error::Binding); }
        FilePublicationCapture::new_at_cut(attempt,
            FileCaptureIdentity { source: self.profile.source, generation: self.input_generation },
            action, self.inputs.clone(), PublicationInputCut {
                source: self.profile.feed, through: self.batch.heartbeat().through,
            })
    }

    /// Build a whole successor without changing this image. Notifications are
    /// derived, not caller supplied: inserts/deletes/value OR version changes
    /// select keys; semantic/domain/closure or opaque changes select All. More
    /// than a window's worth of changed keys becomes one conservative Domain.
    ///
    /// Even an equal image requires an explicit new producer observation to
    /// advance heartbeat time. Consumer rereads never call this method. Failure
    /// preserves all original data and counters; absence retains snapshot floors.
    pub fn advance(&self, expected_generation: u64, inputs: FilePublicationInputs,
        observed_at: ElapsedTick) -> Result<Self, Error>
    {
        if expected_generation != self.generation() { return Err(Error::Stale); }
        if observed_at < self.batch.heartbeat().produced_at { return Err(Error::Stale); }
        let next_floor = input_floor(&inputs);
        if let (Some(old), Some(new)) = (self.floor, next_floor)
            && (new.0 < old.0 || new.1 < old.1 || new.2 < old.2) {
                return Err(Error::Stale);
            }
        let changes = derive_changes(&self.inputs, &inputs)?;
        let generation = self.generation().checked_add(1).ok_or(Error::Overflow)?;
        let input_generation = if inputs == self.inputs { self.input_generation }
            else { self.input_generation.checked_add(1).ok_or(Error::Overflow)? };
        let mut through = self.batch.heartbeat().through;
        let mut records = Vec::new();
        records.try_reserve_exact(MAX_FEED_RECORDS * 2).map_err(|_| Error::Limit)?;
        records.extend_from_slice(self.batch.records());
        for change in changes {
            through = through.checked_add(1).ok_or(Error::Overflow)?;
            records.push(PublicationChange { source: self.profile.feed, sequence: through, change });
        }
        let removed = records.len().saturating_sub(MAX_FEED_RECORDS);
        drop(records.drain(..removed));
        let after = through.checked_sub(records.len() as u64).ok_or(Error::Overflow)?;
        let heartbeat = PublicationHeartbeat { source: self.profile.feed, clock_domain: self.profile.clock_domain,
            generation, through, produced_at: observed_at };
        let next = Self { profile: self.profile, input_generation,
            floor: next_floor.or(self.floor), inputs,
            batch: PublicationFeedBatch::new(heartbeat, after, records)? };
        next.to_bytes()?;
        Ok(next)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> { codec::encode(self) }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> { codec::decode(bytes) }
}

fn input_floor(inputs: &FilePublicationInputs) -> Option<(u64, u64, u64)> {
    inputs.structured().map(|input| {
        let snapshot = input.snapshot();
        (snapshot.revision(), snapshot.control_cut(), snapshot.semantic_epoch())
    })
}

fn derive_changes(old: &FilePublicationInputs, new: &FilePublicationInputs) -> Result<Vec<WitnessChange>, Error> {
    if old.opaque() != new.opaque() { return Ok(vec![WitnessChange::All]); }
    match (old.structured(), new.structured()) {
        (None, None) => Ok(Vec::new()),
        (Some(old), Some(new)) => structured_changes(old, new),
        _ => Ok(vec![WitnessChange::All]),
    }
}

fn structured_changes(old: &FileWitnessInput, new: &FileWitnessInput) -> Result<Vec<WitnessChange>, Error> {
    if old.snapshot.semantic_epoch() != new.snapshot.semantic_epoch()
        || old.snapshot.domain_input() != new.snapshot.domain_input()
        || old.admitted_close != new.admitted_close {
        // All also reaches registrations made under a different domain identity.
        return Ok(vec![WitnessChange::All]);
    }
    let domain = old.snapshot.domain_input().domain();
    let mut keys = Vec::new();
    keys.try_reserve_exact(old.keys.len() + new.keys.len()).map_err(|_| Error::Limit)?;
    keys.extend_from_slice(&old.keys);
    keys.extend_from_slice(&new.keys);
    keys.sort_unstable();
    keys.dedup();
    let mut changes = Vec::new();
    changes.try_reserve_exact(keys.len().min(MAX_FEED_RECORDS)).map_err(|_| Error::Limit)?;
    for key in keys {
        if old.snapshot.entry(key) != new.snapshot.entry(key) {
            if changes.len() == MAX_FEED_RECORDS {
                return Ok(vec![WitnessChange::Domain { domain }]);
            }
            changes.push(WitnessChange::Key { domain, key });
        }
    }
    Ok(changes)
}
