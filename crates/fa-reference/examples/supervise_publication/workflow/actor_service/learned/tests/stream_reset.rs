//! Real pending numerical reset through the complete operator grammar. Fixtures
//! use a genuine native Continue review as the later incident frontier, without
//! changing the effective policy or confirming any new audience-visible output.
use super::super::{anchors as custody, stream_reset as reset};
use super::*;
use fa_reference::action::consequence::{
    activation::{
        monitor::learned::{LearnedMonitorBudget, model::LearnedAuditPreparationBudget},
        probe::learned::CheckedKvBudget,
        tensor::kv::{
            decoder::monitoring::{LearnedDecoderAllowance, restart::KvRestartBudget},
            model::{MAX_MODEL_KV_VALUES, learned::CompressionBudget},
        },
    },
    delivery::persistent::{
        FileDeliverySnapshot,
        observed::{
            containment::FileResetRequest,
            decoder::learned::checkpoint::{FileLearnedResetIntent, FileLearnedResetRecord},
            guarded::anchored::MAX_HISTORY_ANCHOR_BYTES,
        },
    },
    gate::{
        ReviewBinding,
        containment::{ActorState, RestartGrade},
    },
    oversight::{learned_host::HostedLearnedInspection, learned_source::LearnedAvailability},
};
use fa_reference::action::{MAX_ATTEMPTS, ResolvedTarget};
use fa_reference::strict_json::{self, Json, Limits};
use std::{
    cell::Cell,
    io,
    path::{Path, PathBuf},
};

struct Case {
    config: Config,
    recipe: PathBuf,
    loaded: Loaded,
    request: reset::Request,
    before: FileDeliverySnapshot,
    numerical: HostedLearnedInspection,
}

fn allowance() -> KvRestartBudget {
    KvRestartBudget {
        cache_values: MAX_MODEL_KV_VALUES,
        audit: LearnedDecoderAllowance {
            preparation: LearnedAuditPreparationBudget {
                compression: CompressionBudget {
                    source_values: 1000,
                    encoded_bytes: 65_536,
                    work_units: 1_000_000,
                },
                source_check: CheckedKvBudget {
                    source_values: 1000,
                    encoded_bytes: 65_536,
                    reconstruction_products: 1_000_000,
                },
            },
            monitoring: LearnedMonitorBudget {
                encoded_bytes: 65_536,
                probe_coordinates: 10_000,
                reconstruction_products: 1_000_000,
                materialized_values: 256,
                refinements: 64,
            },
        },
    }
}

fn prepared(root: &Root, audit_failure: bool, fenced: bool) -> Case {
    let mut config = configured(root);
    let original = &config.profile.delivery.actor;
    let mut declared = original.profile();
    declared.grade = RestartGrade::FunctionalRestart;
    config.profile.delivery.actor = ActorState::new(
        declared,
        original.tokens().to_vec(),
        original.cache().to_vec(),
        original.sampler().to_vec(),
        original.next_position(),
    )
    .unwrap();
    let path = super::continuation::write(root);
    evidence(root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    let (mut host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000))
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000))
        .unwrap();
    let initial = host.learned_generation_inspection().unwrap().numerical;
    host.advance_learned_generation(host.revision(), initial.actor_revision, initial.position)
        .unwrap()
        .unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 1);
    let saved = host
        .capture_learned_stream_checkpoint(
            host.revision(),
            1,
            numerical.actor_revision,
            host.inspect().control.ledger.epoch,
        )
        .unwrap();

    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    let mut driver = FileSupervisedDriver::new(supervisor);
    let deadline = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(
        generate(
            &mut driver,
            &reviewer,
            &mut config,
            &mut control,
            &deadline,
            &mut || ElapsedTick(1000)
        )
        .unwrap()
    );
    submit(
        &mut driver,
        &mut wire,
        &mut config,
        &loaded,
        &deadline,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let (mut driver, history) =
        super::continuation::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
    let (request, before, numerical) = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let first = host.inspect().control.sequence;
        assert!(saved.info().control_sequence < first);
        assert_eq!(
            host.current_policy().unwrap(),
            &config.profile.delivery.policy
        );
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
        let current = host.inspect();
        let control = FileResetRequest {
            operation: 900,
            expected_control_sequence: current.control.sequence,
            expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
            expected_authority_epoch: current.control.ledger.epoch,
            binding: ReviewBinding {
                round: 900,
                reducer_generation: 1,
                evidence_root: [8; 32],
            },
            retained_targets: vec![current.target],
        };
        let mut budget = allowance();
        if audit_failure {
            budget.audit.monitoring.probe_coordinates = 0;
        }
        let intent = FileLearnedResetIntent::for_recovery(saved.id(), control, budget).unwrap();
        host.begin_learned_stream_reset(
            host.revision(),
            &saved,
            intent.control().clone(),
            intent.budget(),
            first,
        )
        .unwrap();
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        if fenced {
            host.fence(host.revision()).unwrap();
            assert!(host.pending_learned_reset().unwrap().unwrap().interrupted);
        }
        loaded.recovery_anchor = Some(host.history_anchor().unwrap());
        loaded.anchor_output = Some(root.0.join("next.anchor"));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        (
            reset::Request {
                intent,
                first_incident_sequence: first,
            },
            host.inspect(),
            numerical,
        )
    };
    drop(history);
    drop(driver);
    drop(wire);
    drop(reviewer);
    Case {
        config,
        recipe: path,
        loaded,
        request,
        before,
        numerical,
    }
}

