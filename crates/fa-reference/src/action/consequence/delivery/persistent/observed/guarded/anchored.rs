//! Exact, externally retained journal-prefix anchors for guarded recovery.
//!
//! Counter floors detect older cuts but cannot distinguish equal-counter forks.
//! This profile compares canonical ORIGINAL journal bytes, then uses the same
//! guard admission, semantic replay and recovery fence as guarded recovery.
//! The anchor is sensitive operator data, not a signature, permit or checkpoint.

mod wire;
mod text;
mod learned;
pub use wire::{FileHistoryAnchorError, MAX_HISTORY_ANCHOR_BYTES};

use super::{BaseEvent, Event, FileOversight, FileOversightProfile, FileOversightRoles,
    FileRecoveryRequirements, JournalError, Machine, journal, storage};
use crate::Error;
use std::fmt;
use std::path::Path;

/// An operator's required history prefix retained OUTSIDE the mutable publication
/// journal. Capture uses an acknowledged owner; read_trusted imports independently
/// retained data and does not authenticate it. Cloning copies no rights or owner.
/// Replacing both journal and anchor with old copies defeats this protection.
///
/// Exact prefix comparison accepts genuine append-only successors, not just the
/// identical current image. It does not authenticate the unanchored suffix or
/// the original observations, and it never supplies fresh clock/source evidence.
/// The canonical journal includes payloads and private review inputs; Debug is
/// deliberately redacted, and this is never an actor-facing artifact.
#[derive(Clone, PartialEq, Eq)]
pub struct FileHistoryAnchor {
    revision: usize,
    canonical: Vec<u8>,
}

impl fmt::Debug for FileHistoryAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileHistoryAnchor")
            .field("revision", &self.revision)
            .field("retained_bytes", &self.canonical.len())
            .finish_non_exhaustive()
    }
}

impl FileHistoryAnchor {
    pub fn revision(&self) -> u64 { self.revision as u64 }
    /// Logical retained canonical bytes, not heap allocation or storage overhead.
    pub fn retained_bytes(&self) -> usize { self.canonical.len() }

    // Only original owners may move their already encoded, durably acknowledged
    // canonical cut here. This is not an event importer or a public constructor.
    // Terminal recovery uses the bytes of its successful replacement (or the
    // identical confirmed drained cut), without another encoding or replay.
    pub(in super::super) fn from_acknowledged(revision: usize, canonical: Vec<u8>) -> Self {
        Self { revision, canonical }
    }

    // The read-only review auditor shares this exact checker; no public event importer.
    pub(in super::super) fn check(&self, profile: &FileOversightProfile, identity: &Path, events: &[Event])
        -> Result<(), Error>
    {
        if self.revision > events.len() { return Err(Error::Stale); }
        // Comparing the entire encoded file directly would reject legitimate
        // successors because the journal header contains its event count.
        // Re-encode exactly the anchored prefix using the ORIGINAL encoder,
        // including its independent bootstrap, path identity and format domain.
        let prefix = journal::encode(profile, identity, &events[..self.revision])?;
        if prefix != self.canonical { return Err(Error::Binding); }
        Ok(())
    }
}

impl FileOversight {
    /// Capture the acknowledged original history without reading potentially
    /// newer disk bytes or advancing the journal. A poisoned owner cannot issue
    /// an anchor for its old RAM state after an ambiguous replacement.
    pub fn history_anchor(&self) -> Result<FileHistoryAnchor, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let canonical = journal::encode(&self.profile, self.store.identity(), &self.events)?;
        Ok(FileHistoryAnchor::from_acknowledged(self.events.len(), canonical))
    }

    /// Advance an independently held anchor only along this healthy owner's
    /// acknowledged original history. Equal-cut retries are idempotent; an older,
    /// foreign or divergent owner cannot replace a newer required prefix merely
    /// because it was opened through another, weaker recovery entry point.
    ///
    /// This returns data without writing either journal or custodian storage.
    /// The custodian must retain the old anchor until the new one is durably
    /// acknowledged. Passing a stale previous anchor cannot protect intervening
    /// history which the custodian did not independently retain.
    pub fn history_anchor_after(&self, previous: &FileHistoryAnchor)
        -> Result<FileHistoryAnchor, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        previous.check(&self.profile, self.store.identity(), &self.events)?;
        self.history_anchor()
    }

    /// Require exact anchored history AND the independent current guard set,
    /// effective policy, credential epoch and numeric recovery floors.
    ///
    /// One exclusive Store supplies the SAME bytes for prefix validation and
    /// semantic replay. A divergent or truncated prefix refuses before cleanup,
    /// recovery writes or role provisioning. The unanchored suffix must still
    /// pass every original guard and transition; an anchor is not permission to
    /// skip replay. Success appends exactly the original recovery fence and
    /// withdraws saved eligibility/approvals rather than restoring them.
    ///
    /// This is the base composed guarded profile. Evaluated, predictive and
    /// mediated profiles are deliberately rejected by its existing gate-inventory
    /// checks rather than silently downgraded. No remote effect is replayed.
    pub fn open_guarded_anchored(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor)
        -> Result<(Self, FileOversightRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_guarded_anchored_store(store, profile, expected, anchor)
    }

    // Private Store seam for real barrier-failure tests, never an actor callback.
    fn open_guarded_anchored_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor)
        -> Result<(Self, FileOversightRoles), JournalError>
    {
        super::recovery::FileGuardedRecovery::begin_store(store, profile, expected, Some(anchor))?
            .finish_synchronously()
    }
}

#[cfg(test)]
mod tests;

impl FileHistoryAnchor {
    // Registration consumes the SAME original canonical decoder as plan import.
    // A transport-valid archive alone is not a profile/path/revision binding.
    // No semantic replay or filesystem access is done by this private data seam.
    pub(in super::super) fn registration_image(&self, profile: &FileOversightProfile,
        identity: &Path) -> Result<&[u8], Error>
    {
        self.decoded_events(profile, identity)?;
        Ok(&self.canonical)
    }

    // Bounded inspection of this captured cut, never a filesystem reopen or
    // semantic replay. Only native consumers may inspect original event inputs;
    // an imported anchor alone still carries no acknowledgment or live role.
    pub(in super::super) fn decoded_events(&self, profile: &FileOversightProfile,
        identity: &Path) -> Result<Vec<Event>, Error>
    {
        let events = journal::decode(profile, identity, &self.canonical)?;
        if events.len() != self.revision { return Err(Error::Binding); }
        Ok(events)
    }
}
