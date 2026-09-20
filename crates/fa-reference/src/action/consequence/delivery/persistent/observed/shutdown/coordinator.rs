//! Persist visit intent before driving an ORIGINAL domain owner.
//! This journal owns retry/evidence progress, never effect rights or live roles.
mod codec;
mod recovery;
#[cfg(test)]
mod tests;

use super::*;
use super::super::super::{storage, MAX_JOURNAL_BYTES};
use crate::action::consequence::delivery::persistent::requests::actor::FileActorSupervisor;

const MAX_REFUSAL_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownVisitKind {
    Advance { at: ElapsedTick },
    /// Exclusive terminal recovery of an independently registered domain.
    RecoverStopped { at: ElapsedTick },
    InspectCanonical,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShutdownVisitResult {
    /// Durable intent, with no acknowledged coordinator completion. The domain
    /// may already have stopped/drained; a fresh original inspection resolves it.
    Entered,
    /// Historical bookkeeping only. Never restored as a current stop observation.
    Observed { domain_revision: u64 },
    /// Original error display retained as data, not a reconstructed native error.
    Refused { diagnostic: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShutdownVisit {
    pub domain: u64,
    pub kind: ShutdownVisitKind,
    pub result: ShutdownVisitResult,
}

/// Every durable visit survives restart. Current-session observations are kept
/// separate and restart unobserved. Unavailability dominates previous success.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownCoordinatorReport {
    pub revision: u64,
    pub visits: Vec<ShutdownVisit>,
    pub current_session: FileShutdownReport,
    pub unavailable: bool,
}
impl FileShutdownCoordinatorReport {
    pub fn all_observed_drained(&self) -> bool {
        !self.unavailable && self.current_session.all_observed_drained()
    }
}

/// Exclusive write-ahead driver. No owner extraction, clone, actor interface,
/// worker callback or role getter exists. Ordinary visits borrow an existing
/// domain/supervisor. Explicit terminal recovery exclusively opens only the
/// registered domain, preserves its history, and returns no owner or live role.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::FileShutdownCoordinator;
/// fn duplicate(owner: FileShutdownCoordinator) { let _ = owner.clone(); }
/// ```
pub struct FileShutdownCoordinator {
    store: storage::Store,
    campaign: FileShutdownCampaign,
    plan_bytes: Vec<u8>,
    max_bytes: usize,
    revision: u64,
    visits: Vec<ShutdownVisit>,
    unavailable: bool,
    acknowledged: FileShutdownCoordinatorReport,
}
impl fmt::Debug for FileShutdownCoordinator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileShutdownCoordinator").field("revision", &self.revision)
            .field("unavailable", &self.unavailable).finish_non_exhaustive()
    }
}

impl FileShutdownCoordinator {
    /// Create a separate control-operation store using the existing owner lock,
    /// sync and canonical replacement protocol. Existing directories refuse.
    /// The entire plan and image must fit the original Store's byte ceiling.
    pub fn create(directory: impl AsRef<Path>, plan: &FileShutdownPlan, max_bytes: usize)
        -> Result<Self, JournalError>
    {
        Self::check_limit(max_bytes)?;
        let plan_bytes = plan.encode()?;
        // Preflight the content before creating any storage. The actual canonical
        // path is also checked/encoded before its first replacement below.
        let campaign = plan.start();
        codec::encode(directory.as_ref(), &plan_bytes, max_bytes, 0, &[], &campaign)?;
        let store = storage::Store::create(directory.as_ref())?;
        let mut owner = Self::assemble(store, campaign, plan_bytes, max_bytes, 0, Vec::new());
        let bytes = owner.encoded(0)?;
        owner.store.replace(&bytes)?;
        owner.unavailable = false;
        Ok(owner)
    }

    /// Exact plan, capacity, path and independent revision floor are required.
    /// Saved heads keep the newer prefix comparisons; saved successful visits
    /// do NOT initialize current success. Offline domains need not be opened.
    /// A pending visit is retained and charged to the original visit allowance.
    pub fn open(directory: impl AsRef<Path>, plan: &FileShutdownPlan,
        max_bytes: usize, minimum_revision: u64) -> Result<Self, JournalError>
    {
        Self::check_limit(max_bytes)?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(max_bytes)?;
        let plan_bytes = plan.encode()?;
        let restored = codec::decode(store.identity(), plan, &plan_bytes, max_bytes, minimum_revision, &bytes)?;
        // Validation precedes cleanup. Never promote a staged coordinator image.
        store.confirm_and_cleanup()?;
        let mut owner = Self::assemble(store, restored.campaign, plan_bytes, max_bytes,
            restored.revision, restored.visits);
        owner.unavailable = false;
        Ok(owner)
    }

