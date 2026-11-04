//! Carry original source custody across one exact stream-control event only.
use super::{TopologyCheckpoint, check_work, copy};
use crate::Error;
use crate::action::consequence::delivery::persistent::{FileDeliverySnapshot, JournalError,
    observed::{Event, FileOversightProfile,
        decoder::{DecoderEvent, learned::{LearnedEvent, FileLearnedStreamControlHistory,
            FileLearnedStreamControlReceipt, checkpoint::CheckpointEvent}},
        guarded::mediated::predictive::FileMediatedStreamControlRequest,
    },
};
use std::path::Path;

impl TopologyCheckpoint {
    // Structural admission only; the sealed predictive-mediated cursor must
    // still complete every original numerical event and independent guard.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn stream_control_successor_available(&self, profile: &FileOversightProfile,
        identity: &Path, events: &[Event], request: &FileMediatedStreamControlRequest)
        -> Result<bool, Error>
    {
        request.validate()?;
        check_work(&self.expected, self.work, self.last_operation)?;
        self.history.check(profile, identity, events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let (graph, available) = super::terminal::topology_at(&events[..after])?;
        if graph != self.expected.graph() || available != self.available { return Err(Error::Binding); }
        match &events[after..] {
            [] => {},
            [event] if selected_event(request, after, event) => {},
            _ => return Err(Error::Stale),
        }
        // Capture/intent do not change topology. Narrowing revokes the old cut
        // through the original delivery owner but leaves graph availability,
        // retained documents and source expenditure unchanged.
        let (graph, available) = super::terminal::topology_at(events)?;
        if graph != self.expected.graph() || available != self.available { return Err(Error::Binding); }
        Ok(available)
    }

    pub(in crate::action::consequence::delivery::persistent::observed)
    fn check_stream_control_start(&self, profile: &FileOversightProfile, identity: &Path,
        events: &[Event], snapshot: &FileDeliverySnapshot, request: &FileMediatedStreamControlRequest)
        -> Result<(), JournalError>
    {
        if snapshot.revision != events.len() as u64 { return Err(Error::Binding.into()); }
        if snapshot.control.sequence < self.control_sequence
            || snapshot.control.ledger.epoch < self.authority_epoch { return Err(Error::Stale.into()); }
        self.stream_control_successor_available(profile, identity, events, request)?;
        Ok(())
    }

    pub(in crate::action::consequence::delivery::persistent::observed)
    fn after_learned_stream_control(&self, profile: &FileOversightProfile, identity: &Path,
        request: &FileMediatedStreamControlRequest, history: &FileLearnedStreamControlHistory)
        -> Result<Self, JournalError>
    {
        let anchor = history.history_anchor();
        let snapshot = history.snapshot();
        if snapshot.revision != anchor.revision() || history.appended_events() > 1 {
            return Err(Error::Binding.into());
        }
        let receipt_matches = match (request, history.receipt()) {
            (FileMediatedStreamControlRequest::Capture(request),
                FileLearnedStreamControlReceipt::Captured(actual)) => request.checkpoint() == *actual,
            (FileMediatedStreamControlRequest::Narrow(request),
                FileLearnedStreamControlReceipt::Narrowed { request: actual, .. }) => request == actual,
            (FileMediatedStreamControlRequest::BeginReset { intent, first_incident_sequence },
                FileLearnedStreamControlReceipt::ResetBegun { intent: actual, first_incident_sequence: sequence, .. }) =>
                intent == actual && first_incident_sequence == sequence,
            _ => false,
        };
        if !receipt_matches { return Err(Error::Binding.into()); }
        let events = anchor.decoded_events(profile, identity)?;
        self.check_stream_control_start(profile, identity, &events, snapshot, request)?;
        if !events.iter().enumerate().any(|(at, event)| selected_event(request, at, event)) {
            return Err(Error::Binding.into());
        }
        Ok(Self { history: anchor.clone(), topology: copy(&self.topology)?,
            inventory: copy(&self.inventory)?, expected: self.expected.clone(), work: self.work,
            last_operation: self.last_operation, available: self.available,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch })
    }
}

fn selected_event(request: &FileMediatedStreamControlRequest, at: usize, event: &Event) -> bool {
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(event))) = event else { return false; };
    match (request, event) {
        (FileMediatedStreamControlRequest::Capture(request),
            CheckpointEvent::StreamCapture { checkpoint, actor_revision, epoch, .. }) => {
            let info = request.checkpoint();
            request.revision() == at as u64 && *checkpoint == info.checkpoint
                && *actor_revision == info.actor_revision && *epoch == info.authority_epoch
        },
        (FileMediatedStreamControlRequest::Narrow(request),
            CheckpointEvent::StreamNarrow { request: actual, .. }) =>
            request.revision() == at as u64 && request == actual.as_ref(),
        (FileMediatedStreamControlRequest::BeginReset { intent, first_incident_sequence },
            CheckpointEvent::BeginStreamReset { intent: actual, first_incident_sequence: sequence }) =>
            intent == actual.as_ref() && first_incident_sequence == sequence,
        _ => false,
    }
}
