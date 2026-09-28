//! Logical clock schedules expose exact before/after-I/O deadline crossings.
//! These tests do not assert timing performance or replace the original clock.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn complete_at(host: &mut FileOversight, run: &mut FileLearnedTransportReview,
    workers: &mut Clients, tick: u64) -> FileLearnedTransportStatus
{
    for _ in 0..64 {
        let status = run.advance(host, run.revision(), ElapsedTick(tick), snapshot()).unwrap();
        if status != FileLearnedTransportStatus::Running { return status; }
        peer_steps(workers, 0, false);
    }
    panic!("original protocol did not finish within its fixed ceiling")
}
fn wait_for_sent_commitments(host: &mut FileOversight, run: &mut FileLearnedTransportReview,
    workers: &mut Clients)
{
    for _ in 0..32 {
        if ["alpha", "beta"].into_iter().all(|member|
            workers[&(101, member.to_owned())].phase() == ClientPhase::AwaitingReveal) { return; }
        run.advance(host, run.revision(), ElapsedTick(1), snapshot()).unwrap();
        peer_steps(workers, 0, false);
    }
    panic!("workers did not send the original commitment frames")
}
fn drained(stream: &mut UnixStream) -> Vec<u8> {
    let mut out = Vec::new(); let mut scratch = [0; 4096];
    for _ in 0..64 {
        match stream.read(&mut scratch) {
            Ok(0) => return out,
            Ok(n) => out.extend_from_slice(&scratch[..n]),
            other => panic!("expected a closed socket after admitted failure: {other:?}"),
        }
    }
    panic!("bounded original request did not reach EOF")
}

#[test]
fn stable_clock_path_matches_fixed_tick_refinement_and_original_archives() {
    let first = Directory::new(); let (mut a, _, _, sidecar) = setup(&first);
    let (mut left, mut left_workers) = launch(&mut a, sidecar, schedule());
    complete(&mut a, &mut left, &mut left_workers, 2, false, snapshot()).unwrap();
    let second = Directory::new(); let (mut b, _, _, sidecar) = setup(&second);
    let (mut right, mut right_workers) = launch(&mut b, sidecar, schedule());
    let mut samples = 0;
    for _ in 0..64 {
        let before = samples;
        let status = right.advance_with_clock(&mut b, right.revision(), ElapsedTick(1), snapshot(), || {
            samples += 1; ElapsedTick(1)
        }).unwrap();
        assert_eq!(samples - before, 6, "entry, two samples per member, and final observation");
        if status == FileLearnedTransportStatus::Finished { break; }
        peer_steps(&mut right_workers, 2, false);
    }
    assert_eq!(right.status(), FileLearnedTransportStatus::Finished);
    assert_eq!(left.input(), right.input()); assert_eq!(left.records(), right.records());
    assert_eq!(left.polls(), right.polls()); assert_eq!(left.history().len(), right.history().len());
    for (x, y) in left.history().iter().zip(right.history()) { assert_eq!(x.archive(), y.archive()); }
    assert_eq!(a.learned_generation_inspection().unwrap().numerical,
        b.learned_generation_inspection().unwrap().numerical);
}

#[test]
fn current_time_before_first_request_controls_disclosure_not_the_old_preflight_tick() {
    for tick in [9, 10] {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let plan = schedule(); let (peers, mut workers) = sockets(&plan);
        let mut run = host.begin_learned_transport_review(host.revision(), sidecar, plan, peers, snapshot()).unwrap();
        run.advance_with_clock(&mut host, 0, ElapsedTick(1), snapshot(), || ElapsedTick(tick)).unwrap();
        let worker = workers.get_mut(&(101, "alpha".to_owned())).unwrap();
        if tick == 9 {
            let mut bytes = [0; 5]; assert_eq!(worker.read(&mut bytes).unwrap(), 5);
            assert_eq!(&bytes, b"FAHW1");
        } else {
            empty_read(worker, false);
            assert!(!run.worker_statuses()["alpha"].committed);
        }
        for ((round, _), worker) in &mut workers {
            if *round != 101 { empty_read(worker, false); }
        }
        assert_eq!(host.inspect().executions, 0);
        run.cancel(run.revision()).unwrap();
    }
}