    fn assemble(store: storage::Store, campaign: FileShutdownCampaign, plan_bytes: Vec<u8>,
        max_bytes: usize, revision: u64, visits: Vec<ShutdownVisit>) -> Self
    {
        let acknowledged = FileShutdownCoordinatorReport { revision, visits: visits.clone(),
            current_session: campaign.report(), unavailable: false };
        Self { store, campaign, plan_bytes, max_bytes, revision, visits,
            unavailable: true, acknowledged }
    }
    fn check_limit(limit: usize) -> Result<(), Error> {
        if limit == 0 || limit > MAX_JOURNAL_BYTES { Err(Error::Limit) } else { Ok(()) }
    }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn report(&self) -> FileShutdownCoordinatorReport {
        let mut report = self.acknowledged.clone();
        report.unavailable = self.unavailable;
        report
    }

    /// Outer error: coordinator completion is unacknowledged. Inner error: the
    /// original domain refused and that refusal was durably recorded. Neither
    /// can be interpreted as evidence of effect nonexecution or an automatic retry.
    pub fn advance(&mut self, revision: u64, domain: u64, host: &mut FileOversight, at: ElapsedTick)
        -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    {
        let (index, attempt) = self.begin(revision, domain, ShutdownVisitKind::Advance { at })?;
        let result = self.campaign.advance_inner(index, attempt, host, at);
        self.finish(index, attempt, result)
    }

    /// Write intent BEFORE borrowing the original supervisor, so a failed
    /// coordinator write cannot withdraw its unused actor admission snapshot.
    pub fn advance_supervised(&mut self, revision: u64, domain: u64,
        supervisor: &mut FileActorSupervisor<FileOversight>, at: ElapsedTick)
        -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    {
        let (index, attempt) = self.begin(revision, domain, ShutdownVisitKind::Advance { at })?;
        let result = match supervisor.host_mut() {
            Ok(mut host) => self.campaign.advance_inner(index, attempt, &mut host, at),
            Err(error) => Err(error),
        };
        self.finish(index, attempt, result)
    }

    pub fn inspect_canonical(&mut self, revision: u64, domain: u64)
        -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    {
        let (index, attempt) = self.begin(revision, domain, ShutdownVisitKind::InspectCanonical)?;
        let result = self.campaign.read_canonical(index);
        self.finish(index, attempt, result)
    }

    /// Record only a negative operator observation. This can withdraw aggregate
    /// success, never create a stop receipt or silently omit the unavailable member.
    pub fn unavailable(&mut self, revision: u64, domain: u64, error: JournalError) -> Result<(), JournalError> {
        let (index, attempt) = self.begin(revision, domain, ShutdownVisitKind::Unavailable)?;
        let _ = self.finish(index, attempt, Err(error))?;
        Ok(())
    }

    fn begin(&mut self, revision: u64, domain: u64, kind: ShutdownVisitKind) -> Result<(usize, usize), JournalError> {
        if self.unavailable { return Err(JournalError::Unavailable); }
        if revision != self.revision { return Err(Error::Stale.into()); }
        if self.visits.len() >= self.campaign.plan.max_attempts { return Err(Error::Limit.into()); }
        self.visits.try_reserve(1).map_err(|_| Error::Limit)?;
        let step = match kind { ShutdownVisitKind::Advance { .. } => FileShutdownStep::InspectOwner,
            ShutdownVisitKind::InspectCanonical | ShutdownVisitKind::RecoverStopped { .. }
                => FileShutdownStep::InspectCanonical,
            ShutdownVisitKind::Unavailable => FileShutdownStep::Unavailable };
        let pair = self.campaign.enter(domain, step)?;
        // Poison before encoding/storage/native work. Only completion acknowledgment
        // clears it; unwinding cannot reuse the old permitting coordinator report.
        self.unavailable = true;
        self.visits.push(ShutdownVisit { domain, kind, result: ShutdownVisitResult::Entered });
        self.commit()?;
        Ok(pair)
    }

    fn finish(&mut self, index: usize, attempt: usize,
        result: Result<(FileShutdownObservation, Rc<[u8]>, usize), JournalError>)
        -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
    {
        let result = self.campaign.complete(index, attempt, result);
        let recorded = match &result {
            Ok(observation) => ShutdownVisitResult::Observed { domain_revision: observation.journal_revision },
            Err(error) => {
                let diagnostic = error.to_string();
                if diagnostic.len() > MAX_REFUSAL_BYTES { return Err(Error::Limit.into()); }
                ShutdownVisitResult::Refused { diagnostic }
            }
        };
        self.visits.last_mut().ok_or(Error::Missing)?.result = recorded;
        self.commit()?;
        self.unavailable = false;
        Ok(result)
    }

    fn encoded(&self, revision: u64) -> Result<Vec<u8>, Error> {
        codec::encode(self.store.identity(), &self.plan_bytes, self.max_bytes,
            revision, &self.visits, &self.campaign)
    }
    fn commit(&mut self) -> Result<(), JournalError> {
        self.unavailable = true;
        let revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let bytes = self.encoded(revision)?;
        // Preallocate returned reports before publishing their acknowledgment.
        let next = FileShutdownCoordinatorReport { revision, visits: self.visits.clone(),
            current_session: self.campaign.report(), unavailable: false };
        self.store.replace(&bytes)?;
        self.revision = revision;
        self.acknowledged = next;
        Ok(())
    }
}
