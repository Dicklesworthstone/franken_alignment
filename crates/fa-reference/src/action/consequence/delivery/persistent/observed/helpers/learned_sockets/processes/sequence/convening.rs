//! Explicit continuation of a completed Hold through the original fixed process schedule.
//! The next executable roster, window and starts were admitted before the first
//! ballot. No child may start until every predecessor child has been reaped.

#[cfg(all(test, unix))]
mod tests;

use super::*;
use crate::action::ActionState;
use crate::action::consequence::Consequence;
use crate::action::consequence::oversight::learned_host::sidecar::workers::LearnedWorkerRound;

impl FileLearnedProcessReview {
    /// Retained original custody, independent of current source, time or poll
    /// eligibility. Cancellation must still retire it after any of those expire.
    pub(in crate::action::consequence::delivery::persistent::observed) fn has_held_successor_custody(
        &self,
    ) -> bool {
        self.retained_held_successor().is_some()
    }

    fn retained_held_successor(&self) -> Option<LearnedWorkerRound> {
        if self.status != LearnedProcessStatus::Finished
            || self.active.as_ref()?.status() != LearnedSocketStatus::Finished
            || !matches!(
                self.history.last(),
                Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. })
                    if receipt.policy.control.decision.consequence == Consequence::HoldEffect
                        && receipt.policy.control.binding.round == self.round()
            )
        {
            return None;
        }
        let next = *self.schedule.rounds.get(self.index.checked_add(1)?)?;
        (self.programs.contains_key(&next.round)
            && self.records.get(&next.round).is_some_and(|record| {
                record.admission.is_none()
                    && record.processes.is_empty()
                    && !record.socket.completed
            }))
        .then_some(next)
    }

    /// A still-funded fixed slot, not permission to spawn. This remains visible
    /// while predecessor children drain so the owner can service Stop and reap.
    pub(in crate::action::consequence::delivery::persistent::observed) fn held_successor_round(
        &self,
    ) -> Option<LearnedWorkerRound> {
        if self.polls >= self.schedule.polls
            || self.failed_children.is_some()
            || self.unresolved_startup.is_some()
        {
            return None;
        }
        self.retained_held_successor()
    }

    /// Validate the exact acknowledged Hold and original frozen input without
    /// acquiring a source, advancing a clock, launching a child or spending a
    /// protocol poll. A Pending operator ticket may wait on this basis while
    /// the original child owner completes its separate cleanup obligation.
    pub(in crate::action::consequence::delivery::persistent::observed) fn check_held_successor_basis(
        &self,
        host: &FileOversight,
        now: ElapsedTick,
    ) -> Result<LearnedWorkerRound, JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) {
            return Err(Error::Binding.into());
        }
        if host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let next = self.held_successor_round().ok_or(Error::WrongState)?;
        let current = self.active.as_ref().ok_or(Error::Incomplete)?;
        let sidecar = &current.round.sidecar;
        host.machine.check_native_learned_convening(sidecar.attempt())?;
        let state = host.inspect();
        if state.stop.is_some()
            || state.control.suspended
            || !host.machine.sessions.is_empty()
            || state.control.ledger.stages.get(&sidecar.attempt()) != Some(&ActionState::Reviewing)
            || state.control.decisions.get(&sidecar.attempt()) != Some(&Consequence::HoldEffect)
        {
            return Err(Error::WrongState.into());
        }
        let Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. }) =
            self.history.last()
        else {
            return Err(Error::WrongState.into());
        };
        if now >= next.window.commit_by
            || state.control.ledger.elapsed != Some(now)
            || state.control.ledger.epoch != self.input.action().spec().policy_epoch
            || state.control.sequence != receipt.policy.control.sequence
            || receipt.policy.control.attempt != sidecar.attempt()
            || &receipt.policy.control.action != self.input.action()
            || host.events.iter().rev().find_map(|event| match event {
                Event::Begin(attempt, round, ..) if *attempt == sidecar.attempt() => Some(*round),
                _ => None,
            }) != Some(self.round())
        {
            return Err(Error::Stale.into());
        }
        unused(host, next.round)?;
        if host.current_learned_sidecar(sidecar)? != &self.input {
            return Err(Error::Stale.into());
        }
        Ok(next)
    }

    /// Complete predecessor reaping is an additional requirement for consuming
    /// a fixed successor. An unknown startup or lost child owner never counts as
    /// cleanup, and an observed exit remains distinct from an accepted ballot.
    pub(in crate::action::consequence::delivery::persistent::observed) fn check_held_successor(
        &self,
        host: &FileOversight,
        now: ElapsedTick,
    ) -> Result<LearnedWorkerRound, JournalError> {
        let next = self.check_held_successor_basis(host, now)?;
        if !self.all_reaped() {
            return Err(Error::Incomplete.into());
        }
        Ok(next)
    }

    /// Consume the next original fixed roster after the driver's one-use ticket
    /// and fresh policy capture. One original protocol poll pays for this launch;
    /// all prior starts, polls, disclosure and records remain cumulative.
    /// Source/time checks run again before Begin and the original launcher keeps
    /// its post-launch clock check and partial-child cleanup custody.
    pub(in crate::action::consequence::delivery::persistent::observed) fn convene_held_with_clock<F>(
        &mut self,
        host: &mut FileOversight,
        revision: u64,
        snapshot: Snapshot,
        mut clock: F,
    ) -> Result<LearnedProcessStatus, FileProcessFailure>
    where
        F: FnMut() -> ElapsedTick,
    {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) {
            return Err(Error::Binding.into());
        }
        if revision != self.revision {
            return Err(Error::Stale.into());
        }
        if !snapshot.complete {
            return Err(Error::Incomplete.into());
        }
        let recorded = host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)?;
        self.check_held_successor(host, recorded)?;
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let mut operation = ReviewOperation {
            owner: self,
            returned: false,
        };
        let result = (|| -> Result<LearnedProcessStatus, FileProcessFailure> {
            let now = clock();
            if now < recorded {
                return Err(Error::Stale.into());
            }
            if now != recorded {
                host.observe_time(host.revision(), now)?;
            }
            operation.owner.check_held_successor(host, now)?;
            operation.owner.polls = operation.owner.polls.checked_add(1).ok_or(Error::Overflow)?;
            operation.owner.status = LearnedProcessStatus::Failed;
            operation.owner.failure = Some(Error::Incomplete.into());
            let status = operation.owner.start_successor(host, now, snapshot, &mut clock)?;
            if status != LearnedProcessStatus::Running {
                return Err(Error::Incomplete.into());
            }
            Ok(status)
        })();
        match &result {
            Ok(status) => {
                operation.owner.status = *status;
                operation.owner.failure = None;
            }
            Err(error) => {
                operation.owner.stop_children();
                operation.owner.status = LearnedProcessStatus::Failed;
                operation.owner.failure = Some(error.clone());
            }
        }
        operation.returned = true;
        result
    }
}
