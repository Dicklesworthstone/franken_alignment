//! Real mediated numerical reset, paired source custody, and authenticated Stop.
//! Missing live files prove this operator does not silently rearm the old graph.
use super::*;
use super::super::mediated_topology as fixture_topology;
use super::super::super::topology as topology_custody;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedRecoveryStatus,
    driver::topology::checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES},
    guarded::mediated::{FileMediatedRequirements, stream_reset::FileStreamResetTopologyRecovery},
};
use std::io::Cursor;

struct SourceCase {
    case: Case,
    profile: PathBuf,
    saved: TopologyCheckpoint,
    before_intent: TopologyCheckpoint,
}

fn checkpoint(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}

fn profile(root: &Root, saved: &TopologyCheckpoint, source: &str, output: Option<&str>) -> PathBuf {
    let path = root.0.join("reset-topology.json");
    let output = output.map_or(String::new(), |output|
        format!(r#", "retain_checkpoint":"{}""#, root.0.join(output).display()));
    fs::write(&path, format!(r#"{{"schema_version":"fa.mediated-submit/2",
        "initial":{{"topology_path":"{}","inventory_path":"{}"}},
        "next":{{"topology_path":"{}","inventory_path":"{}"}},
        "operation":{},"source_checkpoint":"{}"{output}}}"#,
        root.0.join("initial.json").display(), root.0.join("initial-inventory.json").display(),
        root.0.join("missing-next.json").display(), root.0.join("missing-next-inventory.json").display(),
        saved.last_operation() + 1, root.0.join(source).display())).unwrap();
    path
}

fn select(case: &mut Case, profile: &Path) {
    case.loaded.topology = None;
    topology_custody::admit(&case.config, &mut case.loaded, Some(profile),
        topology_custody::Mode::Live).unwrap();
}

fn prepared(root: &Root, audit_failure: bool) -> SourceCase {
    let (mut config, recipe, mut loaded) = fixture_topology::selected(root, None, false);
    let original = &config.profile.delivery.actor;
    let mut declared = original.profile();
    declared.grade = RestartGrade::FunctionalRestart;
    config.profile.delivery.actor = ActorState::new(declared, original.tokens().to_vec(),
        original.cache().to_vec(), original.sampler().to_vec(), original.next_position()).unwrap();
    let limit = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let started = topology_custody::start(&config, &loaded, false, &mut control,
        &limit, &mut || ElapsedTick(1000)).unwrap();
    let (mut host, reviewer, _, _) = started.custody;
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    let mut driver = FileSupervisedDriver::new(supervisor);
    topology_custody::attach(&mut driver, started.source).unwrap();
    topology_custody::certify(&mut driver, &limit, &mut || ElapsedTick(1000)).unwrap();
    let saved = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let initial = host.learned_generation_inspection().unwrap().numerical;
        host.advance_learned_generation(host.revision(), initial.actor_revision, initial.position)
            .unwrap().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 1);
        host.capture_learned_stream_checkpoint(host.revision(), 1, numerical.actor_revision,
            host.inspect().control.ledger.epoch).unwrap()
    };
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control, &limit,
        &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &limit,
        &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = super::super::continuation::review_current(
        &mut config, &loaded, &reviewer, driver, &limit);
    let before_intent = {
        let host = driver.supervisor().host().unwrap();
        driver.topology().unwrap().checkpoint(&host,
            &fs::read(root.0.join("initial.json")).unwrap(),
            &fs::read(root.0.join("initial-inventory.json")).unwrap()).unwrap()
    };
    let (request, before, numerical) = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let current = host.inspect();
        let first = current.control.sequence;
        assert!(first > saved.info().control_sequence);
        assert_eq!(current.executions, 0);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
        let mut budget = allowance();
        if audit_failure { budget.audit.monitoring.probe_coordinates = 0; }
        let intent = FileLearnedResetIntent::for_recovery(saved.id(), FileResetRequest {
            operation: 900, expected_control_sequence: first,
            expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
            expected_authority_epoch: current.control.ledger.epoch,
            binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
            retained_targets: vec![current.target],
        }, budget).unwrap();
        host.begin_learned_stream_reset(host.revision(), &saved, intent.control().clone(),
            intent.budget(), first).unwrap();
        loaded.recovery_anchor = Some(host.history_anchor().unwrap());
        loaded.anchor_output = Some(root.0.join("next.anchor"));
        (reset::Request { intent, first_incident_sequence: first }, host.inspect(),
            host.learned_generation_inspection().unwrap().numerical)
    };
    {
        let host = driver.supervisor().host().unwrap();
        started.output.as_ref().unwrap().retain(&host, driver.topology()).unwrap();
    }
    drop(history); drop(driver); drop(wire); drop(reviewer);
    let saved = fixture_topology::checkpoint(root, 1);
    assert!(saved.available()); assert!(saved.work().checks > 0);
    assert_eq!(saved.history_anchor(), loaded.recovery_anchor.as_ref().unwrap());
    let profile = profile(root, &saved, "source-1.bin", Some("source-reset.bin"));
    let mut case = Case { config, recipe, loaded, request, before, numerical };
    select(&mut case, &profile);
    SourceCase { case, profile, saved, before_intent }
}

