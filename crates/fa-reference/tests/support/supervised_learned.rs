//! Original learned inference and real socket review; tiny weights and supplied
//! ballots are deterministic controls, not model or restart qualification.
use super::numerical;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{monitoring::restart::KvRestartBudget, sampling::{SamplingPolicy, SamplingStart,
        monitored::{GenerationBudget, GenerationEvent, GenerationSpec, GenerationTelemetryBudget}}},
    model::MAX_MODEL_KV_VALUES,
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{DispatchEnvelope, PublicationEndpoint};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, DispatchKeys,
    HelperContract, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::actor::{ActorPort, ActorProposal, ActorTicket, IntakeLimits};
use fa_reference::action::consequence::oversight::decoder_host::HostedStopPolicy;
use fa_reference::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;
use fa_reference::action::consequence::oversight::helper_client::{ClientProgress, HelperClient};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::consequence::oversight::human::{HumanReviewer, HumanReviewPolicy};
use fa_reference::action::consequence::oversight::learned_host::checkpoint::{
    HostedLearnedCheckpointHandle, HostedLearnedResetRequest,
};
use fa_reference::action::consequence::oversight::learned_source::LearnedSourceConfig;
use fa_reference::action::consequence::oversight::supervised::{DriverEvent, ReviewLaunch, SupervisedDriver};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::net::UnixStream;
use std::rc::Rc;

pub fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
pub fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }
}
pub fn proposal() -> ActorProposal {
    ActorProposal { target: target(), payload: b"publish".to_vec(), units: 16,
        deadline: ElapsedTick(100), expected_policy_epoch: 0 }
}
pub fn reset_request(broker: &OversightBroker, checkpoint: &HostedLearnedCheckpointHandle,
    config: &LearnedSourceConfig, round: u64) -> HostedLearnedResetRequest
{
    let state = broker.inspect();
    HostedLearnedResetRequest { checkpoint: checkpoint.clone(), expected_control_sequence: state.sequence,
        expected_actor_revision: broker.actor_revision(), expected_authority_epoch: state.ledger.epoch,
        binding: ReviewBinding { round, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: TargetCeiling::new(&[target()]).unwrap(),
        restart_budget: KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit: config.policy.allowance() } }
}

