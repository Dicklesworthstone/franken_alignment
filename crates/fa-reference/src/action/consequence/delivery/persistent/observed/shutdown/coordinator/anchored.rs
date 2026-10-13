//! Independently retained coordinator history, not a stop receipt or live role.
//! Coordinator images update pending visits in place, so continuity is checked
//! against their original transition rules rather than a raw byte prefix.
use super::*;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};

const DOMAIN: &[u8; 8] = b"FASCHAN1";
pub const MAX_COORDINATOR_ANCHOR_BYTES: usize = MAX_JOURNAL_BYTES + 20;

/// Private operator custody of an acknowledged coordinator image. Clones share
/// bytes, never rights. A trusted import does not authenticate its custodian.
/// Joint rollback of the coordinator and every retained copy is out of scope.
#[derive(Clone, PartialEq, Eq)]
pub struct FileShutdownCoordinatorAnchor {
    revision: u64,
    canonical: Rc<[u8]>,
}
impl fmt::Debug for FileShutdownCoordinatorAnchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileShutdownCoordinatorAnchor").field("revision", &self.revision)
            .field("retained_bytes", &self.canonical.len()).finish_non_exhaustive()
    }
}
impl FileShutdownCoordinatorAnchor {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn retained_bytes(&self) -> usize { self.canonical.len() }

    /// Bounded transport only. Contains private domain history and diagnostics.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut w = Writer::new(MAX_COORDINATOR_ANCHOR_BYTES);
        w.raw(DOMAIN)?; w.u64(self.revision)?; w.blob(&self.canonical)?;
        Ok(w.finish())
    }

    /// Import independently retained bytes, NOT a checkpoint state or approval.
    /// Full plan/path/capacity and original image validation occurs on use.
    pub fn from_trusted_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_COORDINATOR_ANCHOR_BYTES { return Err(Error::Limit); }
        let mut r = Reader::new(bytes);
        if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::Binding); }
        let revision = r.u64()?;
        if revision > 2 * MAX_SHUTDOWN_ATTEMPTS as u64 { return Err(Error::Limit); }
        let canonical = r.blob(MAX_JOURNAL_BYTES)?;
        if canonical.is_empty() { return Err(Error::Incomplete); }
        r.end()?;
        Ok(Self { revision, canonical: Rc::from(canonical) })
    }

    pub(super) fn check(&self, path: &Path, plan: &FileShutdownPlan, plan_bytes: &[u8],
        max_bytes: usize, current: &codec::Restored, canonical: &[u8]) -> Result<(), Error>
    {
        // Both images use the SAME independently selected native configuration.
        // Decoding retains no current-session success and executes no domain.
        let old = codec::decode(path, plan, plan_bytes, max_bytes, 0, &self.canonical)?;
        if old.revision != self.revision { return Err(Error::Binding); }
        if current.revision < old.revision || current.visits.len() < old.visits.len() {
            return Err(Error::Stale);
        }
        for (before, after) in old.visits.iter().zip(&current.visits) {
            if before.domain != after.domain || before.kind != after.kind {
                return Err(Error::Binding);
            }
            // Only an Entered result may acquire a completion. Acknowledged
            // successes AND failures are immutable; appending visits cannot
            // disguise a rewrite of their earlier outcome, tick or identity.
            if before.result != ShutdownVisitResult::Entered && before.result != after.result {
                return Err(Error::Binding);
            }
        }
        for (index, (domain, slot)) in plan.domains.iter().zip(&current.campaign.slots).enumerate() {
            let events = journal::decode(&domain.profile, &domain.directory, &slot.head)?;
            // Require EACH latest independently retained domain prefix, not just
            // its initial registration or a counter-equal substitute.
            old.campaign.check_prefix(index, &domain.profile, &domain.directory, &events)?;
        }
        if current.revision == old.revision && canonical != self.canonical.as_ref() {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

impl FileShutdownCoordinator {
    /// Capture only this healthy owner's acknowledged image. Do not read newer
    /// possibly unacknowledged disk bytes or export an entered live operation.
    pub fn history_anchor(&self) -> Result<FileShutdownCoordinatorAnchor, JournalError> {
        if self.unavailable { return Err(JournalError::Unavailable); }
        Ok(FileShutdownCoordinatorAnchor { revision: self.revision,
            canonical: self.encoded(self.revision)?.into() })
    }

    /// Advance custody only along the original retained visit/head extension.
    /// External retention is separate; keep the previous archive until the new
    /// one is durably acknowledged. Neither capture nor comparison writes files.
    pub fn history_anchor_after(&self, previous: &FileShutdownCoordinatorAnchor)
        -> Result<FileShutdownCoordinatorAnchor, JournalError>
    {
        let next = self.history_anchor()?;
        let current = codec::decode(self.store.identity(), &self.campaign.plan,
            &self.plan_bytes, self.max_bytes, self.revision, &next.canonical)?;
        previous.check(self.store.identity(), &self.campaign.plan, &self.plan_bytes,
            self.max_bytes, &current, &next.canonical)?;
        Ok(next)
    }

    /// Check exact independently retained coordinator history as well as the
    /// original plan, capacity and revision floor BEFORE staged-file cleanup.
    /// Genuine pending completions/new visits are accepted; finalized rewrites,
    /// dropped visits and changed domain prefixes refuse. No domain is opened.
    /// Reopening starts unobserved exactly as ordinary open does.
    pub fn open_anchored(directory: impl AsRef<Path>, plan: &FileShutdownPlan,
        max_bytes: usize, minimum_revision: u64, anchor: &FileShutdownCoordinatorAnchor)
        -> Result<Self, JournalError>
    {
        Self::open_with_anchor(directory.as_ref(), plan, max_bytes, minimum_revision, Some(anchor))
    }
}

#[cfg(test)]
mod tests;
