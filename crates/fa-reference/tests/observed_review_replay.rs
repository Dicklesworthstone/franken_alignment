//! Public full-input replay with original congress and authority controls.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;

use fa_reference::action::{ActionSpec, ElapsedTick, VERSION};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{ObservedSession, OversightBroker, ReviewWindow};
use fa_reference::action::consequence::oversight::replay::{ObservedDecisionArchive, ObservedReviewAnchor};
use fa_reference::reducer::MAX_VOTES;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::rc::Rc;

fn setup() -> (OversightBroker, ObservedSession, ObservedReviewAnchor) {
    let profile = fixture::profile();
    let d = profile.delivery;
    let mut endpoint = PublicationEndpoint::new(d.target, d.initial_payload,
        d.retention_ticks, d.max_deliveries).unwrap();
    let mut broker = OversightBroker::new(ControllerConfig {
        scope: d.scope, total: d.total, max_attempts: d.max_attempts,
        actor: d.actor, suspend_at_incident: d.suspend_at_incident,
        policy: d.policy, congress: d.congress,
        narrowed_targets: TargetCeiling::new(&d.narrowed_targets).unwrap(),
    }, &mut endpoint, profile.committee).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    let action = broker.propose(1, ActionSpec {
        version: VERSION, scope: d.scope, target: Some(d.target), payload: b"publish".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: broker.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16,
    }, &fixture::snapshot()).unwrap().action;
    let inputs = fixture::inputs(&action, b"complete evidence, including its final paragraph");
    broker.record_inputs(1, 0, inputs).unwrap();
    let session = broker.begin_review(1, 101, fixture::ROOT,
        ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
        &fixture::snapshot()).unwrap();
    let anchor = session.replay_anchor().unwrap();
    (broker, session, anchor)
}

fn cast(session: &mut ObservedSession, verdict: Verdict) {
    for member in fixture::MEMBERS {
        let commitment = session.commitment(member, verdict, &fixture::salt(member)).unwrap();
        session.commit(member, commitment, ElapsedTick(2)).unwrap();
    }
    session.open_reveals(ElapsedTick(3)).unwrap();
    for member in fixture::MEMBERS {
        session.reveal(member, verdict, &fixture::salt(member), ElapsedTick(4)).unwrap();
    }
}

fn archived() -> (ObservedReviewAnchor, ObservedDecisionArchive) {
    let (_, mut session, anchor) = setup();
    cast(&mut session, Verdict::Allow);
    (anchor, session.finish(ElapsedTick(4)).unwrap().replay_archive())
}

#[test]
fn full_input_archive_replays_and_binds_the_original_application_without_issuing_rights() {
    let (mut broker, mut session, anchor) = setup();
    cast(&mut session, Verdict::Allow);
    let review = session.finish(ElapsedTick(4)).unwrap();
    let before = broker.inspect();
    let archive = review.replay_archive();
    assert_eq!(archive.verify(&anchor).unwrap().decision().consequence, Consequence::Continue);
    assert_eq!(broker.inspect(), before);
    assert_eq!(archive.commit_times, vec![ElapsedTick(2); 2]);
    assert_eq!(archive.reveal_times, vec![ElapsedTick(4); 2]);
    assert_eq!(archive.reveals_opened_at, ElapsedTick(3));
    assert_eq!(session.replay_anchor(), Err(Error::WrongState));
    broker.observe_time(ElapsedTick(4)).unwrap();
    let receipt = broker.apply_review(review, Some(anchor.inputs.as_ref()), &fixture::snapshot()).unwrap();
    assert_eq!(archive.verify_receipt(&anchor, &receipt).unwrap().decision(),
        &receipt.policy.control.decision);
    assert_eq!(broker.inspect().ledger.reserved, 0);
    assert_eq!(broker.inspect().ledger.charged, 0);
    let mut changed_receipt = receipt;
    changed_receipt.input_revision += 1;
    assert_eq!(archive.verify_receipt(&anchor, &changed_receipt), Err(Error::Binding));
}

#[test]
fn a_valid_alternate_helper_packet_cannot_replace_the_independently_retained_view() {
    let (anchor, archive) = archived();
    assert!(archive.verify(&anchor).is_ok());
    let mut changed = archive.clone();
    changed.inputs = Rc::new(fixture::inputs(&anchor.policy.action, b"different decisive paragraph"));
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = archive.clone(); changed.input_revision += 1;
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = archive.clone(); changed.window.commit_by = ElapsedTick(6);
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
    let mut changed = archive; changed.started_at = ElapsedTick(0);
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
}

