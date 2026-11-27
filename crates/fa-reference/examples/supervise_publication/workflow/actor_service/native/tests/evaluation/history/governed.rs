//! The retired policy owner leaves original cases available to an independent
//! historical evaluator. Native weights and helper choices are fixture inputs.
use super::*;
use fa_reference::action::consequence::{
    delivery::{
        EndpointOutcome,
        persistent::{
            Reconciliation,
            governance::PolicyUpdate,
            observed::guarded::FileCampaignRequirement,
            requests::FileRequestDisposition,
        },
    },
    gate::containment::session::policy::Policy,
    oversight::{ReviewWindow, credibility::EvaluationCase},
    policy_campaign::ReplayLimits,
};
use fa_reference::round::commitment;

const POLICY: &str = r#"{"generation":2,"nodes":[{"op":"exact_value","key":7,"value_hex":"6f6b"},{"op":"payload_at_most","maximum":256},{"op":"units_at_most","maximum":512},{"op":"all","children":[0,1,2]}]}"#;

fn candidate() -> Policy {
    crate::config::policy_from_json(
        strict_json::parse(POLICY.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap(),
    ).unwrap()
}

fn campaigns() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 4,
    }
}

struct Governed {
    original: Original,
    case: EvaluationCase,
    requirements: PathBuf,
}

fn requirements_document(policy: &str, floor: FileRecoveryFloor) -> String {
    format!(
        r#"{{"schema":"fa.native-recovery-requirements/1","effective_policy":{policy},"guards":{{"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}},"decoder_stop":null,"identity":null,"credential":null}},"minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}}}}"#,
        floor.journal_revision, floor.control_sequence, floor.authority_epoch,
    )
}

/// Exercise the original native source, numerical command, full-input review,
/// human key and campaign reducer before retiring every role and the owner.
fn seed(root: &Root, hold: bool) -> Governed {
    let mut config = configured(root);
    let native = inputs(root, &config);
    let accounting = accounting_file(root);
    let settings = andon_settings::Settings::read(&accounting).unwrap();
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let (mut host, human, governor, evaluator) = bootstrap::prepare_andon_selected(
        &config, &native, selected, &settings, Some(campaigns()),
    ).unwrap();
    let governor = governor.expect("original campaign role");
    let captured = host.refresh_file_source(
        host.revision(), &mut config.source, ElapsedTick(1000),
    ).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        native.generation, numerical.actor_revision, numerical.position, native.request.clone(),
    ).unwrap();
    host.generate_decoder_text(host.revision(), command).unwrap();
    let generated = host.decoder_text_progress(native.generation).unwrap();
    assert_eq!(generated.bytes().unwrap(), b"A");
    let source = FileTextMessageRequest {
        request: 91, generation: native.generation,
        generation_revision: generated.generation_revision(),
        target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(10_000),
    };
    let snapshot = captured.snapshot().clone();
    let status = host.submit_decoder_text_message(host.revision(), source, snapshot.clone()).unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("original native Message was not admitted");
    };
    let action = host.request_action(91).unwrap().clone();
    let full_inputs = captured.inputs_for(&action, &config.profile.committee).unwrap();
    host.record_inputs(host.revision(), attempt, 0, full_inputs.clone()).unwrap();
    let evidence = captured.reference_root();
    host.begin_review(
        host.revision(), attempt, 91, evidence,
        ReviewWindow { commit_by: ElapsedTick(1005), reveal_by: ElapsedTick(1008) },
        snapshot.clone(),
    ).unwrap();
    for name in ["alpha", "beta"] {
        let verdict = if hold && name == "alpha" { Verdict::Hold } else { Verdict::Allow };
        host.commit_review(
            host.revision(), 91, name,
            commitment(91, name, &evidence, verdict, b"salt").unwrap(),
        ).unwrap();
    }
    host.open_reveals(host.revision(), 91).unwrap();
    for name in ["alpha", "beta"] {
        let verdict = if hold && name == "alpha" { Verdict::Hold } else { Verdict::Allow };
        host.reveal_review(host.revision(), 91, name, verdict, b"salt".to_vec()).unwrap();
    }
    host.finish_review(host.revision(), 91, Some(&full_inputs), snapshot.clone())
        .unwrap().unwrap();
    if hold {
        host.cancel(host.revision(), attempt).unwrap();
    } else {
        let automatic = host.authorize(
            host.revision(), attempt, &full_inputs, snapshot.clone(),
        ).unwrap();
        assert!(host.publish_checked(
            host.revision(), attempt, Some(&full_inputs), snapshot.clone(), ElapsedTick(1000),
        ).is_err(), "native review replaced the independent human key");
        let request = host.request_human_approval(
            host.revision(), 1091, attempt, &full_inputs, ElapsedTick(2000),
        ).unwrap();
        let revision = host.revision();
        let permit = human.approve(&mut host, revision, &request).unwrap();
        host.dispatch(
            host.revision(), &automatic, &permit, &action, &full_inputs, snapshot.clone(),
        ).unwrap();
        assert!(matches!(host.publish_checked(
            host.revision(), attempt, Some(&full_inputs), snapshot, ElapsedTick(1000),
        ).unwrap().outcome, EndpointOutcome::Executed { .. }));
        assert!(matches!(host.reconcile(host.revision(), attempt).unwrap(),
            Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
    }
    let case = evaluator.evaluation_case(&host, 91).unwrap().case().clone();
    assert_eq!(case.policy_generation, 1);
    assert_eq!(case.stopped, hold);
    let numerical = host.decoder_inspection().unwrap().numerical;
    let control = host.inspect().control;
    let update = PolicyUpdate::new(701, control.sequence, control.ledger.epoch, candidate()).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &update).unwrap();
    let revision = host.revision();
    let permit = governor.approve(&mut host, revision, &campaign, false).unwrap();
    host.promote_policy_campaign(host.revision(), &permit).unwrap();
    assert_eq!(host.current_policy().unwrap().generation(), 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().control.ledger.charged, control.ledger.charged);
    assert_eq!(host.inspect().executions, u64::from(!hold));
    assert!(host.evaluation_history(91).unwrap().is_empty());
    let before = host.inspect();
    let anchor = root.0.join("retired-governor.anchor");
    archive::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
    let requirements = root.0.join("governed-history-requirements.json");
    fs::write(&requirements, requirements_document(POLICY, FileRecoveryFloor {
        journal_revision: before.revision,
        control_sequence: before.control.sequence,
        authority_epoch: before.control.ledger.epoch,
    })).unwrap();
    drop(evaluator);
    drop(governor);
    drop(human);
    drop(host);
    config.programs.clear();
    Governed { original: Original { config, native, accounting, anchor, before }, case, requirements }
}