fn arguments(root: &Root, case: &SourceCase) -> Vec<String> {
    let mut result = args(root, &case.case);
    result.extend(["--topology-profile".into(), case.profile.display().to_string()]);
    result
}

fn unchanged_source(original: &TopologyCheckpoint, next: &TopologyCheckpoint) {
    assert_eq!(next.topology_bytes(), original.topology_bytes());
    assert_eq!(next.inventory_bytes(), original.inventory_bytes());
    assert_eq!(next.expected(), original.expected());
    assert_eq!(next.work(), original.work());
    assert_eq!(next.last_operation(), original.last_operation());
    assert!(next.minimum().control_sequence >= original.minimum().control_sequence);
    assert!(next.minimum().authority_epoch >= original.minimum().authority_epoch);
    assert!(!next.available(), "recovery Fence cannot rearm the graph");
}

fn remove_live_inputs(root: &Root) {
    for name in ["live-1.json", "inventory-1.json", "evidence.json", "native-roster.json"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
}

#[test]
fn topology_command_restores_and_retains_exact_source_work_without_live_files() {
    let root = Root::new(); let case = prepared(&root, false);
    let args = arguments(&root, &case);
    let old_anchor = fs::read(&args[6]).unwrap();
    let old_source = fs::read(root.0.join("source-1.bin")).unwrap();
    remove_live_inputs(&root);
    let mut output = Vec::new();
    reset::command(&args, None, || ElapsedTick(1000), &mut output).unwrap();
    let value = report(&output);
    assert_eq!(value.get("outcome").and_then(Json::as_str), Some("restored"));
    assert_eq!(value.get("topology_checkpoint_retained").and_then(Json::as_bool), Some(true));
    assert_eq!(value.get("history_anchor_retained").and_then(Json::as_bool), Some(true));
    assert_eq!(value.get("restart_attempts").and_then(Json::as_str), Some("1"));
    let retained = checkpoint(&root.0.join("source-reset.bin"));
    unchanged_source(&case.saved, &retained);
    assert_eq!(retained.history_anchor(), &custody::load(Path::new(&args[8])).unwrap());
    assert_eq!(retained.history_anchor().revision(), case.case.before.revision + 2);
    assert_eq!(snapshot(&case.case).executions, 0);
    assert_eq!(fs::read(&args[6]).unwrap(), old_anchor);
    assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), old_source);
    assert!(!root.0.join("missing-next.json").exists());
}

