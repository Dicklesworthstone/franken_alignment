//! The same original numerical prompt can be contained without installing a
//! predictor. Every live continuation still earns current topology, native
//! review and the separate human key. Finite fixtures do not establish model
//! calibration or production restart secrecy.
use super::*;
use super::super::{functional_creation, peer_intake};
use crate::workflow::actor_service::learned as service;
use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::TOPOLOGY_REFRESH_READ_BYTES;

fn unpredicted(case: &Case) {
    let loaded = recipe::load(&case.recipe, &case.config, false).unwrap();
    assert!(loaded.generation.required_pre_output_forecast().is_none());
    assert!(loaded.generation.required_owned_pre_output_forecast().is_none());
    assert!(!loaded.generation.requires_owned_forecast_stream_restart_audit());
    assert!(!case.root.join("forecast.json").exists());
}

fn args(case: &Case, flag: &str, request: &Path, input: &Pair, output: &Pair,
    label: &str, mediated: bool) -> Vec<String>
{
    if mediated {
        return case.control_args(flag, request, input, output, label);
    }
    vec!["create-learned-generated".into(), case.config_path.display().to_string(),
        case.recipe.display().to_string(), flag.into(), request.display().to_string(),
        "--history-anchor".into(), input.history.display().to_string(),
        "--retain-history-anchor".into(), output.history.display().to_string()]
}

fn prepare(case: &Case, mediated: bool) -> Json {
    unpredicted(case);
    if mediated {
        return case.prepare();
    }
    let mut command = case.preparation_args();
    command.truncate(7);
    let mut output = Vec::new();
    preparation::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    let value = report(&output);
    assert_eq!(text(&value, "outcome"), "prepared");
    assert_eq!(number(&value, "sampling_attempts"), 0);
    assert_eq!(number(&value, "executions"), 0);
    assert!(!case.pair("prepared").source.exists());
    value
}

fn capture_retry(case: &Case, prepared: &Json, mediated: bool) {
    let cut = info(prepared);
    let request = case.root.join("ordinary-capture.json");
    fs::write(&request, format!(r#"{{"schema":"fa.learned-stream-checkpoint/1",
        "expected_journal_revision":"{}","checkpoint":"{}","expected_actor_revision":"{}",
        "expected_authority_epoch":"{}","expected_control_sequence":"{}","expected_stream":"{}",
        "expected_position":"{}","expected_sampled_draws":"{}"}}"#,
        number(prepared, "revision") - 1, cut.checkpoint, cut.actor_revision,
        cut.authority_epoch, cut.control_sequence, cut.stream, cut.position,
        cut.sampled_draws)).unwrap();
    let before = case.disk();
    let output = case.pair("capture-retry");
    let result = control_command(&args(case, "--capture-stream-checkpoint", &request,
        &case.pair("prepared"), &output, "capture-retry", mediated));
    assert_eq!(text(&result, "outcome"), "already_captured");
    assert_eq!(number(&result, "appended_events"), 0);
    assert_eq!(number(&result, "checkpoints"), 1);
    assert_eq!(number(&result, "sampled_draws"), 0);
    assert_eq!(case.disk(), before);
    assert_eq!(custody::load(&output.history).unwrap(),
        custody::load(&case.pair("prepared").history).unwrap());
    if mediated {
        unchanged_source(&checkpoint(&case.pair("prepared").source), &output, true);
    }
}

