//! Public-API decision replay controls. These exercise the reference pipeline,
//! not an authenticated provider, real effect adapter or production checkpoint.

use fa_reference::action::consequence::{Consequence, Rule};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{
    ActorState, ResetRequest, RestartGrade, RestartProfile,
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate, Truth};
use fa_reference::action::consequence::gate::containment::session::policy::controller::{
    ArchivedPolicyReceipt, ControllerConfig, DecisionArchive, MAX_ARCHIVE_BYTES,
    PolicyAuthority, PolicyReview, PolicySession, ReviewAnchor,
};
use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION,
};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, ReadWitness, Snapshot};
use std::collections::BTreeMap;

fn spec(epoch: u64) -> ActionSpec {
    ActionSpec {
        version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 6, object: 7, contract_version: 1, expected_version: 2, generation: 3 }),
        payload: b"publish\0\xff".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: epoch, deadline: ElapsedTick(100), units: 4,
    }
}

fn exact_policy(generation: u64) -> Policy {
    Policy::new(generation, vec![
        Predicate::ExactValue { key: 7, value: vec![9] },
        Predicate::Absent { key: 8 },
        Predicate::EmptyRange { start: 10, end: 20 },
        Predicate::EmptyRange { start: 30, end: 40 },
        Predicate::Not(3),
        Predicate::PayloadIs(spec(0).payload),
        Predicate::TargetIs(spec(0).target.unwrap()),
        Predicate::PayloadAtMost(128),
        Predicate::UnitsAtMost(8),
        Predicate::Any(vec![3, 4]),
        Predicate::All(vec![0, 1, 2, 5, 6, 7, 8, 9]),
    ]).unwrap()
}

fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 13, complete: true,
        values: BTreeMap::from([
            (7, vec![9]), (35, vec![4]),
            (1000, b"UNRELATED_PROVIDER_SECRET_MUST_NOT_BE_EXPORTED".to_vec()),
        ]),
    }
}

fn config() -> ControllerConfig {
    let actor = ActorState::new(RestartProfile {
        id: 1, generation: 1, host_generation: 1, model_generation: 1,
        tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
    }, vec![1], vec![2], vec![3], 1).unwrap();
    let congress = CongressPolicy {
        generation: 4,
        members: BTreeMap::from([
            ("alice".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 3 }),
            ("bob".to_owned(), MemberPolicy { cohort: "b".to_owned(), weight: 3 }),
        ]),
        caps: Caps { per_member: 3, per_cohort: 3 },
        continue_minimum: 6, continue_hold_maximum: 0, narrow_at: 4, suspend_at: 6,
        minimum_members: 2, minimum_cohorts: 2,
    };
    ControllerConfig {
        scope: spec(0).scope, total: 20, max_attempts: 16, actor,
        suspend_at_incident: 3, policy: exact_policy(1), congress,
        narrowed_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap(),
    }
}

fn controller() -> PolicyAuthority {
    let mut controller = PolicyAuthority::new(config()).unwrap();
    controller.observe_time(ElapsedTick(1)).unwrap();
    controller
}

fn finish(mut session: PolicySession, votes: [Option<Verdict>; 2]) -> PolicyReview {
    for (index, member) in ["alice", "bob"].into_iter().enumerate() {
        let value = session.commitment(member, votes[index].unwrap_or(Verdict::Allow), member.as_bytes()).unwrap();
        session.commit(member, value).unwrap();
    }
    session.open_reveals().unwrap();
    for (index, member) in ["alice", "bob"].into_iter().enumerate() {
        if let Some(verdict) = votes[index] {
            session.reveal(member, verdict, member.as_bytes()).unwrap();
        }
    }
    session.finish().unwrap()
}

fn archive() -> (ReviewAnchor, ArchivedPolicyReceipt) {
    let mut controller = controller();
    controller.propose(1, spec(0), &snapshot()).unwrap();
    let session = controller.begin_review(1, 17, [3; 32], &snapshot()).unwrap();
    let anchor = session.replay_anchor();
    let review = finish(session, [Some(Verdict::Allow); 2]);
    let archived = controller.apply_review_archived(review, &snapshot()).unwrap();
    (anchor, archived)
}

#[test]
fn pre_vote_anchor_and_capsule_replay_after_the_controller_is_gone() {
    let (anchor, archived) = archive();
    let expected = ReviewAnchor::from_bytes(&anchor.to_bytes().unwrap()).unwrap();
    let bytes = archived.archive.to_bytes().unwrap();
    let decoded = DecisionArchive::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, archived.archive);
    assert_eq!(decoded.to_bytes().unwrap(), bytes);
    let replayed = decoded.verify(&expected).unwrap();
    assert_eq!(replayed.decision().consequence, Consequence::Continue);
    assert_eq!(replayed.tally().permit_weight, 6);
    assert_eq!(replayed.evaluation(), &archived.receipt.evaluation);
    assert_eq!(archived.verify(&expected).unwrap(), replayed);
    assert!(!bytes.windows(b"UNRELATED_PROVIDER_SECRET".len()).any(|w| w == b"UNRELATED_PROVIDER_SECRET"));
}

