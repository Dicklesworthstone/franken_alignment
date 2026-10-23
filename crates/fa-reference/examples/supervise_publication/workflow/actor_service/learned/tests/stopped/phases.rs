//! Actual selected operator phases and causal failures at their original cuts.
use super::*;
use recovery::StopPhase;

fn run(
    case: &Case,
    phase: StopPhase,
    clock: impl FnOnce() -> Result<ElapsedTick, JournalError>,
) -> (Result<(), String>, String) {
    let mut output = Vec::new();
    let result = command_stop::run_phase_with_elapsed(
        &case.config,
        &case.loaded,
        phase,
        801,
        clock,
        &mut || Duration::ZERO,
        &mut output,
    );
    (result, String::from_utf8(output).unwrap())
}

#[test]
fn learned_operator_phases_preserve_sent_liability_until_the_separate_native_drain() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated_at(mode, Some(RecoveryReserve::terminal()), true);
        case.config.timing.runtime_ms = 50;
        let reads = case.config.source.status().read_attempts;
        let active = files(&case.config.store);
        let (result, output) = run(&case, StopPhase::Drain, || {
            panic!("drain cannot stop an active history or acquire time")
        });
        assert!(result.unwrap_err().contains("WrongState"));
        assert!(output.is_empty());
        assert_eq!(files(&case.config.store), active);

        // Combined completion remains atomic: neither failed acquisition nor a
        // successful clock that consumed the remaining wall budget admits Stop.
        let (result, output) = run(&case, StopPhase::Combined, || {
            Err(JournalError::Unavailable)
        });
        assert!(result.unwrap_err().contains("Unavailable"));
        assert!(output.is_empty());
        assert_eq!(files(&case.config.store), active);
        assert!(observed(&case).stop.is_none());
        let elapsed = Cell::new(Duration::ZERO);
        let mut output = Vec::new();
        let result = command_stop::run_phase_with_elapsed(
            &case.config,
            &case.loaded,
            StopPhase::Combined,
            801,
            || {
                elapsed.set(Duration::from_millis(50));
                Ok(ElapsedTick(1001))
            },
            &mut || elapsed.get(),
            &mut output,
        );
        assert!(
            result
                .unwrap_err()
                .contains(recovery::STOP_ACQUISITION_DEADLINE)
        );
        assert!(output.is_empty());
        assert_eq!(files(&case.config.store), active);
        assert_eq!(observed(&case), case.before);

        let admitted_anchor = case.root.0.join("admitted.anchor");
        case.loaded.anchor_output = Some(admitted_anchor.clone());
        let (result, output) = run(&case, StopPhase::Admission, || {
            panic!("admission stop must be clock independent")
        });
        result.unwrap();
        assert!(output.contains("\"status\":\"admission_stopped\""));
        assert!(output.contains("\"drain_attempted\":false"));
        assert!(output.contains("\"drained\":false"));
        assert!(output.contains("\"appended_events\":2"));
        let admitted = observed(&case);
        assert_eq!(admitted.revision, case.before.revision + 2);
        assert_eq!(admitted.executions, 0);
        assert_eq!(
            admitted.control.ledger.charged,
            case.before.control.ledger.charged
        );
        assert!(admitted.control.ledger.charged > 0);
        assert_eq!(admitted.stop.as_ref().unwrap().request().operation, 801);
        assert_eq!(
            custody::load(&admitted_anchor).unwrap().revision(),
            admitted.revision
        );
        case.loaded.recovery_anchor = Some(custody::load(&admitted_anchor).unwrap());
        case.loaded.anchor_output = None;
        let acknowledged = files(&case.config.store);
        let (result, output) = run(&case, StopPhase::Admission, || {
            panic!("matching admission retry cannot drain or read time")
        });
        result.unwrap();
        assert!(output.contains("\"appended_events\":0"));
        assert_eq!(files(&case.config.store), acknowledged);

        // A failed later drain retains the acknowledged Stop, its charged
        // uncertainty and all staged bytes. No repeated admission is hidden.
        fs::write(
            case.config.store.join("delivery.pending"),
            b"separate-drain staged evidence",
        )
        .unwrap();
        let stopped = files(&case.config.store);
        for phase in [StopPhase::Admission, StopPhase::Drain, StopPhase::Combined] {
            let mut output = Vec::new();
            assert!(
                command_stop::run_phase_with_elapsed(
                    &case.config,
                    &case.loaded,
                    phase,
                    802,
                    || panic!("foreign stop identity has no clock"),
                    &mut || Duration::ZERO,
                    &mut output
                )
                .is_err()
            );
            assert!(output.is_empty());
            assert_eq!(files(&case.config.store), stopped);
        }
        let (result, output) = run(&case, StopPhase::Drain, || Err(JournalError::Unavailable));
        assert!(result.unwrap_err().contains("Unavailable"));
        assert!(output.is_empty());
        assert_eq!(files(&case.config.store), stopped);
        let elapsed = Cell::new(Duration::ZERO);
        let mut output = Vec::new();
        let result = command_stop::run_phase_with_elapsed(
            &case.config,
            &case.loaded,
            StopPhase::Drain,
            801,
            || {
                elapsed.set(Duration::from_millis(50));
                Ok(ElapsedTick(1001))
            },
            &mut || elapsed.get(),
            &mut output,
        );
        assert!(
            result
                .unwrap_err()
                .contains(recovery::STOP_ACQUISITION_DEADLINE)
        );
        assert!(output.is_empty());
        assert_eq!(files(&case.config.store), stopped);
        assert_eq!(observed(&case), admitted);

        let drained_anchor = case.root.0.join("drained.anchor");
        case.loaded.anchor_output = Some(drained_anchor.clone());
        let calls = Cell::new(0);
        let (result, output) = run(&case, StopPhase::Drain, || {
            calls.set(calls.get() + 1);
            Ok(ElapsedTick(1001))
        });
        result.unwrap();
        assert_eq!(calls.get(), 1);
        assert!(output.contains("\"status\":\"stopped_drained\""));
        let drained = observed(&case);
        assert_eq!(drained.revision, admitted.revision + 1);
        assert_eq!(drained.executions, 0);
        assert_eq!(drained.control.ledger.charged, 0);
        assert_eq!(
            custody::load(&drained_anchor).unwrap().revision(),
            drained.revision
        );
        case.loaded.recovery_anchor = Some(custody::load(&drained_anchor).unwrap());
        case.loaded.anchor_output = None;
        let completed = files(&case.config.store);
        for phase in [StopPhase::Admission, StopPhase::Drain, StopPhase::Combined] {
            let (result, output) = run(&case, phase, || {
                panic!("completed retry must not acquire another clock or append")
            });
            result.unwrap();
            assert!(output.contains(if phase == StopPhase::Admission {
                "\"appended_events\":0"
            } else {
                "\"recovery\":\"already_drained\""
            }));
            assert_eq!(files(&case.config.store), completed);
            assert_eq!(observed(&case), drained);
        }
        assert_eq!(case.config.source.status().read_attempts, reads);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        assert_eq!(fs::read_dir(case.root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn learned_phase_acquisition_cut_checks_both_sides_of_time_and_all_replay_phases() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated(mode, Some(RecoveryReserve::terminal()));
        case.config.timing.runtime_ms = 50;
        let limit = Duration::from_millis(50);
        let original = files(&case.config.store);
        let total = usize::try_from(case.before.revision).unwrap();
        for phase in [StopPhase::Admission, StopPhase::Drain, StopPhase::Combined] {
            for expire_at in [0, total, total + 1] {
                let mut calls = 0;
                let mut output = Vec::new();
                let result = command_stop::run_phase_with_elapsed(
                    &case.config,
                    &case.loaded,
                    phase,
                    801,
                    || panic!("replay expiry cannot acquire a clock"),
                    &mut || {
                        let current = calls;
                        calls += 1;
                        if current == expire_at {
                            limit
                        } else {
                            Duration::ZERO
                        }
                    },
                    &mut output,
                );
                assert!(result.unwrap_err().contains(EXPIRED));
                assert!(output.is_empty());
                assert_eq!(files(&case.config.store), original);
            }
        }
        let mut calls = 0;
        let mut output = Vec::new();
        let result = command_stop::run_phase_with_elapsed(
            &case.config,
            &case.loaded,
            StopPhase::Combined,
            801,
            || panic!("pre-acquisition expiry cannot call time"),
            &mut || {
                calls += 1;
                if calls == total + 3 {
                    limit
                } else {
                    Duration::ZERO
                }
            },
            &mut output,
        );
        assert!(
            result
                .unwrap_err()
                .contains(recovery::STOP_ACQUISITION_DEADLINE)
        );
        assert!(output.is_empty());
        assert_eq!(files(&case.config.store), original);
    }
}

