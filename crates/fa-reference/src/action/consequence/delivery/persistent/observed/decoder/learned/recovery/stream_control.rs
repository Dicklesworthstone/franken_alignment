//! Sealed offline capture and reset initiation on the original verified owner.
//! No recovery Fence, source observation, numerical step or live role escapes.
use super::{BaseEvent, Event, FileLearnedRecovery, FileLearnedRecoveryStatus,
    JournalError, journal};
use super::super::{DecoderEvent, LearnedEvent,
    checkpoint::{CheckpointEvent, FileLearnedCheckpointInfo, FileLearnedResetIntent,
        FileLearnedStreamNarrowRequest, check_stream_narrow_retry_event}};
use crate::action::consequence::delivery::persistent::{FileDeliverySnapshot, JournalCapacity};
use crate::action::consequence::delivery::persistent::observed::guarded::anchored::FileHistoryAnchor;
use crate::action::consequence::oversight::learned_host::{HostedLearnedInspection,
    checkpoint::HostedLearnedRecoveryUsage};
use crate::action::consequence::gate::containment::CheckpointNarrowReceipt;
use crate::Error;
use std::rc::Rc;

/// Independent exact capture selection. This is data, not a numerical handle.
/// The original journal revision and every exposed native capture coordinate
/// must match; an exact retry identifies the SAME historical capture event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedStreamCheckpointRequest {
    revision: u64,
    checkpoint: FileLearnedCheckpointInfo,
}
impl FileLearnedStreamCheckpointRequest {
    pub fn new(revision: u64, checkpoint: FileLearnedCheckpointInfo) -> Result<Self, Error> {
        if checkpoint.checkpoint == 0 || checkpoint.stream == 0 || checkpoint.position == 0 {
            return Err(Error::InvalidInput);
        }
        usize::try_from(revision).map_err(|_| Error::Limit)?;
        Ok(Self { revision, checkpoint })
    }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn checkpoint(&self) -> FileLearnedCheckpointInfo { self.checkpoint }
}

/// Acknowledged native control observation, never checkpoint or effect custody.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileLearnedStreamControlReceipt {
    Captured(FileLearnedCheckpointInfo),
    Narrowed {
        request: FileLearnedStreamNarrowRequest,
        receipt: CheckpointNarrowReceipt,
    },
    ResetBegun {
        intent: FileLearnedResetIntent,
        first_incident_sequence: u64,
        /// An exact historical retry can observe an already completed reset.
        /// It does not initiate or audit that operation again.
        already_completed: bool,
    },
}

/// Immutable acknowledged history. No live owner, reviewer, forecast observer,
/// source observer, permit or mutable numerical state can be extracted.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedStreamControlHistory;
/// fn no_owner(history: FileLearnedStreamControlHistory) { history.owner(); }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedStreamControlHistory {
    receipt: FileLearnedStreamControlReceipt,
    snapshot: FileDeliverySnapshot,
    numerical: HostedLearnedInspection,
    usage: HostedLearnedRecoveryUsage,
    anchor: FileHistoryAnchor,
    appended_events: usize,
}
impl FileLearnedStreamControlHistory {
    pub fn receipt(&self) -> &FileLearnedStreamControlReceipt { &self.receipt }
    pub fn snapshot(&self) -> &FileDeliverySnapshot { &self.snapshot }
    pub fn numerical(&self) -> &HostedLearnedInspection { &self.numerical }
    pub fn recovery_usage(&self) -> HostedLearnedRecoveryUsage { self.usage }
    pub fn history_anchor(&self) -> &FileHistoryAnchor { &self.anchor }
    pub fn appended_events(&self) -> usize { self.appended_events }
}

