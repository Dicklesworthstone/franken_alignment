//! Full roster, original native effects and preserved bounded visit allowances.
use super::*;
use super::super::super::super::test_support::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
#[path = "../../../../../recovery/fixture.rs"]
mod effects;

fn bindings(count: u64) -> Vec<FileShutdownClockBinding> {
    (1..=count).map(|domain| FileShutdownClockBinding { domain, clock_domain: 99 }).collect()
}
fn roster(hosts: &[&FileOversight], visits: usize) -> FileShutdownPlan {
    FileShutdownPlan::new(900, hosts.iter().enumerate().map(|(index, host)|
        host.shutdown_domain(index as u64 + 1).unwrap()).collect(), visits, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}
fn coordinator(root: &Directory, p: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), p, MAX_JOURNAL_BYTES).unwrap()
}
fn observed(member: &FileShutdownClockedPassMember) -> &FileShutdownObservation {
    let FileShutdownPassResult::Observed(value) = &member.result else { panic!("original observation"); };
    value
}

#[test]
fn selective_drain_finishes_at_the_visit_ceiling_without_revisiting_executed_work() {
    for expired in [false, true] {
        let roots = [effects::Directory::new(), effects::Directory::new()];
        let control = Directory::new();
        let mut live: Vec<_> = roots.iter().enumerate().map(|(index, root)| {
            let mut profile = effects::profile(); profile.delivery.scope.run = index as u64 + 1;
            effects::Ready::new(root, profile, true)
        }).collect();
        let p = roster(&live.iter().map(|live| &live.host).collect::<Vec<_>>(), 3);
        for ready in &mut live { ready.dispatch(); }
        let revision = live[0].host.revision(); live[0].host.publish(revision, 1).unwrap();
        drop(live);
        let mut c = coordinator(&control, &p);
        c.recover_and_drain_with_clock(0, bindings(2)[0], 1, |_| Ok(ElapsedTick(2)), |_| true).unwrap().unwrap();
        c.recover_admission_stop(c.revision(), bindings(2)[1], 1, |_| true).unwrap().unwrap();
        let first = roots[0].bytes(); let second = roots[1].bytes();
        let initial = c.history_anchor().unwrap(); drop(c);
        let mut c = FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 4, &initial).unwrap();
        let journal = control.image();
        assert_eq!(c.drain_registered_stopped_with_clocks(4, &bindings(2), 1,
            |_| panic!("all-member work cannot fit"), |_| panic!("admission precedes callbacks")), Err(Error::Limit.into()));
        assert_eq!(control.image(), journal);
        let at = ElapsedTick(if expired { 2000 } else { 3 }); let mut calls = Vec::new();
        let pass = c.drain_selected_stopped_with_clocks(4, &bindings(2), &[2], 1,
            |binding| { calls.push(binding.domain); Ok(at) }, |_| true).unwrap();
        assert_eq!(calls, vec![2]); assert_eq!(pass.members().len(), 2);
        assert!(pass.all_observed_stopped()); assert_eq!(pass.all_observed_drained(), !expired);
        assert_eq!(observed(&pass.members()[0]).executions, 1);
        assert_eq!(observed(&pass.members()[0]).stop.as_ref().unwrap().charged_units, 16);
        assert_eq!(observed(&pass.members()[1]).executions, 0);
        assert_eq!(observed(&pass.members()[1]).stop.as_ref().unwrap().charged_units, if expired { 16 } else { 0 });
        assert_eq!(pass.members()[0].acquired_tick, None);
        assert_eq!(pass.members()[1].acquired_tick, Some(at));
        assert_eq!(roots[0].bytes(), first); assert_ne!(roots[1].bytes(), second);
        assert_eq!(c.report().visits.len(), 3); assert_eq!(c.revision(), 6);
        let anchor = c.history_anchor_after(&initial).unwrap(); let saved = control.image(); drop(c);
        let mut c = FileShutdownCoordinator::open_anchored(control.store(), &p, MAX_JOURNAL_BYTES, 6, &anchor).unwrap();
        let repeat = c.resume_registered_pass_with_clocks(6, &bindings(2), 1,
            |_| panic!("saved receipt is not a new drain"), |_| true).unwrap();
        assert_eq!(repeat.all_observed_drained(), !expired); assert_eq!(control.image(), saved);
    }
}

