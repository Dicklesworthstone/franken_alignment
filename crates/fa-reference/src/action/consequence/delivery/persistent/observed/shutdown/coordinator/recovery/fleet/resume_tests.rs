//! Interrupted full-roster recovery at the original finite visit allowance.
use super::*;
use super::super::test_support::*;

fn roster(hosts: &[&FileOversight], visits: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, hosts.iter().map(|host| {
        host.shutdown_domain(host.profile.delivery.scope.run).unwrap()
    }).collect(), visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}
fn clocks(count: u64) -> Vec<FileShutdownClock> {
    (1..=count).map(|domain| FileShutdownClock { domain, clock_domain: 99, at: ElapsedTick(2) }).collect()
}
fn coordinator(root: &Directory, p: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), p, MAX_JOURNAL_BYTES).unwrap()
}
fn pending(c: &mut FileShutdownCoordinator, domain: u64, execute: bool) {
    let (index, attempt) = c.begin(c.revision(), domain,
        ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) }).unwrap();
    if execute { c.recover_domain(index, attempt, ElapsedTick(2)).unwrap(); }
}

#[test]
fn resume_completes_the_unvisited_tail_at_the_original_visit_ceiling() {
    let r1 = Directory::new(); let r2 = Directory::new(); let r3 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99); let h3 = member(&r3, 3, 99);
    let p = roster(&[&h1, &h2, &h3], 3); drop((h1, h2, h3));
    let mut c = coordinator(&control, &p);
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    pending(&mut c, 2, true); assert_eq!(c.revision(), 3); drop(c);
    let stopped = [r1.image(), r2.image()]; let untouched = r3.image();
    let mut c = reopen(&control, &p, 3);
    let before = control.image();
    assert_eq!(c.recover_registered_pass(3, &clocks(3)), Err(Error::Limit.into()));
    assert_eq!(control.image(), before); assert_eq!(r3.image(), untouched);
    let pass = c.resume_registered_pass(3, &clocks(3)).unwrap();
    assert!(pass.all_observed_drained()); assert_eq!(pass.initial_revision(), 3);
    assert_eq!(pass.final_revision(), 6); assert_eq!(c.report().visits.len(), 3);
    assert!(c.report().all_observed_drained()); assert_eq!([r1.image(), r2.image()], stopped);
    assert_ne!(r3.image(), untouched);
    for member in &pass.members()[..2] {
        assert!(matches!(&member.result, FileShutdownPassResult::Observed(observation)
            if observation.source == FileShutdownSource::CanonicalImage));
    }
    let complete = control.image(); let native = [r1.image(), r2.image(), r3.image()]; drop(c);
    let mut c = reopen(&control, &p, 6);
    let old_ticks = clocks(3).into_iter().map(|clock| FileShutdownClock { at: ElapsedTick(0), ..clock }).collect::<Vec<_>>();
    assert!(c.resume_registered_pass(6, &old_ticks).unwrap().all_observed_drained());
    assert_eq!(c.revision(), 6); assert_eq!(control.image(), complete);
    assert_eq!([r1.image(), r2.image(), r3.image()], native);
}

#[test]
fn an_entered_but_unstopped_member_is_not_silently_retried() {
    let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
    let p = roster(&[&h1, &h2], 2); drop((h1, h2));
    let mut c = coordinator(&control, &p); pending(&mut c, 1, false); drop(c);
    let first = r1.image(); let second = r2.image(); let mut c = reopen(&control, &p, 1);
    let pass = c.resume_registered_pass(1, &clocks(2)).unwrap();
    assert_eq!(pass.members()[0].result, FileShutdownPassResult::EvidenceRefused(Error::Incomplete.into()));
    assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(pass.unobserved_stops(), vec![1]); assert!(!pass.all_observed_drained());
    assert_eq!(r1.image(), first); assert_ne!(r2.image(), second);
    assert_eq!(c.report().visits.len(), 2); assert_eq!(c.revision(), 3);
    assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
    assert!(!c.report().unavailable);
}

