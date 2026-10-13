//! Real operator inputs, original source observations and request-key histories.
//! Counterfactuals are exact-policy experiments, not helper judgments or permits.
use super::*;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, VERSION};
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve, requests::FileRequestDisposition};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor, anchored::MAX_HISTORY_ANCHOR_BYTES};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::Snapshot;
use std::{collections::BTreeMap, fs, io, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

const TEMPLATE: &str = include_str!("../../../fixtures/supervised_publication.json");
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-explain-command-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("explanation cleanup: {error}"); } }
}
struct Case {
    config: Config,
    config_file: PathBuf,
    profile_file: PathBuf,
    anchor: PathBuf,
    floor: FileRecoveryFloor,
    host: FileOversight,
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn spec(config: &Config, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION, scope: config.profile.delivery.scope, target: Some(config.profile.delivery.target),
        payload: payload.to_vec(), required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100000), units: 2 }
}
fn profile_bytes(floor: FileRecoveryFloor, anchor: &Path, output: Option<&Path>) -> Vec<u8> {
    let output = output.map(|path| format!(",\"retain_history_anchor\":{:?}", path.to_str().unwrap())).unwrap_or_default();
    format!(concat!("{{\"schema_version\":\"fa.guarded-stop/1\",\"minimum\":{{",
        "\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},",
        "\"history_anchor\":{:?}{}}}"), floor.journal_revision, floor.control_sequence,
        floor.authority_epoch, anchor.to_str().unwrap(), output).into_bytes()
}
fn setup(root: &Root) -> Case {
    let template = TEMPLATE.replace("/operator", root.0.to_str().unwrap())
        .replacen("\"maximum\": 512", "\"maximum\": 4", 1);
    let config_file = root.0.join("config.json"); fs::write(&config_file, template).unwrap();
    let mut config = Config::read(&config_file).unwrap();
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
        scope: config.profile.delivery.scope }, snapshot(), ["alpha", "beta"].into_iter()
        .map(|member| (member.to_owned(), b"original source fixture".to_vec())).collect()).unwrap();
    let evidence = root.0.join("evidence.json"); fs::write(&evidence, data.encode()).unwrap();
    let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.enable_file_source(host.revision(), config.source_policy).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1)).unwrap();
    host.propose(host.revision(), 1, spec(&config, b"prior"), snapshot()).unwrap();
    let denied = host.submit_request(host.revision(), 901, spec(&config, b"blocked"), snapshot()).unwrap();
    assert_eq!(denied.disposition, FileRequestDisposition::Admitted { attempt: 2, stage: ActionState::Denied });
    let allowed = host.submit_request(host.revision(), 2, spec(&config, b"ok"), snapshot()).unwrap();
    assert_eq!(allowed.disposition, FileRequestDisposition::Admitted { attempt: 3, stage: ActionState::Reviewing });
    host.cancel_request(host.revision(), 2).unwrap();
    host.withdraw_file_source(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    // No live assets survive. Diagnosis must read only the retained original
    // source event and request history even while this writer lock remains held.
    fs::remove_file(evidence).unwrap(); config.programs.clear();
    let view = host.inspect();
    let floor = FileRecoveryFloor { journal_revision: view.revision,
        control_sequence: view.control.sequence, authority_epoch: view.control.ledger.epoch };
    let mut bytes = Vec::new(); host.history_anchor().unwrap().write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    let anchor = root.0.join("retained.anchor"); fs::write(&anchor, bytes).unwrap();
    let profile_file = root.0.join("read-profile.json"); fs::write(&profile_file, profile_bytes(floor, &anchor, None)).unwrap();
    fs::write(config.store.join("delivery.pending"), b"not a diagnostic scratch file").unwrap();
    Case { config, config_file, profile_file, anchor, floor, host }
}
fn files(case: &Case) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&case.config.store).unwrap().map(|entry| entry.unwrap().path()).filter(|path| path.is_file())
        .chain([case.anchor.clone()]).map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
}
fn args(case: &Case, request: u64, payload: Option<&Path>) -> Vec<String> {
    let mut args = vec!["explain-proposal".into(), case.config_file.to_str().unwrap().into(),
        request.to_string(), case.profile_file.to_str().unwrap().into()];
    if let Some(path) = payload { args.extend(["--payload".into(), path.to_str().unwrap().into()]); }
    args
}
fn output(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn text<'a>(value: &'a Json, key: &str) -> &'a str { value.get(key).unwrap().as_str().unwrap() }
fn run(case: &Case, request: u64, payload: Option<&[u8]>) -> Vec<u8> {
    let profile = Profile::read(&case.profile_file).unwrap(); let mut out = Vec::new();
    run_with_elapsed(&case.config, &profile, request, payload, &mut || Duration::ZERO, &mut out).unwrap(); out
}

