//! Shared public-API fixture for delayed two-key delivery and reconciliation.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{DispatchEnvelope, PublicationEndpoint};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::collections::BTreeMap;

/// Complete congress and key issuance, without delivering the returned message.
/// Optional expiry selects the separately provisioned human-review profile.
pub fn dispatched(expiry: Option<u64>) -> (OversightBroker, PublicationEndpoint, DispatchEnvelope) {
    let spec = ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: 0,
        deadline: ElapsedTick(100), units: 16,
    };
    let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) };
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"profile-v1".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review".to_vec(),
    ).unwrap())])).unwrap();
    let config = ControllerConfig {
        scope: spec.scope, total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile {
            id: 1, generation: 1, host_generation: 1, model_generation: 1, tokenizer_generation: 1,
            state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
        }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress: CongressPolicy {
            generation: 1, members: BTreeMap::from([("helper".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        },
        narrowed_targets: TargetCeiling::new(&[spec.target.unwrap()]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(spec.target.unwrap(), b"old".to_vec(), 200, 16).unwrap();
    let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
    let reviewer = expiry.map(|_| broker.enable_human_review(HumanReviewPolicy {
        reviewer_id: 90, max_validity_ticks: 100, max_requests: 16,
    }).unwrap());
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    let action = broker.propose(1, spec, &snapshot).unwrap().action;
    let helper = &contracts.members()["helper"];
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
    let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("helper".to_owned(), manifest)])).unwrap();
    broker.record_inputs(1, 0, inputs.clone()).unwrap();
    let mut session = broker.begin_review(1, 11, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(6), reveal_by: ElapsedTick(11),
    }, &snapshot).unwrap();
    let vote = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", vote, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("helper", Verdict::Allow, b"salt", ElapsedTick(1)).unwrap();
    let review = session.finish(ElapsedTick(1)).unwrap();
    broker.apply_review(review, Some(&inputs), &snapshot).unwrap();
    let automatic = broker.authorize(1, Some(&inputs), &snapshot).unwrap();
    let message = match (reviewer, expiry) {
        (Some(reviewer), Some(expiry)) => {
            let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(expiry)).unwrap();
            let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
            broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot).unwrap()
        }
        (None, None) => broker.dispatch(&automatic, &action, Some(&inputs), &snapshot).unwrap(),
        _ => unreachable!("reviewer provisioned exactly when expiry is present"),
    };
    (broker, endpoint, message)
}
