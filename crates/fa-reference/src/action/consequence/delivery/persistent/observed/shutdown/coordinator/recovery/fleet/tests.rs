//! Full-roster original recovery, not independent mock member outcomes.
use super::*;
use super::super::test_support::*;

fn roster(hosts: &[&FileOversight], visits: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, hosts.iter().map(|host| {
        host.shutdown_domain(host.profile.delivery.scope.run).unwrap()
    }).collect(), visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}
fn clock(domain: u64, clock_domain: u64, at: u64) -> FileShutdownClock {
    FileShutdownClock { domain, clock_domain, at: ElapsedTick(at) }
}
fn coordinator(root: &Directory, p: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), p, MAX_JOURNAL_BYTES).unwrap()
}

#[test]
fn full_roster_uses_each_original_clock_and_refreshes_after_exhausting_all_visit_slots() {
    let r1 = Directory::new(); let r2 = Directory::new(); let r3 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 9, 99); let h2 = member(&r2, 2, 101); let h3 = member(&r3, 5, 202);
    let p = roster(&[&h1, &h2, &h3], 3);
    let profiles = [h1.profile.clone(), h2.profile.clone(), h3.profile.clone()];
    drop((h1, h2, h3));
    let mut c = coordinator(&control, &p);
    let pass = c.recover_registered_pass(0, &[clock(9, 99, 2), clock(5, 202, 4), clock(2, 101, 3)]).unwrap();
    assert!(pass.all_observed_stopped()); assert!(pass.all_observed_drained());
    assert!(pass.unobserved_stops().is_empty());
    assert_eq!(pass.operation(), 900); assert_eq!(pass.initial_revision(), 0); assert_eq!(pass.final_revision(), 6);
    assert_eq!(pass.members().iter().map(|member| member.clock.domain).collect::<Vec<_>>(), vec![2, 5, 9]);
    for ((root, profile), tick) in [&r1, &r2, &r3].into_iter().zip(&profiles).zip([2, 3, 4]) {
        let image = FileOversight::read_publication(root.store(), profile).unwrap();
        assert_eq!(image.revision, 5); assert_eq!(image.executions, 0);
        assert_eq!(image.control.ledger.elapsed, Some(ElapsedTick(tick)));
    }
    let complete = control.image(); let native = [r1.image(), r2.image(), r3.image()]; drop(c);
    let mut c = reopen(&control, &p, 6);
    assert!(!c.report().all_observed_drained());
    for visit in 0..3 { c.resolve_shutdown_visit(6, visit).unwrap(); }
    assert!(c.report().all_observed_drained()); assert_eq!(control.image(), complete);
    assert_eq!([r1.image(), r2.image(), r3.image()], native);
}

