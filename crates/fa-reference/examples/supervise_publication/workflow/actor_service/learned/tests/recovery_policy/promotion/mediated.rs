//! Actual generated output, original topology-source custody, native review and
//! the independent human key precede the explicit governor command.
use super::*;
use super::super::super::{functional_creation, mediated_topology as topology_fixture};
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::{
    TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES,
};

struct MediatedCase {
    base: Case,
    saved: TopologyCheckpoint,
    early: TopologyCheckpoint,
    source_path: PathBuf,
    profile: PathBuf,
}

fn save(path: &Path, saved: &TopologyCheckpoint) {
    let mut bytes = Vec::new();
    saved.write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
    fs::write(path, bytes).unwrap();
}
fn source_file(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut fs::read(path).unwrap().as_slice(),
        MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}

impl MediatedCase {
    fn new(root: &Root, unknown: bool) -> Self {
        let (mut config, original, mut loaded) = topology_fixture::selected(root, None, false);
        let config_path = functional_creation::config_file(root, "audit_only");
        crate::config::assert_oversight_profiles_eq(&Config::read(&config_path).unwrap().profile, &config.profile);
        let campaigns_path = root.0.join("campaigns.json");
        fs::write(&campaigns_path, r#"{"schema":"fa.policy-campaign-profile/1","cases":16,"input_bytes":1048576,"max_campaigns":8}"#).unwrap();
        campaigns::admit(&config, &mut loaded, Some(&campaigns_path), false).unwrap();
        let limit = deadline();
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        let topology::Started { custody: (host, reviewer, _, _), source, .. } = topology::start(
            &config, &loaded, false, &mut control, &limit, &mut || ElapsedTick(1000)).unwrap();
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut wire = ActorWire::new(port);
        topology::attach(&mut driver, source).unwrap();
        topology::certify(&mut driver, &limit, &mut || ElapsedTick(1000)).unwrap();
        let early = {
            let host = driver.supervisor().host().unwrap();
            driver.topology().unwrap().checkpoint(&host, &fs::read(root.0.join("live-1.json")).unwrap(),
                &fs::read(root.0.join("inventory-1.json")).unwrap()).unwrap()
        };
        assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
            &limit, &mut || ElapsedTick(1000)).unwrap());
        submit(&mut driver, &mut wire, &mut config, &loaded, &limit, &mut || ElapsedTick(1000)).unwrap();
        let (mut driver, native_history) = native::review_current(&mut config, &loaded, &reviewer, driver, &limit);
        let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
        if unknown {
            assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
                || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            host.reconcile(revision, 1).unwrap();
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        } else {
            native::publish(&mut driver, &mut config, &human);
            assert!(executed(&mut wire, loaded.request));
        }
        let host = driver.supervisor().host().unwrap();
        let before = host.inspect();
        assert_eq!(before.executions, u64::from(!unknown));
        assert!(before.control.ledger.charged > 0);
        let saved = driver.topology().unwrap().checkpoint(&host,
            &fs::read(root.0.join("live-1.json")).unwrap(),
            &fs::read(root.0.join("inventory-1.json")).unwrap()).unwrap();
        let anchor = root.0.join("reviewed.anchor");
        anchors::export_acknowledged(saved.history_anchor(), &anchor).unwrap();
        let source_path = root.0.join("reviewed.source");
        save(&source_path, &saved);
        drop(host); drop(native_history); drop(wire); drop(driver);
        let selected = root.0.join("governed-recipe.json");
        let candidate = root.0.join("candidate.json");
        fs::write(&candidate, format!(r#"{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":1,"limits":{{"cases":16,"input_bytes":1048576,"output_bytes":1048576}},"candidate":{}}}"#,
            policy_json(2, 256))).unwrap();
        let reviewed = root.0.join("reviewed-policy.json");
        let request = root.0.join("governor-request.json");
        fs::write(&request, format!(r#"{{"schema":"fa.learned-policy-promotion/1","operation":"901","expected_journal_revision":"{}","expected_control_sequence":"{}","expected_authority_epoch":"{}","candidate_request":"{}","reviewed_report":"{}","accept_newly_reviewable":false}}"#,
            before.revision, before.control.sequence, before.control.ledger.epoch,
            candidate.display(), reviewed.display())).unwrap();
        let base = Case { config, config_path, original, selected, request, candidate, reviewed, anchor, before };
        base.select_policy(1, 512);
        let profile = Self::profile(root, &saved, &source_path, "promotion-topology.json");
        let mut replay = vec!["create-learned-generated".into(), base.config_path.display().to_string(),
            base.selected.display().to_string(), "--resume".into(), "--policy-replay".into(),
            base.candidate.display().to_string(), "--history-anchor".into(), base.anchor.display().to_string()];
        replay.extend(["--topology-profile".into(), profile.display().to_string()]);
        let canonical = base.canonical();
        let mut bytes = Vec::new();
        recovery::policy_replay::command_with_elapsed(&replay, None, &mut bytes, &mut || Duration::ZERO).unwrap();
        fs::write(&base.reviewed, bytes).unwrap();
        assert_eq!(base.canonical(), canonical);
        // Maintenance consumes independently retained data, not today's source
        // files or helper model assets. The missing next files were never read.
        for name in ["live-1.json", "inventory-1.json", "evidence.json", "native-roster.json",
            "helper-model.json", "helper-weights.safetensors", "helper-monitor.json",
            "helper-tokenizer.bbpe", "helper-sampling.json"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        fs::write(base.config.store.join("delivery.pending"), b"preserve unapproved promotion").unwrap();
        Self { base, saved, early, source_path, profile }
    }

