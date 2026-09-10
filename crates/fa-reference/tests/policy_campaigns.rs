//! Actual oversight history -> separate governance -> fenced policy replacement.
//! These are in-memory reference transitions, not authenticated live deployment.

#[path = "support/stream_fixture.rs"]
mod support;
use support::*;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate, Truth};
use fa_reference::action::consequence::gate::containment::ResetRequest;
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::oversight::{OversightBroker, ReviewWindow};
use fa_reference::action::consequence::oversight::policy_governance::{CampaignDisposition, PolicyCampaignReview, PolicyGovernor};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::consequence::policy_campaign::{PolicyDelta, ReplayCaseId, ReplayLimits};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::{Error, ReadWitness};

fn enable(broker: &mut OversightBroker) -> PolicyGovernor {
    broker.enable_policy_campaigns(ReplayLimits { cases: 64, input_bytes: 1_048_576 }, 16).unwrap()
}
fn campaign(broker: &mut OversightBroker, id: u64, next: Policy) -> PolicyCampaignReview {
    let state = broker.inspect();
    broker.request_policy_campaign(id, state.sequence, state.ledger.epoch, next).unwrap()
}
fn relaxed(generation: u64) -> Policy {
    Policy::new(generation, vec![Predicate::PayloadAtMost(65_536)]).unwrap()
}

#[test]
fn approved_complete_campaign_replaces_policy_and_new_work_publishes() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let governor = enable(&mut broker);
    let (old_action, old_input, old_permit) = ready(&mut broker, &contracts, 1, Some("old"));
    let before = broker.inspect();
    assert_eq!(broker.replace_policy(before.sequence, before.ledger.epoch, policy(2)), Err(Error::Incomplete));
    assert_eq!(broker.inspect(), before);
    let review = campaign(&mut broker, 10, policy(2));
    assert_eq!(review.report().cases().len(), 2);
    let key = governor.approve(&review, false).unwrap();
    let promoted = broker.promote_policy(&key).unwrap();
    assert_eq!(promoted.change.cancelled, vec![1]);
    assert_eq!(promoted.change.refunded_units, old_action.spec().units);
    assert_eq!(review.disposition(), CampaignDisposition::Promoted);
    assert_eq!(broker.policy_campaign(10).unwrap().disposition(), CampaignDisposition::Promoted);
    assert_eq!(broker.promote_policy(&key).unwrap_err(), Error::WrongState);
    assert!(broker.dispatch(&old_permit, &old_action, Some(&old_input), &snapshot()).is_err());
    publish(&mut broker, &mut endpoint, &contracts, 2, Some("new"));
    assert_eq!(endpoint.payload(), b"new");
    assert_eq!(broker.policy_promotions().unwrap(), &[promoted]);
    conserved(&broker);
}

#[test]
fn denied_proposals_are_mandatory_and_relaxations_need_explicit_approval() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let governor = enable(&mut broker);
    let mut bad = snapshot(); bad.values.insert(7, vec![8]);
    let spec = broker.stream_message_spec("denied", ElapsedTick(100)).unwrap();
    assert_eq!(broker.propose(1, spec, &bad).unwrap().state, ActionState::Denied);
    let review = campaign(&mut broker, 10, relaxed(2));
    assert_eq!(review.report().newly_reviewable(), vec![ReplayCaseId::Proposal(1)]);
    assert_eq!(governor.approve(&review, false).unwrap_err(), Error::Binding);
    assert_eq!(review.disposition(), CampaignDisposition::Pending);
    let key = governor.approve(&review, true).unwrap();
    let promoted = broker.promote_policy(&key).unwrap();
    assert!(promoted.accepted_relaxations);
    assert!(promoted.change.cancelled.is_empty());
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Denied);
    publish(&mut broker, &mut endpoint, &contracts, 2, Some("fresh review"));
    assert_eq!(endpoint.execution_count(), 1);
    conserved(&broker);
}

#[test]
fn an_unobserved_candidate_branch_cannot_be_approved_even_with_relaxation_consent() {
    let (mut broker, _, contracts) = fixture();
    let governor = enable(&mut broker);
    let (_, _, _) = ready(&mut broker, &contracts, 1, Some("held"));
    let review = campaign(&mut broker, 10, Policy::new(2, vec![
        Predicate::PayloadAtMost(65_536), Predicate::Absent { key: 99 }, Predicate::Any(vec![0, 1]),
    ]).unwrap());
    assert!(review.report().requires_shadow());
    assert!(review.report().cases().iter().all(|case| case.missing_nodes() == &[1]));
    let before = broker.inspect();
    assert_eq!(governor.approve(&review, true).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect(), before);
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, TOTAL);
}