#[test]
fn commitment_is_admitted_at_post_read_time_not_at_a_predeadline_start() {
    for after_read in [9, 10] {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let (mut run, mut workers) = launch(&mut host, sidecar, schedule());
        wait_for_sent_commitments(&mut host, &mut run, &mut workers);
        assert!(run.worker_statuses().values().all(|status| !status.committed));
        let before = run.records()[&(101, "alpha".to_owned())].returned_steps;
        let mut calls = 0;
        run.advance_with_clock(&mut host, run.revision(), ElapsedTick(1), snapshot(), || {
            calls += 1; ElapsedTick(if calls < 3 { 1 } else { after_read })
        }).unwrap();
        assert_eq!(run.records()[&(101, "alpha".to_owned())].returned_steps, before + 1);
        if after_read == 9 {
            assert!(run.worker_statuses().values().all(|status| status.committed));
            assert_eq!(complete_at(&mut host, &mut run, &mut workers, 9), FileLearnedTransportStatus::Finished);
            assert_eq!(run.history()[0].archive().policy.transcript.commits.len(), 2);
            assert!(host.authorize(host.revision(), 1, run.input(), snapshot()).is_ok());
        } else {
            assert!(run.worker_statuses().values().all(|status| !status.committed));
            run.advance(&mut host, run.revision(), ElapsedTick(15), snapshot()).unwrap();
            assert_eq!(run.history()[0].archive().policy.transcript.commits.len(), 0);
            assert!(host.authorize(host.revision(), 1, run.input(), snapshot()).is_err());
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn a_fully_received_reveal_crossing_the_cutoff_is_not_a_vote() {
    for after_read in [14, 15] {
        let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
        let (mut run, mut workers) = launch(&mut host, sidecar, schedule());
        for _ in 0..32 {
            if ["alpha", "beta"].into_iter().all(|member|
                workers[&(101, member.to_owned())].phase() == ClientPhase::ReplySent) { break; }
            run.advance(&mut host, run.revision(), ElapsedTick(1), snapshot()).unwrap();
            peer_steps(&mut workers, 0, false);
        }
        assert!(run.worker_statuses().values().all(|status| status.committed && !status.revealed));
        // The original receiver first reads the four-byte reveal header. A
        // separate operation then receives the complete 32-byte salt payload.
        run.advance(&mut host, run.revision(), ElapsedTick(14), snapshot()).unwrap();
        assert!(run.worker_statuses().values().all(|status| !status.revealed));
        let mut calls = 0;
        run.advance_with_clock(&mut host, run.revision(), ElapsedTick(14), snapshot(), || {
            calls += 1; ElapsedTick(if calls < 3 { 14 } else { after_read })
        }).unwrap();
        assert_eq!(run.status(), FileLearnedTransportStatus::Finished);
        let archive = run.history()[0].archive();
        assert_eq!(archive.policy.transcript.commits.len(), 2);
        assert_eq!(archive.policy.transcript.reveals.len(), if after_read == 14 { 2 } else { 0 });
        if after_read == 15 {
            assert!(matches!(&run.history()[0], FileLearnedSidecarFinish::Applied {
                outcome: Some(SidecarRefinementOutcome::Missing { members }), .. } if members.len() == 2));
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn clock_unwind_after_actual_io_closes_active_and_future_peers_without_retry() {
    let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
    let plan = schedule(); let (peers, mut workers) = sockets(&plan);
    let mut run = host.begin_learned_transport_review(host.revision(), sidecar, plan, peers, snapshot()).unwrap();
    let mut calls = 0;
    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        let _ = run.advance_with_clock(&mut host, 0, ElapsedTick(1), snapshot(), || {
            calls += 1;
            assert!(calls != 3, "injected interruption after alpha's request write");
            ElapsedTick(1)
        });
    }));
    assert!(interrupted.is_err());
    assert_eq!(run.status(), FileLearnedTransportStatus::Failed);
    assert_eq!(run.failure(), Some(&FileHelperSetupError::from(Error::Incomplete)));
    assert_eq!(run.records()[&(101, "alpha".to_owned())].returned_steps, 1);
    assert_eq!(run.records()[&(101, "beta".to_owned())].attempted_steps, 0);
    for (key, worker) in &mut workers {
        let bytes = drained(worker);
        if key == &(101, "alpha".to_owned()) { assert!(bytes.starts_with(b"FAHW1")); }
        else { assert!(bytes.is_empty()); }
    }
    let before = run.records().clone();
    assert!(run.advance(&mut host, run.revision(), ElapsedTick(1), snapshot()).is_err());
    assert_eq!(run.records(), &before); assert!(run.history().is_empty());
    assert_eq!(host.commit_review(host.revision(), 101, "alpha", 0).err(), Some(Error::WrongState.into()));
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn backwards_post_io_clock_is_terminal_and_keeps_returned_progress() {
    let root = Directory::new(); let (mut host, _, _, sidecar) = setup(&root);
    let plan = schedule(); let (peers, mut workers) = sockets(&plan);
    let mut run = host.begin_learned_transport_review(host.revision(), sidecar, plan, peers, snapshot()).unwrap();
    let mut calls = 0;
    let result = run.advance_with_clock(&mut host, 0, ElapsedTick(1), snapshot(), || {
        calls += 1; ElapsedTick(u64::from(calls < 3))
    });
    assert_eq!(result.err(), Some(Error::Stale.into()));
    assert_eq!(run.status(), FileLearnedTransportStatus::Failed);
    let record = run.records()[&(101, "alpha".to_owned())];
    assert_eq!((record.attempted_steps, record.returned_steps), (1, 1));
    assert_eq!(record.last, Some(Ok(IoProgress::Progress)));
    for (key, worker) in &mut workers {
        let bytes = drained(worker);
        if key != &(101, "alpha".to_owned()) { assert!(bytes.is_empty()); }
    }
    assert!(run.history().is_empty()); assert_eq!(host.inspect().executions, 0);
}
