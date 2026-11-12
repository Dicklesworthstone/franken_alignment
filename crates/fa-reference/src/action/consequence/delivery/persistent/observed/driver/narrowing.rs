//! Explicit local continuation across acknowledged ordinary target restrictions.
//! The original gate keeps its decision and reservation. Only a completed
//! driver's readiness cut moves, after proving every intervening control change.
use super::{FileSupervisedDriver, Job, JournalError, Phase, admitted};
use super::super::FileOversight;
use super::super::narrowing::MAX_AUTHORITY_NARROWINGS;
use crate::Error;
use crate::action::ActionState;
use crate::action::consequence::Consequence;

/// Supervisor-only acknowledgment of a retained driver continuation. This is
/// not a permit, renewed human key, fresh observation or new review receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDriverNarrowingContinuation {
    request: u64,
    attempt: u64,
    operation: u64,
    previous_control_sequence: u64,
    control_sequence: u64,
    authority_epoch: u64,
    narrowing_operations: Vec<u64>,
    retained_automatic_permit: bool,
}

impl FileDriverNarrowingContinuation {
    pub fn request(&self) -> u64 { self.request }
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn previous_control_sequence(&self) -> u64 { self.previous_control_sequence }
    pub fn control_sequence(&self) -> u64 { self.control_sequence }
    pub fn authority_epoch(&self) -> u64 { self.authority_epoch }
    pub fn narrowing_operations(&self) -> &[u64] { &self.narrowing_operations }
    pub fn retained_automatic_permit(&self) -> bool { self.retained_automatic_permit }
}

impl FileSupervisedDriver {
    /// Find the last original restriction in a complete continuation chain.
    /// This reads only acknowledged native state. None means no completed Ready
    /// job needs this operation; it is not evidence of publication eligibility.
    /// An incomplete helper round is never rebased or silently run again.
    pub fn pending_narrowing_continuation(
        &self,
        request: u64,
    ) -> Result<Option<u64>, JournalError> {
        let Some(job) = &self.job else { return Ok(None); };
        if job.request != request { return Err(Error::Binding.into()); }
        if job.phase != Phase::Ready { return Ok(None); }
        let host = self.supervisor.host()?;
        job.check_owner(&host)?;
        if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
        let (attempt, stage) = admitted(host.request_status(request)?)?;
        if attempt != job.attempt { return Err(Error::Binding.into()); }
        if !matches!(stage, ActionState::Reviewing | ActionState::Authorized) {
            return Ok(None);
        }
        if job.control_sequence == Some(host.inspect().control.sequence) { return Ok(None); }
        let (sequence, epoch) = retained_basis(self, job, &host)?;
        Ok(Some(continuation(job, &host, sequence, epoch)?.operation))
    }

    /// Continue the exact retained completed review after one or more ordinary
    /// restrictions, selecting the LAST acknowledged narrowing operation.
    ///
    /// Only contiguous original NarrowAuthority receipts may bridge the old
    /// readiness cut. Same-owner action, full input, epoch, Continue decision,
    /// target and reservation custody must still agree. A policy change, another
    /// review, source/input invalidation, cancellation or recovery cannot borrow
    /// a later narrowing receipt to revive this job.
    ///
    /// This changes only local driver bookkeeping: no journal event, helper,
    /// source/clock read, authorization, refund, dispatch or human approval.
    /// All normal evidence and two-key checks still run. The service must discard
    /// its withdrawn human exchange/key and obtain fresh independent approval.
    /// An exact retry on the same still-current job returns its original result.
    pub fn continue_after_narrowing(
        &mut self,
        request: u64,
        operation: u64,
    ) -> Result<FileDriverNarrowingContinuation, JournalError> {
        if operation == 0 { return Err(Error::InvalidInput.into()); }
        let job = self.job.as_ref().ok_or(Error::Missing)?;
        if job.request != request { return Err(Error::Binding.into()); }
        if job.phase != Phase::Ready { return Err(Error::WrongState.into()); }
        let host = self.supervisor.host()?;
        let (sequence, epoch) = retained_basis(self, job, &host)?;
        let selected = host.authority_narrowing_receipt(operation)?;
        if selected.change().sequence != sequence
            || selected.change().authority_epoch != epoch
        {
            return Err(Error::Stale.into());
        }
        if job.control_sequence == Some(sequence) {
            let previous = self.narrowing_continuation.as_ref().ok_or(Error::WrongState)?;
            if previous.request != request
                || previous.attempt != job.attempt
                || previous.operation != operation
                || previous.control_sequence != sequence
                || previous.authority_epoch != epoch
                || previous.retained_automatic_permit != job.permit.is_some()
            {
                return Err(Error::Stale.into());
            }
            return Ok(previous.clone());
        }
        let result = continuation(job, &host, sequence, epoch)?;
        if result.operation != operation { return Err(Error::Binding.into()); }
        drop(host);
        self.job.as_mut().expect("validated retained Ready job").control_sequence =
            Some(result.control_sequence);
        self.narrowing_continuation = Some(result.clone());
        Ok(result)
    }
}

