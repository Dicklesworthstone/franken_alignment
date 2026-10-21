//! Combined recovery must not commit using a clock acquired after its deadline.
//! Reuse original native owners, complete guards, topology and actual storage.
use super::*;
use crate::recover_stop::guarded::recovery::Recovery;
use fa_reference::action::consequence::delivery::{StopRequest, persistent::observed::shutdown::recovery::{
    FileStopRecovery, FileStopRecoveryStatus, FileStoppedHistory}};

fn combined(route: usize, c: &Config, p: &Profile, clock: impl FnOnce() -> ElapsedTick,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
{
    if route == 0 {
        crate::recover_stop::cooperative::run_with_elapsed(c, 900, clock, elapsed, out)
    } else {
        crate::recover_stop::guarded::run_with_elapsed(c, p, 900, clock, elapsed, out)
    }
}

// Test-only sum over the existing consuming APIs, never another replay machine.
enum Ready { Plain(FileStopRecovery), Guarded(Recovery) }
impl Ready {
    fn request(&self) -> StopRequest {
        match self { Self::Plain(run) => run.stop_request(900),
            Self::Guarded(run) => run.stop_request(900) }.unwrap()
    }
    fn finish(self, request: StopRequest, clock: impl FnOnce() -> Result<ElapsedTick, JournalError>)
        -> Result<FileStoppedHistory, JournalError>
    {
        match self { Self::Plain(run) => run.try_finish_stopped_with_anchor(request, clock),
            Self::Guarded(run) => run.finish_stopped_with_anchor(request, clock) }
    }
    fn legacy(self, request: StopRequest, tick: ElapsedTick) -> FileStoppedHistory {
        match self {
            Self::Plain(run) => run.finish_stopped_with_anchor(request, || tick),
            Self::Guarded(Recovery::Base(run)) => run.finish_stopped_with_anchor(request, || tick),
            Self::Guarded(Recovery::Mediated(run)) => run.finish_stopped_with_anchor(request, || tick),
        }.unwrap()
    }
}
fn ready(route: usize, root: &Directory, c: &Config, p: &Profile) -> Ready {
    if route == 0 {
        let mut run = FileOversight::begin_stopped_recovery_anchored(&c.store, c.profile.clone(),
            &anchor(root, "before.anchor")).unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
        while run.progress().status == FileStopRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, 1).unwrap();
        }
        Ready::Plain(run)
    } else {
        let (mut run, _) = crate::recover_stop::guarded::prepare_recovery(c, p, &mut || Ok(())).unwrap();
        while run.progress().status == FileGuardedRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, 1).unwrap();
        }
        Ready::Guarded(run)
    }
}