#[test]
fn combined_and_clock_free_selection_reuse_original_operations_and_keep_all_members() {
    for admission in [false, true] {
        let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 2); drop((h1, h2));
        let mut c = coordinator(&control, &p);
        c.recover_admission_stop(0, bindings(2)[0], 1, |_| true).unwrap().unwrap();
        let first = a.image(); drop(c); let mut c = reopen(&control, &p, 2);
        let mut identities = bindings(2); identities.reverse();
        let mut calls = Vec::new();
        let pass = if admission { c.stop_selected_admission(2, &identities, &[2], 2, |_| true) }
            else { c.recover_selected_pass_with_clocks(2, &identities, &[2], 2,
                |binding| { calls.push(binding.domain); Ok(ElapsedTick(2)) }, |_| true) }.unwrap();
        assert!(pass.all_observed_stopped()); assert_eq!(pass.members().len(), 2);
        assert_eq!(pass.members()[0].binding.domain, 1); assert_eq!(a.image(), first);
        assert_eq!(calls, if admission { Vec::<u64>::new() } else { vec![2] });
        assert_eq!(c.report().visits.len(), 2); assert_eq!(c.report().current_session.attempts.len(), 2);
    }
}

#[test]
fn selection_and_full_roster_are_admitted_before_callbacks_or_any_work() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 8); drop((h1, h2));
    let mut c = coordinator(&control, &p); let original = control.image();
    for (selected, expected) in [(vec![], Error::InvalidInput), (vec![1, 1], Error::Duplicate),
        (vec![9], Error::Missing), (vec![1, 2, 3], Error::Limit), (vec![2], Error::WrongState)] {
        assert_eq!(c.stop_selected_admission(0, &bindings(2), &selected, 1,
            |_| panic!("invalid selection")), Err(expected.into()));
        assert_eq!(control.image(), original); assert!(c.report().current_session.attempts.is_empty());
    }
    let mut wrong = bindings(2); wrong[0].clock_domain = 100;
    for bad in [bindings(1), wrong, vec![bindings(2)[0]; 2]] {
        assert!(c.stop_selected_admission(0, &bad, &[1, 2], 1, |_| panic!("invalid roster")).is_err());
        assert_eq!(control.image(), original);
    }
    assert_eq!(c.stop_selected_admission(0, &bindings(2), &[1, 2], 0, |_| panic!("zero quantum")), Err(Error::InvalidInput.into()));
    assert!(c.stop_selected_admission(0, &bindings(2), &[2, 1], 1, |_| true).unwrap().all_observed_stopped());
    let saved = control.image();
    c.unavailable(c.revision(), 1, JournalError::Unavailable).unwrap();
    let refused = control.image(); assert_ne!(refused, saved);
    assert_eq!(c.stop_selected_admission(c.revision(), &bindings(2), &[2], 1,
        |_| panic!("latest refusal cannot expose an older convenient success")), Err(Error::WrongState.into()));
    assert_eq!(control.image(), refused);
}

#[test]
fn missing_unselected_receipt_never_turns_into_a_command_and_does_not_hide_reachable_work() {
    for missing in [false, true] {
        let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 4); drop((h1, h2));
        let mut c = coordinator(&control, &p);
        c.recover_admission_stop(0, bindings(2)[0], 1, |_| true).unwrap().unwrap();
        let first = a.image();
        if missing { std::fs::remove_file(a.store().join(storage::CANONICAL)).unwrap(); }
        let pass = c.stop_selected_admission(2, &bindings(2), &[2], 1, |_| true).unwrap();
        assert_eq!(pass.all_observed_stopped(), !missing);
        assert!(matches!(pass.members()[1].result, FileShutdownPassResult::Observed(_)));
        assert_eq!(c.report().visits.len(), 2); assert!(!c.report().unavailable);
        if missing {
            assert!(matches!(pass.members()[0].result, FileShutdownPassResult::EvidenceRefused(_)));
            assert_eq!(pass.unobserved_stops(), vec![1]);
            assert!(!a.store().join(storage::CANONICAL).exists());
        } else { assert_eq!(a.image(), first); }
    }
}

