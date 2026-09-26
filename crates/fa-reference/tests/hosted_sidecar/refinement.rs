use super::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::{human::HumanReviewPolicy,
    sidecar::SidecarRefinementOutcome};

fn bound_review(owner: &mut OversightBroker, sidecar: &HostedSidecar, round: u64,
    verdict: Verdict) -> ObservedReview
{
    let now = owner.inspect().ledger.elapsed.unwrap();
    let mut session = owner.begin_hosted_sidecar_review(sidecar, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 4), reveal_by: ElapsedTick(now.0 + 9),
    }, &snapshot()).unwrap();
    let salt = b"original-reviewer";
    let commitment = session.commitment("reviewer", verdict, salt).unwrap();
    session.commit("reviewer", commitment, now).unwrap();
    session.open_reveals(now).unwrap();
    session.reveal("reviewer", verdict, salt, now).unwrap();
    session.finish(now).unwrap()
}

#[test]
fn abstention_buys_original_residual_then_fresh_round_and_two_keys_publish() {
    let (mut owner, mut endpoint, _, codec) = setup(100.0, &[0, 1]);
    let human = owner.enable_human_review(HumanReviewPolicy {
        reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 }).unwrap();
    let action = propose(&mut owner, 1);
    let mut sidecar = begin(&mut owner, 1, &codec);
    let coarse = sidecar.round().clone();
    let before = owner.hosted_decoder().unwrap();
    let source = sidecar.source().encode().unwrap();
    let residual = sidecar.source().residual_bytes(group()).unwrap().to_vec();
    let completed = bound_review(&mut owner, &sidecar, 101, Verdict::Abstain);
    let refined = owner.refine_hosted_sidecar(&mut sidecar, &completed).unwrap();
    let round = match refined {
        SidecarRefinementOutcome::Refined { group: selected, round } => {
            assert_eq!(selected, group()); round
        }
        other => panic!("expected exact refinement, got {other:?}"),
    };
    assert_eq!(sidecar.input_revision(), 2);
    assert_eq!(sidecar.round(), &round);
    assert_eq!(round.selected_groups(), &[group()]);
    assert_eq!(round.work().rounds, 2);
    assert_eq!(round.work().residual_bytes, residual.len());
    assert_eq!(round.work().committee_bytes, coarse.work().committee_bytes + round.input().logical_bytes());
    assert!(round.payload().windows(residual.len()).any(|bytes| bytes == residual));
    assert!(coarse.selected_groups().is_empty());
    assert_eq!(coarse.work().rounds, 1);
    assert_eq!(sidecar.source().encode().unwrap(), source);
    assert_eq!(owner.hosted_decoder().unwrap(), before);
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &completed), Err(Error::Stale));
    assert!(owner.authorize(1, Some(round.input()), &snapshot()).is_err());
    let completed = bound_review(&mut owner, &sidecar, 102, Verdict::Allow);
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &completed).unwrap(), SidecarRefinementOutcome::Final);
    let input = owner.current_hosted_sidecar(&sidecar).unwrap().clone();
    owner.apply_review(completed, Some(&input), &snapshot()).unwrap();
    let automatic = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert!(owner.dispatch(&automatic, &action, Some(&input), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
    let request = owner.request_human_approval(501, 1, Some(&input), ElapsedTick(30)).unwrap();
    assert_eq!(request.inputs(), &input);
    assert_eq!(request.input_revision(), 2);
    let approval = human.approve(&request, ElapsedTick(1)).unwrap();
    let dispatch = owner.dispatch_with_human(&automatic, &approval, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&dispatch).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(endpoint.payload(), b"visible");
}

#[test]
fn missing_verdicts_are_not_refinement_requests_or_silent_consent() {
    let (mut owner, endpoint, _, codec) = setup(100.0, &[0]);
    propose(&mut owner, 1);
    let mut sidecar = begin(&mut owner, 1, &codec);
    let before = sidecar.round().clone();
    let mut session = owner.begin_hosted_sidecar_review(&sidecar, 101, [9; 32], window(), &snapshot()).unwrap();
    let missing = session.finish(ElapsedTick(10)).unwrap();
    owner.observe_time(ElapsedTick(10)).unwrap();
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &missing).unwrap(),
        SidecarRefinementOutcome::Missing { members: vec!["reviewer".to_owned()] });
    assert_eq!(sidecar.round(), &before);
    assert_eq!(sidecar.input_revision(), 1);
    assert!(owner.authorize(1, Some(before.input()), &snapshot()).is_err());
    let receipt = owner.apply_review(missing, Some(before.input()), &snapshot()).unwrap();
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn all_three_cumulative_budgets_and_empty_priority_preserve_the_old_input() {
    let (mut owner, endpoint, _, codec) = setup(100.0, &[0]);
    propose(&mut owner, 1);
    let initial = begin(&mut owner, 1, &codec);
    let coarse_bytes = initial.round().input().logical_bytes();
    let residual_bytes = initial.source().residual_bytes(group()).unwrap().len();
    for mode in 0..4 {
        let mut request = request(&codec);
        match mode {
            0 => request.budget.rounds = 1,
            1 => request.budget.residual_bytes = residual_bytes - 1,
            2 => request.budget.committee_bytes = coarse_bytes,
            3 => request.priority.clear(),
            _ => unreachable!(),
        }
        let mut sidecar = owner.begin_hosted_sidecar(1, owner.actor_revision(), 1, request).unwrap();
        let before = sidecar.round().clone();
        let stored = owner.captured_input_bytes();
        let completed = bound_review(&mut owner, &sidecar, 101 + mode, Verdict::Abstain);
        let outcome = owner.refine_hosted_sidecar(&mut sidecar, &completed).unwrap();
        if mode == 3 { assert!(matches!(outcome, SidecarRefinementOutcome::Unresolved { .. })); }
        else { assert!(matches!(outcome, SidecarRefinementOutcome::BudgetExhausted { .. })); }
        assert_eq!(sidecar.round(), &before);
        assert_eq!(sidecar.input_revision(), 1);
        assert_eq!(owner.captured_input_bytes(), stored);
        assert_eq!(owner.current_hosted_sidecar(&sidecar).unwrap(), before.input());
    }
    let mut funded = begin(&mut owner, 1, &codec);
    let completed = bound_review(&mut owner, &funded, 201, Verdict::Abstain);
    assert!(matches!(owner.refine_hosted_sidecar(&mut funded, &completed).unwrap(), SidecarRefinementOutcome::Refined { .. }));
    assert_eq!(funded.input_revision(), 2);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn foreign_or_other_attempt_reviews_cannot_buy_evidence_despite_matching_packets() {
    let (mut owner, _, _, codec) = setup(100.0, &[0]);
    let (mut foreign, _, _, foreign_codec) = setup(100.0, &[0]);
    propose(&mut owner, 1); propose(&mut foreign, 1);
    let mut sidecar = begin(&mut owner, 1, &codec);
    let other = begin(&mut foreign, 1, &foreign_codec);
    assert_eq!(sidecar.round(), other.round());
    let review = bound_review(&mut foreign, &other, 101, Verdict::Abstain);
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &review), Err(Error::Binding));
    propose(&mut owner, 2);
    let other = begin(&mut owner, 2, &codec);
    let review = bound_review(&mut owner, &other, 102, Verdict::Abstain);
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &review), Err(Error::Binding));
    assert_eq!(sidecar.round().work().rounds, 1);
    assert_eq!(sidecar.input_revision(), 1);
    let own = bound_review(&mut owner, &sidecar, 103, Verdict::Abstain);
    assert!(matches!(owner.refine_hosted_sidecar(&mut sidecar, &own).unwrap(), SidecarRefinementOutcome::Refined { .. }));
}