#[test]
fn slow_combined_clock_preserves_history_then_full_capacity_completion_skips_clock_on_retry() {
    for route in 0..3 {
        let root = Directory::new(); let (c, floor) = setup(&root, route == 2, true);
        let p = recipe(&root, floor, route == 2, true, "before.anchor", Some("after.anchor"));
        fs::write(root.path("store/delivery.pending"), b"not a receipt").unwrap();
        let before = root.files(); let late = Cell::new(false); let calls = Cell::new(0);
        let mut out = Vec::new();
        let error = combined(route, &c, &p, || {
            calls.set(calls.get() + 1); late.set(true); ElapsedTick(2)
        }, &mut || if late.get() { Duration::from_millis(c.timing.runtime_ms) } else { Duration::ZERO },
            &mut out).unwrap_err();
        assert!(error.contains("deadline"), "{route}: {error}");
        assert_eq!(calls.get(), 1); assert!(out.is_empty());
        assert_eq!(root.files(), before); assert!(!root.path("after.anchor").exists());
        combined(route, &c, &p, || ElapsedTick(2), &mut || Duration::ZERO, &mut out).unwrap();
        assert_eq!(json(&out).get("drained").unwrap().as_bool(), Some(true));
        let state = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(state.revision, floor.journal_revision + 3);
        assert_eq!(state.revision as usize, c.profile.delivery.limits.events);
        let saved = root.bytes();
        let retry = recipe(&root, floor, route == 2, false, "before.anchor", None);
        combined(route, &c, &retry, || panic!("completed receipt must not request time"),
            &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        assert_eq!(root.bytes(), saved);
    }
}

#[test]
fn both_acquisition_deadline_boundaries_refuse_before_any_terminal_write() {
    for route in 0..3 {
        let sizing = Directory::new(); let (c, floor) = setup(&sizing, route == 2, false);
        let p = recipe(&sizing, floor, route == 2, true, "before.anchor", None);
        let mut calls = 0;
        combined(route, &c, &p, || ElapsedTick(2), &mut || { calls += 1; Duration::ZERO }, &mut Vec::new()).unwrap();
        for after in [false, true] {
            let root = Directory::new(); let (c, floor) = setup(&root, route == 2, false);
            let p = recipe(&root, floor, route == 2, true, "before.anchor", None);
            fs::write(root.path("store/delivery.pending"), b"preserve").unwrap();
            let before = root.files(); let mut count = 0; let acquired = Cell::new(0); let mut out = Vec::new();
            let cut = calls - usize::from(!after);
            let error = combined(route, &c, &p, || { acquired.set(acquired.get() + 1); ElapsedTick(2) },
                &mut || { count += 1; if count >= cut { Duration::from_millis(c.timing.runtime_ms) } else { Duration::ZERO } },
                &mut out).unwrap_err();
            assert!(error.contains("deadline")); assert_eq!(acquired.get(), usize::from(after));
            assert!(out.is_empty()); assert_eq!(root.files(), before);
            combined(route, &c, &p, || ElapsedTick(2), &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        }
    }
}

#[test]
fn fallible_native_clock_propagates_refusal_or_unwind_without_cleanup_or_candidate_history() {
    for route in 0..3 { for unwind in [false, true] {
        let root = Directory::new(); let (c, floor) = setup(&root, route == 2, false);
        let p = recipe(&root, floor, route == 2, true, "before.anchor", None);
        fs::write(root.path("store/delivery.pending"), b"preserve on clock failure").unwrap();
        let before = root.files(); let run = ready(route, &root, &c, &p); let request = run.request();
        let result = catch_unwind(AssertUnwindSafe(|| run.finish(request, || {
            if unwind { panic!("trusted clock interrupted"); }
            Err(JournalError::Contract(fa_reference::Error::Stale))
        })));
        if unwind { assert!(result.is_err()); }
        else { assert_eq!(result.unwrap(), Err(JournalError::Contract(fa_reference::Error::Stale))); }
        assert_eq!(root.files(), before);
        let run = ready(route, &root, &c, &p); let request = run.request();
        let history = run.finish(request, || Ok(ElapsedTick(2))).unwrap();
        assert!(history.stopped().progress().drained());
    }}
}

#[test]
fn fallible_completion_matches_original_combined_bytes_and_keeps_canonical_checks() {
    for route in 0..3 {
        let root = Directory::new(); let (c, floor) = setup(&root, route == 2, false);
        let p = recipe(&root, floor, route == 2, true, "before.anchor", None);
        let original = root.bytes(); let run = ready(route, &root, &c, &p); let request = run.request();
        let expected = run.legacy(request, ElapsedTick(2)); let expected_bytes = root.bytes();
        // Replay the identical original fixture in the same bound path for a
        // differential comparison; no production rollback/import API is used.
        fs::write(root.path("store/delivery.bin"), &original).unwrap();
        let run = ready(route, &root, &c, &p); let request = run.request();
        assert_eq!(run.finish(request, || Ok(ElapsedTick(2))).unwrap(), expected);
        assert_eq!(root.bytes(), expected_bytes);
        fs::write(root.path("store/delivery.bin"), &original).unwrap();
        fs::write(root.path("store/delivery.pending"), b"retain on conflict").unwrap();
        let run = ready(route, &root, &c, &p); let mut request = run.request(); request.expected_authority_epoch += 1;
        assert!(run.finish(request, || panic!("intent check precedes clock")).is_err());
        assert_eq!(root.bytes(), original);
        let run = ready(route, &root, &c, &p); let request = run.request();
        let result = run.finish(request, || {
            fs::write(root.path("store/delivery.bin"), b"changed during acquisition").unwrap();
            Ok(ElapsedTick(2))
        });
        assert!(result.is_err()); assert_eq!(root.bytes(), b"changed during acquisition");
        assert_eq!(fs::read(root.path("store/delivery.pending")).unwrap(), b"retain on conflict");
    }
}
