//! Actual loaded operator recipes and original live histories; no recovery entry.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, io, path::PathBuf};

fn config_file(root: &Root, config: &Config) -> PathBuf {
    let original = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"));
    let json = original.replace("/operator/publication-store", root.0.join("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap())
        .replace("\"initial_payload_hex\": \"696e697469616c\"", "\"initial_payload_hex\": \"\"")
        .replace("\"events\": 256", "\"events\": 1024")
        .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 60000")
        .replace("\"poll_ms\": 10", "\"poll_ms\": 1")
        .replace("\"cleanup_ms\": 2000", "\"cleanup_ms\": 1000");
    let path = root.0.join("situation-config.json"); fs::write(&path, json).unwrap();
    let decoded = Config::read(&path).unwrap();
    crate::config::assert_oversight_profiles_eq(&decoded.profile, &config.profile);
    assert_eq!(decoded.source_policy, config.source_policy);
    path
}
fn args(config: &Path, recipe: &Path, anchor: Option<&Path>) -> Vec<String> {
    let mut args = vec!["situation".into(), config.to_str().unwrap().into(),
        "--learned-recipe".into(), recipe.to_str().unwrap().into(), "--page-size".into(), "1".into()];
    if let Some(path) = anchor { args.extend(["--history-anchor".into(), path.to_str().unwrap().into()]); }
    args
}
fn files(config: &Config) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&config.store).unwrap().map(|entry| entry.unwrap().path()).filter(|path| path.is_file())
        .map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
}
fn save_anchor(root: &Root, anchor: &FileHistoryAnchor) -> PathBuf {
    let path = root.0.join("independent.anchor"); let mut bytes = Vec::new();
    anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap(); fs::write(&path, bytes).unwrap(); path
}
fn parsed(bytes: &[u8]) -> Json {
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    strict_json::parse(bytes, Limits { max_bytes: 65_536, max_depth: 12, max_items: 2048, max_string_bytes: 256 }).unwrap()
}
fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, name| &value.as_object().unwrap()[*name])
}
fn report_matches(bytes: &[u8], host: &FileOversight, anchored: Option<u64>) {
    let report = parsed(bytes); let state = host.inspect();
    assert_eq!(field(&report, &["schema"]).as_str(), Some("fa.publication-situation/1"));
    assert_eq!(field(&report, &["historical"]).as_bool(), Some(true));
    assert_eq!(field(&report, &["current_eligibility"]).as_str(), Some("not_observed"));
    let basis = field(&report, &["knowledge", "basis"]);
    assert_eq!(field(basis, &["origin"]).as_str(), Some("canonical_image"));
    assert_eq!(field(basis, &["journal_revision"]).as_str().unwrap(), state.revision.to_string());
    assert_eq!(field(basis, &["control_sequence"]).as_str().unwrap(), state.control.sequence.to_string());
    match anchored {
        Some(revision) => assert_eq!(field(basis, &["anchored_through"]).as_str().unwrap(), revision.to_string()),
        None => assert!(matches!(field(basis, &["anchored_through"]), Json::Null)),
    }
    let summary = field(&report, &["knowledge", "value", "summary"]);
    assert_eq!(field(summary, &["executions"]).as_str().unwrap(), state.executions.to_string());
    assert_eq!(field(summary, &["charged_units"]).as_str().unwrap(), state.control.ledger.charged.to_string());
    assert_eq!(field(summary, &["reserved_units"]).as_str().unwrap(), state.control.ledger.reserved.to_string());
    assert_eq!(field(summary, &["attempts"]).as_str().unwrap(), state.control.ledger.stages.len().to_string());
}

