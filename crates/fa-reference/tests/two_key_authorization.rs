//! Public two-key controls through the existing congress and endpoint model.
//! Neither role is an authenticated real user and publication is in-memory.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewer, HumanReviewPolicy};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn spec(epoch: u64) -> ActionSpec {
    ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 16,
    }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}
fn bare() -> (OversightBroker, PublicationEndpoint, CommitteeContract) {
    let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"profile-v1".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        9, b"Review".to_vec(),
    ).unwrap())])).unwrap();
    let config = ControllerConfig {
        scope: spec(0).scope, total: 100, max_attempts: 16,
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
        narrowed_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap(),
    };
    let mut endpoint = PublicationEndpoint::new(spec(0).target.unwrap(), b"old".to_vec(), 200, 16).unwrap();
    let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    let ack = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(ack).unwrap();
    (broker, endpoint, contracts)
}
fn human_policy(limit: usize) -> HumanReviewPolicy {
    HumanReviewPolicy { reviewer_id: 90, max_validity_ticks: 20, max_requests: limit }
}
fn fixture() -> (OversightBroker, PublicationEndpoint, CommitteeContract, HumanReviewer) {
    let (mut broker, endpoint, contracts) = bare();
    let reviewer = broker.enable_human_review(human_policy(16)).unwrap();
    (broker, endpoint, contracts, reviewer)
}
fn input(action: &FrozenAction, contracts: &CommitteeContract) -> CommitteeInput {
    let helper = &contracts.members()["helper"];
    let mut bytes = action_frame(action);
    let end = bytes.len();
    bytes.extend_from_slice(helper.question());
    let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    CommitteeInput::capture(action, contracts, BTreeMap::from([("helper".to_owned(), manifest)])).unwrap()
}
fn prepare(broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64) -> (FrozenAction, CommitteeInput) {
    let proposal = broker.propose(id, spec(broker.inspect().ledger.epoch), &snapshot()).unwrap();
    let inputs = input(&proposal.action, contracts);
    broker.record_inputs(id, 0, inputs.clone()).unwrap();
    (proposal.action, inputs)
}
fn review(broker: &mut OversightBroker, id: u64, round: u64, inputs: &CommitteeInput) {
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, round, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let vote = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", vote, now).unwrap();
    session.open_reveals(now).unwrap();
    session.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
    let completed = session.finish(now).unwrap();
    broker.apply_review(completed, Some(inputs), &snapshot()).unwrap();
}

#[test]
fn both_keys_publish_once_and_the_legacy_entrypoint_cannot_bypass_human_review() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture();
    let (action, inputs) = prepare(&mut broker, &contracts, 1);
    review(&mut broker, 1, 11, &inputs);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.dispatch(&automatic, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect(), before);
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    assert_eq!(request.action(), &action);
    assert_eq!(request.inputs(), &inputs);
    assert_eq!(request.control_sequence(), before.sequence);
    assert_eq!(request.reviewer_id(), 90);
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let message = broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::WrongState);
    let receipt = endpoint.deliver(&message).unwrap();
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(endpoint.payload(), b"publish");
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn request_requires_congress_approval_and_reject_cannot_be_reissued_in_the_same_context() {
    let (mut broker, _, contracts, reviewer) = fixture();
    let (_, inputs) = prepare(&mut broker, &contracts, 1);
    assert_eq!(broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap_err(), Error::Incomplete);
    review(&mut broker, 1, 11, &inputs);
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    reviewer.reject(&request, ElapsedTick(1)).unwrap();
    reviewer.reject(&request, ElapsedTick(1)).unwrap();
    assert_eq!(reviewer.approve(&request, ElapsedTick(1)).unwrap_err(), Error::WrongState);
    assert_eq!(broker.request_human_approval(102, 1, Some(&inputs), ElapsedTick(10)).unwrap_err(), Error::Duplicate);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Rejected);
    assert_eq!(broker.inspect().ledger.available, 100);
}

