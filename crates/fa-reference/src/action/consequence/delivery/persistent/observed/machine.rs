//! All decisions and accounting belong to the ORIGINAL OversightBroker.
//! Replaying this machine has no filesystem or process/network endpoint.
use super::{FileOversightProfile, journal::{Event, HumanDecision}, views::Views};
use super::super::{Event as BaseEvent, FileDeliverySnapshot, FileStopSweep, Reconciliation, project_status};
use super::super::super::{DispatchEnvelope, EndpointOutcome, PublicationEndpoint, StopReceipt};
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use crate::action::consequence::oversight::{CommitteeInput, ObservedReceipt, ObservedSession, OversightBroker};
use crate::action::consequence::oversight::human::{HumanPermit, HumanRequest, HumanReviewer, HumanRevocation};
use crate::action::{ActionState, ElapsedTick, FrozenAction, Permit};
use crate::Error;
use std::collections::BTreeMap;

pub(super) enum Transition {
    Unit,
    Proposed(FrozenAction),
    Inputs(u64),
    Reviewed(Result<ObservedReceipt, Error>),
    HumanRequested(HumanRequest),
    HumansRevoked(HumanRevocation),
    Published(EndpointOutcome),
    Reconciled(Reconciliation),
    Swept(BTreeMap<u64, Result<Reconciliation, Error>>),
    Stopped(StopReceipt),
    StopProgressed(FileStopSweep),
}

pub(super) struct Machine {
    pub(super) broker: OversightBroker,
    pub(super) actions: BTreeMap<u64, FrozenAction>,
    pub(super) sessions: BTreeMap<u64, (u64, ObservedSession)>,
    pub(super) clock_ready: bool,
    endpoint: PublicationEndpoint,
    automatic: BTreeMap<u64, Permit>,
    human_keys: BTreeMap<u64, HumanPermit>,
    reviewer: HumanReviewer,
    envelopes: BTreeMap<u64, DispatchEnvelope>,
}
impl Machine {
    pub(super) fn new(p: &FileOversightProfile) -> Result<Self, Error> {
        super::super::codec::validate_profile(&p.delivery)?;
        let d = &p.delivery;
        let mut endpoint = PublicationEndpoint::new(d.target, d.initial_payload.clone(), d.retention_ticks, d.max_deliveries)?;
        let mut broker = OversightBroker::new(ControllerConfig {
            scope: d.scope, total: d.total, max_attempts: d.max_attempts, actor: d.actor.clone(),
            suspend_at_incident: d.suspend_at_incident, policy: d.policy.clone(), congress: d.congress.clone(),
            narrowed_targets: TargetCeiling::new(&d.narrowed_targets)?,
        }, &mut endpoint, p.committee.clone())?;
        let reviewer = broker.enable_human_review(p.human)?;
        broker.confirm_fence(endpoint.install_fence(broker.fence_request())?)?;
        Ok(Self { broker, endpoint, reviewer, actions: BTreeMap::new(), sessions: BTreeMap::new(),
            automatic: BTreeMap::new(), human_keys: BTreeMap::new(), envelopes: BTreeMap::new(), clock_ready: false })
    }
    pub(super) fn replay(p: &FileOversightProfile, events: &[Event]) -> Result<Self, Error> {
        let mut machine = Self::new(p)?;
        for event in events { machine.apply(event)?; }
        Ok(machine)
    }
    pub(super) fn snapshot(&self, revision: usize) -> FileDeliverySnapshot {
        FileDeliverySnapshot { revision: revision as u64, control: self.broker.inspect(),
            dispatcher_epoch: self.broker.fence_request().epoch, stop: self.broker.stop_receipt().cloned(),
            target: self.endpoint.target(), payload: self.endpoint.payload().to_vec(), executions: self.endpoint.execution_count() }
    }
    fn now(&self) -> Result<ElapsedTick, Error> { self.broker.inspect().ledger.elapsed.ok_or(Error::Incomplete) }
    fn historical_withdrawal_tick(&self) -> ElapsedTick { self.broker.inspect().ledger.elapsed.unwrap_or(ElapsedTick(0)) }
    fn observe(&mut self, tick: ElapsedTick) -> Result<(), Error> {
        self.broker.observe_time(tick)?;
        self.endpoint.observe_time(tick)?;
        self.clock_ready = true;
        Ok(())
    }
    fn capture(&self, attempt: u64, views: &Views) -> Result<CommitteeInput, Error> {
        CommitteeInput::capture(self.actions.get(&attempt).ok_or(Error::Missing)?, self.broker.contracts(), views.clone())
    }
    fn current(&self, attempt: u64, revision: u64) -> Result<CommitteeInput, Error> {
        if self.broker.input_revision(attempt)? != revision { return Err(Error::Stale); }
        self.broker.current_inputs(attempt)?.cloned().ok_or(Error::Incomplete)
    }
    fn withdraw_keys(&mut self) -> Result<HumanRevocation, Error> {
        // A historical tick is used ONLY to withdraw keys, never grant one or
        // reopen an execution window. Every human transition used broker time.
        let result = self.reviewer.revoke_all(self.historical_withdrawal_tick())?;
        self.human_keys.clear();
        Ok(result)
    }
    fn clear_sendable(&mut self) {
        self.sessions.clear();
        self.automatic.clear();
        self.human_keys.clear();
        self.envelopes.clear();
    }
    fn recover(&mut self) -> Result<(), Error> {
        self.withdraw_keys()?;
        self.broker.revoke_epoch()?;
        for (id, stage) in self.broker.inspect().ledger.stages {
            if matches!(stage, ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized) {
                self.broker.cancel(id)?;
            }
        }
        let fence = self.broker.restart_dispatcher()?;
        self.broker.confirm_fence(self.endpoint.install_fence(fence)?)?;
        self.clear_sendable();
        self.clock_ready = false;
        Ok(())
    }

