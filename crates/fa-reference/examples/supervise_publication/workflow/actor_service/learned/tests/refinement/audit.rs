//! Real original residual purchase, with the fresh native successor still pending.
use super::*;
use super::super::audit::{files, write_config};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
use fa_reference::strict_json::{self, Json, Limits};
use std::io;

struct Case {
    root: Root,
    config: Config,
    loaded: Loaded,
    run: FileNativeSupervisedDriver,
    wire: Wire,
    original: FileLearnedSidecarFinish,
    before_completion: FileHistoryAnchor,
    anchor: FileHistoryAnchor,
}

impl Case {
    fn new() -> Self {
        let root = Root::new();
        let (mut config, loaded, _reviewer, driver, wire, deadline) =
            prepared(&root, b"allow", 2);
        let mut run = review::start(driver, &mut config, &loaded, &deadline, &mut || {
            ElapsedTick(1000)
        })
        .unwrap();
        let before_completion = run.supervisor().host().unwrap().history_anchor().unwrap();
        through_coarse(&mut run, &mut config);
        let original = run.review().history()[0].clone();
        let anchor = run.supervisor().host().unwrap().history_anchor().unwrap();
        Self { root, config, loaded, run, wire, original, before_completion, anchor }
    }

    fn report(&self) -> Json {
        let mut output = Vec::new();
        audit_command::run(&self.config, &self.loaded, FIRST, &self.anchor,
            &mut output, &mut || Ok(())).unwrap();
        assert_refinement_report(&output, &self.original)
    }

    fn finish(mut self) {
        self.run.cancel(self.run.review().revision()).unwrap();
        let handoff = self.run.into_handoff().unwrap();
        drop(self.wire);
        assert_eq!(cleanup(handoff.driver, 1000, 1), 0);
        drop(handoff.review);
    }
}

pub(super) fn value(report: &Json) -> &Json {
    report.get("knowledge").unwrap().get("value").unwrap()
}

fn number(object: &Json, field: &str) -> u64 {
    object.get(field).unwrap().as_str().unwrap().parse().unwrap()
}

pub(super) fn assert_refinement_report(output: &[u8], original: &FileLearnedSidecarFinish) -> Json {
    let FileLearnedSidecarFinish::Refined { group, input_revision, archive } = original
        else { panic!("the actual original round must have purchased a residual"); };
    let report = strict_json::parse(output, Limits::default()).unwrap();
    assert_eq!(report.get("schema").unwrap().as_str(), Some("fa.refinement-audit/1"));
    assert_eq!(report.get("status").unwrap().as_str(), Some("complete"));
    assert_eq!(report.get("authority").unwrap().as_str(), Some("none"));
    assert_eq!(report.get("external_receipt").unwrap().as_str(), Some("not_verified"));
    assert_eq!(report.get("disclosure").unwrap().as_str(),
        Some("summary_only_no_payload_or_helper_input_bytes"));
    let knowledge = report.get("knowledge").unwrap();
    assert_eq!(knowledge.get("kind").unwrap().as_str(), Some("known"));
    let basis = knowledge.get("basis").unwrap();
    assert_eq!(basis.get("class").unwrap().as_str(), Some("bounded_model"));
    assert_eq!(basis.get("scope").unwrap().as_str(), Some("historical_reference_replay"));
    assert_eq!(basis.get("authentic_origin").unwrap().as_str(), Some("unknown"));
    assert_eq!(basis.get("source_truth").unwrap().as_str(), Some("not_established"));
    let value = value(&report);
    assert_eq!(number(value, "round"), FIRST);
    assert_eq!(number(value, "input_revision"), archive.input_revision);
    assert_eq!(number(value, "input_bytes"), archive.inputs.logical_bytes() as u64);
    assert_eq!(number(value, "commit_count"), archive.commit_times.len() as u64);
    assert_eq!(number(value, "reveal_count"), archive.reveal_times.len() as u64);
    assert_eq!(number(value, "started_at"), archive.started_at.0);
    assert_eq!(number(value, "completed_at"), archive.completed_at.0);
    let application = value.get("application").unwrap();
    assert_eq!(application.get("status").unwrap().as_str(), Some("not_attempted"));
    assert!(application.get("error").unwrap().is_null());
    assert_eq!(value.get("application_inputs").unwrap().as_str(), Some("not_applicable"));
    let refinement = value.get("refinement").unwrap();
    assert_eq!(number(refinement, "input_revision"), *input_revision);
    assert!(*input_revision > archive.input_revision);
    assert_eq!(refinement.get("requires_fresh_review").unwrap().as_bool(), Some(true));
    let selected = refinement.get("group").unwrap();
    assert_eq!(number(selected, "layer"), group.row.layer);
    assert_eq!(selected.get("side").unwrap().as_str(), Some(match group.row.side {
        KvSide::Key => "key", KvSide::Value => "value",
    }));
    assert_eq!(number(selected, "position"), group.row.position);
    assert_eq!(number(selected, "head"), group.head as u64);
    assert_eq!(value.get("policy_trace").unwrap().as_array().unwrap().len(),
        archive.policy.evaluation.trace().len());
    let members = value.get("members").unwrap().as_array().unwrap();
    assert_eq!(members.len(), archive.inputs.views().len());
    assert!(members.iter().all(|member| member.get("abstained").unwrap().as_bool() == Some(true)
        && member.get("missing").unwrap().as_bool() == Some(false)));
    assert!(!output.windows(b"payload_hex".len()).any(|part| part == b"payload_hex"));
    report
}

