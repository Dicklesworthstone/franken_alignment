//! Independent bounded supervisor request and original durable narrow event.
use super::{CheckpointEvent, DecoderEvent, Event, FileLearnedCheckpointInfo, FileOversight,
    JournalError, LearnedEvent, Machine, Reader, Transition, Writer, journal};
use crate::action::{MAX_ATTEMPTS, ResolvedTarget};
use crate::action::consequence::gate::{ReviewBinding, TargetCeiling};
use crate::action::consequence::gate::containment::CheckpointNarrowReceipt;
use crate::Error;
use std::rc::Rc;

/// Trusted supervisor selection. These are request data, never actor-supplied
/// approval, effect rights or authenticated production PermitCx custody.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedStreamNarrowControl {
    pub operation: u64,
    pub expected_control_sequence: u64,
    pub expected_actor_revision: u64,
    pub expected_authority_epoch: u64,
    pub binding: ReviewBinding,
    pub retained_targets: Vec<ResolvedTarget>,
}

/// Exact original capture and journal/control predecessor. Only the explicit
/// owned prompt-audit stream profile admits this request. Retained targets may
/// name independently selected versions of the SAME publication endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLearnedStreamNarrowRequest {
    revision: u64,
    checkpoint: FileLearnedCheckpointInfo,
    control: FileLearnedStreamNarrowControl,
}
impl FileLearnedStreamNarrowRequest {
    pub fn new(revision: u64, checkpoint: FileLearnedCheckpointInfo,
        control: FileLearnedStreamNarrowControl) -> Result<Self, Error>
    {
        let request = Self { revision, checkpoint, control };
        request.validate()?;
        Ok(request)
    }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn checkpoint(&self) -> FileLearnedCheckpointInfo { self.checkpoint }
    pub fn control(&self) -> &FileLearnedStreamNarrowControl { &self.control }

    pub(in super::super::super::super) fn validate(&self) -> Result<(), Error> {
        let c = &self.control;
        if self.checkpoint.checkpoint == 0 || self.checkpoint.stream == 0 || self.checkpoint.position == 0
            || c.operation == 0 || c.binding.round == 0 || c.binding.reducer_generation == 0
            || c.binding.evidence_root == [0; 32] { return Err(Error::InvalidInput); }
        usize::try_from(self.revision).map_err(|_| Error::Limit)?;
        TargetCeiling::new(&c.retained_targets)?;
        Ok(())
    }
}

impl FileOversight {
    /// Restrict authority at the same complete unsampled prompt without changing
    /// policy, predictor, actor, source or numerical work. The native gate owns
    /// cancellation and epoch advance. Exact historical retries perform neither.
    pub fn narrow_learned_stream_checkpoint(&mut self, request: &FileLearnedStreamNarrowRequest)
        -> Result<CheckpointNarrowReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        request.validate()?;
        if let Some((receipt, witness)) = self.machine.learned_stream_narrow_retry(request)? {
            check_retry_event(&self.events, request, &witness)?;
            return Ok(receipt);
        }
        self.check_learned_event_capacity(request.revision(), 1)?;
        self.machine.check_learned_stream_narrow(request)?;
        self.check_source_admission(&event(CheckpointEvent::StreamNarrow {
            request: Rc::new(request.clone()), witness: Rc::from(&b""[..]) }))?;
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let narrowed = event(candidate.prepare_learned_stream_narrow(request)?);
        let receipt = candidate.learned_stream_narrow_retry(request)?.ok_or(Error::Incomplete)?.0;
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &narrowed)?;
        self.persist_candidate(narrowed, bytes, candidate, Transition::Unit)?;
        Ok(receipt)
    }
}

pub(in super::super::super::super) fn check_retry_event(events: &[Event],
    request: &FileLearnedStreamNarrowRequest, witness: &[u8]) -> Result<(), Error>
{
    let index = usize::try_from(request.revision()).map_err(|_| Error::Limit)?;
    match events.get(index) {
        Some(Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
            CheckpointEvent::StreamNarrow { request: prior, witness: recorded }))))
            if prior.as_ref() == request && recorded.as_ref() == witness => Ok(()),
        _ => Err(Error::Binding),
    }
}

pub(in super::super::super::super) fn write_narrow_request(w: &mut Writer,
    request: &FileLearnedStreamNarrowRequest) -> Result<(), Error>
{
    request.validate()?;
    let p = request.checkpoint(); let c = request.control();
    for value in [request.revision(), p.checkpoint, p.actor_revision, p.authority_epoch,
        p.control_sequence, p.stream, p.position, p.sampled_draws, c.operation,
        c.expected_control_sequence, c.expected_actor_revision, c.expected_authority_epoch,
        c.binding.round, c.binding.reducer_generation] { w.u64(value)?; }
    w.raw(&c.binding.evidence_root)?;
    w.count(c.retained_targets.len())?;
    for target in &c.retained_targets { w.target(*target)?; }
    Ok(())
}

pub(super) fn read_narrow_request(r: &mut Reader<'_>) -> Result<FileLearnedStreamNarrowRequest, Error> {
    let revision = r.u64()?;
    let checkpoint = FileLearnedCheckpointInfo { checkpoint: r.u64()?, actor_revision: r.u64()?,
        authority_epoch: r.u64()?, control_sequence: r.u64()?, stream: r.u64()?,
        position: r.u64()?, sampled_draws: r.u64()? };
    let operation = r.u64()?; let expected_control_sequence = r.u64()?;
    let expected_actor_revision = r.u64()?; let expected_authority_epoch = r.u64()?;
    let round = r.u64()?; let reducer_generation = r.u64()?;
    let evidence_root = r.take(32)?.try_into().map_err(|_| Error::Incomplete)?;
    let count = r.count(MAX_ATTEMPTS)?;
    let mut retained_targets = Vec::new();
    retained_targets.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count { retained_targets.push(r.target()?); }
    FileLearnedStreamNarrowRequest::new(revision, checkpoint, FileLearnedStreamNarrowControl {
        operation, expected_control_sequence, expected_actor_revision, expected_authority_epoch,
        binding: ReviewBinding { round, reducer_generation, evidence_root }, retained_targets,
    })
}

fn event(value: CheckpointEvent) -> Event { Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(value))) }