#[test]
fn a_new_exact_denial_invalidates_approval_without_advancing_the_control_sequence() {
    let (mut broker, _, contracts) = fixture();
    let governor = enable(&mut broker);
    ready(&mut broker, &contracts, 1, Some("first"));
    let review = campaign(&mut broker, 10, policy(2));
    let key = governor.approve(&review, false).unwrap();
    let sequence = broker.inspect().sequence;
    let mut bad = snapshot(); bad.values.insert(7, vec![8]);
    let spec = broker.stream_message_spec("second", ElapsedTick(100)).unwrap();
    assert_eq!(broker.propose(2, spec, &bad).unwrap().state, ActionState::Denied);
    assert_eq!(broker.inspect().sequence, sequence);
    let before = broker.inspect();
    assert_eq!(broker.promote_policy(&key).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect(), before);
    let replacement = campaign(&mut broker, 11, policy(2));
    assert_eq!(replacement.report().cases().len(), 3);
    let key = governor.approve(&replacement, false).unwrap();
    broker.promote_policy(&key).unwrap();
    conserved(&broker);
}

#[test]
fn a_later_review_uses_its_own_violating_observation_not_the_earlier_passing_one() {
    let (mut broker, _, contracts) = fixture();
    let governor = enable(&mut broker);
    prepare(&mut broker, &contracts, 1, Some("first"));
    let mut bad = snapshot(); bad.values.insert(7, vec![8]);
    let now = ElapsedTick(1);
    let mut session = broker.begin_review(1, 11, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
    }, &bad).unwrap();
    let vote = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", vote, now).unwrap(); session.open_reveals(now).unwrap();
    session.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
    broker.apply_review(session.finish(now).unwrap(), None, &bad).unwrap();
    let review = campaign(&mut broker, 10, relaxed(2));
    assert_eq!(review.report().cases().len(), 2);
    assert_eq!(review.report().cases()[0].delta(), PolicyDelta::Unchanged);
    let later = &review.report().cases()[1];
    assert_eq!(later.delta(), PolicyDelta::NewlyReviewable);
    assert_eq!(later.original().result(), Truth::Violated);
    assert_eq!(later.original().witnesses(), &[ReadWitness::Exact { key: 7, value: Some(vec![8]) }]);
    assert_eq!(governor.approve(&review, false).unwrap_err(), Error::Binding);
}

#[test]
fn changed_control_or_opened_round_prevents_stale_promotion() {
    let (mut broker, _, contracts) = fixture();
    let governor = enable(&mut broker);
    let (_, inputs, _) = ready(&mut broker, &contracts, 1, Some("first"));
    let first = campaign(&mut broker, 10, policy(2));
    let key = governor.approve(&first, false).unwrap();
    let session = broker.begin_review(1, 90, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
    }, &snapshot()).unwrap();
    drop(session);
    assert_eq!(broker.promote_policy(&key).unwrap_err(), Error::Stale);
    let second = campaign(&mut broker, 11, policy(2));
    let key = governor.approve(&second, false).unwrap();
    review(&mut broker, 1, 91, &inputs, Verdict::Allow);
    assert_eq!(broker.promote_policy(&key).unwrap_err(), Error::Stale);
    conserved(&broker);
}

#[test]
fn foreign_governors_keys_and_revoked_approvals_cannot_change_policy() {
    let (mut first, _, contracts) = fixture();
    let first_governor = enable(&mut first);
    let (mut second, _, _) = fixture();
    let second_governor = enable(&mut second);
    prepare(&mut first, &contracts, 1, Some("first"));
    prepare(&mut second, &contracts, 1, Some("first"));
    let review = campaign(&mut first, 10, policy(2));
    assert_eq!(second_governor.approve(&review, false).unwrap_err(), Error::Binding);
    let key = first_governor.approve(&review, false).unwrap();
    let before = second.inspect();
    assert_eq!(second.promote_policy(&key).unwrap_err(), Error::Binding);
    assert_eq!(second.inspect(), before);
    first_governor.revoke(&review).unwrap();
    first_governor.revoke(&review).unwrap();
    assert_eq!(first.promote_policy(&key).unwrap_err(), Error::WrongState);
    assert_eq!(review.disposition(), CampaignDisposition::Revoked);
    let state = first.inspect();
    assert_eq!(first.request_policy_campaign(12, state.sequence, state.ledger.epoch, policy(2)).unwrap_err(), Error::Duplicate);
}