#[test]
fn coarse_review_cli_is_inspectable_while_fresh_native_review_is_pending() {
    let case = Case::new();
    let snapshot = case.run.supervisor().host().unwrap().inspect();
    let native = case.run.progress();
    let numerical = case.run.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical;
    let source_reads = case.config.source.status().read_attempts;
    let reservation = case.run.review().reservation();
    let config = write_config(&case.root);
    let anchor = case.root.0.join("independent-refinement.anchor");
    let mut bytes = Vec::new();
    case.anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    fs::write(&anchor, bytes).unwrap();
    for name in ["evidence.json", "native-roster.json", "helper-model.json",
        "helper-weights.safetensors", "helper-tokenizer.bbpe", "helper-sampling.json",
        "helper-monitor.json"] {
        fs::remove_file(case.root.0.join(name)).unwrap();
    }
    fs::remove_dir(case.root.0.join("peers")).unwrap();
    let before = files(&case.config.store);
    let args = vec![
        "create-learned-generated".into(),
        config.to_str().unwrap().into(),
        case.root.0.join("recipe.json").to_str().unwrap().into(),
        "--audit-review".into(),
        FIRST.to_string(),
        anchor.to_str().unwrap().into(),
    ];
    let mut output = Vec::new();
    audit_command::command(&args, &mut output).unwrap();
    let report = assert_refinement_report(&output, &case.original);
    let value = value(&report);
    let basis = report.get("knowledge").unwrap().get("basis").unwrap();
    assert_eq!(number(basis, "independent_history_through"), case.anchor.revision());
    assert_eq!(number(basis, "verified_journal_revision"), snapshot.revision);
    assert_eq!(number(basis, "unanchored_suffix_events"), 0);
    assert!(number(value, "begin_revision") < number(value, "completed_revision"));
    assert!(number(value, "completed_revision") < snapshot.revision);
    let effects = value.get("attempt_effects").unwrap();
    let stage = format!("{:?}", snapshot.control.ledger.stages[&1]);
    assert_eq!(effects.get("stage_at_review").unwrap().as_str(), Some(stage.as_str()));
    assert_eq!(effects.get("stage_at_journal_cut").unwrap().as_str(), Some(stage.as_str()));
    assert_eq!(effects.get("selected_review_dispatched").unwrap().as_bool(), Some(false));
    assert!(effects.get("dispatch").unwrap().is_null());
    assert_eq!(effects.get("terminal_outcome_recorded").unwrap().as_bool(), Some(false));
    assert_eq!(effects.get("reconciliation_recorded").unwrap().as_bool(), Some(false));
    assert_eq!(number(value.get("historical_effects").unwrap(), "executions_at_review"), 0);
    assert_eq!(number(value.get("historical_effects").unwrap(), "executions_at_journal_cut"), 0);
    assert_eq!(number(value.get("replay_work").unwrap(), "verified_events"), snapshot.revision);
    assert_eq!(number(value.get("replay_work").unwrap(), "attempted_events"), snapshot.revision);
    crate::command(args).unwrap();
    assert_eq!(files(&case.config.store), before);
    assert_eq!(case.run.supervisor().host().unwrap().inspect(), snapshot);
    assert_eq!(case.run.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(case.run.progress(), native);
    assert_eq!(case.run.review().reservation(), reservation);
    assert_eq!(case.config.source.status().read_attempts, source_reads);
    assert_eq!(case.run.review().records()[&SECOND].polls, 0);
    assert!(!case.root.0.join("peers").exists());
    case.finish();
}

struct FailedOutput(usize);
impl io::Write for FailedOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.0 += 1;
        Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("failed output cannot flush")
    }
}

