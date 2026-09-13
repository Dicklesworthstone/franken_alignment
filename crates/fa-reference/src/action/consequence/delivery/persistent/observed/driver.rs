//! Drive actor requests through the ORIGINAL durable full-input/two-key owner.
//! The host supplies observation providers, helper sockets, time and human keys.
//! This owner coordinates transitions; it is not another ledger or executor.

mod lifecycle;
mod workers;
pub use workers::{FileDriverProcessError, FileDriverRelease};
use workers::WorkerSet;
use crate::action::consequence::oversight::helper_processes::HelperChildren;

use super::{FileHumanPermit, FileHumanRequest, FileOversight};
use super::helpers::{FileHelperFailure, FileHelperLaunch, FileHelperSetupError};
use super::super::{FilePermit, JournalError, Reconciliation};
use super::super::requests::{FileRequestDisposition, FileRequestStatus};
use super::super::requests::actor::{FileActorPort, FileActorSupervisor};
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, ObservedReceipt, ReviewWindow};
use crate::action::consequence::oversight::helper_workers::HelperLimits;
use crate::action::consequence::oversight::helper_workers::io::HelperPump;
use crate::action::consequence::oversight::supervised::DriverEvidence;
use crate::action::{ActionState, ElapsedTick, FrozenAction};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::os::unix::net::UnixStream;

/// Explicit trusted review of an ALREADY durably submitted request. The actor
/// does not choose a roster, snapshot, evidence root, round, or helper process.
pub struct FileDriverLaunch<W = UnixStream> {
    pub request: u64,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub window: ReviewWindow,
    pub expected_input_revision: u64,
    pub inputs: CommitteeInput,
    pub workers: BTreeMap<String, W>,
    pub limits: HelperLimits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDriverPhase {
    Idle,
    Reviewing { request: u64 },
    AwaitingDispatch { request: u64 },
    AwaitingPublication { request: u64 },
    AwaitingReconciliation { request: u64 },
}

/// Supervisor diagnostics, never actor responses or effect capabilities. Each
/// publication event is separate from its subsequent authority acknowledgment.
#[derive(Debug)]
pub enum FileDriverEvent {
    Idle,
    Workers { request: u64, report: HelperPump },
    WorkersFailed { request: u64, failure: FileHelperFailure },
    ReviewApplied { request: u64, receipt: Box<ObservedReceipt> },
    ReviewRejected { request: u64, error: Error },
    AwaitingHuman { request: u64 },
    Dispatched { request: u64, attempt: u64 },
    Published { request: u64, outcome: EndpointOutcome },
    PublicationUnknown { request: u64, error: JournalError },
    Reconciled { request: u64, outcome: Reconciliation },
    Stopped { request: u64, stage: ActionState },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase { Review, Ready, Publish, Reconcile, Closed }
struct Job {
    issuer: Rc<()>,
    request: u64,
    attempt: u64,
    action: FrozenAction,
    inputs: Option<CommitteeInput>,
    input_revision: u64,
    control_sequence: Option<u64>,
    pool: Option<WorkerSet>,
    permit: Option<FilePermit>,
    phase: Phase,
}
impl Job {
    fn check_owner(&self, host: &FileOversight) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) { return Err(Error::Binding.into()); }
        Ok(())
    }
    fn close(&mut self) { self.permit = None; self.phase = Phase::Closed; }
    fn check_ready(&self, host: &FileOversight, current: Option<&CommitteeInput>) -> Result<(), JournalError> {
        self.check_owner(host)?;
        if self.phase != Phase::Ready { return Err(Error::WrongState.into()); }
        if self.control_sequence != Some(host.inspect().control.sequence)
            || self.input_revision != host.input_revision(self.attempt)? { return Err(Error::Stale.into()); }
        if current.ok_or(Error::Incomplete)? != self.inputs.as_ref().ok_or(Error::Incomplete)? { return Err(Error::Stale.into()); }
        Ok(())
    }
}