#[test]
fn selected_phase_modes_keep_reserve_floor_and_anchor_refusals_before_mutation() {
    for mode in [None, Some(false), Some(true)] {
        for reserve in [
            None,
            Some(RecoveryReserve {
                events: 4,
                bytes: 60,
            }),
        ] {
            let case = generated(mode, reserve);
            let before = files(&case.config.store);
            for phase in [StopPhase::Admission, StopPhase::Drain, StopPhase::Combined] {
                let (result, output) = run(&case, phase, || {
                    panic!("wrong original terminal reserve cannot reach time")
                });
                assert!(result.unwrap_err().contains("Binding"));
                assert!(output.is_empty());
                assert_eq!(files(&case.config.store), before);
            }
        }
        let mut case = generated(mode, Some(RecoveryReserve::terminal()));
        let before = files(&case.config.store);
        let floor = case.loaded.floor;
        case.loaded.floor.journal_revision = case.before.revision + 1;
        for phase in [StopPhase::Admission, StopPhase::Drain, StopPhase::Combined] {
            let (result, output) = run(&case, phase, || panic!("failed floor cannot reach time"));
            assert!(result.unwrap_err().contains("Stale"));
            assert!(output.is_empty());
            assert_eq!(files(&case.config.store), before);
        }
        case.loaded.floor = floor;
        let mut foreign = generated(mode, Some(RecoveryReserve::terminal()));
        case.loaded.recovery_anchor = foreign.loaded.recovery_anchor.take();
        for phase in [StopPhase::Admission, StopPhase::Drain, StopPhase::Combined] {
            let (result, output) = run(&case, phase, || panic!("foreign anchor cannot reach time"));
            assert!(result.unwrap_err().contains("Binding"));
            assert!(output.is_empty());
            assert_eq!(files(&case.config.store), before);
        }
    }
}

