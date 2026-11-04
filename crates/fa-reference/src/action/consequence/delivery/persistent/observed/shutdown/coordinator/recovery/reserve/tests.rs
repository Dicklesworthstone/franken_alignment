//! Original codec, file stores and native terminal paths; no mock success ledger.
use super::*;
use super::super::test_support::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn kind(mode: usize) -> ShutdownVisitKind {
    match mode {
        0 | 1 => ShutdownVisitKind::RecoverStopped { at: ElapsedTick(2) },
        2 => ShutdownVisitKind::RecoverStoppedAtCompletion,
        3 => ShutdownVisitKind::StopAdmission,
        4 => ShutdownVisitKind::DrainStoppedAtCompletion,
        _ => unreachable!(),
    }
}
fn binding() -> FileShutdownClockBinding { FileShutdownClockBinding { domain: 1, clock_domain: 99 } }
fn refusal_limit(root: &Directory, plan: &FileShutdownPlan, kind: ShutdownVisitKind) -> usize {
    let identity = std::fs::canonicalize(&root.0).unwrap().join("publication");
    let visits = [ShutdownVisit { domain: 1, kind, result: ShutdownVisitResult::Refused {
        diagnostic: "x".repeat(MAX_REFUSAL_BYTES),
    } }];
    codec::encode(&identity, &plan.encode().unwrap(), MAX_JOURNAL_BYTES,
        2, &visits, &plan.start()).unwrap().len()
}
fn blocked(c: &mut FileShutdownCoordinator, mode: usize)
    -> Result<Result<FileShutdownObservation, JournalError>, JournalError>
{
    let revision = c.revision();
    match mode {
        0 => c.recover_and_drain(revision, 1, ElapsedTick(2)),
        1 => match c.begin_recover_and_drain(revision, 1, ElapsedTick(2))? {
            Ok(_) => panic!("the original domain owner still holds its lock"),
            Err(error) => Ok(Err(error)),
        },
        2 => c.recover_and_drain_with_clock(revision, binding(), 1,
            |_| panic!("no clock for inaccessible domain"), |_| panic!("no replay")),
        3 => c.recover_admission_stop(revision, binding(), 1, |_| panic!("no replay")),
        4 => c.drain_stopped_with_clock(revision, binding(), 1,
            |_| panic!("no clock for inaccessible domain"), |_| panic!("no replay")),
        _ => unreachable!(),
    }
}

#[test]
fn exact_refusal_budget_neighbors_cover_every_registered_entry_without_poisoning_rejection() {
    for mode in 0..5 { for missing in [false, true] {
        let root = Directory::new(); let control = Directory::new();
        let host = create(&root, 64); let p = plan(&host, 8);
        let cap = refusal_limit(&control, &p, kind(mode)) - usize::from(missing);
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        let before = c.report(); let canonical = control.image(); let native = root.image();
        let result = blocked(&mut c, mode);
        assert_eq!(root.image(), native);
        assert!(!c.report().unavailable);
        if missing {
            assert_eq!(result, Err(Error::Limit.into()));
            assert_eq!(c.report(), before); assert_eq!(control.image(), canonical);
        } else {
            assert_eq!(result, Ok(Err(JournalError::Busy)));
            assert_eq!(c.revision(), 2);
            assert_eq!(c.report().visits[0].kind, kind(mode));
            assert_eq!(c.report().visits[0].result,
                ShutdownVisitResult::Refused { diagnostic: JournalError::Busy.to_string() });
            drop(c);
            let c = FileShutdownCoordinator::open(control.store(), &p, cap, 2).unwrap();
            assert!(!c.report().all_observed_drained()); assert_eq!(c.revision(), 2);
        }
    }}
}

