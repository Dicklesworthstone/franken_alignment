//! Clock-independent admission stop, followed by an explicitly separate drain.
//! Both consume the original verified cursor; neither returns a live owner.
use super::*;
use super::super::{FileDeliverySnapshot, FileStopSweep, StopProgress};

/// An acknowledged admission stop and the exact history retained at that cut.
/// This is NOT a drain receipt: sent/unknown/executed effects remain charged
/// until the original endpoint protocol settles them. No synthetic sweep exists.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::FileAdmissionStoppedHistory;
/// fn authorize(stop: FileAdmissionStoppedHistory) -> FilePermit { stop }
/// ```
#[must_use = "Admission stopped does not mean all external obligations drained."]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAdmissionStoppedHistory {
    snapshot: FileDeliverySnapshot,
    progress: StopProgress,
    anchor: FileHistoryAnchor,
    appended_events: usize,
}
impl FileAdmissionStoppedHistory {
    pub fn snapshot(&self) -> &FileDeliverySnapshot { &self.snapshot }
    pub fn progress(&self) -> &StopProgress { &self.progress }
    pub fn history_anchor(&self) -> &FileHistoryAnchor { &self.anchor }
    /// Two for a new Stop/Fence; zero for a matching already-stopped image.
    /// Zero does not assert a new fence or a new time observation.
    pub fn appended_events(&self) -> usize { self.appended_events }
}

