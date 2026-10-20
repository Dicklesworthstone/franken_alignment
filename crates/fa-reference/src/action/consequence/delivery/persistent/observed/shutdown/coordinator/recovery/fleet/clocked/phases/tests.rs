//! Full-roster phase behavior through the original coordinator and native owners.
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

#[test]
fn complete_roster_stops_without_time_then_drains_with_independent_late_ticks() {
    let roots = [effects::Directory::new(), effects::Directory::new()]; let control = Directory::new();
    let mut live: Vec<_> = roots.iter().enumerate().map(|(index, root)| {
        let mut profile = effects::profile(); profile.delivery.scope.run = index as u64 + 1;
        effects::Ready::new(root, profile, true)
    }).collect();
    let p = roster(&live.iter().map(|live| &live.host).collect::<Vec<_>>(), 4);
    for ready in &mut live { ready.dispatch(); }
    let revisions: Vec<_> = live.iter().map(|live| live.host.revision()).collect(); drop(live);
    let mut c = coordinator(&control, &p);
    let stopped = c.stop_registered_admission(0, &bindings(2), 1, |_| true).unwrap();
    assert!(stopped.all_observed_stopped()); assert!(!stopped.all_observed_drained());
    for (index, member) in stopped.members().iter().enumerate() {
        assert_eq!(member.acquired_tick, None);
        let FileShutdownPassResult::Observed(observation) = &member.result else { panic!("stop"); };
        assert_eq!(observation.journal_revision, revisions[index] + 2);
        assert_eq!(observation.stop.as_ref().unwrap().charged_units, 16);
        assert!(observation.last_drain.is_none());
    }
    let mut clocks = Vec::new();
    let drained = c.drain_registered_stopped_with_clocks(c.revision(), &bindings(2), 1,
        |binding| { clocks.push(binding.domain); Ok(ElapsedTick(binding.domain + 1)) }, |_| true).unwrap();
    assert_eq!(clocks, vec![1, 2]); assert!(drained.all_observed_drained());
    for (index, member) in drained.members().iter().enumerate() {
        assert_eq!(member.acquired_tick, Some(ElapsedTick(index as u64 + 2)));
        let FileShutdownPassResult::Observed(observation) = &member.result else { panic!("drain"); };
        assert_eq!(observation.journal_revision, revisions[index] + 3);
        assert_eq!(observation.stop.as_ref().unwrap().charged_units, 0);
    }
    let saved = [roots[0].bytes(), roots[1].bytes()]; let journal = control.image(); drop(c);
    let mut c = reopen(&control, &p, 8);
    assert!(c.resume_registered_stopped_with_clocks(8, &bindings(2), 1,
        |_| panic!("retained resolution needs no time"), |_| true).unwrap().all_observed_drained());
    assert_eq!([roots[0].bytes(), roots[1].bytes()], saved); assert_eq!(control.image(), journal);
}