#[test]
fn loaded_pending_publication_reports_without_evidence_or_native_reviewer_assets() {
    let root = Root::new();
    let (config, _loaded, _reviewer, driver, _wire, _deadline) = prepared(&root, b"allow");
    let path = config_file(&root, &config); let recipe = root.0.join("recipe.json");
    let host = driver.supervisor().host().unwrap();
    let anchor = host.history_anchor().unwrap(); let anchor_path = save_anchor(&root, &anchor);
    let before = host.inspect(); let numerical = host.learned_generation_inspection().unwrap();
    assert_eq!(before.control.ledger.stages[&1], ActionState::Reviewing);
    let original = files(&config); let reads = config.source.status().read_attempts;
    for name in ["evidence.json", "native-roster.json", "helper-model.json", "helper-weights.safetensors"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    for selected in [None, Some(anchor_path.as_path())] {
        let mut out = Vec::new(); crate::situation::command(&args(&path, &recipe, selected), &mut out).unwrap();
        report_matches(&out, &host, selected.map(|_| anchor.revision()));
        assert_eq!(field(&parsed(&out), &["knowledge", "value", "summary", "pending_attempts"]).as_str(), Some("1"));
        assert!(!String::from_utf8(out).unwrap().contains("helper_input"));
    }
    assert_eq!(host.inspect(), before); assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
    assert_eq!(config.source.status().read_attempts, reads); assert_eq!(files(&config), original);
}

#[test]
fn plain_raw_and_owned_loaded_recipes_use_the_original_numerical_binding() {
    for lane in 0..3 {
        let root = Root::new(); let mut config = configured(&root);
        let recipe_path = if lane == 0 { fixture::write(&root, b"allow") } else {
            let path = super::continuation::write(&root);
            super::forecast::install(&root, &path, lane == 2, 1000); path
        };
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&recipe_path, &config, true).unwrap();
        let (host, _reviewer, _observer, _identity) = recovery::create_selected(&config, &loaded).unwrap();
        let (_port, mut driver) = host.into_supervised_driver();
        driver.prepare_file_intake(&mut config.source, &mut || ElapsedTick(1000)).result.unwrap();
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let n = host.learned_generation_inspection().unwrap().numerical;
            let revision = host.revision();
            host.advance_learned_generation(revision, n.actor_revision, n.position).unwrap().unwrap();
        }
        let path = config_file(&root, &config); let original = files(&config);
        let host = driver.supervisor().host().unwrap(); let numerical = host.learned_generation_inspection().unwrap();
        let mut out = Vec::new(); crate::situation::command(&args(&path, &recipe_path, None), &mut out).unwrap();
        report_matches(&out, &host, None);
        let no_recipe = vec!["situation".into(), path.to_str().unwrap().into()];
        let mut refused = Vec::new(); assert!(crate::situation::command(&no_recipe, &mut refused).is_err());
        assert!(refused.is_empty()); assert_eq!(files(&config), original);
        assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
    }
}

#[test]
fn changed_recipe_or_missing_model_cannot_fall_back_or_print_partial_success() {
    let root = Root::new(); let config = configured(&root);
    let recipe_path = fixture::write(&root, b"allow");
    let loaded = recipe::load(&recipe_path, &config, false).unwrap();
    let (host, _) = recovery::create(&config, &loaded).unwrap();
    let path = config_file(&root, &config); let original = files(&config);
    let argv = args(&path, &recipe_path, None);
    let mut out = Vec::new(); crate::situation::command(&argv, &mut out).unwrap();
    report_matches(&out, &host, None);
    let prompt_path = root.0.join("prompt.txt"); let prompt = fs::read(&prompt_path).unwrap();
    let mut changed = prompt.clone(); changed.push(b'x'); fs::write(&prompt_path, changed).unwrap();
    out.clear(); assert!(crate::situation::command(&argv, &mut out).is_err()); assert!(out.is_empty());
    fs::write(&prompt_path, prompt).unwrap();
    let weights_path = root.0.join("weights.safetensors"); let weights = fs::read(&weights_path).unwrap();
    fs::remove_file(&weights_path).unwrap();
    assert!(crate::situation::command(&argv, &mut out).is_err()); assert!(out.is_empty());
    fs::write(&weights_path, weights).unwrap();
    crate::situation::command(&argv, &mut out).unwrap(); report_matches(&out, &host, None);
    assert_eq!(files(&config), original);
}

