//! Source-checked coarse sidecar bytes through the ORIGINAL congress and effect gate.
#[path = "support/file_oversight.rs"] mod host;
#[path = "support/learned_kv.rs"] mod kv;
use fa_reference::action::{ActionSpec, ElapsedTick, VERSION};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::{EndpointOutcome, PublicationEndpoint};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, ObservedReview, ObservedSession, OversightBroker, ReviewWindow};
use fa_reference::action::consequence::oversight::sidecar::{SidecarCongressBudget, SidecarCongressPlan,
    SidecarIdentity, SidecarRefinementOutcome};
use fa_reference::action::consequence::activation::probe::learned::{CheckedKvBudget, CheckedLearnedKv, KvGroup, KvRow, ResidualRetention};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::learned::{CompressionBudget, FitBudget,
    LearnedKvCodec, LearnedKvPolicy};
use fa_reference::full_input::PartKind;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;

const ROOT: [u8; 32] = [9; 32];
fn setup() -> (OversightBroker, PublicationEndpoint, CommitteeContract) {
    let p = host::profile(); let d = p.delivery; let committee = p.committee;
    let ceiling = TargetCeiling::new(&d.narrowed_targets).unwrap();
    let mut endpoint = PublicationEndpoint::new(d.target, d.initial_payload.clone(), d.retention_ticks, d.max_deliveries).unwrap();
    let config = ControllerConfig { scope: d.scope, total: d.total, max_attempts: d.max_attempts,
        actor: d.actor, suspend_at_incident: d.suspend_at_incident, policy: d.policy,
        congress: d.congress, narrowed_targets: ceiling };
    let mut broker = OversightBroker::new(config, &mut endpoint, committee.clone()).unwrap();
    let acknowledgment = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(acknowledgment).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    (broker, endpoint, committee)
}
fn action(broker: &OversightBroker, payload: &[u8]) -> ActionSpec {
    let state = broker.inspect();
    ActionSpec { version: VERSION, scope: host::profile().delivery.scope, target: Some(host::profile().delivery.target),
        payload: payload.to_vec(), required_witnesses: Vec::new(), policy_epoch: state.ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn checked(retention: ResidualRetention) -> CheckedLearnedKv {
    let training = kv::line(11, 3, 5);
    let source = kv::image(21, 1, 3, &[vec![1.0, 2.0, 0.25]]);
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let (image, _) = codec.evaluate_held_out(201, &source, CompressionBudget::default()).unwrap();
    CheckedLearnedKv::new(image, &source, retention, CheckedKvBudget::default()).unwrap()
}
fn group() -> KvGroup { KvGroup { row: KvRow { layer: 1, side: KvSide::Value, position: 0 }, head: 0 } }
fn plan(source: CheckedLearnedKv, priority: Vec<KvGroup>, budget: SidecarCongressBudget) -> SidecarCongressPlan {
    SidecarCongressPlan::new(source, SidecarIdentity { object_id: 77, generation: 1, transform_id: 9001 }, priority, budget).unwrap()
}
fn finish(session: &mut ObservedSession, verdict: Verdict, now: ElapsedTick) -> ObservedReview {
    for member in host::MEMBERS {
        let salt = host::salt(member);
        let commitment = session.commitment(member, verdict, &salt).unwrap();
        session.commit(member, commitment, now).unwrap();
    }
    session.open_reveals(now).unwrap();
    for member in host::MEMBERS { session.reveal(member, verdict, &host::salt(member), now).unwrap(); }
    session.finish(now).unwrap()
}
fn review(broker: &mut OversightBroker, id: u64, round: u64, verdict: Verdict) -> ObservedReview {
    let mut session = broker.begin_review(id, round, ROOT, ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
    }, &host::snapshot()).unwrap();
    finish(&mut session, verdict, ElapsedTick(1))
}
fn missing_review(broker: &mut OversightBroker, id: u64, round: u64) -> ObservedReview {
    let mut session = broker.begin_review(id, round, ROOT, ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
    }, &host::snapshot()).unwrap();
    let salt = host::salt("alpha");
    let commitment = session.commitment("alpha", Verdict::Allow, &salt).unwrap();
    session.commit("alpha", commitment, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(5)).unwrap();
    session.reveal("alpha", Verdict::Allow, &salt, ElapsedTick(5)).unwrap();
    session.finish(ElapsedTick(10)).unwrap()
}
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn abstention_purchases_exact_residual_then_a_new_round_can_authorize_and_execute() {
    let (mut broker, mut endpoint, committee) = setup();
    let proposal = broker.propose(1, action(&broker, b"sidecar publication"), &host::snapshot()).unwrap();
    let source = checked(ResidualRetention::All); let residual = source.residual_bytes(group()).unwrap().to_vec();
    let mut planner = plan(source, vec![group()], SidecarCongressBudget::default());
    let coarse = planner.initial(&proposal.action, &committee).unwrap();
    assert!(!contains(coarse.payload(), b"FAKVRX\0\x01"));
    assert_eq!(broker.record_inputs(1, 0, coarse.input().clone()).unwrap(), 1);
    let coarse_review = review(&mut broker, 1, 101, Verdict::Abstain);
    let refined = match planner.refine_after(&coarse_review, &proposal.action, &committee).unwrap() {
        SidecarRefinementOutcome::Refined { group: selected, round } => { assert_eq!(selected, group()); round }
        other => panic!("expected refinement, got {other:?}"),
    };
    assert!(contains(refined.payload(), &residual));
    assert_eq!(refined.selected_groups(), &[group()]);
    assert_eq!(planner.work().rounds, 2); assert_eq!(planner.work().residual_bytes, residual.len());
    assert_eq!(broker.record_inputs(1, 1, refined.input().clone()).unwrap(), 2);
    // Do not apply the old restrictive review: the refinement is a replacement
    // evidence round, and only the fresh substantive review is used for Continue.
    drop(coarse_review);
    let final_review = review(&mut broker, 1, 102, Verdict::Allow);
    assert_eq!(planner.refine_after(&final_review, &proposal.action, &committee).unwrap(), SidecarRefinementOutcome::Final);
    let receipt = broker.apply_review(final_review, Some(refined.input()), &host::snapshot()).unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
    let permit = broker.authorize(1, Some(refined.input()), &host::snapshot()).unwrap();
    let envelope = broker.dispatch(&permit, &proposal.action, Some(refined.input()), &host::snapshot()).unwrap();
    let endpoint_receipt = endpoint.deliver(&envelope).unwrap();
    assert_eq!(endpoint_receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    broker.accept_receipt(endpoint_receipt).unwrap();
    assert_eq!(endpoint.execution_count(), 1); assert_eq!(endpoint.payload(), b"sidecar publication");
}

#[test]
fn old_coarse_restrictive_review_can_only_hold_after_refinement_never_authorize() {
    let (mut broker, _endpoint, committee) = setup();
    let proposal = broker.propose(1, action(&broker, b"old hold"), &host::snapshot()).unwrap();
    let mut planner = plan(checked(ResidualRetention::All), vec![group()], SidecarCongressBudget::default());
    let coarse = planner.initial(&proposal.action, &committee).unwrap(); broker.record_inputs(1, 0, coarse.input().clone()).unwrap();
    let old = review(&mut broker, 1, 101, Verdict::Abstain);
    let refined = match planner.refine_after(&old, &proposal.action, &committee).unwrap() {
        SidecarRefinementOutcome::Refined { round, .. } => round, other => panic!("unexpected {other:?}"),
    };
    broker.record_inputs(1, 1, refined.input().clone()).unwrap();
    let receipt = broker.apply_review(old, Some(coarse.input()), &host::snapshot()).unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert!(broker.authorize(1, Some(refined.input()), &host::snapshot()).is_err());
    assert_eq!(broker.inspect().ledger.reserved, 0);
}

#[test]
fn coarse_helper_views_contain_the_same_checked_payload_but_keep_original_questions_and_profiles() {
    let (mut broker, _endpoint, committee) = setup();
    let proposal = broker.propose(1, action(&broker, b"inspect"), &host::snapshot()).unwrap();
    let source = checked(ResidualRetention::All); let mut planner = plan(source, vec![group()], SidecarCongressBudget::default());
    let coarse = planner.initial(&proposal.action, &committee).unwrap();
    assert!(coarse.payload().starts_with(b"FASIDE\0\x01"));
    for (member, view) in coarse.input().views() {
        let helper = &committee.members()[member]; let input = view.actual_input();
        let evidence = input.ordered_parts().iter().position(|part| matches!(&part.kind, PartKind::Evidence { .. })).unwrap();
        let question = input.ordered_parts().iter().position(|part| matches!(&part.kind, PartKind::Question)).unwrap();
        assert_eq!(input.part_bytes(evidence).unwrap(), coarse.payload());
        assert_eq!(input.part_bytes(question).unwrap(), helper.question());
        assert_eq!(input.input_profile(), &helper.profile_at(proposal.action.spec().policy_epoch));
        assert_eq!(view.evidence_parts()[0].original.object_id, 77);
        assert_eq!(view.evidence_parts()[0].window.window_len as usize, coarse.payload().len());
    }
}

#[test]
fn missing_worker_is_not_interpreted_as_an_unanswerable_refinement_request() {
    let (mut broker, _endpoint, committee) = setup();
    let proposal = broker.propose(1, action(&broker, b"missing"), &host::snapshot()).unwrap();
    let mut planner = plan(checked(ResidualRetention::All), vec![group()], SidecarCongressBudget::default());
    let coarse = planner.initial(&proposal.action, &committee).unwrap();
    broker.record_inputs(1, 0, coarse.input().clone()).unwrap();
    let review = missing_review(&mut broker, 1, 101);
    assert_eq!(planner.refine_after(&review, &proposal.action, &committee).unwrap(),
        SidecarRefinementOutcome::Missing { members: vec!["beta".to_owned()] });
    assert_eq!(planner.work(), coarse.work()); assert!(planner.selected_groups().next().is_none());
    assert_eq!(broker.input_revision(1).unwrap(), 1);
}

#[test]
fn cumulative_round_committee_and_residual_limits_hold_instead_of_skipping_scrutiny() {
    let source = checked(ResidualRetention::All); let residual_bytes = source.residual_bytes(group()).unwrap().len();
    let budgets = [
        SidecarCongressBudget { rounds: 1, residual_bytes: 8 * 1_048_576, committee_bytes: 8 * 1_048_576 },
        SidecarCongressBudget { rounds: 32, residual_bytes: residual_bytes - 1, committee_bytes: 8 * 1_048_576 },
    ];
    for (index, budget) in budgets.into_iter().enumerate() {
        let (mut broker, _endpoint, committee) = setup();
        let proposal = broker.propose(1, action(&broker, b"budget"), &host::snapshot()).unwrap();
        let mut planner = plan(source.clone(), vec![group()], budget);
        let coarse = planner.initial(&proposal.action, &committee).unwrap();
        broker.record_inputs(1, 0, coarse.input().clone()).unwrap();
        let review = review(&mut broker, 1, 201 + index as u64, Verdict::Abstain);
        assert!(matches!(planner.refine_after(&review, &proposal.action, &committee).unwrap(),
            SidecarRefinementOutcome::BudgetExhausted { .. }));
        assert_eq!(planner.work(), coarse.work()); assert!(planner.selected_groups().next().is_none());
    }
    // Committee replication is an independent cumulative resource boundary.
    let (mut broker, _endpoint, committee) = setup();
    let proposal = broker.propose(1, action(&broker, b"committee budget"), &host::snapshot()).unwrap();
    let mut oracle = plan(source.clone(), vec![group()], SidecarCongressBudget::default());
    let baseline = oracle.initial(&proposal.action, &committee).unwrap().work().committee_bytes;
    let mut planner = plan(source, vec![group()], SidecarCongressBudget {
        rounds: 32, residual_bytes: 8 * 1_048_576, committee_bytes: baseline });
    let coarse = planner.initial(&proposal.action, &committee).unwrap(); broker.record_inputs(1, 0, coarse.input().clone()).unwrap();
    let review = review(&mut broker, 1, 301, Verdict::Abstain);
    assert!(matches!(planner.refine_after(&review, &proposal.action, &committee).unwrap(),
        SidecarRefinementOutcome::BudgetExhausted { .. }));
    assert_eq!(planner.work(), coarse.work());
}

#[test]
fn absent_retained_residual_and_empty_priority_are_explicitly_unresolved() {
    assert_eq!(SidecarCongressPlan::new(checked(ResidualRetention::None), SidecarIdentity {
        object_id: 77, generation: 1, transform_id: 9001 }, vec![group()], SidecarCongressBudget::default()).unwrap_err(), Error::Missing);
    let (mut broker, _endpoint, committee) = setup();
    let proposal = broker.propose(1, action(&broker, b"unresolved"), &host::snapshot()).unwrap();
    let mut planner = plan(checked(ResidualRetention::All), Vec::new(), SidecarCongressBudget::default());
    let coarse = planner.initial(&proposal.action, &committee).unwrap(); broker.record_inputs(1, 0, coarse.input().clone()).unwrap();
    let review = review(&mut broker, 1, 101, Verdict::Abstain);
    assert!(matches!(planner.refine_after(&review, &proposal.action, &committee).unwrap(),
        SidecarRefinementOutcome::Unresolved { .. }));
}

#[test]
fn review_over_a_different_sidecar_identity_cannot_purchase_refinement() {
    let (mut broker, _endpoint, committee) = setup();
    let proposal = broker.propose(1, action(&broker, b"binding"), &host::snapshot()).unwrap();
    let source = checked(ResidualRetention::All);
    let mut actual = plan(source.clone(), vec![group()], SidecarCongressBudget::default());
    let coarse = actual.initial(&proposal.action, &committee).unwrap(); broker.record_inputs(1, 0, coarse.input().clone()).unwrap();
    let review = review(&mut broker, 1, 101, Verdict::Abstain);
    let mut foreign = SidecarCongressPlan::new(source, SidecarIdentity { object_id: 78, generation: 1, transform_id: 9001 },
        vec![group()], SidecarCongressBudget::default()).unwrap();
    foreign.initial(&proposal.action, &committee).unwrap();
    assert_eq!(foreign.refine_after(&review, &proposal.action, &committee).unwrap_err(), Error::Binding);
    assert_eq!(foreign.work().rounds, 1); assert!(foreign.selected_groups().next().is_none());
}
