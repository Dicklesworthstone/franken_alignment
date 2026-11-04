//! End-to-end trusted maintenance over a real learned publication and the
//! original read-only report. Descriptive replay never performs the promotion.
use super::*;
use super::super::super::{campaigns, policy_promotion as operator};
use fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedPolicyPromotionRequest, FileLearnedRecoveryStatus,
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{cell::Cell, io::{self, Write}};

const LIMITS: ReplayLimits = ReplayLimits { cases: 16, input_bytes: 1_048_576 };

struct Case {
    config: Config,
    config_path: PathBuf,
    original: PathBuf,
    selected: PathBuf,
    request: PathBuf,
    candidate: PathBuf,
    reviewed: PathBuf,
    anchor: PathBuf,
    before: FileDeliverySnapshot,
}
impl Case {
    fn new(root: &Root) -> Self {
        let config_path = super::super::functional_creation::config_file(root, "functional_restart");
        let mut config = Config::read_learned_generation(&config_path).unwrap();
        let original = native::write(root);
        evidence(root, &config, 1, true, false);
        let mut loaded = recipe::load(&original, &config, true).unwrap();
        let profile = root.0.join("create-campaigns.json");
        fs::write(&profile, r#"{"schema":"fa.policy-campaign-profile/1","cases":16,"input_bytes":1048576,"max_campaigns":8}"#).unwrap();
        campaigns::admit(&config, &mut loaded, Some(&profile), false).unwrap();
        let (host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut wire = ActorWire::new(port);
        let limit = deadline();
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
            &limit, &mut || ElapsedTick(1000)).unwrap());
        submit(&mut driver, &mut wire, &mut config, &loaded, &limit, &mut || ElapsedTick(1000)).unwrap();
        let (mut driver, history) = native::review_current(&mut config, &loaded, &reviewer, driver, &limit);
        let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
        native::publish(&mut driver, &mut config, &human);
        assert!(executed(&mut wire, loaded.request));
        let host = driver.supervisor().host().unwrap();
        let before = host.inspect();
        assert_eq!(before.executions, 1);
        let anchor = root.0.join("reviewed.anchor");
        anchors::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
        drop(host); drop(history); drop(wire); drop(driver);
        let selected = root.0.join("governed-recipe.json");
        let candidate = root.0.join("candidate.json");
        fs::write(&candidate, format!(r#"{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":1,"limits":{{"cases":16,"input_bytes":1048576,"output_bytes":1048576}},"candidate":{}}}"#,
            policy_json(2, 256))).unwrap();
        let reviewed = root.0.join("reviewed-policy.json");
        let request = root.0.join("governor-request.json");
        fs::write(&request, format!(r#"{{"schema":"fa.learned-policy-promotion/1","operation":"901","expected_journal_revision":"{}","expected_control_sequence":"{}","expected_authority_epoch":"{}","candidate_request":"{}","reviewed_report":"{}","accept_newly_reviewable":false}}"#,
            before.revision, before.control.sequence, before.control.ledger.epoch,
            candidate.display(), reviewed.display())).unwrap();
        let case = Self { config, config_path, original, selected, request, candidate, reviewed,
            anchor, before };
        case.select_policy(1, 512);
        case.retain_report();
        case
    }
    fn select_policy(&self, generation: u64, maximum: usize) {
        fs::write(&self.selected, format!(r#"{{"schema":"fa.learned-recovery-policy/2","effective_policy":{},"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":8}},"recipe":{}}}"#,
            policy_json(generation, maximum), fs::read_to_string(&self.original).unwrap())).unwrap();
    }
    fn retain_report(&self) {
        let args = vec!["create-learned-generated".into(), self.config_path.display().to_string(),
            self.selected.display().to_string(), "--resume".into(), "--policy-replay".into(),
            self.candidate.display().to_string(), "--history-anchor".into(), self.anchor.display().to_string()];
        let before = self.canonical();
        let mut bytes = Vec::new();
        recovery::policy_replay::command_with_elapsed(&args, None, &mut bytes, &mut || Duration::ZERO).unwrap();
        assert_eq!(self.canonical(), before, "replay remains descriptive and read-only");
        fs::write(&self.reviewed, bytes).unwrap();
    }
    fn canonical(&self) -> Vec<u8> { fs::read(self.config.store.join("delivery.bin")).unwrap() }
    fn args(&self, output: &Path) -> Vec<String> {
        vec!["create-learned-generated".into(), self.config_path.display().to_string(),
            self.selected.display().to_string(), "--promote-policy".into(), self.request.display().to_string(),
            "--history-anchor".into(), self.anchor.display().to_string(),
            "--retain-history-anchor".into(), output.display().to_string()]
    }
}
fn json(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn text<'a>(value: &'a Json, key: &str) -> &'a str { value.get(key).unwrap().as_str().unwrap() }
fn invoke(args: &[String]) -> Vec<u8> {
    let mut output = Vec::new();
    operator::command_with_elapsed(args, None, &mut output, &mut || Duration::ZERO).unwrap();
    output
}
fn refused(case: &Case, args: &[String]) -> String {
    let before = case.canonical(); let mut output = Vec::new();
    let error = operator::command_with_elapsed(args, None, &mut output, &mut || Duration::ZERO).unwrap_err();
    assert!(output.is_empty(), "{error}");
    assert_eq!(case.canonical(), before);
    error
}