/// One active review, since the original congress binds a shared predecessor.
/// The existing actor port remains live; its original durable request identities
/// and restricted outcome projection are never replaced by this driver's phase.
/// The human reviewer stays separately owned and is never stored here.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::driver::FileSupervisedDriver;
/// fn approve(driver: FileSupervisedDriver) { driver.reviewer(); }
/// ```
#[must_use = "drive or release this owner and poll retained helper children"]
pub struct FileSupervisedDriver {
    supervisor: FileActorSupervisor<FileOversight>,
    job: Option<Job>,
    retiring: Option<HelperChildren>,
}
impl fmt::Debug for FileSupervisedDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisedDriver").field("phase", &self.phase()).finish_non_exhaustive()
    }
}
impl FileOversight {
    /// Move the exact locked owner into the existing actor gateway and driver.
    /// An opened host has already fenced old keys; this never restores a job.
    pub fn into_supervised_driver(self) -> (FileActorPort<FileOversight>, FileSupervisedDriver) {
        let (port, supervisor) = self.into_actor_gateway();
        (port, FileSupervisedDriver::new(supervisor))
    }
}
impl FileSupervisedDriver {
    /// Adopt an existing gateway without changing its port, identity or rights.
    pub fn new(supervisor: FileActorSupervisor<FileOversight>) -> Self { Self { supervisor, job: None, retiring: None } }
    pub fn supervisor(&self) -> &FileActorSupervisor<FileOversight> { &self.supervisor }
    /// Trusted integration, including use of a separately held FileHumanReviewer.
    /// Do not pass this role or its mutable host to the actor/helper processes.
    pub fn supervisor_mut(&mut self) -> &mut FileActorSupervisor<FileOversight> { &mut self.supervisor }
    pub fn phase(&self) -> FileDriverPhase {
        match &self.job {
            None => FileDriverPhase::Idle,
            Some(job) => match job.phase {
                Phase::Review => FileDriverPhase::Reviewing { request: job.request },
                Phase::Ready => FileDriverPhase::AwaitingDispatch { request: job.request },
                Phase::Publish => FileDriverPhase::AwaitingPublication { request: job.request },
                Phase::Reconcile => FileDriverPhase::AwaitingReconciliation { request: job.request },
                Phase::Closed => FileDriverPhase::Idle,
            },
        }
    }
    pub fn next_review_deadline(&self) -> Option<ElapsedTick> {
        self.job.as_ref().and_then(|job| job.pool.as_ref()).and_then(WorkerSet::next_deadline)
    }

    /// Current observations and exact action/roster checks precede capture.
    /// Input capture, clock observations and beginning the original round are
    /// separate durable operations; a later setup error does not roll them back.
    /// No helper I/O, automatic permit or human approval is produced here.
    pub fn start_review<F>(&mut self, launch: FileDriverLaunch, snapshot: Snapshot, mut clock: F)
        -> Result<(), FileHelperSetupError>
    where F: FnMut() -> ElapsedTick {
        if self.job.is_some() { return Err(Error::WrongState.into()); }
        self.ensure_child_slot()?;
        let mut host = self.supervisor.host_mut()?;
        let (attempt, action, input_revision) = prepare_review(&mut host, &launch, &snapshot, &mut clock)?;
        let revision = host.revision();
        let pool = host.begin_helper_review(revision, FileHelperLaunch {
            attempt, round: launch.round, evidence_root: launch.evidence_root, window: launch.window,
            expected_input_revision: input_revision, streams: launch.workers, limits: launch.limits,
        }, snapshot)?;
        self.job = Some(Job { issuer: Rc::clone(&host.issuer), request: launch.request, attempt, action, inputs: Some(launch.inputs),
            input_revision, control_sequence: None, pool: Some(WorkerSet::Sockets(pool)), permit: None, phase: Phase::Review });
        Ok(())
    }

    /// Request the independent key for this reviewed effect. This never approves
    /// it and does not reserve automatic rights while the reviewer is unavailable.
    pub fn request_human_approval(&mut self, key: u64, current: &CommitteeInput,
        expires_at: ElapsedTick, now: ElapsedTick) -> Result<FileHumanRequest, JournalError>
    {
        let result = (|| {
            let job = self.job.as_ref().ok_or(Error::Missing)?;
            let mut host = self.supervisor.host_mut()?;
            job.check_owner(&host)?;
            observe(&mut host, now)?;
            job.check_ready(&host, Some(current))?;
            let revision = host.revision();
            host.request_human_approval(revision, key, job.attempt, current, expires_at)
        })();
        self.reap_helpers();
        result
    }