#[test]
fn busy_and_missing_members_do_not_hide_failures_or_prevent_later_domains_from_stopping() {
    let r1 = Directory::new(); let r2 = Directory::new(); let r3 = Directory::new(); let r4 = Directory::new();
    let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
    let h3 = member(&r3, 3, 99); let h4 = member(&r4, 4, 99);
    let p = roster(&[&h1, &h2, &h3, &h4], 4);
    drop((h1, h3, h4));
    let hidden = r3.0.join("offline"); std::fs::rename(r3.store(), &hidden).unwrap();
    let mut c = coordinator(&control, &p);
    let pass = c.recover_registered_pass(0, &[clock(1, 99, 2), clock(2, 99, 2), clock(3, 99, 2), clock(4, 99, 2)]).unwrap();
    assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(pass.members()[1].result, FileShutdownPassResult::Refused(JournalError::Busy));
    assert!(matches!(&pass.members()[2].result, FileShutdownPassResult::Refused(_)));
    assert!(matches!(&pass.members()[3].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(pass.unobserved_stops(), vec![2, 3]); assert!(!pass.all_observed_drained());
    assert!(!c.report().unavailable); assert_eq!(c.report().visits.len(), 4); assert_eq!(c.revision(), 8);
    assert!(h2.inspect().stop.is_none());
    std::fs::rename(hidden, r3.store()).unwrap();
}

#[test]
fn roster_and_clock_binding_errors_refuse_before_any_coordinator_or_domain_change() {
    let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 202);
    let p = roster(&[&h1, &h2], 2); let mut c = coordinator(&control, &p);
    let native = [r1.image(), r2.image()]; let before = c.report(); let bytes = control.image();
    for (clocks, error) in [
        (vec![clock(1, 99, 2)], Error::Binding),
        (vec![clock(1, 99, 2), clock(1, 99, 2)], Error::Duplicate),
        (vec![clock(1, 99, 2), clock(3, 202, 2)], Error::Binding),
        (vec![clock(1, 99, 2), clock(2, 99, 2)], Error::Binding),
        (vec![clock(1, 99, 2); MAX_SHUTDOWN_DOMAINS + 1], Error::Limit),
    ] {
        assert_eq!(c.recover_registered_pass(0, &clocks), Err(error.into()));
        assert_eq!(c.report(), before); assert_eq!(control.image(), bytes);
        assert_eq!([r1.image(), r2.image()], native);
    }
    assert_eq!(c.recover_registered_pass(1, &[clock(1, 99, 2), clock(2, 202, 2)]), Err(Error::Stale.into()));
    drop((h1, h2));
    assert!(c.recover_registered_pass(0, &[clock(2, 202, 2), clock(1, 99, 2)]).unwrap().all_observed_drained());
}

#[test]
fn full_pass_visit_capacity_is_checked_before_spending_the_first_member_slot() {
    for enough in [false, true] {
        let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
        let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
        let p = roster(&[&h1, &h2], if enough { 2 } else { 1 }); drop((h1, h2));
        let mut c = coordinator(&control, &p);
        let before = control.image(); let native = [r1.image(), r2.image()];
        let result = c.recover_registered_pass(0, &[clock(1, 99, 2), clock(2, 99, 2)]);
        if enough { assert!(result.unwrap().all_observed_drained()); }
        else {
            assert_eq!(result, Err(Error::Limit.into())); assert_eq!(c.revision(), 0);
            assert_eq!(control.image(), before); assert_eq!([r1.image(), r2.image()], native);
        }
    }
}

#[test]
fn every_intent_storage_barrier_stops_the_pass_before_touching_later_members() {
    for barrier in BARRIERS {
        let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
        let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
        let p = roster(&[&h1, &h2], 2); drop((h1, h2));
        let native = [r1.image(), r2.image()]; let mut c = coordinator(&control, &p);
        c.store.fail_once(barrier);
        let pass = c.recover_registered_pass(0, &[clock(1, 99, 2), clock(2, 99, 2)]).unwrap();
        assert!(matches!(&pass.members()[0].result,
            FileShutdownPassResult::Unacknowledged(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(pass.members()[1].result, FileShutdownPassResult::NotVisited);
        assert_eq!(pass.unobserved_stops(), vec![1, 2]); assert!(!pass.all_observed_drained());
        assert_eq!(pass.final_revision(), 0); assert!(c.report().unavailable);
        assert_eq!([r1.image(), r2.image()], native);
    }
}

#[test]
fn native_shutdown_before_failed_completion_is_not_reported_as_a_successful_pass() {
    let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
    let p = roster(&[&h1, &h2], 2); let native_profile = h1.profile.clone(); drop((h1, h2));
    let visit = ShutdownVisit { domain: 1, kind: ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) },
        result: ShutdownVisitResult::Entered };
    let cap = codec::encode(&control.store(), &p.encode().unwrap(), MAX_JOURNAL_BYTES,
        1, &[visit], &p.start()).unwrap().len();
    let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
    let later = r2.image();
    let pass = c.recover_registered_pass(0, &[clock(1, 99, 2), clock(2, 99, 2)]).unwrap();
    assert_eq!(pass.members()[0].result, FileShutdownPassResult::Unacknowledged(Error::Limit.into()));
    assert_eq!(pass.members()[1].result, FileShutdownPassResult::NotVisited);
    assert!(!pass.all_observed_stopped()); assert!(!pass.all_observed_drained());
    assert_eq!(pass.final_revision(), 1); assert_eq!(r2.image(), later);
    assert!(FileOversight::read_publication(r1.store(), &native_profile).unwrap().stop.is_some());
    assert!(c.report().unavailable); assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
}

#[test]
fn an_individually_stale_clock_refuses_that_domain_without_reusing_another_domains_time() {
    let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
    let mut h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 202);
    h1.observe_time(h1.revision(), ElapsedTick(10)).unwrap();
    let p = roster(&[&h1, &h2], 2); let before = r1.image(); drop((h1, h2));
    let mut c = coordinator(&control, &p);
    let pass = c.recover_registered_pass(0, &[clock(1, 99, 9), clock(2, 202, 2)]).unwrap();
    assert_eq!(pass.members()[0].result, FileShutdownPassResult::Refused(Error::Stale.into()));
    assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(pass.unobserved_stops(), vec![1]); assert_eq!(r1.image(), before);
    assert!(!c.report().unavailable); assert_eq!(c.revision(), 4);
}

#[test]
fn an_earlier_success_cannot_make_a_later_pass_ignore_a_missing_member() {
    let r1 = Directory::new(); let r2 = Directory::new(); let control = Directory::new();
    let h1 = member(&r1, 1, 99); let h2 = member(&r2, 2, 99);
    let p = roster(&[&h1, &h2], 4); drop((h1, h2));
    let mut c = coordinator(&control, &p); let clocks = [clock(1, 99, 2), clock(2, 99, 2)];
    assert!(c.recover_registered_pass(0, &clocks).unwrap().all_observed_drained());
    let second = r2.image(); let hidden = r1.0.join("offline"); std::fs::rename(r1.store(), &hidden).unwrap();
    let pass = c.recover_registered_pass(c.revision(), &clocks).unwrap();
    assert_eq!(pass.unobserved_stops(), vec![1]); assert!(!pass.all_observed_drained());
    assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::Refused(_)));
    assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::Observed(observation)
        if observation.source == FileShutdownSource::CanonicalImage));
    assert_eq!(r2.image(), second); assert!(!c.report().all_observed_drained());
    std::fs::rename(hidden, r1.store()).unwrap();
}