#[test]
fn actual_command_explains_original_denial_and_payload_repair_without_live_assets_or_writes() {
    let root = Root::new(); let case = setup(&root); let before = files(&case);
    let original = case.host.investigate_proposal(2, InterventionScope::new(901, true, false, false, &[]).unwrap()).unwrap();
    let baseline = run(&case, 901, None); let json = output(&baseline);
    assert_eq!(text(&json, "permission"), "not_issued"); assert_eq!(text(&json, "purpose"), "experiment");
    assert_eq!(text(&json, "request"), "901"); assert_eq!(text(&json, "attempt"), "2");
    assert_eq!(text(&json, "admission_state"), "Denied"); assert_eq!(text(&json, "ending_state"), "Denied");
    assert_eq!(text(&json, "baseline"), "violated"); assert_eq!(text(&json, "counterfactual"), "violated");
    assert_eq!(text(&json, "next_requirement"), "exact_policy_violation");
    assert_eq!(text(&json, "admission_revision"), original.origin().admission_snapshot().revision.to_string());
    assert_eq!(text(&json, "journal_revision"), case.host.revision().to_string());
    let replacement = root.0.join("replacement.bin"); fs::write(&replacement, b"ok").unwrap();
    let mut out = Vec::new(); command_with_elapsed(&args(&case, 901, Some(&replacement)),
        &mut || Duration::ZERO, &mut out).unwrap();
    let expected = original.run(&[Intervention::Payload(b"ok".to_vec())]).unwrap();
    assert_eq!(output(&out), output(&render::encode(&expected).unwrap()));
    let json = output(&out); assert_eq!(text(&json, "counterfactual"), "satisfied");
    assert_eq!(text(&json, "next_requirement"), "fresh_independent_review");
    let Json::Array(nodes) = json.get("predicate_tree").unwrap().get("nodes").unwrap() else { panic!("native nodes") };
    assert_eq!(nodes.len(), 4);
    assert_eq!(text(&nodes[1], "baseline"), "violated"); assert_eq!(text(&nodes[1], "counterfactual"), "satisfied");
    assert_eq!(text(nodes[1].get("predicate").unwrap(), "op"), "payload_at_most");
    assert_eq!(text(&nodes[0], "baseline"), "satisfied");
    assert!(out.is_ascii()); assert!(!out.contains(&0x1b));
    assert_eq!(case.host.inspect().control.ledger.stages[&2], ActionState::Denied);
    assert_eq!(case.host.inspect().executions, 0); assert_eq!(files(&case), before);
    assert!(!root.0.join("helpers").exists()); assert!(!root.0.join("evidence.json").exists());
    assert!(!case.config.socket(901).exists());
    let reads = case.config.source.status().read_attempts;
    assert_eq!(run(&case, 901, None), baseline); assert_eq!(case.config.source.status().read_attempts, reads);
    // The actual top-level dispatcher selects this same read-only implementation.
    crate::command(args(&case, 901, None)).unwrap(); assert_eq!(files(&case), before);
}

#[test]
fn colliding_request_key_keeps_admission_distinct_from_later_cancellation() {
    let root = Root::new(); let case = setup(&root); let before = files(&case);
    let json = output(&run(&case, 2, None));
    assert_eq!(text(&json, "request"), "2"); assert_eq!(text(&json, "attempt"), "3");
    assert_eq!(text(&json, "admission_state"), "Reviewing"); assert_eq!(text(&json, "ending_state"), "Cancelled");
    assert_eq!(text(&json, "baseline"), "satisfied");
    assert_eq!(text(&json, "next_requirement"), "fresh_independent_review");
    for key in [1, 777] {
        let mut out = Vec::new();
        assert!(command_with_elapsed(&args(&case, key, None), &mut || Duration::ZERO, &mut out)
            .unwrap_err().contains("Missing"));
        assert!(out.is_empty()); assert_eq!(files(&case), before);
    }
}