#[test]
fn refinement_audit_cancellation_and_output_failure_preserve_pending_successor() {
    let case = Case::new();
    let before = files(&case.config.store);
    let snapshot = case.run.supervisor().host().unwrap().inspect();
    let native = case.run.progress();
    let events = snapshot.revision as usize;
    for fail in [0, 1, 5, events, events + 1, events + 2, events + 3] {
        let mut calls = 0;
        let mut output = Vec::new();
        let error = audit_command::run(&case.config, &case.loaded, FIRST, &case.anchor,
            &mut output, &mut || {
                let now = calls;
                calls += 1;
                if now == fail { Err("selected refinement audit deadline".into()) }
                else { Ok(()) }
            }).unwrap_err();
        assert_eq!(error, "selected refinement audit deadline");
        assert_eq!(calls, fail + 1);
        assert!(output.is_empty());
        assert_eq!(files(&case.config.store), before);
        assert_eq!(case.run.progress(), native);
    }
    let mut output = FailedOutput(0);
    let error = audit_command::run(&case.config, &case.loaded, FIRST, &case.anchor,
        &mut output, &mut || Ok(())).unwrap_err();
    assert!(error.contains("audit output failed"));
    assert_eq!(output.0, 1);
    assert_eq!(case.report().get("status").unwrap().as_str(), Some("complete"));
    assert_eq!(files(&case.config.store), before);
    assert_eq!(case.run.supervisor().host().unwrap().inspect(), snapshot);
    assert_eq!(case.run.progress(), native);
    case.finish();
}

#[test]
fn refinement_audit_rejects_short_anchor_changed_recipe_and_malformed_suffix() {
    let case = Case::new();
    let before = files(&case.config.store);
    let native = case.run.progress();
    for (round, anchor) in [(FIRST, &case.before_completion), (SECOND, &case.anchor)] {
        let mut output = Vec::new();
        assert_eq!(audit_command::run(&case.config, &case.loaded, round, anchor,
            &mut output, &mut || Ok(())).unwrap_err(), "Contract(Incomplete)");
        assert!(output.is_empty());
        assert_eq!(files(&case.config.store), before);
    }
    let path = case.root.0.join("recipe.json");
    let original = fs::read_to_string(&path).unwrap();
    let changed = original.replace("\"max_new_tokens\":2", "\"max_new_tokens\":3");
    assert_ne!(changed, original);
    fs::write(&path, changed).unwrap();
    let different = recipe::load(&path, &case.config, false).unwrap();
    let mut output = Vec::new();
    let error = audit_command::run(&case.config, &different, FIRST, &case.anchor,
        &mut output, &mut || Ok(())).unwrap_err();
    assert!(error.contains("Binding"));
    assert!(output.is_empty());
    assert_eq!(files(&case.config.store), before);
    fs::write(&path, original).unwrap();
    let canonical = case.config.store.join("delivery.bin");
    let original = fs::read(&canonical).unwrap();
    let mut malformed = original.clone();
    malformed.push(0xff);
    fs::write(&canonical, &malformed).unwrap();
    let changed = files(&case.config.store);
    let mut output = Vec::new();
    assert!(audit_command::run(&case.config, &case.loaded, FIRST, &case.anchor,
        &mut output, &mut || Ok(())).is_err());
    assert!(output.is_empty());
    assert_eq!(files(&case.config.store), changed);
    assert_eq!(case.run.progress(), native);
    // Restore only this test's deliberate corruption before retiring the owner.
    fs::write(canonical, original).unwrap();
    assert_eq!(case.report().get("schema").unwrap().as_str(), Some("fa.refinement-audit/1"));
    assert_eq!(files(&case.config.store), before);
    assert_eq!(case.run.progress(), native);
    case.finish();
}