fn config_file(root: &Root, config: &Config) -> PathBuf {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    ))
    .replace(
        "/operator/publication-store",
        config.store.to_str().unwrap(),
    )
    .replace(
        "/operator/evidence.json",
        root.0.join("evidence.json").to_str().unwrap(),
    )
    .replace(
        "\"initial_payload_hex\": \"696e697469616c\"",
        "\"initial_payload_hex\": \"\"",
    )
    .replace(
        "\"grade\": \"audit_only\"",
        "\"grade\": \"functional_restart\"",
    )
    .replace("\"events\": 256", "\"events\": 1024")
    .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 60000")
    .replace("\"poll_ms\": 10", "\"poll_ms\": 1")
    .replace("\"cleanup_ms\": 2000", "\"cleanup_ms\": 1000");
    let path = root.0.join("reset-config.json");
    fs::write(&path, text).unwrap();
    let actual = Config::read_learned_stream_reset(&path).unwrap();
    crate::config::assert_oversight_profiles_eq(&actual.profile, &config.profile);
    assert_eq!(actual.source_policy, config.source_policy);
    path
}

fn request_bytes(request: &reset::Request) -> Vec<u8> {
    let control = request.intent.control();
    let targets = control.retained_targets.iter().map(|target| format!(
        r#"{{"adapter":"{}","object":"{}","contract_version":"{}","expected_version":"{}","generation":"{}"}}"#,
        target.adapter, target.object, target.contract_version, target.expected_version, target.generation,
    )).collect::<Vec<_>>().join(",");
    let root = control
        .binding
        .evidence_root
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let budget = request.intent.budget();
    let p = budget.audit.preparation;
    let m = budget.audit.monitoring;
    format!(
        concat!(
            "{{\"schema\":\"fa.learned-stream-reset-recovery/1\",\"checkpoint\":\"{}\",",
            "\"first_incident_sequence\":\"{}\",\"control\":{{\"operation\":\"{}\",",
            "\"expected_control_sequence\":\"{}\",\"expected_actor_revision\":\"{}\",",
            "\"expected_authority_epoch\":\"{}\",\"binding\":{{\"round\":\"{}\",",
            "\"reducer_generation\":\"{}\",\"evidence_root_hex\":\"{}\"}},\"retained_targets\":[{}]}},",
            "\"restart_budget\":{{\"cache_values\":\"{}\",\"audit\":{{\"preparation\":{{",
            "\"compression\":{{\"source_values\":\"{}\",\"encoded_bytes\":\"{}\",\"work_units\":\"{}\"}},",
            "\"source_check\":{{\"source_values\":\"{}\",\"encoded_bytes\":\"{}\",\"reconstruction_products\":\"{}\"}}}},",
            "\"monitoring\":{{\"encoded_bytes\":\"{}\",\"probe_coordinates\":\"{}\",",
            "\"reconstruction_products\":\"{}\",\"materialized_values\":\"{}\",\"refinements\":\"{}\"}}}}}}}}"
        ),
        request.intent.checkpoint(), request.first_incident_sequence, control.operation,
        control.expected_control_sequence, control.expected_actor_revision, control.expected_authority_epoch,
        control.binding.round, control.binding.reducer_generation, root, targets,
        budget.cache_values, p.compression.source_values, p.compression.encoded_bytes,
        p.compression.work_units, p.source_check.source_values, p.source_check.encoded_bytes,
        p.source_check.reconstruction_products, m.encoded_bytes, m.probe_coordinates,
        m.reconstruction_products, m.materialized_values, m.refinements,
    ).into_bytes()
}

