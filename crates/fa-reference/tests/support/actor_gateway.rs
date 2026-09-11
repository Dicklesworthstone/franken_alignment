use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::actor::{ActorPort, ActorProposal, ActorSupervisor, IntakeLimits};
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::collections::BTreeMap;

pub fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}

pub fn proposal() -> ActorProposal {
    ActorProposal {
        target: ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 },
        payload: b"publish".to_vec(), units: 16, deadline: ElapsedTick(100), expected_policy_epoch: 0,
    }
}

pub fn fixture(limits: IntakeLimits) -> (ActorPort, ActorSupervisor, PublicationEndpoint) {
    let target = proposal().target;
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let contracts = CommitteeContract::new(BTreeMap::from([("secret-helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"secret-model-profile".to_vec(),
            model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"secret-detector-question".to_vec(),
    ).unwrap())])).unwrap();
    let config = ControllerConfig {
        scope, total: 100, max_attempts: 128,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("secret-helper".to_owned(), MemberPolicy { cohort: "secret-cohort".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(target, b"old".to_vec(), 200, 128).unwrap();
    let (port, mut supervisor) = ActorSupervisor::new(config, &mut endpoint, contracts, limits).unwrap();
    supervisor.broker_mut().observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    let ack = endpoint.install_fence(supervisor.broker().fence_request()).unwrap();
    supervisor.broker_mut().confirm_fence(ack).unwrap();
    (port, supervisor, endpoint)
}

pub fn review(supervisor: &mut ActorSupervisor, request: u64, round: u64) -> CommitteeInput {
    let id = supervisor.attempt(request).unwrap();
    let action = supervisor.action(request).unwrap().clone();
    let broker = supervisor.broker_mut();
    let contracts = broker.contracts().clone();
    let helper = &contracts.members()["secret-helper"];
    let mut bytes = action_frame(&action);
    let end = bytes.len();
    bytes.extend_from_slice(helper.question());
    let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("secret-helper".to_owned(), manifest)])).unwrap();
    let revision = broker.input_revision(id).unwrap();
    broker.record_inputs(id, revision, inputs.clone()).unwrap();
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, round, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let commitment = session.commitment("secret-helper", Verdict::Allow, b"secret-salt").unwrap();
    session.commit("secret-helper", commitment, now).unwrap();
    session.open_reveals(now).unwrap();
    session.reveal("secret-helper", Verdict::Allow, b"secret-salt", now).unwrap();
    let completed = session.finish(now).unwrap();
    broker.apply_review(completed, Some(&inputs), &snapshot()).unwrap();
    inputs
}