    pub(super) fn apply(&mut self, event: &Event) -> Result<Transition, Error> {
        let without_current_time = matches!(event,
            Event::Core(BaseEvent::Time(_) | BaseEvent::Cancel(_) | BaseEvent::Fence | BaseEvent::Stop(_) | BaseEvent::StopProgress(_))
            | Event::InputsUnavailable(..) | Event::Human(_, HumanDecision::Reject | HumanDecision::Revoke) | Event::RevokeHumans);
        if !without_current_time && !self.clock_ready { return Err(Error::Incomplete); }
        match event {
            Event::Core(event) => return self.apply_core(event),
            Event::Inputs(id, revision, views) => {
                let inputs = self.capture(*id, views)?;
                return Ok(Transition::Inputs(self.broker.record_inputs(*id, *revision, inputs)?));
            }
            Event::InputsUnavailable(id, revision) => {
                return Ok(Transition::Inputs(self.broker.inputs_unavailable(*id, *revision)?));
            }
            Event::Begin(id, round, root, window, snapshot) => {
                let session = self.broker.begin_review(*id, *round, *root, *window, snapshot)?;
                self.sessions.insert(*round, (*id, session));
            }
            Event::Commit(round, member, digest) => {
                let now = self.now()?;
                self.sessions.get_mut(round).ok_or(Error::Missing)?.1.commit_from_worker(member, *digest, now)?;
            }
            Event::OpenReveals(round) => {
                let now = self.now()?;
                self.sessions.get_mut(round).ok_or(Error::Missing)?.1.open_reveals(now)?;
            }
            Event::Reveal(round, member, verdict, salt) => {
                let now = self.now()?;
                self.sessions.get_mut(round).ok_or(Error::Missing)?.1.reveal(member, *verdict, salt, now)?;
            }
            Event::Finish(round, supplied, snapshot) => {
                let id = self.sessions.get(round).ok_or(Error::Missing)?.0;
                let current = supplied.as_ref().map(|views| self.capture(id, views)).transpose()?;
                let now = self.now()?;
                let review = self.sessions.get_mut(round).ok_or(Error::Missing)?.1.finish(now)?;
                self.sessions.remove(round);
                // Application refusal is a COMMITTED result. The original round
                // remains consumed, so stale input cannot be repaired by rerolling
                // this completed judgment or applying its evidence a second time.
                return Ok(Transition::Reviewed(self.broker.apply_review(review, current.as_ref(), snapshot)));
            }
            Event::Authorize(id, revision, snapshot) => {
                let current = self.current(*id, *revision)?;
                let permit = self.broker.authorize(*id, Some(&current), snapshot)?;
                self.automatic.insert(*id, permit);
            }
            Event::RequestHuman(request, id, revision, expires) => {
                let current = self.current(*id, *revision)?;
                return Ok(Transition::HumanRequested(self.broker.request_human_approval(*request, *id, Some(&current), *expires)?));
            }
            Event::Human(request, decision) => {
                let evidence = self.broker.human_request(*request)?;
                match decision {
                    HumanDecision::Approve => {
                        let key = self.reviewer.approve(&evidence, self.now()?)?;
                        self.human_keys.insert(*request, key);
                    }
                    HumanDecision::Reject => {
                        self.reviewer.reject(&evidence, self.historical_withdrawal_tick())?;
                        self.human_keys.remove(request);
                    }
                    HumanDecision::Revoke => {
                        self.reviewer.revoke(&evidence, self.historical_withdrawal_tick())?;
                        self.human_keys.remove(request);
                    }
                }
            }
            Event::RevokeHumans => return Ok(Transition::HumansRevoked(self.withdraw_keys()?)),
            Event::Dispatch(id, request, revision, snapshot) => {
                let current = self.current(*id, *revision)?;
                let automatic = self.automatic.get(id).ok_or(Error::Missing)?;
                let human = self.human_keys.get(request).ok_or(Error::Missing)?;
                let action = self.actions.get(id).ok_or(Error::Missing)?;
                let envelope = self.broker.dispatch_with_human(automatic, human, action, Some(&current), snapshot)?;
                self.envelopes.insert(*id, envelope);
                self.automatic.remove(id);
                self.human_keys.remove(request);
            }
        }
        Ok(Transition::Unit)
    }