#[test]
fn original_read_only_report_promotes_once_and_repairs_lost_export_with_current_policy_selection() {
    let root = Root::new(); let case = Case::new(&root);
    let old_anchor = fs::read(&case.anchor).unwrap();
    let reviewed = fs::read(&case.reviewed).unwrap();
    let report = json(&reviewed);
    let value = report.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(value.get("case_count").unwrap().as_u64(), Some(2));
    assert_eq!(value.get("policy_changed").unwrap().as_bool(), Some(false));
    // Actual maintenance has no dependency on today's source/helper evidence.
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let output = root.0.join("promoted.anchor");
    let result = json(&invoke(&case.args(&output)));
    assert_eq!(text(&result, "outcome"), "promoted");
    assert_eq!(text(&result, "appended_events"), "3");
    assert_eq!(text(&result, "current_policy_generation"), "2");
    assert_eq!(text(&result, "charged"), case.before.control.ledger.charged.to_string());
    assert_eq!(text(&result, "executions"), "1");
    assert_eq!(fs::read(&case.anchor).unwrap(), old_anchor);
    assert_eq!(fs::read(&case.reviewed).unwrap(), reviewed);
    assert!(output.is_file());
    let after = case.canonical();
    let repaired = root.0.join("repaired.anchor");
    let error = refused(&case, &case.args(&repaired));
    assert!(error.contains("Binding"), "old expected policy cannot relabel the current owner: {error}");
    assert!(!repaired.exists());
    case.select_policy(2, 256);
    fs::remove_file(&output).unwrap();
    let retried = json(&invoke(&case.args(&repaired)));
    assert_eq!(text(&retried, "outcome"), "already_promoted");
    assert_eq!(text(&retried, "appended_events"), "0");
    assert_eq!(text(&retried, "reviewed_revision"), case.before.revision.to_string());
    assert_eq!(case.canonical(), after);
    assert!(repaired.is_file());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    let mut loaded = recipe::load(&case.selected, &case.config, false).unwrap();
    loaded.recovery_anchor = Some(anchors::load(&repaired).unwrap());
    let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
    assert_eq!(host.current_policy().unwrap(), &policy(2, 256));
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, case.before.control.ledger.charged);
}

#[test]
fn any_report_or_original_cut_mismatch_refuses_before_staging_cleanup_and_policy_mutation() {
    let root = Root::new(); let case = Case::new(&root);
    let report = fs::read_to_string(&case.reviewed).unwrap();
    let request = fs::read_to_string(&case.request).unwrap();
    let pending = case.config.store.join("delivery.pending");
    fs::write(&pending, b"retain unapproved campaign evidence").unwrap();
    let output = root.0.join("promoted.anchor");
    for changed in [
        report[..report.len() - 1].to_owned(),
        report.replace("\"case_count\":2", "\"case_count\":1"),
        report.replace("\"authority\":\"not_issued\"", "\"authority\":\"approved\""),
        report.replace("\"case_count\":2", "\"case_count\":2,\"skip_review\":true"),
    ] {
        assert_ne!(changed, report);
        fs::write(&case.reviewed, changed).unwrap();
        assert!(refused(&case, &case.args(&output)).contains("report"));
        assert_eq!(fs::read(&pending).unwrap(), b"retain unapproved campaign evidence");
        assert!(!output.exists());
    }
    fs::write(&case.reviewed, &report).unwrap();
    for (name, value) in [("expected_journal_revision", case.before.revision),
        ("expected_control_sequence", case.before.control.sequence),
        ("expected_authority_epoch", case.before.control.ledger.epoch)] {
        fs::write(&case.request, request.replace(&format!("\"{name}\":\"{value}\""),
            &format!("\"{name}\":\"{}\"", value + 1))).unwrap();
        refused(&case, &case.args(&output));
        assert_eq!(fs::read(&pending).unwrap(), b"retain unapproved campaign evidence");
    }
    fs::write(&case.request, request).unwrap();
    let accepted = json(&invoke(&case.args(&output)));
    assert_eq!(text(&accepted, "outcome"), "promoted");
    assert!(!pending.exists());
}