pub struct Rig {
    pub port: ActorPort,
    pub driver: SupervisedDriver,
    pub config: LearnedSourceConfig,
    pub inputs: Option<CommitteeInput>,
    pub clients: BTreeMap<String, HelperClient<UnixStream>>,
}
impl Rig {
    pub fn new(two_key: bool, mode: u8, automatic_stop: bool) -> (Self, Option<HumanReviewer>) {
        Self::with_endpoint(PublicationEndpoint::new(target(), b"old".to_vec(), 200, 16).unwrap(),
            two_key, mode, automatic_stop)
    }
    pub fn with_endpoint(endpoint: PublicationEndpoint, two_key: bool, mode: u8,
        automatic_stop: bool) -> (Self, Option<HumanReviewer>)
    {
        let contracts = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
            HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
                model_epoch: 3, tokenizer_epoch: 4, policy_epoch: 0 }, 1, b"approve?".to_vec()).unwrap(),
        )])).unwrap();
        let controller = ControllerConfig {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            // This registered grade is a caller claim, not an empirical test result.
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, Vec::new(), vec![0], vec![0], 0).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("reviewer".to_owned(),
                    MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
                continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
        };
        let (port, mut driver) = SupervisedDriver::new(controller, endpoint, contracts,
            IntakeLimits::default()).unwrap();
        let reviewer = two_key.then(|| driver.supervisor_mut().broker_mut().enable_human_review(
            HumanReviewPolicy { reviewer_id: 55, max_validity_ticks: 100, max_requests: 8 },
        ).unwrap());
        let model = numerical::model();
        let config = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
            spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
                policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
            }).unwrap(), policy: numerical::policy(&model, mode, 1),
            budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
        let broker = driver.supervisor_mut().broker_mut();
        broker.own_learned_generation(model, config.clone(), LearnedDecoderBindingLimits::default()).unwrap();
        if automatic_stop { broker.enable_learned_host_stop(HostedStopPolicy::new(1, 1, 700).unwrap()).unwrap(); }
        driver.observe_time(ElapsedTick(1)).unwrap();
        driver.confirm_dispatcher_fence().unwrap();
        (Self { port, driver, config, inputs: None, clients: BTreeMap::new() }, reviewer)
    }
    pub fn advance(&mut self) -> Rc<GenerationEvent> {
        let state = self.driver.supervisor().broker().hosted_learned_generation().unwrap();
        let result = self.driver.advance_hosted_learned(state.actor_revision, state.position, || ElapsedTick(1));
        result.synchronization.unwrap();
        assert!(result.containment.is_none());
        let event = result.inference.unwrap();
        assert!(event.accepted().is_some());
        event
    }
    pub fn checkpoint(&mut self, id: u64) -> HostedLearnedCheckpointHandle {
        let revision = self.driver.supervisor().broker().actor_revision();
        self.driver.capture_hosted_learned_checkpoint(id, revision).unwrap()
    }
    pub fn request(&self, checkpoint: &HostedLearnedCheckpointHandle, round: u64)
        -> HostedLearnedResetRequest
    {
        reset_request(self.driver.supervisor().broker(), checkpoint, &self.config, round)
    }
    pub fn accept(&mut self, request: u64) -> ActorTicket {
        let ticket = self.port.submit(request, &proposal()).unwrap();
        let intake = self.driver.accept_next(&snapshot()).unwrap().unwrap();
        assert_eq!(intake.request, request);
        assert!(intake.result.unwrap().is_some());
        ticket
    }
    pub fn start(&mut self, request: u64, round: u64) {
        let supervisor = self.driver.supervisor();
        let action = supervisor.action(request).unwrap();
        let contracts = supervisor.broker().contracts();
        let mut views = BTreeMap::new();
        let mut streams = BTreeMap::new();
        self.clients.clear();
        for (member, helper) in contracts.members() {
            let mut bytes = action_frame(action);
            let boundary = bytes.len();
            bytes.extend_from_slice(helper.question());
            let end = bytes.len();
            let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
                SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
                SubmittedPart { span: ByteSpan { start: boundary, end }, kind: PartKind::Question },
            ], Vec::new()).unwrap();
            views.insert(member.clone(), EvidenceViewManifest::new(actual, AuthorizationProjection {
                projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch,
                projected_originals: Vec::new(),
            }, Vec::new()).unwrap());
            let (server, client) = UnixStream::pair().unwrap();
            streams.insert(member.clone(), server);
            self.clients.insert(member.clone(), HelperClient::from_unix(client,
                helper.profile_at(action.spec().policy_epoch)).unwrap());
        }
        let inputs = CommitteeInput::capture(action, contracts, views).unwrap();
        self.inputs = Some(inputs.clone());
        let revision = supervisor.broker().input_revision(supervisor.attempt(request).unwrap()).unwrap();
        self.driver.start_review(ReviewLaunch { request, round, evidence_root: [1; 32],
            window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10) },
            expected_input_revision: revision, inputs, streams, limits: HelperLimits::default(),
        }, &snapshot()).unwrap();
    }
    pub fn finish_review(&mut self) {
        for _ in 0..128 {
            for (member, client) in &mut self.clients {
                if client.step().unwrap() == ClientProgress::NeedsInference {
                    assert_eq!(client.input().unwrap().actual_input(),
                        self.inputs.as_ref().unwrap().views()[member].actual_input());
                    client.respond(Verdict::Allow, member.as_bytes()).unwrap();
                }
            }
            match self.driver.step(ElapsedTick(1), self.inputs.as_ref(), &snapshot(), None).unwrap() {
                DriverEvent::Workers { .. } => {}
                DriverEvent::ReviewApplied { .. } => return,
                other => panic!("unexpected review event: {other:?}"),
            }
        }
        panic!("original socket review did not complete within its bounded fixture");
    }
    pub fn dispatch(&mut self, request: u64, reviewer: Option<&HumanReviewer>) -> DispatchEnvelope {
        let automatic = self.driver.supervisor_mut().authorize_request(request, self.inputs.as_ref(),
            &snapshot()).unwrap();
        let human = reviewer.map(|reviewer| {
            let request = self.driver.request_human_approval(500, self.inputs.as_ref(), ElapsedTick(40)).unwrap();
            reviewer.approve(&request, ElapsedTick(1)).unwrap()
        });
        self.driver.supervisor_mut().dispatch_request(request,
            DispatchKeys { automatic: &automatic, human: human.as_ref() }, self.inputs.as_ref(), &snapshot()).unwrap()
    }
    pub fn remove_live_inputs(&mut self, request: u64) {
        let attempt = self.driver.supervisor().attempt(request).unwrap();
        let broker = self.driver.supervisor_mut().broker_mut();
        let revision = broker.input_revision(attempt).unwrap();
        broker.inputs_unavailable(attempt, revision).unwrap();
        self.clients.clear(); self.inputs = None;
    }
    pub fn assert_empty_source(&self) {
        use fa_reference::action::consequence::oversight::learned_source::{LearnedAvailability, LearnedEvidenceLimits};
        let source = self.driver.supervisor().broker().hosted_learned_observation().unwrap();
        assert_eq!(source.availability(), LearnedAvailability::Empty);
        assert_eq!(source.capture(LearnedEvidenceLimits::default()).err(), Some(Error::Incomplete));
    }
}
