//! Independent client frames through original durable refinement and publication.
use super::*;
use fa_reference::action::consequence::oversight::learned_host::sidecar::workers::{LearnedWorkerRound, LearnedWorkerSchedule};
use fa_reference::action::consequence::delivery::persistent::observed::helpers::learned_sockets::sequence::FileLearnedSocketReview;

fn schedule(polls: usize) -> LearnedWorkerSchedule {
    LearnedWorkerSchedule { rounds: vec![
        LearnedWorkerRound { round: 101, evidence_root: [9; 32],
            window: ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) } },
        LearnedWorkerRound { round: 102, evidence_root: [8; 32],
            window: ReviewWindow { commit_by: ElapsedTick(40), reveal_by: ElapsedTick(50) } },
    ], helpers: HelperLimits::default(), polls }
}
fn sockets() -> (BTreeMap<u64, BTreeMap<String, UnixStream>>, UnixStream, UnixStream) {
    let (first, a) = launch(); let (second, b) = launch();
    (BTreeMap::from([(101, first.streams), (102, second.streams)]), a, b)
}
fn drive_to_next(review: &mut FileLearnedSocketReview, host: &mut Host,
    client: &mut HelperClient<UnixStream>, verdict: Verdict) {
    let start = review.round();
    for _ in 0..256 {
        review.advance(host, review.revision(), ElapsedTick(1), snapshot()).unwrap();
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference { client.respond(verdict, &[11; 32]).unwrap(); }
        if review.status() != LearnedSocketStatus::Running || review.round() != start { return; }
    }
    panic!("frozen socket schedule did not complete current round");
}
fn drain_closed(peer: &mut UnixStream) {
    let mut scratch = [0; 4096];
    for _ in 0..1024 {
        match peer.read(&mut scratch) {
            Ok(0) => return,
            Ok(_) => {},
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => return,
            other => panic!("expected closed peer, observed {other:?}"),
        }
    }
    panic!("socket did not close within bounded drain");
}