fn args(root: &Root, case: &Case) -> Vec<String> {
    let request = root.0.join("independent-reset.json");
    fs::write(&request, request_bytes(&case.request)).unwrap();
    let anchor = root.0.join("independent-input.anchor");
    let mut bytes = Vec::new();
    case.loaded
        .recovery_anchor
        .as_ref()
        .unwrap()
        .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
        .unwrap();
    fs::write(&anchor, bytes).unwrap();
    vec![
        "create-learned-generated".into(),
        config_file(root, &case.config).display().to_string(),
        case.recipe.display().to_string(),
        "--complete-stream-reset".into(),
        request.display().to_string(),
        "--history-anchor".into(),
        anchor.display().to_string(),
        "--retain-history-anchor".into(),
        case.loaded
            .anchor_output
            .as_ref()
            .unwrap()
            .display()
            .to_string(),
    ]
}

fn disk(case: &Case) -> Vec<u8> {
    fs::read(case.config.store.join("delivery.bin")).unwrap()
}
fn snapshot(case: &Case) -> FileDeliverySnapshot {
    FileOversight::read_publication_with_learned_generation(
        &case.config.store,
        &case.config.profile,
        &case.loaded.generation,
    )
    .unwrap()
}
fn report(bytes: &[u8]) -> Json {
    let parsed = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(
        parsed.get("schema").and_then(Json::as_str),
        Some("fa.learned-stream-reset-result/1")
    );
    assert_eq!(
        parsed.get("acknowledged").and_then(Json::as_bool),
        Some(true)
    );
    assert_eq!(parsed.get("authority").and_then(Json::as_str), Some("none"));
    parsed
}

#[test]
fn command_completes_the_actual_pending_stream_before_fencing_and_retains_its_cut() {
    let root = Root::new();
    let case = prepared(&root, false, false);
    let arguments = args(&root, &case);
    let input = fs::read(&arguments[6]).unwrap();
    // Historical completion does not load current source/helper evidence.
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    fs::remove_file(root.0.join("native-roster.json")).unwrap();
    let mut out = Vec::new();
    reset::command(&arguments, None, || ElapsedTick(1000), &mut out).unwrap();
    let value = report(&out);
    assert_eq!(
        value.get("outcome").and_then(Json::as_str),
        Some("restored")
    );
    assert_eq!(value.get("restored").and_then(Json::as_bool), Some(true));
    assert_eq!(value.get("position").and_then(Json::as_str), Some("1"));
    assert_eq!(
        value.get("restart_attempts").and_then(Json::as_str),
        Some("1")
    );
    assert_eq!(
        value.get("history_anchor_retained").and_then(Json::as_bool),
        Some(true)
    );
    assert_eq!(snapshot(&case).revision, case.before.revision + 2);
    assert_eq!(
        snapshot(&case).control.ledger.charged,
        case.before.control.ledger.charged
    );
    assert_eq!(fs::read(&arguments[6]).unwrap(), input);
    assert_eq!(
        custody::load(Path::new(&arguments[8])).unwrap().revision(),
        case.before.revision + 2
    );

    // Exactly completed retry adds only its ordinary Fence. It cannot repeat
    // the audit, spend the restart allowance again, or count another incident.
    let mut retry = arguments.clone();
    retry[8] = root.0.join("retry.anchor").display().to_string();
    let mut retried = Vec::new();
    reset::command(&retry, None, || ElapsedTick(1000), &mut retried).unwrap();
    let retried = report(&retried);
    for field in [
        "restart_attempts",
        "reserved_cache_values",
        "incident_count",
        "values_restored",
        "charged",
    ] {
        assert_eq!(retried.get(field), value.get(field), "{field}");
    }
    assert_eq!(snapshot(&case).revision, case.before.revision + 3);
    // The real executable selector uses the same narrow Config admission and
    // exact native retry with the production clock, without a new reset audit.
    let mut dispatched = arguments.clone();
    dispatched[8] = root.0.join("root-dispatch.anchor").display().to_string();
    crate::command_with_output(dispatched.clone(), &mut Vec::new()).unwrap();
    assert_eq!(snapshot(&case).revision, case.before.revision + 4);
    assert_eq!(
        custody::load(Path::new(&dispatched[8])).unwrap().revision(),
        case.before.revision + 4
    );
    let (host, _, _, _) = recovery::open_selected(&case.config, &case.loaded).unwrap();
    let actual = host.learned_generation_inspection().unwrap();
    assert!(actual.paused);
    assert!(!host.clock_ready());
    assert_eq!(actual.numerical.position, 1);
    assert_eq!(actual.numerical.sampled_draws, 0);
    assert_eq!(
        actual.numerical.cumulative_work,
        case.numerical.cumulative_work
    );
    assert_eq!(actual.numerical.availability, LearnedAvailability::Empty);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
}

