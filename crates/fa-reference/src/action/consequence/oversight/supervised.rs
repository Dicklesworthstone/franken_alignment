//! A supervised actor request driven through the existing congress and endpoint.
//! The host supplies actual evidence, connected workers and the elapsed clock.
//! This is a synchronous integration owner, not another executor or authority.

use super::actor::{ActorPort, ActorSupervisor, IntakeLimits, IntakeResult};
use super::helper_workers::HelperLimits;
use super::helper_workers::io::{HelperPool, HelperPump, WorkerIoError};
use super::human::{HumanPermit, HumanRequest};
use super::{CommitteeContract, CommitteeInput, DispatchKeys, ObservedReceipt, ReviewWindow};
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::{EndpointReceipt, PublicationEndpoint};
use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use crate::action::{ActionState, ElapsedTick, Permit};
use crate::{Error, Snapshot};
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;

/// Trusted launch data. The actor cannot provide helper identities or evidence.
pub struct ReviewLaunch {
    pub request: u64,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub window: ReviewWindow,
    pub expected_input_revision: u64,
    pub inputs: CommitteeInput,
    pub streams: BTreeMap<String, UnixStream>,
    pub limits: HelperLimits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverPhase { Idle, Reviewing { request: u64 }, AwaitingDispatch { request: u64 } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverError { Control(Error), Worker(WorkerIoError) }
impl From<Error> for DriverError { fn from(value: Error) -> Self { Self::Control(value) } }
impl From<WorkerIoError> for DriverError { fn from(value: WorkerIoError) -> Self { Self::Worker(value) } }

/// Supervisor-only results. ActorPort continues to expose only its redacted
/// Knowledge projection. A completed review is not a publication receipt.
#[derive(Debug)]
pub enum DriverEvent {
    Idle,
    Workers { request: u64, report: HelperPump },
    ReviewApplied { request: u64, receipt: Box<ObservedReceipt> },
    ReviewRejected { request: u64, error: Error },
    AwaitingHuman { request: u64 },
    PublicationResolved { request: u64, receipt: EndpointReceipt },
    DeliveryUnknown { request: u64, error: Error },
    Stopped { request: u64, state: ActionState },
}

struct Job {
    request: u64,
    attempt: u64,
    inputs: CommitteeInput,
    revision: u64,
    sequence: Option<u64>,
    pool: Option<HelperPool>,
    permit: Option<Permit>,
}

/// One active review in the authority domain: the original congress binds a
/// global control predecessor, so parallel reviews cannot be blindly applied.
/// Queued actor requests remain in the original bounded FIFO. No helper result,
/// receipt, key or ledger is manufactured by this driver.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor::ActorPort;
/// fn drive(port: ActorPort) { let _ = port.start_review(); }
/// ```
pub struct SupervisedDriver {
    supervisor: ActorSupervisor,
    endpoint: PublicationEndpoint,
    job: Option<Job>,
}

impl SupervisedDriver {
    pub fn new(
        config: ControllerConfig, mut endpoint: PublicationEndpoint,
        contracts: CommitteeContract, limits: IntakeLimits,
    ) -> Result<(ActorPort, Self), Error> {
        let (port, supervisor) = ActorSupervisor::new(config, &mut endpoint, contracts, limits)?;
        Ok((port, Self { supervisor, endpoint, job: None }))
    }

    /// Trusted bootstrap/governance only. These handles must not reach actors.
    pub fn supervisor(&self) -> &ActorSupervisor { &self.supervisor }
    pub fn supervisor_mut(&mut self) -> &mut ActorSupervisor { &mut self.supervisor }
    pub fn endpoint(&self) -> &PublicationEndpoint { &self.endpoint }
    pub fn endpoint_mut(&mut self) -> &mut PublicationEndpoint { &mut self.endpoint }

    pub fn phase(&self) -> DriverPhase {
        match &self.job {
            None => DriverPhase::Idle,
            Some(job) if job.pool.is_some() => DriverPhase::Reviewing { request: job.request },
            Some(job) => DriverPhase::AwaitingDispatch { request: job.request },
        }
    }

    /// Both observations are monotone, but this is not an atomic remote clock
    /// update. A later refusal does not roll back a preceding clock observation.
    pub fn observe_time(&mut self, now: ElapsedTick) -> Result<(), Error> {
        self.endpoint.observe_time(now)?;
        self.supervisor.broker_mut().observe_time(now)
    }

    pub fn confirm_dispatcher_fence(&mut self) -> Result<(), Error> {
        let request = self.supervisor.broker().fence_request();
        let acknowledgment = self.endpoint.install_fence(request)?;
        self.supervisor.broker_mut().confirm_fence(acknowledgment)
    }

    /// A refused intake remains terminal for its original actor key. This does
    /// not poll a provider, invent a snapshot, or reroll a refused proposal.
    pub fn accept_next(&mut self, snapshot: &Snapshot) -> Result<Option<IntakeResult>, Error> {
        if self.job.is_some() { return Err(Error::WrongState); }
        self.supervisor.accept_next(snapshot)
    }

    /// Setup errors never send input bytes. If session creation succeeded before
    /// an I/O setup refusal, its round ID stays used in the original broker.
    pub fn start_review(&mut self, launch: ReviewLaunch, snapshot: &Snapshot) -> Result<(), DriverError> {
        if self.job.is_some() { return Err(Error::WrongState.into()); }
        self.supervisor.synchronize()?;
        let attempt = self.supervisor.attempt(launch.request)?;
        let broker = self.supervisor.broker();
        if !matches!(broker.inspect().ledger.stages.get(&attempt), Some(ActionState::Reviewing | ActionState::Authorized)) {
            return Err(Error::WrongState.into());
        }
        if !broker.contracts().members().keys().eq(launch.streams.keys()) {
            return Err(Error::Binding.into());
        }
        launch.inputs.validate_for(self.supervisor.action(launch.request)?, broker.contracts())?;
        let revision = self.supervisor.broker_mut().record_inputs(
            attempt, launch.expected_input_revision, launch.inputs.clone(),
        )?;
        let session = self.supervisor.broker_mut().begin_review(
            attempt, launch.round, launch.evidence_root, launch.window, snapshot,
        )?;
        let pool = HelperPool::new(session, launch.streams, launch.limits)?;
        self.job = Some(Job { request: launch.request, attempt, inputs: launch.inputs,
            revision, sequence: None, pool: Some(pool), permit: None });
        Ok(())
    }

    /// A scheduling hint, never evidence that a reviewed action is authorized.
    pub fn next_review_deadline(&self) -> Option<ElapsedTick> {
        self.job.as_ref().and_then(|job| job.pool.as_ref()).and_then(HelperPool::next_deadline)
    }

    /// Explicit withdrawal uses the real ledger. Dropping a helper round does
    /// not fabricate a verdict, and an in-flight effect cannot be cancelled here.
    pub fn cancel_active(&mut self) -> Result<(), Error> {
        let job = self.job.as_ref().ok_or(Error::Missing)?;
        self.supervisor.broker_mut().cancel(job.attempt)?;
        self.supervisor.synchronize()?;
        self.job = None;
        Ok(())
    }

    pub fn request_human_approval(
        &mut self, key: u64, current: Option<&CommitteeInput>, expires_at: ElapsedTick,
    ) -> Result<HumanRequest, Error> {
        self.supervisor.synchronize()?;
        self.check_ready(current)?;
        let attempt = self.job.as_ref().ok_or(Error::Missing)?.attempt;
        self.supervisor.broker_mut().request_human_approval(key, attempt, current, expires_at)
    }

    /// Controlled-time hosts may treat this as one logical-tick batch. Physical
    /// elapsed-clock hosts must instead use step_with_clock below.
    pub fn step(
        &mut self, now: ElapsedTick, current: Option<&CommitteeInput>,
        snapshot: &Snapshot, human: Option<&HumanPermit>,
    ) -> Result<DriverEvent, DriverError> {
        self.step_with_clock(|| now, current, snapshot, human)
    }

    /// One helper-pool pass OR one publication attempt, never both. The callback
    /// is the trusted host's clock, not a helper timestamp. Completed reviews
    /// are applied at a fresh post-I/O tick; dispatch observes a fresh tick after
    /// reservation. Transient pre-dispatch errors keep the original permit.
    pub fn step_with_clock<F>(
        &mut self, mut clock: F, current: Option<&CommitteeInput>,
        snapshot: &Snapshot, human: Option<&HumanPermit>,
    ) -> Result<DriverEvent, DriverError>
    where F: FnMut() -> ElapsedTick {
        self.observe_time(clock())?;
        self.supervisor.synchronize()?;
        let Some(job) = self.job.as_ref() else { return Ok(DriverEvent::Idle); };
        let request = job.request;
        let stage = *self.supervisor.broker().inspect().ledger.stages.get(&job.attempt).ok_or(Error::Missing)?;
        if !matches!(stage, ActionState::Reviewing | ActionState::Authorized) {
            self.job = None;
            return Ok(DriverEvent::Stopped { request, state: stage });
        }
        if job.pool.is_some() {
            let pumped = self.job.as_mut().expect("active job").pool.as_mut().expect("review pool")
                .pump_with_clock(&mut clock);
            let now = clock();
            self.observe_time(now)?;
            let report = pumped?;
            let pool = self.job.as_mut().expect("active job").pool.as_mut().expect("review pool");
            if !pool.ready_to_finish() { return Ok(DriverEvent::Workers { request, report }); }
            let review = pool.finish(now)?;
            // The consumed review must never be applied twice, including when
            // current evidence or governance changed during helper execution.
            let mut completed = self.job.take().expect("active job");
            completed.pool = None;
            let applied = self.supervisor.broker_mut().apply_review(review, current, snapshot);
            self.supervisor.synchronize()?;
            return match applied {
                Ok(receipt) => {
                    if receipt.policy.control.decision.consequence == Consequence::Continue {
                        completed.sequence = Some(receipt.policy.control.sequence);
                        self.job = Some(completed);
                    }
                    Ok(DriverEvent::ReviewApplied { request, receipt: Box::new(receipt) })
                }
                Err(error) => Ok(DriverEvent::ReviewRejected { request, error }),
            };
        }
        self.check_ready(current)?;
        match (self.supervisor.broker().human_review_required(), human.is_some()) {
            (true, false) => return Ok(DriverEvent::AwaitingHuman { request }),
            (false, true) => return Err(Error::Binding.into()),
            _ => {}
        }
        let job = self.job.as_mut().expect("ready job");
        if job.permit.is_none() {
            job.permit = Some(self.supervisor.authorize_request(request, current, snapshot)?);
        }
        self.observe_time(clock())?;
        let job = self.job.as_ref().expect("ready job");
        let keys = DispatchKeys { automatic: job.permit.as_ref().expect("reserved original permit"), human };
        let result = self.supervisor.deliver_request(request, keys, current, snapshot, &mut self.endpoint);
        match result {
            Ok(receipt) => {
                self.job = None;
                Ok(DriverEvent::PublicationResolved { request, receipt })
            }
            Err(error) => {
                let stage = self.supervisor.broker().inspect().ledger.stages[&job.attempt];
                if matches!(stage, ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown) {
                    self.job = None;
                    Ok(DriverEvent::DeliveryUnknown { request, error })
                } else if !matches!(stage, ActionState::Reviewing | ActionState::Authorized) {
                    self.job = None;
                    Ok(DriverEvent::Stopped { request, state: stage })
                } else {
                    Err(error.into())
                }
            }
        }
    }

    fn check_ready(&self, current: Option<&CommitteeInput>) -> Result<(), Error> {
        let job = self.job.as_ref().ok_or(Error::Missing)?;
        if job.pool.is_some() { return Err(Error::Incomplete); }
        let broker = self.supervisor.broker();
        if job.sequence != Some(broker.inspect().sequence) || job.revision != broker.input_revision(job.attempt)? {
            return Err(Error::Stale);
        }
        if current.ok_or(Error::Incomplete)? != &job.inputs { return Err(Error::Stale); }
        Ok(())
    }
}
