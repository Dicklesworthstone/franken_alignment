//! Public guarded stop phases retain original liabilities, guard refusals and anchors.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::recovery::{
    FileAdmissionStoppedHistory, FileStoppedHistory,
};

impl Run {
    fn finish_admission_stop(self, request: StopRequest)
        -> Result<FileAdmissionStoppedHistory, JournalError>
    {
        match self {
            Self::Learned(run) => run.finish_admission_stop(request),
            Self::Guarded(run) => run.finish_admission_stop(request),
            Self::Raw(run) => run.finish_admission_stop(request),
            Self::Owned(run) => run.finish_admission_stop(request),
        }
    }

    fn finish_stopped_drain(self, request: StopRequest,
        clock: impl FnOnce() -> Result<ElapsedTick, JournalError>)
        -> Result<FileStoppedHistory, JournalError>
    {
        match self {
            Self::Learned(run) => run.finish_stopped_drain(request, clock),
            Self::Guarded(run) => run.finish_stopped_drain(request, clock),
            Self::Raw(run) => run.finish_stopped_drain(request, clock),
            Self::Owned(run) => run.finish_stopped_drain(request, clock),
        }
    }

    fn try_finish_stopped_with_anchor(self, request: StopRequest,
        clock: impl FnOnce() -> Result<ElapsedTick, JournalError>)
        -> Result<FileStoppedHistory, JournalError>
    {
        match self {
            Self::Learned(run) => run.try_finish_stopped_with_anchor(request, clock),
            Self::Guarded(run) => run.try_finish_stopped_with_anchor(request, clock),
            Self::Raw(run) => run.try_finish_stopped_with_anchor(request, clock),
            Self::Owned(run) => run.try_finish_stopped_with_anchor(request, clock),
        }
    }
}

#[test]
fn guarded_phases_keep_original_effect_liabilities_and_acknowledged_anchors() {
    for lane in LANES {
        for cut in [Cut::Reserved, Cut::Sent, Cut::Executed, Cut::Expired] {
            let mut history = History::new(lane, cut, None);
            let admitted = history.ready().finish_admission_stop(history.request).unwrap();
            assert_eq!(admitted.appended_events(), 2);
            assert_eq!(admitted.snapshot().revision, history.before.revision + 2);
            assert_eq!(admitted.progress().receipt.request(), history.request);
            assert_eq!(admitted.progress().reserved_units, 0);
            assert_eq!(admitted.progress().charged_units,
                if cut == Cut::Reserved { 0 } else { history.units });
            assert_eq!(admitted.snapshot().executions, history.before.executions);
            if cut != Cut::Reserved {
                assert!(!admitted.progress().drained(), "dispatched effects still require settlement");
            }
            assert_eq!(admitted.history_anchor().revision(), admitted.snapshot().revision);
            assert_eq!(admitted.history_anchor().retained_bytes(), history.bytes().len());
            history.anchor = admitted.history_anchor().clone();

            let canonical = history.bytes();
            history.pending();
            let again = history.ready().finish_admission_stop(history.request).unwrap();
            assert_eq!(again.appended_events(), 0);
            assert_eq!(again.snapshot(), admitted.snapshot());
            assert_eq!(again.history_anchor(), admitted.history_anchor());
            assert_eq!(history.bytes(), canonical);
            assert!(!history.root.store().join("delivery.pending").exists());

            let needs_drain = !admitted.progress().drained();
            let calls = Cell::new(0);
            let drained = history.ready().finish_stopped_drain(history.request, || {
                calls.set(calls.get() + 1);
                Ok(ElapsedTick(if cut == Cut::Expired { 1001 } else { 2 }))
            }).unwrap();
            let result = drained.stopped();
            assert_eq!(calls.get(), usize::from(needs_drain));
            assert_eq!(result.snapshot().revision, admitted.snapshot().revision + u64::from(needs_drain));
            assert_eq!(result.snapshot().executions, history.before.executions);
            assert_eq!(result.progress().charged_units,
                if matches!(cut, Cut::Executed | Cut::Expired) { history.units } else { 0 });
            assert_eq!(result.progress().drained(), cut != Cut::Expired);
            if cut == Cut::Expired {
                assert_eq!(result.progress().unresolved, vec![history.attempt.unwrap()]);
                assert_eq!(result.progress().irrecoverable, vec![history.attempt.unwrap()]);
            }
            assert_eq!(drained.history_anchor().revision(), result.snapshot().revision);
            assert_eq!(drained.history_anchor().retained_bytes(), history.bytes().len());
            history.assert_observed(result);
            history.anchor = drained.history_anchor().clone();
            let canonical = history.bytes();
            let readmitted = history.ready().finish_admission_stop(history.request).unwrap();
            assert_eq!(readmitted.appended_events(), 0);
            assert_eq!(readmitted.history_anchor(), drained.history_anchor());
            if result.progress().drained() {
                let retried = history.ready().finish_stopped_drain(history.request,
                    || panic!("drained retry must not acquire time")).unwrap();
                assert!(matches!(retried.stopped(), FileStoppedRecovery::AlreadyDrained { .. }));
                assert_eq!(retried.history_anchor(), drained.history_anchor());
                let combined = history.ready().try_finish_stopped_with_anchor(history.request,
                    || panic!("drained combined retry must not acquire time")).unwrap();
                assert_eq!(combined.history_anchor(), drained.history_anchor());
            }
            assert_eq!(history.bytes(), canonical);
        }
    }
}

