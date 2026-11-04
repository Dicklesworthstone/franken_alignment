//! Actual operator capture, reset and publication with the original owned
//! predictor. The new source is freshly audited; no sampled history is rewound.
use super::*;
use super::super::super::{checkpoint_prepare, stream_checkpoint};
use fa_reference::action::consequence::delivery::{
    persistent::observed::{
        decoder::learned::{FileLearnedStreamControlReceipt, checkpoint::{
            FileLearnedCheckpointInfo, FileLearnedStreamNarrowControl, FileLearnedStreamNarrowRequest,
        }},
        reviewer::wire::ReviewDecision,
    },
    stream::ReleaseFrame,
};

struct Prepared {
    config: Config,
    recipe: PathBuf,
    loaded: Loaded,
    request: FileLearnedStreamNarrowRequest,
    command: Vec<String>,
}

fn number(value: &Json, name: &str) -> u64 {
    value.get(name).and_then(Json::as_str).unwrap().parse().unwrap()
}

fn narrow_bytes(request: &FileLearnedStreamNarrowRequest) -> Vec<u8> {
    let checkpoint = request.checkpoint(); let c = request.control();
    let targets = c.retained_targets.iter().map(|target| format!(
        r#"{{"adapter":"{}","object":"{}","contract_version":"{}","expected_version":"{}","generation":"{}"}}"#,
        target.adapter, target.object, target.contract_version, target.expected_version, target.generation,
    )).collect::<Vec<_>>().join(",");
    let evidence = c.binding.evidence_root.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    format!(concat!(
        "{{\"schema\":\"fa.learned-stream-narrow/1\",\"expected_journal_revision\":\"{}\",",
        "\"checkpoint\":{{\"checkpoint\":\"{}\",\"actor_revision\":\"{}\",\"authority_epoch\":\"{}\",",
        "\"control_sequence\":\"{}\",\"stream\":\"{}\",\"position\":\"{}\",\"sampled_draws\":\"{}\"}},",
        "\"control\":{{\"operation\":\"{}\",\"expected_control_sequence\":\"{}\",",
        "\"expected_actor_revision\":\"{}\",\"expected_authority_epoch\":\"{}\",",
        "\"binding\":{{\"round\":\"{}\",\"reducer_generation\":\"{}\",\"evidence_root_hex\":\"{}\"}},",
        "\"retained_targets\":[{}]}}}}"), request.revision(), checkpoint.checkpoint,
        checkpoint.actor_revision, checkpoint.authority_epoch, checkpoint.control_sequence,
        checkpoint.stream, checkpoint.position, checkpoint.sampled_draws, c.operation,
        c.expected_control_sequence, c.expected_actor_revision, c.expected_authority_epoch,
        c.binding.round, c.binding.reducer_generation, evidence, targets).into_bytes()
}