#[test]
fn exact_expectation_is_required_and_refusal_preserves_canonical_and_staged_bytes() {
    let root = Root::new();
    let case = prepared(&root, false, false);
    let arguments = args(&root, &case);
    let before = disk(&case);
    let staging = case.config.store.join("delivery.pending");
    fs::write(&staging, b"do not clean before exact reset admission").unwrap();
    for change in 0..6 {
        let mut request = case.request.clone();
        let mut control = request.intent.control().clone();
        let mut budget = request.intent.budget();
        let mut checkpoint = request.intent.checkpoint();
        match change {
            0 => checkpoint += 1,
            1 => request.first_incident_sequence += 1,
            2 => control.expected_authority_epoch += 1,
            3 => control.binding.evidence_root[0] ^= 1,
            4 => budget.cache_values -= 1,
            _ => control.operation += 1,
        }
        request.intent = FileLearnedResetIntent::for_recovery(checkpoint, control, budget).unwrap();
        fs::write(&arguments[4], request_bytes(&request)).unwrap();
        let mut out = Vec::new();
        assert!(reset::command(&arguments, None, || ElapsedTick(1000), &mut out).is_err());
        assert!(out.is_empty());
        assert_eq!(disk(&case), before);
        assert_eq!(
            fs::read(&staging).unwrap(),
            b"do not clean before exact reset admission"
        );
        assert!(!Path::new(&arguments[8]).exists());
    }
    fs::write(&arguments[4], request_bytes(&case.request)).unwrap();
    reset::command(&arguments, None, || ElapsedTick(1000), &mut Vec::new()).unwrap();
    assert!(!staging.exists());
    assert_eq!(snapshot(&case).revision, case.before.revision + 2);
}