#[test]
fn ready_and_post_comparison_deadlines_leave_no_campaign_but_late_acknowledgment_is_retained() {
    let root = Root::new(); let case = Case::new(&root);
    let output = root.0.join("promoted.anchor");
    let args = case.args(&output);
    let mut loaded = recipe::load(&case.selected, &case.config, false).unwrap();
    let (_, custody) = anchors::take_options(&args).unwrap();
    custody.admit(&case.config, &mut loaded, true).unwrap();
    let request = operator::decode(&fs::read(&case.request).unwrap()).unwrap();
    let selected = FileLearnedPolicyPromotionRequest::new(case.before.revision, PolicyUpdate::new(901,
        case.before.control.sequence, case.before.control.ledger.epoch, policy(2, 256)).unwrap()).unwrap();
    let before = case.canonical();
    let stopped = recovery::policy_promotion::ready(&case.config, &loaded, selected,
        &mut |progress| if progress.is_some_and(|p| p.status == FileLearnedRecoveryStatus::Ready) {
            Err("selected Ready deadline".into())
        } else { Ok(()) });
    assert!(stopped.err().unwrap().contains("Ready deadline"));
    assert_eq!(case.canonical(), before);
    let calls = Cell::new(0_u64);
    let candidate = crate::policy_replay::Request::read(&case.candidate).unwrap();
    let mut bytes = Vec::new();
    // run() has initial admission, pre-open, each original event/Ready, then
    // its distinct comparison checkpoint. Expire at that final prewrite cut.
    let cutoff = case.before.revision + 4;
    let result = operator::run(&case.config, &loaded, &request, candidate,
        &fs::read(&case.reviewed).unwrap(), &mut bytes, &mut || {
            calls.set(calls.get() + 1);
            if calls.get() >= cutoff { Duration::from_millis(case.config.timing.runtime_ms) }
            else { Duration::ZERO }
        });
    assert!(result.unwrap_err().contains("deadline"));
    assert_eq!(calls.get(), cutoff);
    assert!(bytes.is_empty());
    assert_eq!(case.canonical(), before);
    assert!(!output.exists());
    let mut bytes = Vec::new();
    let error = operator::command_with_elapsed(&args, None, &mut bytes, &mut || {
        if case.canonical() == before { Duration::ZERO }
        else { Duration::from_millis(case.config.timing.runtime_ms) }
    }).unwrap_err();
    assert!(error.contains("acknowledged"));
    assert_eq!(text(&json(&bytes), "outcome"), "promoted");
    assert_eq!(json(&bytes).get("deadline_reached").unwrap().as_bool(), Some(true));
    assert!(output.is_file());
    assert_ne!(case.canonical(), before);
}

#[test]
fn output_failure_does_not_undo_promotion_or_prevent_independent_anchor_retention() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::other("selected report sink failure")) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let root = Root::new(); let case = Case::new(&root);
    let output = root.0.join("retained-despite-output.anchor");
    let before = case.canonical();
    let error = operator::command_with_elapsed(&case.args(&output), None, &mut Broken,
        &mut || Duration::ZERO).unwrap_err();
    assert!(error.contains("acknowledged") && error.contains("selected report sink failure"));
    assert!(output.is_file());
    assert_ne!(case.canonical(), before);
    case.select_policy(2, 256);
    let repaired = root.0.join("repair-report.anchor");
    assert_eq!(text(&json(&invoke(&case.args(&repaired))), "outcome"), "already_promoted");
}

#[test]
fn promotion_intent_is_closed_and_replay_limits_cannot_silently_select_a_smaller_corpus() {
    let root = Root::new(); let case = Case::new(&root);
    let request = fs::read_to_string(&case.request).unwrap();
    for invalid in [
        request.replace("\"operation\":\"901\"", "\"operation\":\"0\""),
        request.replace("\"operation\":\"901\"", "\"operation\":\"0901\""),
        request.replace("\"operation\":\"901\"", "\"operation\":901"),
        request.replace("\"accept_newly_reviewable\":false", "\"accept_newly_reviewable\":\"false\""),
        request.replace("\"accept_newly_reviewable\":false", "\"accept_newly_reviewable\":false,\"approve_all\":true"),
    ] { assert!(operator::decode(invalid.as_bytes()).is_err()); }
    let output = root.0.join("limited.anchor");
    let original = fs::read_to_string(&case.candidate).unwrap();
    fs::write(&case.candidate, original.replace("\"cases\":16", "\"cases\":1")).unwrap();
    assert!(refused(&case, &case.args(&output)).contains("limits"));
    fs::write(&case.candidate, original).unwrap();
    let base = case.args(&output);
    for flag in ["--resume", "--policy-replay", "--actor-profile", "--after-message",
        "--topology-profile", "--precheck-profile", "--campaign-profile", "--recover-stop"] {
        let mut args = base.clone(); args.push(flag.into());
        assert!(refused(&case, &args).contains("--promote-policy"));
    }
    assert_eq!(LIMITS, recipe::load(&case.selected, &case.config, false).unwrap()
        .expected_campaigns.unwrap().limits);
    assert_eq!(text(&json(&invoke(&base)), "outcome"), "promoted");
}