fn arguments(root: &Root, seeded: &Governed, pid: u32) -> Vec<String> {
    let mut args = historical_args(root, &seeded.original, pid, false);
    args.extend(["--recovery-requirements".into(), seeded.requirements.display().to_string()]);
    args
}

fn expected(seeded: &Governed) -> FileAndonRecoveryRequirements {
    let original = &seeded.original;
    let selected = bootstrap::selection(&original.config, &original.native, None).unwrap();
    let governed = recovery_requirements::Selection::read(
        &seeded.requirements, &original.config,
    ).unwrap().requirements(&original.config, &original.native);
    evaluation::recovery::Requirements::read(
        &seeded.requirements.parent().unwrap().join("recovery-requirements.json"),
    ).unwrap().expected_selected(
        &andon_settings::Settings::read(&original.accounting).unwrap(),
        &original.config, &original.native, &selected, Some(&governed),
    ).unwrap()
}

#[test]
fn retired_native_governor_allows_delayed_original_labels_without_new_authority_or_work() {
    for hold in [false, true] {
        let root = Root::new();
        let seeded = seed(&root, hold);
        let mut child = pending_peer(&root, false);
        let arguments = arguments(&root, &seeded, child.0.id());
        let required = expected(&seeded);
        let original = seeded.original;
        let (actor, reviewer) = profiles(&root, &original.config);
        let (evaluation, history) = admit(&original.config, &actor, &reviewer, &arguments, true);
        let store = original.config.store.clone();
        let profile = original.config.profile.clone();
        let decoder = original.native.decoder.clone();
        let tokenizer = original.native.tokenizer.clone();
        let before = FileOversight::read_decoder_text_progress(
            &store, &profile, &decoder, &tokenizer, 7,
        ).unwrap();
        let old_anchor = fs::read(&original.anchor).unwrap();
        fs::remove_file(original.config.source.path()).unwrap();
        fs::remove_file(root.0.join("model.safetensors")).unwrap();
        assert!(original.config.programs.is_empty());
        let ready = root.0.join("history.ready");
        let result = evaluation.serve_history(
            original.config, (&actor, &reviewer), original.native, None, &history, None,
            || {
                assert!(!actor.socket.exists());
                assert!(!reviewer.socket(91).exists());
                assert!(!root.0.join("governor.sock").exists());
                assert!(!root.0.join("operator.sock").exists());
                if evaluation.peer.socket_path().exists() && !ready.exists() {
                    fs::write(&ready, b"retired original governor; independent labels only").unwrap();
                }
                crate::workflow::clock()
            },
        );
        fs::write(root.0.join("history.finished"), b"historical evaluator retired").unwrap();
        assert!(result.is_ok(), "governed history failed: {result:?}");
        child.finish();
        let retained = archive::load(history.output().unwrap()).unwrap();
        let saved = FileOversight::read_andon_guarded(
            &store, &profile, &required,
            &andon_settings::Settings::read(&original.accounting).unwrap().protocol, &retained,
        ).unwrap();
        let after = FileOversight::read_decoder_text_progress(
            &store, &profile, &decoder, &tokenizer, 7,
        ).unwrap();
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        assert_eq!(after.text.command(), before.text.command());
        assert_eq!(after.text.generation_revision(), before.text.generation_revision());
        assert_eq!(after.publication.executions, before.publication.executions);
        assert_eq!(after.publication.payload, before.publication.payload);
        assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
        assert_eq!(after.publication.control.ledger.reserved, before.publication.control.ledger.reserved);
        assert_eq!(after.publication.control.ledger.epoch, before.publication.control.ledger.epoch + 1);
        assert_eq!(after.publication.revision, before.publication.revision + 3);
        assert_eq!(retained.revision(), after.publication.revision);
        assert_eq!(saved.credibility.report.policy_generation, 2);
        assert_eq!(saved.credibility.report.retained_cases, 1);
        assert_eq!(saved.credibility.report.scoped_cases, 0);
        assert_eq!(saved.credibility.report.benign_origins, 0);
        assert_eq!(saved.credibility.report.lifetime_false_stops, u64::from(hold));
        assert_eq!(saved.credibility.report.revision, 2);
        let window = &saved.accounting.windows()[&1];
        assert_eq!(window.false_stops, u64::from(hold));
        assert_eq!(window.total_cost, if hold { 7 } else { 0 });
        assert_eq!(window.incident.is_some(), hold);
        let page = read_json(&root.0.join("history.case.json"));
        let cases = field(&page, &["knowledge", "value", "cases"]).as_array().unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(number(&cases[0], &["round"]), seeded.case.round);
        assert_eq!(number(&cases[0], &["policy_generation"]), seeded.case.policy_generation);
        let evidence: String = seeded.case.evidence_root.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(field(&cases[0], &["case_evidence_root_hex"]).as_str(), Some(evidence.as_str()));
        assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
        assert!(!evaluation.peer.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, 91).exists());
        assert!(!root.0.join("evidence.json").exists());
        assert!(!root.0.join("model.safetensors").exists());
    }
}

