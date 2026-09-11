//! Managed executable launch for the existing supervised driver.
//! Child liveness and reap state are never substituted for congress evidence.

use super::{DriverError, Job, ReviewLaunch, SupervisedDriver};
use super::super::{CommitteeInput, ReviewWindow};
use super::super::helper_processes::{HelperChildren, HelperProgram, ProcessFailure, ProcessStatus, launch_helpers};
use super::super::helper_workers::{HelperLimits, HelperPhase};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;

/// Host configuration only. Neither actor submissions nor helper replies can
/// select executable paths, environment entries or the original frozen roster.
pub struct ProcessReviewLaunch {
    pub request: u64,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub window: ReviewWindow,
    pub expected_input_revision: u64,
    pub inputs: CommitteeInput,
    pub programs: BTreeMap<String, HelperProgram>,
    pub limits: HelperLimits,
}

/// Children from a partial launch stay owned by the DRIVER, not this diagnostic.
/// Even an error must be followed by inspecting/polling its helper processes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessReviewError {
    Driver(DriverError),
    Launch { member: Option<String>, failure: ProcessFailure },
}
impl From<Error> for ProcessReviewError {
    fn from(error: Error) -> Self { Self::Driver(DriverError::Control(error)) }
}

impl SupervisedDriver {
    /// Spawn once and connect the resulting entire roster to start_review.
    /// Input/attempt/revision and roster checks precede process creation. If a
    /// later original session/limit check refuses, children are stopped and kept
    /// for reaping; any consumed original round ID remains consumed.
    pub fn start_process_review(
        &mut self, launch: ProcessReviewLaunch, snapshot: &Snapshot,
    ) -> Result<(), ProcessReviewError> {
        self.check_review(launch.request, &launch.inputs, launch.expected_input_revision, &launch.programs)?;
        let (streams, children) = match launch_helpers(self.supervisor.broker().contracts(), &launch.programs) {
            Ok(launched) => launched,
            Err(failure) => {
                self.children = Some(failure.children);
                return Err(ProcessReviewError::Launch { member: failure.member, failure: failure.failure });
            }
        };
        let result = self.start_review(ReviewLaunch {
            request: launch.request, round: launch.round, evidence_root: launch.evidence_root,
            window: launch.window, expected_input_revision: launch.expected_input_revision,
            inputs: launch.inputs, streams, limits: launch.limits,
        }, snapshot);
        self.children = Some(children);
        self.reap_helpers();
        result.map_err(ProcessReviewError::Driver)
    }

    /// Supervisor-only OS observations, never an actor outcome or authorization.
    pub fn helper_processes(&self) -> BTreeMap<String, ProcessStatus> {
        self.children.as_ref().map_or_else(BTreeMap::new, HelperChildren::statuses)
    }
    pub fn helpers_reaped(&self) -> bool {
        self.children.as_ref().is_none_or(HelperChildren::all_reaped)
    }
    /// One bounded cleanup pass. Protocol-terminal children are stopped; other
    /// children are only observed. After review/cancellation, all are stopped.
    /// The host keeps polling during shutdown; Drop is only a best effort.
    pub fn reap_helpers(&mut self) -> BTreeMap<String, ProcessStatus> {
        maintain(&mut self.children, &self.job)
    }
    /// Stop direct children, without manufacturing votes, cancelling an effect,
    /// refunding rights, or changing the existing worker-missing denominator.
    pub fn stop_helper_processes(&mut self) -> BTreeMap<String, ProcessStatus> {
        self.children.as_mut().map_or_else(BTreeMap::new, HelperChildren::request_stop_all)
    }
}

pub(super) fn maintain(
    children: &mut Option<HelperChildren>, job: &Option<Job>,
) -> BTreeMap<String, ProcessStatus> {
    let Some(children) = children else { return BTreeMap::new(); };
    match job.as_ref().and_then(|job| job.pool.as_ref()) {
        Some(pool) => {
            for (member, status) in pool.statuses() {
                if matches!(status.phase, HelperPhase::Complete | HelperPhase::Failed | HelperPhase::Closed) {
                    // The child map was constructed from exactly this roster.
                    children.request_stop(&member).expect("owned process roster");
                }
            }
        }
        None => { children.request_stop_all(); }
    }
    children.reap()
}

/// One retained launch cohort bounds live and retiring OS children together.
/// This only runs when there is no active job. Reaped status may be replaced by
/// the next explicit launch, but a live child can never be forgotten to do so.
pub(super) fn ensure_slot(children: &mut Option<HelperChildren>) -> Result<(), Error> {
    if let Some(previous) = children {
        previous.request_stop_all();
        if !previous.all_reaped() { return Err(Error::Incomplete); }
    }
    *children = None;
    Ok(())
}
