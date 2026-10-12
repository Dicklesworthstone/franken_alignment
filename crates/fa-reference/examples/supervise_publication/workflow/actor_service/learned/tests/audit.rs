//! Original loaded generation, native helpers and two-key publication, not canned audits.
use super::*;
use super::super::audit as audit_command;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    replay::FileReviewReplay,
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, io, path::PathBuf};

struct Case {
    root: Root, config: Config, loaded: Loaded, driver: FileSupervisedDriver,
    wire: Wire, expected: FileReviewReplay, anchor: FileHistoryAnchor,
}
fn recorded(mode: Option<bool>, spelling: &[u8], publish: bool) -> Case {
    let root = Root::new(); let mut config = configured(&root);
    let path = fixture::write(&root, spelling);
    if let Some(owned) = mode { super::forecast::install(&root, &path, owned, 60_000); }
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer, observer) = recovery::create_selected(&config, &loaded).unwrap();
    let mut prediction = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
    let deadline = deadline(); let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000), prediction.as_mut()).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
        &deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let mut driver = reviewed.driver;
    let native = reviewed.history.unwrap(); assert_native(&native);
    if publish {
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { .. }));
        let request = driver.request_learned_human_approval_from_policy_file(&mut config.source,
            701, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
        let human = {
            let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
            reviewer.approve(&mut host, revision, &request).unwrap()
        };
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::Reconciled { .. }));
        assert!(executed(&mut wire, loaded.request));
    }
    let host = driver.supervisor().host().unwrap();
    let expected = host.review_replay(loaded.rounds[0]).unwrap();
    let anchor = host.history_anchor().unwrap();
    assert_eq!(expected.journal_snapshot(), &host.inspect());
    drop(host); drop(native);
    Case { root, config, loaded, driver, wire, expected, anchor }
}
fn files(store: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(store).unwrap().map(|entry| {
        let entry = entry.unwrap(); assert!(entry.file_type().unwrap().is_file());
        (entry.file_name().into_string().unwrap(), fs::read(entry.path()).unwrap())
    }).collect()
}
fn report(case: &Case) -> Json {
    let mut output = Vec::new();
    audit_command::run(&case.config, &case.loaded, case.loaded.rounds[0], &case.anchor,
        &mut output, &mut || Ok(())).unwrap();
    strict_json::parse(&output, Limits::default()).unwrap()
}
fn value(report: &Json) -> &Json { report.get("knowledge").unwrap().get("value").unwrap() }
fn anchor_file(case: &Case) -> PathBuf {
    let mut bytes = Vec::new(); case.anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    let path = case.root.0.join("independent-review.anchor"); fs::write(&path, bytes).unwrap(); path
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
fn actual_cli_audits_all_original_modes_after_live_helper_and_evidence_removal() {
    for mode in [None, Some(false), Some(true)] {
        let case = recorded(mode, b"allow", true);
        let anchor = anchor_file(&case); let config = write_config(&case.root);
        assert_eq!(Config::read(&config).unwrap().profile, case.config.profile);
        for name in ["evidence.json", "native-roster.json", "helper-model.json",
            "helper-weights.safetensors", "helper-tokenizer.bbpe", "helper-sampling.json", "helper-monitor.json"] {
            fs::remove_file(case.root.0.join(name)).unwrap();
        }
        fs::remove_dir(case.root.0.join("peers")).unwrap();
        fs::write(case.config.store.join("delivery.pending"), b"retain audit staging").unwrap();
        let before = files(&case.config.store);
        let args = vec!["create-learned-generated".into(), config.to_str().unwrap().into(),
            case.root.0.join("recipe.json").to_str().unwrap().into(), "--audit-review".into(),
            case.loaded.rounds[0].to_string(), anchor.to_str().unwrap().into()];
        let mut output = Vec::new(); audit_command::command(&args, &mut output).unwrap();
        let decoded = strict_json::parse(&output, Limits::default()).unwrap();
        assert_eq!(decoded.get("authority").unwrap().as_str(), Some("none"));
        assert_eq!(value(&decoded).get("consequence").unwrap().as_str(), Some("Continue"));
        assert_eq!(value(&decoded).get("application").unwrap().get("status").unwrap().as_str(), Some("applied"));
        assert_eq!(value(&decoded).get("historical_effects").unwrap().get("executions_at_journal_cut").unwrap().as_str(), Some("1"));
        assert_eq!(value(&decoded).get("completed_revision").unwrap().as_str(),
            Some(case.expected.completed_snapshot().revision.to_string().as_str()));
        assert_eq!(value(&decoded).get("policy_trace").unwrap().as_array().unwrap().len(),
            case.expected.archive().policy.evaluation.trace().len());
        assert_eq!(value(&decoded).get("members").unwrap().as_array().unwrap().len(), 2);
        assert!(!output.windows(b"payload_hex".len()).any(|part| part == b"payload_hex"));
        crate::command(args).unwrap(); // The REAL top-level route, not only a helper.
        assert_eq!(files(&case.config.store), before);
        assert!(!case.root.0.join("peers").exists());
        assert_eq!(case.driver.supervisor().host().unwrap().inspect(), *case.expected.journal_snapshot());
        drop(case.wire); assert_eq!(cleanup(case.driver, 1000, 1), 0);
    }
}

#[test]
fn native_denial_is_a_successful_historical_audit_not_an_execution_or_permit() {
    let case = recorded(None, b"deny", false);
    let before = files(&case.config.store); let decoded = report(&case);
    assert_ne!(value(&decoded).get("consequence").unwrap().as_str(), Some("Continue"));
    assert_eq!(decoded.get("status").unwrap().as_str(), Some("complete"));
    assert_eq!(value(&decoded).get("historical_effects").unwrap().get("executions_at_journal_cut").unwrap().as_str(), Some("0"));
    assert_eq!(files(&case.config.store), before);
    drop(case.wire); assert_eq!(cleanup(case.driver, 1000, 1), 0);
}

#[test]
fn cancellation_before_read_during_replay_at_ready_and_before_output_emits_nothing() {
    let case = recorded(None, b"allow", false);
    let before = files(&case.config.store); let events = case.expected.journal_snapshot().revision as usize;
    // One pre-read checkpoint, one per original event, Ready, verification and rendering.
    for fail in [0, 1, 5, events, events + 1, events + 2, events + 3] {
        let mut calls = 0; let mut output = Vec::new();
        let error = audit_command::run(&case.config, &case.loaded, case.loaded.rounds[0],
            &case.anchor, &mut output, &mut || {
                let now = calls; calls += 1;
                if now == fail { Err("selected audit deadline".into()) } else { Ok(()) }
            }).unwrap_err();
        assert_eq!(error, "selected audit deadline"); assert_eq!(calls, fail + 1);
        assert!(output.is_empty()); assert_eq!(files(&case.config.store), before);
    }
    assert_eq!(report(&case).get("status").unwrap().as_str(), Some("complete"));
    drop(case.wire); assert_eq!(cleanup(case.driver, 1000, 1), 0);
}

#[test]
fn malformed_modes_and_nonindependent_anchor_paths_have_no_weaker_fallback() {
    let root = Root::new(); let absent = root.0.join("absent").to_str().unwrap().to_owned();
    let valid = vec!["create-learned-generated".into(), absent.clone(), absent.clone(),
        "--audit-review".into(), "1".into(), absent.clone()];
    for args in [valid[..5].to_vec(), [valid.clone(), vec!["--resume".into()]].concat(),
        [valid.clone(), vec!["--retain-history-anchor".into(), absent.clone()]].concat()] {
        let mut output = Vec::new(); let error = audit_command::command(&args, &mut output).unwrap_err();
        assert!(error.starts_with("create-learned-generated CONFIG")); assert!(output.is_empty());
    }
    assert_eq!(command(&valid, Some(Path::new(&absent))).unwrap_err(), "historical audit cannot activate credibility");
    assert!(!root.0.join("store").exists());
    let case = recorded(None, b"allow", false); let outside = anchor_file(&case);
    let inside = case.config.store.join("copied.anchor"); fs::copy(&outside, &inside).unwrap();
    assert!(audit_command::load_anchor(&case.config, &inside).is_err());
    assert_eq!(audit_command::load_anchor(&case.config, &outside).unwrap(), case.anchor);
    let link = case.root.0.join("anchor-symlink"); std::os::unix::fs::symlink(&outside, &link).unwrap();
    assert!(audit_command::load_anchor(&case.config, &link).is_err());
    fs::write(&outside, b"truncated").unwrap(); assert!(audit_command::load_anchor(&case.config, &outside).is_err());
    drop(case.wire); assert_eq!(cleanup(case.driver, 1000, 1), 0);
}

struct FailedOutput(usize);
impl io::Write for FailedOutput {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.0 += 1; Err(io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> io::Result<()> { panic!("failed output cannot flush") }
}
#[test]
fn output_failure_leaves_original_effect_history_and_review_keys_untouched() {
    let case = recorded(Some(true), b"allow", true);
    let before = files(&case.config.store); let mut output = FailedOutput(0);
    let error = audit_command::run(&case.config, &case.loaded, case.loaded.rounds[0],
        &case.anchor, &mut output, &mut || Ok(())).unwrap_err();
    assert!(error.contains("audit output failed")); assert_eq!(output.0, 1);
    assert_eq!(files(&case.config.store), before);
    assert_eq!(case.driver.supervisor().host().unwrap().inspect(), *case.expected.journal_snapshot());
    drop(case.wire); assert_eq!(cleanup(case.driver, 1000, 1), 0);
}

#[test]
fn valid_changed_generation_recipe_refuses_even_when_the_original_first_message_is_unchanged() {
    let case = recorded(Some(true), b"allow", false);
    let path = case.root.0.join("recipe.json"); let original = fs::read_to_string(&path).unwrap();
    let changed = original.replace("\"max_new_tokens\":2", "\"max_new_tokens\":3");
    assert_ne!(changed, original); fs::write(&path, changed).unwrap();
    let different = recipe::load(&path, &case.config, false).unwrap();
    let before = files(&case.config.store); let mut output = Vec::new();
    let error = audit_command::run(&case.config, &different, case.loaded.rounds[0],
        &case.anchor, &mut output, &mut || Ok(())).unwrap_err();
    assert!(error.contains("Binding")); assert!(output.is_empty());
    assert_eq!(files(&case.config.store), before);
    assert_eq!(report(&case).get("status").unwrap().as_str(), Some("complete"));
    fs::write(path, original).unwrap();
    drop(case.wire); assert_eq!(cleanup(case.driver, 1000, 1), 0);
}
