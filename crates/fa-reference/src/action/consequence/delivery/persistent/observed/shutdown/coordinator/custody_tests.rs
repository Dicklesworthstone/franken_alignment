//! Real original journals and Store barriers; out-of-band writes deliberately
//! violate the cooperating lock to exercise detected custody loss, not CAS.
use super::*;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStopRecoveryStatus;
use std::{cell::RefCell, fs};
#[allow(dead_code)]
#[path = "recovery/test_support.rs"]
mod fixture;
use fixture::{BARRIERS, Directory};
#[allow(dead_code)]
#[path = "../recovery/fixture.rs"]
mod effect_fixture;

fn coordinator(root: &Directory, plan: &FileShutdownPlan) -> FileShutdownCoordinator {
    FileShutdownCoordinator::create(root.store(), plan, MAX_JOURNAL_BYTES).unwrap()
}
fn image(root: &Directory) -> Vec<u8> { root.image() }
fn replace(root: &Directory, bytes: &[u8]) {
    fs::write(root.store().join(storage::CANONICAL), bytes).unwrap();
}
fn binding() -> FileShutdownClockBinding {
    FileShutdownClockBinding { domain: 1, clock_domain: 99 }
}
fn assert_quarantined(c: &FileShutdownCoordinator) {
    assert!(c.report().unavailable);
    assert!(!c.report().all_observed_drained());
    assert_eq!(c.history_anchor(), Err(JournalError::Unavailable));
}

#[test]
fn valid_rollback_and_equal_revision_fork_refuse_all_new_work_before_domain_access() {
    for fork in [false, true] { for route in 0..7 { for staged in [false, true] {
        let root = Directory::new(); let control = Directory::new();
        let mut host = fixture::create(&root, 64); let plan = fixture::plan(&host, 16);
        let native = image(&root); let mut c = coordinator(&control, &plan);
        let initial = image(&control);
        c.unavailable(0, 1, JournalError::Unavailable).unwrap();
        let acknowledged = image(&control); let anchor = c.history_anchor().unwrap();
        let revision = c.revision();
        let changed = if fork {
            let mut visits = c.visits.clone();
            visits[0].result = ShutdownVisitResult::Refused { diagnostic: "Binding".to_owned() };
            codec::encode(c.store.identity(), &c.plan_bytes, c.max_bytes,
                c.revision(), &visits, &c.campaign).unwrap()
        } else { initial };
        // The substitute is a valid ORIGINAL image, not merely malformed bytes.
        codec::decode(c.store.identity(), &plan, &c.plan_bytes, c.max_bytes, 0, &changed).unwrap();
        replace(&control, &changed);
        let pending = control.store().join("delivery.pending");
        if staged { fs::write(&pending, b"preserve").unwrap(); }
        let result = match route {
            0 => c.recover_and_drain(revision, 1, ElapsedTick(2)).map(|_| ()),
            1 => c.begin_recover_and_drain(revision, 1, ElapsedTick(2)).map(|_| ()),
            2 => c.recover_and_drain_with_clock(revision, binding(), 1,
                |_| panic!("no clock after custody loss"), |_| panic!("no domain replay")).map(|_| ()),
            3 => c.recover_admission_stop(revision, binding(), 1,
                |_| panic!("no domain replay")).map(|_| ()),
            4 => c.drain_stopped_with_clock(revision, binding(), 1,
                |_| panic!("no clock"), |_| panic!("no domain replay")).map(|_| ()),
            5 => c.advance(revision, 1, &mut host, ElapsedTick(2)).map(|_| ()),
            _ => c.inspect_canonical(revision, 1).map(|_| ()),
        };
        assert_eq!(result, Err(Error::Binding.into()), "fork={fork} route={route}");
        assert_eq!(image(&control), changed); assert_eq!(image(&root), native);
        if staged { assert_eq!(fs::read(&pending).unwrap(), b"preserve"); }
        else { assert!(!pending.exists()); }
        assert_eq!(c.revision(), revision); assert_quarantined(&c);
        replace(&control, &acknowledged);
        assert_eq!(c.unavailable(revision, 1, JournalError::Unavailable), Err(JournalError::Unavailable));
        drop(c); drop(host);
        // A genuinely restored cut plus explicit anchored reopen can progress.
        let mut c = FileShutdownCoordinator::open_anchored(control.store(), &plan,
            MAX_JOURNAL_BYTES, revision, &anchor).unwrap();
        let stop = c.recover_admission_stop(revision, binding(), 1, |_| true).unwrap().unwrap();
        assert!(stop.stop.is_some()); assert!(!c.report().unavailable);
    }}}
}