#[test]
fn accepted_times_obey_half_open_deadlines_and_complete_phase_order() {
    let (anchor, archive) = archived();
    for changed in [
        { let mut a = archive.clone(); a.commit_times[0] = ElapsedTick(5); a },
        { let mut a = archive.clone(); a.commit_times[0] = ElapsedTick(3); a },
        { let mut a = archive.clone(); a.reveals_opened_at = ElapsedTick(1); a },
        { let mut a = archive.clone(); a.reveal_times[0] = ElapsedTick(2); a },
        { let mut a = archive.clone(); a.reveal_times[0] = ElapsedTick(8); a },
        { let mut a = archive.clone(); a.completed_at = ElapsedTick(3); a },
    ] { assert_eq!(changed.verify(&anchor), Err(Error::Stale)); }
    let mut missing = archive; missing.commit_times.pop();
    assert_eq!(missing.verify(&anchor), Err(Error::Binding));
}

#[test]
fn a_missing_specialist_remains_missing_and_cannot_be_finished_before_expiry() {
    let (_, mut session, anchor) = setup();
    let commitment = session.commitment("alpha", Verdict::Allow, &fixture::salt("alpha")).unwrap();
    session.commit("alpha", commitment, ElapsedTick(2)).unwrap();
    assert_eq!(session.open_reveals(ElapsedTick(3)), Err(Error::Incomplete));
    session.open_reveals(ElapsedTick(5)).unwrap();
    session.reveal("alpha", Verdict::Allow, &fixture::salt("alpha"), ElapsedTick(6)).unwrap();
    assert!(matches!(session.finish(ElapsedTick(7)), Err(Error::Incomplete)));
    let archive = session.finish(ElapsedTick(8)).unwrap().replay_archive();
    let replayed = archive.verify(&anchor).unwrap();
    assert_eq!(replayed.missing(), &["beta".to_owned()]);
    assert_eq!(replayed.decision().consequence, Consequence::HoldEffect);
    let mut early_open = archive.clone(); early_open.reveals_opened_at = ElapsedTick(4);
    assert_eq!(early_open.verify(&anchor), Err(Error::Incomplete));
    let mut early_finish = archive; early_finish.completed_at = ElapsedTick(7);
    assert_eq!(early_finish.verify(&anchor), Err(Error::Incomplete));
}

#[test]
fn automatic_timeout_opening_and_all_missing_votes_have_a_replayable_history() {
    let (_, mut session, anchor) = setup();
    let archive = session.finish(ElapsedTick(8)).unwrap().replay_archive();
    assert!(archive.commit_times.is_empty()); assert!(archive.reveal_times.is_empty());
    assert_eq!(archive.reveals_opened_at, ElapsedTick(8));
    assert_eq!(archive.verify(&anchor).unwrap().missing().len(), 2);
}

#[test]
fn rejected_vote_attempts_do_not_create_fictitious_accepted_timestamps() {
    let (_, mut session, anchor) = setup();
    for member in fixture::MEMBERS {
        let commitment = session.commitment(member, Verdict::Allow, &fixture::salt(member)).unwrap();
        session.commit(member, commitment, ElapsedTick(2)).unwrap();
    }
    let duplicate = session.commitment("alpha", Verdict::Allow, &fixture::salt("alpha")).unwrap();
    assert_eq!(session.commit("alpha", duplicate, ElapsedTick(2)), Err(Error::Duplicate));
    session.open_reveals(ElapsedTick(3)).unwrap();
    assert_eq!(session.reveal("alpha", Verdict::Allow, b"wrong salt", ElapsedTick(3)), Err(Error::Binding));
    for member in fixture::MEMBERS {
        session.reveal(member, Verdict::Allow, &fixture::salt(member), ElapsedTick(4)).unwrap();
    }
    let archive = session.finish(ElapsedTick(4)).unwrap().replay_archive();
    assert_eq!(archive.commit_times.len(), 2); assert_eq!(archive.reveal_times.len(), 2);
    assert!(archive.verify(&anchor).is_ok());
}

#[test]
fn historical_replay_does_not_extend_the_live_action_deadline() {
    let (mut broker, mut session, anchor) = setup();
    cast(&mut session, Verdict::Allow);
    let review = session.finish(ElapsedTick(101)).unwrap();
    let archive = review.replay_archive();
    assert_eq!(archive.verify(&anchor).unwrap().decision().consequence, Consequence::Continue);
    broker.observe_time(ElapsedTick(101)).unwrap();
    assert!(broker.apply_review(review, Some(anchor.inputs.as_ref()), &fixture::snapshot()).is_err());
    assert_eq!(broker.inspect().ledger.reserved, 0);
    assert_eq!(broker.inspect().ledger.charged, 0);
}

#[test]
fn archive_bounds_version_and_original_vote_verification_remain_mandatory() {
    let (anchor, archive) = archived();
    let mut oversized = archive.clone(); oversized.commit_times = vec![ElapsedTick(2); MAX_VOTES + 1];
    assert_eq!(oversized.verify(&anchor), Err(Error::Limit));
    let mut version = archive.clone(); version.version += 1;
    assert_eq!(version.verify(&anchor), Err(Error::InvalidInput));
    let mut changed = archive; changed.policy.transcript.reveals[0].verdict = Verdict::Deny;
    assert_eq!(changed.verify(&anchor), Err(Error::Binding));
}