#[test]
fn governed_historical_expectations_refuse_before_fence_and_never_refund_original_work() {
    for defect in 0..11 {
        let root = Root::new();
        let seeded = seed(&root, false);
        let mut arguments = arguments(
            &root, &seeded, std::process::id().checked_add(100_000).unwrap(),
        );
        let original = seeded.original;
        let mut policy = fs::read_to_string(&seeded.requirements).unwrap();
        let evaluation_path = root.0.join("recovery-requirements.json");
        let mut evaluation_document = fs::read_to_string(&evaluation_path).unwrap();
        match defect {
            0 => {
                let index = arguments.iter().position(|arg| arg == "--recovery-requirements").unwrap();
                arguments.drain(index..index + 2);
            }
            1 => policy = policy.replace(POLICY, &POLICY.replace("\"generation\":2", "\"generation\":1")
                .replace("\"maximum\":256", "\"maximum\":512")),
            2 => policy = policy.replace("\"max_campaigns\":4", "\"max_campaigns\":5"),
            3..=8 => {
                let (coordinate, value) = match (defect - 3) % 3 {
                    0 => ("journal_revision", original.before.revision),
                    1 => ("control_sequence", original.before.control.sequence),
                    _ => ("authority_epoch", original.before.control.ledger.epoch),
                };
                let selected = if defect < 6 { &mut policy } else { &mut evaluation_document };
                *selected = selected.replace(
                    &format!("\"{coordinate}\":{value}"),
                    &format!("\"{coordinate}\":{}", value + 1),
                );
            }
            9 => {
                let accounting = fs::read_to_string(&original.accounting).unwrap();
                fs::write(&original.accounting, accounting.replace(
                    "\"minimum_evaluation_revision\": 0", "\"minimum_evaluation_revision\": 2",
                )).unwrap();
            }
            10 => {}
            _ => unreachable!(),
        }
        fs::write(&seeded.requirements, policy).unwrap();
        fs::write(&evaluation_path, evaluation_document).unwrap();
        let (mut actor, reviewer) = profiles(&root, &original.config);
        if defect == 10 { actor.request = 92; }
        let (evaluation, history) = admit(&original.config, &actor, &reviewer, &arguments, true);
        let store = original.config.store.clone();
        let canonical = fs::read(store.join("delivery.bin")).unwrap();
        let staging = store.join("delivery.pending");
        fs::write(&staging, b"unacknowledged original governed staging").unwrap();
        let old_anchor = fs::read(&original.anchor).unwrap();
        let profile = original.config.profile.clone();
        let decoder = original.native.decoder.clone();
        let tokenizer = original.native.tokenizer.clone();
        let before = FileOversight::read_decoder_text_progress(
            &store, &profile, &decoder, &tokenizer, 7,
        ).unwrap();
        fs::remove_file(original.config.source.path()).unwrap();
        let result = evaluation.serve_history(
            original.config, (&actor, &reviewer), original.native, None, &history, None,
            || {
                assert!(!actor.socket.exists());
                assert!(!reviewer.socket(actor.request).exists());
                assert!(!evaluation.peer.socket_path().exists());
                ElapsedTick(1001)
            },
        );
        assert!(result.is_err(), "changed governed history {defect} acquired custody");
        assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), canonical, "{defect}");
        assert_eq!(fs::read(&staging).unwrap(), b"unacknowledged original governed staging");
        assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
        let after = FileOversight::read_decoder_text_progress(
            &store, &profile, &decoder, &tokenizer, 7,
        ).unwrap();
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
        assert_eq!(after.publication.control.ledger.epoch, before.publication.control.ledger.epoch);
        assert_eq!(after.publication.executions, before.publication.executions);
        assert!(!history.output().unwrap().exists());
        assert!(!evaluation.peer.socket_path().exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, actor.request).exists());
    }
}

