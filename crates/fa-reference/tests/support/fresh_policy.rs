//! Existing congress/actor fixture with mandatory live policy-state freshness.
#[path = "actor_gateway.rs"]
mod gateway;
use fa_reference::action::{ElapsedTick, Permit, Purpose, Scope};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::oversight::{CommitteeInput, DispatchKeys};
use fa_reference::action::consequence::oversight::actor::{ActorSupervisor, IntakeLimits};
use fa_reference::action::consequence::oversight::human::{HumanPermit, HumanReviewPolicy};
use fa_reference::action::consequence::oversight::policy_state::{
    PolicyStateWriter, StateEvent, StateFreshness, StateLimits, StateSource,
};
pub use gateway::{proposal, snapshot};

pub fn source() -> StateSource {
    StateSource { source: 9, generation: 1,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
}
pub fn publish(writer: &PolicyStateWriter, sequence: u64, at: u64) {
    let snapshot = snapshot();
    writer.record(sequence, &StateEvent::Snapshot {
        semantic_epoch: snapshot.semantic_epoch, values: snapshot.values,
    }).unwrap();
    writer.close_observed(sequence, sequence, ElapsedTick(at)).unwrap();
}
pub struct Ready {
    pub supervisor: ActorSupervisor,
    pub endpoint: PublicationEndpoint,
    pub writer: Option<PolicyStateWriter>,
    pub inputs: CommitteeInput,
    pub permit: Permit,
    pub human: Option<HumanPermit>,
}
impl Ready {
    pub fn new(two_key: bool) -> Self {
        let (port, supervisor, endpoint) = gateway::fixture(IntakeLimits::default());
        Self::prepare(port, supervisor, endpoint, two_key)
    }
    pub fn with_endpoint(mut endpoint: PublicationEndpoint, two_key: bool) -> Self {
        let (port, supervisor) = gateway::attach(&mut endpoint, IntakeLimits::default());
        Self::prepare(port, supervisor, endpoint, two_key)
    }
    fn prepare(
        port: fa_reference::action::consequence::oversight::actor::ActorPort,
        mut supervisor: ActorSupervisor, endpoint: PublicationEndpoint, two_key: bool,
    ) -> Self {
        let reviewer = two_key.then(|| supervisor.broker_mut().enable_human_review(HumanReviewPolicy {
            reviewer_id: 90, max_validity_ticks: 100, max_requests: 16,
        }).unwrap());
        let writer = supervisor.broker_mut().enable_fresh_policy_state(
            source(), StateLimits::default(), StateFreshness::new(4).unwrap(),
        ).unwrap();
        publish(&writer, 1, 1);
        port.submit(42, &proposal()).unwrap();
        supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap();
        let inputs = gateway::review(&mut supervisor, 42, 11);
        let permit = supervisor.authorize_request(42, Some(&inputs), &snapshot()).unwrap();
        let human = reviewer.map(|reviewer| {
            let attempt = supervisor.attempt(42).unwrap();
            let request = supervisor.broker_mut().request_human_approval(
                88, attempt, Some(&inputs), ElapsedTick(90),
            ).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        Self { supervisor, endpoint, writer: Some(writer), inputs, permit, human }
    }
    pub fn dispatch(&mut self) -> Result<fa_reference::action::consequence::delivery::DispatchEnvelope, fa_reference::Error> {
        self.supervisor.dispatch_request(42, DispatchKeys {
            automatic: &self.permit, human: self.human.as_ref(),
        }, Some(&self.inputs), &snapshot())
    }
}
