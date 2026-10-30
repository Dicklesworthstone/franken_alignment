//! Bounded, supervisor-only situation pages from one ORIGINAL publication cut.
//! Inspection never obtains a live owner, refreshes evidence, fences or sends.
mod learned;
pub mod fleet;

use super::{Event, FileOversight, FileOversightProfile, JournalError, Machine, journal, storage};
use super::super::guarded::anchored::FileHistoryAnchor;
use crate::action::{ActionState, ElapsedTick, ResolvedTarget, Scope};
use crate::action::consequence::oversight::actor::Knowledge;
use crate::Error;
use std::{fmt, path::Path, rc::Rc};

pub const MAX_SITUATION_PAGE_ATTEMPTS: usize = 64;
pub const MAX_SITUATION_REPLAY_EVENTS: usize = 64;

/// Which historical cut was inspected, not a durability or authenticity grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSituationOrigin { AcknowledgedOwner, CanonicalImage }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSituationBasis {
    pub scope: Scope,
    pub journal_revision: u64,
    pub control_sequence: u64,
    /// A recorded clock observation, NOT the current time or a fresh lease.
    pub recorded_at: Option<ElapsedTick>,
    pub origin: FileSituationOrigin,
    /// Exact independently supplied prefix checked against the SAME image.
    /// A smaller prefix does not authenticate the remaining suffix.
    pub anchored_through: Option<u64>,
}

/// Whole-cut accounting. These totals never shrink to the selected page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSituationSummary {
    pub total_units: u64,
    pub available_units: u64,
    pub reserved_units: u64,
    pub charged_units: u64,
    pub policy_epoch: u64,
    pub dispatcher_epoch: u64,
    pub suspended: bool,
    pub stop_recorded: bool,
    pub executions: u64,
    pub attempts: usize,
    /// Retained actor keys, including admissions refused before an attempt.
    pub retained_requests: usize,
    pub pending_attempts: usize,
    /// Dispatching, Unknown AND IrrecoverablyUnknown remain unresolved outcomes.
    pub unresolved_attempts: usize,
    /// A subset of unresolved_attempts, not a confirmed terminal outcome.
    pub irrecoverably_unknown_attempts: usize,
    pub terminal_attempts: usize,
}

/// Exact original stage and proposal metadata, with no payload, witnesses,
/// helper inputs/votes, keys, empirical review or inferred nonexecution receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSituationAttempt {
    pub attempt: u64,
    pub stage: ActionState,
    pub target: ResolvedTarget,
    pub units: u64,
    pub payload_bytes: usize,
    pub proposal_policy_epoch: u64,
    pub deadline: ElapsedTick,
}

/// Continuation DATA branded to one immutable capture, not a live capability.
/// Even an equal-revision fork or separately recaptured owner cannot accept it.
#[derive(Clone)]
pub struct FileSituationPageToken { capture: Rc<()>, next: usize }
impl fmt::Debug for FileSituationPageToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSituationPageToken").field("next", &self.next).finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct FileSituationPage {
    pub summary: FileSituationSummary,
    pub attempts: Vec<FileSituationAttempt>,
    pub offset: usize,
    pub next: Option<FileSituationPageToken>,
}
pub type FileSituationObservation = Knowledge<FileSituationPage, FileSituationBasis>;

/// Fully checked historical data. No mutable broker, recovered role or journal
/// survives extraction. Pages always refer to this exact capture, not live state.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::FileSituation;
/// fn no_effect(report: FileSituation) { report.publish(1, 1); }
/// ```
pub struct FileSituation {
    capture: Rc<()>,
    basis: FileSituationBasis,
    summary: FileSituationSummary,
    attempts: Vec<FileSituationAttempt>,
}
impl fmt::Debug for FileSituation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSituation").field("basis", &self.basis)
            .field("summary", &self.summary).finish_non_exhaustive()
    }
}
impl FileSituation {
    /// Unresolved outcomes first, then pending work, then confirmed/denied/
    /// cancelled outcomes; IDs break ties. This is lifecycle triage, not a
    /// learned/planner estimate of information gain or a safe-next-action grant.
    pub fn page(&self, continuation: Option<&FileSituationPageToken>, limit: usize)
        -> Result<FileSituationObservation, JournalError>
    {
        if limit == 0 { return Err(Error::InvalidInput.into()); }
        if limit > MAX_SITUATION_PAGE_ATTEMPTS { return Err(Error::Limit.into()); }
        let offset = match continuation {
            Some(token) if Rc::ptr_eq(&token.capture, &self.capture) => token.next,
            Some(_) => return Err(Error::Binding.into()),
            None => 0,
        };
        if offset > self.attempts.len() { return Err(Error::Binding.into()); }
        let end = offset.saturating_add(limit).min(self.attempts.len());
        let next = (end < self.attempts.len()).then(|| FileSituationPageToken {
            capture: Rc::clone(&self.capture), next: end,
        });
        Ok(Knowledge::Known { value: FileSituationPage {
            summary: self.summary, attempts: self.attempts[offset..end].to_vec(), offset, next,
        }, basis: self.basis })
    }

