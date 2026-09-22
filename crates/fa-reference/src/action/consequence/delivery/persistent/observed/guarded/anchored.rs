//! Exact, externally retained journal-prefix anchors for guarded recovery.
//!
//! Counter floors detect older cuts but cannot distinguish equal-counter forks.
//! This profile compares canonical ORIGINAL journal bytes, then uses the same
//! guard admission, semantic replay and recovery fence as guarded recovery.
//! The anchor is sensitive operator data, not a signature, permit or checkpoint.

use super::{BaseEvent, Event, FileOversight, FileOversightProfile, FileOversightRoles,
    FileRecoveryRequirements, JournalError, Machine, journal, storage};
use crate::Error;
use std::fmt;
use std::path::Path;

/// An acknowledged history prefix retained OUTSIDE the mutable publication
/// journal. Cloning this data copies no rights or live owner. Its usefulness
/// depends on independently retaining the latest required anchor: replacing both
/// journal and anchor with old copies defeats this protection.
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

    fn check(&self, profile: &FileOversightProfile, identity: &Path, events: &[Event])
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
        Ok(FileHistoryAnchor { revision: self.events.len(), canonical })
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
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        anchor.check(&profile, store.identity(), &events)?;
        expected.guards.check_decoder_config(&events)?;
        let machine = Machine::replay(&profile, &events)?;
        expected.check(&profile, &machine, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, human) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }
}

#[cfg(test)]
mod tests;
