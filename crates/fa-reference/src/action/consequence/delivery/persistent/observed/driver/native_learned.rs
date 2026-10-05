//! Original native-model sidecar sequences feeding the ORIGINAL supervised publication job.
//! The numerical sequence and the actor gateway have one owner during review.
//! Handoff retains the original work records and cannot manufacture either key.
use super::{FileDriverPhase, FileSupervisedDriver, Job, Phase, admitted, observe, stage};
use super::super::{FileOversight, JournalError};
use super::super::decoder::learned::sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish};
use super::super::helpers::learned::native::{NativeReviewCost, NativeReviewLimits, NativeReviewStatus,
    sequence::{FileNativeSidecarSequence, NativeSequenceRosters}};
use super::super::super::requests::actor::FileActorSupervisor;
use crate::action::consequence::Consequence;
use crate::action::consequence::oversight::learned_host::sidecar::workers::LearnedWorkerRound;
use crate::action::{ActionState, ElapsedTick};
use crate::{Error, Snapshot};
use std::fmt;
use std::rc::Rc;

#[cfg(test)]
mod tests;

/// Supervisor-selected original source, fixed native rosters, salts and finite schedule.
/// The request must already belong to this gateway's durable request book.
pub struct FileNativeDriverLaunch {
    pub journal_revision: u64,
    pub request: u64,
    pub sidecar: FileLearnedSidecar,
    pub rounds: Vec<LearnedWorkerRound>,
    pub rosters: NativeSequenceRosters,
    pub limits: NativeReviewLimits,
}

/// Setup may have acknowledged a clock or original review operation. Return the
/// SAME owner on error, rather than losing its lock, actor port or child custody.
#[derive(Debug)]
pub struct FileNativeDriverSetupError {
    pub error: JournalError,
    pub driver: FileSupervisedDriver,
}

/// Supervisor progress only. Neither a finished computation nor its counters
/// are approval, process-independence evidence or a successful external effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileNativeDriverProgress {
    pub request: u64,
    pub review_revision: u64,
    pub status: NativeReviewStatus,
    pub polls: usize,
    pub reservation: NativeReviewCost,
    pub input_revision: u64,
    pub phase: FileDriverPhase,
}

/// The ordinary driver is inaccessible while numerical review is running. The
/// actor still holds the original restricted port and can observe/cancel its
/// original ticket. No duplicate gateway, policy, evidence source or ledger is
/// created. The separate human-reviewer role is never installed in this owner.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::driver::native_learned::FileNativeSupervisedDriver;
/// fn early(run: &mut FileNativeSupervisedDriver) { run.driver_mut(); }
/// ```
#[must_use = "advance or cancel the review, then retain both handoff fields"]
pub struct FileNativeSupervisedDriver {
    driver: FileSupervisedDriver,
    review: FileNativeSidecarSequence,
    request: u64,
    input_revision: u64,
}
impl fmt::Debug for FileNativeSupervisedDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileNativeSupervisedDriver").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

/// Both halves are retained explicitly: the original publication state machine
/// and terminal numerical work/history. Returning a driver does not mean Ready;
/// callers inspect its original phase and the original review outcome.
#[derive(Debug)]
#[must_use = "retain original numerical work and drive or release the original authority"]
pub struct FileNativeDriverHandoff {
    pub driver: FileSupervisedDriver,
    pub review: FileNativeSidecarSequence,
}

impl FileSupervisedDriver {
    /// Bind a computed congress to this already-admitted actor request. This
    /// consumes the driver so an unrelated socket review cannot take its slot.
    /// Setup computes no new helper token and issues neither key; historical
    /// numerical replay still belongs to the original journal transactions.
    pub fn start_native_learned_sequence(mut self, launch: FileNativeDriverLaunch,
        snapshot: Snapshot, now: ElapsedTick)
        -> Result<FileNativeSupervisedDriver, Box<FileNativeDriverSetupError>>
    {
        let request = launch.request;
        let result = (|| {
            if self.job.is_some() || self.learned_probe_review().is_some() { return Err(JournalError::from(Error::WrongState)); }
            self.ensure_child_slot()?;
            let mut host = self.supervisor.host_mut()?;
            if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
            if launch.journal_revision != host.revision() { return Err(Error::Stale.into()); }
            let (attempt, current) = admitted(host.request_status(request)?)?;
            if current != ActionState::Reviewing { return Err(Error::WrongState.into()); }
            if attempt != launch.sidecar.attempt() { return Err(Error::Binding.into()); }
            let action = host.request_action(request)?.clone();
            let input = host.current_learned_sidecar(&launch.sidecar)?;
            if input.action() != &action { return Err(Error::Binding.into()); }
            let input = input.clone();
            let input_revision = host.input_revision(attempt)?;
            if input_revision != launch.sidecar.input_revision() { return Err(Error::Stale.into()); }
            if !snapshot.complete { return Err(Error::Incomplete.into()); }
            observe(&mut host, now)?;
            let revision = host.revision();
            let review = host.begin_native_sidecar_sequence(revision, launch.sidecar,
                launch.rounds, launch.rosters, launch.limits, snapshot)?;
            self.job = Some(Job { issuer: Rc::clone(&host.issuer), request, attempt, action,
                inputs: Some(input), input_revision, control_sequence: None, pool: None,
                permit: None, phase: Phase::Review });
            Ok((review, input_revision))
        })();
        match result {
            Ok((review, input_revision)) => Ok(FileNativeSupervisedDriver { driver: self, review, request, input_revision }),
            Err(error) => Err(Box::new(FileNativeDriverSetupError { error, driver: self })),
        }
    }
}

