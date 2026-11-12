//! Explicit successor rounds for one original held ordinary request.
//! No actor poll, source change, old receipt or helper result requests a retry.

use super::{
    Config, Deadline, ElapsedTick, FileHumanReviewer, FileReviewLaunch, FileSupervisedDriver,
    HelperLimits, LiveControl, ReviewWindow, debug, pause, plus,
};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use std::collections::BTreeMap;

pub(super) struct Continuation {
    request: u64,
    programs: BTreeMap<String, HelperProgram>,
}

impl Continuation {
    /// Freeze the host-selected recipe before the first worker starts. The
    /// original optional operator custodian determines whether Hold may wait;
    /// legacy callers retain their original cancellation behavior.
    pub(super) fn new(
        driver: &FileSupervisedDriver,
        request: u64,
        programs: &BTreeMap<String, HelperProgram>,
    ) -> Result<Option<Self>, String> {
        Ok(driver
            .supervisor()
            .has_live_convening()
            .map_err(debug)?
            .then(|| Self {
                request,
                programs: programs.clone(),
            }))
    }

    /// Wait only within the original action/service lifetime. The original
    /// Stop path remains first; operator and actor ingress retain their same
    /// identities, quotas and listeners. Each consumed ticket can launch once.
    pub(super) fn wait<F>(
        &self,
        driver: &mut FileSupervisedDriver,
        reviewer: &FileHumanReviewer,
        config: &mut Config,
        deadline: &Deadline,
        control: &mut LiveControl<'_>,
        time: &mut F,
    ) -> Result<bool, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        loop {
            deadline.check(time())?;
            if control.checkpoint(driver, reviewer, deadline, time)? {
                return Ok(false);
            }
            // Retiring the operator listener drops the original custodian and
            // revokes its unconsumed requests. No replacement grant is issued.
            if !driver.supervisor().has_live_convening().map_err(debug)? {
                return Ok(false);
            }
            driver.reap_helpers();
            if driver.helpers_reaped() {
                if let Some(ticket) = driver.take_pending_convene(self.request).map_err(debug)? {
                    if ticket.request() != self.request {
                        return Err("held-review handoff belongs to a different request".into());
                    }
                    let now = time();
                    deadline.check(now)?;
                    let window = ReviewWindow {
                        commit_by: plus(now, config.timing.commit_ms)?,
                        reveal_by: plus(now, config.timing.reveal_ms)?,
                    };
                    if window.reveal_by >= deadline.logical {
                        return Err(
                            "insufficient original action lifetime for the requested successor review"
                                .into(),
                        );
                    }
                    let launch = FileReviewLaunch {
                        request: self.request,
                        round: ticket.round(),
                        window,
                        expected_input_revision: ticket.expected().input_revision,
                        workers: self.programs.clone(),
                        limits: HelperLimits::default(),
                    };
                    // This native method validates the exact original held
                    // predecessor before any read, then captures the source
                    // afresh and opens an entirely new frozen round. Identical
                    // complete evidence is valid for an explicit bounded retry
                    // after helper recovery. A failure never retries itself.
                    driver
                        .start_file_convened_process_review(
                            &mut config.source,
                            ticket,
                            launch,
                            &mut *time,
                        )
                        .map_err(debug)?;
                    return Ok(true);
                }
            }
            pause(config.timing.poll_ms);
        }
    }
}
