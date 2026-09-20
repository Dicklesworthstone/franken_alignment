//! Terminal composition for the ORIGINAL full-input, mandatory two-key owner.
//! Replay is memory-only; the existing canonical journal is the sole effect sink.
use super::super::{
    BaseEvent, Event, FileOversight, FileOversightProfile, FileStopSweep, JournalError,
    JournalFailure, JournalIo, Machine, StopRequest, Transition, journal, storage,
};
use crate::Error;
use crate::action::ElapsedTick;
use std::path::Path;

impl FileOversight {
    /// Stop admission, withdraw native review/human keys and drain original
    /// endpoint obligations in ONE canonical replacement. Executed and expired
    /// unresolved effects remain charged; only original receipts release charges.
    /// This never acquires helper evidence, issues a key or resends an effect.
    ///
    /// Both original events may use the installed recovery reserve, even when
    /// ordinary observation/query capacity is exhausted. Every encoded prefix
    /// still satisfies the original event AND byte limits. The reserve is finite
    /// logical capacity, not reserved disk space or unlimited restart capacity.
    ///
    /// A rejected stop request, stale predecessor/tick or insufficient capacity
    /// leaves the canonical history unchanged, INCLUDING its admission state.
    /// Use request_stop separately when immediate stop acknowledgment is needed
    /// without a trusted clock. Storage failure or unwind makes this owner
    /// unavailable and returns no candidate sweep, receipt or refund.
    pub fn stop_and_drain(
        &mut self,
        revision: u64,
        request: StopRequest,
        observed_tick: ElapsedTick,
    ) -> Result<FileStopSweep, JournalError> {
        terminal_transaction(self, revision, request, observed_tick, false)
    }

    /// Exclusively recover directly into a stopped, fenced and drained state.
    /// Only the ORIGINAL Stop -> Fence -> StopProgress events are appended, in
    /// one replacement. The stop preconditions refer to the recovered controller
    /// BEFORE fencing; they are not silently rewritten for a different epoch.
    ///
    /// Canonical decoding/replay preserves every stored optional guard, source
    /// floor, human policy and original outcome. No current producer, helper or
    /// reviewer is needed. Saved time is history: observed_tick must be a fresh
    /// trusted observation in profile.delivery.clock_domain.
    ///
    /// A new process-local identity and the original recovery fence invalidate
    /// old automatic, human and reviewer handles. No reviewer role is returned,
    /// intake remains stopped and unresolved liabilities remain explicit. This
    /// is not an OS/process stop or a remote-provider recovery transaction.
    /// The canonical store and its nonrollback protection remain trusted.
    pub fn open_stopped(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        request: StopRequest,
        observed_tick: ElapsedTick,
    ) -> Result<(Self, FileStopSweep), JournalError> {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        let machine = Machine::replay(&profile, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, _reviewer) = Self::owner(profile, store, events, machine);
        let sweep = host.finish_stopped_recovery(request, observed_tick)?;
        Ok((host, sweep))
    }

    // Shared only inside the original durable owner. Callers must retain the
    // exclusive Store and never expose the unfenced replay owner or its roles.
    pub(in super::super) fn finish_stopped_recovery(
        &mut self, request: StopRequest, observed_tick: ElapsedTick,
    ) -> Result<FileStopSweep, JournalError> {
        terminal_transaction(self, self.revision(), request, observed_tick, true)
    }
}

// No public event importer and no general batch-authorization path. Only these
// two fixed terminal sequences can reach this cut. Original Machine::apply also
// enforces source/decoder/consistency/authority laws during replay.
fn terminal_transaction(
    host: &mut FileOversight,
    revision: u64,
    request: StopRequest,
    tick: ElapsedTick,
    recover: bool,
) -> Result<FileStopSweep, JournalError> {
    if host.fault.is_some() {
        return Err(JournalError::Unavailable);
    }
    if revision != host.revision() {
        return Err(Error::Stale.into());
    }
    let mut next = Vec::with_capacity(3);
    next.push(Event::Core(BaseEvent::Stop(request)));
    if recover {
        next.push(Event::Core(BaseEvent::Fence));
    }
    next.push(Event::Core(BaseEvent::StopProgress(tick)));
    let count = host.events.len().checked_add(next.len()).ok_or(Error::Overflow)?;
    if count > host.profile.delivery.limits.events {
        return Err(Error::Limit.into());
    }
    let mut history = Vec::new();
    history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    // Preflight the first new record before retaining a cloned history.
    let mut bytes = journal::encode_appended(
        &host.profile, host.store.identity(), &host.events, &next[0],
    )?;
    history.extend(host.events.iter().cloned());
    for (index, event) in next.iter().enumerate() {
        host.check_source_admission(event)?;
        if index != 0 {
            bytes = journal::encode_appended(
                &host.profile, host.store.identity(), &history, event,
            )?;
        }
        history.push(event.clone());
    }
    let mut candidate = Machine::replay(&host.profile, &host.events)?;
    let mut sweep = None;
    for event in &next {
        candidate.preflight_consistency(event)?;
        if let Transition::StopProgressed(result) = candidate.apply(event)? {
            sweep = Some(result);
        }
    }
    let sweep = sweep.ok_or(Error::Incomplete)?;
    // Poison BEFORE replacement, including caught unwinds. Inspect remains the
    // last acknowledged cut; a newer disk image may already be visible.
    host.fault = Some(JournalFailure {
        operation: JournalIo::Stage,
        kind: std::io::ErrorKind::Other,
        replacement_may_be_visible: true,
    });
    if let Err(error) = host.store.replace(&bytes) {
        if let JournalError::Io(failure) = &error {
            host.fault = Some(failure.clone());
        }
        return Err(error);
    }
    for event in &next {
        host.source_operation_committed(event);
    }
    host.events = history;
    host.machine = candidate;
    host.fault = None;
    Ok(sweep)
}

#[cfg(test)]
mod tests;
