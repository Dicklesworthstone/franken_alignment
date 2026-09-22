//! Terminal-only transactions over the existing journal publication endpoint.
//! No network or independently visible filesystem effect is replayed here.

mod cancellation;

use super::super::{Event, FileDelivery, FileStopSweep, JournalError, StopRequest, Transition};
use crate::action::ElapsedTick;

impl FileDelivery {
    /// Permanently stop admission and drain the original endpoint obligations
    /// with one canonical journal replacement.
    ///
    /// The original Stop and StopProgress reducers run in a private RAM
    /// projection. Undispatched reservations can be cancelled; dispatched
    /// charges are released only by accepted endpoint nonexecution evidence.
    /// Executed outcomes remain charged, and unresolved outcomes remain explicit
    /// in the returned sweep. No request is resent.
    ///
    /// Both events may spend an explicitly installed recovery reserve. This
    /// operation does not append an ordinary Time or Sweep event, so exhaustion
    /// of ordinary admission capacity does not prevent terminal recovery when
    /// the two recovery records still fit. Logical capacity is not reserved
    /// physical disk space.
    ///
    /// A fresh trusted elapsed observation is required. A stale observation,
    /// stale predecessor, rejected stop request or insufficient capacity leaves
    /// the canonical state unchanged. In particular, a rejected drain does not
    /// acknowledge a local stop: use request_stop alone when a trusted clock is
    /// unavailable and immediate, separate local-stop acknowledgment is needed.
    ///
    /// Storage failure returns no candidate receipt or refund and makes this
    /// owner unavailable until exclusive reopen. Even an exact StopRequest retry
    /// still requires the current journal revision and a valid new observation:
    /// this method advances drain progress rather than returning cached progress.
    /// It never reopens intake after an acknowledged stop.
    pub fn stop_and_drain(
        &mut self,
        revision: u64,
        request: StopRequest,
        observed_tick: ElapsedTick,
    ) -> Result<FileStopSweep, JournalError> {
        let mut transitions = self.commit_request_events(
            revision,
            &[Event::Stop(request), Event::StopProgress(observed_tick)],
        )?;
        match transitions.pop() {
            Some(Transition::StopProgressed(sweep)) => Ok(sweep),
            _ => unreachable!("terminal transaction ends with original stop progress"),
        }
    }
}

impl FileDelivery {
    /// Recover an exclusively owned journal directly into a terminal state,
    /// fencing old permissions and draining obligations in ONE replacement.
    ///
    /// Unlike open_reconciled, this terminal-only operation does not require
    /// ordinary clock/query capacity. Its Stop, Fence and StopProgress records
    /// can use the existing three-event terminal recovery reserve. It does not
    /// replenish that finite reserve or reopen intake when recovery succeeds.
    ///
    /// Stop precedes Fence intentionally: the caller's StopRequest is checked
    /// against the independently replayed canonical controller, not against a
    /// silently rewritten post-fence sequence or epoch. Exact retries of an
    /// already recorded stop preserve its original receipt. The subsequent Fence
    /// still advances the current dispatcher generation on every successful
    /// reopen, including recovery after an ambiguous prior terminal replacement.
    ///
    /// The original stop reducer withdraws unsent work, and the original endpoint
    /// supplies all execution/nonexecution evidence. Missing or expired evidence
    /// remains an unresolved charged liability. No saved tick is current time,
    /// no old process-local key is accepted, and no effect is resent.
    ///
    /// Rejected preconditions, stale observations and insufficient capacity
    /// leave the canonical history unchanged. A failed replacement exposes no
    /// writable owner or candidate sweep. This is exclusively the existing
    /// journal-as-publication profile, not a remote-provider transaction.
    pub fn open_stopped(
        directory: impl AsRef<std::path::Path>,
        profile: super::super::FileDeliveryProfile,
        request: StopRequest,
        observed_tick: ElapsedTick,
    ) -> Result<(Self, FileStopSweep), JournalError> {
        profile.limits.check()?;
        let store = super::super::storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.limits.bytes)?;
        let events = super::super::codec::decode(&profile, store.identity(), &bytes)?;
        let machine = super::super::Machine::replay(&profile, &events)?;
        store.confirm_and_cleanup()?;
        let mut host = Self {
            profile,
            store,
            events,
            machine,
            issuer: std::rc::Rc::new(()),
            fault: None,
        };
        let mut transitions = host.commit_request_events(
            host.revision(),
            &[
                Event::Stop(request),
                Event::Fence,
                Event::StopProgress(observed_tick),
            ],
        )?;
        let sweep = match transitions.pop() {
            Some(Transition::StopProgressed(sweep)) => sweep,
            _ => unreachable!("terminal recovery ends with original stop progress"),
        };
        Ok((host, sweep))
    }
}