    fn capture(profile: &FileOversightProfile, machine: &Machine, revision: usize,
        origin: FileSituationOrigin, anchored_through: Option<u64>) -> Result<Self, JournalError>
    {
        let snapshot = machine.snapshot(revision);
        let control = &snapshot.control;
        let ledger = &control.ledger;
        let total = ledger.available.checked_add(ledger.reserved)
            .and_then(|sum| sum.checked_add(ledger.charged)).ok_or(Error::Overflow)?;
        if total != profile.delivery.total { return Err(Error::Binding.into()); }
        let mut summary = FileSituationSummary {
            total_units: total, available_units: ledger.available, reserved_units: ledger.reserved,
            charged_units: ledger.charged, policy_epoch: ledger.epoch,
            dispatcher_epoch: snapshot.dispatcher_epoch, suspended: control.suspended,
            stop_recorded: snapshot.stop.is_some(), executions: snapshot.executions,
            attempts: ledger.stages.len(), retained_requests: machine.requests.len(), pending_attempts: 0, unresolved_attempts: 0,
            irrecoverably_unknown_attempts: 0, terminal_attempts: 0,
        };
        let mut attempts = Vec::with_capacity(ledger.stages.len());
        for (&attempt, &stage) in &ledger.stages {
            let action = machine.actions.get(&attempt).ok_or(Error::Missing)?.spec();
            match priority(stage) {
                0 => summary.unresolved_attempts += 1,
                1 => summary.pending_attempts += 1,
                _ => summary.terminal_attempts += 1,
            }
            if stage == ActionState::IrrecoverablyUnknown {
                summary.irrecoverably_unknown_attempts += 1;
            }
            attempts.push(FileSituationAttempt { attempt, stage,
                target: action.target.ok_or(Error::Incomplete)?, units: action.units,
                payload_bytes: action.payload.len(), proposal_policy_epoch: action.policy_epoch,
                deadline: action.deadline });
        }
        attempts.sort_unstable_by_key(|row| (priority(row.stage), row.attempt));
        Ok(Self { capture: Rc::new(()), basis: FileSituationBasis {
            scope: profile.delivery.scope, journal_revision: snapshot.revision,
            control_sequence: control.sequence, recorded_at: ledger.elapsed, origin, anchored_through,
        }, summary, attempts })
    }
}