#[test]
fn replay_transport_contains_no_process_local_authority() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ReviewAnchor>(); send_sync::<DecisionArchive>();
    let (anchor, archived) = archive();
    let bytes = archived.archive.to_bytes().unwrap();
    let result = std::thread::spawn(move || DecisionArchive::verify_bytes(&bytes, &anchor)).join().unwrap().unwrap();
    assert_eq!(result.decision().consequence, Consequence::Continue);
}

#[test]
fn archived_review_still_needs_normal_authorization_and_current_evidence() {
    let mut controller = controller();
    let action = controller.propose(1, spec(0), &snapshot()).unwrap().action;
    let session = controller.begin_review(1, 17, [3; 32], &snapshot()).unwrap();
    let anchor = session.replay_anchor();
    let review = finish(session, [Some(Verdict::Allow); 2]);
    let archived = controller.apply_review_archived(review, &snapshot()).unwrap();
    archived.verify(&anchor).unwrap();
    assert_eq!(controller.inspect().ledger.available, 20);
    assert_eq!(controller.inspect().ledger.stages[&1], ActionState::Reviewing);
    let permit = controller.authorize(1, &snapshot()).unwrap();
    let mut changed = snapshot(); changed.values.insert(8, vec![1]);
    assert_eq!(controller.dispatch(&permit, &action, &changed), Err(Error::Binding));
    controller.dispatch(&permit, &action, &snapshot()).unwrap();
    assert_eq!(controller.inspect().ledger.charged, 4);
}