#[test]
fn missing_retained_evidence_does_not_consume_a_new_native_visit_or_hide_the_tail() {
    let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
    let p = roster(&[&h1, &h2], 2); drop((h1, h2));
    let mut c = coordinator(&control, &p);
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap(); drop(c);
    let hidden = r1.0.join("offline"); std::fs::rename(r1.store(), &hidden).unwrap();
    let mut c = reopen(&control, &p, 2);
    let pass = c.resume_registered_pass(2, &clocks(2)).unwrap();
    assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::EvidenceRefused(_)));
    assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(pass.unobserved_stops(), vec![1]); assert_eq!(c.revision(), 4);
    assert_eq!(c.report().visits.len(), 2); assert!(!c.report().all_observed_drained());
    std::fs::rename(hidden, r1.store()).unwrap();
}

#[test]
fn every_resolution_write_barrier_stops_before_an_unvisited_domain_and_can_reopen() {
    for barrier in BARRIERS {
        let r1 = Directory::new(); let r2 = Directory::new(); let r3 = Directory::new(); let control = Directory::new();
        let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99); let h3 = member(&r3, 3, 99);
        let p = roster(&[&h1, &h2, &h3], 3); drop((h1, h2, h3));
        let mut c = coordinator(&control, &p);
        c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
        pending(&mut c, 2, true); drop(c);
        let native = [r1.image(), r2.image(), r3.image()];
        let mut c = reopen(&control, &p, 3); c.store.fail_once(barrier);
        let pass = c.resume_registered_pass(3, &clocks(3)).unwrap();
        assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::Observed(_)));
        assert!(matches!(&pass.members()[1].result,
            FileShutdownPassResult::Unacknowledged(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(pass.members()[2].result, FileShutdownPassResult::NotVisited);
        assert_eq!(pass.final_revision(), 3); assert!(c.report().unavailable);
        assert!(!pass.all_observed_drained()); assert_eq!([r1.image(), r2.image(), r3.image()], native);
        drop(c); let mut c = reopen(&control, &p, 3);
        assert!(c.resume_registered_pass(c.revision(), &clocks(3)).unwrap().all_observed_drained());
        assert_eq!(c.revision(), 6); assert_eq!(c.report().visits.len(), 3);
    }
}

#[test]
fn resume_keeps_whole_roster_clock_and_attempt_admission_before_any_work() {
    let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
    let p = roster(&[&h1, &h2], 2); drop((h1, h2));
    let mut c = coordinator(&control, &p);
    assert!(c.recover_registered_pass(0, &clocks(2)).unwrap().all_observed_drained());
    let bytes = control.image(); let before = c.report(); let native = [r1.image(), r2.image()];
    assert_eq!(c.resume_registered_pass(4, &clocks(2)), Err(Error::Limit.into()));
    assert_eq!(c.report(), before); assert_eq!(control.image(), bytes); drop(c);
    let mut c = reopen(&control, &p, 4); let before = c.report();
    let mut foreign = clocks(2); foreign[1].clock_domain = 101;
    for (inputs, error) in [(clocks(1), Error::Binding), (vec![clocks(2)[0]; 2], Error::Duplicate),
        (foreign, Error::Binding)] {
        assert_eq!(c.resume_registered_pass(4, &inputs), Err(error.into()));
        assert_eq!(c.report(), before); assert_eq!(control.image(), bytes);
        assert_eq!([r1.image(), r2.image()], native);
    }
    assert!(c.resume_registered_pass(4, &clocks(2)).unwrap().all_observed_drained());
    assert_eq!(c.revision(), 4); assert_eq!(control.image(), bytes);
}

#[test]
fn the_latest_refusal_cannot_be_skipped_in_favor_of_an_older_success() {
    let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
    let p = roster(&[&h1, &h2], 4); drop((h1, h2));
    let mut c = coordinator(&control, &p);
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    c.unavailable(2, 1, JournalError::Busy).unwrap();
    let pass = c.resume_registered_pass(4, &clocks(2)).unwrap();
    assert!(pass.all_observed_drained()); assert_eq!(c.revision(), 8);
    assert_eq!(c.report().visits.len(), 4);
    assert!(matches!(c.report().visits[1].result, ShutdownVisitResult::Refused { .. }));
    assert!(matches!(c.report().visits[2].result, ShutdownVisitResult::Observed { .. }));
}