#[test]
fn failed_intent_and_completion_writes_never_advance_the_cached_acknowledged_image() {
    for completion in [false, true] { for barrier in BARRIERS {
        let root = Directory::new(); let control = Directory::new();
        let mut host = fixture::create(&root, 64); let plan = fixture::plan(&host, 16);
        let mut c = coordinator(&control, &plan); let anchor = c.history_anchor().unwrap();
        assert_eq!(c.acknowledged_image.as_deref(), Some(image(&control).as_slice()));
        let saved;
        let result = if completion {
            let pair = c.begin(0, 1, ShutdownVisitKind::Advance { at: ElapsedTick(2) }).unwrap();
            saved = image(&control);
            let native = c.campaign.advance_inner(pair.0, pair.1, &mut host, ElapsedTick(2));
            c.store.fail_once(barrier);
            c.finish(pair.0, pair.1, native)
        } else {
            saved = image(&control); c.store.fail_once(barrier);
            c.advance(0, 1, &mut host, ElapsedTick(2))
        };
        assert!(matches!(result, Err(JournalError::Io(ref failure)) if failure.operation == barrier));
        assert_eq!(c.acknowledged_image.as_deref(), Some(saved.as_slice()));
        assert_eq!(image(&control) != saved, barrier == JournalIo::DirectorySync);
        assert_quarantined(&c); let visible = image(&control); drop(c);
        let mut c = FileShutdownCoordinator::open_anchored(control.store(), &plan,
            MAX_JOURNAL_BYTES, 0, &anchor).unwrap();
        assert_eq!(c.acknowledged_image.as_deref(), Some(visible.as_slice()));
        c.inspect_canonical(c.revision(), 1).unwrap().unwrap();
        assert_eq!(c.acknowledged_image.as_deref(), Some(image(&control).as_slice()));
    }}
}