impl FileDelivery {
    /// Enumerate retained durable request identities at one acknowledged cut.
    ///
    /// This supervisor-only recovery view includes refused admissions and all
    /// terminal outcomes. It exposes neither payloads nor permits. Rows are in
    /// ascending request-ID order, NOT dispatch or submission order. Continue
    /// with the last returned request as `after`; an empty page ends the scan.
    /// An arbitrary exclusive cursor, including u64::MAX, is valid.
    ///
    /// Every page must use the SAME journal revision. A concurrent/intervening
    /// transition rejects a continuation instead of silently skipping a newly
    /// inserted lower ID or mixing old and new dispositions. Restart changes
    /// the revision, so begin a new scan after exclusive reopen. These are
    /// historical statuses, not fresh clock observations or authority to resend.
    ///
    /// Reading consumes no journal/recovery capacity and requires no clock. A
    /// poisoned owner refuses even if its in-memory cut is still readable.
    pub fn request_status_page(
        &self,
        revision: u64,
        after: Option<u64>,
        limit: usize,
    ) -> Result<Vec<super::FileRequestStatus>, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        if revision != self.revision() {
            return Err(crate::Error::Stale.into());
        }
        if limit == 0 || limit > super::MAX_FILE_REQUESTS {
            return Err(crate::Error::Limit.into());
        }
        let mut page = Vec::new();
        page.try_reserve_exact(limit.min(self.machine.requests.len()))
            .map_err(|_| crate::Error::Limit)?;
        let start = after.map_or(std::ops::Bound::Unbounded, std::ops::Bound::Excluded);
        for (_, row) in self.machine.requests.records
            .range((start, std::ops::Bound::Unbounded)).take(limit)
        {
            page.push(row.status);
        }
        Ok(page)
    }

    /// Discover outstanding requests after a supervisor loses its own ID list.
    ///
    /// This projects the ORIGINAL request book and ledger at one revision; no
    /// second durable queue, saved authority or caller-asserted outcome exists.
    /// Undispatched work and all unknown liabilities remain visible, including
    /// irrecoverably unknown work. Refused, denied, cancelled and receipt-backed
    /// terminal work is omitted. Discovery never dispatches, cancels or refunds.
    /// In particular, an unknown entry is not a retry opportunity.
    pub fn pending_request_statuses(
        &self,
        revision: u64,
    ) -> Result<Vec<super::FileRequestStatus>, JournalError> {
        let mut statuses = self.request_status_page(revision, None, super::MAX_FILE_REQUESTS)?;
        statuses.retain(|status| matches!(status.disposition,
            super::FileRequestDisposition::Admitted {
                stage: crate::action::ActionState::Proposed
                    | crate::action::ActionState::Prepared
                    | crate::action::ActionState::Reviewing
                    | crate::action::ActionState::Authorized
                    | crate::action::ActionState::Dispatching
                    | crate::action::ActionState::Unknown
                    | crate::action::ActionState::IrrecoverablyUnknown,
                ..
            }
        ));
        Ok(statuses)
    }
}
