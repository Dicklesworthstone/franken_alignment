//! Preserve exact source expenditure across one original learned stream reset.
//! These conversions do not read source files or execute numerical work.
use super::{FileOversight, JournalError, TopologyCheckpoint, check_work, copy};
use crate::action::consequence::delivery::persistent::observed::{
    BaseEvent, Event, FileOversightProfile,
    decoder::{DecoderEvent, learned::{LearnedEvent, checkpoint::{CheckpointEvent, FileLearnedResetIntent}}},
    shutdown::recovery::FileAdmissionStoppedHistory,
};
use crate::Error;
use std::path::Path;

impl TopologyCheckpoint {
    // Only a sealed, operation-bound recovery may use this pre-replay scan to
    // select availability. Full original semantic replay is still mandatory.
    pub(in super::super::super::super) fn stream_reset_successor_available(
        &self, profile: &FileOversightProfile, identity: &Path, events: &[Event],
        intent: &FileLearnedResetIntent, first_incident_sequence: u64,
    ) -> Result<bool, Error> {
        self.check_stream_reset_prefix(profile, identity, events, intent, first_incident_sequence)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let mut available = self.available;
        for event in &events[after..] {
            match event {
                Event::Core(BaseEvent::Fence) => available = false,
                Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                    CheckpointEvent::StreamReset { operation, .. },
                ))) if *operation == intent.control().operation => {}
                _ => return Err(Error::Stale),
            }
        }
        Ok(available)
    }

    fn check_stream_reset_prefix(&self, profile: &FileOversightProfile, identity: &Path,
        events: &[Event], intent: &FileLearnedResetIntent, first_incident_sequence: u64,
    ) -> Result<(), Error> {
        intent.validate()?;
        if first_incident_sequence == 0 { return Err(Error::InvalidInput); }
        check_work(&self.expected, self.work, self.last_operation)?;
        self.history.check(profile, identity, events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let (graph, available) = super::terminal::topology_at(&events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding);
        }
        // A checkpoint preceding the intent cannot invent its missing custody.
        // Both the complete native command and the supervisor's incident frontier
        // must already be in the exact independently retained prefix.
        let original = events[..after].iter().rev().find_map(|event| match event {
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::BeginStreamReset { intent, first_incident_sequence },
            ))) => Some((intent.as_ref(), *first_incident_sequence)),
            _ => None,
        });
        if original != Some((intent, first_incident_sequence)) { return Err(Error::Binding); }
        Ok(())
    }

    /// Pair retained source bytes and expenditure with the SAME healthy owner's
    /// acknowledged original stream-reset completion and recovery Fence. Only
    /// that exact reset operation and Fence records may follow the retained cut.
    /// Changed commands, new source checks, numerical work, clocks, reconciliation
    /// and terminal events refuse. Original source work is never refreshed.
    ///
    /// This also supports an exact already-completed retry after lost export;
    /// its extra Fence cannot rearm an unavailable graph. No source files are
    /// read, no audit is rerun, and durable export remains the caller's obligation.
    pub fn after_learned_stream_reset(&self, host: &FileOversight,
        intent: &FileLearnedResetIntent, first_incident_sequence: u64,
    ) -> Result<Self, JournalError> {
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        let available = self.stream_reset_successor_available(&host.profile,
            host.store.identity(), &host.events, intent, first_incident_sequence)?;
        // An acknowledged refusal is a real outcome too. A missing completion
        // cannot turn an interrupted or merely pending intent into a successor.
        let _ = host.learned_reset_result(intent.control().operation)?;
        let current = host.mediation_snapshot()?;
        if current.graph != *self.expected.graph() || current.available != available {
            return Err(Error::Binding.into());
        }
        let snapshot = host.inspect();
        self.stream_reset_successor(host.history_anchor()?, available,
            snapshot.control.sequence, snapshot.control.ledger.epoch)
    }

    /// Retain the original source work when an authenticated admission Stop wins
    /// over stream-reset completion. The native stopped result binds the selected
    /// Stop; any preceding suffix may contain only this reset and recovery Fence.
    /// This also preserves custody if an earlier completion outlived its export.
    /// No live owner, observer, new source read or numerical restart is produced.
    pub fn after_stream_reset_admission_stop(&self, profile: &FileOversightProfile,
        identity: &Path, history: &FileAdmissionStoppedHistory,
        intent: &FileLearnedResetIntent, first_incident_sequence: u64,
    ) -> Result<Self, JournalError> {
        let snapshot = history.snapshot();
        let progress = history.progress();
        if snapshot.revision != history.history_anchor().revision()
            || snapshot.stop.as_ref() != Some(&progress.receipt) {
            return Err(Error::Binding.into());
        }
        let events = history.history_anchor().decoded_events(profile, identity)?;
        self.check_stream_reset_prefix(profile, identity, &events, intent, first_incident_sequence)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let request = progress.receipt.request();
        let mut stopped = false;
        for event in &events[after..] {
            match event {
                Event::Core(BaseEvent::Stop(original)) if *original == request => stopped = true,
                Event::Core(BaseEvent::Fence) => {},
                Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                    CheckpointEvent::StreamReset { operation, .. },
                ))) if !stopped && *operation == intent.control().operation => {},
                _ => return Err(Error::Stale.into()),
            }
        }
        let (graph, available) = super::terminal::topology_at(&events)?;
        if graph != self.expected.graph() { return Err(Error::Binding.into()); }
        self.stream_reset_successor(history.history_anchor().clone(), available,
            snapshot.control.sequence, snapshot.control.ledger.epoch)
    }


    // A recovery-only terminal capability may repair a lost source export even
    // when this exact stream reset completed before the selected Stop. Unlike
    // reset completion, this admission REQUIRES a matching Stop already in the
    // original locked history; it cannot create a first Stop or another Fence.
    // Full guarded semantic replay still validates every reset/terminal witness.
    pub(in super::super::super::super) fn stopped_stream_reset_successor_available(
        &self, profile: &FileOversightProfile, identity: &Path, events: &[Event],
        intent: &FileLearnedResetIntent, first_incident_sequence: u64, operation: u64,
    ) -> Result<bool, Error> {
        if operation == 0 { return Err(Error::InvalidInput); }
        self.check_stream_reset_prefix(profile, identity, events, intent, first_incident_sequence)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let reset_operation = intent.control().operation;
        let mut completed = events[..after].iter().any(|event| matches!(event,
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::StreamReset { operation: original, .. },
            ))) if *original == reset_operation));
        let mut stopped = events[..after].iter().rev().find_map(|event| match event {
            Event::Core(BaseEvent::Stop(request)) => Some(*request), _ => None,
        });
        if stopped.is_some_and(|request| request.operation != operation) {
            return Err(Error::Duplicate);
        }
        for event in &events[after..] {
            match event {
                Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                    CheckpointEvent::StreamReset { operation: original, .. },
                ))) if !completed && stopped.is_none() && *original == reset_operation => {
                    completed = true;
                }
                Event::Core(BaseEvent::Stop(request)) if request.operation == operation => {
                    if stopped.is_some_and(|original| original != *request) {
                        return Err(Error::Binding);
                    }
                    stopped = Some(*request);
                }
                Event::Core(BaseEvent::Fence) if completed || stopped.is_some() => {}
                _ => return Err(Error::Stale),
            }
        }
        if stopped.is_none() { return Err(Error::Missing); }
        let (graph, available) = super::terminal::topology_at(events)?;
        if graph != self.expected.graph() { return Err(Error::Binding); }
        Ok(available)
    }

    fn stream_reset_successor(&self,
        history: super::FileHistoryAnchor, available: bool,
        control_sequence: u64, authority_epoch: u64,
    ) -> Result<Self, JournalError> {
        if control_sequence < self.control_sequence || authority_epoch < self.authority_epoch {
            return Err(Error::Stale.into());
        }
        Ok(Self { history, topology: copy(&self.topology)?, inventory: copy(&self.inventory)?,
            expected: self.expected.clone(), work: self.work, last_operation: self.last_operation,
            available, control_sequence, authority_epoch })
    }
}