#[test]
fn governed_history_requires_explicit_evaluation_only_and_never_starts_live_authority() {
    let root = Root::new();
    let mut valid = args(&root, Path::new("missing-andon"), Path::new("missing-evaluator"));
    valid[0] = "serve-open".into();
    valid.extend([
        "--history-anchor".into(), "missing-history".into(),
        "--evaluation-recovery".into(), "missing-evaluation-floors".into(),
        "--recovery-requirements".into(), "missing-policy-requirements".into(),
        "--evaluate-only".into(),
    ]);
    let (remaining, options) = evaluation::take_options(&valid).unwrap();
    assert!(options.is_some());
    assert!(!remaining.iter().any(|arg| arg == "--recovery-requirements" || arg == "--evaluate-only"));
    let mut finish = valid.clone();
    finish.extend(["--finish-after-message".into(), "90".into()]);
    assert!(evaluation::take_options(&finish).is_ok());
    finish[0] = "serve-open-checked".into();
    finish.insert(4, "missing-publication".into());
    assert!(evaluation::take_options(&finish).is_ok());
    for defect in 0..9 {
        let mut invalid = valid.clone();
        match defect {
            0 => invalid.retain(|arg| arg != "--evaluate-only"),
            1 => invalid[0] = "serve-create".into(),
            2 => invalid.extend(["--governor-profile".into(), "missing-governor".into()]),
            3 => invalid.extend([
                "--operator-profile".into(), "missing-operator".into(),
                "--tower-generation".into(), "1".into(),
            ]),
            4 => invalid.extend(["--precheck-profile".into(), "missing-precheck".into()]),
            5 => invalid.extend(["--after-message".into(), "90".into()]),
            6 => invalid.extend(["--recovery-requirements".into(), "other-policy".into()]),
            7 => {
                let index = invalid.iter().position(|arg| arg == "--evaluation-recovery").unwrap();
                invalid.drain(index..index + 2);
            }
            _ => {
                let index = invalid.iter().position(|arg| arg == "--recovery-requirements").unwrap();
                invalid.remove(index + 1);
            }
        }
        let mut output = Vec::new();
        let error = command_with_clock(
            &invalid, None, || panic!("invalid historical governance observed time"), &mut output,
        ).unwrap_err();
        assert!(error.contains("native evaluation requires"), "{defect}: {error}");
        assert!(output.is_empty());
        assert!(!root.0.join("store").exists());
    }
}