#[test]
fn revocation_before_dispatch_preserves_reservation_but_prevents_publication() {
    let (mut broker, endpoint, contracts, reviewer) = fixture();
    let (action, inputs) = prepare(&mut broker, &contracts, 1);
    review(&mut broker, 1, 11, &inputs);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    reviewer.revoke(&request, ElapsedTick(1)).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::WrongState);
    assert_eq!(broker.inspect(), before);
    assert_eq!(before.ledger.reserved, 16);
    assert_eq!(endpoint.execution_count(), 0);
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, 100);
}

#[test]
fn stale_evidence_refusal_does_not_consume_either_key() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture();
    let (action, inputs) = prepare(&mut broker, &contracts, 1);
    review(&mut broker, 1, 11, &inputs);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    let mut changed = snapshot();
    changed.values.insert(7, vec![8]);
    let before = broker.inspect();
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &changed).unwrap_err(), Error::Binding);
    assert_eq!(broker.inspect(), before);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Approved);
    let message = broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn human_key_cannot_replace_or_launder_a_foreign_automatic_permit() {
    let (mut broker, mut endpoint, contracts, reviewer) = fixture();
    let (mut foreign, _, other_contracts, _) = fixture();
    let (action, inputs) = prepare(&mut broker, &contracts, 1);
    review(&mut broker, 1, 11, &inputs);
    let (_, other_inputs) = prepare(&mut foreign, &other_contracts, 1);
    review(&mut foreign, 1, 11, &other_inputs);
    let wrong = foreign.authorize(1, Some(&other_inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    assert_eq!(broker.dispatch_with_human(&wrong, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Binding);
    assert_eq!(broker.inspect().ledger.available, 100);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Approved);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn identically_numbered_foreign_reviewer_and_key_are_not_authority() {
    let (mut first, _, contracts, reviewer) = fixture();
    let (mut second, _, other_contracts, other_reviewer) = fixture();
    let (action, inputs) = prepare(&mut first, &contracts, 1);
    let (_, other_inputs) = prepare(&mut second, &other_contracts, 1);
    review(&mut first, 1, 11, &inputs);
    review(&mut second, 1, 11, &other_inputs);
    let request = first.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let other = second.request_human_approval(101, 1, Some(&other_inputs), ElapsedTick(10)).unwrap();
    assert_eq!(other_reviewer.approve(&request, ElapsedTick(1)).unwrap_err(), Error::Binding);
    let wrong = other_reviewer.approve(&other, ElapsedTick(1)).unwrap();
    let automatic = first.authorize(1, Some(&inputs), &snapshot()).unwrap();
    assert_eq!(first.dispatch_with_human(&automatic, &wrong, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Binding);
    let correct = reviewer.approve(&request, ElapsedTick(1)).unwrap();
    first.dispatch_with_human(&automatic, &correct, &action, Some(&inputs), &snapshot()).unwrap();
}

#[test]
fn human_expiry_and_independent_clock_observation_are_enforced() {
    let (mut broker, _, contracts, reviewer) = fixture();
    let (action, inputs) = prepare(&mut broker, &contracts, 1);
    review(&mut broker, 1, 11, &inputs);
    let automatic = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(3)).unwrap();
    let human = reviewer.approve(&request, ElapsedTick(2)).unwrap();
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    broker.observe_time(ElapsedTick(3)).unwrap();
    assert_eq!(broker.dispatch_with_human(&automatic, &human, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect().ledger.reserved, 16);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Approved);
}

#[test]
fn bootstrap_mode_cannot_be_enabled_late_or_disabled_by_reconfiguration() {
    let (mut broker, _, contracts) = bare();
    prepare(&mut broker, &contracts, 1);
    assert_eq!(broker.enable_human_review(human_policy(16)).unwrap_err(), Error::WrongState);
    let (mut enabled, _, _, _) = fixture();
    assert_eq!(enabled.enable_human_review(human_policy(16)).unwrap_err(), Error::Duplicate);
    assert!(enabled.human_review_required());
    let (mut plain, _, contracts) = bare();
    let (action, inputs) = prepare(&mut plain, &contracts, 1);
    review(&mut plain, 1, 11, &inputs);
    let permit = plain.authorize(1, Some(&inputs), &snapshot()).unwrap();
    assert!(!plain.human_review_required());
    plain.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
}