/// Validate retained custody, not fresh evidence. Normal driver capture and
/// native authorization still establish current prerequisites after continuation.
fn retained_basis(
    driver: &FileSupervisedDriver,
    job: &Job,
    host: &FileOversight,
) -> Result<(u64, u64), JournalError> {
    job.check_owner(host)?;
    if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
    if host.source_interrupted { return Err(Error::Incomplete.into()); }
    if !host.authority_narrowing_supported()
        || driver.topology.is_some()
        || driver.learned_review.is_some()
    {
        return Err(Error::Incomplete.into());
    }
    let (attempt, stage) = admitted(host.request_status(job.request)?)?;
    if attempt != job.attempt || host.request_action(job.request)? != &job.action {
        return Err(Error::Binding.into());
    }
    match (stage, job.permit.is_some()) {
        (ActionState::Reviewing, false) | (ActionState::Authorized, true) => {}
        _ => return Err(Error::WrongState.into()),
    }
    let state = host.inspect();
    if state.stop.is_some() || state.control.suspended
        || state.control.decisions.get(&job.attempt) != Some(&Consequence::Continue)
    {
        return Err(Error::WrongState.into());
    }
    if job.action.spec().policy_epoch != state.control.ledger.epoch
        || job.input_revision != host.input_revision(job.attempt)?
        || host.machine.broker.current_inputs(job.attempt)?
            != Some(job.inputs.as_ref().ok_or(Error::Incomplete)?)
    {
        return Err(Error::Stale.into());
    }
    let target = job.action.spec().target.ok_or(Error::Binding)?;
    if state.control.ceiling.as_ref().is_some_and(|ceiling| !ceiling.contains(target)) {
        return Err(Error::Binding.into());
    }
    Ok((state.control.sequence, state.control.ledger.epoch))
}

fn continuation(
    job: &Job,
    host: &FileOversight,
    sequence: u64,
    epoch: u64,
) -> Result<FileDriverNarrowingContinuation, JournalError> {
    let previous = job.control_sequence.ok_or(Error::Incomplete)?;
    let distance = sequence.checked_sub(previous).ok_or(Error::Stale)?;
    if distance == 0 || distance > MAX_AUTHORITY_NARROWINGS as u64 {
        return Err(Error::Stale.into());
    }
    // The original native history caps this inventory at 128. No journal replay,
    // numerical work or unbounded search is hidden in this local continuation.
    let mut changes = Vec::new();
    changes.try_reserve_exact(distance as usize).map_err(|_| Error::Limit)?;
    for (operation, receipt) in &host.machine.narrowings {
        if receipt.change().sequence > previous && receipt.change().sequence <= sequence {
            changes.push((*operation, receipt));
        }
    }
    if changes.len() != distance as usize { return Err(Error::Stale.into()); }
    changes.sort_unstable_by_key(|(_, receipt)| receipt.change().sequence);
    let mut expected = previous;
    let target = job.action.spec().target.ok_or(Error::Binding)?;
    let mut operations = Vec::new();
    operations.try_reserve_exact(changes.len()).map_err(|_| Error::Limit)?;
    for (operation, receipt) in &changes {
        let change = receipt.change();
        let next = expected.checked_add(1).ok_or(Error::Overflow)?;
        if *operation != receipt.request().operation()
            || receipt.request().expected_control_sequence() != expected
            || receipt.request().expected_authority_epoch() != epoch
            || change.previous_sequence != expected
            || change.sequence != next
            || change.authority_epoch != epoch
            || change.scope != job.action.spec().scope
            || change.cancelled.contains(&job.attempt)
            || !change.ceiling.contains(target)
        {
            return Err(Error::Stale.into());
        }
        operations.push(*operation);
        expected = next;
    }
    let (operation, final_receipt) = changes.last().ok_or(Error::Incomplete)?;
    if expected != sequence
        || host.inspect().control.ceiling.as_ref() != Some(&final_receipt.change().ceiling)
    {
        return Err(Error::Stale.into());
    }
    Ok(FileDriverNarrowingContinuation {
        request: job.request,
        attempt: job.attempt,
        operation: *operation,
        previous_control_sequence: previous,
        control_sequence: sequence,
        authority_epoch: epoch,
        narrowing_operations: operations,
        retained_automatic_permit: job.permit.is_some(),
    })
}