fn prepare(root: &Root) -> Prepared {
    let config_path = super::super::functional_creation::config_file(root, "functional_restart");
    let config = Config::read_learned_generation(&config_path).unwrap();
    let path = super::super::continuation::write(root);
    super::super::forecast::install_restart(root, &path, 60_000);
    evidence(root, &config, 1, true, false);
    let profile = root.0.join("owned-prepare-checkpoint.json");
    fs::write(&profile, r#"{"schema":"fa.learned-generation-checkpoint/1","checkpoint":"71","expected_position":"2","expected_sampled_draws":"0"}"#).unwrap();
    let input = root.0.join("owned-prepared.anchor");
    let prepare_args = vec!["create-learned-generated".into(), config_path.display().to_string(),
        path.display().to_string(), "--prepare-stream-checkpoint".into(), profile.display().to_string(),
        "--retain-history-anchor".into(), input.display().to_string()];
    let mut output = Vec::new();
    checkpoint_prepare::command(&prepare_args, None, || ElapsedTick(1000), &mut output).unwrap();
    let prepared = strict_json::parse(&output, Limits::default()).unwrap();
    assert_eq!(prepared.get("outcome").and_then(Json::as_str), Some("prepared"));
    assert_eq!(number(&prepared, "sampling_attempts"), 0);
    assert_eq!(number(&prepared, "executions"), 0);
    let checkpoint = FileLearnedCheckpointInfo {
        checkpoint: number(&prepared, "checkpoint"), actor_revision: number(&prepared, "actor_revision"),
        authority_epoch: number(&prepared, "authority_epoch"), control_sequence: number(&prepared, "control_sequence"),
        stream: number(&prepared, "stream"), position: number(&prepared, "position"),
        sampled_draws: number(&prepared, "sampled_draws"),
    };
    let mut loaded = recipe::load(&path, &config, false).unwrap();
    loaded.recovery_anchor = Some(custody::load(&input).unwrap());
    loaded.anchor_output = Some(root.0.join("owned-narrowed.anchor"));
    let before = FileOversight::read_publication_with_learned_generation(
        &config.store, &config.profile, &loaded.generation).unwrap();
    assert_eq!(before.revision, number(&prepared, "revision"));
    assert_eq!(before.control.sequence, checkpoint.control_sequence);
    assert_eq!(before.control.ledger.epoch, checkpoint.authority_epoch);
    let retained_targets = (0..3).map(|offset| ResolvedTarget {
        expected_version: before.target.expected_version.checked_add(offset).unwrap(), ..before.target
    }).collect();
    let request = FileLearnedStreamNarrowRequest::new(before.revision, checkpoint,
        FileLearnedStreamNarrowControl { operation: 801,
            expected_control_sequence: before.control.sequence,
            expected_actor_revision: checkpoint.actor_revision,
            expected_authority_epoch: before.control.ledger.epoch,
            binding: ReviewBinding { round: 801, reducer_generation: 1, evidence_root: [7; 32] },
            retained_targets }).unwrap();
    let request_path = root.0.join("owned-narrow.json");
    fs::write(&request_path, narrow_bytes(&request)).unwrap();
    let command = vec!["create-learned-generated".into(), config_path.display().to_string(),
        path.display().to_string(), "--narrow-stream-checkpoint".into(), request_path.display().to_string(),
        "--history-anchor".into(), input.display().to_string(), "--retain-history-anchor".into(),
        loaded.anchor_output.as_ref().unwrap().display().to_string()];
    Prepared { config, recipe: path, loaded, request, command }
}

fn prepared_owned(root: &Root) -> Case {
    let Prepared { config, recipe: path, mut loaded, request: narrow, mut command } = prepare(root);
    let before = FileOversight::read_publication_with_learned_generation(
        &config.store, &config.profile, &loaded.generation).unwrap();
    let mut output = Vec::new();
    stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    let narrowed = strict_json::parse(&output, Limits::default()).unwrap();
    assert_eq!(narrowed.get("outcome").and_then(Json::as_str), Some("narrowed"));
    assert_eq!(number(&narrowed, "appended_events"), 1);
    let first = number(&narrowed, "first_incident_sequence");
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    command[8] = root.0.join("owned-narrow-retry.anchor").display().to_string();
    output.clear();
    stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    let retried = strict_json::parse(&output, Limits::default()).unwrap();
    assert_eq!(retried.get("outcome").and_then(Json::as_str), Some("already_narrowed"));
    assert_eq!(number(&retried, "first_incident_sequence"), first);
    assert_eq!(number(&retried, "appended_events"), 0);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    // The sealed retry exposes observations only and adds no ordinary Fence.
    let history = recovery::stream_checkpoint::execute(&config, &loaded,
        &stream_checkpoint::Request::Narrow(narrow.clone()), &mut |_| Ok(())).unwrap();
    assert_eq!(history.appended_events(), 0);
    let FileLearnedStreamControlReceipt::Narrowed { receipt, .. } = history.receipt()
        else { panic!("original narrowing receipt missing"); };
    assert_eq!(receipt.sequence, first);
    assert_eq!(receipt.incident_count, 0);
    assert_eq!(history.snapshot().control.ledger.charged, before.control.ledger.charged);
    assert_eq!(history.snapshot().executions, before.executions);
    assert_eq!(history.numerical().actor_revision, narrow.checkpoint().actor_revision);
    assert_eq!(history.numerical().sampled_draws, 0);
    assert_eq!(history.numerical().cumulative_work.sampling_attempts, 0);
    assert!(narrow.checkpoint().control_sequence < first);
    assert_eq!(loaded.expected_policy, config.profile.delivery.policy);
    let before = history.snapshot().clone();
    let control = FileResetRequest { operation: 900, expected_control_sequence: first,
        expected_actor_revision: history.numerical().actor_revision,
        expected_authority_epoch: before.control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: narrow.control().retained_targets.clone() };
    let request = reset::Request {
        intent: FileLearnedResetIntent::for_recovery(narrow.checkpoint().checkpoint, control, allowance()).unwrap(),
        first_incident_sequence: first,
    };
    loaded.recovery_anchor = Some(history.history_anchor().clone());
    loaded.anchor_output = Some(root.0.join("owned-begun.anchor"));
    let numerical = history.numerical().clone();
    Case { config, recipe: path, loaded, request, before, numerical }
}

fn begin(root: &Root, case: &Case) -> Vec<String> {
    let mut command = args(root, case);
    command[3] = "--begin-stream-reset".into();
    let mut output = Vec::new();
    stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    let value = strict_json::parse(&output, Limits::default()).unwrap();
    assert_eq!(value.get("outcome").and_then(Json::as_str), Some("pending"));
    let canonical = disk(case);
    command[8] = root.0.join("owned-begin-retry.anchor").display().to_string();
    stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut Vec::new()).unwrap();
    assert_eq!(disk(case), canonical);
    command[6] = command[8].clone();
    command[3] = "--complete-stream-reset".into();
    command[8] = root.0.join("owned-completed.anchor").display().to_string();
    command
}

