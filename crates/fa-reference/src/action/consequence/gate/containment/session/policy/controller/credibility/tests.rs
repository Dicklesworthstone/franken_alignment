use super::*;
use super::super::{ControllerConfig, PolicyReview, PolicySession};
use crate::action::consequence::Consequence;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::congress::credibility::{
    Campaign, CaseSpec, CredibilityLedger, EvaluationLabel, EvaluationScope,
    HelperGeneration, LabelSource, LabelVerdict, Observation,
};
use crate::action::consequence::gate::TargetCeiling;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, VERSION};
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;
use std::collections::{BTreeMap, BTreeSet};

fn profile() -> RestartProfile {
    RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
        tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::ExactRestart }
}
fn actor(profile: RestartProfile) -> ActorState {
    ActorState::new(profile, vec![1], vec![2], vec![3], 1).unwrap()
}
fn spec(epoch: u64) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1,
            expected_version: 1, generation: 1 }),
        payload: b"hello".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 5 }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }
}
fn config() -> ControllerConfig {
    ControllerConfig { scope: spec(0).scope, total: 100, max_attempts: 128,
        actor: actor(profile()), suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::Absent { key: 7 }]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([
                ("alice".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 8 }),
                ("bob".to_owned(), MemberPolicy { cohort: "b".to_owned(), weight: 8 }),
            ]), caps: Caps { per_member: 10, per_cohort: 10 },
            continue_minimum: 16, continue_hold_maximum: 0, narrow_at: 16, suspend_at: 20,
            minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap() }
}
fn finish(mut session: PolicySession, bob: Option<Verdict>) -> PolicyReview {
    for (id, verdict) in [("alice", Verdict::Allow), ("bob", bob.unwrap_or(Verdict::Allow))] {
        let commitment = session.commitment(id, verdict, b"salt").unwrap();
        session.commit(id, commitment).unwrap();
    }
    session.open_reveals().unwrap();
    session.reveal("alice", Verdict::Allow, b"salt").unwrap();
    if let Some(verdict) = bob { session.reveal("bob", verdict, b"salt").unwrap(); }
    session.finish().unwrap()
}
fn review(controller: &mut PolicyAuthority, id: u64) -> FrozenAction {
    let proposal = controller.propose(id, spec(controller.inspect().ledger.epoch), &snapshot()).unwrap();
    let session = controller.begin_review(id, 100 + id, [1; 32], &snapshot()).unwrap();
    controller.apply_review(finish(session, Some(Verdict::Allow)), &snapshot()).unwrap();
    proposal.action
}
fn ready() -> PolicyAuthority {
    let mut controller = PolicyAuthority::new(config()).unwrap();
    controller.observe_time(ElapsedTick(1)).unwrap();
    for id in 1..=2 {
        review(&mut controller, id);
        controller.cancel(id).unwrap();
    }
    assert_eq!(controller.inspect().sequence, 2);
    controller
}
fn evidence(campaign: u64, model: u64, label: LabelVerdict) -> CredibilitySnapshot {
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign, model_generation: model, evaluator_generation: 1,
            held_out_manifest: [9; 32] }, label_owner: "evaluator".to_owned(),
        helpers: BTreeMap::from([
            ("alice".to_owned(), HelperGeneration { generation: 1, cohort: "a".to_owned() }),
            ("bob".to_owned(), HelperGeneration { generation: 1, cohort: "b".to_owned() }),
        ]), strata: BTreeSet::from(["publication".to_owned()]),
        cases: (1..=2).map(|id| CaseSpec { id, stratum: "publication".to_owned(),
            evidence_root: [id as u8; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for id in 1..=2 {
        let observation = if id == 1 { Observation::Clear } else { Observation::Hold { first_sequence: 1 } };
        ledger.record_observations(id, BTreeMap::from([
            ("alice".to_owned(), observation), ("bob".to_owned(), observation),
        ])).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "evaluator".to_owned(),
            evaluator_generation: 1, source: LabelSource::IndependentEvaluation,
            evidence_root: [id as u8; 32], recorded_sequence: 2,
            verdict: if id == 1 { LabelVerdict::Safe } else { label } }).unwrap();
    }
    ledger.seal(2).unwrap()
}
fn activation(controller: &PolicyAuthority, operation: u64) -> CredibilityActivation {
    let snapshot = evidence(operation, controller.actor().profile().model_generation, LabelVerdict::Violation);
    CredibilityActivation { operation, expected_control_sequence: controller.inspect().sequence,
        expected_epoch: controller.inspect().ledger.epoch, scope: spec(0).scope,
        policy_generation: controller.policy().generation(), actor_profile: controller.actor().profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().to_owned(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(),
            reducer_generation: controller.congress.generation + 1 },
        stratum: "publication".to_owned(),
        requirements: CredibilityRequirements { minimum_safe_cases: 1, minimum_violation_cases: 1,
            minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 10, lead_bonus_weight: 0,
            lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}
fn authorize(controller: &mut PolicyAuthority, id: u64) -> (FrozenAction, Permit) {
    let action = review(controller, id);
    let permit = controller.authorize(id, &snapshot()).unwrap();
    (action, permit)
}

#[test]
fn activated_weights_drive_original_review_permit_and_dispatch() {
    let mut controller = ready();
    let (old, old_permit) = authorize(&mut controller, 10);
    let (sent, sent_permit) = authorize(&mut controller, 11);
    controller.dispatch(&sent_permit, &sent, &snapshot()).unwrap();
    controller.mark_unknown(11).unwrap();
    controller.propose(12, spec(0), &snapshot()).unwrap();
    let old_review = finish(controller.begin_review(12, 112, [1; 32], &snapshot()).unwrap(), Some(Verdict::Allow));
    let before = controller.inspect();
    let request = activation(&controller, 1);
    let receipt = controller.activate_credibility(request).unwrap();
    assert_eq!(receipt.cancelled, vec![10, 12]);
    assert_eq!(receipt.refunded_units, 5);
    assert_eq!(receipt.sequence, before.sequence + 1);
    assert_eq!(receipt.revocation_floor, before.ledger.epoch + 1);
    assert_eq!(controller.congress.members["alice"].weight, 10);
    assert_eq!(controller.congress.continue_minimum, 16);
    assert_eq!(controller.inspect().ledger.charged, 5);
    assert_eq!(controller.inspect().ledger.stages[&11], ActionState::Unknown);
    assert!(controller.dispatch(&old_permit, &old, &snapshot()).is_err());
    assert!(controller.apply_review(old_review, &snapshot()).is_err());
    let (fresh, permit) = authorize(&mut controller, 13);
    controller.dispatch(&permit, &fresh, &snapshot()).unwrap();
    assert_eq!(controller.review_receipts().last().unwrap().control.binding.reducer_generation, 2);
    assert_eq!(controller.inspect().ledger.charged, 10);
}

#[test]
fn wrong_scope_profile_predecessor_and_binding_are_atomic() {
    for variant in 0..8 {
        let mut controller = ready();
        let mut request = activation(&controller, 1);
        match variant {
            0 => request.scope.principal += 1,
            1 => request.actor_profile.tokenizer_generation += 1,
            2 => request.policy_generation += 1,
            3 => request.expected_control_sequence += 1,
            4 => request.expected_epoch += 1,
            5 => request.binding.scope.model_generation += 1,
            6 => request.binding.reducer_generation = 1,
            _ => request.binding.helpers.get_mut("bob").unwrap().generation += 1,
        }
        let before = controller.inspect();
        assert!(controller.activate_credibility(request).is_err(), "variant {variant}");
        assert_eq!(controller.inspect(), before);
        assert_eq!(controller.credibility_changes().len(), 0);
        let valid = activation(&controller, 1);
        controller.activate_credibility(valid).unwrap();
        assert!(controller.check_credibility().is_ok());
    }
}

#[test]
fn incomplete_evaluator_outcomes_cannot_activate_but_complete_neighbors_can() {
    let mut controller = ready();
    let mut request = activation(&controller, 1);
    request.snapshot = evidence(1, 1, LabelVerdict::Censored);
    let before = controller.inspect();
    assert_eq!(controller.activate_credibility(request), Err(Error::Incomplete));
    assert_eq!(controller.inspect(), before);
    let valid = activation(&controller, 1);
    controller.activate_credibility(valid).unwrap();
    let (action, permit) = authorize(&mut controller, 10);
    controller.dispatch(&permit, &action, &snapshot()).unwrap();
}

#[test]
fn exact_retries_neither_advance_epoch_nor_restore_an_old_activation() {
    let mut controller = ready();
    let request = activation(&controller, 1);
    let first = controller.activate_credibility(request.clone()).unwrap();
    let second = activation(&controller, 2);
    controller.activate_credibility(second).unwrap();
    let before = controller.inspect();
    assert_eq!(controller.activate_credibility(request.clone()).unwrap(), first);
    assert_eq!(controller.inspect(), before);
    assert_eq!(controller.congress.generation, 3);
    assert_eq!(controller.credibility_changes().len(), 2);
    let mut changed = request;
    changed.expected_epoch += 1;
    assert_eq!(controller.activate_credibility(changed), Err(Error::Binding));
    assert_eq!(controller.inspect(), before);
}

#[test]
fn expiry_blocks_all_positive_paths_and_preserves_reservations() {
    let mut controller = ready();
    let mut request = activation(&controller, 1);
    request.requirements.maximum_evidence_age = 3; // inclusive end: sequence 5
    controller.activate_credibility(request).unwrap(); // sequence 3
    let (action, permit) = authorize(&mut controller, 10); // sequence 4
    review(&mut controller, 11); // sequence 5, not yet authorized
    assert_eq!(controller.inspect().sequence, 5);
    assert!(controller.check_credibility().is_ok());
    controller.propose(12, spec(1), &snapshot()).unwrap();
    let allow = finish(controller.begin_review(12, 112, [1; 32], &snapshot()).unwrap(), Some(Verdict::Allow));
    let hold = finish(controller.begin_review(12, 113, [1; 32], &snapshot()).unwrap(), None);
    controller.apply_review(hold, &snapshot()).unwrap(); // REAL restrictive transition to 6
    let before = controller.inspect();
    assert_eq!(controller.check_credibility(), Err(Error::Stale));
    assert_eq!(controller.dispatch(&permit, &action, &snapshot()), Err(Error::Stale));
    assert_eq!(controller.authorize(11, &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(controller.begin_review(11, 114, [1; 32], &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(controller.apply_review(allow, &snapshot()), Err(Error::Stale));
    assert_eq!(controller.propose(13, spec(1), &snapshot()), Err(Error::Stale));
    assert_eq!(controller.inspect(), before);
    assert_eq!(controller.inspect().ledger.reserved, 5);
    controller.cancel(10).unwrap();
    assert_eq!(controller.inspect().ledger.available, 100);
}

#[test]
fn expired_evidence_cannot_block_an_exact_policy_denial() {
    let mut controller = ready();
    let mut request = activation(&controller, 1);
    request.requirements.maximum_evidence_age = 1;
    controller.activate_credibility(request).unwrap(); // sequence 3, valid at end
    controller.propose(10, spec(1), &snapshot()).unwrap();
    let mut violation = snapshot();
    violation.values.insert(7, vec![9]);
    let denial = finish(controller.begin_review(10, 110, [1; 32], &violation).unwrap(), Some(Verdict::Allow));
    controller.apply_review(denial, &violation).unwrap(); // sequence 4, expires
    assert_eq!(controller.check_credibility(), Err(Error::Stale));
    let proposal = controller.propose(11, spec(1), &violation).unwrap();
    assert_eq!(proposal.state, ActionState::Denied);
    assert_eq!(controller.inspect().ledger.available, 100);
}

#[test]
fn evidence_withdrawal_sticks_while_actor_profile_substitution_still_refuses() {
    let mut controller = ready();
    let request = activation(&controller, 1);
    controller.activate_credibility(request.clone()).unwrap();
    let same = ActorState::new(profile(), vec![1, 2], vec![4], vec![5], 2).unwrap();
    controller.replace_actor_state(controller.actor_revision(), same).unwrap();
    assert!(controller.check_credibility().is_ok());
    let mut changed = profile();
    changed.tokenizer_generation += 1;
    let before = controller.inspect();
    assert_eq!(controller.replace_actor_state(controller.actor_revision(), actor(changed)), Err(Error::Binding));
    assert_eq!(controller.inspect(), before);
    assert!(controller.check_credibility().is_ok());
    controller.withdraw_credibility(CredibilityWithdrawalRequest {
        operation: 1, expected_control_sequence: before.sequence,
        expected_epoch: before.ledger.epoch,
    }).unwrap();
    assert_eq!(controller.check_credibility(), Err(Error::Stale));
    controller.replace_actor_state(controller.actor_revision(), actor(profile())).unwrap();
    assert_eq!(controller.check_credibility(), Err(Error::Stale));
    controller.activate_credibility(request).unwrap(); // retry is historical, NOT reactivation
    assert_eq!(controller.check_credibility(), Err(Error::Stale));
    let refresh = activation(&controller, 2);
    controller.activate_credibility(refresh).unwrap();
    assert!(controller.check_credibility().is_ok());
}

#[test]
fn exact_policy_change_requires_new_activation_without_erasing_old_evidence() {
    let mut controller = ready();
    let request = activation(&controller, 1);
    controller.activate_credibility(request).unwrap();
    let before = controller.inspect();
    let next = Policy::new(2, vec![Predicate::Absent { key: 8 }]).unwrap();
    controller.replace_policy(before.sequence, before.ledger.epoch, next).unwrap();
    assert_eq!(controller.check_credibility(), Err(Error::Stale));
    let refresh = activation(&controller, 2);
    controller.activate_credibility(refresh).unwrap();
    assert_eq!(controller.credibility_changes().len(), 2);
    assert_eq!(controller.credibility_changes().next().unwrap().campaign, 1);
    let (action, permit) = authorize(&mut controller, 10);
    controller.dispatch(&permit, &action, &snapshot()).unwrap();
}

#[test]
fn refresh_cannot_weaken_requirements_or_change_evaluator_ownership() {
    for variant in 0..4 {
        let mut controller = ready();
        let first = activation(&controller, 1);
        controller.activate_credibility(first).unwrap();
        let mut next = activation(&controller, 2);
        match variant {
            0 => next.requirements.maximum_evidence_age += 1,
            1 => next.requirements.minimum_timely_recall_ppm -= 1,
            2 => next.binding.label_owner = "replacement".to_owned(),
            _ => { next.binding.strata.insert("other".to_owned()); }
        }
        let before = controller.inspect();
        assert_eq!(controller.activate_credibility(next), Err(Error::Binding));
        assert_eq!(controller.inspect(), before);
        let valid = activation(&controller, 2);
        controller.activate_credibility(valid).unwrap();
    }
}

#[test]
fn activation_at_expiry_neighbor_and_overflow_are_atomic() {
    let mut controller = ready();
    let mut request = activation(&controller, 1);
    request.requirements.maximum_evidence_age = 1;
    controller.activate_credibility(request).unwrap(); // result 3 is still current
    let mut late = activation(&controller, 2);
    late.requirements.maximum_evidence_age = 1;
    let before = controller.inspect();
    assert_eq!(controller.activate_credibility(late), Err(Error::Stale)); // result 4 is not
    assert_eq!(controller.inspect(), before);
    // Explicit boundary injection, not a claimed organically produced history.
    // Use a fresh controller to distinguish arithmetic refusal from expiry.
    let mut fresh = ready();
    fresh.host.gate.authority.rights.epoch = u64::MAX;
    let overflow = CredibilityActivation { expected_epoch: u64::MAX, ..activation(&fresh, 1) };
    let before = fresh.inspect();
    assert_eq!(fresh.activate_credibility(overflow), Err(Error::Overflow));
    assert_eq!(fresh.inspect(), before);
    assert_eq!(fresh.credibility_changes().len(), 0);
}

#[test]
fn suspended_authority_cannot_be_reopened_by_credibility() {
    let mut controller = ready();
    controller.propose(10, spec(0), &snapshot()).unwrap();
    let mut session = controller.begin_review(10, 110, [1; 32], &snapshot()).unwrap();
    for id in ["alice", "bob"] {
        let digest = session.commitment(id, Verdict::Hold, b"salt").unwrap();
        session.commit(id, digest).unwrap();
    }
    session.open_reveals().unwrap();
    for id in ["alice", "bob"] { session.reveal(id, Verdict::Hold, b"salt").unwrap(); }
    // Initial weights sum to 16, so this first narrows. Force no fake suspension:
    // install first, then actual 20-weight hold votes select SuspendRun.
    let narrowing = session.finish().unwrap();
    assert_eq!(narrowing.decision().consequence, Consequence::NarrowAuthority);
    controller.apply_review(narrowing, &snapshot()).unwrap();
    let request = activation(&controller, 1);
    controller.activate_credibility(request).unwrap();
    controller.propose(11, spec(controller.inspect().ledger.epoch), &snapshot()).unwrap();
    let mut session = controller.begin_review(11, 111, [1; 32], &snapshot()).unwrap();
    for id in ["alice", "bob"] {
        let digest = session.commitment(id, Verdict::Hold, b"salt").unwrap();
        session.commit(id, digest).unwrap();
    }
    session.open_reveals().unwrap();
    for id in ["alice", "bob"] { session.reveal(id, Verdict::Hold, b"salt").unwrap(); }
    controller.apply_review(session.finish().unwrap(), &snapshot()).unwrap();
    assert!(controller.inspect().suspended);
    let request = activation(&controller, 2);
    let before = controller.inspect();
    assert_eq!(controller.activate_credibility(request), Err(Error::WrongState));
    assert_eq!(controller.inspect(), before);
}

#[test]
fn withdrawal_refunds_only_pending_work_and_old_retries_cannot_close_fresh_work() {
    let mut controller = ready();
    let request = activation(&controller, 1);
    controller.activate_credibility(request).unwrap();
    let (pending, permit) = authorize(&mut controller, 10);
    let (sent, sent_permit) = authorize(&mut controller, 11);
    controller.dispatch(&sent_permit, &sent, &snapshot()).unwrap();
    controller.mark_unknown(11).unwrap();
    let before = controller.inspect();
    let request = CredibilityWithdrawalRequest {
        operation: 7, expected_control_sequence: before.sequence, expected_epoch: before.ledger.epoch,
    };
    let receipt = controller.withdraw_credibility(request.clone()).unwrap();
    assert_eq!(receipt.cancelled, vec![10]);
    assert_eq!(receipt.refunded_units, 5);
    assert_eq!(receipt.sequence, before.sequence + 1);
    assert_eq!(controller.inspect().ledger.charged, 5);
    assert_eq!(controller.inspect().ledger.stages[&11], ActionState::Unknown);
    assert_eq!(controller.dispatch(&permit, &pending, &snapshot()), Err(Error::Stale));
    let after = controller.inspect();
    assert_eq!(controller.withdraw_credibility(request.clone()).unwrap(), receipt);
    assert_eq!(controller.inspect(), after);
    let refresh = activation(&controller, 2);
    controller.activate_credibility(refresh).unwrap();
    let (fresh, fresh_permit) = authorize(&mut controller, 12);
    let after = controller.inspect();
    assert_eq!(controller.withdraw_credibility(request.clone()).unwrap(), receipt);
    assert_eq!(controller.inspect(), after);
    let conflict = CredibilityWithdrawalRequest { expected_epoch: after.ledger.epoch, ..request };
    assert_eq!(controller.withdraw_credibility(conflict), Err(Error::Binding));
    controller.dispatch(&fresh_permit, &fresh, &snapshot()).unwrap();
    assert_eq!(controller.inspect().ledger.charged, 10);
}

#[test]
fn withdrawal_predecessor_and_absence_refuse_without_mutation() {
    let mut controller = ready();
    let request = CredibilityWithdrawalRequest {
        operation: 1, expected_control_sequence: 2, expected_epoch: 0,
    };
    let before = controller.inspect();
    assert_eq!(controller.withdraw_credibility(request), Err(Error::Incomplete));
    assert_eq!(controller.inspect(), before);
    let activation = activation(&controller, 1);
    controller.activate_credibility(activation).unwrap();
    let request = CredibilityWithdrawalRequest {
        operation: 1, expected_control_sequence: 3, expected_epoch: 1,
    };
    let before = controller.inspect();
    for invalid in [
        CredibilityWithdrawalRequest { operation: 0, ..request.clone() },
        CredibilityWithdrawalRequest { expected_control_sequence: 4, ..request.clone() },
        CredibilityWithdrawalRequest { expected_epoch: 2, ..request.clone() },
    ] {
        assert!(controller.withdraw_credibility(invalid).is_err());
        assert_eq!(controller.inspect(), before);
    }
    controller.withdraw_credibility(request).unwrap();
    let after = controller.inspect();
    let repeated_loss = CredibilityWithdrawalRequest {
        operation: 2, expected_control_sequence: after.sequence, expected_epoch: after.ledger.epoch,
    };
    assert_eq!(controller.withdraw_credibility(repeated_loss), Err(Error::WrongState));
    assert_eq!(controller.inspect(), after);
    assert_eq!(controller.credibility_withdrawals().count(), 1);
}