fn contained(case: &Case, mediated: bool) -> (Pair, PathBuf, Json) {
    let prepared = prepare(case, mediated);
    let cut = info(&prepared);
    assert_eq!(cut.position, 2);
    assert_eq!(cut.sampled_draws, 0);
    capture_retry(case, &prepared, mediated);
    let original = mediated.then(|| checkpoint(&case.pair("prepared").source));
    let before = case.publication();
    let narrow = case.narrow(cut);
    let narrowed = case.pair("narrowed");
    let result = control_command(&args(case, "--narrow-stream-checkpoint", &narrow,
        &case.pair("prepared"), &narrowed, "narrow", mediated));
    assert_eq!(text(&result, "outcome"), "narrowed");
    assert_eq!(number(&result, "appended_events"), 1);
    assert_eq!(number(&result, "revision"), before.revision + 1);
    assert_eq!(number(&result, "actor_revision"), cut.actor_revision);
    assert_eq!(number(&result, "position"), cut.position);
    assert_eq!(number(&result, "sampled_draws"), 0);
    assert_eq!(number(&result, "restart_attempts"), 0);
    assert_eq!(number(&result, "authority_epoch"), before.control.ledger.epoch + 1);
    assert_eq!(number(&result, "charged"), before.control.ledger.charged);
    assert_eq!(number(&result, "reserved"), before.control.ledger.reserved);
    let first = number(&result, "first_incident_sequence");
    assert_eq!(first, before.control.sequence + 1);
    assert!(first > cut.control_sequence);
    if let Some(original) = &original {
        unchanged_source(original, &narrowed, true);
    }
    let reset = case.reset(cut, first);
    let pending = case.pair("pending");
    let begun = control_command(&args(case, "--begin-stream-reset", &reset,
        &narrowed, &pending, "begin", mediated));
    assert_eq!(text(&begun, "outcome"), "pending");
    assert_eq!(number(&begun, "appended_events"), 1);
    assert_eq!(number(&begun, "restart_attempts"), 0);
    assert_eq!(number(&begun, "sampled_draws"), 0);
    assert_eq!(number(&begun, "executions"), 0);
    if let Some(original) = &original {
        unchanged_source(original, &pending, true);
    }
    let complete = case.pair("complete");
    let result = reset_command(&args(case, "--complete-stream-reset", &reset,
        &pending, &complete, "complete", mediated));
    assert_eq!(text(&result, "outcome"), "restored");
    assert_eq!(number(&result, "restart_attempts"), 1);
    assert_eq!(number(&result, "incident_count"), 1);
    assert_eq!(number(&result, "position"), cut.position);
    assert_eq!(number(&result, "sampled_draws"), 0);
    assert!(number(&result, "reserved_cache_values") > 0);
    assert!(number(&result, "values_restored") > 0);
    assert_eq!(number(&result, "executions"), 0);
    assert_eq!(number(&result, "charged"), before.control.ledger.charged);
    assert_eq!(result.get("paused").and_then(Json::as_bool), Some(true));
    if let Some(original) = &original {
        unchanged_source(original, &complete, false);
    }
    assert_eq!(fs::read_dir(case.root.join("peers")).unwrap().count(), 0);
    assert_eq!(case.publication().executions, 0);
    unpredicted(case);
    (complete, reset, result)
}

fn continuation_args(case: &Case, peer_path: &Path, input: &Pair, output: &Pair,
    profile: Option<&Path>) -> Vec<String>
{
    let mut args = vec!["create-learned-generated".into(), case.config_path.display().to_string(),
        case.recipe.display().to_string(), peer_path.display().to_string(),
        "--continue-generation".into(), "--history-anchor".into(), input.history.display().to_string(),
        "--retain-history-anchor".into(), output.history.display().to_string()];
    if let Some(profile) = profile {
        args.extend(["--topology-profile".into(), profile.display().to_string()]);
    }
    args
}