#[test]
fn owned_operator_reset_rearms_the_first_message_and_still_requires_native_and_human_review() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new(); let case = prepared_owned(&root);
        let command = begin(&root, &case);
        let mut output = Vec::new();
        reset::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
        let restored = report(&output);
        assert_eq!(restored.get("restored").and_then(Json::as_bool), Some(true));
        assert_eq!(restored.get("position").and_then(Json::as_str), Some("2"));
        assert_eq!(restored.get("sampled_draws").and_then(Json::as_str), Some("0"));
        let result = FileOversight::read_learned_reset_result(&case.config.store,
            &case.config.profile, &case.loaded.generation, 900).unwrap();
        let FileLearnedResetRecord::Completed { result: Ok(receipt), .. } = result.record()
            else { panic!("original reset did not complete"); };
        assert!(receipt.restored_forecast_audit.is_some());
        let (peer_path, peers) = super::super::functional_creation::peers_file(&root, &case.config);
        let live = vec!["create-learned-generated".into(), command[1].clone(), command[2].clone(),
            peer_path.display().to_string(), "--continue-generation".into(), "--history-anchor".into(),
            command[8].clone(), "--retain-history-anchor".into(),
            root.0.join("owned-published.anchor").display().to_string()];
        let mut human = None;
        let published = super::super::super::command_with_clock(&live, None, || {
            if human.is_none() && peers.socket(1).exists() {
                human = Some(super::super::peer_intake::human(peers.clone(), 1, decision));
            }
            ElapsedTick(1001)
        });
        let packet = human.expect("new forecast and native review reached human custody").join().unwrap();
        assert_eq!(published.is_ok(), decision == ReviewDecision::Approve);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        let after = snapshot(&case);
        assert_eq!(after.executions, u64::from(decision == ReviewDecision::Approve));
        if decision == ReviewDecision::Approve {
            let canonical = disk(&case);
            let retry = vec!["create-learned-generated".into(), command[1].clone(), command[2].clone(),
                "--narrow-stream-checkpoint".into(), root.0.join("owned-narrow.json").display().to_string(),
                "--history-anchor".into(), root.0.join("owned-prepared.anchor").display().to_string(),
                "--retain-history-anchor".into(), root.0.join("owned-historical-narrow.anchor").display().to_string()];
            let mut retry_output = Vec::new();
            stream_checkpoint::command(&retry, None, || ElapsedTick(1001), &mut retry_output).unwrap();
            let historical = strict_json::parse(&retry_output, Limits::default()).unwrap();
            assert_eq!(historical.get("outcome").and_then(Json::as_str), Some("already_narrowed"));
            assert_eq!(number(&historical, "appended_events"), 0);
            assert_eq!(number(&historical, "first_incident_sequence"), case.request.first_incident_sequence);
            assert!(number(&historical, "control_sequence") > number(&historical, "first_incident_sequence"));
            assert_eq!(disk(&case), canonical);
            let mut loaded = recipe::load(&case.recipe, &case.config, false).unwrap();
            loaded.recovery_anchor = Some(custody::load(Path::new(&live[8])).unwrap());
            let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.sampled_draws, 2);
            assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
            assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
            assert_eq!(host.inspect().control.ledger.charged, packet.action().spec().units);
            assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
            assert_eq!(host.current_policy().unwrap(), &loaded.expected_policy);
        }
    }
}

