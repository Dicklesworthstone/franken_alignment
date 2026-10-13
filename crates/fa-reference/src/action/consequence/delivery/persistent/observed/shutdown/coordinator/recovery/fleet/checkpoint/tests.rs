//! Full fixed-roster behavior using original journals and acknowledged visits.
use super::*;
use super::super::super::test_support::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn setup(visits: usize, event_limit: usize) -> (Vec<Directory>, Directory, FileShutdownPlan,
    FileShutdownCoordinator, Vec<FileShutdownClock>)
{
    let roots = vec![Directory::new(), Directory::new()]; let control = Directory::new();
    let mut domains = Vec::new(); let mut clocks = Vec::new();
    for (i, root) in roots.iter().enumerate() {
        let id = i as u64 + 1; let mut p = profile();
        p.delivery.scope.run = id; p.delivery.scope.authority = 100 + id;
        p.delivery.clock_domain = 90 + id; p.delivery.limits.events = event_limit;
        let (mut host, _) = FileOversight::create(root.store(), p).unwrap();
        host.enable_publication_guard(host.revision()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        domains.push(host.shutdown_domain(id).unwrap());
        clocks.push(FileShutdownClock { domain: id, clock_domain: 90 + id, at: ElapsedTick(2) });
    }
    let p = FileShutdownPlan::new(900, domains, visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
    let c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    (roots, control, p, c, clocks)
}

#[test]
fn checkpointed_pass_matches_the_original_batch_and_bounds_each_native_quantum() {
    for quantum in [1, 2, MAX_JOURNAL_EVENTS] {
        let (_roots, _control, _p, mut c, mut clocks) = setup(8, 64);
        clocks.reverse(); let mut seen = Vec::new();
        let pass = c.recover_registered_pass_with_checkpoints(0, &clocks, quantum, |point| {
            seen.push(point); true
        }).unwrap();
        assert!(pass.all_observed_drained()); assert_eq!(pass.final_revision(), 4);
        assert_eq!(c.report().visits.len(), 2);
        for id in 1..=2 {
            let entries: Vec<_> = seen.iter().filter(|point| point.domain == id).collect();
            assert_eq!(entries[0].replay, None);
            let progress: Vec<_> = entries.iter().filter_map(|point| point.replay).collect();
            assert_eq!(progress[0].replayed_events, 0);
            assert_eq!(progress.last().unwrap().status, FileStopRecoveryStatus::Ready);
            assert_eq!(progress.last().unwrap().attempted_events, 2);
            for pair in progress.windows(2) {
                assert!(pair[1].attempted_events - pair[0].attempted_events <= quantum);
            }
        }
        let (_roots, _control, _p, mut old, clocks) = setup(8, 64);
        let expected = old.recover_registered_pass(0, &clocks).unwrap();
        assert_eq!(pass, expected, "same original receipts and denominator");
    }
}

#[test]
fn cancellation_at_every_checkpoint_keeps_unfinished_and_unvisited_members_explicit() {
    // Each two-event member has before-intent, zero, one, and Ready checkpoints.
    for stop_at in 0..8 {
        let (roots, _control, _p, mut c, clocks) = setup(8, 64);
        let before: Vec<_> = roots.iter().map(Directory::image).collect();
        for root in &roots { std::fs::write(root.store().join("delivery.pending"), b"retained").unwrap(); }
        let mut calls = 0;
        let pass = c.recover_registered_pass_with_checkpoints(0, &clocks, 1, |_| {
            let keep = calls != stop_at; calls += 1; keep
        }).unwrap();
        assert_eq!(calls, stop_at + 1); assert!(!pass.all_observed_stopped());
        let completed = stop_at / 4; let entered = usize::from(stop_at % 4 != 0);
        assert_eq!(c.report().visits.len(), completed + entered);
        assert_eq!(pass.final_revision(), ((completed + entered) * 2) as u64);
        assert!(!c.report().unavailable);
        for (i, member) in pass.members().iter().enumerate() {
            if i < completed {
                assert!(matches!(member.result, FileShutdownPassResult::Observed(_)));
                assert_ne!(roots[i].image(), before[i]);
                assert!(!roots[i].store().join("delivery.pending").exists());
            } else {
                assert_eq!(roots[i].image(), before[i]);
                assert!(roots[i].store().join("delivery.pending").exists());
                assert_eq!(member.result, if i == completed && entered == 1 {
                    FileShutdownPassResult::Refused(JournalError::Unavailable)
                } else { FileShutdownPassResult::NotVisited });
            }
        }
        assert_eq!(pass.unobserved_stops(), ((completed + 1) as u64..=2).collect::<Vec<_>>());
    }
}

#[test]
fn full_roster_clock_and_capacity_admission_precede_callbacks_or_mutation() {
    let (roots, control, _p, mut c, clocks) = setup(8, 64);
    let before = control.image(); let domain_bytes: Vec<_> = roots.iter().map(Directory::image).collect();
    let mut foreign = clocks.clone(); foreign[0].clock_domain += 1;
    for invalid in [vec![], vec![clocks[0]], vec![clocks[0], clocks[0]], foreign] {
        assert!(c.recover_registered_pass_with_checkpoints(0, &invalid, 1, |_| panic!("not admitted")).is_err());
    }
    for quantum in [0, MAX_JOURNAL_EVENTS + 1] {
        assert!(c.recover_registered_pass_with_checkpoints(0, &clocks, quantum, |_| panic!("invalid quantum")).is_err());
    }
    assert_eq!(control.image(), before); assert!(c.report().visits.is_empty());
    for (root, bytes) in roots.iter().zip(domain_bytes) { assert_eq!(root.image(), bytes); }
    let (_roots, control, _p, mut c, clocks) = setup(1, 64);
    let before = control.image();
    assert_eq!(c.recover_registered_pass_with_checkpoints(0, &clocks, 1, |_| panic!("insufficient visits")), Err(Error::Limit.into()));
    assert_eq!(control.image(), before);
}

#[test]
fn offline_or_busy_members_refuse_without_hiding_or_blocking_a_reachable_member() {
    for busy in [false, true] {
        let (roots, _control, _p, mut c, clocks) = setup(8, 64);
        let before = roots[0].image();
        let lock = if busy { Some(storage::Store::open(&roots[0].store()).unwrap()) } else { None };
        let hidden = roots[0].0.join("offline");
        if !busy { std::fs::rename(roots[0].store(), &hidden).unwrap(); }
        let mut seen = Vec::new();
        let pass = c.recover_registered_pass_with_checkpoints(0, &clocks, 1, |point| { seen.push(point); true }).unwrap();
        assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::Refused(_)));
        if busy { assert_eq!(pass.members()[0].result, FileShutdownPassResult::Refused(JournalError::Busy)); }
        assert!(matches!(pass.members()[1].result, FileShutdownPassResult::Observed(_)));
        assert_eq!(pass.unobserved_stops(), vec![1]); assert!(!pass.all_observed_drained());
        assert_eq!(seen.iter().filter(|point| point.domain == 1).count(), 1);
        drop(lock);
        if !busy { std::fs::rename(hidden, roots[0].store()).unwrap(); }
        assert_eq!(roots[0].image(), before);
    }
}