#[test]
fn lost_source_export_retries_the_original_pair_without_a_second_audit_or_source_read() {
    let root = Root::new(); let mut case = prepared(&root, false);
    let old_source = fs::read(root.0.join("source-1.bin")).unwrap();
    let original_anchor = case.case.loaded.recovery_anchor.as_ref().unwrap().clone();
    let mut control = Control::new(&case.case.config, 900, None).unwrap();
    let occupied = root.0.join("source-reset.bin");
    let mut first = true; let mut output = Vec::new();
    let error = reset::run(&case.case.config, &case.case.loaded, &case.case.request,
        &mut control, &deadline(), &mut || {
            // The destination appears AFTER admission but BEFORE persistence.
            // Its create-new refusal must not hide acknowledged reset completion.
            if first { fs::write(&occupied, b"independent destination appeared").unwrap(); first = false; }
            ElapsedTick(1000)
        }, &mut output).unwrap_err();
    assert!(error.contains("native stream reset acknowledged"), "{error}");
    assert!(error.contains("topology checkpoint retention failed"), "{error}");
    assert_eq!(fs::read(&occupied).unwrap(), b"independent destination appeared");
    let first = report(&output);
    assert_eq!(first.get("outcome").and_then(Json::as_str), Some("restored"));
    assert_eq!(first.get("history_anchor_retained").and_then(Json::as_bool), Some(true));
    assert_eq!(first.get("topology_checkpoint_retained").and_then(Json::as_bool), Some(false));
    assert_eq!(snapshot(&case.case).revision, case.case.before.revision + 2);
    case.case.loaded.anchor_output = Some(root.0.join("retry.anchor"));
    case.profile = profile(&root, &case.saved, "source-1.bin", Some("source-retry.bin"));
    select(&mut case.case, &case.profile);
    remove_live_inputs(&root);
    let args = arguments(&root, &case); let mut output = Vec::new();
    reset::command(&args, None, || ElapsedTick(1000), &mut output).unwrap();
    let retry = report(&output);
    for field in ["restart_attempts", "reserved_cache_values", "values_restored",
        "incident_count", "charged", "refunded_units", "position", "sampled_draws"] {
        assert_eq!(retry.get(field), first.get(field), "{field}");
    }
    let retained = checkpoint(&root.0.join("source-retry.bin"));
    unchanged_source(&case.saved, &retained);
    assert_eq!(retained.history_anchor(), &custody::load(Path::new(&args[8])).unwrap());
    assert_eq!(retained.history_anchor().revision(), case.case.before.revision + 3);
    assert_eq!(case.case.loaded.recovery_anchor.as_ref().unwrap(), &original_anchor);
    assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), old_source);
}

#[test]
fn a_recorded_audit_refusal_still_retains_both_acknowledged_topology_archives() {
    let root = Root::new(); let case = prepared(&root, true);
    remove_live_inputs(&root);
    let mut output = Vec::new();
    let error = reset::command(&arguments(&root, &case), None,
        || ElapsedTick(1000), &mut output).unwrap_err();
    assert!(error.contains("acknowledged refusal"), "{error}");
    let value = report(&output);
    assert_eq!(value.get("outcome").and_then(Json::as_str), Some("refused"));
    assert_eq!(value.get("restored").and_then(Json::as_bool), Some(false));
    assert_eq!(value.get("restart_attempts").and_then(Json::as_str), Some("1"));
    assert_eq!(value.get("topology_checkpoint_retained").and_then(Json::as_bool), Some(true));
    assert_eq!(value.get("history_anchor_retained").and_then(Json::as_bool), Some(true));
    let retained = checkpoint(&root.0.join("source-reset.bin"));
    unchanged_source(&case.saved, &retained);
    assert_eq!(retained.history_anchor().revision(), case.case.before.revision + 2);
    assert_eq!(retained.history_anchor(),
        &custody::load(case.case.loaded.anchor_output.as_ref().unwrap()).unwrap());
}