impl FileLearnedRecovery {
    pub(in super::super::super::super) fn prepare_narrowed_stream_checkpoint(mut self,
        expected: &FileLearnedStreamNarrowRequest)
        -> Result<(Self, FileLearnedStreamControlReceipt, usize), JournalError>
    {
        self.check_stream_control_profile()?;
        if !self.expected.requires_owned_forecast_stream_restart_audit() { return Err(Error::Binding.into()); }
        expected.validate()?;
        if let Some((receipt, witness)) = self.machine.learned_stream_narrow_retry(expected)? {
            check_stream_narrow_retry_event(&self.events, expected, &witness)?;
            return Ok((self, FileLearnedStreamControlReceipt::Narrowed {
                request: expected.clone(), receipt }, 0));
        }
        if self.events.len() as u64 != expected.revision() { return Err(Error::Stale.into()); }
        self.machine.check_learned_stream_narrow(expected)?;
        // Ordinary capacity only; no terminal reserve, Fence, inference, audit
        // or role is spent to produce this original containment consequence.
        self.check_stream_control_capacity(1)?;
        self.status = FileLearnedRecoveryStatus::Interrupted;
        let narrowed = self.machine.prepare_learned_stream_narrow(expected)?;
        let receipt = self.machine.learned_stream_narrow_retry(expected)?.ok_or(Error::Incomplete)?.0;
        self.append_stream_control(narrowed)?;
        Ok((self, FileLearnedStreamControlReceipt::Narrowed {
            request: expected.clone(), receipt }, 1))
    }