#[test]
fn every_elapsed_checkpoint_including_ready_and_preoutput_rejects_without_partial_json() {
    let root = Root::new(); let mut case = setup(&root); case.config.timing.runtime_ms = 50;
    let profile = Profile::read(&case.profile_file).unwrap(); let before = files(&case);
    let wall = Duration::from_millis(50); let below = wall - Duration::from_nanos(1);
    let mut count = 0;
    run_with_elapsed(&case.config, &profile, 901, Some(b"ok"),
        &mut || { count += 1; below }, &mut Vec::new()).unwrap();
    assert_eq!(count, case.host.revision() as usize + 6);
    for cut in 0..count {
        let mut calls = 0; let mut out = Vec::new();
        let result = run_with_elapsed(&case.config, &profile, 901, Some(b"ok"), &mut || {
            let here = calls; calls += 1; if here == cut { wall } else { below }
        }, &mut out);
        assert_eq!(result.err().as_deref(), Some(DEADLINE)); assert_eq!(calls, cut + 1);
        assert!(out.is_empty()); assert_eq!(files(&case), before);
    }
    let mut total = Duration::ZERO; let mut out = Vec::new();
    assert_eq!(run_with_elapsed(&case.config, &profile, 901, None, &mut || {
        total += Duration::from_millis(10); total
    }, &mut out).err().as_deref(), Some(DEADLINE));
    assert!(out.is_empty()); assert_eq!(files(&case), before);
    // The executable's config/profile reads share the same, not renewed, budget.
    let original_wall = Duration::from_millis(Config::read(&case.config_file).unwrap().timing.runtime_ms);
    for cut in 0..3 {
        let mut calls = 0; let mut out = Vec::new();
        assert_eq!(command_with_elapsed(&args(&case, 901, None), &mut || {
            let here = calls; calls += 1; if here == cut { original_wall } else { Duration::ZERO }
        }, &mut out).err().as_deref(), Some(DEADLINE));
        assert!(out.is_empty()); assert_eq!(calls, cut + 1); assert_eq!(files(&case), before);
    }
}

#[test]
fn wrong_custody_floors_and_source_never_fall_back_or_export_a_new_anchor() {
    let root = Root::new(); let mut case = setup(&root); let before = files(&case);
    let output_path = root.0.join("must-not-exist.anchor");
    let disallowed = Profile::decode(&profile_bytes(case.floor, &root.0.join("missing"), Some(&output_path))).unwrap();
    let mut out = Vec::new();
    assert_eq!(run_with_elapsed(&case.config, &disallowed, 901, None, &mut || Duration::ZERO, &mut out)
        .err().as_deref(), Some("read-only proposal explanation cannot retain a history anchor"));
    assert!(out.is_empty()); assert!(!output_path.exists());
    for dimension in 0..3 {
        let mut floor = case.floor;
        match dimension { 0 => floor.journal_revision = u64::MAX, 1 => floor.control_sequence = u64::MAX,
            _ => floor.authority_epoch = u64::MAX }
        let wrong = Profile::decode(&profile_bytes(floor, &case.anchor, None)).unwrap();
        assert!(run_with_elapsed(&case.config, &wrong, 901, None, &mut || Duration::ZERO, &mut out)
            .unwrap_err().contains("Stale")); assert!(out.is_empty());
    }
    let source = case.config.source_policy; case.config.source_policy.source.generation += 1;
    let profile = Profile::read(&case.profile_file).unwrap();
    assert!(run_with_elapsed(&case.config, &profile, 901, None, &mut || Duration::ZERO, &mut out)
        .unwrap_err().contains("Binding")); case.config.source_policy = source;
    let other_root = Root::new(); let other = setup(&other_root);
    for path in [&other.anchor, &root.0.join("missing.anchor")] {
        let wrong = Profile::decode(&profile_bytes(case.floor, path, None)).unwrap();
        assert!(run_with_elapsed(&case.config, &wrong, 901, None, &mut || Duration::ZERO, &mut out).is_err());
        assert!(out.is_empty()); assert_eq!(files(&case), before);
    }
    let inside = case.config.store.join("inside.anchor"); fs::write(&inside, fs::read(&case.anchor).unwrap()).unwrap();
    let wrong = Profile::decode(&profile_bytes(case.floor, &inside, None)).unwrap();
    assert!(run_with_elapsed(&case.config, &wrong, 901, None, &mut || Duration::ZERO, &mut out)
        .unwrap_err().contains("custody outside")); assert!(out.is_empty()); fs::remove_file(inside).unwrap();
    assert!(!run(&case, 901, Some(b"ok")).is_empty()); assert_eq!(files(&case), before);
}