    /// One review pass OR durable dispatch OR publication OR reconciliation.
    /// Providers are called after review I/O and again after reservation, never
    /// for existing outcome obligations. Clocks are sampled after each provider.
    /// No automatic re-review, human approval, source fallback or resend exists.
    pub fn step_with_evidence<F, P>(&mut self, mut clock: F, mut provider: P,
        human: Option<&FileHumanPermit>) -> Result<FileDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        self.reap_helpers();
        let result = (|| {
            let mut host = self.supervisor.host_mut()?;
            if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
            let Some(job) = &mut self.job else { return Ok(FileDriverEvent::Idle); };
            job.check_owner(&host)?;
            // Check the original ledger before a fallible clock or provider call.
            // External cancellation/fencing cannot leave a stale review driving.
            let current = stage(&host, job.request)?;
            if !matches!(current, ActionState::Reviewing | ActionState::Authorized | ActionState::Dispatching | ActionState::Unknown) {
                let request = job.request;
                job.close();
                return Ok(FileDriverEvent::Stopped { request, stage: current });
            }
            if current == ActionState::Unknown
                || (current == ActionState::Dispatching && matches!(job.phase, Phase::Review | Phase::Ready)) {
                job.permit = None; job.phase = Phase::Reconcile;
            }
            observe(&mut host, clock())?;
            step_job(&mut host, job, &mut clock, &mut provider, human)
        })();
        self.reap_helpers();
        if self.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) { self.job = None; }
        result
    }
}

fn prepare_review<W, F>(host: &mut FileOversight, launch: &FileDriverLaunch<W>, snapshot: &Snapshot,
    clock: &mut F) -> Result<(u64, FrozenAction, u64), JournalError>
where F: FnMut() -> ElapsedTick {
    let attempt = admitted(host.request_status(launch.request)?)?.0;
    if stage(host, launch.request)? != ActionState::Reviewing { return Err(Error::WrongState.into()); }
    if host.input_revision(attempt)? != launch.expected_input_revision { return Err(Error::Stale.into()); }
    let action = host.request_action(launch.request)?.clone();
    launch.inputs.validate_for(&action, &host.profile.committee)?;
    if !launch.workers.keys().eq(host.profile.committee.members().keys()) { return Err(Error::Binding.into()); }
    if !snapshot.complete { return Err(Error::Incomplete.into()); }
    observe(host, clock())?;
    let revision = host.revision();
    let input_revision = host.record_inputs(revision, attempt, launch.expected_input_revision, launch.inputs.clone())?;
    observe(host, clock())?;
    Ok((attempt, action, input_revision))
}

fn admitted(status: FileRequestStatus) -> Result<(u64, ActionState), JournalError> {
    match status.disposition {
        FileRequestDisposition::Admitted { attempt, stage } => Ok((attempt, stage)),
        FileRequestDisposition::NotAdmitted(_) => Err(Error::WrongState.into()),
    }
}
fn stage(host: &FileOversight, request: u64) -> Result<ActionState, JournalError> {
    Ok(admitted(host.request_status(request)?)?.1)
}
fn observe(host: &mut FileOversight, now: ElapsedTick) -> Result<(), JournalError> {
    if !host.clock_ready() || host.inspect().control.ledger.elapsed != Some(now) {
        let revision = host.revision();
        host.observe_time(revision, now)?;
    }
    Ok(())
}

struct Sample { evidence: DriverEvidence, failure: Option<Error> }
fn sample<P>(host: &mut FileOversight, job: &Job, provider: &mut P) -> Result<Sample, JournalError>
where P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error> {
    let result = provider(&job.action, &host.profile.committee).and_then(|evidence| {
        if !evidence.snapshot.complete { return Err(Error::Incomplete); }
        evidence.inputs.as_ref().ok_or(Error::Incomplete)?.validate_for(&job.action, &host.profile.committee)?;
        Ok(evidence)
    });
    let changed = result.as_ref().map_or(true, |evidence| evidence.inputs.as_ref() != job.inputs.as_ref());
    if changed && host.machine.broker.current_inputs(job.attempt)?.is_some() {
        let expected = host.input_revision(job.attempt)?;
        let revision = host.revision();
        host.inputs_unavailable(revision, job.attempt, expected)?;
    }
    Ok(match result {
        Ok(evidence) => Sample { evidence, failure: None },
        Err(error) => Sample { evidence: DriverEvidence { snapshot: Snapshot::default(), inputs: None }, failure: Some(error) },
    })
}