#[test]
fn guarded_phase_refusals_and_unavailable_clocks_preserve_the_existing_cut() {
    for lane in LANES {
        let mut history = History::new(lane, Cut::Sent, None);
        history.pending();
        let before = history.root.files();
        assert_eq!(history.begin().finish_admission_stop(history.request).unwrap_err(),
            JournalError::Contract(Error::Incomplete));
        assert_eq!(history.ready().finish_stopped_drain(history.request,
            || panic!("active history must refuse before acquiring time")).unwrap_err(),
            JournalError::Contract(Error::WrongState));
        let calls = Cell::new(0);
        assert_eq!(history.ready().try_finish_stopped_with_anchor(history.request, || {
            calls.set(calls.get() + 1);
            Err(JournalError::Unavailable)
        }).unwrap_err(), JournalError::Unavailable);
        assert_eq!(calls.get(), 1);
        assert_eq!(history.root.files(), before, "combined clock failure must not stop admission");

        let admitted = history.ready().finish_admission_stop(history.request).unwrap();
        history.anchor = admitted.history_anchor().clone();
        history.pending();
        let stopped = history.root.files();
        let mut foreign = history.request;
        foreign.operation += 1;
        assert_eq!(history.ready().finish_stopped_drain(foreign,
            || panic!("another stop identity must refuse before time")).unwrap_err(),
            JournalError::Contract(Error::Duplicate));
        assert_eq!(history.ready().finish_stopped_drain(history.request,
            || Err(JournalError::Unavailable)).unwrap_err(), JournalError::Unavailable);
        assert_eq!(history.root.files(), stopped, "failed drain must retain the acknowledged admission stop");
        let drained = history.ready().finish_stopped_drain(history.request, || Ok(ElapsedTick(2))).unwrap();
        assert_eq!(drained.stopped().snapshot().revision, admitted.snapshot().revision + 1);
        assert!(drained.stopped().progress().drained());
        assert_eq!(drained.stopped().snapshot().executions, 0);
    }
}

#[test]
fn guarded_drain_rechecks_canonical_identity_after_fallible_time() {
    for lane in LANES {
        let mut history = History::new(lane, Cut::Sent, None);
        let admitted = history.ready().finish_admission_stop(history.request).unwrap();
        history.anchor = admitted.history_anchor().clone();
        let canonical = history.bytes();
        let mut successor = history.ready().ordinary_fixture_owner();
        successor.observe_time(successor.revision(), ElapsedTick(2)).unwrap();
        let alternate = history.bytes();
        assert_ne!(alternate, canonical);
        drop(successor);
        std::fs::write(history.root.store().join("delivery.bin"), &canonical).unwrap();
        history.pending();
        let run = history.ready();
        let calls = Cell::new(0);
        let error = run.finish_stopped_drain(history.request, || {
            calls.set(calls.get() + 1);
            std::fs::write(history.root.store().join("delivery.bin"), &alternate).unwrap();
            Ok(ElapsedTick(2))
        }).unwrap_err();
        assert_eq!(error, JournalError::Contract(Error::Binding));
        assert_eq!(calls.get(), 1);
        assert_eq!(history.bytes(), alternate);
        assert_eq!(std::fs::read(history.root.store().join("delivery.pending")).unwrap(),
            b"unconfirmed candidate");
    }
}

#[test]
fn guarded_phase_consumers_cannot_bypass_failed_independent_floors() {
    for lane in [Lane::Guarded, Lane::Raw, Lane::Owned] {
        let history = History::new(lane, Cut::Sent, None);
        history.pending();
        let before = history.root.files();
        let mut stale = history.expected.clone();
        stale.minimum.journal_revision += 1;
        for phase in 0..3 {
            let mut run = history.begin_with(&stale, RecoveryReserve::terminal()).unwrap();
            assert_eq!(run.advance(0, run.progress().total_events), Err(Error::Stale.into()));
            let error = match phase {
                0 => run.finish_admission_stop(history.request).err(),
                1 => run.finish_stopped_drain(history.request,
                    || panic!("failed guard must not reach a drain clock")).err(),
                _ => run.try_finish_stopped_with_anchor(history.request,
                    || panic!("failed guard must not reach a combined clock")).err(),
            };
            assert_eq!(error, Some(JournalError::Contract(Error::Stale)));
            assert_eq!(history.root.files(), before);
        }
    }
}
