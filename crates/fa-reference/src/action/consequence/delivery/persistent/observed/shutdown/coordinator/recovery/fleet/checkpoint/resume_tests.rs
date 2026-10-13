//! Original fixed-roster resume across passive reads and native recovery visits.
use super::*;
use super::super::super::test_support::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
#[path = "../../../../recovery/fixture.rs"]
mod effect_fixture;

fn clocks(count: u64) -> Vec<FileShutdownClock> {
    (1..=count).map(|domain| FileShutdownClock { domain, clock_domain: 99, at: ElapsedTick(2) }).collect()
}
fn roster(hosts: &[&FileOversight], visits: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, hosts.iter().map(|h| h.shutdown_domain(h.profile.delivery.scope.run).unwrap())
        .collect(), visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}
fn coordinator(root: &Directory, p: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), p, MAX_JOURNAL_BYTES).unwrap()
}
fn pending(c: &mut FileShutdownCoordinator, id: u64, execute: bool) {
    let (index, attempt) = c.begin(c.revision(), id, ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) }).unwrap();
    if execute { c.recover_domain(index, attempt, ElapsedTick(2)).unwrap(); }
}
fn mixed(roots: &[Directory], control: &Directory) -> (FileShutdownPlan, FileShutdownCoordinator) {
    let hosts: Vec<_> = roots.iter().enumerate().map(|(i, root)| member(root, i as u64 + 1, 99)).collect();
    let p = roster(&hosts.iter().collect::<Vec<_>>(), roots.len()); drop(hosts);
    let mut c = coordinator(control, &p);
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    pending(&mut c, 2, true); drop(c);
    let c = reopen(control, &p, 3); (p, c)
}

#[test]
fn mixed_resume_matches_batch_and_rechecks_exact_success_at_the_visit_ceiling() {
    let mut expected = None;
    for quantum in [0, 1, 2, MAX_JOURNAL_EVENTS] {
        let roots: Vec<_> = (0..3).map(|_| Directory::new()).collect(); let control = Directory::new();
        let (p, mut c) = mixed(&roots, &control);
        let native: Vec<_> = roots.iter().map(Directory::image).collect();
        let mut checkpoints = Vec::new();
        let pass = if quantum == 0 { c.resume_registered_pass(3, &clocks(3)).unwrap() } else {
            c.resume_registered_pass_with_checkpoints(3, &clocks(3), quantum, |point| {
                checkpoints.push(point); true
            }).unwrap()
        };
        assert!(pass.all_observed_drained()); assert_eq!(pass.final_revision(), 6);
        assert_eq!(c.report().visits.len(), 3);
        if let Some(original) = &expected { assert_eq!(&pass, original); }
        else { expected = Some(pass.clone()); }
        if quantum != 0 {
            for domain in 1..=3 {
                let points: Vec<_> = checkpoints.iter().filter(|p| p.domain == domain).collect();
                assert_eq!(points[0].replay, None);
                assert_eq!(points[1].replay.unwrap().replayed_events, 0);
                let final_progress = points.last().unwrap().replay.unwrap();
                assert_eq!(final_progress.status, FileStopRecoveryStatus::Ready);
                assert_eq!(final_progress.replayed_events, if domain == 3 { 2 } else { 5 });
                for pair in points[1..].windows(2) {
                    assert!(pair[1].replay.unwrap().replayed_events - pair[0].replay.unwrap().replayed_events <= quantum);
                }
            }
        }
        assert_eq!(roots[0].image(), native[0]); assert_eq!(roots[1].image(), native[1]);
        assert_ne!(roots[2].image(), native[2]);
        let acknowledged = control.image(); let native: Vec<_> = roots.iter().map(Directory::image).collect(); drop(c);
        let mut c = reopen(&control, &p, 6);
        let old: Vec<_> = clocks(3).into_iter().map(|clock| FileShutdownClock { at: ElapsedTick(0), ..clock }).collect();
        assert!(c.resume_registered_pass_with_checkpoints(6, &old, 1, |_| true).unwrap().all_observed_drained());
        assert_eq!(control.image(), acknowledged); assert_eq!(roots.iter().map(Directory::image).collect::<Vec<_>>(), native);
    }
}

