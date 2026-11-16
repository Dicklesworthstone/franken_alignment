//! Pump the selected original operator at existing native Stop checkpoints.
//! An operator transport failure retires only that sidecar. A granted Fence
//! is observed through the original Stop cleanup before any forward work.

use super::super::operator::Service;
use super::{Config, Deadline, ElapsedTick, FileHumanReviewer, FileSupervisedDriver, PeerProfile};
use crate::workflow::control::Control as StopControl;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop;

pub(super) struct Control {
    stop: StopControl,
    operator: Option<Service>,
}

impl Control {
    pub(super) fn new(
        config: &Config,
        operation: u64,
        profile: Option<&PeerProfile>,
    ) -> Result<Self, String> {
        Ok(Self { stop: StopControl::new(config, operation, profile)?, operator: None })
    }

    pub(super) fn install_operator(&mut self, operator: Option<Service>) {
        self.operator = operator;
    }

    pub(super) fn take_operator(&mut self) -> Option<Service> {
        self.operator.take()
    }

    pub(super) fn has_operator(&self) -> bool {
        self.operator.is_some()
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
            return Ok(true);
        }
        let Some(operator) = &mut self.operator else {
            return Ok(false);
        };
        operator.checkpoint();
        self.stop.native_stop(driver, time)
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
        self.stop
            .stop_recovery(target, deadline, time)
            .map(|outcome| outcome.terminal_message())
    }
}
