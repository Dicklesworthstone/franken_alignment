//! Independent phase controls through the public mediated recovery cursors.
//! Real numerical generation, native helper review and both publication keys
//! create the liabilities. Synthetic weights and graphs are not calibration or
//! evidence that a deployed operating-system perimeter has no bypass.
use super::*;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        FileDeliverySnapshot,
        observed::{
            consistency::learned::FileLearnedConsistencySnapshot,
            shutdown::recovery::FileAdmissionStoppedHistory,
        },
    },
};

impl Recovery {
    fn admission(self, request: StopRequest) -> Result<FileAdmissionStoppedHistory, JournalError> {
        match self {
            Self::Plain(run) => run.finish_admission_stop(request),
            Self::Predictive(run) => run.finish_admission_stop(request),
        }
    }

    fn drain(
        self,
        request: StopRequest,
        clock: impl FnOnce() -> Result<ElapsedTick, JournalError>,
    ) -> Result<FileStoppedHistory, JournalError> {
        match self {
            Self::Plain(run) => run.finish_stopped_drain(request, clock),
            Self::Predictive(run) => run.finish_stopped_drain(request, clock),
        }
    }

    fn combined(
        self,
        request: StopRequest,
        clock: impl FnOnce() -> Result<ElapsedTick, JournalError>,
    ) -> Result<FileStoppedHistory, JournalError> {
        match self {
            Self::Plain(run) => run.try_finish_stopped_with_anchor(request, clock),
            Self::Predictive(run) => run.try_finish_stopped_with_anchor(request, clock),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Reserved,
    Sent,
    Executed,
    Expired,
}

struct History {
    root: Directory,
    lane: Lane,
    profile: FileOversightProfile,
    config: FileLearnedConfig,
    expected: FileMediatedRequirements,
    anchor: FileHistoryAnchor,
    before: FileDeliverySnapshot,
    request: StopRequest,
    attempt: u64,
    units: u64,
    owned_source: Option<FileLearnedConsistencySnapshot>,
}

impl History {
    fn new(lane: Lane, cut: Cut, full: bool) -> Self {
        let root = Directory::new();
        let mut profile = boundary_profile(false);
        profile.delivery.limits.events = 128;
        let config = configuration(lane, false);
        let (mut host, roles) = create_case(
            lane,
            &root,
            profile.clone(),
            &config,
            Some(RecoveryReserve::terminal()),
        )
        .unwrap();
        prefill(&mut host);
        assert!(matches!(
            certify(&mut host, &roles.topology_observer).unwrap(),
            CutCheck::Verified(_)
        ));
        forecast(lane, &mut host, &roles, 71);
        let (samples, last) = finish_generation(&mut host);
        assert_eq!(samples.len(), 3);
        assert_eq!(
            host.learned_text_message(LearnedEvidenceLimits::default())
                .unwrap()
                .bytes(),
            b"OK"
        );
        custody::fresh_identity(&mut host, &roles.oversight, 1, 1);
        submit_generated(&mut host, 71);
        let FileRequestDisposition::Admitted { attempt, .. } =
            host.request_status(71).unwrap().disposition
        else {
            panic!("original generated request must be admitted");
        };
        let (action, input, automatic, offer) = prepared(&mut host, 71, 101, &last);
        let units = action.spec().units;
        assert_eq!(host.inspect().control.ledger.reserved, units);
        assert_eq!(host.inspect().executions, 0);
        if cut != Cut::Reserved {
            let revision = host.revision();
            let human = roles
                .oversight
                .human
                .approve(&mut host, revision, &offer)
                .unwrap();
            host.dispatch(
                host.revision(),
                &automatic,
                &human,
                &action,
                &input,
                snapshot(),
            )
            .unwrap();
            assert_eq!(host.inspect().control.ledger.charged, units);
            assert_eq!(
                host.delivery_mediation(attempt).unwrap().unwrap().graph(),
                &graph(1, false)
            );
            if cut == Cut::Executed {
                let receipt = host
                    .publish_checked(
                        host.revision(),
                        attempt,
                        Some(&input),
                        snapshot(),
                        ElapsedTick(3),
                    )
                    .unwrap();
                assert_eq!(
                    receipt.outcome,
                    EndpointOutcome::Executed {
                        resulting_version: 2
                    }
                );
            }
        }
        if full {
            while host.journal_capacity().unwrap().ordinary_remaining().events != 0 {
                host.observe_time(host.revision(), ElapsedTick(4)).unwrap();
            }
            assert_eq!(host.journal_capacity().unwrap().remaining().events, 3);
        }
        let before = host.inspect();
        assert_eq!(before.executions, u64::from(cut == Cut::Executed));
        let request = StopRequest {
            operation: 901,
            expected_control_sequence: before.control.sequence,
            expected_authority_epoch: before.control.ledger.epoch,
        };
        let expected = expected(&host, &profile, &config);
        let anchor = host.history_anchor().unwrap();
        let owned_source =
            (lane == Lane::Owned).then(|| host.learned_action_consistency_snapshot().unwrap());
        drop(host);
        Self {
            root,
            lane,
            profile,
            config,
            expected,
            anchor,
            before,
            request,
            attempt,
            units,
            owned_source,
        }
    }

    fn begin(&self) -> Recovery {
        begin(
            self.lane,
            &self.root,
            &self.profile,
            &self.config,
            &self.expected,
            Some(&self.anchor),
        )
        .unwrap()
        .require(RecoveryReserve::terminal())
        .unwrap()
    }

    fn ready(&self) -> Recovery {
        let before = files(&self.root);
        let mut run = self.begin();
        run.drive().unwrap();
        assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
        assert_eq!(
            files(&self.root),
            before,
            "replay cannot clean staging or append a fence"
        );
        run
    }

    fn stopped(&mut self, anchor: &FileHistoryAnchor) {
        self.anchor = anchor.clone();
        // The original Stop/Fence withdraws topology availability. It does not
        // manufacture a new certificate or permit a weaker recovery inventory.
        self.expected.topology.available = false;
    }

    fn pending(&self) {
        std::fs::write(
            self.root.store().join("delivery.pending"),
            b"unconfirmed mediated candidate",
        )
        .unwrap();
    }

    fn observe(&self) -> FileDeliverySnapshot {
        if let Some(original) = self.owned_source.as_ref() {
            let observed = FileOversight::read_publication_with_owned_learned_consistency(
                self.root.store(),
                &self.profile,
                &self.config,
                self.config.required_owned_pre_output_forecast().unwrap(),
            )
            .unwrap();
            assert_eq!(observed.consistency.work, original.work);
            assert_eq!(
                observed.consistency.retained_source_bytes,
                original.retained_source_bytes
            );
            assert_eq!(
                observed.consistency.consistency.evidence,
                original.consistency.evidence
            );
            return observed.publication;
        }
        FileOversight::read_publication_with_learned_generation(
            self.root.store(),
            &self.profile,
            &self.config,
        )
        .unwrap()
    }
}

#[test]
fn mediated_phases_preserve_original_liabilities_and_exact_anchors_at_capacity() {
    for lane in LANES {
        for cut in [Cut::Reserved, Cut::Sent, Cut::Executed, Cut::Expired] {
            let mut history = History::new(lane, cut, true);
            history.pending();
            let admitted = history.ready().admission(history.request).unwrap();
            assert_eq!(admitted.appended_events(), 2);
            assert_eq!(admitted.snapshot().revision, history.before.revision + 2);
            assert_eq!(admitted.progress().receipt.request(), history.request);
            assert_eq!(admitted.progress().reserved_units, 0);
            assert_eq!(
                admitted.progress().charged_units,
                if cut == Cut::Reserved {
                    0
                } else {
                    history.units
                }
            );
            assert_eq!(admitted.snapshot().executions, history.before.executions);
            assert_eq!(admitted.progress().drained(), cut == Cut::Reserved);
            assert_eq!(
                admitted.history_anchor().revision(),
                admitted.snapshot().revision
            );
            assert_eq!(
                admitted.history_anchor().retained_bytes(),
                history.root.bytes().len()
            );
            assert_eq!(&history.observe(), admitted.snapshot());
            history.stopped(admitted.history_anchor());

            let canonical = history.root.bytes();
            history.pending();
            let again = history.ready().admission(history.request).unwrap();
            assert_eq!(again.appended_events(), 0);
            assert_eq!(again.snapshot(), admitted.snapshot());
            assert_eq!(again.history_anchor(), admitted.history_anchor());
            assert_eq!(history.root.bytes(), canonical);
            assert!(!history.root.store().join("delivery.pending").exists());

            let calls = Cell::new(0);
            let drained = history
                .ready()
                .drain(history.request, || {
                    calls.set(calls.get() + 1);
                    Ok(ElapsedTick(if cut == Cut::Expired { 1001 } else { 4 }))
                })
                .unwrap();
            let result = drained.stopped();
            assert_eq!(calls.get(), usize::from(cut != Cut::Reserved));
            assert_eq!(
                result.snapshot().revision,
                admitted.snapshot().revision + u64::from(cut != Cut::Reserved)
            );
            assert_eq!(result.snapshot().executions, history.before.executions);
            assert_eq!(
                result.progress().charged_units,
                if matches!(cut, Cut::Executed | Cut::Expired) {
                    history.units
                } else {
                    0
                }
            );
            assert_eq!(result.progress().drained(), cut != Cut::Expired);
            assert_eq!(&history.observe(), result.snapshot());
            assert_eq!(
                drained.history_anchor().revision(),
                result.snapshot().revision
            );
            assert_eq!(
                drained.history_anchor().retained_bytes(),
                history.root.bytes().len()
            );
            history.stopped(drained.history_anchor());
            let canonical = history.root.bytes();
            let retried_stop = history.ready().admission(history.request).unwrap();
            assert_eq!(retried_stop.appended_events(), 0);
            assert_eq!(retried_stop.history_anchor(), drained.history_anchor());
            if cut == Cut::Expired {
                assert_eq!(result.snapshot().revision, 128);
                assert_eq!(result.progress().unresolved, vec![history.attempt]);
                assert_eq!(result.progress().irrecoverable, vec![history.attempt]);
                history.pending();
                let files_before = files(&history.root);
                assert_eq!(
                    history
                        .ready()
                        .drain(history.request, || panic!(
                            "full pending drain must reject before time"
                        ))
                        .unwrap_err(),
                    Error::Limit.into()
                );
                assert_eq!(files(&history.root), files_before);
                assert_eq!(history.observe().control.ledger.charged, history.units);
            } else {
                let retried = history
                    .ready()
                    .drain(history.request, || {
                        panic!("completed drain needs no clock or extra event")
                    })
                    .unwrap();
                assert!(matches!(
                    retried.stopped(),
                    FileStoppedRecovery::AlreadyDrained { .. }
                ));
                assert_eq!(retried.history_anchor(), drained.history_anchor());
                let combined = history
                    .ready()
                    .combined(history.request, || {
                        panic!("completed combined retry needs no clock")
                    })
                    .unwrap();
                assert_eq!(combined.history_anchor(), drained.history_anchor());
            }
            assert_eq!(history.root.bytes(), canonical);
        }
    }
}

#[test]
fn mediated_phase_failures_preserve_active_or_stopped_history_without_fallback() {
    for lane in LANES {
        let mut history = History::new(lane, Cut::Sent, false);
        history.pending();
        let before = files(&history.root);
        assert_eq!(
            history.begin().admission(history.request).unwrap_err(),
            Error::Incomplete.into()
        );
        assert_eq!(
            history
                .ready()
                .drain(history.request, || panic!(
                    "active history has no acknowledged stop"
                ))
                .unwrap_err(),
            Error::WrongState.into()
        );
        let calls = Cell::new(0);
        assert_eq!(
            history
                .ready()
                .combined(history.request, || {
                    calls.set(calls.get() + 1);
                    Err(JournalError::Unavailable)
                })
                .unwrap_err(),
            JournalError::Unavailable
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(
            files(&history.root),
            before,
            "combined clock failure must not stop admission"
        );
        assert_eq!(history.observe().control.ledger.charged, history.units);

        let admitted = history.ready().admission(history.request).unwrap();
        history.stopped(admitted.history_anchor());
        history.pending();
        let stopped = files(&history.root);
        let mut foreign = history.request;
        foreign.operation += 1;
        assert_eq!(
            history
                .ready()
                .drain(foreign, || panic!("foreign intent must refuse before time"))
                .unwrap_err(),
            Error::Duplicate.into()
        );
        assert_eq!(
            history
                .ready()
                .drain(history.request, || Err(JournalError::Unavailable))
                .unwrap_err(),
            JournalError::Unavailable
        );
        assert!(
            history
                .ready()
                .drain(history.request, || Ok(ElapsedTick(0)))
                .is_err()
        );
        assert_eq!(
            files(&history.root),
            stopped,
            "failed drain retains the exact stop and staging"
        );
        assert_eq!(history.observe().control.ledger.charged, history.units);

        let drained = history
            .ready()
            .drain(history.request, || Ok(ElapsedTick(4)))
            .unwrap();
        assert_eq!(
            drained.stopped().snapshot().revision,
            admitted.snapshot().revision + 1
        );
        assert!(drained.stopped().progress().drained());
        assert_eq!(drained.stopped().snapshot().executions, 0);
        assert_eq!(drained.stopped().progress().charged_units, 0);
    }
}

#[test]
fn mediated_phase_consumers_reject_unmet_independent_floor_and_topology() {
    for lane in LANES {
        let history = History::new(lane, Cut::Sent, false);
        history.pending();
        let before = files(&history.root);
        for guard in 0..2 {
            let mut wrong = history.expected.clone();
            if guard == 0 {
                wrong.oversight.minimum.journal_revision += 1;
            } else {
                wrong.topology.available = false;
            }
            for phase in 0..3 {
                let mut run = begin(
                    lane,
                    &history.root,
                    &history.profile,
                    &history.config,
                    &wrong,
                    Some(&history.anchor),
                )
                .unwrap()
                .require(RecoveryReserve::terminal())
                .unwrap();
                let rejected = run.drive().unwrap_err();
                assert!(matches!(
                    run.progress().status,
                    FileLearnedRecoveryStatus::Failed(_)
                ));
                let error = match phase {
                    0 => run.admission(history.request).err(),
                    1 => run
                        .drain(history.request, || {
                            panic!("failed guard cannot acquire drain time")
                        })
                        .err(),
                    _ => run
                        .combined(history.request, || {
                            panic!("failed guard cannot acquire combined time")
                        })
                        .err(),
                };
                assert_eq!(error, Some(rejected));
                assert_eq!(files(&history.root), before);
            }
        }
        let allowed = history.ready().admission(history.request).unwrap();
        assert_eq!(allowed.progress().charged_units, history.units);
        assert!(!allowed.progress().drained());
    }
}

#[test]
fn mediated_drain_rechecks_canonical_image_after_fallible_clock_acquisition() {
    for lane in LANES {
        let mut history = History::new(lane, Cut::Sent, false);
        let admitted = history.ready().admission(history.request).unwrap();
        history.stopped(admitted.history_anchor());
        let canonical = history.root.bytes();
        let valid_successor = history
            .ready()
            .drain(history.request, || Ok(ElapsedTick(4)))
            .unwrap();
        let alternate = history.root.bytes();
        assert_ne!(alternate, canonical);
        assert!(valid_successor.stopped().progress().drained());
        std::fs::write(history.root.store().join("delivery.bin"), &canonical).unwrap();
        history.pending();
        let run = history.ready();
        let calls = Cell::new(0);
        assert_eq!(
            run.drain(history.request, || {
                calls.set(calls.get() + 1);
                std::fs::write(history.root.store().join("delivery.bin"), &alternate).unwrap();
                Ok(ElapsedTick(4))
            })
            .unwrap_err(),
            Error::Binding.into()
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(history.root.bytes(), alternate);
        assert_eq!(
            std::fs::read(history.root.store().join("delivery.pending")).unwrap(),
            b"unconfirmed mediated candidate"
        );
        // Verify the real independently acknowledged successor can still be
        // consumed, without treating the rejected candidate as an acknowledgment.
        history.stopped(valid_successor.history_anchor());
        let retried = history
            .ready()
            .drain(history.request, || {
                panic!("the real successor is already drained")
            })
            .unwrap();
        assert_eq!(retried.history_anchor(), valid_successor.history_anchor());
    }
}