    fn apply_core(&mut self, event: &BaseEvent) -> Result<Transition, Error> {
        match event {
            BaseEvent::Time(tick) => self.observe(*tick)?,
            BaseEvent::Propose(id, spec, snapshot) => {
                let action = self.broker.propose(*id, spec.clone(), snapshot)?.action;
                self.actions.insert(*id, action.clone());
                return Ok(Transition::Proposed(action));
            }
            BaseEvent::Publish(id) => {
                let receipt = self.endpoint.deliver(self.envelopes.get(id).ok_or(Error::Missing)?)?;
                return Ok(Transition::Published(receipt.outcome()));
            }
            BaseEvent::Reconcile(id) => {
                let query = self.broker.status_query(*id)?;
                let status = self.endpoint.status(&query)?;
                return Ok(Transition::Reconciled(project_status(self.broker.reconcile_status(&query, status)?)));
            }
            BaseEvent::Seal(id) => {
                let query = self.broker.status_query(*id)?;
                let receipt = self.endpoint.seal_unexecuted(&query)?;
                let outcome = receipt.outcome();
                self.broker.accept_receipt(receipt)?;
                return Ok(Transition::Reconciled(Reconciliation::Resolved(outcome)));
            }
            BaseEvent::Sweep => {
                let outcomes = self.broker.reconcile_pending(&mut self.endpoint)?;
                return Ok(Transition::Swept(outcomes.into_iter().map(|(id, result)| (id, result.map(project_status))).collect()));
            }
            BaseEvent::Cancel(id) => { self.broker.cancel(*id)?; self.automatic.remove(id); }
            BaseEvent::Fence => self.recover()?,
            BaseEvent::Stop(request) => {
                let receipt = self.broker.request_stop(*request)?;
                self.withdraw_keys()?;
                self.clear_sendable();
                return Ok(Transition::Stopped(receipt));
            }
            BaseEvent::StopProgress(tick) => {
                self.broker.stop_receipt().ok_or(Error::Incomplete)?;
                self.observe(*tick)?;
                let sweep = self.broker.progress_stop(&mut self.endpoint)?;
                return Ok(Transition::StopProgressed(FileStopSweep {
                    outcomes: sweep.outcomes.into_iter().map(|(id, result)| (id, result.map(project_status))).collect(),
                    progress: sweep.progress,
                }));
            }
            // Only the listed original operations are admitted. Neither old
            // one-key operations nor newly added gateway events gain authority.
            _ => return Err(Error::Binding),
        }
        Ok(Transition::Unit)
    }
}
