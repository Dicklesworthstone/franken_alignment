use super::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::{MAX_CAPTURED_INPUT_BYTES, sidecar::SidecarRefinementOutcome};

#[test]
fn coarse_abstention_buys_exact_original_residual_then_fresh_two_key_review_publishes() {
    let (mut owner, mut endpoint, human) = owner(0, true, true); step(&mut owner);
    let action = propose(&mut owner, 1); let mut sidecar = begin(&mut owner, 1);
    let coarse = sidecar.round().clone(); let numerical = owner.hosted_learned_generation().unwrap();
    let source = sidecar.source().encode().unwrap();
    let group = sidecar.source().groups().next().unwrap();
    let residual = sidecar.source().residual_bytes(group).unwrap().to_vec();
    // Two independently completed original rounds over the same coarse input.
    // A retained Allow cannot approve a later richer input version.
    let old_allow = review(&mut owner, &sidecar, 101, Verdict::Allow);
    let abstention = review(&mut owner, &sidecar, 102, Verdict::Abstain);
    let refined = owner.refine_learned_sidecar(&mut sidecar, &abstention).unwrap();
    let round = match refined {
        SidecarRefinementOutcome::Refined { group: selected, round } => { assert_eq!(selected, group); round }
        other => panic!("expected original residual: {other:?}"),
    };
    assert_eq!(sidecar.input_revision(), 2); assert_eq!(sidecar.round(), &round);
    assert_eq!(round.selected_groups(), &[group]);
    assert_eq!(round.work().rounds, 2); assert_eq!(round.work().residual_bytes, residual.len());
    assert_eq!(round.work().committee_bytes, coarse.input().logical_bytes() + round.input().logical_bytes());
    assert!(round.payload().ends_with(&residual));
    assert_eq!(sidecar.source().encode().unwrap(), source);
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
    assert_eq!(owner.refine_learned_sidecar(&mut sidecar, &abstention), Err(Error::Stale));
    assert_eq!(owner.apply_review(old_allow, Some(round.input()), &snapshot()).err(), Some(Error::Stale));
    assert!(owner.authorize(1, Some(round.input()), &snapshot()).is_err());
    let completed = review(&mut owner, &sidecar, 103, Verdict::Allow);
    assert_eq!(owner.refine_learned_sidecar(&mut sidecar, &completed).unwrap(), SidecarRefinementOutcome::Final);
    let input = owner.current_learned_sidecar(&sidecar).unwrap().clone();
    owner.apply_review(completed, Some(&input), &snapshot()).unwrap();
    let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&permit, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    let request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    assert_eq!(request.input_revision(), 2); assert_eq!(request.inputs(), &input);
    let key = human.unwrap().approve(&request, ElapsedTick(1)).unwrap();
    let message = owner.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1); assert_eq!(owner.inspect().ledger.charged, 16);
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
}

#[test]
fn all_three_exact_budgets_succeed_and_one_less_cannot_advance_or_relax_required_input() {
    let (mut control, _, _) = owner(0, true, false); step(&mut control); propose(&mut control, 1);
    let mut sidecar = begin(&mut control, 1); let coarse = sidecar.round().input().logical_bytes();
    let completed = review(&mut control, &sidecar, 101, Verdict::Abstain);
    control.refine_learned_sidecar(&mut sidecar, &completed).unwrap();
    let exact = SidecarCongressBudget { rounds: 2, residual_bytes: sidecar.round().work().residual_bytes,
        committee_bytes: coarse + sidecar.round().input().logical_bytes() };
    for mode in 0..4 {
        let (mut owner, _, _) = owner(0, true, false); step(&mut owner); propose(&mut owner, 1);
        let mut choice = request(&owner, 1); choice.budget = exact;
        match mode {
            1 => choice.budget.rounds -= 1,
            2 => choice.budget.residual_bytes -= 1,
            3 => choice.budget.committee_bytes -= 1,
            _ => {}
        }
        let mut sidecar = owner.begin_learned_sidecar(1, owner.actor_revision(), choice).unwrap();
        let before = sidecar.round().clone(); let spent = owner.captured_input_bytes();
        let completed = review(&mut owner, &sidecar, 101, Verdict::Abstain);
        let outcome = owner.refine_learned_sidecar(&mut sidecar, &completed).unwrap();
        if mode == 0 {
            assert!(matches!(outcome, SidecarRefinementOutcome::Refined { .. }));
            assert_eq!(sidecar.round().work().rounds, exact.rounds);
            assert_eq!(sidecar.round().work().residual_bytes, exact.residual_bytes);
            assert_eq!(sidecar.round().work().committee_bytes, exact.committee_bytes);
        } else {
            assert!(matches!(outcome, SidecarRefinementOutcome::BudgetExhausted { .. }));
            assert_eq!(sidecar.round(), &before); assert_eq!(sidecar.input_revision(), 1);
            assert_eq!(owner.captured_input_bytes(), spent);
            assert_eq!(owner.current_learned_sidecar(&sidecar).unwrap(), before.input());
            assert!(owner.authorize(1, Some(before.input()), &snapshot()).is_err());
        }
    }
}