#[test]
fn malformed_cli_and_oversized_or_missing_payload_are_not_silent_baseline_requests() {
    let root = Root::new(); let case = setup(&root); let before = files(&case);
    let valid = args(&case, 901, None);
    for bad in ["0", "+1", "-1", "1.0", " 901", "18446744073709551616", ""] {
        let mut argv = valid.clone(); argv[2] = bad.into(); argv[1] = "missing-config".into();
        assert_eq!(command_with_elapsed(&argv, &mut || panic!("syntax precedes time and file reads"),
            &mut Vec::new()).err().as_deref(), Some(USAGE));
        assert_eq!(crate::command(argv).err().as_deref(), Some(USAGE));
    }
    for extra in [vec!["--payload"], vec!["--other", "absent"], vec!["--payload", "absent", "extra"],
        vec!["--credibility-activation", "absent"]] {
        let mut argv = valid.clone(); argv.extend(extra.into_iter().map(str::to_owned)); argv[1] = "missing-config".into();
        assert_eq!(command_with_elapsed(&argv, &mut || panic!("invalid option must not read time"),
            &mut Vec::new()).err().as_deref(), Some(USAGE));
    }
    let path = root.0.join("replacement.bin"); let mut out = Vec::new();
    assert!(command_with_elapsed(&args(&case, 901, Some(&path)), &mut || Duration::ZERO, &mut out).is_err());
    assert!(out.is_empty());
    fs::write(&path, vec![0xff; PAYLOAD_BYTES + 1]).unwrap();
    assert!(command_with_elapsed(&args(&case, 901, Some(&path)), &mut || Duration::ZERO, &mut out).is_err());
    assert!(out.is_empty());
    for bytes in [vec![], b"\x1b\xff".to_vec(), vec![0xff; PAYLOAD_BYTES]] {
        fs::write(&path, &bytes).unwrap(); let mut out = Vec::new();
        command_with_elapsed(&args(&case, 901, Some(&path)), &mut || Duration::ZERO, &mut out).unwrap();
        assert_eq!(text(&output(&out), "intervention"), "payload"); assert!(out.is_ascii()); assert!(!out.contains(&0x1b));
    }
    assert_eq!(files(&case), before);
}

#[test]
fn output_write_and_flush_failures_are_errors_without_any_effect_or_journal_change() {
    struct Broken { fail_write: bool, bytes: Vec<u8>, flushed: bool }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_write { return Err(io::ErrorKind::BrokenPipe.into()); }
            self.bytes.extend_from_slice(bytes); Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> { self.flushed = true; Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let root = Root::new(); let case = setup(&root); let before = files(&case);
    let profile = Profile::read(&case.profile_file).unwrap();
    for fail_write in [true, false] {
        let mut out = Broken { fail_write, bytes: Vec::new(), flushed: false };
        let error = run_with_elapsed(&case.config, &profile, 901, None, &mut || Duration::ZERO, &mut out).unwrap_err();
        assert!(error.contains("output")); assert!(error.contains("no authority or effect was changed"));
        assert_eq!(out.flushed, !fail_write); assert_eq!(out.bytes.is_empty(), fail_write);
        if !fail_write { assert_eq!(text(&output(&out.bytes), "permission"), "not_issued"); }
        assert_eq!(files(&case), before);
    }
    assert!(!run(&case, 901, None).is_empty()); assert_eq!(files(&case), before);
}

#[test]
fn concurrent_writer_progress_and_elapsed_unwind_do_not_relabel_or_mutate_the_captured_read() {
    let root = Root::new(); let mut case = setup(&root);
    let profile = Profile::read(&case.profile_file).unwrap(); let original = case.host.revision();
    let mut calls = 0; let mut out = Vec::new();
    run_with_elapsed(&case.config, &profile, 901, None, &mut || {
        calls += 1;
        if calls == 3 { case.host.observe_time(case.host.revision(), ElapsedTick(3)).unwrap(); }
        Duration::ZERO
    }, &mut out).unwrap();
    assert_eq!(text(&output(&out), "journal_revision"), original.to_string());
    assert_eq!(case.host.revision(), original + 1); let before = files(&case);
    let mut calls = 0;
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_with_elapsed(&case.config, &profile, 901, None, &mut || {
            calls += 1; assert_ne!(calls, 4, "injected elapsed unwind after an original event"); Duration::ZERO
        }, &mut Vec::new())
    }));
    assert!(unwound.is_err()); assert_eq!(files(&case), before);
    assert_eq!(text(&output(&run(&case, 901, None)), "journal_revision"), case.host.revision().to_string());
    assert_eq!(files(&case), before);
}