#[test]
fn unselected_entered_work_is_only_resolved_not_reissued() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 3); drop((h1, h2));
    let mut c = coordinator(&control, &p);
    assert!(catch_unwind(AssertUnwindSafe(|| c.recover_admission_stop(0, bindings(2)[0], 1,
        |_| panic!("entered but no native stop")))).is_err());
    let first = a.image(); drop(c); let mut c = reopen(&control, &p, 1);
    let pass = c.stop_selected_admission(1, &bindings(2), &[2], 1, |_| true).unwrap();
    assert_eq!(pass.members()[0].result, FileShutdownPassResult::EvidenceRefused(Error::Incomplete.into()));
    assert!(matches!(pass.members()[1].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(a.image(), first); assert!(matches!(c.report().visits[0].result, ShutdownVisitResult::Entered));
    assert_eq!(c.report().visits.len(), 2); assert!(!pass.all_observed_stopped());
}

#[test]
fn cancellation_at_every_checkpoint_preserves_full_roster_and_does_not_acquire_a_tick() {
    let mut cuts = 0;
    for cut in 0..128 {
        if cut != 0 && cut > cuts { break; }
        let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 4); drop((h1, h2));
        let mut c = coordinator(&control, &p);
        c.recover_admission_stop(0, bindings(2)[0], 1, |_| true).unwrap().unwrap();
        let saved = [a.image(), b.image()]; let mut seen = 0; let mut clocks = 0;
        let pass = c.recover_selected_pass_with_clocks(2, &bindings(2), &[2], 1,
            |_| { clocks += 1; Ok(ElapsedTick(2)) }, |_| { seen += 1; cut == 0 || seen < cut }).unwrap();
        if cut == 0 { cuts = seen; assert!(cuts < 128); assert!(pass.all_observed_stopped()); }
        else {
            assert_eq!(seen, cut); assert_eq!(clocks, 0); assert!(!pass.all_observed_stopped());
            assert_eq!([a.image(), b.image()], saved); assert!(!c.report().unavailable);
        }
        assert_eq!(pass.members().len(), 2);
    }
}

#[test]
fn failed_selected_intent_retains_prior_observation_and_an_explicit_unvisited_tail() {
    for barrier in BARRIERS {
        let a = Directory::new(); let b = Directory::new(); let d = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let h3 = member(&d, 3, 99);
        let p = roster(&[&h1, &h2, &h3], 6); drop((h1, h2, h3)); let mut c = coordinator(&control, &p);
        c.recover_admission_stop(0, bindings(3)[0], 1, |_| true).unwrap().unwrap();
        let saved = [a.image(), b.image(), d.image()]; c.store.fail_once(barrier);
        let pass = c.stop_selected_admission(2, &bindings(3), &[3, 2], 1, |_| true).unwrap();
        assert!(matches!(pass.members()[0].result, FileShutdownPassResult::Observed(_)));
        assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::Unacknowledged(JournalError::Io(f)) if f.operation == barrier));
        assert_eq!(pass.members()[2].result, FileShutdownPassResult::NotVisited);
        assert_eq!([a.image(), b.image(), d.image()], saved); assert!(c.report().unavailable);
        assert!(!pass.all_observed_stopped());
    }
}

#[test]
fn selected_clock_unwind_requires_reopen_and_passive_resume_not_a_hidden_retry() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 4); drop((h1, h2));
    let mut c = coordinator(&control, &p);
    c.recover_admission_stop(0, bindings(2)[0], 1, |_| true).unwrap().unwrap();
    let second = b.image();
    assert!(catch_unwind(AssertUnwindSafe(|| c.recover_selected_pass_with_clocks(2, &bindings(2), &[2], 1,
        |_| panic!("selected terminal clock"), |_| true))).is_err());
    assert!(c.report().unavailable); assert_eq!(b.image(), second); drop(c);
    let mut c = reopen(&control, &p, 3);
    let pass = c.resume_registered_pass_with_clocks(3, &bindings(2), 1,
        |_| panic!("entered work must remain evidence-only on resume"), |_| true).unwrap();
    assert_eq!(pass.members()[1].result, FileShutdownPassResult::EvidenceRefused(Error::Incomplete.into()));
    assert_eq!(b.image(), second); assert_eq!(c.report().visits.len(), 2);
}