    fn profile(root: &Root, saved: &TopologyCheckpoint, source: &Path, name: &str) -> PathBuf {
        let path = root.0.join(name);
        fs::write(&path, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{{"topology_path":"{}","inventory_path":"{}"}},"next":{{"topology_path":"{}","inventory_path":"{}"}},"operation":{},"source_checkpoint":"{}"}}"#,
            root.0.join("initial.json").display(), root.0.join("initial-inventory.json").display(),
            root.0.join("missing-next.json").display(), root.0.join("missing-next-inventory.json").display(),
            saved.last_operation() + 1, source.display())).unwrap();
        path
    }

    fn args(&self, history: &Path, source: &Path) -> Vec<String> {
        let mut args = self.base.args(history);
        args.extend(["--topology-profile".into(), self.profile.display().to_string(),
            "--retain-topology-checkpoint".into(), source.display().to_string()]);
        args
    }

    fn pair(&self, history: &Path, source: &Path) -> TopologyCheckpoint {
        let saved = source_file(source);
        assert_eq!(saved.history_anchor(), &anchors::load(history).unwrap());
        assert_eq!(saved.history_anchor().revision(), self.base.before.revision + 3);
        assert_eq!(saved.expected(), self.saved.expected());
        assert_eq!(saved.topology_bytes(), self.saved.topology_bytes());
        assert_eq!(saved.inventory_bytes(), self.saved.inventory_bytes());
        assert_eq!(saved.work(), self.saved.work());
        assert_eq!(saved.last_operation(), self.saved.last_operation());
        assert_eq!(saved.available(), self.saved.available());
        assert!(saved.minimum().control_sequence > self.saved.minimum().control_sequence);
        assert!(saved.minimum().authority_epoch > self.saved.minimum().authority_epoch);
        saved
    }
}