fn step_job<F, P>(host: &mut FileOversight, job: &mut Job, clock: &mut F,
    provider: &mut P, human: Option<&FileHumanPermit>) -> Result<FileDriverEvent, JournalError>
where F: FnMut() -> ElapsedTick,
    P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
{
    let request = job.request;
    match job.phase {
        Phase::Review => {
            let pool = job.pool.as_mut().ok_or(Error::Incomplete)?;
            let report = match pool.pump_with_clock(host, &mut *clock) {
                Ok(report) => report,
                Err(failure) => { job.close(); return Ok(FileDriverEvent::WorkersFailed { request, failure }); }
            };
            if !pool.ready_to_finish() { return Ok(FileDriverEvent::Workers { request, report }); }
            let captured = sample(host, job, provider)?;
            let now = clock();
            observe(host, now)?;
            let pool = job.pool.as_mut().ok_or(Error::Incomplete)?;
            let completed = pool.finish(host, now, captured.evidence.inputs.as_ref(), captured.evidence.snapshot);
            match completed {
                Ok(Ok(receipt)) => {
                    if receipt.policy.control.decision.consequence == Consequence::Continue {
                        job.control_sequence = Some(receipt.policy.control.sequence);
                        job.phase = Phase::Ready;
                    } else { job.close(); }
                    Ok(FileDriverEvent::ReviewApplied { request, receipt: Box::new(receipt) })
                }
                Ok(Err(error)) => { job.close(); Ok(FileDriverEvent::ReviewRejected { request, error }) }
                Err(error) => {
                    if pool.is_closed() { job.close(); }
                    Err(error)
                }
            }
        }
        Phase::Ready => {
            let captured = sample(host, job, provider)?;
            observe(host, clock())?;
            if let Some(error) = captured.failure { return Err(error.into()); }
            job.check_ready(host, captured.evidence.inputs.as_ref())?;
            let Some(human) = human else { return Ok(FileDriverEvent::AwaitingHuman { request }); };
            let current = captured.evidence.inputs.as_ref().ok_or(Error::Incomplete)?;
            if job.permit.is_none() {
                let revision = host.revision();
                job.permit = Some(host.authorize(revision, job.attempt, current, captured.evidence.snapshot)?);
            }
            // A successful reservation is not permission to reuse the provider
            // snapshot from before it. Source loss keeps the original reservation.
            let captured = sample(host, job, provider)?;
            observe(host, clock())?;
            if let Some(error) = captured.failure { return Err(error.into()); }
            job.check_ready(host, captured.evidence.inputs.as_ref())?;
            let revision = host.revision();
            host.dispatch(revision, job.permit.as_ref().ok_or(Error::Incomplete)?, human, &job.action,
                captured.evidence.inputs.as_ref().ok_or(Error::Incomplete)?, captured.evidence.snapshot)?;
            job.permit = None;
            job.phase = Phase::Publish;
            Ok(FileDriverEvent::Dispatched { request, attempt: job.attempt })
        }
        Phase::Publish => {
            // Before the publication call, retire its send path. A failed call
            // (including caught unwind) must never automatically publish again.
            job.phase = Phase::Reconcile;
            let revision = host.revision();
            match host.publish(revision, job.attempt) {
                Ok(outcome) => Ok(FileDriverEvent::Published { request, outcome }),
                Err(error) => Ok(FileDriverEvent::PublicationUnknown { request, error }),
            }
        }
        Phase::Reconcile => {
            let revision = host.revision();
            let outcome = host.reconcile(revision, job.attempt)?;
            job.close();
            Ok(FileDriverEvent::Reconciled { request, outcome })
        }
        Phase::Closed => Err(Error::WrongState.into()),
    }
}