#[test]
fn abstention_buys_original_residual_then_new_external_review_reaches_two_keys() {
    let (dir, mut h, reviewer, action, plan) = setup();
    let coarse = h.current_learned_sidecar(&plan).unwrap().clone();
    let numerical = h.learned_generation_inspection().unwrap().numerical;
    let (streams, first, mut second) = sockets(); let mut a = client(first, &action);
    let mut review = h.begin_learned_socket_review(h.revision(), plan, schedule(256), streams, snapshot()).unwrap();
    drive_to_next(&mut review, &mut h, &mut a, Verdict::Abstain);
    assert_eq!(review.round(), 102); assert_eq!(review.history().len(), 1);
    assert!(matches!(review.history()[0], FileLearnedSidecarFinish::Refined { input_revision: 2, .. }));
    assert_eq!(h.input_revision(1).unwrap(), 2);
    assert_ne!(review.input(), &coarse); assert!(review.input().logical_bytes() > coarse.logical_bytes());
    assert!(review.records()[&101].completed); assert_eq!(review.records()[&102].polls, 0);
    // The transition creates the next original round but sends it no bytes.
    assert!(matches!(second.read(&mut [0; 1]), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
    let mut b = client(second, &action);
    drive_to_next(&mut review, &mut h, &mut b, Verdict::Allow);
    assert_eq!(review.status(), LearnedSocketStatus::Finished); assert_eq!(review.history().len(), 2);
    assert_eq!(a.input().unwrap().actual_input(), coarse.views()["reviewer"].actual_input());
    assert_eq!(b.input().unwrap().actual_input(), review.input().views()["reviewer"].actual_input());
    assert_eq!(h.learned_generation_inspection().unwrap().numerical, numerical);
    let input = review.input();
    let automatic = h.authorize(h.revision(), 1, input, snapshot()).unwrap();
    let request = h.request_human_approval(h.revision(), 1001, 1, input, ElapsedTick(40)).unwrap();
    let rev = h.revision(); let human = reviewer.approve(&mut h, rev, &request).unwrap();
    h.dispatch(h.revision(), &automatic, &human, &action, input, snapshot()).unwrap();
    h.publish_checked(h.revision(), 1, Some(input), snapshot(), ElapsedTick(2)).unwrap();
    h.reconcile(h.revision(), 1).unwrap();
    assert_eq!(h.inspect().executions, 1); assert_eq!(h.inspect().control.ledger.charged, 16);
    for (index, round) in [101, 102].into_iter().enumerate() {
        let saved = Host::read_learned_sidecar_outcome(dir.store(), &profile(), &config(), round).unwrap();
        assert_eq!(saved.result.archive(), review.history()[index].archive());
    }
}

#[test]
fn disconnected_member_remains_missing_and_does_not_spend_future_disclosure() {
    let (_dir, mut h, _, _, plan) = setup();
    let (streams, first, mut second) = sockets(); drop(first);
    let mut review = h.begin_learned_socket_review(h.revision(), plan, schedule(8), streams, snapshot()).unwrap();
    review.advance(&mut h, 0, ElapsedTick(1), snapshot()).unwrap();
    assert!(!review.records()[&101].wire_failures.is_empty());
    review.advance(&mut h, review.revision(), ElapsedTick(30), snapshot()).unwrap();
    assert_eq!(review.status(), LearnedSocketStatus::Finished); assert_eq!(review.history().len(), 1);
    assert!(matches!(review.history()[0], FileLearnedSidecarFinish::Applied { .. }));
    assert_eq!(h.input_revision(1).unwrap(), 1); assert_eq!(review.records()[&102].polls, 0);
    assert!(!review.records()[&101].wire_failures.is_empty()); drain_closed(&mut second);
    assert!(h.authorize(h.revision(), 1, review.input(), snapshot()).is_err());
}

#[test]
fn cancellation_and_poll_exhaustion_close_all_preprovisioned_peers() {
    for exhaust in [false, true] {
        let (dir, mut h, _, _, plan) = setup(); let (streams, mut a, mut b) = sockets();
        let mut review = h.begin_learned_socket_review(h.revision(), plan, schedule(1), streams, snapshot()).unwrap();
        review.advance(&mut h, 0, ElapsedTick(1), snapshot()).unwrap();
        let bytes = dir.bytes();
        if exhaust {
            assert_eq!(review.advance(&mut h, review.revision(), ElapsedTick(1), snapshot()).err(),
                Some(FileHelperSetupError::Journal(JournalError::Contract(Error::Limit))));
            assert_eq!(review.status(), LearnedSocketStatus::Failed);
        } else {
            review.cancel(review.revision()).unwrap(); assert_eq!(review.status(), LearnedSocketStatus::Cancelled);
        }
        assert_eq!(review.polls(), 1); assert_eq!(review.records()[&102].polls, 0);
        assert_eq!(dir.bytes(), bytes); assert!(review.history().is_empty());
        drain_closed(&mut a); drain_closed(&mut b);
        assert!(review.advance(&mut h, review.revision(), ElapsedTick(1), snapshot()).is_err());
        assert!(h.open_reveals(h.revision(), 101).is_err());
    }
}

#[test]
fn future_inventory_and_duplicate_ids_are_admitted_before_the_first_round() {
    for duplicate in [false, true] {
        let (dir, mut h, _, _, plan) = setup(); let (mut streams, mut a, mut b) = sockets();
        let mut selection = schedule(16);
        if duplicate { selection.rounds[1].round = 101; } else { streams.get_mut(&102).unwrap().clear(); }
        let before = dir.bytes(); let revision = h.revision();
        assert!(h.begin_learned_socket_review(revision, plan, selection, streams, snapshot()).is_err());
        assert_eq!(h.revision(), revision); assert_eq!(dir.bytes(), before);
        drain_closed(&mut a); drain_closed(&mut b);
    }
}

#[test]
fn richer_input_admission_failure_keeps_the_acknowledged_refinement_and_old_votes_stale() {
    let (_dir, mut h, _, action, plan) = setup();
    let coarse = h.current_learned_sidecar(&plan).unwrap().clone();
    let (streams, first, mut second) = sockets(); let mut a = client(first, &action);
    let mut selection = schedule(256); selection.helpers.input_bytes = coarse.logical_bytes();
    let mut review = h.begin_learned_socket_review(h.revision(), plan, selection, streams, snapshot()).unwrap();
    let mut failed = false;
    for _ in 0..256 {
        match review.advance(&mut h, review.revision(), ElapsedTick(1), snapshot()) {
            Ok(_) => {},
            Err(FileHelperSetupError::Journal(JournalError::Contract(Error::Limit))) => { failed = true; break; },
            Err(other) => panic!("unexpected failure {other:?}"),
        }
        a.step().unwrap(); if a.phase() == ClientPhase::NeedsInference { a.respond(Verdict::Abstain, &[11; 32]).unwrap(); }
    }
    assert!(failed); assert_eq!(review.status(), LearnedSocketStatus::Failed);
    assert_eq!(review.history().len(), 1); assert_eq!(h.input_revision(1).unwrap(), 2);
    assert!(matches!(h.learned_sidecar_outcome(101).unwrap().result, FileLearnedSidecarFinish::Refined { .. }));
    assert!(h.authorize(h.revision(), 1, &coarse, snapshot()).is_err()); drain_closed(&mut second);
    assert_eq!(h.inspect().executions, 0);
}

#[test]
fn source_loss_after_refinement_preserves_its_result_but_never_contacts_next_peer() {
    let (_dir, mut h, _, action, plan) = setup(); let (streams, first, mut second) = sockets();
    let mut a = client(first, &action);
    let mut review = h.begin_learned_socket_review(h.revision(), plan, schedule(256), streams, snapshot()).unwrap();
    drive_to_next(&mut review, &mut h, &mut a, Verdict::Abstain);
    assert_eq!(review.round(), 102); step(&mut h);
    assert!(review.advance(&mut h, review.revision(), ElapsedTick(1), snapshot()).is_err());
    assert_eq!(review.status(), LearnedSocketStatus::Failed);
    assert_eq!(review.history().len(), 1); assert_eq!(review.records()[&102].connection_steps, 0);
    assert_eq!(second.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(review.history()[0].archive(), h.learned_sidecar_outcome(101).unwrap().result.archive());
}

#[test]
fn a_clock_unwind_closes_active_and_future_sockets_and_poisoned_work_cannot_retry() {
    let (dir, mut h, _, _, plan) = setup(); let (streams, mut first, mut second) = sockets();
    let mut review = h.begin_learned_socket_review(h.revision(), plan, schedule(16), streams, snapshot()).unwrap();
    let before = dir.bytes(); let mut calls = 0;
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        review.advance_with_clock(&mut h, 0, || {
            calls += 1; assert!(calls < 2, "injected clock failure before first I/O"); ElapsedTick(1)
        }, snapshot())
    }));
    assert!(interrupted.is_err()); assert_eq!(review.status(), LearnedSocketStatus::Failed);
    assert_eq!(review.polls(), 1); assert_eq!(dir.bytes(), before);
    assert_eq!(first.read(&mut [0; 1]).unwrap(), 0); assert_eq!(second.read(&mut [0; 1]).unwrap(), 0);
    assert!(review.advance(&mut h, review.revision(), ElapsedTick(1), snapshot()).is_err());
    assert_eq!(h.inspect().executions, 0);
}