impl FileStopRecovery {
    // Sealed recovery handoff only. The caller has completed original replay
    // and rechecked its FULL independent requirements. Keep the SAME machine,
    // canonical image and exclusive Store; never reopen or replay the history.
    // The resulting cursor is consumed immediately by a fixed terminal method,
    // not exposed to callers or used as a registered fleet observation.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn from_verified_terminal(profile: FileOversightProfile, store: storage::Store,
        canonical: Vec<u8>, events: Vec<Event>, machine: Machine) -> Self
    {
        let count = events.len();
        Self { profile, store, canonical, events, machine, last_drain: None,
            replayed: count, attempted: count, status: FileStopRecoveryStatus::Ready }
    }

    /// Stop admission and withdraw old dispatch eligibility without acquiring a
    /// clock, publishing an effect or claiming settlement of unknown effects.
    /// A new stop appends only ORIGINAL Stop/Fence in one replacement. Existing
    /// matching stops are confirmed unchanged, even when not drained or full.
    /// No live role escapes; a later explicit drain must reopen/replay the cut.
    ///
    /// Every original encoded prefix, source-admission and consistency check
    /// applies. Invalid input/time-independent preflight leaves staging intact.
    /// A failed replacement may already be visible; never treat it as a receipt.
    pub fn finish_admission_stop(self, request: StopRequest)
        -> Result<FileAdmissionStoppedHistory, JournalError>
    {
        self.check_terminal_request(request)?;
        if self.machine.snapshot(self.events.len()).stop.is_some() {
            let (snapshot, progress, anchor) = self.confirm_existing_stop()?;
            return Ok(FileAdmissionStoppedHistory { snapshot, progress, anchor, appended_events: 0 });
        }
        let committed = self.apply_stop_tail(StopTail::Admission(request))?;
        Ok(FileAdmissionStoppedHistory { snapshot: committed.snapshot,
            progress: committed.progress, anchor: committed.anchor, appended_events: 2 })
    }

    /// Drain an ALREADY stopped domain with one original StopProgress event.
    /// Does not stop an active domain, append a redundant recovery fence, reopen
    /// intake, restore a key, or resend a publication. This preserves the final
    /// event of a three-event terminal reserve after an admission Stop/Fence.
    ///
    /// The exact original stop identity and canonical image must match before
    /// calling the fallible clock. Already-drained retries need no clock or write,
    /// including at capacity. The clock is requested only after complete replay;
    /// its result is checked by the original time/endpoint implementation.
    pub fn finish_stopped_drain<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.check_ready()?;
        if self.machine.snapshot(self.events.len()).stop.is_none() {
            return Err(Error::WrongState.into());
        }
        self.check_terminal_request(request)?;
        if self.machine.broker.stop_progress()?.drained() {
            let (snapshot, progress, anchor) = self.confirm_existing_stop()?;
            return Ok(FileStoppedHistory::new(
                FileStoppedRecovery::AlreadyDrained { snapshot, progress }, anchor));
        }
        if self.events.len() >= self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        self.check_canonical_stop_image()?;
        let tick = clock()?;
        let committed = self.apply_stop_tail(StopTail::Drain(tick))?;
        // The fixed Drain tail requires this result before any storage change.
        let sweep = committed.sweep.expect("validated original StopProgress transition");
        Ok(FileStoppedHistory::new(FileStoppedRecovery::Advanced {
            snapshot: committed.snapshot, sweep,
        }, committed.anchor))
    }

    fn check_terminal_request(&self, request: StopRequest) -> Result<(), JournalError> {
        let expected = self.stop_request(request.operation)?;
        if expected != request {
            return Err(if self.machine.snapshot(self.events.len()).stop.is_some() {
                Error::Binding
            } else { Error::Stale }.into());
        }
        Ok(())
    }

    fn check_canonical_stop_image(&self) -> Result<(), JournalError> {
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

    fn confirm_existing_stop(self)
        -> Result<(FileDeliverySnapshot, StopProgress, FileHistoryAnchor), JournalError>
    {
        let snapshot = self.machine.snapshot(self.events.len());
        let progress = self.machine.broker.stop_progress()?;
        self.check_canonical_stop_image()?;
        self.store.confirm_and_cleanup()?;
        let anchor = FileHistoryAnchor::from_acknowledged(self.events.len(), self.canonical);
        Ok((snapshot, progress, anchor))
    }

    // This private consumer accepts only the two FIXED terminal sequences below.
    // No general event importer, reducer, authority path or storage format exists.
    fn apply_stop_tail(self, tail: StopTail) -> Result<CommittedStop, JournalError> {
        self.check_canonical_stop_image()?;
        let Self { profile, store, canonical, events, machine, .. } = self;
        let (mut host, _reviewer) = FileOversight::owner(profile, store, events, machine);
        let next = match tail {
            StopTail::Admission(request) => vec![Event::Core(BaseEvent::Stop(request)), Event::Core(BaseEvent::Fence)],
            StopTail::Drain(tick) => vec![Event::Core(BaseEvent::StopProgress(tick))],
        };
        let count = host.events.len().checked_add(next.len()).ok_or(Error::Overflow)?;
        if count > host.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let mut history = Vec::new();
        history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        history.extend(host.events.iter().cloned());
        let mut bytes = Vec::new();
        for event in &next {
            host.check_source_admission(event)?;
            bytes = journal::encode_appended(&host.profile, host.store.identity(), &history, event)?;
            history.push(event.clone());
        }
        // The owner is private and consumed: there is no observer of speculative
        // memory and no second historical replay. All reports allocate before I/O.
        let mut sweep = None;
        for event in &next {
            host.machine.preflight_consistency(event)?;
            if let Transition::StopProgressed(result) = host.machine.apply(event)? { sweep = Some(result); }
        }
        if matches!(tail, StopTail::Drain(_)) && sweep.is_none() { return Err(Error::Incomplete.into()); }
        let snapshot = host.machine.snapshot(count);
        let progress = host.machine.broker.stop_progress()?;
        if host.store.read(host.profile.delivery.limits.bytes)? != canonical { return Err(Error::Binding.into()); }
        host.store.confirm_and_cleanup()?;
        host.store.replace(&bytes)?;
        // No owner/role or speculative anchor escapes on failure or unwind.
        let anchor = FileHistoryAnchor::from_acknowledged(count, bytes);
        Ok(CommittedStop { snapshot, progress, sweep, anchor })
    }
}

#[derive(Clone, Copy)]
enum StopTail { Admission(StopRequest), Drain(ElapsedTick) }
struct CommittedStop {
    snapshot: FileDeliverySnapshot,
    progress: StopProgress,
    sweep: Option<FileStopSweep>,
    anchor: FileHistoryAnchor,
}

#[cfg(test)]
mod tests;