fn priority(stage: ActionState) -> u8 {
    match stage {
        ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown => 0,
        ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized => 1,
        ActionState::Confirmed | ActionState::Denied | ActionState::Cancelled
            | ActionState::ConfirmedNotExecuted => 2,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSituationReplayStatus { Reading, Ready, Failed }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSituationReplayProgress {
    pub status: FileSituationReplayStatus,
    pub checked_events: usize,
    pub total_events: usize,
}

/// Original semantic replay in bounded event quanta. No prefix situation is
/// exposed: even events after the interesting attempt must validate before finish.
/// Reading/decoding the bounded canonical image and each event remain synchronous.
/// Dropping the cursor cancels only private RAM work, never a production effect.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::FileSituationReplay;
/// fn no_prefix(cursor: FileSituationReplay) { cursor.page(None, 1); }
/// ```
pub struct FileSituationReplay {
    profile: FileOversightProfile,
    events: Vec<Event>,
    machine: Machine,
    position: usize,
    status: FileSituationReplayStatus,
    anchored_through: Option<u64>,
}
impl fmt::Debug for FileSituationReplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSituationReplay").field("progress", &self.progress()).finish_non_exhaustive()
    }
}
impl FileSituationReplay {
    fn new(profile: &FileOversightProfile, events: Vec<Event>, anchored_through: Option<u64>)
        -> Result<Self, JournalError>
    {
        let machine = Machine::new(profile)?;
        let status = if events.is_empty() { FileSituationReplayStatus::Ready }
            else { FileSituationReplayStatus::Reading };
        Ok(Self { profile: profile.clone(), events, machine, position: 0, status, anchored_through })
    }
    pub fn progress(&self) -> FileSituationReplayProgress {
        FileSituationReplayProgress { status: self.status, checked_events: self.position,
            total_events: self.events.len() }
    }
    pub fn advance(&mut self, events: usize) -> Result<FileSituationReplayProgress, JournalError> {
        if events > MAX_SITUATION_REPLAY_EVENTS { return Err(Error::Limit.into()); }
        if self.status == FileSituationReplayStatus::Failed { return Err(JournalError::Unavailable); }
        if events == 0 || self.status == FileSituationReplayStatus::Ready { return Ok(self.progress()); }
        // A failed original transition or caught unwind cannot resume a suspect
        // machine or export its prefix as a valid situation.
        self.status = FileSituationReplayStatus::Failed;
        let end = self.position.saturating_add(events).min(self.events.len());
        while self.position < end {
            self.machine.apply(&self.events[self.position])?;
            self.position += 1;
        }
        self.status = if self.position == self.events.len() { FileSituationReplayStatus::Ready }
            else { FileSituationReplayStatus::Reading };
        Ok(self.progress())
    }
    pub fn finish(self) -> Result<FileSituation, JournalError> {
        match self.status {
            FileSituationReplayStatus::Ready => FileSituation::capture(&self.profile, &self.machine,
                self.events.len(), FileSituationOrigin::CanonicalImage, self.anchored_through),
            FileSituationReplayStatus::Reading => Err(Error::Incomplete.into()),
            FileSituationReplayStatus::Failed => Err(JournalError::Unavailable),
        }
    }
    pub fn complete(mut self) -> Result<FileSituation, JournalError> {
        while self.status == FileSituationReplayStatus::Reading {
            self.advance(MAX_SITUATION_REPLAY_EVENTS)?;
        }
        self.finish()
    }
}

impl FileOversight {
    /// Capture this healthy owner's ACKNOWLEDGED state, never potentially newer
    /// disk bytes. This reads no clock/source, consumes no snapshot and writes no
    /// journal event. The result never asserts that historical eligibility is live.
    pub fn situation(&self) -> Result<FileSituation, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        FileSituation::capture(&self.profile, &self.machine, self.events.len(),
            FileSituationOrigin::AcknowledgedOwner, None)
    }

    /// Inspect one atomically read canonical image, even beside a live writer.
    /// No lock, fence, cleanup, source refresh or recovery role is acquired. All
    /// original events must replay; unsupported numerical recipes fail closed.
    /// An optional independently retained anchor checks exact prefix identity,
    /// not source authenticity or acknowledgement of any unanchored suffix.
    pub fn begin_read_situation(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        anchor: Option<&FileHistoryAnchor>) -> Result<FileSituationReplay, JournalError>
    {
        let events = read_events(directory.as_ref(), profile, anchor)?;
        FileSituationReplay::new(profile, events, anchor.map(FileHistoryAnchor::revision))
    }

    pub fn read_situation(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        anchor: Option<&FileHistoryAnchor>) -> Result<FileSituation, JournalError>
    {
        Self::begin_read_situation(directory, profile, anchor)?.complete()
    }
}

// One bounded canonical image supplies both independent-prefix checking and
// every later transition. Recipe binding must not reopen a different image.
fn read_events(directory: &Path, profile: &FileOversightProfile,
    anchor: Option<&FileHistoryAnchor>) -> Result<Vec<Event>, JournalError>
{
    super::super::super::codec::validate_profile(&profile.delivery)?;
    let identity = storage::identity(directory)?;
    let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
    let events = journal::decode(profile, &identity, &bytes)?;
    if let Some(anchor) = anchor { anchor.check(profile, &identity, &events)?; }
    Ok(events)
}

#[cfg(test)]
mod tests;