#[test]
fn replaying_an_archive_cannot_approve_another_controller() {
    let (anchor, archived) = archive();
    let mut other = controller();
    other.propose(1, spec(0), &snapshot()).unwrap();
    let before = other.inspect();
    assert_eq!(archived.verify(&anchor).unwrap().decision().consequence, Consequence::Continue);
    assert_eq!(other.authorize(1, &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(other.inspect(), before);
}

#[test]
fn false_decision_weights_trace_and_missing_lists_are_recomputed() {
    let (anchor, valid) = archive();
    valid.verify(&anchor).unwrap();
    let mut changed = valid.archive.clone();
    changed.decision.consequence = Consequence::SuspendRun;
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = valid.archive.clone();
    changed.decision.rules.push(Rule::MandatoryAbsent);
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = valid.archive.clone();
    changed.tally.permit_weight += 1;
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = valid.archive.clone();
    changed.tally.admitted_weights.insert("alice".to_owned(), 4);
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = valid.archive.clone();
    let mut unavailable = snapshot(); unavailable.complete = false;
    changed.evaluation = anchor.policy.evaluate(&anchor.action, &unavailable).unwrap();
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = valid.archive;
    changed.missing.push("bob".to_owned());
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
}

#[test]
fn commitments_and_reveals_are_checked_not_just_reported_verdicts() {
    let (anchor, valid) = archive();
    let mut changed = valid.archive.clone();
    changed.transcript.reveals[0].salt.push(1);
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = valid.archive.clone();
    changed.transcript.commits[0].digest ^= 1;
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = valid.archive.clone();
    changed.transcript.commits.remove(0);
    assert_eq!(changed.verify(&anchor), Err(Error::Missing));
    let mut changed = valid.archive;
    changed.transcript.reveals.remove(0);
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
}

#[test]
fn absent_reveal_and_explicit_abstention_remain_distinct_holds() {
    for bob in [None, Some(Verdict::Abstain)] {
        let mut controller = controller();
        controller.propose(1, spec(0), &snapshot()).unwrap();
        let session = controller.begin_review(1, 17, [3; 32], &snapshot()).unwrap();
        let anchor = session.replay_anchor();
        let review = finish(session, [Some(Verdict::Allow), bob]);
        let archived = controller.apply_review_archived(review, &snapshot()).unwrap();
        let replayed = archived.verify(&anchor).unwrap();
        assert_eq!(replayed.decision().consequence, Consequence::HoldEffect);
        assert_eq!(replayed.missing().is_empty(), bob.is_some());
        assert_eq!(replayed.abstained().is_empty(), bob.is_none());
        assert_eq!(controller.authorize(1, &snapshot()).unwrap_err(), Error::WrongState);
        DecisionArchive::verify_bytes(&archived.archive.to_bytes().unwrap(), &anchor).unwrap();
    }
}

#[test]
fn later_exact_denial_replays_its_own_changed_evidence_after_provider_loss() {
    let mut controller = controller();
    let proposal = controller.propose(1, spec(0), &snapshot()).unwrap();
    let mut changed = snapshot(); changed.values.insert(8, vec![1]);
    let session = controller.begin_review(1, 17, [3; 32], &changed).unwrap();
    let anchor = session.replay_anchor();
    let review = finish(session, [Some(Verdict::Allow); 2]);
    let mut missing = snapshot(); missing.complete = false;
    let archived = controller.apply_review_archived(review, &missing).unwrap();
    let replayed = archived.verify(&anchor).unwrap();
    assert!(proposal.evaluation.certifiable());
    assert_eq!(replayed.evaluation().result(), Truth::Violated);
    assert_eq!(replayed.decision().consequence, Consequence::Deny);
    assert!(replayed.evaluation().witnesses().contains(&ReadWitness::Exact { key: 8, value: Some(vec![1]) }));
    assert_eq!(controller.inspect().ledger.stages[&1], ActionState::Denied);
    let decoded = DecisionArchive::from_bytes(&archived.archive.to_bytes().unwrap()).unwrap();
    assert_eq!(decoded.verify(&anchor).unwrap(), replayed);
}

#[test]
fn historical_replay_survives_policy_change_and_reset_without_restoring_permission() {
    let mut controller = controller();
    let checkpoint = controller.capture_checkpoint(1, 0).unwrap();
    let action = controller.propose(1, spec(0), &snapshot()).unwrap().action;
    let session = controller.begin_review(1, 17, [3; 32], &snapshot()).unwrap();
    let anchor = session.replay_anchor();
    let review = finish(session, [Some(Verdict::Allow); 2]);
    let archived = controller.apply_review_archived(review, &snapshot()).unwrap();
    let old_permit = controller.authorize(1, &snapshot()).unwrap();
    controller.replace_policy(1, 0, exact_policy(2)).unwrap();
    controller.reset(ResetRequest {
        checkpoint, expected_control_sequence: 2, expected_actor_revision: 0,
        binding: ReviewBinding { round: 18, evidence_root: [4; 32], reducer_generation: 4 },
        retained_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap(),
    }).unwrap();
    archived.verify(&anchor).unwrap();
    DecisionArchive::verify_bytes(&archived.archive.to_bytes().unwrap(), &anchor).unwrap();
    assert!(controller.dispatch(&old_permit, &action, &snapshot()).is_err());
    assert_eq!(controller.policy().generation(), 2);
    assert_eq!(controller.inspect().ledger.epoch, 2);
    assert_eq!(controller.inspect().ledger.available, 20);
}

#[test]
fn substituted_context_is_rejected_even_when_the_new_archive_is_self_consistent() {
    let (anchor, valid) = archive();
    let mut changed = valid.archive;
    let mut action = changed.anchor.action.spec().clone(); action.scope.principal += 1;
    changed.anchor.action = FrozenAction::freeze(action).unwrap();
    // The exact policy does not inspect principal: this is a coherent different
    // session, not an invalid vote. Only the separately retained anchor binds it.
    let bytes = changed.to_bytes().unwrap();
    let decoded = DecisionArchive::from_bytes(&bytes).unwrap();
    assert!(decoded.verify(&decoded.anchor).is_ok());
    assert_eq!(decoded.verify(&anchor), Err(Error::Binding));
}

#[test]
fn changed_control_receipt_identity_cannot_attach_to_a_valid_replay() {
    let (anchor, valid) = archive();
    for field in 0..5 {
        let mut changed = valid.clone();
        match field {
            0 => changed.receipt.control.attempt += 1,
            1 => changed.receipt.control.sequence += 1,
            2 => changed.receipt.control.binding.round += 1,
            3 => changed.receipt.control.binding.evidence_root[0] ^= 1,
            _ => changed.receipt.control.binding.reducer_generation += 1,
        }
        assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    }
}

#[test]
fn incomplete_conflicting_and_oversized_observation_bases_refuse() {
    let (_, valid) = archive();
    let mut changed = valid.archive.clone();
    changed.anchor.complete = false;
    assert_eq!(changed.verify(&changed.anchor), Err(Error::Incomplete));
    let mut changed = valid.archive.clone();
    changed.anchor.observations.push(ReadWitness::Exact { key: 7, value: Some(vec![8]) });
    assert_eq!(changed.verify(&changed.anchor), Err(Error::Binding));
    let mut changed = valid.archive.clone();
    changed.anchor.observations.push(ReadWitness::Exact { key: 15, value: Some(vec![1]) });
    assert_eq!(changed.verify(&changed.anchor), Err(Error::Binding));
    let mut changed = valid.archive;
    changed.anchor.observations = vec![ReadWitness::Exact { key: 7, value: Some(vec![0; 65_537]) }];
    assert_eq!(changed.verify(&changed.anchor), Err(Error::Limit));
}

#[test]
fn all_truncations_trailing_data_unknown_version_and_length_bombs_refuse() {
    let (anchor, archived) = archive();
    let bytes = archived.archive.to_bytes().unwrap();
    for end in 0..bytes.len() { assert!(DecisionArchive::from_bytes(&bytes[..end]).is_err(), "end={end}"); }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert_eq!(DecisionArchive::from_bytes(&trailing), Err(Error::InvalidInput));
    let mut version = bytes.clone(); version[b"FA-DECISION-REFERENCE\0".len() + 7] = 2;
    assert_eq!(DecisionArchive::from_bytes(&version), Err(Error::InvalidInput));
    let mut lengths = bytes.clone();
    let payload_length = b"FA-DECISION-REFERENCE\0".len() + 8 + 8 + 8 + 5 * 8 + 1 + 1 + 5 * 8;
    lengths[payload_length..payload_length + 8].copy_from_slice(&u64::MAX.to_be_bytes());
    assert_eq!(DecisionArchive::from_bytes(&lengths), Err(Error::Limit));
    assert_eq!(DecisionArchive::from_bytes(&vec![0; MAX_ARCHIVE_BYTES + 1]), Err(Error::Limit));
    DecisionArchive::verify_bytes(&bytes, &anchor).unwrap();
}

#[test]
fn all_primary_empirical_restrictions_replay_through_actual_control_transitions() {
    for (votes, consequence, narrow_at, suspend_at) in [
        ([Some(Verdict::Allow); 2], Consequence::Continue, 4, 6),
        ([Some(Verdict::Allow), Some(Verdict::Hold)], Consequence::HoldEffect, 4, 6),
        ([Some(Verdict::Allow), Some(Verdict::Hold)], Consequence::NarrowAuthority, 3, 7),
        ([Some(Verdict::Hold); 2], Consequence::SuspendRun, 4, 6),
    ] {
        let mut config = config();
        config.congress.narrow_at = narrow_at; config.congress.suspend_at = suspend_at;
        let mut controller = PolicyAuthority::new(config).unwrap();
        controller.observe_time(ElapsedTick(1)).unwrap();
        controller.propose(1, spec(0), &snapshot()).unwrap();
        let session = controller.begin_review(1, 17, [3; 32], &snapshot()).unwrap();
        let anchor = session.replay_anchor();
        let archived = controller.apply_review_archived(finish(session, votes), &snapshot()).unwrap();
        assert_eq!(archived.verify(&anchor).unwrap().decision().consequence, consequence);
        assert_eq!(DecisionArchive::verify_bytes(&archived.archive.to_bytes().unwrap(), &anchor).unwrap().decision().consequence, consequence);
    }
}

#[test]
fn proportional_cohort_clipping_is_replayed_without_rounding_up() {
    let mut config = config();
    config.congress.members.get_mut("bob").unwrap().cohort = "a".to_owned();
    config.congress.caps.per_cohort = 5;
    config.congress.continue_minimum = 4;
    config.congress.minimum_cohorts = 1;
    let mut controller = PolicyAuthority::new(config).unwrap();
    controller.observe_time(ElapsedTick(1)).unwrap();
    controller.propose(1, spec(0), &snapshot()).unwrap();
    let session = controller.begin_review(1, 17, [3; 32], &snapshot()).unwrap();
    let anchor = session.replay_anchor();
    let archived = controller.apply_review_archived(finish(session, [Some(Verdict::Allow); 2]), &snapshot()).unwrap();
    let replayed = archived.verify(&anchor).unwrap();
    assert_eq!(replayed.tally().permit_weight, 4);
    assert_eq!(replayed.tally().admitted_cohort_weights["a"], 4);
    assert_eq!(replayed.decision().consequence, Consequence::Continue);
}

#[test]
fn rejected_reveals_do_not_pollute_the_accepted_transcript() {
    let mut controller = controller();
    controller.propose(1, spec(0), &snapshot()).unwrap();
    let mut session = controller.begin_review(1, 17, [3; 32], &snapshot()).unwrap();
    let anchor = session.replay_anchor();
    for member in ["alice", "bob"] {
        let value = session.commitment(member, Verdict::Allow, b"salt").unwrap();
        session.commit(member, value).unwrap();
    }
    session.open_reveals().unwrap();
    assert_eq!(session.reveal("alice", Verdict::Deny, b"salt"), Err(Error::Binding));
    for member in ["alice", "bob"] { session.reveal(member, Verdict::Allow, b"salt").unwrap(); }
    assert_eq!(session.reveal("alice", Verdict::Allow, b"salt"), Err(Error::Duplicate));
    let review = session.finish().unwrap();
    let archive = review.replay_archive();
    assert_eq!(archive.transcript.reveals.len(), 2);
    archive.verify(&anchor).unwrap();
}
