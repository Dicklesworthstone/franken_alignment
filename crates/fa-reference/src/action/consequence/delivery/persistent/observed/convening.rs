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
use crate::action::consequence::oversight::broker::MAX_OBSERVED_ROUNDS;

impl FileOversight {
    pub(crate) fn convening_requires_topology(&self) -> bool {
        self.machine.convening_requires_topology()
    }

    pub(crate) fn convene_round_used(&self, round: u64) -> bool {
        self.machine.broker.review_round_used(round)
    }

    pub(crate) fn convene_status(
        &self,
        request: u64,
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
        let blocked = if !self.machine.convening_supported() {
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
        }) {
            Some(StaleReview)
        } else {
            None
        };
        Ok((basis, blocked))
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
