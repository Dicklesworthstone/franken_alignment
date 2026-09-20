//! Close the parent before stopping children; settle each original endpoint.
//!
//! This is an in-memory supervisor sweep, not an atomic remote transaction.
//! Every domain remains represented, including failed stops, missing endpoints
//! and expired retention. No failure rolls back another domain's real progress.

use super::{FundingInspection, FundingPool, FundingReturn};
use crate::action::consequence::delivery::{PublicationEndpoint, StopReceipt, StopRequest, StopSweep};
use crate::Error;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolStopRequest {
    pub operation: u64,
    /// The parent's funding/return revision, not any child's control sequence.
    pub expected_revision: u64,
}

/// Historical results of stopping admission, not endpoint nonexecution evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolStopSweep {
    pub request: PoolStopRequest,
    pub domains: BTreeMap<u64, Result<StopReceipt, Error>>,
    pub accounting: Result<FundingInspection, Error>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainDrain {
    /// The parent is still closed. No endpoint or funding-return operation ran
    /// for this child, and any reservation/charge stays in its original ledger.
    StopFailed(Error),
    Stopped {
        receipt: StopReceipt,
        endpoint: Result<StopSweep, Error>,
        /// Collection can succeed for unused rights even if the endpoint failed.
        /// It never counts unresolved or executed charges as available.
        returned: Result<FundingReturn, Error>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolDrainSweep {
    pub request: PoolStopRequest,
    pub domains: BTreeMap<u64, DomainDrain>,
    pub accounting: Result<FundingInspection, Error>,
}

impl PoolDrainSweep {
    /// At this observation only: every original endpoint acknowledged its fence
    /// and settled its sends, and collection/accounting completed. Executed
    /// effects remain charged. This diagnostic is not a capability or proof.
    pub fn drained(&self) -> bool {
        self.accounting.as_ref().is_ok_and(FundingInspection::conserved)
            && self.domains.values().all(|domain| matches!(domain,
                DomainDrain::Stopped { endpoint: Ok(sweep), returned: Ok(_), .. }
                    if sweep.progress.drained()))
    }
}

impl FundingPool {
    pub fn stop_request(&self) -> Option<PoolStopRequest> { self.stop }

    /// Permanently close ALL parent admission before attempting any child stop.
    /// No endpoint I/O occurs. Exact retries use the original operation and retry
    /// failed children; already-stopped children retain their original receipts.
    ///
    /// Closing consumes no funding revision: even revision exhaustion must not
    /// keep admission open. The named predecessor is still checked on first use.
    /// Later collection may fail on revision overflow without reopening anything.
    pub fn request_stop_all(&mut self, request: PoolStopRequest) -> Result<PoolStopSweep, Error> {
        if request.operation == 0 { return Err(Error::InvalidInput); }
        if let Some(previous) = self.stop {
            if previous != request {
                return Err(if previous.operation == request.operation { Error::Binding } else { Error::Duplicate });
            }
        } else {
            if request.expected_revision != self.revision { return Err(Error::Stale); }
            // There are no external callbacks here. A borrowed domain cannot
            // coexist with this mutable pool borrow or bypass this stop bit.
            self.stop = Some(request);
        }
        let mut domains = BTreeMap::new();
        for (authority, allocation) in &mut self.allocations {
            let broker = &mut allocation.broker;
            let result = match broker.stop_receipt() {
                Some(receipt) => Ok(receipt.clone()),
                None => {
                    let before = broker.inspect();
                    broker.request_stop(StopRequest {
                        operation: request.operation,
                        expected_control_sequence: before.sequence,
                        expected_authority_epoch: before.ledger.epoch,
                    })
                }
            };
            domains.insert(*authority, result);
        }
        Ok(PoolStopSweep { request, domains, accounting: self.inspect() })
    }

    /// Visit every allocated child, not merely the endpoints supplied by the
    /// caller. Missing or foreign endpoints are explicit per-domain failures.
    /// Original receipt reconciliation and exact-once collection stay separate.
    /// No effect is retried and no child or endpoint is automatically created.
    /// Endpoint clocks must be observed independently before this call.
    pub fn progress_stop_all(&mut self, endpoints: &mut BTreeMap<u64, PublicationEndpoint>)
        -> Result<PoolDrainSweep, Error>
    {
        let request = self.stop.ok_or(Error::Incomplete)?;
        let stopped = self.request_stop_all(request)?;
        let mut domains = BTreeMap::new();
        for (authority, result) in stopped.domains {
            let disposition = match result {
                Err(error) => DomainDrain::StopFailed(error),
                Ok(receipt) => {
                    let broker = &mut self.allocations.get_mut(&authority).expect("retained allocation").broker;
                    let endpoint = match endpoints.get_mut(&authority) {
                        Some(endpoint) => broker.progress_stop(endpoint),
                        None => Err(Error::Missing),
                    };
                    // Only original available rights can return, irrespective
                    // of this endpoint result. Preserve both outcomes on error.
                    let returned = self.collect_returned(self.revision, authority);
                    DomainDrain::Stopped { receipt, endpoint, returned }
                }
            };
            domains.insert(authority, disposition);
        }
        Ok(PoolDrainSweep { request, domains, accounting: self.inspect() })
    }
}