#[test]
fn every_read_checkpoint_cancels_without_rewriting_a_saved_visit_or_either_domain() {
    // Two captured five-event histories: before-read, zero, each event = 14.
    for stop_at in 0..14 {
        let roots: Vec<_> = (0..2).map(|_| Directory::new()).collect(); let control = Directory::new();
        let (_p, mut c) = mixed(&roots, &control);
        let before = c.report(); let durable = control.image(); let native: Vec<_> = roots.iter().map(Directory::image).collect();
        let mut calls = 0;
        let pass = c.resume_registered_pass_with_checkpoints(3, &clocks(2), 1, |_| {
            let go = calls != stop_at; calls += 1; go
        }).unwrap();
        assert_eq!(calls, stop_at + 1); assert_eq!(pass.final_revision(), 3);
        let active = usize::from(stop_at >= 7);
        if stop_at % 7 == 0 { assert_eq!(pass.members()[active].result, FileShutdownPassResult::NotVisited); }
        else { assert_eq!(pass.members()[active].result, FileShutdownPassResult::EvidenceRefused(JournalError::Unavailable)); }
        for member in &pass.members()[active + 1..] { assert_eq!(member.result, FileShutdownPassResult::NotVisited); }
        assert!(!pass.all_observed_drained()); assert!(!c.report().unavailable);
        assert_eq!(c.report().visits, before.visits); assert_eq!(control.image(), durable);
        assert_eq!(roots.iter().map(Directory::image).collect::<Vec<_>>(), native);
    }
}

#[test]
fn missing_or_unstopped_evidence_is_not_retried_while_a_new_tail_can_progress() {
    for missing in [false, true] {
        let roots: Vec<_> = (0..2).map(|_| Directory::new()).collect(); let control = Directory::new();
        let h1 = member(&roots[0], 1, 99); let h2 = member(&roots[1], 2, 99);
        let p = roster(&[&h1, &h2], 2); drop((h1, h2));
        let mut c = coordinator(&control, &p); pending(&mut c, 1, false); drop(c);
        let first = roots[0].image(); let second = roots[1].image();
        let hidden = roots[0].store().join("saved-image");
        if missing { std::fs::rename(roots[0].store().join(storage::CANONICAL), &hidden).unwrap(); }
        let mut c = reopen(&control, &p, 1);
        let pass = c.resume_registered_pass_with_checkpoints(1, &clocks(2), 1, |_| true).unwrap();
        assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::EvidenceRefused(_)));
        assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::Observed(_)));
        assert_eq!(pass.unobserved_stops(), vec![1]); assert_eq!(pass.final_revision(), 3);
        assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
        assert_eq!(c.report().visits.len(), 2); assert_ne!(roots[1].image(), second);
        if missing { assert!(!roots[0].store().join(storage::CANONICAL).exists()); std::fs::rename(hidden, roots[0].store().join(storage::CANONICAL)).unwrap(); }
        assert_eq!(roots[0].image(), first);
    }
}

#[test]
fn pending_resolution_ack_failures_stop_before_a_new_domain_and_reopen_without_repeating_effects() {
    for barrier in BARRIERS {
        let roots: Vec<_> = (0..3).map(|_| Directory::new()).collect(); let control = Directory::new();
        let (p, mut c) = mixed(&roots, &control);
        let native: Vec<_> = roots.iter().map(Directory::image).collect(); c.store.fail_once(barrier);
        let mut touched = Vec::new();
        let pass = c.resume_registered_pass_with_checkpoints(3, &clocks(3), 1, |point| { touched.push(point.domain); true }).unwrap();
        assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::Observed(_)));
        assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::Unacknowledged(JournalError::Io(f)) if f.operation == barrier));
        assert_eq!(pass.members()[2].result, FileShutdownPassResult::NotVisited); assert!(!touched.contains(&3));
        assert_eq!(pass.final_revision(), 3); assert!(c.report().unavailable);
        assert_eq!(roots.iter().map(Directory::image).collect::<Vec<_>>(), native); drop(c);
        let mut c = reopen(&control, &p, 3); let revision = c.revision();
        assert!(c.resume_registered_pass_with_checkpoints(revision, &clocks(3), 1, |_| true).unwrap().all_observed_drained());
        assert_eq!(c.revision(), 6); assert_eq!(c.report().visits.len(), 3);
        assert_eq!(roots[0].image(), native[0]); assert_eq!(roots[1].image(), native[1]);
    }
}