#[test]
fn late_coordinator_replacement_cannot_hide_native_settlement_or_refund_liabilities() {
    for mode in 0..4 {
        let root = effect_fixture::Directory::new(); let control = Directory::new();
        let mut live = effect_fixture::Ready::new(&root, effect_fixture::profile(), true);
        let plan = FileShutdownPlan::new(900, vec![live.host.shutdown_domain(1).unwrap()],
            16, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
        if mode > 0 { live.dispatch(); }
        if mode == 2 { live.host.publish(live.host.revision(), 1).unwrap(); }
        drop(live);
        let mut c = coordinator(&control, &plan); let prior = image(&control);
        let anchor = c.history_anchor().unwrap(); let entered = RefCell::new(None);
        let result = c.recover_and_drain_with_clock(0, binding(), 1, |_| {
            *entered.borrow_mut() = Some(image(&control));
            replace(&control, &prior);
            Ok(ElapsedTick(if mode == 3 { 1001 } else { 2 }))
        }, |_| true);
        assert_eq!(result, Err(Error::Binding.into()));
        assert_eq!(image(&control), prior); assert_eq!(c.revision(), 1); assert_quarantined(&c);
        let native = root.bytes();
        let state = FileOversight::read_publication(root.store(), &effect_fixture::profile()).unwrap();
        assert!(state.stop.is_some()); assert_eq!(state.executions, u64::from(mode == 2));
        assert_eq!(state.control.ledger.charged, if mode >= 2 { 16 } else { 0 });
        let intent = entered.into_inner().unwrap();
        assert_eq!(c.acknowledged_image.as_deref(), Some(intent.as_slice()));
        // Test-controlled restoration of the real entered cut. Evidence-only
        // resolution then reads actual native results; it does not repeat a stop.
        replace(&control, &intent); drop(c);
        let mut c = FileShutdownCoordinator::open_anchored(control.store(), &plan,
            MAX_JOURNAL_BYTES, 1, &anchor).unwrap();
        let observed = c.resolve_shutdown_visit(c.revision(), 0).unwrap();
        assert_eq!(observed.journal_revision, state.revision);
        assert_eq!(observed.executions, state.executions);
        assert_eq!(observed.stop.unwrap().charged_units, state.control.ledger.charged);
        assert_eq!(root.bytes(), native);
    }
}

#[test]
fn passive_finish_and_cancel_cannot_clear_a_coordinator_conflict() {
    for pending in [false, true] { for cancel in [false, true] {
        let root = Directory::new(); let control = Directory::new();
        let mut host = fixture::create(&root, 64); let plan = fixture::plan(&host, 8);
        let mut c = coordinator(&control, &plan); let old = image(&control);
        let pair = c.begin(0, 1, ShutdownVisitKind::Advance { at: ElapsedTick(2) }).unwrap();
        let result = c.campaign.advance_inner(pair.0, pair.1, &mut host, ElapsedTick(2));
        assert!(result.is_ok());
        if !pending { c.finish(pair.0, pair.1, result).unwrap().unwrap(); }
        drop(c);
        let mut c = fixture::reopen(&control, &plan, 1);
        let acknowledged = image(&control); let revision = c.revision(); let native = image(&root);
        let mut read = c.begin_resolve_shutdown_visit(revision, 0).unwrap();
        while read.progress().status == FileStopRecoveryStatus::Replaying {
            read.advance(read.progress().replayed_events, 1).unwrap();
        }
        replace(&control, &old);
        let staged = control.store().join("delivery.pending"); fs::write(&staged, b"keep").unwrap();
        let result = if cancel { read.cancel() } else { read.finish().map(|_| ()) };
        assert_eq!(result, Err(Error::Binding.into()));
        assert_eq!(image(&control), old); assert_eq!(image(&root), native);
        assert_eq!(fs::read(&staged).unwrap(), b"keep"); assert_quarantined(&c);
        replace(&control, &acknowledged); drop(c);
        let mut c = fixture::reopen(&control, &plan, revision);
        c.resolve_shutdown_visit(revision, 0).unwrap();
        assert_eq!(c.revision(), revision + u64::from(pending));
        assert_eq!(image(&root), native); assert_eq!(c.report().visits.len(), 1);
        if !pending { assert_eq!(image(&control), acknowledged); }
    }}
}

#[test]
fn lost_coordinator_image_refuses_passive_entry_without_reading_the_domain() {
    for missing in [false, true] {
        let root = Directory::new(); let control = Directory::new();
        let host = fixture::create(&root, 64); let plan = fixture::plan(&host, 8); drop(host);
        let mut c = coordinator(&control, &plan);
        c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
        let saved = image(&control); let native = image(&root);
        if missing { fs::remove_file(control.store().join(storage::CANONICAL)).unwrap(); }
        else { replace(&control, b"invalid substituted history"); }
        // The failed coordinator check must precede even passive domain intake.
        let parked = root.store().with_extension("parked"); fs::rename(root.store(), &parked).unwrap();
        let error = c.begin_resolve_shutdown_visit(c.revision(), 0).unwrap_err();
        if missing { assert!(matches!(error, JournalError::Io(_))); }
        else { assert_eq!(error, Error::Binding.into()); }
        assert_quarantined(&c);
        fs::rename(&parked, root.store()).unwrap(); assert_eq!(image(&root), native);
        replace(&control, &saved); drop(c);
        let mut c = fixture::reopen(&control, &plan, 2);
        c.resolve_shutdown_visit(2, 0).unwrap(); assert_eq!(image(&control), saved);
    }
}

#[test]
fn unchanged_custody_allows_domain_read_refusal_cancellation_and_exact_resolution() {
    let root = Directory::new(); let control = Directory::new();
    let host = fixture::create(&root, 64); let plan = fixture::plan(&host, 16); drop(host);
    let mut c = coordinator(&control, &plan);
    c.recover_and_drain(0, 1, ElapsedTick(2)).unwrap().unwrap();
    let saved = image(&control); let native = image(&root); let revision = c.revision();
    let parked = root.store().with_extension("parked"); fs::rename(root.store(), &parked).unwrap();
    assert!(c.resolve_shutdown_visit(revision, 0).is_err());
    assert!(!c.report().unavailable); assert!(!c.report().all_observed_drained());
    fs::rename(&parked, root.store()).unwrap();
    c.begin_resolve_shutdown_visit(revision, 0).unwrap().cancel().unwrap();
    assert!(!c.report().unavailable); assert!(!c.report().all_observed_drained());
    c.resolve_shutdown_visit(revision, 0).unwrap();
    assert!(c.report().all_observed_drained()); assert_eq!(c.revision(), revision);
    assert_eq!(image(&control), saved); assert_eq!(image(&root), native);
    assert_eq!(c.acknowledged_image.as_deref(), Some(saved.as_slice()));
}

#[test]
fn fleet_reports_coordinator_conflict_as_unacknowledged_and_keeps_the_tail_unvisited() {
    let first = Directory::new(); let second = Directory::new(); let control = Directory::new();
    let one = fixture::member(&first, 1, 99); let two = fixture::member(&second, 2, 100);
    let plan = FileShutdownPlan::new(900, vec![one.shutdown_domain(1).unwrap(),
        two.shutdown_domain(2).unwrap()], 8, MAX_SHUTDOWN_HEAD_BYTES).unwrap();
    drop(one); drop(two);
    let first_before = image(&first); let second_before = image(&second);
    let mut c = coordinator(&control, &plan); let prior = image(&control);
    let bindings = [binding(), FileShutdownClockBinding { domain: 2, clock_domain: 100 }];
    let pass = c.recover_registered_pass_with_clocks(0, &bindings, 1, |actual| {
        assert_eq!(actual.domain, 1); replace(&control, &prior); Ok(ElapsedTick(2))
    }, |_| true).unwrap();
    assert!(matches!(&pass.members()[0].result,
        FileShutdownPassResult::Unacknowledged(JournalError::Contract(Error::Binding))));
    assert!(matches!(&pass.members()[1].result, FileShutdownPassResult::NotVisited));
    assert!(!pass.all_observed_stopped()); assert!(!pass.all_observed_drained());
    assert_eq!(pass.final_revision(), 1); assert_eq!(image(&control), prior);
    assert_ne!(image(&first), first_before); assert_eq!(image(&second), second_before);
    assert_quarantined(&c);
}