#[test]
fn unknown_stream_liability_survives_policy_promotion_and_still_reconciles() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let governor = enable(&mut broker);
    let (action, input, permit) = ready(&mut broker, &contracts, 1, Some("already sent"));
    let envelope = broker.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let charged = broker.inspect().ledger.charged;
    let review = campaign(&mut broker, 10, policy(2));
    let key = governor.approve(&review, false).unwrap();
    let change = broker.promote_policy(&key).unwrap();
    assert_eq!(change.change.refunded_units, 0);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(broker.inspect().ledger.charged, charged);
    assert_eq!(broker.stream_pending(), Some(1));
    broker.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    publish(&mut broker, &mut endpoint, &contracts, 2, Some(" later"));
    assert_eq!(endpoint.payload(), b"already sent later");
    conserved(&broker);
}

#[test]
fn policy_promotion_invalidates_preexisting_human_keys_but_fresh_two_key_work_runs() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let governor = enable(&mut broker);
    let human = broker.enable_human_review(HumanReviewPolicy {
        reviewer_id: 9, max_validity_ticks: 20, max_requests: 8,
    }).unwrap();
    let (action, input, permit) = ready(&mut broker, &contracts, 1, Some("old"));
    let request = broker.request_human_approval(1, 1, Some(&input), ElapsedTick(10)).unwrap();
    let old_human = human.approve(&request, ElapsedTick(1)).unwrap();
    let review = campaign(&mut broker, 10, policy(2));
    let key = governor.approve(&review, false).unwrap();
    broker.promote_policy(&key).unwrap();
    assert!(broker.dispatch_with_human(&permit, &old_human, &action, Some(&input), &snapshot()).is_err());
    assert_eq!(broker.human_status(1).unwrap().disposition, HumanDisposition::Approved);
    let (action, input, permit) = ready(&mut broker, &contracts, 2, Some("new"));
    let request = broker.request_human_approval(2, 2, Some(&input), ElapsedTick(10)).unwrap();
    let fresh = human.approve(&request, ElapsedTick(1)).unwrap();
    let envelope = broker.dispatch_with_human(&permit, &fresh, &action, Some(&input), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"new");
    conserved(&broker);
}

#[test]
fn reset_preserves_governance_and_history_without_resurrecting_an_approval() {
    let (mut broker, _, contracts) = fixture();
    let governor = enable(&mut broker);
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    ready(&mut broker, &contracts, 1, Some("old"));
    let review = campaign(&mut broker, 10, policy(2));
    let key = governor.approve(&review, false).unwrap();
    let state = broker.inspect();
    broker.reset(ResetRequest { checkpoint, expected_control_sequence: state.sequence,
        expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 99, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[target(1), target(2)]).unwrap(),
    }).unwrap();
    assert!(broker.policy_campaigns_required());
    assert_eq!(broker.promote_policy(&key).unwrap_err(), Error::Stale);
    let next = campaign(&mut broker, 11, policy(2));
    assert_eq!(next.report().cases().len(), 2);
    let key = governor.approve(&next, false).unwrap();
    broker.promote_policy(&key).unwrap();
    assert_eq!(broker.policy_campaign(10).unwrap().disposition(), CampaignDisposition::Approved);
    conserved(&broker);
}

#[test]
fn quotas_cannot_select_a_smaller_corpus_or_block_cancellation() {
    let (mut broker, _, contracts) = fixture();
    broker.enable_policy_campaigns(ReplayLimits { cases: 1, input_bytes: 1_048_576 }, 1).unwrap();
    ready(&mut broker, &contracts, 1, Some("reserved"));
    let state = broker.inspect();
    assert_eq!(broker.request_policy_campaign(10, state.sequence, state.ledger.epoch, policy(2)).unwrap_err(), Error::Limit);
    assert_eq!(broker.policy_campaign(10).unwrap_err(), Error::Missing);
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, TOTAL);
}

#[test]
fn campaign_slots_are_not_reset_and_enabled_mode_cannot_be_changed_after_work() {
    let (mut broker, _, contracts) = fixture();
    let governor = broker.enable_policy_campaigns(ReplayLimits { cases: 8, input_bytes: 1_048_576 }, 1).unwrap();
    assert_eq!(broker.enable_policy_campaigns(ReplayLimits { cases: 8, input_bytes: 1_048_576 }, 8).unwrap_err(), Error::Duplicate);
    prepare(&mut broker, &contracts, 1, Some("one"));
    let review = campaign(&mut broker, 10, policy(2));
    governor.reject(&review).unwrap(); governor.reject(&review).unwrap();
    let state = broker.inspect();
    assert_eq!(broker.request_policy_campaign(11, state.sequence, state.ledger.epoch, policy(3)).unwrap_err(), Error::Limit);
    broker.cancel(1).unwrap();
    let (mut legacy, _, contracts) = fixture();
    prepare(&mut legacy, &contracts, 1, Some("already started"));
    assert_eq!(legacy.enable_policy_campaigns(ReplayLimits { cases: 8, input_bytes: 1_048_576 }, 1).unwrap_err(), Error::WrongState);
}
