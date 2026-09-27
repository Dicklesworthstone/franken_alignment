//! Real source fitting and original congress; tiny parameters and ballots are
//! synthetic plumbing controls, not detector calibration or live authentication.
#[path = "restart_model.rs"]
pub mod original;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::probe::{LinearProbe, learned::{
    CheckedKvBudget, CheckedLearnedKv, KvGroup, KvRow, ResidualRetention,
}};
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, MAX_DECODER_PRODUCTS}, experiment::KvSide,
    model::learned::{CompressionBudget, FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{TargetCeiling, containment::{
    ActorState, RestartGrade, RestartProfile, session::policy::{Policy, Predicate, controller::ControllerConfig},
}};
use fa_reference::action::consequence::oversight::{
    CommitteeContract, HelperContract, ObservedReview, OversightBroker, ReviewWindow,
    helper_workers::{HelperLimits, HelperPort, HelperRound, wire::{WorkerInput, decode_request, encode_request}},
    sidecar::{SidecarCommitteeRound, SidecarCongressBudget, SidecarCongressPlan, SidecarIdentity, SidecarRefinementOutcome},
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::BTreeMap;

pub fn row() -> KvRow { KvRow { layer: 1, side: KvSide::Value, position: 0 } }
pub fn group() -> KvGroup { KvGroup { row: row(), head: 0 } }
pub fn source(stream: u64) -> CheckedLearnedKv {
    let model = original::model();
    let budget = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], budget).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let source = model.recompute(stream, &[2], budget).unwrap().cache_image().unwrap();
    let (image, _) = codec.evaluate_held_out(201, &source, CompressionBudget::default()).unwrap();
    CheckedLearnedKv::new(image, &source, ResidualRetention::All, CheckedKvBudget::default()).unwrap()
}
pub fn probe(axis: usize, threshold: f32) -> LinearProbe {
    let model = original::model();
    let contract = model.cache_profile().layers()[&1].values();
    let mut weights = vec![0.0; contract.dimensions()]; weights[axis] = 1.0;
    LinearProbe::new(axis as u64 + 1, 1, contract.profile(), &weights, 0.0, threshold).unwrap()
}
pub fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }

pub struct Setup {
    pub owner: OversightBroker,
    pub endpoint: PublicationEndpoint,
    pub action: FrozenAction,
    pub source: CheckedLearnedKv,
    pub plan: SidecarCongressPlan,
    pub packet: SidecarCommitteeRound,
}
impl Setup {
    pub fn new(question: &[u8]) -> Self {
        let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
        let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
        let contract = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
            HelperContract::new(InputProfileBinding { profile_id: 7, profile_bytes: Vec::new(),
                model_epoch: 3, tokenizer_epoch: 4, policy_epoch: 0 }, 1, question.to_vec()).unwrap(),
        )])).unwrap();
        // This test uses the original unhosted reference authority. Its opaque
        // actor bytes are not presented as a numerical restart qualification.
        let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
            grade: RestartGrade::AuditOnly }, vec![2], vec![0], vec![0], 1).unwrap();
        let mut endpoint = PublicationEndpoint::new(target, b"initial".to_vec(), 1000, 8).unwrap();
        let mut owner = OversightBroker::new(ControllerConfig {
            scope, total: 100, max_attempts: 8, actor, suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
                continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
            }, narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
        }, &mut endpoint, contract).unwrap();
        owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
        owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
        let action = owner.propose(1, ActionSpec { version: VERSION, scope, target: Some(target),
            payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
            deadline: ElapsedTick(100), units: 16 }, &snapshot()).unwrap().action;
        let source = source(21);
        let mut plan = SidecarCongressPlan::new(source.clone(), SidecarIdentity {
            object_id: 1001, generation: 1, transform_id: 7 }, vec![group()], SidecarCongressBudget::default()).unwrap();
        let packet = plan.initial(&action, owner.contracts()).unwrap();
        owner.record_inputs(1, 0, packet.input().clone()).unwrap();
        Self { owner, endpoint, action, source, plan, packet }
    }
    pub fn workers(&mut self, id: u64) -> (HelperRound, BTreeMap<String, HelperPort>) {
        let session = self.owner.begin_review(1, id, [7; 32], ReviewWindow {
            commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
        }, &snapshot()).unwrap();
        HelperRound::new(session, HelperLimits::default()).unwrap()
    }
    pub fn refine(&mut self, review: &ObservedReview) {
        self.packet = match self.plan.refine_after(review, &self.action, self.owner.contracts()).unwrap() {
            SidecarRefinementOutcome::Refined { round, .. } => round,
            other => panic!("expected actual residual refinement, got {other:?}"),
        };
        self.owner.record_inputs(1, self.owner.input_revision(1).unwrap(), self.packet.input().clone()).unwrap();
    }
}
pub fn input(port: &HelperPort) -> WorkerInput { decode_request(&encode_request(port).unwrap()).unwrap() }
pub fn finish(round: &mut HelperRound, port: &HelperPort, verdict: Verdict) -> ObservedReview {
    let salt = b"actual-worker-control";
    port.submit_commitment(port.request().commitment(verdict, salt).unwrap()).unwrap();
    round.advance(ElapsedTick(1)).unwrap();
    port.reveal(verdict, salt).unwrap();
    round.finish(ElapsedTick(1)).unwrap()
}
pub fn refined() -> Setup {
    let mut setup = Setup::new(b"approve?");
    let (mut round, ports) = setup.workers(10);
    let review = finish(&mut round, &ports["reviewer"], Verdict::Abstain);
    setup.refine(&review); setup
}
pub fn expect_limit<T>(result: Result<T, Error>) { assert!(matches!(result, Err(Error::Limit))); }