#[test]
fn topology_reset_requires_paired_v2_custody_and_distinct_new_outputs() {
    let root = Root::new(); let case = prepared(&root, false);
    let mut args = arguments(&root, &case);
    let canonical = disk(&case.case);
    let staging = case.case.config.store.join("delivery.pending");
    fs::write(&staging, b"rejected custody never reaches cleanup").unwrap();
    for (output, fragment) in [(None, "new retained topology checkpoint"),
        (Some("next.anchor"), "distinct new files")] {
        let selected = profile(&root, &case.saved, "source-1.bin", output);
        assert_eq!(selected, case.profile);
        let error = reset::command(&args, None,
            || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
        assert!(error.contains(fragment), "{error}");
        assert_eq!(disk(&case.case), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"rejected custody never reaches cleanup");
    }
    let pair = format!(r#"{{"topology_path":"{}","inventory_path":"{}"}}"#,
        root.0.join("initial.json").display(), root.0.join("initial-inventory.json").display());
    fs::write(&case.profile, format!(r#"{{"schema_version":"fa.mediated-submit/1",
        "initial":{pair},"current":{pair},"next":{pair},"available":true,
        "operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
        case.saved.last_operation() + 1, case.saved.work().checks,
        case.saved.work().reserved_read_bytes, case.saved.last_operation())).unwrap();
    let error = reset::command(&args, None,
        || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("exact v2 topology checkpoint"), "{error}");
    assert_eq!(disk(&case.case), canonical);
    assert_eq!(fs::read(&staging).unwrap(), b"rejected custody never reaches cleanup");
    profile(&root, &case.saved, "source-1.bin", Some("source-reset.bin"));
    let earlier_anchor = root.0.join("earlier.anchor");
    let mut bytes = Vec::new();
    case.before_intent.history_anchor().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    fs::write(&earlier_anchor, bytes).unwrap();
    args[6] = earlier_anchor.display().to_string();
    let error = reset::command(&args, None,
        || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("different cuts"), "{error}");
    assert_eq!(disk(&case.case), canonical);
    assert_eq!(fs::read(&staging).unwrap(), b"rejected custody never reaches cleanup");
    assert!(!case.case.loaded.anchor_output.as_ref().unwrap().exists());
    assert!(!root.0.join("source-reset.bin").exists());
}

fn native_ready(case: &SourceCase) -> FileStreamResetTopologyRecovery {
    let history = topology_custody::history(&case.case.loaded).unwrap().unwrap();
    let expected = FileMediatedRequirements {
        oversight: recovery::requirements(&case.case.config, &case.case.loaded),
        topology: history.requirement.clone(), prediction: None, evaluation: None,
    };
    let mut run = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        &case.case.config.store, case.case.config.profile.clone(), &expected,
        &case.case.loaded.generation, case.saved.history_anchor()).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
        .require_stream_reset_checkpoint(&case.saved, &case.case.request.intent,
            case.case.request.first_incident_sequence).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    run
}

#[test]
fn source_successor_and_operator_retry_refuse_an_unrelated_native_time_event() {
    let root = Root::new(); let case = prepared(&root, false);
    let (mut host, _roles) = native_ready(&case).finish_pending_stream_reset().unwrap();
    let retained = case.saved.after_learned_stream_reset(&host,
        &case.case.request.intent, case.case.request.first_incident_sequence).unwrap();
    unchanged_source(&case.saved, &retained);
    let usage = host.learned_recovery_usage().unwrap();
    let incident = host.actor_snapshot().unwrap().incident_count;
    // An ordinary, valid source-free event is still outside the closed reset
    // successor. Its presence cannot silently bless older source custody.
    host.observe_time(host.revision(), ElapsedTick(1001)).unwrap();
    assert_eq!(case.saved.after_learned_stream_reset(&host,
        &case.case.request.intent, case.case.request.first_incident_sequence).err(),
        Some(fa_reference::Error::Stale.into()));
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, incident);
    drop(host);
    let original = disk(&case.case);
    let staging = case.case.config.store.join("delivery.pending");
    fs::write(&staging, b"unrelated suffix is not reset custody").unwrap();
    let error = reset::command(&arguments(&root, &case), None,
        || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("Stale"), "{error}");
    assert_eq!(disk(&case.case), original);
    assert_eq!(fs::read(&staging).unwrap(), b"unrelated suffix is not reset custody");
    assert!(!case.case.loaded.anchor_output.as_ref().unwrap().exists());
    assert!(!root.0.join("source-reset.bin").exists());
}

