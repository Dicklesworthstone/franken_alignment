//! Worker response source is distinct from the actor and the owning congress.
//! These tests exercise reference semantics; no trained helper is claimed.
#[path = "support/helper_workers.rs"]
mod support;

use support::{Fixture, snapshot};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::{ObservedSession, ReviewWindow};
use fa_reference::action::consequence::oversight::helper_workers::{
    HelperFailure, HelperLimits, HelperPhase, HelperPort, HelperRound, MAX_WORKER_SALT_BYTES,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;

fn workers(session: ObservedSession) -> (HelperRound, BTreeMap<String, HelperPort>) {
    HelperRound::new(session, HelperLimits::default()).unwrap()
}
fn commit_all(ports: &BTreeMap<String, HelperPort>, verdict: Verdict) {
    for port in ports.values() {
        let digest = port.request().commitment(verdict, b"salt").unwrap();
        port.submit_commitment(digest).unwrap();
    }
}
fn reveal_all(ports: &BTreeMap<String, HelperPort>, verdict: Verdict) {
    for port in ports.values() { port.reveal(verdict, b"salt").unwrap(); }
}

#[test]
fn independent_worker_responses_reach_the_original_permit_and_publication() {
    let mut f = Fixture::new();
    let (mut round, ports) = workers(f.start(11));
    assert_eq!(ports.len(), 2);
    for (name, port) in &ports {
        assert_eq!(port.request().view(), &f.inputs.views()[name]);
        assert!(port.request().view().actual_input().submitted_bytes().windows(7).any(|bytes| bytes == b"publish"));
    }
    commit_all(&ports, Verdict::Allow);
    assert!(f.broker.authorize(1, Some(&f.inputs), &snapshot()).is_err());
    round.advance(ElapsedTick(1)).unwrap();
    reveal_all(&ports, Verdict::Allow);
    let reviewed = round.finish(ElapsedTick(1)).unwrap();
    assert_eq!(reviewed.decision().consequence, Consequence::Continue);
    assert_eq!(reviewed.inputs(), &f.inputs);
    assert!(reviewed.missing().is_empty());
    f.broker.apply_review(reviewed, Some(&f.inputs), &snapshot()).unwrap();
    let permit = f.broker.authorize(1, Some(&f.inputs), &snapshot()).unwrap();
    let envelope = f.broker.dispatch(&permit, &f.action, Some(&f.inputs), &snapshot()).unwrap();
    f.broker.accept_receipt(f.endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(f.endpoint.payload(), b"publish");
    assert_eq!(f.endpoint.execution_count(), 1);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert!(f.broker.dispatch(&permit, &f.action, Some(&f.inputs), &snapshot()).is_err());
}

#[test]
fn all_commitments_close_before_any_worker_can_reveal() {
    let mut f = Fixture::new();
    let (mut round, ports) = workers(f.start(11));
    let alpha = &ports["alpha"];
    assert_eq!(alpha.reveal(Verdict::Allow, b"salt"), Err(Error::WrongState));
    alpha.submit_commitment(alpha.request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
    assert_eq!(alpha.submit_commitment(0), Err(Error::WrongState));
    round.advance(ElapsedTick(1)).unwrap();
    assert_eq!(alpha.phase(), HelperPhase::AwaitReveal);
    assert_eq!(alpha.reveal(Verdict::Allow, b"salt"), Err(Error::WrongState));
    assert_eq!(round.finish(ElapsedTick(1)).unwrap_err(), Error::Incomplete);
    let beta = &ports["beta"];
    beta.submit_commitment(beta.request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
    round.advance(ElapsedTick(2)).unwrap();
    assert_eq!(alpha.phase(), HelperPhase::ReadyReveal);
    reveal_all(&ports, Verdict::Allow);
    assert_eq!(round.finish(ElapsedTick(2)).unwrap().decision().consequence, Consequence::Continue);
    assert_eq!(round.finish(ElapsedTick(2)).unwrap_err(), Error::WrongState);
}

#[test]
fn missing_worker_is_not_removed_from_the_reducer_denominator() {
    let mut f = Fixture::new();
    let (mut round, ports) = workers(f.start(11));
    let alpha = &ports["alpha"];
    alpha.submit_commitment(alpha.request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
    round.advance(ElapsedTick(1)).unwrap();
    round.advance(ElapsedTick(5)).unwrap();
    alpha.reveal(Verdict::Allow, b"salt").unwrap();
    assert_eq!(round.finish(ElapsedTick(5)).unwrap_err(), Error::Incomplete);
    let review = round.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
    assert_eq!(review.missing(), &["beta".to_owned()]);
    assert!(review.abstained().is_empty());
    assert_eq!(round.statuses()["beta"].failure, Some(HelperFailure::CommitDeadline));
    f.broker.observe_time(ElapsedTick(10)).unwrap();
    f.broker.apply_review(review, Some(&f.inputs), &snapshot()).unwrap();
    assert!(f.broker.authorize(1, Some(&f.inputs), &snapshot()).is_err());
    assert_eq!(f.endpoint.execution_count(), 0);
    assert_eq!(f.broker.inspect().ledger.available, 100);
}

#[test]
fn mismatched_reveal_cannot_be_replaced_by_a_more_convenient_vote() {
    let mut f = Fixture::new();
    let (mut round, ports) = workers(f.start(11));
    commit_all(&ports, Verdict::Allow);
    round.advance(ElapsedTick(1)).unwrap();
    ports["alpha"].reveal(Verdict::Allow, b"wrong salt").unwrap();
    ports["beta"].reveal(Verdict::Allow, b"salt").unwrap();
    round.advance(ElapsedTick(2)).unwrap();
    assert_eq!(round.statuses()["alpha"].failure, Some(HelperFailure::Rejected(Error::Binding)));
    assert_eq!(ports["alpha"].reveal(Verdict::Allow, b"salt"), Err(Error::WrongState));
    let review = round.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing(), &["alpha".to_owned()]);
    assert_ne!(review.decision().consequence, Consequence::Continue);
}

#[test]
fn queued_replies_cannot_backdate_their_receipt_at_exact_deadlines() {
    let mut f = Fixture::new();
    let (mut round, ports) = workers(f.start(11));
    commit_all(&ports, Verdict::Allow);
    round.advance(ElapsedTick(5)).unwrap();
    assert!(round.statuses().values().all(|s| s.failure == Some(HelperFailure::CommitDeadline)));
    assert_eq!(round.finish(ElapsedTick(10)).unwrap().missing().len(), 2);
    let mut f = Fixture::new();
    let (mut round, ports) = workers(f.start(11));
    commit_all(&ports, Verdict::Allow);
    round.advance(ElapsedTick(4)).unwrap();
    reveal_all(&ports, Verdict::Allow);
    assert_eq!(round.advance(ElapsedTick(3)), Err(Error::Stale));
    assert_eq!(ports["alpha"].phase(), HelperPhase::RevealQueued);
    let review = round.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing().len(), 2);
    assert!(round.statuses().values().all(|s| s.failure == Some(HelperFailure::RevealDeadline)));
}

#[test]
fn disconnection_and_coordinator_loss_never_become_abstention_or_permission() {
    let mut f = Fixture::new();
    let (mut round, mut ports) = workers(f.start(11));
    commit_all(&ports, Verdict::Allow);
    round.advance(ElapsedTick(1)).unwrap();
    drop(ports.remove("alpha").unwrap());
    ports["beta"].reveal(Verdict::Allow, b"salt").unwrap();
    let review = round.finish(ElapsedTick(10)).unwrap();
    assert_eq!(review.missing(), &["alpha".to_owned()]);
    assert!(review.abstained().is_empty());
    assert_eq!(round.statuses()["alpha"].failure, Some(HelperFailure::Disconnected));
    let mut f = Fixture::new();
    let (round, ports) = workers(f.start(11));
    drop(round);
    assert_eq!(ports["alpha"].phase(), HelperPhase::Closed);
    assert_eq!(ports["alpha"].submit_commitment(1), Err(Error::WrongState));
    assert!(f.broker.authorize(1, Some(&f.inputs), &snapshot()).is_err());
}

#[test]
fn helper_inputs_and_debug_do_not_contain_peer_questions_or_private_witnesses() {
    let mut f = Fixture::new();
    let (_round, ports) = workers(f.start(11));
    for (member, peer) in [("alpha", "beta"), ("beta", "alpha")] {
        let port = &ports[member];
        let bytes = port.request().view().actual_input().submitted_bytes();
        let own = format!("{member}-private-question");
        let other = format!("{peer}-private-question");
        assert!(bytes.windows(own.len()).any(|part| part == own.as_bytes()));
        assert!(!bytes.windows(other.len()).any(|part| part == other.as_bytes()));
        let printed = format!("{port:?}");
        assert!(!printed.contains(peer));
        assert!(!printed.contains("private-question"));
        assert!(!printed.contains("profile"));
    }
}

#[test]
fn another_round_or_another_member_cannot_supply_the_reveal_basis() {
    for wrong_round in [false, true] {
        let mut f = Fixture::new();
        let (mut round, ports) = workers(f.start(11));
        let wrong = fa_reference::round::commitment(
            if wrong_round { 12 } else { 11 }, if wrong_round { "alpha" } else { "beta" },
            &[8; 32], Verdict::Allow, b"salt",
        ).unwrap();
        ports["alpha"].submit_commitment(wrong).unwrap();
        ports["beta"].submit_commitment(ports["beta"].request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
        round.advance(ElapsedTick(1)).unwrap();
        reveal_all(&ports, Verdict::Allow);
        assert_eq!(round.finish(ElapsedTick(10)).unwrap().missing().len(), 2);
        // Late replies are already refused; separately verify the binding failure below.
    }
    let mut f = Fixture::new();
    let (mut round, ports) = workers(f.start(11));
    ports["alpha"].submit_commitment(ports["beta"].request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
    ports["beta"].submit_commitment(ports["beta"].request().commitment(Verdict::Allow, b"salt").unwrap()).unwrap();
    round.advance(ElapsedTick(1)).unwrap();
    reveal_all(&ports, Verdict::Allow);
    round.advance(ElapsedTick(2)).unwrap();
    assert_eq!(round.statuses()["alpha"].failure, Some(HelperFailure::Rejected(Error::Binding)));
    assert!(round.statuses()["beta"].revealed);
}

#[test]
fn constructor_and_reply_limits_pair_exact_acceptance_with_one_over_refusal() {
    let mut f = Fixture::new();
    let bytes = f.inputs.logical_bytes();
    assert!(matches!(HelperRound::new(f.start(11), HelperLimits { members: 1, input_bytes: bytes, salt_bytes: 3 }), Err(Error::Limit)));
    assert!(matches!(HelperRound::new(f.start(12), HelperLimits { members: 2, input_bytes: bytes - 1, salt_bytes: 3 }), Err(Error::Limit)));
    assert!(matches!(HelperRound::new(f.start(13), HelperLimits { members: 2, input_bytes: bytes, salt_bytes: MAX_WORKER_SALT_BYTES + 1 }), Err(Error::Limit)));
    let (mut round, ports) = HelperRound::new(f.start(14), HelperLimits { members: 2, input_bytes: bytes, salt_bytes: 3 }).unwrap();
    for port in ports.values() { port.submit_commitment(port.request().commitment(Verdict::Allow, b"abc").unwrap()).unwrap(); }
    round.advance(ElapsedTick(1)).unwrap();
    for port in ports.values() {
        assert_eq!(port.reveal(Verdict::Allow, b"abcd"), Err(Error::Limit));
        port.reveal(Verdict::Allow, b"abc").unwrap();
    }
    assert_eq!(round.finish(ElapsedTick(1)).unwrap().decision().consequence, Consequence::Continue);
}

#[test]
fn caller_voted_sessions_cannot_be_reopened_as_independent_workers() {
    let mut f = Fixture::new();
    let mut session = f.start(11);
    let commitment = session.commitment("alpha", Verdict::Allow, b"salt").unwrap();
    session.commit("alpha", commitment, ElapsedTick(1)).unwrap();
    assert!(matches!(HelperRound::new(session, HelperLimits::default()), Err(Error::WrongState)));
}

#[test]
fn completed_worker_reviews_still_recheck_input_revision_and_current_policy() {
    let mut f = Fixture::new();
    let (mut round, ports) = workers(f.start(11));
    commit_all(&ports, Verdict::Allow);
    round.advance(ElapsedTick(1)).unwrap();
    reveal_all(&ports, Verdict::Allow);
    let review = round.finish(ElapsedTick(1)).unwrap();
    f.broker.inputs_unavailable(1, 1).unwrap();
    f.broker.record_inputs(1, 2, f.inputs.clone()).unwrap();
    assert!(f.broker.apply_review(review, Some(&f.inputs), &snapshot()).is_err());
    assert_eq!(f.broker.inspect().ledger.available, 100);
    assert_eq!(f.endpoint.execution_count(), 0);
}

#[test]
fn exact_disqualifier_dominates_every_worker_allow_vote() {
    let mut f = Fixture::new();
    let mut changed = snapshot();
    changed.values.insert(7, vec![8]);
    let session = f.broker.begin_review(1, 11, [8; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
    }, &changed).unwrap();
    let (mut round, ports) = workers(session);
    commit_all(&ports, Verdict::Allow);
    round.advance(ElapsedTick(1)).unwrap();
    reveal_all(&ports, Verdict::Allow);
    let review = round.finish(ElapsedTick(1)).unwrap();
    assert_eq!(review.decision().consequence, Consequence::Deny);
    f.broker.apply_review(review, Some(&f.inputs), &changed).unwrap();
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Denied);
    assert_eq!(f.endpoint.payload(), b"old");
}
