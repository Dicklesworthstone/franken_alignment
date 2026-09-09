//! Public complete-input -> independent review -> publication integration.
//! These tests use declared capture and an in-memory endpoint, not real inference.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedReview, ObservedSession,
    OversightBroker, ReviewWindow, action_frame,
};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{
    EvidenceViewManifest, EvidenceWindow, OriginalBinding, ProjectionBinding, RedactionBinding,
    TransformBinding, ViewBinding, ViewSpec,
};
use fa_reference::full_input::{ActualHelperInput, InputPart, InputProfileBinding, PartKind};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn spec(epoch: u64) -> ActionSpec {
    ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 6, object: 7, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 16,
    }
}

fn contracts() -> CommitteeContract {
    CommitteeContract::new(["alice", "bob"].into_iter().map(|name| (
        name.to_owned(), HelperContract::new(InputProfileBinding {
            profile_id: name.to_owned(), generation: 1, model_space: format!("model-{name}"),
            model_epoch: 1, tokenizer: "tok".to_owned(), tokenizer_epoch: 1,
            policy_epoch: 0, input_contract: "framed-action-v1".to_owned(),
        }, 9, b"Review this action".to_vec()).unwrap(),
    )).collect()).unwrap()
}

fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}

fn fixture() -> (OversightBroker, PublicationEndpoint, CommitteeContract) {
    let contracts = contracts();
    let config = ControllerConfig {
        scope: spec(0).scope, total: 100, max_attempts: 16,
        actor: ActorState::new(RestartProfile {
            id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
        }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
        congress: CongressPolicy {
            generation: 1, members: ["alice", "bob"].into_iter().map(|name| (
                name.to_owned(), MemberPolicy { cohort: name.to_owned(), weight: 1 },
            )).collect(), caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 2, continue_hold_maximum: 0, narrow_at: 3, suspend_at: 4,
            minimum_members: 2, minimum_cohorts: 2,
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

fn inputs(action: &FrozenAction, contracts: &CommitteeContract, context: &[u8], transform: u64) -> CommitteeInput {
    let views = contracts.members().iter().map(|(name, contract)| {
        let mut bytes = action_frame(action);
        let action_end = bytes.len();
        bytes.extend_from_slice(contract.question());
        let question_end = bytes.len();
        bytes.extend_from_slice(context);
        let input = ActualHelperInput::new(contract.profile_at(action.spec().policy_epoch), bytes.clone(), vec![
            InputPart { kind: PartKind::Other, ordinal: 0, start: 0, end: action_end },
            InputPart { kind: PartKind::Question, ordinal: 0, start: action_end, end: question_end },
            InputPart { kind: PartKind::Retrieved, ordinal: 0, start: question_end, end: bytes.len() },
        ], Vec::new()).unwrap();
        let original = OriginalBinding { tenant: 1, object_id: format!("context-{name}"), generation: 1 };
        let manifest = EvidenceViewManifest::capture(&input, ViewSpec {
            projection: ProjectionBinding { tenant: 1, projection_id: 9, allowed_originals: vec![original.clone()] },
            views: vec![ViewBinding {
                part_index: 2, original, transform: TransformBinding { contract_id: "identity".to_owned(), generation: transform },
                redaction: RedactionBinding::None,
                window: EvidenceWindow { source_start: 0, source_end: context.len() as u64, full_source_end: context.len() as u64 },
            }], exact_submitted: bytes,
        }).unwrap();
        (name.clone(), manifest)
    }).collect();
    CommitteeInput::capture(action, contracts, views).unwrap()
}

fn window() -> ReviewWindow { ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(20) } }

fn prepare(broker: &mut OversightBroker, contracts: &CommitteeContract, id: u64) -> (FrozenAction, CommitteeInput) {
    let proposal = broker.propose(id, spec(broker.inspect().ledger.epoch), &snapshot()).unwrap();
    let input = inputs(&proposal.action, contracts, b"context-a", 1);
    assert_eq!(broker.record_inputs(id, 0, input.clone()), Ok(1));
    (proposal.action, input)
}

fn complete(mut session: ObservedSession, verdict: Verdict) -> ObservedReview {
    for member in ["alice", "bob"] {
        let commitment = session.commitment(member, verdict, member.as_bytes()).unwrap();
        session.commit(member, commitment, ElapsedTick(1)).unwrap();
    }
    session.open_reveals(ElapsedTick(1)).unwrap();
    for member in ["alice", "bob"] {
        session.reveal(member, verdict, member.as_bytes(), ElapsedTick(1)).unwrap();
    }
    session.finish(ElapsedTick(1)).unwrap()
}

fn approve(broker: &mut OversightBroker, id: u64, round: u64, input: &CommitteeInput) {
    let session = broker.begin_review(id, round, [1; 32], window(), &snapshot()).unwrap();
    let receipt = broker.apply_review(complete(session, Verdict::Allow), Some(input), &snapshot()).unwrap();
    assert_eq!(receipt.inputs.as_ref(), input);
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
}

#[test]
fn actual_helper_views_reach_publication_without_exporting_their_context() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (action, input) = prepare(&mut broker, &contracts, 1);
    let bytes = broker.captured_input_bytes();
    assert_eq!(broker.record_inputs(1, 1, input.clone()), Ok(1));
    assert_eq!(broker.captured_input_bytes(), bytes);
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
    assert_eq!(message.action().payload(), b"publish");
    let receipt = endpoint.deliver(&message).unwrap();
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.payload(), b"publish");
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn changed_whole_input_invalidates_approval_even_when_exact_policy_reads_match() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (action, original) = prepare(&mut broker, &contracts, 1);
    approve(&mut broker, 1, 11, &original);
    let permit = broker.authorize(1, Some(&original), &snapshot()).unwrap();
    let changed = inputs(&action, &contracts, b"context-b", 1);
    assert_eq!(broker.dispatch(&permit, &action, Some(&changed), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(broker.record_inputs(1, 1, changed.clone()), Ok(2));
    assert_eq!(broker.dispatch(&permit, &action, Some(&original), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect().ledger.reserved, 16);
    approve(&mut broker, 1, 12, &changed);
    let message = broker.dispatch(&permit, &action, Some(&changed), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn transformation_metadata_alone_invalidates_a_pending_review() {
    let (mut broker, _, contracts) = fixture();
    let (action, original) = prepare(&mut broker, &contracts, 1);
    let session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap();
    let changed = inputs(&action, &contracts, b"context-a", 2);
    assert_eq!(original.views()["alice"].input().submitted(), changed.views()["alice"].input().submitted());
    broker.record_inputs(1, 1, changed.clone()).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.apply_review(complete(session, Verdict::Allow), Some(&changed), &snapshot()), Err(Error::Stale));
    assert_eq!(broker.inspect(), before);
    approve(&mut broker, 1, 12, &changed);
    broker.authorize(1, Some(&changed), &snapshot()).unwrap();
}

#[test]
fn outage_then_identical_recapture_requires_new_independent_review() {
    let (mut broker, _, contracts) = fixture();
    let (action, input) = prepare(&mut broker, &contracts, 1);
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    assert_eq!(broker.inputs_unavailable(1, 1), Ok(2));
    assert_eq!(broker.dispatch(&permit, &action, None, &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.record_inputs(1, 2, input.clone()), Ok(3));
    assert_eq!(broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap_err(), Error::Incomplete);
    approve(&mut broker, 1, 12, &input);
    broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
}

#[test]
fn input_outage_blocks_new_effects_but_not_receipt_reconciliation() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let (action, input) = prepare(&mut broker, &contracts, 1);
    approve(&mut broker, 1, 11, &input);
    let permit = broker.authorize(1, Some(&input), &snapshot()).unwrap();
    let message = broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    broker.inputs_unavailable(1, 1).unwrap();
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert_eq!(broker.pending_reconciliation().unwrap().len(), 1);
    assert_eq!(broker.cancel(1), Err(Error::WrongState));
    assert_eq!(broker.accept_receipt(receipt), Ok(true));
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(broker.inspect().ledger.available, 84);
}

#[test]
fn frozen_restriction_still_applies_when_the_current_views_are_unavailable() {
    let (mut broker, _, contracts) = fixture();
    let (_, input) = prepare(&mut broker, &contracts, 1);
    let session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap();
    let review = complete(session, Verdict::Hold);
    broker.inputs_unavailable(1, 1).unwrap();
    let mut unavailable = snapshot();
    unavailable.complete = false;
    let receipt = broker.apply_review(review, None, &unavailable).unwrap();
    assert_eq!(receipt.inputs.as_ref(), &input);
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert_eq!(broker.inspect().ledger.available, 100);
}

#[test]
fn identical_contexts_from_another_broker_cannot_supply_a_review() {
    let (mut first, _, contracts) = fixture();
    let (mut second, _, _) = fixture();
    let (_, input) = prepare(&mut first, &contracts, 1);
    prepare(&mut second, &contracts, 1);
    let session = first.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap();
    assert_eq!(second.apply_review(complete(session, Verdict::Allow), Some(&input), &snapshot()), Err(Error::Binding));
    assert_eq!(second.inspect().sequence, 0);
}

#[test]
fn deadlines_record_missing_helpers_instead_of_waiting_or_accepting_late_votes() {
    let (mut broker, _, contracts) = fixture();
    let (_, input) = prepare(&mut broker, &contracts, 1);
    let mut session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap();
    let alice = session.commitment("alice", Verdict::Allow, b"a").unwrap();
    let bob = session.commitment("bob", Verdict::Allow, b"b").unwrap();
    session.commit("alice", alice, ElapsedTick(2)).unwrap();
    assert_eq!(session.open_reveals(ElapsedTick(3)), Err(Error::Incomplete));
    assert_eq!(session.commit("bob", bob, ElapsedTick(10)), Err(Error::Stale));
    session.open_reveals(ElapsedTick(10)).unwrap();
    session.reveal("alice", Verdict::Allow, b"a", ElapsedTick(11)).unwrap();
    assert_eq!(session.finish(ElapsedTick(19)).unwrap_err(), Error::Incomplete);
    let review = session.finish(ElapsedTick(20)).unwrap();
    assert_eq!(review.missing(), &["bob".to_owned()]);
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
    broker.observe_time(ElapsedTick(20)).unwrap();
    broker.apply_review(review, None, &snapshot()).unwrap();
    assert!(broker.authorize(1, Some(&input), &snapshot()).is_err());
    assert_eq!(broker.inspect().ledger.available, 100);
}

#[test]
fn early_finish_preserves_session_and_success_needs_both_reveals() {
    let (mut broker, _, contracts) = fixture();
    let (_, input) = prepare(&mut broker, &contracts, 1);
    let mut session = broker.begin_review(1, 11, [1; 32], window(), &snapshot()).unwrap();
    for member in ["alice", "bob"] {
        let commitment = session.commitment(member, Verdict::Allow, member.as_bytes()).unwrap();
        session.commit(member, commitment, ElapsedTick(2)).unwrap();
    }
    session.open_reveals(ElapsedTick(2)).unwrap();
    session.reveal("alice", Verdict::Allow, b"alice", ElapsedTick(3)).unwrap();
    assert_eq!(session.finish(ElapsedTick(3)).unwrap_err(), Error::Incomplete);
    assert_eq!(session.reveal("bob", Verdict::Allow, b"bob", ElapsedTick(2)), Err(Error::Stale));
    session.reveal("bob", Verdict::Allow, b"bob", ElapsedTick(4)).unwrap();
    let review = session.finish(ElapsedTick(4)).unwrap();
    assert_eq!(review.decision().consequence, Consequence::Continue);
    assert_eq!(session.finish(ElapsedTick(4)).unwrap_err(), Error::WrongState);
    broker.observe_time(ElapsedTick(4)).unwrap();
    broker.apply_review(review, Some(&input), &snapshot()).unwrap();
    broker.authorize(1, Some(&input), &snapshot()).unwrap();
}
