//! Offline stop through the executable's original guarded learned recovery.
//! Model, forecast source, policy acquisition and file history are real. These
//! bounded fixtures do not qualify elapsed latency or independent anchor custody.
use super::*;
use super::super::{anchors as custody, prediction, stopped as command_stop};
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot,
    observed::{consistency::learned::FileLearnedConsistencySnapshot,
        guarded::anchored::MAX_HISTORY_ANCHOR_BYTES, shutdown::recovery::FileStoppedRecovery}};
use std::{cell::Cell, collections::BTreeMap, io, path::{Path, PathBuf}};

const EXPIRED: &str = "learned stop recovery replay deadline reached";

struct Case {
    root: Root, config: Config, loaded: Loaded, recipe: PathBuf,
    anchor: PathBuf, archive: Vec<u8>, before: FileDeliverySnapshot,
    learned: Option<FileLearnedConsistencySnapshot>,
}

fn generated(mode: Option<bool>, reserve: Option<RecoveryReserve>) -> Case {
    generated_at(mode, reserve, false)
}

fn generated_at(mode: Option<bool>, reserve: Option<RecoveryReserve>, dispatched: bool) -> Case {
    let root = Root::new(); let mut config = configured(&root);
    let path = fixture::write(&root, b"allow");
    if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, observer) = match (mode, reserve) {
        (None, None) => {
            let (host, reviewer) = FileOversight::create_with_learned_text_stream(
                &config.store, config.profile.clone(), loaded.generation.clone()).unwrap();
            (host, reviewer, None)
        }
        (None, Some(reserve)) => {
            let (host, reviewer) = FileOversight::create_with_learned_text_stream_with_reserve(
                &config.store, config.profile.clone(), loaded.generation.clone(), reserve).unwrap();
            (host, reviewer, None)
        }
        (Some(_), reserve) => {
            let (host, reviewer, observer) = match reserve {
                None => FileOversight::create_with_pre_output_forecast(
                    &config.store, config.profile.clone(), loaded.generation.clone()),
                Some(reserve) => FileOversight::create_with_pre_output_forecast_with_reserve(
                    &config.store, config.profile.clone(), loaded.generation.clone(), reserve),
            }.unwrap();
            (host, reviewer, Some(observer))
        }
    };
    let mut prediction = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &deadline(), &mut || ElapsedTick(1000), prediction.as_mut()).unwrap());
    if dispatched {
        let deadline = deadline();
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
        let (reviewed, history) = super::continuation::review_current(
            &mut config, &loaded, &reviewer, driver, &deadline);
        driver = reviewed;
        let human = super::continuation::approve(&mut driver, &mut config, &reviewer, loaded.request);
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
        assert!(driver.supervisor().host().unwrap().inspect().control.ledger.charged > 0);
        drop(history);
    }
    let host = driver.supervisor().host().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 4); assert_eq!(numerical.sampled_draws, 2);
    assert!(numerical.work.accepted_decoder.scalar_products().unwrap() > 0);
    let before = host.inspect(); assert_eq!(before.executions, 0); assert!(before.stop.is_none());
    let learned = (mode == Some(true)).then(|| host.learned_action_consistency_snapshot().unwrap());
    let mut archive = Vec::new();
    host.history_anchor().unwrap().write_to(&mut archive, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    drop(host); drop(wire); drop(prediction); assert_eq!(cleanup(driver, 1000, 1), 0);
    let anchor = root.0.join("independent-stop.anchor"); fs::write(&anchor, &archive).unwrap();
    // Recovery reloads the independent recipe without native helper intake.
    let mut loaded = recipe::load(&path, &config, false).unwrap(); assert!(loaded.native.is_none());
    loaded.recovery_anchor = Some(custody::load(&anchor).unwrap());
    for name in ["evidence.json", "native-roster.json", "helper-model.json",
        "helper-weights.safetensors", "helper-monitor.json", "helper-tokenizer.bbpe", "helper-sampling.json"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::write(config.store.join("delivery.pending"), b"retained offline-stop staging").unwrap();
    Case { root, config, loaded, recipe: path, anchor, archive, before, learned }
}

fn files(store: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(store).unwrap().map(|entry| {
        let entry = entry.unwrap(); assert!(entry.file_type().unwrap().is_file());
        (entry.file_name().into_string().unwrap(), fs::read(entry.path()).unwrap())
    }).collect()
}

fn observed(case: &Case) -> FileDeliverySnapshot {
    if let Some(predictor) = case.loaded.generation.required_owned_pre_output_forecast() {
        let current = FileOversight::read_publication_with_owned_learned_consistency(
            &case.config.store, &case.config.profile, &case.loaded.generation, predictor).unwrap();
        let prior = case.learned.as_ref().unwrap();
        assert_eq!(current.consistency.work, prior.work);
        assert_eq!(current.consistency.retained_source_bytes, prior.retained_source_bytes);
        assert_eq!(current.consistency.consistency.evidence, prior.consistency.evidence);
        current.publication
    } else {
        FileOversight::read_publication_with_learned_generation(
            &case.config.store, &case.config.profile, &case.loaded.generation).unwrap()
    }
}

fn report(bytes: Vec<u8>, recovery: &str, revision: u64) {
    let report = String::from_utf8(bytes).unwrap();
    assert!(report.contains("\"status\":\"stopped_drained\""));
    assert!(report.contains(&format!("\"recovery\":\"{recovery}\"")));
    assert!(report.contains("\"operation\":\"801\""));
    assert!(report.contains(&format!("\"revision\":\"{revision}\"")));
    assert!(report.contains("\"executions\":\"0\""));
    assert!(report.contains("\"charged\":\"0\""));
}

#[test]
fn operator_stop_reports_original_terminal_completion_and_exact_clock_free_retry_in_every_mode() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode, Some(RecoveryReserve::terminal()));
        let reads = case.config.source.status().read_attempts; let calls = Cell::new(0); let mut output = Vec::new();
        command_stop::run(&case.config, &case.loaded, 801, || {
            calls.set(calls.get() + 1); ElapsedTick(1001)
        }, &mut output).unwrap();
        assert_eq!(calls.get(), 1); report(output, "advanced", case.before.revision + 3);
        let state = observed(&case); assert_eq!(state.revision, case.before.revision + 3);
        assert_eq!(state.stop.as_ref().unwrap().request().operation, 801);
        assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.reserved, 0);
        let completed = files(&case.config.store); let mut output = Vec::new();
        command_stop::run(&case.config, &case.loaded, 801,
            || panic!("already drained stop reads no terminal clock"), &mut output).unwrap();
        report(output, "already_drained", state.revision);
        assert_eq!(files(&case.config.store), completed); assert_eq!(observed(&case), state);
        assert_eq!(case.config.source.status().read_attempts, reads);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        assert_eq!(fs::read_dir(case.root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn stop_wall_budget_cancels_every_preopen_event_and_ready_cut_before_cleanup_or_terminal_clock() {
    for mode in [None, Some(false), Some(true)] {
        let mut case = generated(mode, Some(RecoveryReserve::terminal())); case.config.timing.runtime_ms = 50;
        let total = usize::try_from(case.before.revision).unwrap(); assert!(total > 4);
        let original = files(&case.config.store); let reads = case.config.source.status().read_attempts;
        let limit = Duration::from_millis(50); let before = limit.checked_sub(Duration::from_nanos(1)).unwrap();
        for expire_at in 0..=total + 1 {
            let mut calls = 0;
            let result = recovery::stop_selected_with_elapsed(&case.config, &case.loaded, 801,
                || panic!("cancelled replay cannot acquire terminal time"), &mut || {
                    assert_eq!(files(&case.config.store), original);
                    let cut = calls; calls += 1; if cut == expire_at { limit } else { before }
                });
            assert_eq!(result.err().as_deref(), Some(EXPIRED), "mode {mode:?}, cut {expire_at}");
            assert_eq!(calls, expire_at + 1); assert_eq!(files(&case.config.store), original);
            // Reacquiring the same original writer proves cancellation released
            // custody, even when the prior callback ran at verified Ready.
            let mut retry_calls = 0;
            let retry = recovery::stop_selected_with_elapsed(&case.config, &case.loaded, 801,
                || panic!("second replay is cancelled too"), &mut || {
                    retry_calls += 1; if retry_calls == 1 { Duration::ZERO } else { limit }
                });
            assert_eq!(retry.err().as_deref(), Some(EXPIRED)); assert_eq!(retry_calls, 2);
            assert_eq!(files(&case.config.store), original);
        }
        let mut calls = 0; let clocks = Cell::new(0);
        let result = recovery::stop_selected_with_elapsed(&case.config, &case.loaded, 801, || {
            clocks.set(clocks.get() + 1); ElapsedTick(1001)
        }, &mut || { calls += 1; before }).unwrap();
        assert_eq!(calls, total + 2); assert_eq!(clocks.get(), 1);
        assert!(matches!(result, FileStoppedRecovery::Advanced { .. }));
        assert!(result.progress().drained()); assert_eq!(result.snapshot().revision, case.before.revision + 3);
        assert_eq!(observed(&case), *result.snapshot());
        assert_eq!(case.config.source.status().read_attempts, reads);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    }
}

#[test]
fn offline_stop_keeps_exact_reserve_anchor_and_guard_admission_without_a_live_fallback() {
    for mode in [None, Some(false), Some(true)] {
        for reserve in [None, Some(RecoveryReserve { events: 4, bytes: 60 })] {
            let case = generated(mode, reserve); let original = files(&case.config.store); let mut cuts = 0;
            let result = recovery::stop_selected_with_elapsed(&case.config, &case.loaded, 801,
                || panic!("wrong reserve has no clock"), &mut || { cuts += 1; Duration::ZERO });
            assert_eq!(result.unwrap_err(), debug(JournalError::Contract(Error::Binding)));
            assert_eq!(cuts, 1); assert_eq!(files(&case.config.store), original);
        }
        let mut case = generated(mode, Some(RecoveryReserve::terminal())); let original = files(&case.config.store);
        let floor = case.loaded.floor; case.loaded.floor.journal_revision = case.before.revision + 1;
        let result = recovery::stop_selected_with_elapsed(&case.config, &case.loaded, 801,
            || panic!("failed guard has no clock"), &mut || Duration::ZERO);
        assert_eq!(result.unwrap_err(), debug(JournalError::Contract(Error::Stale)));
        assert_eq!(files(&case.config.store), original); case.loaded.floor = floor;
        let mut foreign = generated(mode, Some(RecoveryReserve::terminal()));
        assert_eq!(foreign.before.revision, case.before.revision);
        let own = case.loaded.recovery_anchor.take(); case.loaded.recovery_anchor = foreign.loaded.recovery_anchor.take();
        let result = recovery::stop_selected_with_elapsed(&case.config, &case.loaded, 801,
            || panic!("foreign anchor has no clock"), &mut || Duration::ZERO);
        assert_eq!(result.unwrap_err(), debug(JournalError::Contract(Error::Binding)));
        assert_eq!(files(&case.config.store), original); case.loaded.recovery_anchor = own;
        let result = recovery::stop_selected_with_elapsed(&case.config, &case.loaded, 801,
            || ElapsedTick(1001), &mut || Duration::ZERO).unwrap();
        assert!(result.progress().drained()); assert_eq!(observed(&case), *result.snapshot());
    }
}

fn write_config(root: &Root) -> PathBuf {
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"))
        .replace("/operator/publication-store", root.0.join("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap())
        .replace("\"initial_payload_hex\": \"696e697469616c\"", "\"initial_payload_hex\": \"\"")
        .replace("\"events\": 256", "\"events\": 1024");
    let path = root.0.join("config.json"); fs::write(&path, text).unwrap(); path
}

#[test]
fn actual_offline_stop_cli_needs_no_live_assets_peer_profile_or_credibility_capsule() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode, Some(RecoveryReserve::terminal())); let path = write_config(&case.root);
        let disk_config = Config::read(&path).unwrap();
        crate::config::assert_oversight_profiles_eq(&disk_config.profile, &case.config.profile);
        assert_eq!(disk_config.source_policy, case.config.source_policy);
        fs::remove_dir(case.root.0.join("peers")).unwrap();
        let missing = case.root.0.join("missing-qualification-capsule");
        let args = vec!["create-learned-generated".into(), path.to_str().unwrap().into(),
            case.recipe.to_str().unwrap().into(), "--recover-stop".into(), "801".into(),
            "--history-anchor".into(), case.anchor.to_str().unwrap().into(),
            "--credibility-activation".into(), missing.to_str().unwrap().into()];
        crate::command(args.clone()).unwrap();
        let state = observed(&case); assert_eq!(state.revision, case.before.revision + 3);
        assert_eq!(state.stop.as_ref().unwrap().request().operation, 801); assert_eq!(state.executions, 0);
        let stopped = files(&case.config.store); crate::command(args).unwrap();
        assert_eq!(observed(&case), state); assert_eq!(files(&case.config.store), stopped);
        assert!(!missing.exists()); assert!(!case.root.0.join("peers").exists());
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    }
    // A supported output cannot bypass configuration admission or create an
    // archive/authority store when the selected configuration is unavailable.
    let root = Root::new(); let absent = root.0.join("absent"); let output = root.0.join("new.anchor");
    let args = vec!["create-learned-generated".into(), absent.to_str().unwrap().into(),
        absent.to_str().unwrap().into(), "--recover-stop".into(), "801".into(),
        "--retain-history-anchor".into(), output.to_str().unwrap().into()];
    let error = command(&args, None).unwrap_err(); assert_eq!(error, Config::read(&absent).err().unwrap());
    assert!(!output.exists()); assert!(!root.0.join("store").exists());
}

struct FailedOutput { writes: usize }
impl io::Write for FailedOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.writes += 1; Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> { panic!("write failure cannot be reported as flushed") }
}