#[test]
fn an_original_fence_interrupts_pending_reset_and_cannot_be_rebased() {
    let root = Root::new();
    let case = prepared(&root, false, true);
    let arguments = args(&root, &case);
    let before = disk(&case);
    assert!(reset::command(&arguments, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
    assert_eq!(disk(&case), before);
    assert!(!Path::new(&arguments[8]).exists());
    let historical = FileOversight::read_learned_reset_result(
        &case.config.store,
        &case.config.profile,
        &case.loaded.generation,
        900,
    )
    .unwrap();
    assert!(
        matches!(historical.record(), FileLearnedResetRecord::Pending(pending) if pending.interrupted)
    );
}

#[test]
fn acknowledged_inner_audit_failure_is_retained_reported_and_never_refunded_on_retry() {
    let root = Root::new();
    let case = prepared(&root, true, false);
    let arguments = args(&root, &case);
    let mut out = Vec::new();
    let error = reset::command(&arguments, None, || ElapsedTick(1000), &mut out).unwrap_err();
    assert!(error.contains("acknowledged refusal"), "{error}");
    let first = report(&out);
    assert_eq!(first.get("outcome").and_then(Json::as_str), Some("refused"));
    assert_eq!(first.get("restored").and_then(Json::as_bool), Some(false));
    assert_eq!(
        first.get("restart_attempts").and_then(Json::as_str),
        Some("1")
    );
    assert_eq!(
        first.get("history_anchor_retained").and_then(Json::as_bool),
        Some(true)
    );
    let before = snapshot(&case);
    assert_eq!(before.revision, case.before.revision + 2);
    let original = FileOversight::read_learned_reset_result(
        &case.config.store,
        &case.config.profile,
        &case.loaded.generation,
        900,
    )
    .unwrap();
    assert!(matches!(
        original.record(),
        FileLearnedResetRecord::Completed { result: Err(_), .. }
    ));
    let mut retry = arguments.clone();
    retry[8] = root.0.join("refused-retry.anchor").display().to_string();
    let mut out = Vec::new();
    assert!(reset::command(&retry, None, || ElapsedTick(1000), &mut out).is_err());
    let second = report(&out);
    for field in [
        "outcome",
        "restored",
        "restart_attempts",
        "reserved_cache_values",
        "incident_count",
        "charged",
    ] {
        assert_eq!(first.get(field), second.get(field), "{field}");
    }
    assert_eq!(snapshot(&case).revision, before.revision + 1);
    let (host, _, _, _) = recovery::open_selected(&case.config, &case.loaded).unwrap();
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert!(host.learned_recovery_usage().unwrap().reserved_cache_values > 0);
    assert_eq!(
        host.learned_generation_inspection()
            .unwrap()
            .numerical
            .cumulative_work,
        case.numerical.cumulative_work
    );
}

#[test]
fn preopen_middle_and_ready_deadlines_do_not_finish_or_clean_the_same_pending_reset() {
    let root = Root::new();
    let case = prepared(&root, false, false);
    let before = disk(&case);
    let staging = case.config.store.join("delivery.pending");
    fs::write(&staging, b"preserve pending cut").unwrap();
    for cutoff in [
        1,
        2,
        case.before.revision as usize / 2,
        case.before.revision as usize + 2,
    ] {
        let mut calls = 0;
        let deadline = deadline();
        let mut time = || {
            calls += 1;
            if calls >= cutoff {
                deadline.logical
            } else {
                ElapsedTick(1000)
            }
        };
        let mut control = Control::new(&case.config, 900, None).unwrap();
        let mut out = Vec::new();
        assert!(
            reset::run(
                &case.config,
                &case.loaded,
                &case.request,
                &mut control,
                &deadline,
                &mut time,
                &mut out
            )
            .is_err()
        );
        assert!(out.is_empty());
        assert_eq!(disk(&case), before);
        assert_eq!(fs::read(&staging).unwrap(), b"preserve pending cut");
        assert!(!case.loaded.anchor_output.as_ref().unwrap().exists());
    }
}

#[test]
fn a_late_report_keeps_acknowledgment_and_retention_but_cannot_extend_the_original_budget() {
    struct Delayed<'a> {
        bytes: Vec<u8>,
        expired: &'a Cell<bool>,
    }
    impl Write for Delayed<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            self.expired.set(true);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let root = Root::new();
    let case = prepared(&root, false, false);
    let expired = Cell::new(false);
    let mut out = Delayed {
        bytes: Vec::new(),
        expired: &expired,
    };
    let deadline = deadline();
    let mut control = Control::new(&case.config, 900, None).unwrap();
    let error = reset::run(
        &case.config,
        &case.loaded,
        &case.request,
        &mut control,
        &deadline,
        &mut || {
            if expired.get() {
                deadline.logical
            } else {
                ElapsedTick(1000)
            }
        },
        &mut out,
    )
    .unwrap_err();
    assert!(
        error.contains("deadline reached during acknowledged output"),
        "{error}"
    );
    assert_eq!(
        report(&out.bytes).get("restored").and_then(Json::as_bool),
        Some(true)
    );
    assert!(case.loaded.anchor_output.as_ref().unwrap().exists());
    assert_eq!(snapshot(&case).revision, case.before.revision + 2);
}

#[test]
fn predictive_and_topology_modes_cannot_downgrade_an_original_stream_reset() {
    let root = Root::new();
    let case = prepared(&root, false, false);
    let arguments = args(&root, &case);
    let before = disk(&case);
    let original = fs::read(&case.recipe).unwrap();
    for owned in [false, true] {
        fs::write(&case.recipe, &original).unwrap();
        super::forecast::install(&root, &case.recipe, owned, 60_000);
        let error =
            reset::command(&arguments, None, || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
        assert!(error.contains("predictive histories"), "{error}");
        assert_eq!(disk(&case), before);
    }
    fs::write(&case.recipe, original).unwrap();
    let mut topology = arguments.clone();
    topology.extend(["--topology-profile".into(), "/missing/topology.json".into()]);
    assert!(reset::command(&topology, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
    assert_eq!(disk(&case), before);
    reset::command(&arguments, None, || ElapsedTick(1000), &mut Vec::new()).unwrap();
}

#[test]
fn expectation_parser_preserves_large_integer_identity_and_closes_every_nested_object() {
    let control = FileResetRequest {
        operation: u64::MAX,
        expected_control_sequence: u64::MAX - 1,
        expected_actor_revision: u64::MAX - 2,
        expected_authority_epoch: u64::MAX - 3,
        binding: ReviewBinding {
            round: u64::MAX,
            reducer_generation: 1,
            evidence_root: [8; 32],
        },
        retained_targets: (1..=MAX_ATTEMPTS)
            .map(|index| ResolvedTarget {
                adapter: 1,
                object: index as u64,
                contract_version: 1,
                expected_version: 1,
                generation: 1,
            })
            .collect(),
    };
    let request = reset::Request {
        intent: FileLearnedResetIntent::for_recovery(u64::MAX, control, allowance()).unwrap(),
        first_incident_sequence: u64::MAX - 4,
    };
    let bytes = request_bytes(&request);
    assert_eq!(reset::decode(&bytes).unwrap(), request);
    let valid = String::from_utf8(bytes).unwrap();
    for invalid in [
        valid.replacen(
            "\"operation\":\"18446744073709551615\"",
            "\"operation\":18446744073709551615",
            1,
        ),
        valid.replacen(
            "\"operation\":\"18446744073709551615\"",
            "\"operation\":\"018446744073709551615\"",
            1,
        ),
        valid.replacen("\"checkpoint\":", "\"unexpected\":true,\"checkpoint\":", 1),
        valid.replacen("\"binding\":{", "\"binding\":{\"unexpected\":true,", 1),
        valid.replacen(
            "\"cache_values\":",
            "\"cache_values\":\"0\",\"cache_values\":",
            1,
        ),
        valid.replacen(
            "\"first_incident_sequence\":\"18446744073709551611\"",
            "\"first_incident_sequence\":\"0\"",
            1,
        ),
    ] {
        assert!(reset::decode(invalid.as_bytes()).is_err());
    }
}

#[test]
fn invalid_combinations_fail_before_the_nonexistent_config_or_any_owner() {
    let valid: Vec<String> = [
        "create-learned-generated",
        "/missing/config",
        "/missing/recipe",
        "--complete-stream-reset",
        "/held/request.json",
        "--history-anchor",
        "/held/input.anchor",
        "--retain-history-anchor",
        "/held/new.anchor",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for extra in [
        vec!["--resume"],
        vec!["--continue-generation"],
        vec!["--history-anchor", "/held/second.anchor"],
        vec!["--stop-profile"],
        vec!["--topology-profile"],
        vec!["--topology-profile", "/held/first.json", "--topology-profile", "/held/second.json"],
        vec!["--actor-profile", "/held/actor.json"],
    ] {
        let mut args = valid.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        let error = reset::command(
            &args,
            None,
            || panic!("invalid grammar reached clock"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(!error.contains("No such file"), "{error}");
    }
    assert!(
        reset::command(
            &valid,
            Some(Path::new("/held/capsule")),
            || panic!("credibility refusal reached clock"),
            &mut Vec::new()
        )
        .is_err()
    );
}

#[test]
fn functional_restart_declaration_is_explicit_and_confined_to_reset_recovery() {
    let root = Root::new();
    let path = root.0.join("declared-profile.json");
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    ));
    fs::write(&path, original).unwrap();
    assert_eq!(
        Config::read(&path)
            .unwrap()
            .profile
            .delivery
            .actor
            .profile()
            .grade,
        RestartGrade::AuditOnly
    );
    assert_eq!(
        Config::read_learned_stream_reset(&path)
            .unwrap()
            .profile
            .delivery
            .actor
            .profile()
            .grade,
        RestartGrade::AuditOnly
    );
    let functional = original.replace(
        "\"grade\": \"audit_only\"",
        "\"grade\": \"functional_restart\"",
    );
    fs::write(&path, &functional).unwrap();
    assert!(Config::read(&path).is_err());
    assert_eq!(
        Config::read_learned_stream_reset(&path)
            .unwrap()
            .profile
            .delivery
            .actor
            .profile()
            .grade,
        RestartGrade::FunctionalRestart
    );
    for unsupported in ["bit_exact", "automatic", "functional"] {
        fs::write(
            &path,
            original.replace(
                "\"grade\": \"audit_only\"",
                &format!("\"grade\": \"{unsupported}\""),
            ),
        )
        .unwrap();
        assert!(Config::read(&path).is_err());
        assert!(Config::read_learned_stream_reset(&path).is_err());
    }
    assert!(!root.0.join("store").exists());
}

#[test]
fn an_authenticated_stop_interrupts_the_pending_reset_before_any_completion() {
    use crate::workflow::control::socket_path;
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket, control::StopClientProgress,
    };
    use std::{os::unix::net::UnixStream, thread};

    let root = Root::new();
    let case = prepared(&root, false, false);
    let original_anchor = case.loaded.recovery_anchor.as_ref().unwrap().clone();
    let peers = fixture::peers(&root, &case.config);
    let operation = case.request.intent.control().operation;
    let mut control = Control::new(&case.config, operation, Some(&peers)).unwrap();
    let stream = UnixStream::connect(socket_path(&peers, operation)).unwrap();
    let expected = peers.expected;
    let client = thread::spawn(move || {
        let credentials = PeerCredentials::observe(&stream).unwrap();
        let verified = VerifiedReviewerSocket::verify(
            stream,
            PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap(),
        ).unwrap();
        let mut client = verified.into_stop_client(expected, operation).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(15), "reset recovery Stop client stalled");
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => {
                    assert!(!client.outcome_unknown());
                    return client.receipt().expect("original Stop receipt");
                }
                _ => {}
            }
            thread::sleep(Duration::from_millis(1));
        }
    });
    let mut out = Vec::new();
    let error = reset::run(
        &case.config, &case.loaded, &case.request, &mut control, &deadline(),
        &mut || ElapsedTick(1000), &mut out,
    ).unwrap_err();
    let receipt = client.join().unwrap();
    assert!(error.contains("native admission Stop acknowledged"), "{error}");
    assert!(out.is_empty(), "a stopped reset cannot emit a restoration report");
    let application = control.recovery_application().unwrap();
    assert!(application.receipt.acknowledged());
    assert_eq!(application.receipt, receipt);
    let stopped = application.result.as_ref().unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(stopped.snapshot().revision, case.before.revision + 2);
    assert!(stopped.snapshot().stop.is_some());
    assert_eq!(stopped.snapshot().control.ledger.elapsed, case.before.control.ledger.elapsed);
    assert_eq!(stopped.snapshot().control.ledger.charged, case.before.control.ledger.charged);
    assert_eq!(stopped.snapshot().executions, case.before.executions);
    assert_eq!(snapshot(&case), *stopped.snapshot());
    assert_eq!(custody::load(case.loaded.anchor_output.as_ref().unwrap()).unwrap(),
        *stopped.history_anchor());
    assert_eq!(case.loaded.recovery_anchor.as_ref().unwrap(), &original_anchor);

    // The exact original intent remains pending and interrupted. Only native
    // Stop/Fence were appended; no reset audit/completion or new numerical
    // timeline was acknowledged. The inspection itself adds no further Fence.
    let canonical = disk(&case);
    let history = FileOversight::read_learned_reset_result(
        &case.config.store, &case.config.profile, &case.loaded.generation, operation,
    ).unwrap();
    match history.record() {
        FileLearnedResetRecord::Pending(pending) => {
            assert!(pending.interrupted);
            assert_eq!(pending.intent, case.request.intent);
        }
        _ => panic!("Stop cannot complete the interrupted reset"),
    }
    assert_eq!(history.journal_revision(), case.before.revision + 2);
    assert_eq!(disk(&case), canonical);
}

#[path = "stream_reset/topology.rs"]
mod topology;
