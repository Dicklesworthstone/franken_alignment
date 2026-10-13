//! Actual native journals and coordinator custody; no supplied stop reports.
use super::*;
use super::super::super::test_support::*;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn bindings(count: u64) -> Vec<FileShutdownClockBinding> {
    (1..=count).map(|domain| FileShutdownClockBinding { domain, clock_domain: 99 + domain }).collect()
}
fn roster(hosts: &[&FileOversight], visits: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, hosts.iter().map(|host| host.shutdown_domain(host.profile.delivery.scope.run).unwrap())
        .collect(), visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}
fn coordinator(root: &Directory, plan: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), plan, MAX_JOURNAL_BYTES).unwrap()
}

#[test]
fn each_domain_samples_after_its_ready_cut_and_matches_supplied_time_native_results() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 100); let h2 = member(&b, 2, 101);
    let plan = roster(&[&h1, &h2], 4); let before = [a.image(), b.image()]; drop((h1, h2));
    let mut c = coordinator(&control, &plan);
    let ready = Cell::new(0); let calls = RefCell::new(Vec::new());
    let pass = c.recover_registered_pass_with_clocks(0, &bindings(2), 1, |binding| {
        assert_eq!(ready.get(), binding.domain);
        calls.borrow_mut().push(binding);
        Ok(ElapsedTick(binding.domain + 2))
    }, |checkpoint| {
        if checkpoint.replay.is_some_and(|p| p.status == FileStopRecoveryStatus::Ready) {
            ready.set(checkpoint.domain);
        }
        true
    }).unwrap();
    assert!(pass.all_observed_drained()); assert_eq!(*calls.borrow(), bindings(2));
    for (index, member) in pass.members().iter().enumerate() {
        assert_eq!(member.acquired_tick, Some(ElapsedTick(index as u64 + 3)));
    }
    let completed = [a.image(), b.image()]; drop(c);
    std::fs::write(a.store().join(storage::CANONICAL), &before[0]).unwrap();
    std::fs::write(b.store().join(storage::CANONICAL), &before[1]).unwrap();
    let other = Directory::new(); let mut c = coordinator(&other, &plan);
    let old = c.recover_registered_pass(0, &bindings(2).into_iter().map(|binding| FileShutdownClock {
        domain: binding.domain, clock_domain: binding.clock_domain, at: ElapsedTick(binding.domain + 2),
    }).collect::<Vec<_>>()).unwrap();
    assert_eq!([a.image(), b.image()], completed);
    for (new, old) in pass.members().iter().zip(old.members()) { assert_eq!(new.result, old.result); }
}

#[test]
fn mixed_resume_uses_no_time_for_retained_visits_or_completed_capacity_retry() {
    let a = Directory::new(); let b = Directory::new(); let d = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 100); let h2 = member(&b, 2, 101); let h3 = member(&d, 3, 102);
    let plan = roster(&[&h1, &h2, &h3], 3); drop((h1, h2, h3));
    let mut c = coordinator(&control, &plan);
    c.recover_and_drain_with_clock(0, bindings(1)[0], 1, |_| Ok(ElapsedTick(2)), |_| true).unwrap().unwrap();
    let (index, attempt) = c.begin(2, 2, ShutdownVisitKind::RecoverStoppedAtCompletion).unwrap();
    c.recover_domain(index, attempt, ElapsedTick(2)).unwrap(); drop(c);
    let settled = [a.image(), b.image()]; let mut c = reopen(&control, &plan, 3);
    let pass = c.resume_registered_pass_with_clocks(3, &bindings(3), 2, |binding| {
        assert_eq!(binding.domain, 3); Ok(ElapsedTick(4))
    }, |_| true).unwrap();
    assert!(pass.all_observed_drained()); assert_eq!(pass.final_revision(), 6);
    assert_eq!(pass.members()[0].acquired_tick, None); assert_eq!(pass.members()[1].acquired_tick, None);
    assert_eq!(pass.members()[2].acquired_tick, Some(ElapsedTick(4)));
    assert_eq!([a.image(), b.image()], settled); let bytes = control.image(); drop(c);
    let mut c = reopen(&control, &plan, 6);
    let pass = c.resume_registered_pass_with_clocks(6, &bindings(3), 1, |_| panic!("receipt-only pass"), |_| true).unwrap();
    assert!(pass.all_observed_drained()); assert_eq!(pass.final_revision(), 6);
    assert!(pass.members().iter().all(|m| m.acquired_tick.is_none())); assert_eq!(control.image(), bytes);
}

#[test]
fn clock_refusal_is_not_a_tick_and_stale_acquisition_is_not_an_acknowledgment() {
    for fail in [false, true] {
        let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 100); let h2 = member(&b, 2, 101);
        let plan = roster(&[&h1, &h2], 2); drop((h1, h2)); let before = a.image();
        let mut c = coordinator(&control, &plan);
        let pass = c.recover_registered_pass_with_clocks(0, &bindings(2), 1, |binding| {
            if binding.domain == 1 {
                if fail { Err(JournalError::Unavailable) } else { Ok(ElapsedTick(0)) }
            } else { Ok(ElapsedTick(2)) }
        }, |_| true).unwrap();
        assert_eq!(pass.members()[0].acquired_tick, if fail { None } else { Some(ElapsedTick(0)) });
        assert_eq!(pass.members()[0].result, FileShutdownPassResult::Refused(
            if fail { JournalError::Unavailable } else { Error::Stale.into() }));
        assert!(matches!(pass.members()[1].result, FileShutdownPassResult::Observed(_)));
        assert_eq!(pass.unobserved_stops(), vec![1]); assert_eq!(a.image(), before);
        assert!(!c.report().unavailable); assert!(!pass.all_observed_drained());
    }
}