#[test]
fn callback_unwind_after_one_success_keeps_the_pending_visit_and_does_not_start_the_tail() {
    let roots: Vec<_> = (0..3).map(|_| Directory::new()).collect(); let control = Directory::new();
    let (p, mut c) = mixed(&roots, &control);
    let native: Vec<_> = roots.iter().map(Directory::image).collect(); let durable = control.image();
    assert!(catch_unwind(AssertUnwindSafe(|| c.resume_registered_pass_with_checkpoints(3, &clocks(3), 1, |point| {
        if point.domain == 2 && point.replay.is_some_and(|p| p.replayed_events == 1) { panic!("elapsed clock unavailable"); }
        true
    }))).is_err());
    assert!(c.report().unavailable); assert!(!c.report().all_observed_drained());
    assert_eq!(c.revision(), 3); assert_eq!(control.image(), durable);
    assert_eq!(roots.iter().map(Directory::image).collect::<Vec<_>>(), native); drop(c);
    let mut c = reopen(&control, &p, 3);
    assert!(c.resume_registered_pass_with_checkpoints(3, &clocks(3), 2, |_| true).unwrap().all_observed_drained());
    assert_eq!(c.revision(), 6);
}

#[test]
fn whole_roster_clock_and_quantum_admission_run_before_callbacks_and_do_not_refresh_allowances() {
    let roots: Vec<_> = (0..2).map(|_| Directory::new()).collect(); let control = Directory::new();
    let (_p, mut c) = mixed(&roots, &control);
    let before = c.report(); let durable = control.image(); let native: Vec<_> = roots.iter().map(Directory::image).collect();
    let mut foreign = clocks(2); foreign[1].clock_domain += 1;
    for (revision, inputs, quantum, error) in [
        (2, clocks(2), 1, Error::Stale), (3, clocks(1), 1, Error::Binding),
        (3, vec![clocks(2)[0]; 2], 1, Error::Duplicate), (3, foreign, 1, Error::Binding),
        (3, clocks(2), 0, Error::InvalidInput), (3, clocks(2), MAX_JOURNAL_EVENTS + 1, Error::Limit),
    ] {
        assert_eq!(c.resume_registered_pass_with_checkpoints(revision, &inputs, quantum, |_| panic!("admission first")), Err(error.into()));
        assert_eq!(c.report(), before); assert_eq!(control.image(), durable);
    }
    assert!(c.resume_registered_pass_with_checkpoints(3, &clocks(2), 1, |_| true).unwrap().all_observed_drained());
    let spent = c.report(); let bytes = control.image();
    assert_eq!(c.resume_registered_pass_with_checkpoints(c.revision(), &clocks(2), 1, |_| panic!("attempt allowance spent")), Err(Error::Limit.into()));
    assert_eq!(c.report(), spent); assert_eq!(control.image(), bytes);
    assert_eq!(roots.iter().map(Directory::image).collect::<Vec<_>>(), native);
}

#[test]
fn real_two_key_execution_and_expired_unknowns_keep_their_original_receipt_accounting() {
    for mode in 0..4 {
        let root = effect_fixture::Directory::new(); let control = Directory::new();
        let mut ready = effect_fixture::Ready::new(&root, effect_fixture::profile(), true);
        if mode != 0 { ready.dispatch(); }
        if mode == 2 { ready.host.publish(ready.host.revision(), 1).unwrap(); }
        let p = FileShutdownPlan::new(900, vec![ready.host.shutdown_domain(1).unwrap()], 4, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        let mut c = coordinator(&control, &p); pending(&mut c, 1, false); drop(c);
        let request = effect_fixture::request(&ready.host);
        ready.host.stop_and_drain(ready.host.revision(), request, ElapsedTick(if mode == 3 { 1001 } else { 2 })).unwrap();
        let native = root.bytes(); let mut c = reopen(&control, &p, 1);
        let pass = c.resume_registered_pass_with_checkpoints(1, &clocks(1), 1, |_| true).unwrap();
        let FileShutdownPassResult::Observed(observation) = &pass.members()[0].result else { panic!("original receipt"); };
        let progress = observation.stop.as_ref().unwrap();
        assert_eq!(observation.executions, u64::from(mode == 2));
        assert_eq!(progress.charged_units, if mode >= 2 { 16 } else { 0 });
        assert_eq!(progress.reserved_units, 0); assert_eq!(progress.drained(), mode != 3);
        if mode == 3 { assert_eq!(progress.irrecoverable, vec![1]); }
        assert_eq!(root.bytes(), native); assert_eq!(c.report().visits.len(), 1);
    }
}