impl FileNativeSupervisedDriver {
    pub fn supervisor(&self) -> &FileActorSupervisor<FileOversight> { self.driver.supervisor() }
    /// Trusted original-host integration only; never give this role to the actor.
    pub fn supervisor_mut(&mut self) -> &mut FileActorSupervisor<FileOversight> { self.driver.supervisor_mut() }
    pub fn review(&self) -> &FileNativeSidecarSequence { &self.review }
    pub fn progress(&self) -> FileNativeDriverProgress {
        FileNativeDriverProgress { request: self.request, review_revision: self.review.revision(),
            status: self.review.status(), polls: self.review.polls(), reservation: self.review.reservation(),
            input_revision: self.input_revision, phase: self.driver.phase() }
    }

    /// One ORIGINAL cooperative numerical/protocol quantum. Refinement starts
    /// another original round; the old input cannot reach publication Ready.
    /// The caller's snapshot is a trusted policy observation, not a helper vote.
    pub fn advance(&mut self, expected_revision: u64, now: ElapsedTick, snapshot: Snapshot)
        -> Result<FileNativeDriverProgress, JournalError>
    {
        if expected_revision != self.review.revision() { return Err(Error::Stale.into()); }
        if self.review.status() != NativeReviewStatus::Running { return Err(Error::WrongState.into()); }
        {
            let mut host = self.driver.supervisor.host_mut()?;
            let job = self.driver.job.as_mut().ok_or(Error::Missing)?;
            job.check_owner(&host)?;
            if host.storage_failure().is_some() {
                self.review.cancel(expected_revision)?;
                job.close();
                return Err(JournalError::Unavailable);
            }
            let current = stage(&host, self.request)?;
            if current != ActionState::Reviewing || host.inspect().stop.is_some() {
                self.review.cancel(expected_revision)?;
                // A trusted lower-level dispatch is still an unknown effect.
                // Cancelling local computation does not settle or refund it.
                if matches!(current, ActionState::Dispatching | ActionState::Unknown) {
                    job.permit = None;
                    job.phase = Phase::Reconcile;
                } else { job.close(); }
            } else {
                if !snapshot.complete { return Err(Error::Incomplete.into()); }
                observe(&mut host, now)?;
                let result = self.review.advance(&mut host, expected_revision, now, snapshot);
                if self.review.status() == NativeReviewStatus::Failed {
                    job.close();
                }
                // Only an ORIGINAL acknowledged refinement advances the input
                // predecessor. A changed host revision alone is not admission.
                if let Some(input_revision) = self.review.history().iter().rev().find_map(|event| {
                    match event {
                        FileLearnedSidecarFinish::Refined { input_revision, .. } => Some(*input_revision),
                        _ => None,
                    }
                }) { self.input_revision = input_revision; }
                result?;
                if self.review.status() == NativeReviewStatus::Finished {
                    // Close BEFORE fallible handoff checks or allocations. Only
                    // this original acknowledged Continue can set Ready again.
                    job.close();
                    let Some(FileLearnedSidecarFinish::Applied { receipt, .. }) = self.review.history().last() else {
                        return Err(Error::Incomplete.into());
                    };
                    if let Ok(receipt) = receipt {
                        if receipt.policy.control.decision.consequence == Consequence::Continue {
                            let input_revision = self.input_revision;
                            if host.input_revision(job.attempt)? != input_revision
                                || host.inspect().control.sequence != receipt.policy.control.sequence
                                || self.review.input().action() != &job.action {
                                return Err(Error::Stale.into());
                            }
                            job.inputs = Some(self.review.input().clone());
                            job.input_revision = input_revision;
                            job.control_sequence = Some(receipt.policy.control.sequence);
                            job.phase = Phase::Ready;
                        }
                    }
                }
            }
        }
        self.driver.reap_helpers();
        Ok(self.progress())
    }

    /// Cancel unfinished numerical work, then use the ORIGINAL request cancel.
    /// A journal failure leaves the same owner and retained work inspectable.
    /// This cannot refund a dispatched/unknown effect or manufacture a result.
    pub fn cancel(&mut self, expected_revision: u64) -> Result<(), JournalError> {
        if expected_revision != self.review.revision() { return Err(Error::Stale.into()); }
        if self.review.status() == NativeReviewStatus::Running {
            self.review.cancel(expected_revision)?;
        }
        self.driver.cancel_active().map(|_| ())
    }

    /// Partial reviews never escape to an ordinary review/dispatch loop. A
    /// refusal returns this entire owner, including its work and live actor port.
    /// The finished driver keeps the existing fresh-evidence, both-key, final
    /// publication and unknown-reconciliation checks without reimplementing them.
    pub fn into_handoff(mut self) -> Result<FileNativeDriverHandoff, Box<Self>> {
        if self.review.status() == NativeReviewStatus::Running { return Err(Box::new(self)); }
        if let Some(job) = &mut self.driver.job {
            if job.phase == Phase::Review { job.close(); }
        }
        if self.driver.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) {
            self.driver.job = None;
        }
        Ok(FileNativeDriverHandoff { driver: self.driver, review: self.review })
    }
}