#[test]
fn admitted_recovery_still_runs_the_original_native_positive_paths() {
    for mode in 0..5 {
        let root = Directory::new(); let control = Directory::new();
        let mut host = create(&root, 64);
        if mode == 4 {
            let stop = request(&host, 900);
            host.request_stop(host.revision(), stop).unwrap();
        }
        let p = plan(&host, 8); drop(host);
        let mut c = FileShutdownCoordinator::create(control.store(), &p, MAX_JOURNAL_BYTES).unwrap();
        let result = match mode {
            0 => c.recover_and_drain(0, 1, ElapsedTick(2)),
            1 => {
                let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
                while visit.progress().status == FileStopRecoveryStatus::Replaying {
                    visit.advance(visit.progress().replayed_events, 1).unwrap();
                }
                visit.finish()
            }
            2 => c.recover_and_drain_with_clock(0, binding(), 1, |_| Ok(ElapsedTick(2)), |_| true),
            3 => c.recover_admission_stop(0, binding(), 1, |_| true),
            4 => c.drain_stopped_with_clock(0, binding(), 1, |_| Ok(ElapsedTick(2)), |_| true),
            _ => unreachable!(),
        }.unwrap().unwrap();
        assert_eq!(result.stop.as_ref().unwrap().receipt.request().operation, 900);
        assert_eq!(c.revision(), 2); assert!(!c.report().unavailable);
        assert_eq!(c.report().visits[0].result,
            ShutdownVisitResult::Observed { domain_revision: result.journal_revision });
        let (passive, _, _) = c.campaign.read_canonical(0).unwrap();
        let mut expected = result; expected.source = FileShutdownSource::CanonicalImage;
        assert_eq!(passive, expected);
    }
}

#[test]
fn admitted_cancellation_can_record_its_refusal_at_the_exact_budget() {
    for cut in 0..=2 {
        let root = Directory::new(); let control = Directory::new();
        let host = create(&root, 64); let p = plan(&host, 8); let native = root.image(); drop(host);
        let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"preserve").unwrap();
        let cap = refusal_limit(&control, &p, kind(1));
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        let mut visit = c.begin_recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
        for _ in 0..cut { visit.advance(visit.progress().replayed_events, 1).unwrap(); }
        visit.cancel().unwrap();
        assert_eq!(root.image(), native); assert_eq!(std::fs::read(&pending).unwrap(), b"preserve");
        assert_eq!(c.revision(), 2); assert!(!c.report().unavailable);
        assert_eq!(c.report().visits[0].result,
            ShutdownVisitResult::Refused { diagnostic: JournalError::Unavailable.to_string() });
    }
}

#[test]
fn intent_storage_failures_and_callback_unwind_retain_original_uncertainty() {
    for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new();
        let host = create(&root, 64); let p = plan(&host, 8); let native = root.image(); drop(host);
        let cap = refusal_limit(&control, &p, kind(2));
        let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
        c.store.fail_once(barrier);
        let result = c.recover_and_drain_with_clock(0, binding(), 1,
            |_| panic!("unacknowledged intent"), |_| panic!("no replay"));
        assert!(matches!(result, Err(JournalError::Io(ref e)) if e.operation == barrier));
        assert!(c.report().unavailable); assert_eq!(root.image(), native); drop(c);
        let c = FileShutdownCoordinator::open(control.store(), &p, cap, 0).unwrap();
        assert_eq!(c.report().visits.len(), usize::from(barrier == JournalIo::DirectorySync));
        assert!(!c.report().all_observed_drained());
    }
    let root = Directory::new(); let control = Directory::new();
    let host = create(&root, 64); let p = plan(&host, 8); let native = root.image(); drop(host);
    let cap = refusal_limit(&control, &p, kind(3));
    let mut c = FileShutdownCoordinator::create(control.store(), &p, cap).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| {
        let _ = c.recover_admission_stop(0, binding(), 1, |_| panic!("checkpoint interrupted"));
    })).is_err());
    assert!(c.report().unavailable); assert_eq!(c.revision(), 1);
    assert_eq!(c.report().visits[0].result, ShutdownVisitResult::Entered);
    assert_eq!(root.image(), native);
}
