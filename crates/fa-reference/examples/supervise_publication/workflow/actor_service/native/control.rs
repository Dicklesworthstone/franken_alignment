//! Pump the selected original operator at existing native Stop checkpoints.
//! An operator transport failure retires only that sidecar. A granted Fence
//! is observed through the original Stop cleanup before any forward work.

use super::super::operator::Service;
use super::{Config, Deadline, ElapsedTick, FileHumanReviewer, FileSupervisedDriver, PeerProfile};
use crate::workflow::control::Control as StopControl;

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

    pub(super) fn into_parts(self) -> (StopControl, Option<Service>) {
        (self.stop, self.operator)
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
}