fn numerical(case: &Case, pair: &Pair, mediated: bool, position: u64, draws: u64) {
    let mut loaded = recipe::load(&case.recipe, &case.config, false).unwrap();
    loaded.recovery_anchor = Some(custody::load(&pair.history).unwrap());
    if mediated {
        let unused = case.pair("inspection-unused");
        let profile = case.profile(pair, &unused, "inspect", None);
        let document = fs::read_to_string(&profile).unwrap().replace(
            &format!(r#","retain_checkpoint":"{}""#, unused.source.display()), "");
        fs::write(&profile, document).unwrap();
        service::topology::admit(&case.config, &mut loaded, Some(&profile),
            service::topology::Mode::Historical).unwrap();
    }
    let (host, _, forecast, _) = recovery::open_selected(&case.config, &loaded).unwrap();
    assert!(forecast.is_none());
    assert!(!host.action_consistency_required());
    assert!(!host.learned_action_consistency_required());
    assert!(!host.owned_learned_action_consistency_required());
    let actual = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(actual.position, position);
    assert_eq!(actual.sampled_draws, draws);
    assert_eq!(actual.cumulative_work.admitted_tokens, position);
    assert_eq!(actual.cumulative_work.sampling_attempts, draws);
    assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
}

#[test]
fn ordinary_plain_and_mediated_containment_restore_then_require_fresh_native_and_human_approval() {
    for (mediated, decision) in [
        (false, ReviewDecision::Approve),
        (true, ReviewDecision::Approve),
        (true, ReviewDecision::Reject),
    ] {
        let root = Root::new();
        let case = Case::ordinary(&root);
        // Maintenance needs the actor's original assets but no native helper
        // inference. The live service must load those helpers afresh below.
        let helper = root.0.join("helper-weights.safetensors");
        let weights = fs::read(&helper).unwrap();
        fs::remove_file(&helper).unwrap();
        let (complete, _, _) = contained(&case, mediated);
        fs::write(&helper, weights).unwrap();
        let prior = mediated.then(|| checkpoint(&complete.source));
        let input_history = fs::read(&complete.history).unwrap();
        let input_source = mediated.then(|| fs::read(&complete.source).unwrap());
        let published = case.pair("published");
        let profile = mediated.then(|| case.profile(&complete, &published, "continue", Some((2, false))));
        let (peer_path, peers) = functional_creation::peers_file(&root, &case.config);
        let command = continuation_args(&case, &peer_path, &complete, &published, profile.as_deref());
        let mut human = None;
        let result = service::command_with_clock(&command, None, || {
            if human.is_none() && peers.socket(1).exists() {
                human = Some(peer_intake::human(peers.clone(), 1, decision));
            }
            ElapsedTick(1001)
        });
        let packet = human.expect("reset never provides the separate human key").join().unwrap();
        assert_eq!(result.is_ok(), decision == ReviewDecision::Approve, "{result:?}");
        assert_eq!(packet.views().len(), 2);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        for view in packet.views().values() {
            assert!(!view.actual_input().submitted_bytes().is_empty());
        }
        let state = case.publication();
        assert_eq!(state.executions, u64::from(decision == ReviewDecision::Approve));
        assert_eq!(state.control.ledger.charged,
            if decision == ReviewDecision::Approve { packet.action().spec().units } else { 0 });
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.available + state.control.ledger.charged,
            case.config.profile.delivery.total);
        assert_eq!(custody::load(&published.history).unwrap().revision(), state.revision);
        if let Some(prior) = &prior {
            let source = checkpoint(&published.source);
            assert_eq!(source.history_anchor(), &custody::load(&published.history).unwrap());
            assert_eq!(source.work().checks,
                prior.work().checks + 1 + u64::from(decision == ReviewDecision::Approve));
            assert_eq!(source.work().reserved_read_bytes,
                source.work().checks * TOPOLOGY_REFRESH_READ_BYTES);
            assert_eq!(source.expected().source_limits(), prior.expected().source_limits());
            assert_eq!(source.expected().graph().spec().generation, 2);
            assert!(source.last_operation() > prior.last_operation());
            assert_eq!(fs::read(&complete.source).unwrap(), input_source.unwrap());
        }
        assert_eq!(fs::read(&complete.history).unwrap(), input_history);
        unpredicted(&case);
        numerical(&case, &published, mediated, 4, 2);
    }
}

#[test]
fn ordinary_narrowing_requires_the_original_prompt_target_predecessors_and_exact_paired_custody() {
    let root = Root::new();
    let case = Case::ordinary(&root);
    let prepared = prepare(&case, true);
    let cut = info(&prepared);
    let narrow = case.narrow(cut);
    let original = fs::read_to_string(&narrow).unwrap();
    let state = case.publication();
    let before = case.disk();
    let changed = |key: &str, from: u64, to: u64| {
        original.replacen(&format!(r#""{key}":"{from}""#),
            &format!(r#""{key}":"{to}""#), 1)
    };
    for (name, document) in [
        ("journal", changed("expected_journal_revision", state.revision, state.revision + 1)),
        ("control", changed("expected_control_sequence", state.control.sequence, state.control.sequence + 1)),
        ("actor", changed("expected_actor_revision", cut.actor_revision, cut.actor_revision + 1)),
        ("epoch", changed("expected_authority_epoch", state.control.ledger.epoch, state.control.ledger.epoch + 1)),
        ("capture", changed("checkpoint", cut.checkpoint, cut.checkpoint + 1)),
        ("position", changed("position", cut.position, cut.position - 1)),
        ("samples", changed("sampled_draws", 0, 1)),
        ("stream", changed("stream", cut.stream, cut.stream + 1)),
        ("audience", changed("object", state.target.object, state.target.object + 1)),
    ] {
        assert_ne!(document, original, "{name}");
        // These are valid independent requests for a different native cut,
        // not malformed JSON standing in for a predecessor/target check.
        control::decode_narrow(document.as_bytes()).unwrap();
        fs::write(&narrow, document).unwrap();
        let output = case.pair(name);
        let command = args(&case, "--narrow-stream-checkpoint", &narrow,
            &case.pair("prepared"), &output, name, true);
        let mut report = Vec::new();
        assert!(control::command(&command, None, || ElapsedTick(1000), &mut report).is_err(), "{name}");
        assert!(report.is_empty(), "{name}");
        assert_eq!(case.disk(), before, "{name}");
        assert!(!output.history.exists() && !output.source.exists(), "{name}");
    }
    fs::write(&narrow, original).unwrap();
    let prepared_pair = case.pair("prepared");
    let narrowed = case.pair("narrowed");
    let result = control_command(&args(&case, "--narrow-stream-checkpoint", &narrow,
        &prepared_pair, &narrowed, "narrow", true));
    let canonical = case.disk();
    for mode in ["wrong-pair", "changed-initial", "omitted-topology", "same-output"] {
        let output = case.pair(mode);
        let mut command = args(&case, "--narrow-stream-checkpoint", &narrow,
            &prepared_pair, &output, mode, true);
        let initial = fs::read(case.root.join("initial.json")).unwrap();
        match mode {
            "wrong-pair" => command[6] = narrowed.history.display().to_string(),
            "changed-initial" => fs::write(case.root.join("initial.json"), fixture::graph(1, true)).unwrap(),
            "omitted-topology" => command.truncate(9),
            "same-output" => {
                let path = Path::new(&command[10]);
                let document = fs::read_to_string(path).unwrap().replace(
                    output.source.to_str().unwrap(), output.history.to_str().unwrap());
                fs::write(path, document).unwrap();
            }
            _ => unreachable!(),
        }
        let mut report = Vec::new();
        assert!(control::command(&command, None, || ElapsedTick(1000), &mut report).is_err(), "{mode}");
        assert!(report.is_empty(), "{mode}");
        assert_eq!(case.disk(), canonical, "{mode}");
        assert!(!output.history.exists() && !output.source.exists(), "{mode}");
        fs::write(case.root.join("initial.json"), initial).unwrap();
    }
    let request = case.reset(cut, number(&result, "first_incident_sequence"));
    control_command(&args(&case, "--begin-stream-reset", &request,
        &narrowed, &case.pair("pending"), "begin", true));
    let before = case.disk();
    let stale = case.pair("unrelated-suffix");
    // The exact old Narrow event is still recorded, but old source custody
    // cannot adopt an additional Begin event under Narrow's retry allowance.
    let command = args(&case, "--narrow-stream-checkpoint", &narrow,
        &prepared_pair, &stale, "unrelated-suffix", true);
    assert!(control::command(&command, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
    assert_eq!(case.disk(), before);
    assert!(!stale.history.exists() && !stale.source.exists());
}

#[test]
fn ordinary_containment_retries_lost_paired_outputs_without_live_documents_or_new_work() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("lost ordinary containment report"))
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    for lose_source in [false, true] {
        let root = Root::new();
        let case = Case::ordinary(&root);
        let prepared = prepare(&case, true);
        let cut = info(&prepared);
        let narrow = case.narrow(cut);
        let original = checkpoint(&case.pair("prepared").source);
        for name in ["live-1.json", "inventory-1.json", "evidence.json", "native-roster.json",
            "helper-weights.safetensors"] {
            fs::remove_file(case.root.join(name)).unwrap();
        }
        let output = case.pair("lost-export");
        let command = args(&case, "--narrow-stream-checkpoint", &narrow,
            &case.pair("prepared"), &output, "lost-export", true);
        let before = case.disk();
        let mut occupied = false;
        let error = control::command(&command, None, || {
            if !occupied && case.disk() != before {
                fs::write(if lose_source { &output.source } else { &output.history },
                    b"independent output winner").unwrap();
                occupied = true;
            }
            ElapsedTick(1000)
        }, &mut Broken).unwrap_err();
        assert!(occupied);
        assert!(error.contains("acknowledged") && error.contains("lost ordinary containment report"), "{error}");
        assert_eq!(fs::read(if lose_source { &output.source } else { &output.history }).unwrap(),
            b"independent output winner");
        assert!(if lose_source { output.history.exists() } else { output.source.exists() });
        let canonical = case.disk();
        let repaired = case.pair("repaired");
        let command = args(&case, "--narrow-stream-checkpoint", &narrow,
            &case.pair("prepared"), &repaired, "repaired", true);
        // The public dispatcher repairs the exact acknowledged operation; it
        // cannot infer a new request or rerun numerical/source work for output.
        let mut output = Vec::new();
        crate::command_with_output(command, &mut output).unwrap();
        let result = report(&output);
        assert_eq!(text(&result, "outcome"), "already_narrowed");
        assert_eq!(number(&result, "appended_events"), 0);
        assert_eq!(number(&result, "sampled_draws"), 0);
        assert_eq!(number(&result, "restart_attempts"), 0);
        assert_eq!(case.disk(), canonical);
        unchanged_source(&original, &repaired, true);
        let reset = case.reset(cut, number(&result, "first_incident_sequence"));
        let pending = case.pair("pending");
        control_command(&args(&case, "--begin-stream-reset", &reset,
            &repaired, &pending, "begin", true));
        let canonical = case.disk();
        let retry = control_command(&args(&case, "--begin-stream-reset", &reset,
            &repaired, &case.pair("begin-retry"), "begin-retry", true));
        assert_eq!(text(&retry, "outcome"), "already_pending");
        assert_eq!(number(&retry, "appended_events"), 0);
        assert_eq!(case.disk(), canonical);
        let complete = case.pair("complete");
        let first = reset_command(&args(&case, "--complete-stream-reset", &reset,
            &pending, &complete, "complete", true));
        let retried = case.pair("complete-retry");
        let second = reset_command(&args(&case, "--complete-stream-reset", &reset,
            &pending, &retried, "complete-retry", true));
        for key in ["restart_attempts", "reserved_cache_values", "values_restored",
            "incident_count", "position", "sampled_draws", "charged", "reserved", "executions"] {
            assert_eq!(first.get(key), second.get(key), "{key}");
        }
        assert_eq!(number(&second, "revision"), number(&first, "revision") + 1,
            "only the original recovery Fence repeats");
        unchanged_source(&original, &retried, false);
        assert_eq!(case.publication().executions, 0);
        assert!(!case.root.join("missing-next.json").exists());
        unpredicted(&case);
    }
}

#[test]
fn ordinary_restoration_cannot_publish_with_a_missing_source_or_a_current_bypass() {
    for bypass in [false, true] {
        let root = Root::new();
        let case = Case::ordinary(&root);
        let (complete, _, _) = contained(&case, true);
        let before = checkpoint(&complete.source);
        let old_history = fs::read(&complete.history).unwrap();
        let old_source = fs::read(&complete.source).unwrap();
        let after = case.pair("refused");
        let profile = case.profile(&complete, &after, "refused", Some((2, bypass)));
        if !bypass {
            fs::remove_file(case.root.join("inventory-2.json")).unwrap();
        }
        let (peer_path, peers) = functional_creation::peers_file(&root, &case.config);
        let command = continuation_args(&case, &peer_path, &complete, &after, Some(&profile));
        let result = service::command_with_clock(&command, None, || {
            assert!(!peers.socket(1).exists(), "unqualified source cannot request the human key");
            ElapsedTick(1001)
        });
        assert!(result.is_err());
        let state = case.publication();
        assert_eq!(state.executions, 0);
        assert_eq!(state.control.ledger.charged, 0);
        assert_eq!(state.control.ledger.reserved, 0);
        let retained = checkpoint(&after.source);
        assert_eq!(retained.history_anchor(), &custody::load(&after.history).unwrap());
        assert!(!retained.available());
        assert_eq!(retained.work().checks, before.work().checks + u64::from(bypass));
        assert_eq!(retained.work().reserved_read_bytes,
            retained.work().checks * TOPOLOGY_REFRESH_READ_BYTES);
        assert_eq!(fs::read(&complete.history).unwrap(), old_history);
        assert_eq!(fs::read(&complete.source).unwrap(), old_source);
        unpredicted(&case);
    }
}