#[test]
fn complete_mediated_command_promotes_actual_review_and_retries_without_source_or_effect_work() {
    for unknown in [false, true] {
        let root = Root::new(); let case = MediatedCase::new(&root, unknown);
        let history = root.0.join("promoted.anchor"); let source = root.0.join("promoted.source");
        let old_source = fs::read(&case.source_path).unwrap();
        let old_anchor = fs::read(&case.base.anchor).unwrap();
        let result = json(&invoke(&case.args(&history, &source)));
        assert_eq!(text(&result, "schema"), "fa.learned-policy-promotion-result/2");
        assert_eq!(text(&result, "outcome"), "promoted");
        assert_eq!(text(&result, "appended_events"), "3");
        assert_eq!(text(&result, "charged"), case.base.before.control.ledger.charged.to_string());
        assert_eq!(text(&result, "executions"), u64::from(!unknown).to_string());
        assert_eq!(result.get("topology_checkpoint_retained").unwrap().as_bool(), Some(true));
        assert_eq!(text(&result, "topology_source_checks"), case.saved.work().checks.to_string());
        let paired = case.pair(&history, &source);
        assert_eq!(fs::read(&case.source_path).unwrap(), old_source);
        assert_eq!(fs::read(&case.base.anchor).unwrap(), old_anchor);
        let after = case.base.canonical();
        let retry_history = root.0.join("retry.anchor"); let retry_source = root.0.join("retry.source");
        assert!(refused(&case.base, &case.args(&retry_history, &retry_source)).contains("Binding"));
        case.base.select_policy(2, 256);
        let retry = json(&invoke(&case.args(&retry_history, &retry_source)));
        assert_eq!(text(&retry, "outcome"), "already_promoted");
        assert_eq!(text(&retry, "appended_events"), "0");
        assert_eq!(case.pair(&retry_history, &retry_source), paired);
        assert_eq!(case.base.canonical(), after);
        // The new paired cut supports ordinary read-only recovery; old source
        // custody remains usable only by the operation-specific retry.
        let profile = MediatedCase::profile(&root, &paired, &retry_source, "current-topology.json");
        let mut loaded = recipe::load(&case.base.selected, &case.base.config, false).unwrap();
        loaded.recovery_anchor = Some(paired.history_anchor().clone());
        topology::admit(&case.base.config, &mut loaded, Some(&profile), topology::Mode::Historical).unwrap();
        let response = recovery::readonly::read(&case.base.config, &loaded, None, &mut || Duration::ZERO).unwrap();
        if unknown { assert!(matches!(response.result, Ok(Knowledge::Unknown { .. }))); }
        else { assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))); }
        assert_eq!(case.base.canonical(), after);
    }
}

#[test]
fn both_exports_are_attempted_after_acknowledgment_even_when_one_fails_or_the_deadline_expires() {
    for (history_fails, source_fails) in [(true, false), (false, true), (true, true)] {
        let root = Root::new(); let case = MediatedCase::new(&root, false);
        let history = root.0.join("raced.anchor"); let source = root.0.join("raced.source");
        let before = case.base.canonical(); let mut raced = false; let mut bytes = Vec::new();
        let error = operator::command_with_elapsed(&case.args(&history, &source), None, &mut bytes, &mut || {
            if !raced && case.base.canonical() != before {
                if history_fails { fs::write(&history, b"history winner").unwrap(); }
                if source_fails { fs::write(&source, b"source winner").unwrap(); }
                raced = true;
            }
            if raced { Duration::from_secs(3600) } else { Duration::ZERO }
        }).unwrap_err();
        assert!(error.contains("acknowledged") && error.contains("deadline reached after acknowledgment"));
        let report = json(&bytes);
        assert_eq!(text(&report, "outcome"), "promoted");
        assert_eq!(report.get("history_anchor_retained").unwrap().as_bool(), Some(!history_fails));
        assert_eq!(report.get("topology_checkpoint_retained").unwrap().as_bool(), Some(!source_fails));
        if history_fails { assert_eq!(fs::read(&history).unwrap(), b"history winner"); }
        else { assert_eq!(anchors::load(&history).unwrap().revision(), case.base.before.revision + 3); }
        if source_fails { assert_eq!(fs::read(&source).unwrap(), b"source winner"); }
        else { assert_eq!(source_file(&source).history_anchor().revision(), case.base.before.revision + 3); }
        case.base.select_policy(2, 256);
        let after = case.base.canonical();
        let retry_history = root.0.join("retained.anchor"); let retry_source = root.0.join("retained.source");
        assert_eq!(text(&json(&invoke(&case.args(&retry_history, &retry_source))), "appended_events"), "0");
        case.pair(&retry_history, &retry_source);
        assert_eq!(case.base.canonical(), after);
    }
}