#[test]
fn owned_reset_requires_the_original_recipe_current_policy_and_exact_intent_before_mutation() {
    let root = Root::new(); let case = prepared_owned(&root);
    let mut command = args(&root, &case); command[3] = "--begin-stream-reset".into();
    let canonical = disk(&case);
    let recipe = fs::read_to_string(&case.recipe).unwrap();
    let wrong_policy = r#"{"generation":2,"nodes":[{"op":"exact_value","key":7,"value_hex":"6f6b"},{"op":"payload_at_most","maximum":512},{"op":"units_at_most","maximum":512},{"op":"all","children":[0,1,2]}]}"#;
    fs::write(&case.recipe, format!(
        r#"{{"schema":"fa.learned-recovery-policy/1","effective_policy":{wrong_policy},"recipe":{recipe}}}"#)).unwrap();
    assert!(stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
    assert_eq!(disk(&case), canonical);
    fs::write(&case.recipe, recipe).unwrap();
    let forecast = root.0.join("forecast.json");
    let selected = fs::read_to_string(&forecast).unwrap();
    fs::write(&forecast, selected.replace(r#""reset":"owned_prompt_audit""#, r#""reset":"disabled""#)).unwrap();
    assert!(stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
    assert_eq!(disk(&case), canonical);
    fs::write(&forecast, selected).unwrap();
    let request = fs::read_to_string(&command[4]).unwrap();
    fs::write(&command[4], request.replace(r#""checkpoint":"71""#, r#""checkpoint":"72""#)).unwrap();
    assert!(stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
    assert_eq!(disk(&case), canonical);
    assert!(!Path::new(&command[8]).exists());
    fs::write(&command[4], request).unwrap();
    stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut Vec::new()).unwrap();
    assert_eq!(snapshot(&case).revision, case.before.revision + 1);
}

#[test]
fn owned_reset_lost_report_retry_retains_both_audits_without_another_incident() {
    let root = Root::new(); let case = prepared_owned(&root);
    let mut command = begin(&root, &case);
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::other("lost report")) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let error = reset::command(&command, None, || ElapsedTick(1000), &mut Broken).unwrap_err();
    assert!(error.contains("acknowledged"), "{error}");
    assert!(Path::new(&command[8]).exists());
    let before = FileOversight::read_learned_reset_result(&case.config.store,
        &case.config.profile, &case.loaded.generation, 900).unwrap();
    let completed = snapshot(&case);
    command[8] = root.0.join("owned-complete-retry.anchor").display().to_string();
    let mut output = Vec::new();
    reset::command(&command, None, || ElapsedTick(1000), &mut output).unwrap();
    let after = FileOversight::read_learned_reset_result(&case.config.store,
        &case.config.profile, &case.loaded.generation, 900).unwrap();
    let (FileLearnedResetRecord::Completed { intent: first, result: Ok(first_result) },
        FileLearnedResetRecord::Completed { intent: last, result: Ok(last_result) }) =
        (before.record(), after.record()) else { panic!("completed reset evidence was lost"); };
    assert_eq!(first, last);
    assert_eq!(first_result.actor_revision, last_result.actor_revision);
    assert_eq!(first_result.resumed_stream, last_result.resumed_stream);
    assert_eq!(first_result.position, last_result.position);
    assert_eq!(first_result.sampled_draws, last_result.sampled_draws);
    let first_audit = first_result.restored_forecast_audit.as_ref().unwrap();
    let last_audit = last_result.restored_forecast_audit.as_ref().unwrap();
    assert_eq!(first_audit.compression().source, last_audit.compression().source);
    assert_eq!(first_audit.compression().work_units_reserved, last_audit.compression().work_units_reserved);
    assert_eq!(first_audit.monitoring().work(), last_audit.monitoring().work());
    assert!(first_audit.monitoring().complete_quiet() && last_audit.monitoring().complete_quiet());
    assert_eq!(snapshot(&case).revision, completed.revision + 1); // Ordinary recovery Fence only.
    let value = report(&output);
    assert_eq!(value.get("restart_attempts").and_then(Json::as_str), Some("1"));
    assert_eq!(value.get("incident_count").and_then(Json::as_str), Some("1"));
    assert_eq!(value.get("charged").and_then(Json::as_str), Some("0"));
}

#[test]
fn owned_prompt_narrowing_requires_exact_custody_and_closed_independent_request_data() {
    let root = Root::new(); let prepared = prepare(&root);
    let canonical = fs::read(prepared.config.store.join("delivery.bin")).unwrap();
    let original = String::from_utf8(narrow_bytes(&prepared.request)).unwrap();
    let stale = prepared.request.control().expected_control_sequence + 1;
    let current_control = format!("\"expected_control_sequence\":\"{}\"", prepared.request.control().expected_control_sequence);
    let wrong_control = format!("\"expected_control_sequence\":\"{stale}\"");
    for document in [
        original.replacen(r#""operation":"801""#, r#""operation":"0801""#, 1),
        original.replacen(r#""operation":"801""#, r#""operation":801"#, 1),
        original.replacen(r#""operation":"801""#, r#""operation":"801","unknown":true"#, 1),
        original.replacen(r#""operation":"801""#, r#""operation":"801","operation":"801""#, 1),
        original.replacen(r#""checkpoint":"71""#, r#""checkpoint":"72""#, 1),
        original.replacen(&current_control, &wrong_control, 1),
        original.replacen(&"07".repeat(32), &"00".repeat(32), 1),
    ] {
        assert_ne!(document, original);
        fs::write(&prepared.command[4], document).unwrap();
        assert!(stream_checkpoint::command(&prepared.command, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
        assert_eq!(fs::read(prepared.config.store.join("delivery.bin")).unwrap(), canonical);
        assert!(!Path::new(&prepared.command[8]).exists());
    }
    fs::write(&prepared.command[4], &original).unwrap();
    let forecast = root.0.join("forecast.json"); let original_forecast = fs::read_to_string(&forecast).unwrap();
    for disabled in [
        original_forecast.replace(r#""reset":"owned_prompt_audit""#, r#""reset":"disabled""#),
        original_forecast.replace(r#""schema":"fa.learned-forecast/2","reset":"owned_prompt_audit""#,
            r#""schema":"fa.learned-forecast/1""#),
    ] {
        fs::write(&forecast, disabled).unwrap();
        let error = stream_checkpoint::command(&prepared.command, None, || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
        assert!(error.contains("explicit owned prompt-audit"), "{error}");
        assert_eq!(fs::read(prepared.config.store.join("delivery.bin")).unwrap(), canonical);
    }
    fs::write(&forecast, original_forecast).unwrap();
    let plain_path = root.0.join("plain-recipe.json");
    let plain = fs::read_to_string(&prepared.recipe).unwrap()
        .replace("fa.learned-publication/5", "fa.learned-publication/4")
        .replace(&format!("\"forecast\":\"{}\",", forecast.display()), "");
    fs::write(&plain_path, plain).unwrap();
    let mut command = prepared.command.clone(); command[2] = plain_path.display().to_string();
    let error = stream_checkpoint::command(&command, None, || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("narrowing requires the explicit owned prompt-audit"), "{error}");
    assert_eq!(fs::read(prepared.config.store.join("delivery.bin")).unwrap(), canonical);
    stream_checkpoint::command(&prepared.command, None, || ElapsedTick(1000), &mut Vec::new()).unwrap();
}

#[test]
fn owned_prompt_narrowing_retries_lost_output_without_widening_or_repeating_control() {
    for lose_archive in [false, true] {
        let root = Root::new(); let mut prepared = prepare(&root);
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::other("lost narrowing report")) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        let failure = if lose_archive {
            // The selected destination appears after admission. Exercise the
            // shared production run's create_new boundary after acknowledgment.
            fs::write(&prepared.command[8], b"original independent archive").unwrap();
            stream_checkpoint::run(&prepared.config, &prepared.loaded,
                &stream_checkpoint::Request::Narrow(prepared.request.clone()), &deadline(),
                &mut || ElapsedTick(1000), &mut Broken).unwrap_err()
        } else {
            stream_checkpoint::command(&prepared.command, None, || ElapsedTick(1000), &mut Broken).unwrap_err()
        };
        assert!(failure.contains("acknowledged"), "{failure}");
        if lose_archive { assert_eq!(fs::read(&prepared.command[8]).unwrap(), b"original independent archive"); }
        else { custody::load(Path::new(&prepared.command[8])).unwrap(); }
        let canonical = fs::read(prepared.config.store.join("delivery.bin")).unwrap();
        prepared.command[8] = root.0.join("owned-narrow-recovered.anchor").display().to_string();
        let mut output = Vec::new();
        stream_checkpoint::command(&prepared.command, None, || ElapsedTick(1000), &mut output).unwrap();
        let result = strict_json::parse(&output, Limits::default()).unwrap();
        assert_eq!(result.get("outcome").and_then(Json::as_str), Some("already_narrowed"));
        assert_eq!(number(&result, "appended_events"), 0);
        assert_eq!(fs::read(prepared.config.store.join("delivery.bin")).unwrap(), canonical);
        let mut changed = prepared.request.control().clone();
        changed.retained_targets.push(ResolvedTarget { expected_version: 99, ..changed.retained_targets[0] });
        let wider = FileLearnedStreamNarrowRequest::new(prepared.request.revision(),
            prepared.request.checkpoint(), changed).unwrap();
        prepared.command[8] = root.0.join("owned-narrow-wider.anchor").display().to_string();
        fs::write(&prepared.command[4], narrow_bytes(&wider)).unwrap();
        assert!(stream_checkpoint::command(&prepared.command, None, || ElapsedTick(1000), &mut Vec::new()).is_err());
        assert_eq!(fs::read(prepared.config.store.join("delivery.bin")).unwrap(), canonical);
        assert!(!Path::new(&prepared.command[8]).exists());
    }
}