#[test]
fn a_single_assigned_round_does_not_buy_a_residual_after_abstention() {
    let (_dir, mut h, _, action, plan) = setup(); let (mut streams, first, second) = sockets();
    streams.remove(&102); drop(second); let mut selection = schedule(256); selection.rounds.truncate(1);
    let mut a = client(first, &action);
    let mut review = h.begin_learned_socket_review(h.revision(), plan, selection, streams, snapshot()).unwrap();
    drive_to_next(&mut review, &mut h, &mut a, Verdict::Abstain);
    assert_eq!(review.status(), LearnedSocketStatus::Finished); assert_eq!(review.history().len(), 1);
    assert!(matches!(review.history()[0], FileLearnedSidecarFinish::Applied { .. }));
    assert_eq!(h.input_revision(1).unwrap(), 1);
    assert!(h.authorize(h.revision(), 1, review.input(), snapshot()).is_err());
}

#[test]
fn wrong_owner_revision_and_initial_clock_leave_the_sequence_usable() {
    let (_dir, mut h, _, action, plan) = setup(); let (_other_dir, mut other, _, _, _) = setup();
    let (streams, first, mut second) = sockets(); let mut a = client(first, &action);
    let mut review = h.begin_learned_socket_review(h.revision(), plan, schedule(256), streams, snapshot()).unwrap();
    let before = h.revision();
    for (host, revision, now) in [(&mut other, 0, ElapsedTick(1)), (&mut h, 1, ElapsedTick(1))] {
        assert!(review.advance(host, revision, now, snapshot()).is_err());
    }
    assert!(review.advance(&mut h, 0, ElapsedTick(0), snapshot()).is_err());
    assert_eq!(h.revision(), before); assert_eq!(review.revision(), 0); assert_eq!(review.polls(), 0);
    assert_eq!(review.records()[&101].connection_steps, 0);
    drive_to_next(&mut review, &mut h, &mut a, Verdict::Allow);
    assert_eq!(review.status(), LearnedSocketStatus::Finished); assert_eq!(review.records()[&102].polls, 0);
    assert_eq!(second.read(&mut [0; 1]).unwrap(), 0);
}