#[test]
fn recipe_loading_and_image_binding_share_the_callers_unrenewed_deadline() {
    let root = Root::new(); let config = configured(&root);
    let path = fixture::write(&root, b"allow");
    let loaded = recipe::load(&path, &config, false).unwrap();
    let (host, _) = recovery::create(&config, &loaded).unwrap(); let original = files(&config);
    for cut in 0..3 {
        let mut calls = 0;
        let selected = if cut == 0 { root.0.join("missing-recipe") } else { path.clone() };
        let error = config.begin_learned_situation(&selected, None, &mut || {
            let at = calls; calls += 1;
            if at == cut { Err("selected elapsed boundary".to_owned()) } else { Ok(()) }
        }).unwrap_err();
        assert_eq!(error, "selected elapsed boundary"); assert_eq!(calls, cut + 1);
        assert_eq!(files(&config), original);
    }
    let mut calls = 0;
    let cursor = config.begin_learned_situation(&path, None, &mut || { calls += 1; Ok(()) }).unwrap();
    assert_eq!(calls, 3); assert_eq!(cursor.progress().checked_events, 0);
    let report = cursor.complete().unwrap();
    let Knowledge::Known { basis, .. } = report.page(None, 1).unwrap() else { panic!("known") };
    assert_eq!(basis.journal_revision, host.revision()); assert_eq!(files(&config), original);
}

#[test]
fn selected_anchor_cannot_be_silently_ignored_after_journal_rollback() {
    let root = Root::new(); let config = configured(&root);
    let recipe_path = fixture::write(&root, b"allow");
    let loaded = recipe::load(&recipe_path, &config, false).unwrap();
    let (mut host, _) = recovery::create(&config, &loaded).unwrap();
    let canonical = config.store.join("delivery.bin"); let old = fs::read(&canonical).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let current = fs::read(&canonical).unwrap(); let anchor = host.history_anchor().unwrap();
    let anchor_path = save_anchor(&root, &anchor); let path = config_file(&root, &config);
    let argv = args(&path, &recipe_path, Some(&anchor_path)); let mut out = Vec::new();
    crate::situation::command(&argv, &mut out).unwrap(); report_matches(&out, &host, Some(anchor.revision()));
    fs::write(&canonical, &old).unwrap(); out.clear();
    assert!(crate::situation::command(&argv, &mut out).is_err()); assert!(out.is_empty());
    assert_eq!(fs::read(&canonical).unwrap(), old);
    fs::write(&canonical, &current).unwrap();
    crate::situation::command(&argv, &mut out).unwrap(); report_matches(&out, &host, Some(anchor.revision()));
    assert_eq!(fs::read(&canonical).unwrap(), current);
}

struct BrokenOutput { bytes: Vec<u8>, writes: usize }
impl Write for BrokenOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.writes != 1 { return Err(io::Error::from(io::ErrorKind::BrokenPipe)); }
        let count = bytes.len().min(3); self.bytes.extend_from_slice(&bytes[..count]); Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
#[test]
fn learned_report_output_failure_does_not_recover_or_rewrite_the_live_owner() {
    let root = Root::new(); let config = configured(&root);
    let recipe_path = fixture::write(&root, b"allow");
    let loaded = recipe::load(&recipe_path, &config, false).unwrap();
    let (host, _) = recovery::create(&config, &loaded).unwrap(); let original = files(&config);
    let state = host.inspect(); let path = config_file(&root, &config);
    let argv = args(&path, &recipe_path, None);
    let mut expected = Vec::new(); crate::situation::command(&argv, &mut expected).unwrap();
    let mut out = BrokenOutput { bytes: Vec::new(), writes: 0 };
    assert!(crate::situation::command(&argv, &mut out).unwrap_err().contains("output failed"));
    assert_eq!(out.bytes, expected[..3]); assert_eq!(out.writes, 2);
    assert_eq!(host.inspect(), state); assert_eq!(files(&config), original);
}