#[test]
fn actual_learned_phase_cli_retains_exact_cuts_without_live_assets_or_peers() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode, Some(RecoveryReserve::terminal()));
        let config = write_config(&case.root);
        fs::remove_dir(case.root.0.join("peers")).unwrap();
        let admitted = case.root.0.join("cli-admitted.anchor");
        let drained = case.root.0.join("cli-drained.anchor");
        let missing = case.root.0.join("missing-activation");
        let args = |phase: &str, input: &Path, output: &Path| {
            vec![
                "create-learned-generated".into(),
                config.display().to_string(),
                case.recipe.display().to_string(),
                phase.into(),
                "801".into(),
                "--history-anchor".into(),
                input.display().to_string(),
                "--retain-history-anchor".into(),
                output.display().to_string(),
                "--credibility-activation".into(),
                missing.display().to_string(),
            ]
        };
        crate::command(args("--stop-admission", &case.anchor, &admitted)).unwrap();
        let state = observed(&case);
        assert_eq!(state.revision, case.before.revision + 2);
        assert_eq!(custody::load(&admitted).unwrap().revision(), state.revision);
        let bytes = files(&case.config.store);
        // With no dispatched liability admission is already drained; this
        // explicit separate command confirms the same cut without spending time.
        crate::command(args("--drain-stopped", &admitted, &drained)).unwrap();
        assert_eq!(files(&case.config.store), bytes);
        assert_eq!(observed(&case), state);
        assert_eq!(fs::read(&admitted).unwrap(), fs::read(&drained).unwrap());
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        assert!(!case.root.0.join("peers").exists());
        assert!(!missing.exists());
    }
}