    // Only the original guarded wrapper calls these consuming preparations.
    // It checks its complete independent requirements on BOTH sides before
    // invoking the sole storage boundary below. Unwind drops the candidate.
    pub(in super::super::super::super) fn prepare_captured_stream_checkpoint(mut self,
        expected: FileLearnedStreamCheckpointRequest)
        -> Result<(Self, FileLearnedStreamControlReceipt, usize), JournalError>
    {
        self.check_stream_control_profile()?;
        let info = expected.checkpoint;
        match self.machine.learned_stream_checkpoint_info(info.checkpoint) {
            Ok(actual) => {
                let index = usize::try_from(expected.revision).map_err(|_| Error::Limit)?;
                if actual != info || !matches!(self.events.get(index),
                    Some(Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                        CheckpointEvent::StreamCapture { checkpoint, actor_revision, epoch, .. }))))
                        if *checkpoint == info.checkpoint && *actor_revision == info.actor_revision
                            && *epoch == info.authority_epoch) {
                    return Err(Error::Binding.into());
                }
                return Ok((self, FileLearnedStreamControlReceipt::Captured(info), 0));
            }
            Err(Error::Missing) => {},
            Err(error) => return Err(error.into()),
        }
        if self.events.len() as u64 != expected.revision { return Err(Error::Stale.into()); }
        let numerical = self.machine.broker.hosted_learned_generation()?;
        let control = self.machine.broker.inspect();
        if numerical.actor_revision != info.actor_revision
            || control.ledger.epoch != info.authority_epoch
            || control.sequence != info.control_sequence
            || self.machine.broker.hosted_learned_observation()?.stream() != info.stream
            || numerical.position != info.position || numerical.sampled_draws != info.sampled_draws {
            return Err(Error::Stale.into());
        }
        self.machine.check_learned_stream_capture(info.checkpoint, info.actor_revision, info.authority_epoch)?;
        self.check_stream_control_capacity(1)?;
        self.status = FileLearnedRecoveryStatus::Interrupted;
        let captured = self.machine.prepare_learned_stream_capture(
            info.checkpoint, info.actor_revision, info.authority_epoch)?;
        if self.machine.learned_stream_checkpoint_info(info.checkpoint)? != info {
            return Err(Error::Binding.into());
        }
        self.append_stream_control(captured)?;
        Ok((self, FileLearnedStreamControlReceipt::Captured(info), 1))
    }

    pub(in super::super::super::super) fn prepare_begun_stream_reset(mut self,
        expected: &FileLearnedResetIntent, first_incident_sequence: u64)
        -> Result<(Self, FileLearnedStreamControlReceipt, usize), JournalError>
    {
        self.check_stream_control_profile()?;
        expected.validate()?;
        if first_incident_sequence == 0 { return Err(Error::InvalidInput.into()); }
        let mut appended = 0;
        let completed = self.machine.learned_stream_reset_retry(expected, first_incident_sequence)?.is_some();
        if !completed {
            if let Some(pending) = self.machine.pending_learned_reset() {
                if &pending.intent != expected
                    || self.machine.pending_learned_stream_reset_sequence() != Some(first_incident_sequence) {
                    return Err(Error::Binding.into());
                }
                // A prior Fence/Stop cannot become a fresh reset instruction.
                self.machine.check_pending_learned_stream_reset(expected.control().operation)?;
            } else {
                self.machine.check_learned_stream_reset_intent(expected, first_incident_sequence)?;
                // Preserve native live initiation's two ORDINARY slots. A
                // terminal reserve cannot fund either intent or completion.
                self.check_stream_control_capacity(2)?;
                self.status = FileLearnedRecoveryStatus::Interrupted;
                let begun = CheckpointEvent::BeginStreamReset {
                    intent: Rc::new(expected.clone()), first_incident_sequence };
                self.machine.apply(&event(begun.clone()))?;
                self.append_stream_control(begun)?;
                appended = 1;
            }
        }
        Ok((self, FileLearnedStreamControlReceipt::ResetBegun {
            intent: expected.clone(), first_incident_sequence, already_completed: completed,
        }, appended))
    }

    fn check_stream_control_profile(&self) -> Result<(), JournalError> {
        let machine = self.verified_guarded_machine()?;
        // Topology custody and every legacy/raw/optional predictor still refuse.
        // Only the explicit owned prompt-audit recipe can compose with the
        // original stream protocol; its native per-cut checks prevent sampling
        // or missing statistical evidence from being erased by this finalizer.
        if self.expected.text_stream_profile().is_none()
            || (machine.broker.action_consistency_required()
                && !self.expected.requires_owned_forecast_stream_restart_audit())
            || self.events.iter().any(|event| matches!(event, Event::Mediation(_))) {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

    fn check_stream_control_capacity(&self, ordinary_events: usize) -> Result<(), JournalError> {
        let reserve = self.events.iter().find_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(reserve)) => Some(*reserve), _ => None,
        });
        let capacity = JournalCapacity::new(self.profile.delivery.limits, reserve,
            self.events.len(), self.canonical.len());
        if capacity.ordinary_remaining().events < ordinary_events { return Err(Error::Limit.into()); }
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        Ok(())
    }

    fn append_stream_control(&mut self, value: CheckpointEvent) -> Result<(), JournalError> {
        let event = event(value);
        journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        self.events.push(event);
        self.replayed = self.events.len();
        self.status = FileLearnedRecoveryStatus::Ready;
        Ok(())
    }

    pub(in super::super::super::super) fn finish_stream_control(self,
        receipt: FileLearnedStreamControlReceipt, appended_events: usize)
        -> Result<FileLearnedStreamControlHistory, JournalError>
    {
        let machine = self.verified_guarded_machine()?;
        let snapshot = machine.snapshot(self.events.len());
        let numerical = machine.broker.hosted_learned_generation()?;
        let usage = machine.broker.hosted_learned_recovery_usage()?;
        let bytes = if appended_events == 0 { self.canonical.clone() }
            else { journal::encode(&self.profile, self.store.identity(), &self.events)? };
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        self.store.confirm_and_cleanup()?;
        if appended_events != 0 { self.store.replace(&bytes)?; }
        let anchor = FileHistoryAnchor::from_acknowledged(self.events.len(), bytes);
        Ok(FileLearnedStreamControlHistory { receipt, snapshot, numerical, usage, anchor, appended_events })
    }
}

fn event(value: CheckpointEvent) -> Event {
    Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(value)))
}