#[test]
fn missing_workers_and_unanswerable_priority_never_become_consent_or_new_packets() {
    let (mut owner, endpoint, _) = owner(0, true, false); step(&mut owner); propose(&mut owner, 1);
    let mut sidecar = begin(&mut owner, 1); let before = sidecar.round().clone();
    let mut session = owner.begin_learned_sidecar_review(&sidecar, 101, [7; 32], window(), &snapshot()).unwrap();
    let missing = session.finish(ElapsedTick(30)).unwrap(); owner.observe_time(ElapsedTick(30)).unwrap();
    assert_eq!(owner.refine_learned_sidecar(&mut sidecar, &missing).unwrap(),
        SidecarRefinementOutcome::Missing { members: vec!["reviewer".to_owned()] });
    assert_eq!(sidecar.round(), &before);
    let result = owner.apply_review(missing, Some(before.input()), &snapshot()).unwrap();
    assert_eq!(result.policy.control.decision.consequence, Consequence::HoldEffect);
    assert!(owner.authorize(1, Some(before.input()), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);

    let (mut limited, _, _) = super::owner(0, true, false); step(&mut limited); propose(&mut limited, 1);
    let mut choice = request(&limited, 1); choice.priority.clear();
    let mut sidecar = limited.begin_learned_sidecar(1, limited.actor_revision(), choice).unwrap();
    let before = sidecar.round().clone(); let completed = review(&mut limited, &sidecar, 101, Verdict::Abstain);
    assert!(matches!(limited.refine_learned_sidecar(&mut sidecar, &completed).unwrap(), SidecarRefinementOutcome::Unresolved { .. }));
    assert_eq!(sidecar.round(), &before); assert!(limited.learned_sidecar_required());
}

#[test]
fn same_number_foreign_round_and_different_attempt_cannot_refine_a_source_bound_plan() {
    let (mut first, _, _) = owner(0, true, false); step(&mut first); propose(&mut first, 1);
    let (mut foreign, _, _) = owner(0, true, false); step(&mut foreign); propose(&mut foreign, 1);
    let mut sidecar = begin(&mut first, 1); let other = begin(&mut foreign, 1);
    assert_eq!(sidecar.round(), other.round());
    let completed = review(&mut foreign, &other, 101, Verdict::Abstain);
    assert_eq!(first.refine_learned_sidecar(&mut sidecar, &completed), Err(Error::Binding));
    propose(&mut first, 2); let other = begin(&mut first, 2);
    let completed = review(&mut first, &other, 102, Verdict::Abstain);
    assert_eq!(first.refine_learned_sidecar(&mut sidecar, &completed), Err(Error::Binding));
    assert_eq!(sidecar.round().work().rounds, 1);
    let completed = review(&mut first, &sidecar, 103, Verdict::Abstain);
    assert!(matches!(first.refine_learned_sidecar(&mut sidecar, &completed).unwrap(), SidecarRefinementOutcome::Refined { .. }));
}

#[test]
fn actual_broker_input_exhaustion_retains_planner_and_mandatory_revision_atomically() {
    let (mut owner, endpoint, _) = owner(0, true, false); step(&mut owner);
    let action = propose(&mut owner, 1); let mut sidecar = begin(&mut owner, 1);
    let completed = review(&mut owner, &sidecar, 101, Verdict::Abstain);
    let before = sidecar.round().clone(); let numerical = owner.hosted_learned_generation().unwrap();
    // Independent ORIGINAL planner proves this residual fits the frozen plan;
    // it is the broker-wide recorder, not a synthetic failure, that will refuse.
    let choice = request(&owner, 1);
    let mut control = SidecarCongressPlan::new(sidecar.source().clone(), choice.identity, choice.priority, choice.budget).unwrap();
    assert_eq!(control.initial(&action, owner.contracts()).unwrap(), before);
    let refined_size = match control.refine_after(&completed, &action, owner.contracts()).unwrap() {
        SidecarRefinementOutcome::Refined { round, .. } => round.input().logical_bytes(),
        other => panic!("original funded control failed: {other:?}"),
    };
    let filler_action = propose(&mut owner, 2);
    let mut packets = Vec::new();
    for transform_id in [7, 8] {
        let mut planner = SidecarCongressPlan::new(sidecar.source().clone(),
            SidecarIdentity { object_id: 1002, generation: 1, transform_id }, Vec::new(), SidecarCongressBudget::default()).unwrap();
        packets.push(planner.initial(&filler_action, owner.contracts()).unwrap().input().clone());
    }
    assert!(refined_size > packets.iter().map(CommitteeInput::logical_bytes).max().unwrap());
    let bound = MAX_CAPTURED_INPUT_BYTES / packets.iter().map(CommitteeInput::logical_bytes).min().unwrap() + 2;
    let mut exhausted = false;
    for index in 0..bound {
        let input = &packets[index % 2]; let spent = owner.captured_input_bytes();
        match owner.record_inputs(2, owner.input_revision(2).unwrap(), input.clone()) {
            Ok(_) => assert_eq!(owner.captured_input_bytes(), spent + input.logical_bytes()),
            Err(Error::Limit) => { exhausted = true; break; }
            Err(error) => panic!("unexpected original recorder result: {error:?}"),
        }
    }
    assert!(exhausted); let spent = owner.captured_input_bytes();
    assert!(MAX_CAPTURED_INPUT_BYTES - spent < refined_size);
    for _ in 0..2 { assert_eq!(owner.refine_learned_sidecar(&mut sidecar, &completed), Err(Error::Limit)); }
    assert_eq!(owner.captured_input_bytes(), spent); assert_eq!(sidecar.input_revision(), 1);
    assert_eq!(sidecar.round(), &before); assert_eq!(owner.current_learned_sidecar(&sidecar).unwrap(), before.input());
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
    // The preserved marker still admits a fresh ORIGINAL coarse round.
    assert!(owner.begin_learned_sidecar_review(&sidecar, 103, [7; 32], window(), &snapshot()).is_ok());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn source_change_or_input_replacement_refuses_before_any_residual_purchase() {
    for mode in 0..3 {
        let (mut owner, _, _) = owner(if mode == 1 { 2 } else { 0 }, true, false); step(&mut owner);
        let action = propose(&mut owner, 1); let mut sidecar = begin(&mut owner, 1);
        let completed = review(&mut owner, &sidecar, 101, Verdict::Abstain); let before = sidecar.round().clone();
        if mode < 2 { step(&mut owner); }
        else { owner.record_inputs(1, sidecar.input_revision(), text_only(&owner, &action)).unwrap(); }
        let spent = owner.captured_input_bytes();
        assert!(owner.refine_learned_sidecar(&mut sidecar, &completed).is_err());
        assert_eq!(owner.captured_input_bytes(), spent); assert_eq!(sidecar.round(), &before);
        assert_eq!(sidecar.input_revision(), 1);
    }
}

#[test]
fn independently_observed_completion_time_and_deadline_bound_refinement() {
    let (mut owner, _, _) = owner(0, true, false); step(&mut owner); propose(&mut owner, 1);
    let mut sidecar = begin(&mut owner, 1);
    let mut session = owner.begin_learned_sidecar_review(&sidecar, 101, [7; 32], window(), &snapshot()).unwrap();
    let salt = [11; 32]; let commitment = session.commitment("reviewer", Verdict::Abstain, &salt).unwrap();
    session.commit("reviewer", commitment, ElapsedTick(2)).unwrap();
    session.open_reveals(ElapsedTick(2)).unwrap();
    session.reveal("reviewer", Verdict::Abstain, &salt, ElapsedTick(2)).unwrap();
    let completed = session.finish(ElapsedTick(2)).unwrap(); let before = sidecar.round().clone();
    assert_eq!(owner.refine_learned_sidecar(&mut sidecar, &completed), Err(Error::Stale));
    assert_eq!(sidecar.round(), &before); owner.observe_time(ElapsedTick(2)).unwrap();
    assert!(matches!(owner.refine_learned_sidecar(&mut sidecar, &completed).unwrap(), SidecarRefinementOutcome::Refined { .. }));
    // A new completed round at the current tick, not the old coarse verdict.
    let mut session = owner.begin_learned_sidecar_review(&sidecar, 102, [7; 32], window(), &snapshot()).unwrap();
    let commitment = session.commitment("reviewer", Verdict::Abstain, &salt).unwrap();
    session.commit("reviewer", commitment, ElapsedTick(2)).unwrap(); session.open_reveals(ElapsedTick(2)).unwrap();
    session.reveal("reviewer", Verdict::Abstain, &salt, ElapsedTick(2)).unwrap();
    let completed = session.finish(ElapsedTick(2)).unwrap(); let before = sidecar.round().clone();
    owner.observe_time(ElapsedTick(100)).unwrap();
    assert_eq!(owner.refine_learned_sidecar(&mut sidecar, &completed), Err(Error::Stale));
    assert_eq!(sidecar.round(), &before);
}