#[test]
fn every_checkpoint_cancellation_keeps_unsampled_and_unvisited_members_explicit() {
    for stop in 0..8 {
        let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 100); let h2 = member(&b, 2, 101);
        let plan = roster(&[&h1, &h2], 2); drop((h1, h2)); let before = [a.image(), b.image()];
        let mut c = coordinator(&control, &plan); let polls = Cell::new(0); let clocks = Cell::new(0);
        let pass = c.recover_registered_pass_with_clocks(0, &bindings(2), 1, |_| {
            clocks.set(clocks.get() + 1); Ok(ElapsedTick(2))
        }, |_| { let index = polls.get(); polls.set(index + 1); index != stop }).unwrap();
        assert_eq!(clocks.get(), stop / 4); assert!(!pass.all_observed_drained());
        for (i, member) in pass.members().iter().enumerate() {
            if i < stop / 4 { assert_eq!(member.acquired_tick, Some(ElapsedTick(2))); }
            else {
                assert_eq!(member.acquired_tick, None);
                assert_eq!([a.image(), b.image()][i], before[i]);
                if i > stop / 4 || stop % 4 == 0 { assert_eq!(member.result, FileShutdownPassResult::NotVisited); }
                else { assert_eq!(member.result, FileShutdownPassResult::Refused(JournalError::Unavailable)); }
            }
        }
        assert!(!c.report().unavailable);
    }
}

#[test]
fn complete_roster_and_allowances_are_checked_before_any_clock_or_checkpoint() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 100); let h2 = member(&b, 2, 101);
    let plan = roster(&[&h1, &h2], 2); drop((h1, h2)); let mut c = coordinator(&control, &plan);
    let before = control.image();
    let mut wrong = bindings(2); wrong[1].clock_domain = 500;
    for (inputs, quantum, error) in [(bindings(1), 1, Error::Binding),
        (vec![bindings(1)[0]; 2], 1, Error::Duplicate), (wrong, 1, Error::Binding),
        (bindings(2), 0, Error::InvalidInput), (bindings(2), MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(c.recover_registered_pass_with_clocks(0, &inputs, quantum,
            |_| panic!("admission first"), |_| panic!("admission first")), Err(error.into()));
        assert_eq!(control.image(), before);
    }
    assert!(c.recover_registered_pass_with_clocks(0, &bindings(2), 1, |_| Ok(ElapsedTick(2)), |_| true).unwrap().all_observed_drained());
    let before = control.image();
    assert_eq!(c.resume_registered_pass_with_clocks(4, &bindings(2), 1,
        |_| panic!("spent session allowance"), |_| panic!("spent session allowance")), Err(Error::Limit.into()));
    assert_eq!(control.image(), before);
}

#[test]
fn intent_write_failure_never_calls_time_and_preserves_the_remaining_roster() {
    for barrier in BARRIERS {
        let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 100); let h2 = member(&b, 2, 101);
        let plan = roster(&[&h1, &h2], 2); drop((h1, h2)); let before = [a.image(), b.image()];
        let mut c = coordinator(&control, &plan); c.store.fail_once(barrier);
        let pass = c.recover_registered_pass_with_clocks(0, &bindings(2), 1,
            |_| panic!("intent did not acknowledge"), |_| true).unwrap();
        assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::Unacknowledged(
            JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(pass.members()[1].result, FileShutdownPassResult::NotVisited);
        assert!(pass.members().iter().all(|m| m.acquired_tick.is_none()));
        assert!(c.report().unavailable); assert_eq!([a.image(), b.image()], before);
    }
}

#[test]
fn second_clock_unwind_preserves_first_settlement_without_retrying_second_intent() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 100); let h2 = member(&b, 2, 101);
    let plan = roster(&[&h1, &h2], 2); drop((h1, h2)); let before = b.image();
    let mut c = coordinator(&control, &plan);
    assert!(catch_unwind(AssertUnwindSafe(|| c.recover_registered_pass_with_clocks(0, &bindings(2), 1, |binding| {
        if binding.domain == 2 { panic!("clock interrupted after earlier settlement"); }
        Ok(ElapsedTick(2))
    }, |_| true))).is_err());
    assert!(c.report().unavailable); assert_eq!(c.revision(), 3); assert_eq!(b.image(), before);
    let first = a.image(); drop(c); let mut c = reopen(&control, &plan, 3);
    let pass = c.resume_registered_pass_with_clocks(3, &bindings(2), 1, |_| panic!("no command retry"), |_| true).unwrap();
    assert!(matches!(pass.members()[0].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(pass.members()[1].result, FileShutdownPassResult::EvidenceRefused(Error::Incomplete.into()));
    assert!(pass.members().iter().all(|m| m.acquired_tick.is_none()));
    assert_eq!(a.image(), first); assert_eq!(b.image(), before);
}
