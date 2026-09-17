//! Actual domain effects and coordinator restart, not asserted stop receipts.
#![cfg(unix)]
#[path = "support/file_shutdown.rs"] mod support;
use support::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{JournalError, MAX_JOURNAL_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::*;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::*;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::Error;

#[test]
fn process_restart_keeps_every_domain_visit_and_native_effect_liability() {
    let aroot = Directory::new(); let broot = Directory::new(); let controller = Directory::new();
    let (mut a, ahuman) = create(&aroot, 1); let (mut b, bhuman) = create(&broot, 2);
    let executed = ready(&mut a, &ahuman, 1, 1, b"already visible"); dispatch(&mut a, &executed);
    a.publish_checked(a.revision(), 1, Some(&executed.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let _unsent = ready(&mut a, &ahuman, 1, 2, b"not dispatched");
    let queued = ready(&mut b, &bhuman, 2, 1, b"not visible"); dispatch(&mut b, &queued);
    let p = plan(vec![a.shutdown_domain(1).unwrap(), b.shutdown_domain(2).unwrap()]);
    let plan_image = p.encode().unwrap();
    let mut c = FileShutdownCoordinator::create(controller.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    c.advance(c.revision(), 1, &mut a, ElapsedTick(2)).unwrap().unwrap();
    assert_eq!(a.inspect().control.ledger.reserved, 0);
    assert_eq!(a.inspect().control.ledger.charged, 16);
    c.advance(c.revision(), 1, &mut a, ElapsedTick(2)).unwrap().unwrap();
    assert_eq!(a.inspect().control.ledger.charged, 16);
    c.unavailable(c.revision(), 2, JournalError::Unavailable).unwrap();
    assert_eq!(c.report().visits.len(), 3); assert!(!c.report().all_observed_drained());
    let floor = c.revision(); drop(c); drop(a); drop(b); drop(p);
    // Reconstruct operator configuration and exact saved plan, not live objects.
    let p = FileShutdownPlan::decode(&plan_image, vec![
        FileShutdownMember { id: 1, directory: aroot.store(), profile: profile(1) },
        FileShutdownMember { id: 2, directory: broot.store(), profile: profile(2) },
    ]).unwrap();
    let mut c = FileShutdownCoordinator::open(controller.store(), &p, MAX_JOURNAL_BYTES, floor).unwrap();
    assert_eq!(c.report().visits.len(), 3);
    assert_eq!(c.report().current_session.unobserved_stops(), vec![1, 2]);
    assert!(matches!(c.report().visits[2].result, ShutdownVisitResult::Refused { .. }));
    c.inspect_canonical(c.revision(), 1).unwrap().unwrap();
    let (mut b, _) = FileOversight::open(broot.store(), profile(2)).unwrap();
    c.advance(c.revision(), 2, &mut b, ElapsedTick(2)).unwrap().unwrap();
    c.advance(c.revision(), 2, &mut b, ElapsedTick(2)).unwrap().unwrap();
    assert!(c.report().all_observed_drained()); assert_eq!(c.report().visits.len(), 6);
    assert_eq!(b.inspect().control.ledger.charged, 0); assert_eq!(b.inspect().executions, 0);
    let last = &c.report().current_session.domains[0];
    assert_eq!(last.last_observation.as_ref().unwrap().stop.as_ref().unwrap().charged_units, 16);
}

#[test]
fn attempt_budget_and_newer_observed_heads_survive_repeated_reopen() {
    let root = Directory::new(); let control = Directory::new(); let (mut h, _) = create(&root, 1);
    let p = FileShutdownPlan::new(900, vec![h.shutdown_domain(1).unwrap()], 3, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
    let old = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    c.advance(c.revision(), 1, &mut h, ElapsedTick(2)).unwrap().unwrap();
    let floor = c.revision(); drop(c);
    let mut c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, floor).unwrap();
    let canonical = root.store().join("delivery.bin"); let stopped = std::fs::read(&canonical).unwrap();
    std::fs::write(&canonical, old).unwrap();
    assert!(c.inspect_canonical(c.revision(), 1).unwrap().is_err());
    assert_eq!(c.report().visits.len(), 2); assert!(!c.report().all_observed_drained());
    std::fs::write(&canonical, stopped).unwrap();
    c.advance(c.revision(), 1, &mut h, ElapsedTick(2)).unwrap().unwrap();
    assert!(c.report().all_observed_drained()); let floor = c.revision(); drop(c);
    let mut c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, floor).unwrap();
    assert_eq!(c.report().visits.len(), 3); assert!(!c.report().all_observed_drained());
    let before = h.inspect();
    assert_eq!(c.advance(c.revision(), 1, &mut h, ElapsedTick(2)), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(h.inspect(), before); assert_eq!(c.revision(), floor);
}

#[test]
fn coordinator_write_failure_cannot_touch_domain_or_clean_an_unrelated_pending_file() {
    let root = Directory::new(); let control = Directory::new(); let (mut h, _) = create(&root, 1);
    let p = plan(vec![h.shutdown_domain(1).unwrap()]);
    let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    let before = h.inspect();
    std::fs::write(control.store().join("delivery.pending"), b"retain pending coordinator write").unwrap();
    assert!(c.advance(c.revision(), 1, &mut h, ElapsedTick(2)).is_err());
    assert_eq!(h.inspect(), before); assert!(c.report().unavailable);
    assert_eq!(c.advance(c.revision(), 1, &mut h, ElapsedTick(2)), Err(JournalError::Unavailable));
    drop(c);
    let different = FileShutdownPlan::new(901, p.domains().to_vec(), p.max_attempts(), p.max_head_bytes()).unwrap();
    assert!(FileShutdownCoordinator::open(control.store(), &different, MAX_JOURNAL_BYTES, 0).is_err());
    assert_eq!(std::fs::read(control.store().join("delivery.pending")).unwrap(), b"retain pending coordinator write");
    assert!(FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 1).is_err());
    assert!(control.store().join("delivery.pending").exists());
    let mut c = FileShutdownCoordinator::open(control.store(), &p, MAX_JOURNAL_BYTES, 0).unwrap();
    assert!(!control.store().join("delivery.pending").exists());
    c.advance(c.revision(), 1, &mut h, ElapsedTick(2)).unwrap().unwrap();
    assert!(h.inspect().stop.is_some());
}

#[test]
fn original_actor_supervisor_retains_outcome_unknown_until_the_native_drain() {
    let root = Directory::new(); let control = Directory::new(); let (h, human) = create(&root, 1);
    let (actor, mut supervisor) = h.into_actor_gateway();
    let proposal = { let h = supervisor.host().unwrap(); let s = spec(&h, 1, b"actor output");
        ActorProposal { target: s.target.unwrap(), payload: s.payload, expected_policy_epoch: s.policy_epoch,
            deadline: s.deadline, units: s.units } };
    let revision = supervisor.host().unwrap().revision(); supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let ticket = actor.submit(7000, &proposal).unwrap();
    {
        let mut h = supervisor.host_mut().unwrap(); let action = h.request_action(7000).unwrap().clone();
        let keys = ready_existing(&mut h, &human, 1, action); dispatch(&mut h, &keys);
        let revision = h.revision(); h.publish_checked(revision, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    }
    let p = plan(vec![supervisor.host().unwrap().shutdown_domain(1).unwrap()]);
    let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
    c.advance_supervised(c.revision(), 1, &mut supervisor, ElapsedTick(2)).unwrap().unwrap();
    assert!(matches!(actor.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
    c.advance_supervised(c.revision(), 1, &mut supervisor, ElapsedTick(2)).unwrap().unwrap();
    assert!(matches!(actor.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert!(c.report().all_observed_drained());
}