#[test]
fn topology_reset_requires_the_recorded_intent_and_frontier_in_the_retained_prefix() {
    let root = Root::new(); let mut case = prepared(&root, false);
    let canonical = disk(&case.case);
    let staging = case.case.config.store.join("delivery.pending");
    fs::write(&staging, b"retain rejected original prefix").unwrap();
    for changed in 0..3 {
        let mut request = case.case.request.clone();
        match changed {
            0 => request.first_incident_sequence += 1,
            1 => {
                let mut control = request.intent.control().clone(); control.operation += 1;
                request.intent = FileLearnedResetIntent::for_recovery(request.intent.checkpoint(),
                    control, request.intent.budget()).unwrap();
            }
            _ => {
                let mut budget = request.intent.budget(); budget.cache_values -= 1;
                request.intent = FileLearnedResetIntent::for_recovery(request.intent.checkpoint(),
                    request.intent.control().clone(), budget).unwrap();
            }
        }
        let mut control = Control::new(&case.case.config, request.intent.control().operation, None).unwrap();
        let error = reset::run(&case.case.config, &case.case.loaded, &request,
            &mut control, &deadline(), &mut || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
        assert!(error.contains("Binding"), "{error}");
        assert_eq!(disk(&case.case), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"retain rejected original prefix");
    }
    let mut bytes = Vec::new();
    case.before_intent.write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
    fs::write(root.0.join("source-before-intent.bin"), bytes).unwrap();
    case.case.loaded.recovery_anchor = Some(case.before_intent.history_anchor().clone());
    case.profile = profile(&root, &case.before_intent, "source-before-intent.bin", Some("source-reset.bin"));
    select(&mut case.case, &case.profile);
    let error = reset::command(&arguments(&root, &case), None,
        || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("Binding"), "{error}");
    assert_eq!(disk(&case.case), canonical);
    assert_eq!(fs::read(&staging).unwrap(), b"retain rejected original prefix");
    assert!(!root.0.join("source-reset.bin").exists());
}

#[test]
fn topology_reset_deadline_at_ready_cannot_append_completion_or_discard_custody() {
    let root = Root::new(); let case = prepared(&root, false);
    let canonical = disk(&case.case);
    let staging = case.case.config.store.join("delivery.pending");
    fs::write(&staging, b"Ready is still vetoable").unwrap();
    let total = usize::try_from(case.case.before.revision).unwrap();
    for expire_at in [0, total / 2, total + 1] {
        let limit = deadline(); let mut calls = 0;
        let mut control = Control::new(&case.case.config, 900, None).unwrap();
        let error = reset::run(&case.case.config, &case.case.loaded, &case.case.request,
            &mut control, &limit, &mut || {
                let cut = calls; calls += 1;
                if cut == expire_at { limit.logical } else { ElapsedTick(1000) }
            }, &mut Vec::new()).unwrap_err();
        assert!(error.contains("one-shot workflow deadline reached"), "{error}");
        assert_eq!(calls, expire_at + 1);
        assert_eq!(disk(&case.case), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"Ready is still vetoable");
        assert!(!case.case.loaded.anchor_output.as_ref().unwrap().exists());
        assert!(!root.0.join("source-reset.bin").exists());
    }
}

#[test]
fn checked_stop_arriving_at_topology_reset_ready_retains_both_original_custodies() {
    for completed_first in [false, true] { stop_at_ready(completed_first); }
}

fn stop_at_ready(completed_first: bool) {
    use crate::workflow::control::socket_path;
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket, control::StopClientProgress,
    };
    use std::{os::unix::net::UnixStream, thread};
    let root = Root::new(); let case = prepared(&root, false);
    if completed_first {
        // Simulate a completed reset whose successor exports were both lost.
        // The SAME old pair must retain custody if the next checked Stop wins.
        let (host, _roles) = native_ready(&case).finish_pending_stream_reset().unwrap();
        assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        drop(host);
    }
    let before = snapshot(&case.case);
    remove_live_inputs(&root);
    let peers = fixture::peers(&root, &case.case.config);
    let operation = case.case.request.intent.control().operation;
    let mut control = Control::new(&case.case.config, operation, Some(&peers)).unwrap();
    let path = socket_path(&peers, operation);
    let expected = peers.expected;
    let total = usize::try_from(before.revision).unwrap();
    let mut calls = 0; let mut client = None; let mut output = Vec::new();
    let error = reset::run(&case.case.config, &case.case.loaded, &case.case.request,
        &mut control, &deadline(), &mut || {
            let cut = calls; calls += 1;
            if cut == total + 1 {
                // Establish a real checked peer at the final Ready checkpoint.
                // No earlier event boundary had a pending transport to latch.
                let stream = UnixStream::connect(&path).unwrap();
                client = Some(thread::spawn(move || {
                    let credentials = PeerCredentials::observe(&stream).unwrap();
                    let verified = VerifiedReviewerSocket::verify(stream, PeerPolicy::new(
                        credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap()).unwrap();
                    let mut client = verified.into_stop_client(expected, operation).unwrap();
                    let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(15), "topology reset Stop client stalled");
                        match client.step().unwrap() {
                            StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                            StopClientProgress::Complete => {
                                assert!(!client.outcome_unknown());
                                return client.receipt().expect("native admission Stop receipt");
                            }
                            _ => {}
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                }));
            }
            ElapsedTick(1000)
        }, &mut output).unwrap_err();
    let receipt = client.expect("Ready checkpoint reached").join().unwrap();
    assert!(error.contains("native admission Stop acknowledged"), "{error}");
    assert!(output.is_empty(), "terminal Stop cannot emit a restoration report");
    let application = control.recovery_application().unwrap();
    assert_eq!(application.receipt, receipt); assert!(receipt.acknowledged());
    let stopped = application.result.as_ref().unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(stopped.snapshot().revision, before.revision + 2);
    assert_eq!(stopped.snapshot().executions, 0);
    assert_eq!(stopped.snapshot().control.ledger.charged, case.case.before.control.ledger.charged);
    let retained = checkpoint(&root.0.join("source-reset.bin"));
    unchanged_source(&case.saved, &retained);
    assert_eq!(retained.history_anchor(), stopped.history_anchor());
    assert_eq!(custody::load(case.case.loaded.anchor_output.as_ref().unwrap()).unwrap(),
        *stopped.history_anchor());
    let canonical = disk(&case.case);
    let record = FileOversight::read_learned_reset_result(&case.case.config.store,
        &case.case.config.profile, &case.case.loaded.generation, operation).unwrap();
    match (completed_first, record.record()) {
        (false, FileLearnedResetRecord::Pending(pending)) => {
            assert!(pending.interrupted); assert_eq!(pending.intent, case.case.request.intent);
        }
        (true, FileLearnedResetRecord::Completed { result: Ok(receipt), .. }) => {
            assert!(receipt.control.restored, "Stop retains the already acknowledged reset result");
        }
        _ => panic!("Stop cannot create or relabel numerical reset completion"),
    }
    assert_eq!(disk(&case.case), canonical);
}