#[test]
fn later_source_or_input_changes_refuse_before_purchasing_a_residual() {
    for source_changes in [false, true] {
        let (mut owner, endpoint, _, codec) = setup(100.0, &[0]);
        propose(&mut owner, 1);
        let mut sidecar = begin(&mut owner, 1, &codec);
        let completed = bound_review(&mut owner, &sidecar, 101, Verdict::Abstain);
        let before = sidecar.round().clone();
        if source_changes {
            owner.advance_hosted_forced(owner.actor_revision(), 1, 1, numerical_budget()).unwrap();
        } else {
            let mut replacement = request(&codec); replacement.identity.transform_id += 1;
            owner.begin_hosted_sidecar(1, owner.actor_revision(), 1, replacement).unwrap();
        }
        let stored = owner.captured_input_bytes();
        let numerical = owner.hosted_decoder().unwrap();
        assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &completed), Err(Error::Stale));
        assert_eq!(owner.begin_hosted_sidecar_review(&sidecar, 102, [9; 32], window(), &snapshot()).err(), Some(Error::Stale));
        assert_eq!(sidecar.round(), &before);
        assert_eq!(owner.captured_input_bytes(), stored);
        assert_eq!(owner.hosted_decoder().unwrap(), numerical);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn a_coarse_allow_cannot_approve_newly_refined_bytes() {
    let (mut owner, endpoint, _, codec) = setup(100.0, &[0]);
    propose(&mut owner, 1);
    let mut sidecar = begin(&mut owner, 1, &codec);
    let old = bound_review(&mut owner, &sidecar, 101, Verdict::Allow);
    let need_more = bound_review(&mut owner, &sidecar, 102, Verdict::Abstain);
    owner.refine_hosted_sidecar(&mut sidecar, &need_more).unwrap();
    let input = owner.current_hosted_sidecar(&sidecar).unwrap().clone();
    assert_eq!(owner.apply_review(old, Some(&input), &snapshot()).err(), Some(Error::Stale));
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
    let fresh = bound_review(&mut owner, &sidecar, 103, Verdict::Allow);
    owner.apply_review(fresh, Some(&input), &snapshot()).unwrap();
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_ok());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn future_review_observation_and_expired_action_do_not_spend_refinement_budget() {
    let (mut owner, _, _, codec) = setup(100.0, &[0]);
    propose(&mut owner, 1);
    let mut sidecar = begin(&mut owner, 1, &codec);
    let mut session = owner.begin_hosted_sidecar_review(&sidecar, 101, [9; 32], window(), &snapshot()).unwrap();
    let salt = b"original-reviewer";
    let digest = session.commitment("reviewer", Verdict::Abstain, salt).unwrap();
    session.commit("reviewer", digest, ElapsedTick(2)).unwrap();
    session.open_reveals(ElapsedTick(2)).unwrap();
    session.reveal("reviewer", Verdict::Abstain, salt, ElapsedTick(2)).unwrap();
    let completed = session.finish(ElapsedTick(2)).unwrap();
    let before = sidecar.round().clone();
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &completed), Err(Error::Stale));
    assert_eq!(sidecar.round(), &before);
    owner.observe_time(ElapsedTick(2)).unwrap();
    owner.refine_hosted_sidecar(&mut sidecar, &completed).unwrap();
    assert_eq!(sidecar.input_revision(), 2);
    let completed = bound_review(&mut owner, &sidecar, 102, Verdict::Abstain);
    let before = sidecar.round().clone();
    owner.observe_time(ElapsedTick(100)).unwrap();
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &completed), Err(Error::Stale));
    assert_eq!(sidecar.round(), &before);
}