#[test]
fn governed_history_requirements_have_independent_custody_before_the_original_cursor() {
    let root = Root::new();
    let seeded = seed(&root, false);
    let original = &seeded.original;
    let valid = arguments(&root, &seeded, std::process::id().checked_add(100_000).unwrap());
    let (actor, reviewer) = profiles(&root, &original.config);
    let canonical = fs::read(original.config.store.join("delivery.bin")).unwrap();
    for alias in [
        original.config.store.join("selected-policy.json"),
        original.config.source.path().to_path_buf(),
        actor.socket.clone(), reviewer.socket(91),
        crate::workflow::control::socket_path(&reviewer, 91),
        root.0.join("evaluator.sock"),
        original.accounting.clone(), root.0.join("evaluator.json"),
        root.0.join("recovery-requirements.json"),
        original.anchor.clone(), root.0.join("historical.anchor"),
    ] {
        let mut invalid = valid.clone();
        let index = invalid.iter().position(|arg| arg == "--recovery-requirements").unwrap();
        invalid[index + 1] = alias.display().to_string();
        let (remaining, options) = evaluation::take_options(&invalid).unwrap();
        let (_, history) = anchors::take_options(&remaining).unwrap();
        let history = history.admit(&original.config, true).unwrap().unwrap();
        let error = options.unwrap().read(
            (&original.config, &actor, &reviewer), (None, None, Some(&history)),
        ).err().expect("overlapping policy requirements acquired custody");
        assert!(error.contains("overlap") || error.contains("distinct") || error.contains("alias"), "{alias:?}: {error}");
        assert_eq!(fs::read(original.config.store.join("delivery.bin")).unwrap(), canonical);
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(91).exists());
        assert!(!root.0.join("evaluator.sock").exists());
    }
    let (evaluation, _) = admit(&original.config, &actor, &reviewer, &valid, true);
    assert!(evaluation.check_inputs(&[seeded.requirements.as_path()]).is_err());
}

#[test]
fn governed_historical_stop_precedes_missing_request_and_retains_original_policy_and_charges() {
    let root = Root::new();
    let seeded = seed(&root, false);
    let arguments = arguments(&root, &seeded, std::process::id().checked_add(100_000).unwrap());
    let required = expected(&seeded);
    let mut original = seeded.original;
    let (actor, reviewer) = profiles(&root, &original.config);
    let (evaluation, history) = admit(&original.config, &actor, &reviewer, &arguments, true);
    // This exact recipe has no original Message for generation eight. Ready
    // Stop must still consume the original terminal target before that lookup.
    original.native.generation += 1;
    let store = original.config.store.clone();
    let profile = original.config.profile.clone();
    let old_anchor = fs::read(&original.anchor).unwrap();
    fs::remove_file(original.config.source.path()).unwrap();
    let total = usize::try_from(original.before.revision).unwrap();
    let mut calls = 0;
    let mut client = None;
    let error = evaluation.serve_history(
        original.config, (&actor, &reviewer), original.native, None, &history, None,
        || {
            let at_ready = calls == total + 2;
            calls += 1;
            if at_ready { client = Some(recovery::stopping(&reviewer)); }
            assert!(!actor.socket.exists());
            assert!(!reviewer.socket(91).exists());
            assert!(!evaluation.peer.socket_path().exists());
            ElapsedTick(1001)
        },
    ).unwrap_err();
    assert!(error.starts_with("native admission Stop acknowledged at revision"), "{error}");
    assert!(client.expect("original Ready Stop selected").join().unwrap().acknowledged());
    let after = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(after.executions, original.before.executions);
    assert_eq!(after.control.ledger.charged, original.before.control.ledger.charged);
    assert_eq!(after.control.ledger.reserved, original.before.control.ledger.reserved);
    assert_eq!(after.revision, original.before.revision + 2);
    let retained = archive::load(history.output().unwrap()).unwrap();
    let saved = FileOversight::read_andon_guarded(
        &store, &profile, &required,
        &andon_settings::Settings::read(&original.accounting).unwrap().protocol, &retained,
    ).unwrap();
    assert_eq!(saved.credibility.report.policy_generation, 2);
    assert_eq!(saved.credibility.report.revision, 1);
    assert_eq!(saved.credibility.journal.stop.as_ref().unwrap().request().operation, 91);
    assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
    assert!(!evaluation.peer.socket_path().exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 91).exists());
}