#[test]
fn lost_stop_report_preserves_acknowledged_history_and_retry_only_reports_the_completed_cut() {
    for mode in [None, Some(false), Some(true)] {
        let case = generated(mode, Some(RecoveryReserve::terminal())); let mut output = FailedOutput { writes: 0 };
        let error = command_stop::run(&case.config, &case.loaded, 801,
            || ElapsedTick(1001), &mut output).unwrap_err();
        assert!(error.contains("native stop acknowledged")); assert!(error.contains("output failed"));
        assert_eq!(output.writes, 1);
        let state = observed(&case); assert_eq!(state.revision, case.before.revision + 3);
        assert!(state.stop.is_some()); assert_eq!(state.executions, 0);
        let original = files(&case.config.store); let mut output = Vec::new();
        command_stop::run(&case.config, &case.loaded, 801,
            || panic!("lost output cannot repeat native stopping or acquire new time"), &mut output).unwrap();
        report(output, "already_drained", state.revision);
        assert_eq!(files(&case.config.store), original); assert_eq!(observed(&case), state);
        let mut retained = recipe::load(&case.recipe, &case.config, false).unwrap();
        let archive = case.root.0.join("retained-terminal.anchor");
        retained.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
        retained.anchor_output = Some(archive.clone());
        let mut output = Vec::new();
        command_stop::run(&case.config, &retained, 801,
            || panic!("retaining a completed stop cannot acquire time"), &mut output).unwrap();
        report(output, "already_drained", state.revision);
        assert_eq!(files(&case.config.store), original);
        assert_eq!(custody::load(&archive).unwrap().revision(), state.revision);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    }
}

#[path = "stopped/anchor_export.rs"] mod anchor_export;
#[path = "stopped/phases.rs"] mod phases;