#[test]
fn malformed_unpaired_manual_or_stale_source_custody_refuses_before_any_policy_change() {
    let root = Root::new(); let case = MediatedCase::new(&root, false);
    let history = root.0.join("promoted.anchor"); let source = root.0.join("promoted.source");
    let base = case.args(&history, &source);
    for args in [
        base[..base.len() - 2].to_vec(),
        { let mut v = case.base.args(&history); v.extend(["--retain-topology-checkpoint".into(), source.display().to_string()]); v },
        { let mut v = base.clone(); v.extend(["--topology-profile".into(), case.profile.display().to_string()]); v },
        { let mut v = base.clone(); *v.last_mut().unwrap() = history.display().to_string(); v },
        { let mut v = base.clone(); v.push("--recover-credential-checkpoint".into()); v },
    ] {
        refused(&case.base, &args);
        assert!(!history.exists() && !source.exists());
    }
    let original = fs::read(&case.source_path).unwrap();
    let mut changed = original.clone(); changed[72] ^= 1;
    fs::write(&case.source_path, changed).unwrap();
    refused(&case.base, &base);
    fs::write(&case.source_path, &original).unwrap();
    let pair = format!(r#"{{"topology_path":"{}","inventory_path":"{}"}}"#,
        root.0.join("initial.json").display(), root.0.join("initial-inventory.json").display());
    let manual = root.0.join("manual-topology.json");
    fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1","initial":{pair},"current":{pair},"next":{pair},"available":true,"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
        case.saved.last_operation() + 1, case.saved.work().checks,
        case.saved.work().reserved_read_bytes, case.saved.last_operation())).unwrap();
    let mut args = base.clone(); args[10] = manual.display().to_string();
    assert!(refused(&case.base, &args).contains("exact v2"));
    // A matched earlier pair is still incomplete if ordinary numerical/source
    // work intervened. Only the exact campaign triple can explain a lost export.
    let early_source = root.0.join("early.source"); save(&early_source, &case.early);
    let early_anchor = root.0.join("early.anchor");
    anchors::export_acknowledged(case.early.history_anchor(), &early_anchor).unwrap();
    let early_profile = MediatedCase::profile(&root, &case.early, &early_source, "early-topology.json");
    let mut args = base.clone();
    args[6] = early_anchor.display().to_string(); args[10] = early_profile.display().to_string();
    refused(&case.base, &args);
    assert_eq!(fs::read(case.base.config.store.join("delivery.pending")).unwrap(), b"preserve unapproved promotion");
    assert_eq!(text(&json(&invoke(&base)), "outcome"), "promoted");
    case.pair(&history, &source);
}

#[test]
fn mediated_governor_checks_the_complete_report_and_explicit_current_policy_without_fallback() {
    let root = Root::new(); let case = MediatedCase::new(&root, false);
    let history = root.0.join("promoted.anchor"); let source = root.0.join("promoted.source");
    let args = case.args(&history, &source);
    let report = fs::read(&case.base.reviewed).unwrap();
    fs::write(&case.base.reviewed, &report[..report.len() - 1]).unwrap();
    assert!(refused(&case.base, &args).contains("report"));
    fs::write(&case.base.reviewed, report).unwrap();
    let original = fs::read(&case.base.original).unwrap();
    for owned in [false, true] {
        super::super::super::forecast::install(&root, &case.base.original, owned, 60_000);
        case.base.select_policy(1, 512);
        assert!(refused(&case.base, &args).contains("nonpredictive"));
        fs::write(&case.base.original, &original).unwrap();
    }
    case.base.select_policy(1, 512);
    assert_eq!(text(&json(&invoke(&args)), "outcome"), "promoted");
    case.pair(&history, &source);
}

#[test]
fn failed_result_transport_does_not_prevent_either_independent_archive() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::other("selected mediated report failure")) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let root = Root::new(); let case = MediatedCase::new(&root, false);
    let history = root.0.join("retained.anchor"); let source = root.0.join("retained.source");
    let error = operator::command_with_elapsed(&case.args(&history, &source), None, &mut Broken,
        &mut || Duration::ZERO).unwrap_err();
    assert!(error.contains("acknowledged") && error.contains("selected mediated report failure"));
    case.pair(&history, &source);
    case.base.select_policy(2, 256);
    let retry_history = root.0.join("retry.anchor"); let retry_source = root.0.join("retry.source");
    assert_eq!(text(&json(&invoke(&case.args(&retry_history, &retry_source))), "outcome"), "already_promoted");
    case.pair(&retry_history, &retry_source);
}
