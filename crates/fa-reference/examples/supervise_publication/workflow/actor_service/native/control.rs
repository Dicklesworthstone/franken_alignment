//! Pump the selected original operator at existing native Stop checkpoints.
//! An operator transport failure retires only that sidecar. A granted Fence
//! is observed through the original Stop cleanup before any forward work.

use super::super::operator::Service;
use super::super::evaluator;
use super::{Config, Deadline, ElapsedTick, FileHumanReviewer, FileSupervisedDriver, PeerProfile};
use crate::workflow::control::Control as StopControl;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::FileHistoryAnchor,
    reviewer::peer::control::FileRecoveryStop,
};

pub(super) struct Control {
    stop: StopControl,
    operator: Option<Service>,
    evaluator: Option<evaluator::Service>,
}

impl Control {
    pub(super) fn new(
        config: &Config,
        operation: u64,
        profile: Option<&PeerProfile>,
    ) -> Result<Self, String> {
        Ok(Self { stop: StopControl::new(config, operation, profile)?, operator: None, evaluator: None })
    }

    pub(super) fn install_operator(&mut self, operator: Option<Service>) {
        self.operator = operator;
    }

    pub(super) fn take_operator(&mut self) -> Option<Service> {
        self.operator.take()
    }

    pub(super) fn install_evaluator(&mut self, evaluator: Option<evaluator::Service>) {
        self.evaluator = evaluator;
    }

    pub(super) fn take_evaluator(&mut self) -> Option<evaluator::Service> {
        self.evaluator.take()
    }

    pub(super) fn has_operator(&self) -> bool {
        self.operator.is_some()
    }

    pub(super) fn has_evaluator(&self) -> bool {
        self.evaluator.is_some()
    }

    pub(super) fn has_live_evaluator(&self) -> bool {
        self.evaluator
            .as_ref()
            .is_some_and(|service| !service.is_retired())
    }

    /// Stop accepting labels before terminal-only reply work. The original
    /// service can drain a previously produced response but cannot use a
    /// partially received request to mutate the retained final history.
    pub(super) fn finish_evaluator(&mut self) {
        if let Some(evaluator) = &mut self.evaluator {
            evaluator.finish();
        }
    }

    pub(super) fn checkpoint_evaluator(&mut self, driver: &mut FileSupervisedDriver) {
        if let Some(evaluator) = &mut self.evaluator {
            evaluator.checkpoint_driver(driver);
        }
    }

    /// Pump the retained original operator session after a Stop/drain attempt.
    /// Its selected grants remain independent; this never retries cleanup or
    /// reissues a grant when actor ingress has failed.
    pub(super) fn checkpoint_operator(&mut self) {
        if let Some(operator) = &mut self.operator {
            operator.checkpoint();
        }
    }

    pub(super) fn into_parts(self) -> (StopControl, Option<Service>) {
        (self.stop, self.operator)
    }

    /// Retain the SAME original Stop listener across resident request cycles.
    /// Sharing preserves its credentials, candidate quota and one-use cleanup
    /// outcome; a later review cannot provision another control allowance.
    pub(super) fn share_stop(&self) -> StopControl {
        self.stop.share()
    }

    pub(super) fn checkpoint<F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        reviewer: &FileHumanReviewer,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<bool, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        if self.stop.checkpoint(driver, reviewer, deadline, time)? {
            if let Some(evaluator) = &mut self.evaluator {
                evaluator.retire();
            }
            return Ok(true);
        }
        if let Some(operator) = &mut self.operator {
            operator.checkpoint_driver(driver);
        }
        if self.stop.native_stop(driver, time)? {
            if let Some(evaluator) = &mut self.evaluator {
                evaluator.retire();
            }
            return Ok(true);
        }
        self.checkpoint_evaluator(driver);
        Ok(false)
    }

    pub(super) fn native_stop<F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        time: &mut F,
    ) -> Result<bool, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        self.stop.native_stop(driver, time)
    }

    pub(super) fn native_cleanup_started(&self) -> bool {
        self.stop.native_cleanup_started()
    }

    pub(super) fn recovery_pending_peer(&mut self) -> Result<bool, String> {
        self.stop.recovery_pending_peer()
    }

    pub(super) fn recovery_operation(&self) -> Result<u64, String> {
        self.stop.recovery_operation()
    }

    /// A sealed terminal recovery never returns a live owner, even when the
    /// original admission Stop was acknowledged and its reply was lost.
    pub(super) fn stop_recovery<F>(
        &mut self,
        target: FileRecoveryStop,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        self.stop_recovery_with_history(target, None, deadline, time)
    }

    /// Preserve an acknowledged terminal cut even if its transport reply or a
    /// post-acknowledgment deadline failed. This never opens another live owner.
    pub(super) fn stop_recovery_with_history<F>(
        &mut self,
        target: FileRecoveryStop,
        history: Option<&super::anchors::Selection>,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<String, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        let outcome = self.stop.stop_recovery(target, deadline, time)?;
        let message = outcome.terminal_message();
        if let (Some(selected), Ok(stopped)) = (history, &outcome.application.result) {
            // The same sealed anchored replay supplied this successful native
            // acknowledgment. Its exact successor is safe to retain directly.
            return Ok(match selected.finish_capture(
                Ok(Some(stopped.history_anchor().clone())),
                Ok(()),
            ) {
                Ok(()) => message,
                Err(error) => format!("{message}; {error}"),
            });
        }
        Ok(message)
    }

    /// Preserve the already acknowledged original Stop archive even when its
    /// reply or a later budget observation fails. No owner or new replay escapes.
    pub(super) fn stop_recovery_with_anchor<F>(
        &mut self,
        target: FileRecoveryStop,
        deadline: &Deadline,
        time: &mut F,
        retained: &mut Option<FileHistoryAnchor>,
    ) -> Result<String, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        let outcome = self.stop.stop_recovery(target, deadline, time)?;
        *retained = outcome.application.result.as_ref().ok()
            .map(|history| history.history_anchor().clone());
        Ok(outcome.terminal_message())
    }
}
