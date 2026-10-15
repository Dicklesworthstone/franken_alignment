//! Selected source custody follows the ORIGINAL driver, not an optional step.
//! This process-local composition is not a persisted deployment flag or watcher.
use super::{FileOversight, FileSupervisedDriver, JournalError};
use super::super::mediation::FileMediationObserver;
use super::super::mediation::source::{FileTopologySource, TopologyRefresh,
    TopologySourceRecovery, TOPOLOGY_REFRESH_READ_BYTES};
use crate::action::consequence::delivery::TopologyChange;
use crate::perimeter_inventory::topology::LoadedAuthorityTopology;
use crate::Error;

pub mod checkpoint;

/// An interrupted source operation is not a completed native refusal. The
/// original custodian additionally retains its last source/checker diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriverTopologyOutcome {
    Interrupted,
    Finished(Result<TopologyRefresh, JournalError>),
}

/// Supervisor-only historical observation, not a current certificate or key.
/// None request/attempt identifies an explicit pre-admission refresh. A skipped
/// receipt/reconciliation does not erase or relabel the last actual observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriverTopologyObservation {
    pub request: Option<u64>,
    pub attempt: Option<u64>,
    pub started_at_revision: u64,
    pub outcome: DriverTopologyOutcome,
}

/// One original source, with fixed paths, expectation, budgets and work. Once
/// installed, every driver stepping API consults it at first publication. There
/// is no mutable source getter, clear-selection operation or per-step opt-out.
/// Releasing the driver returns this SAME value with its spent work and report.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::FileDriverTopology;
/// fn duplicate(selected: FileDriverTopology) { let _ = selected.clone(); }
/// ```
#[derive(Debug)]
#[must_use = "install or retain the original topology-source custody"]
pub struct FileDriverTopology {
    source: FileTopologySource,
    last: Option<DriverTopologyObservation>,
}
impl FileDriverTopology {
    pub fn new(source: FileTopologySource) -> Self { Self { source, last: None } }
    pub fn source(&self) -> &FileTopologySource { &self.source }
    pub fn last_observation(&self) -> Option<&DriverTopologyObservation> { self.last.as_ref() }

    pub(super) fn refresh_for_publication(&mut self, host: &mut FileOversight,
        request: u64, attempt: u64) -> Result<(), JournalError>
    {
        let revision = host.revision();
        self.refresh(host, revision, Some((request, attempt))).map(|_| ())
    }

    fn refresh(&mut self, host: &mut FileOversight, revision: u64,
        target: Option<(u64, u64)>) -> Result<TopologyRefresh, JournalError>
    {
        // Caller/owner mistakes spend no work and leave the previous report.
        self.source.check_driver_binding(host, revision)?;
        self.last = Some(DriverTopologyObservation {
            request: target.map(|(request, _)| request),
            attempt: target.map(|(_, attempt)| attempt),
            started_at_revision: revision, outcome: DriverTopologyOutcome::Interrupted,
        });
        let result = self.source.refresh_next(host, revision);
        self.last.as_mut().expect("installed before original source work").outcome =
            DriverTopologyOutcome::Finished(result.clone());
        result
    }
}

/// A rejected setup returns the exact original custody for a corrected retry.
/// The existing selection, owner, journal and source allowances are untouched.
#[derive(Debug)]
pub struct DriverTopologySetupError {
    pub error: JournalError,
    pub topology: FileDriverTopology,
}

impl FileSupervisedDriver {
    /// Explicitly select automatic fresh topology checks while no review/job
    /// is active. This validates binding only: it does not read, certify or
    /// grant an actor request. Use refresh_topology before initial admission
    /// when the original graph has not yet been certified.
    pub fn require_topology(&mut self, revision: u64, topology: FileDriverTopology)
        -> Result<(), Box<DriverTopologySetupError>>
    {
        let admitted = (|| {
            if self.topology.is_some() { return Err(Error::Duplicate.into()); }
            if self.job.is_some() || self.learned_review.is_some() {
                return Err(Error::WrongState.into());
            }
            let host = self.supervisor.host()?;
            topology.source.check_driver_binding(&host, revision)
        })();
        if let Err(error) = admitted { return Err(Box::new(DriverTopologySetupError { error, topology })); }
        self.topology = Some(topology);
        Ok(())
    }

    /// Reconnect independently retained source work to this idle recovered
    /// driver, then perform a real native source refresh. The original source
    /// is installed BEFORE reading: refusal/error/unwind cannot discard spent
    /// work or silently turn the next step into an unselected publication.
    ///
    /// Allowance must cover both this initial check and a final publication
    /// check. This is admission, not a reservation against other later refreshes.
    /// The caller supplies current native clock state as for refresh_topology;
    /// final publication retains its original post-read clock/deadline checks.
    /// Ok with a refusal is NOT certification. No review, human key, dispatch or
    /// endpoint effect is manufactured here. Resumption is not disk atomicity:
    /// a completed rebind may precede a failed read or failed durable withdrawal.
    pub fn resume_topology(&mut self, revision: u64, operation: u64,
        observer: FileMediationObserver, recovery: TopologySourceRecovery)
        -> Result<TopologyRefresh, JournalError>
    {
        if self.topology.is_some() { return Err(Error::Duplicate.into()); }
        if self.job.is_some() || self.learned_review.is_some() {
            return Err(Error::WrongState.into());
        }
        let limits = recovery.previous.source_limits();
        if recovery.work.checks.checked_add(2).is_none_or(|checks| checks > limits.checks)
            || recovery.work.reserved_read_bytes.checked_add(2 * TOPOLOGY_REFRESH_READ_BYTES)
                .is_none_or(|bytes| bytes > limits.read_bytes) {
            return Err(Error::Limit.into());
        }
        if operation > u64::MAX - 2 { return Err(Error::Overflow.into()); }
        let source = {
            let mut host = self.supervisor.host_mut()?;
            observer.resume_topology_source(&mut host, revision, operation, recovery)?
        };
        // Infallible custody installation follows the original acknowledged
        // rebind; only then may the fallible source/checker pipeline run.
        self.topology = Some(FileDriverTopology::new(source));
        let revision = self.supervisor.host()?.revision();
        self.refresh_topology(revision)
    }

    pub fn topology(&self) -> Option<&FileDriverTopology> { self.topology.as_ref() }

    /// An explicit supervisory refresh uses the SAME custodian and counters as
    /// automatic first publication. It is not a new source or budget allowance.
    /// Known loss uses native revocation; dispatched uncertainty stays charged.
    pub fn refresh_topology(&mut self, revision: u64) -> Result<TopologyRefresh, JournalError> {
        let topology = self.topology.as_mut().ok_or(Error::Missing)?;
        let mut host = self.supervisor.host_mut()?;
        topology.refresh(&mut host, revision, None)
    }

    /// Independently provision newer topology through the ORIGINAL revoking
    /// update. Never adopt changed files automatically or replace the source.
    /// Paths, checker allowance, source limits and spent work remain unchanged.
    /// The next publication still performs a fresh check of those actual files.
    pub fn rebind_topology(&mut self, revision: u64, operation: u64,
        expected: LoadedAuthorityTopology) -> Result<Option<TopologyChange>, JournalError>
    {
        let topology = self.topology.as_mut().ok_or(Error::Missing)?;
        let mut host = self.supervisor.host_mut()?;
        topology.source.rebind(&mut host, revision, operation, expected)
    }
}

#[cfg(test)]
mod tests;