#[test]
fn learned_terminal_option_grammar_refuses_live_actor_and_conflicts_before_file_admission() {
    let root = Root::new();
    let absent = root.0.join("absent");
    for phase in ["--recover-stop", "--stop-admission", "--drain-stopped"] {
        let base = vec![
            "create-learned-generated".into(),
            absent.display().to_string(),
            absent.display().to_string(),
            phase.into(),
            "801".into(),
        ];
        for tail in [
            vec!["--actor-profile", "/absent-actor"],
            vec!["--drain-stopped", "801"],
            vec!["--continue-generation"],
            vec!["--topology-profile"],
            vec!["--history-anchor"],
            vec!["--unrecognized"],
        ] {
            let mut args = base.clone();
            args.extend(tail.into_iter().map(str::to_owned));
            let error = command_with_clock(&args, None, || {
                panic!("invalid terminal selection cannot enter a live workflow")
            })
            .unwrap_err();
            assert!(
                !error.contains("No such file"),
                "grammar must refuse before CONFIG: {error}"
            );
        }
        for operation in ["0", "-1", "+1", "18446744073709551616"] {
            let mut args = base.clone();
            args[4] = operation.into();
            let error =
                command_with_clock(&args, None, || panic!("invalid operation reads no time"))
                    .unwrap_err();
            assert_eq!(error, USAGE);
        }
    }
    assert!(!absent.exists());
    assert!(!root.0.join("store").exists());
}

#[test]
fn admission_report_or_export_failure_keeps_the_acknowledged_phase_and_exact_retry() {
    let mut case = generated(None, Some(RecoveryReserve::terminal()));
    let retained = case.root.0.join("admission-output.anchor");
    case.loaded.anchor_output = Some(retained.clone());
    let mut failed = FailedOutput { writes: 0 };
    let error = command_stop::run_phase_with_elapsed(
        &case.config,
        &case.loaded,
        StopPhase::Admission,
        801,
        || panic!("admission never needs time"),
        &mut || Duration::ZERO,
        &mut failed,
    )
    .unwrap_err();
    assert!(error.contains("native admission stop acknowledged"));
    assert!(error.contains("output failed"));
    assert_eq!(failed.writes, 1);
    let state = observed(&case);
    assert_eq!(state.revision, case.before.revision + 2);
    assert_eq!(custody::load(&retained).unwrap().revision(), state.revision);
    let files = files(&case.config.store);
    let archive = fs::read(&retained).unwrap();
    let (result, output) = run(&case, StopPhase::Admission, || {
        panic!("retry cannot read time")
    });
    let error = result.unwrap_err();
    assert!(error.contains("history anchor retention failed"));
    assert!(output.contains("\"appended_events\":0"));
    assert_eq!(super::files(&case.config.store), files);
    assert_eq!(fs::read(&retained).unwrap(), archive);
    case.loaded.anchor_output = None;
    let (result, output) = run(&case, StopPhase::Admission, || {
        panic!("retry cannot read time")
    });
    result.unwrap();
    assert!(output.contains("\"appended_events\":0"));
    assert_eq!(observed(&case), state);
    assert_eq!(super::files(&case.config.store), files);
}