#[test]
fn unwind_after_one_success_keeps_the_next_intent_and_requires_explicit_recovery() {
    let (roots, control, p, mut c, clocks) = setup(16, 64);
    let before = roots[1].image();
    let error = catch_unwind(AssertUnwindSafe(|| {
        c.recover_registered_pass_with_checkpoints(0, &clocks, 1, |point| {
            if point.domain == 2 && point.replay.is_some_and(|progress| progress.replayed_events == 1) {
                panic!("operator interrupted between native events");
            }
            true
        })
    }));
    assert!(error.is_err()); assert_eq!(roots[1].image(), before);
    assert!(c.report().unavailable); assert!(!c.report().all_observed_drained());
    assert!(matches!(c.report().visits[1].result, ShutdownVisitResult::Entered));
    assert!(storage::Store::open(&roots[1].store()).is_ok()); drop(c);
    let mut c = reopen(&control, &p, 3);
    let completed = roots[0].image();
    let resume = c.resume_registered_pass(c.revision(), &clocks).unwrap();
    assert!(matches!(resume.members()[0].result, FileShutdownPassResult::Observed(_)));
    assert!(matches!(resume.members()[1].result, FileShutdownPassResult::EvidenceRefused(_)));
    assert_eq!(roots[1].image(), before, "resolution cannot silently retry a stop");
    let recovered = c.recover_registered_pass_with_checkpoints(c.revision(), &clocks, 1, |_| true).unwrap();
    assert!(recovered.all_observed_drained()); assert_eq!(roots[0].image(), completed);
}

#[test]
fn unacknowledged_completion_or_cancellation_stops_the_pass_without_fabricating_a_result() {
    for cancel in [false, true] {
        let (roots, control, p, mut c, clocks) = setup(8, 64);
        let before: Vec<_> = roots.iter().map(Directory::image).collect();
        let pass = c.recover_registered_pass_with_checkpoints(0, &clocks, 1, |point| {
            if point.domain == 1 && point.replay.is_some_and(|p| p.status == FileStopRecoveryStatus::Ready) {
                std::fs::write(control.store().join("delivery.pending"), b"blocks acknowledgment").unwrap();
                return !cancel;
            }
            true
        }).unwrap();
        assert!(matches!(pass.members()[0].result, FileShutdownPassResult::Unacknowledged(_)));
        assert_eq!(pass.members()[1].result, FileShutdownPassResult::NotVisited);
        assert_eq!(roots[0].image() == before[0], cancel);
        assert_eq!(roots[1].image(), before[1]); assert!(c.report().unavailable);
        assert!(!pass.all_observed_drained()); drop(c);
        let mut c = reopen(&control, &p, 1); let completed = roots[0].image();
        let recovered = c.recover_registered_pass(c.revision(), &clocks).unwrap();
        assert!(recovered.all_observed_drained());
        if !cancel { assert_eq!(roots[0].image(), completed, "native success is not repeated"); }
    }
}

#[test]
fn drained_full_capacity_members_need_no_new_tick_or_domain_tail() {
    let (roots, _control, _p, mut c, mut clocks) = setup(8, 5);
    let first = c.recover_registered_pass_with_checkpoints(0, &clocks, 1, |_| true).unwrap();
    assert!(first.all_observed_drained()); let before: Vec<_> = roots.iter().map(Directory::image).collect();
    for clock in &mut clocks { clock.at = ElapsedTick(0); }
    let again = c.recover_registered_pass_with_checkpoints(c.revision(), &clocks, 2, |_| true).unwrap();
    assert!(again.all_observed_drained());
    for ((root, bytes), member) in roots.iter().zip(before).zip(again.members()) {
        assert_eq!(root.image(), bytes);
        let FileShutdownPassResult::Observed(observation) = &member.result else { panic!("native observed stop"); };
        assert_eq!(observation.journal_revision, 5); assert_eq!(observation.source, FileShutdownSource::CanonicalImage);
        assert!(observation.last_drain.as_ref().unwrap().sweep.progress.drained());
    }
}
