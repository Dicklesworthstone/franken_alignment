//! Fresh topology at the original first-publication boundary, not a new sink.
use super::{FileOversight, FileTopologySource, JournalError, TopologyRefresh};
use super::super::super::publication::CheckedPublication;
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::CommitteeInput;
use crate::{Error, Snapshot};

/// The same explicit inputs as native checked publication, plus the independently
/// retained source operation. Neither this data nor its construction grants keys.
/// The owner must already have an acknowledged clock; a new tick is acquired
/// AFTER fresh topology work, rather than lending a saved tick to publication.
pub struct TopologyPublication<'a> {
    pub revision: u64,
    pub operation: u64,
    pub attempt: u64,
    pub current: Option<&'a CommitteeInput>,
    pub snapshot: Snapshot,
}

/// The two ordered native cuts remain separate, including on failure. A source
/// acknowledgment is not undone by a later publication refusal or storage fault.
/// None topology means the ORIGINAL endpoint classifier needed no new evidence;
/// None publication means source work failed before any publication was attempted.
/// An acknowledged source refusal can still lead to native sealed NONexecution.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::TopologyPublicationReport;
/// fn authorize(report: TopologyPublicationReport) -> FilePermit { report }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopologyPublicationReport {
    pub topology: Option<Result<TopologyRefresh, JournalError>>,
    pub publication: Option<Result<CheckedPublication, JournalError>>,
}

impl FileOversight {
    /// Reacquire complete topology evidence immediately before the ORIGINAL
    /// checked publication. Both automatic and human approvals must already have
    /// been consumed by native dispatch; this method never manufactures them.
    /// A missing/changed source withdraws coverage first, after which the native
    /// endpoint can seal nonexecution. Only native reconciliation refunds it.
    ///
    /// Already recorded outcomes, elapsed requests and receipt-only envelopes
    /// keep the original priority: no topology read, budget spend, operation-ID
    /// advance or recertification occurs for those paths. A repeated source ID
    /// cannot prevent querying an already resolved original outcome. The same
    /// owner/source issuer and current journal predecessor are still required.
    ///
    /// Mandatory policy, whole-input, witness, identity, credential and forecast
    /// checks remain in publish_checked. Any unsupported stronger route refuses;
    /// there is no weaker fallback. Source work and publication are separately
    /// durable, not a transaction over files and the endpoint. Reads and writes
    /// are synchronous; trusted file/clock/provider assumptions are unchanged.
    pub fn publish_checked_from_topology_source<F>(&mut self, source: &mut FileTopologySource,
        request: TopologyPublication<'_>, clock: F)
        -> Result<TopologyPublicationReport, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.publish_checked_from_topology_source_with(source, request, |_| clock())
    }

    // Restricted fault seam at the real post-capture clock boundary. Public
    // callers never receive a mutable host through the clock or a reader hook.
    pub(super) fn publish_checked_from_topology_source_with<F>(&mut self,
        source: &mut FileTopologySource, request: TopologyPublication<'_>, clock: F)
        -> Result<TopologyPublicationReport, JournalError>
    where F: FnOnce(&mut Self) -> ElapsedTick {
        source.observer.check_owner(self)?;
        let TopologyPublication { revision, operation, attempt, current, snapshot } = request;
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if let Some(input) = current { self.check_action(attempt, input.action())?; }
        // Do not approximate original receipt/deadline semantics by looking at
        // execution counts or treating absent evidence as a false outcome.
        let topology = if self.machine.publication_needs_evidence(attempt)? {
            let captured = source.refresh(self, revision, operation);
            if captured.is_err() {
                return Ok(TopologyPublicationReport { topology: Some(captured), publication: None });
            }
            Some(captured)
        } else { None };
        // A known source loss is already acknowledged here. Even if this clock
        // fails or publication refuses, that earlier withdrawal is not rolled back.
        let now = clock(self);
        let publication = self.publish_checked(self.revision(), attempt, current, snapshot, now);
        Ok(TopologyPublicationReport { topology, publication: Some(publication) })
    }
}