#[test]
fn active_drain_refuses_without_time_while_later_members_remain_reachable() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 8); drop((h1, h2));
    let mut c = coordinator(&control, &p);
    c.recover_admission_stop(0, bindings(2)[1], 1, |_| true).unwrap().unwrap();
    let first = a.image(); let second = b.image();
    let pass = c.drain_registered_stopped_with_clocks(c.revision(), &bindings(2), 1,
        |_| panic!("active or already drained"), |_| true).unwrap();
    assert_eq!(pass.members()[0].result, FileShutdownPassResult::Refused(Error::WrongState.into()));
    assert!(matches!(pass.members()[1].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(a.image(), first); assert_eq!(b.image(), second); assert_eq!(pass.unobserved_stops(), vec![1]);
    assert!(c.stop_registered_admission(c.revision(), &bindings(2), 1, |_| true).unwrap().all_observed_stopped());
}

#[test]
fn mixed_resume_at_visit_ceiling_resolves_evidence_without_retrying_entered_stop() {
    let a = Directory::new(); let b = Directory::new(); let d = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let h3 = member(&d, 3, 99);
    let p = roster(&[&h1, &h2, &h3], 3); drop((h1, h2, h3)); let mut c = coordinator(&control, &p);
    c.recover_admission_stop(0, bindings(3)[0], 1, |_| true).unwrap().unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| c.recover_admission_stop(c.revision(), bindings(3)[1], 1,
        |_| panic!("entered before stop")))).is_err());
    let first = a.image(); let second = b.image(); let third = d.image(); drop(c);
    let mut c = reopen(&control, &p, 3);
    let pass = c.resume_registered_admission(3, &bindings(3), 1, |_| true).unwrap();
    assert!(matches!(pass.members()[0].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(pass.members()[1].result, FileShutdownPassResult::EvidenceRefused(Error::Incomplete.into()));
    assert!(matches!(pass.members()[2].result, FileShutdownPassResult::Observed(_)));
    assert_eq!(pass.unobserved_stops(), vec![2]); assert_eq!(c.report().visits.len(), 3);
    assert_eq!(a.image(), first); assert_eq!(b.image(), second); assert_ne!(d.image(), third);
    assert!(pass.members().iter().all(|member| member.acquired_tick.is_none()));
}

#[test]
fn each_checkpoint_keeps_the_unfinished_roster_visible_without_acquiring_time() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 2); drop((h1, h2));
    let mut c = coordinator(&control, &p); let mut calls = 0;
    let _ = c.stop_registered_admission(0, &bindings(2), 1, |_| { calls += 1; true }).unwrap();
    for cut in 1..=calls {
        let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 2); drop((h1, h2));
        let mut c = coordinator(&control, &p); let mut seen = 0;
        let pass = c.stop_registered_admission(0, &bindings(2), 1, |_| { seen += 1; seen < cut }).unwrap();
        assert_eq!(seen, cut); assert!(!pass.all_observed_stopped()); assert_eq!(pass.members().len(), 2);
        assert!(pass.members().iter().all(|member| member.acquired_tick.is_none()));
        assert!(!c.report().unavailable);
    }
}

#[test]
fn entire_identity_and_capacity_admission_precedes_every_callback() {
    let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
    let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 2); drop((h1, h2));
    let mut c = coordinator(&control, &p); let saved = control.image();
    let mut foreign = bindings(2); foreign[1].clock_domain = 100;
    for bad in [bindings(1), vec![bindings(1)[0]; 2], foreign] {
        assert!(c.stop_registered_admission(0, &bad, 1, |_| panic!("invalid roster")).is_err());
        assert_eq!(control.image(), saved);
    }
    c.recover_admission_stop(0, bindings(2)[0], 1, |_| true).unwrap().unwrap();
    assert_eq!(c.stop_registered_admission(c.revision(), &bindings(2), 1, |_| panic!("visit allowance")), Err(Error::Limit.into()));
    drop(c); let mut c = reopen(&control, &p, 2);
    assert!(c.resume_registered_admission(2, &bindings(2), 1, |_| true).unwrap().all_observed_stopped());
}

#[test]
fn failed_intent_acknowledgment_never_starts_a_domain_or_hides_the_tail() {
    for barrier in BARRIERS {
        let a = Directory::new(); let b = Directory::new(); let control = Directory::new();
        let h1 = member(&a, 1, 99); let h2 = member(&b, 2, 99); let p = roster(&[&h1, &h2], 4); drop((h1, h2));
        let mut c = coordinator(&control, &p); let before = [a.image(), b.image()];
        c.store.fail_once(barrier);
        let pass = c.stop_registered_admission(0, &bindings(2), 1, |_| true).unwrap();
        assert!(matches!(&pass.members()[0].result, FileShutdownPassResult::Unacknowledged(JournalError::Io(f)) if f.operation == barrier));
        assert_eq!(pass.members()[1].result, FileShutdownPassResult::NotVisited);
        assert_eq!([a.image(), b.image()], before); assert!(c.report().unavailable);
    }
}