#[test]
fn failed_original_input_capacity_admission_cannot_advance_the_refinement_handle() {
    use fa_reference::action::consequence::oversight::{MAX_CAPTURED_INPUT_BYTES,
        sidecar::SidecarCongressPlan};
    let (mut owner, endpoint, _, codec) = setup(100.0, &[0]);
    let action = propose(&mut owner, 1);
    let mut sidecar = begin(&mut owner, 1, &codec);
    let completed = bound_review(&mut owner, &sidecar, 101, Verdict::Abstain);
    let before = sidecar.round().clone();
    // Establish that the frozen sidecar budget itself CAN buy this residual.
    let parameters = request(&codec);
    let mut independent = SidecarCongressPlan::new(sidecar.source().clone(), parameters.identity,
        parameters.priority, parameters.budget).unwrap();
    assert_eq!(independent.initial(&action, owner.contracts()).unwrap(), before);
    let refined_size = match independent.refine_after(&completed, &action, owner.contracts()).unwrap() {
        SidecarRefinementOutcome::Refined { round, .. } => round.input().logical_bytes(),
        other => panic!("funded original planner must refine: {other:?}"),
    };
    // Spend the broker's real cumulative input allowance on ANOTHER attempt,
    // without mutating private counters or touching the source-bound input.
    propose(&mut owner, 2);
    let first = begin(&mut owner, 2, &codec).round().input().clone();
    let mut different = request(&codec); different.identity.transform_id += 1;
    let second = owner.begin_hosted_sidecar(2, owner.actor_revision(), 1, different).unwrap().round().input().clone();
    assert!(refined_size > first.logical_bytes().max(second.logical_bytes()));
    let mut choose_first = true;
    let mut exhausted = false;
    let bound = MAX_CAPTURED_INPUT_BYTES / first.logical_bytes().min(second.logical_bytes()) + 2;
    for _ in 0..bound {
        let next = if choose_first { &first } else { &second };
        let spent = owner.captured_input_bytes();
        match owner.record_inputs(2, owner.input_revision(2).unwrap(), next.clone()) {
            Ok(_) => {
                assert_eq!(owner.captured_input_bytes(), spent + next.logical_bytes());
                choose_first = !choose_first;
            }
            Err(Error::Limit) => { exhausted = true; break; }
            Err(error) => panic!("unexpected filler admission: {error:?}"),
        }
    }
    assert!(exhausted);
    let spent = owner.captured_input_bytes();
    assert!(MAX_CAPTURED_INPUT_BYTES - spent < refined_size);
    assert_eq!(owner.current_hosted_sidecar(&sidecar).unwrap(), before.input());
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &completed), Err(Error::Limit));
    // Retrying the same bounded request must reach the same recorder refusal,
    // not discover a silently consumed residual or a changed planner frontier.
    assert_eq!(owner.refine_hosted_sidecar(&mut sidecar, &completed), Err(Error::Limit));
    assert_eq!(owner.captured_input_bytes(), spent);
    assert_eq!(sidecar.input_revision(), 1);
    assert_eq!(sidecar.round(), &before);
    assert_eq!(owner.current_hosted_sidecar(&sidecar).unwrap(), before.input());
    assert_eq!(endpoint.execution_count(), 0);
}
