//! Read the original successful Hold application and its current exact cut.
//! No public receipt, copied replay or caller assertion establishes this fact.
use super::super::JournalError;
use super::super::requests::{
    FileRequestDisposition,
    actor::{FileConveneBasis, FileConveneBlockReason, FileConveneOpened},
};
use super::{Event, FileOversight};
use crate::Error;
use crate::action::ActionState;
use crate::action::consequence::Consequence;
use crate::action::consequence::oversight::MAX_OBSERVED_ROUNDS;

impl FileOversight {
    pub(crate) fn convening_requires_topology(&self) -> bool {
        self.machine.convening_requires_topology()
    }

    pub(crate) fn convene_round_used(&self, round: u64) -> bool {
        self.machine.broker.review_round_used(round) || self.worker_rounds.contains(&round)
    }

    pub(crate) fn native_convene_round_available(&self, round: u64) -> bool {
        self.machine.native_learned_convening_supported()
            && self.worker_rounds.contains(&round)
            && !self.machine.broker.review_round_used(round)
    }

    pub(crate) fn convene_status(
        &self,
        request: u64,
    ) -> Result<(FileConveneBasis, Option<FileConveneBlockReason>), JournalError> {
        self.convene_status_for(request, false)
    }

    /// Eligibility still needs the live native wrapper's sealed offer. A
    /// retained journal packet or a generic supervisor cannot supply it.
    pub(crate) fn native_convene_status(
        &self,
        request: u64,
    ) -> Result<(FileConveneBasis, Option<FileConveneBlockReason>), JournalError> {
        self.learned_convene_status(request)
    }

    /// Shared frozen learned-evidence eligibility. Native and process owners
    /// must separately prove their distinct retained roster custody; this
    /// observation alone supplies neither a helper implementation nor a ticket.
    pub(crate) fn learned_convene_status(
        &self,
        request: u64,
    ) -> Result<(FileConveneBasis, Option<FileConveneBlockReason>), JournalError> {
        self.convene_status_for(request, true)
    }

    fn convene_status_for(
        &self,
        request: u64,
        native_learned: bool,
    ) -> Result<(FileConveneBasis, Option<FileConveneBlockReason>), JournalError> {
        use FileConveneBlockReason::*;
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        let status = self.request_status(request)?;
        let FileRequestDisposition::Admitted { attempt, stage } = status.disposition else {
            return Err(Error::WrongState.into());
        };
        let action = self.request_action(request)?;
        let state = self.inspect();
        let applied = self.machine.broker.latest_applied_review(attempt);
        let held_round = applied.map_or(0, |receipt| receipt.control.binding.round);
        let basis = FileConveneBasis {
            scope: self.profile.delivery.scope,
            request,
            request_generation: status.generation,
            attempt,
            held_round,
            journal_revision: state.revision,
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
            input_revision: self.input_revision(attempt)?,
            recorded_at: state.control.ledger.elapsed,
            deadline: action.spec().deadline,
        };
        let supported = if native_learned {
            self.machine.native_learned_convening_supported()
        } else {
            self.machine.convening_supported()
        };
        let blocked = if !supported {
            Some(UnsupportedHistory)
        } else if state.stop.is_some() {
            Some(Stopped)
        } else if state.control.suspended {
            Some(Suspended)
        } else if stage != ActionState::Reviewing
            || state.control.decisions.get(&attempt) != Some(&Consequence::HoldEffect)
        {
            Some(NotHeld)
        } else if !self.machine.sessions.is_empty() {
            Some(ActiveReview)
        } else if self.machine.broker.review_round_count() >= MAX_OBSERVED_ROUNDS {
            Some(RoundLimit)
        } else if basis.recorded_at.is_none_or(|now| now >= basis.deadline) {
            Some(DeadlineElapsed)
        } else if applied.is_none_or(|receipt| {
            receipt.control.sequence != basis.control_sequence
                || receipt.control.attempt != attempt
                || &receipt.control.action != action
                || receipt.control.before != ActionState::Reviewing
                || receipt.control.after != ActionState::Reviewing
                || receipt.control.decision.consequence != Consequence::HoldEffect
                || action.spec().policy_epoch != basis.authority_epoch
                || !self.machine.broker.review_round_used(held_round)
                // A later failed or un-applied congress cannot borrow the old
                // successful Hold to claim that it is the current completed one.
                || self.events.iter().rev().find_map(|event| match event {
                    Event::Begin(id, round, ..) if *id == attempt => Some(*round),
                    _ => None,
                }) != Some(held_round)
        }) || (native_learned
            && self.check_convened_native_learned(request, attempt).is_err())
            || (!native_learned
                && self.machine.generated_text_only
                && self.check_convened_native_text(request).is_err())
        {
            Some(StaleReview)
        } else {
            None
        };
        Ok((basis, blocked))
    }

    fn check_convened_native_learned(
        &self,
        request: u64,
        attempt: u64,
    ) -> Result<(), JournalError> {
        if self.source_interrupted
            || !self.clock_ready()
            || self.machine.learned_paused()
            || self.machine.pending_learned_step().is_some()
            || self.machine.pending_learned_reset().is_some()
        {
            return Err(Error::Incomplete.into());
        }
        self.machine.check_native_learned_convening(attempt)?;
        let original = self.machine.checked_learned_sidecar(attempt)?;
        if original.input_revision() != self.input_revision(attempt)?
            || original.round().input().action() != self.request_action(request)?
        {
            return Err(Error::Stale.into());
        }
        Ok(())
    }

    pub(crate) fn convene_opened(
        &self,
        request: u64,
        round: u64,
    ) -> Result<FileConveneOpened, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        let status = self.request_status(request)?;
        let FileRequestDisposition::Admitted {
            attempt,
            stage: ActionState::Reviewing,
        } = status.disposition
        else {
            return Err(Error::WrongState.into());
        };
        let (actual, _) = self.machine.sessions.get(&round).ok_or(Error::Missing)?;
        if *actual != attempt || !self.machine.broker.review_round_used(round) {
            return Err(Error::Binding.into());
        }
        let anchor = self.review_anchor(round)?;
        if &anchor.policy.action != self.request_action(request)? {
            return Err(Error::Binding.into());
        }
        Ok(FileConveneOpened {
            journal_revision: self.revision(),
            input_revision: anchor.input_revision,
            started_at: anchor.started_at,
        })
    }
}
