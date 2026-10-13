//! Real original continuation helpers/reviewer; synthetic votes are not calibration.
use super::*;
use super::super::guarded::{self as submission, Submission};
use crate::recover_stop::guarded::{Profile, ResumeProfile, SubmitProfile};
use fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
use std::{cell::Cell, fs, path::Path};

struct Prepared {
    before: FileDeliverySnapshot,
    input: PathBuf,
    bytes: Vec<u8>,
    output: PathBuf,
    json: String,
}
fn profile_json(before: &FileDeliverySnapshot, input: &Path, output: Option<&Path>) -> String {
    let export = output.map(|path| format!(",\"retain_history_anchor\":{:?}", path.to_str().unwrap())).unwrap_or_default();
    format!("{{\"schema_version\":\"fa.guarded-submit/1\",\"minimum\":{{\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},\"history_anchor\":{:?}{export}}}",
        before.revision, before.control.sequence, before.control.ledger.epoch, input.to_str().unwrap())
}
fn captured(root: &Directory) -> Prepared {
    let config = configured(root, 0);
    // Acknowledge a real original owner for the independently retained fixture
    // prefix. This setup fence is accounted for when constructing new proposals.
    let (host, _) = FileOversight::open(&config.store, config.profile.clone()).unwrap();
    let before = host.inspect(); let anchor = host.history_anchor().unwrap();
    let mut bytes = Vec::new(); anchor.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    drop(host);
    let input = root.0.join("before.anchor"); fs::write(&input, &bytes).unwrap();
    let output = root.0.join("after.anchor");
    let json = profile_json(&before, &input, Some(&output));
    Prepared { before, input, bytes, output, json }
}
fn read_anchor(path: &Path) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(&mut io::Cursor::new(fs::read(path).unwrap()), MAX_HISTORY_ANCHOR_BYTES).unwrap()
}
fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(root.0.join("store")).unwrap().map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file()).map(|path| {
            let bytes = fs::read(&path).unwrap(); (path, bytes)
        }).collect()
}
fn submit(config: Config, profile: &SubmitProfile, document: &[u8]) -> Result<workflow::RunResult, String> {
    submission::submit(config, Submission { document, profile, peers: None, credibility: None }, || ElapsedTick(1_005))
}

#[test]
fn guarded_new_publications_require_fresh_keys_and_retain_the_original_budget_and_history() {
    let root = Directory::new(); first_publication(&root); let prepared = captured(&root);
    let mut before = prepared.before.clone(); let mut input = prepared.input.clone(); let mut charged = 5;
    for (request, payload) in [(2, b"second".as_slice()), (3, b"third".as_slice())] {
        let output = root.0.join(format!("after-{request}.anchor"));
        let profile = SubmitProfile::decode(profile_json(&before, &input, Some(&output)).as_bytes()).unwrap();
        let epoch = before.control.ledger.epoch + 1; let config = configured(&root, epoch);
        let original = document(request, payload, before.target, epoch); let frozen = original.clone();
        let human = reviewer(&config, request);
        let result = submit(config, &profile, &original).unwrap();
        let packet = human.join().unwrap(); assert_executed(&result);
        assert_eq!(original, frozen); assert_eq!(packet.action().spec().payload, payload);
        assert_eq!(packet.action().spec().policy_epoch, epoch);
        let config = configured(&root, epoch);
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        charged += payload.len() as u64;
        assert_eq!(after.executions, request); assert_eq!(after.payload, payload);
        assert_eq!(after.target.expected_version, before.target.expected_version + 1);
        assert_eq!(after.control.ledger.epoch, epoch); assert_eq!(after.control.ledger.charged, charged);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.control.ledger.available, config.profile.delivery.total - charged);
        assert_eq!(after.control.ledger.stages.len(), request as usize);
        let retained = read_anchor(&output); assert_eq!(retained.revision(), after.revision);
        assert_eq!(retained.retained_bytes(), fs::read(config.store.join("delivery.bin")).unwrap().len());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&output).unwrap().permissions().mode() & 0o077, 0);
        assert_eq!(fs::read(&prepared.input).unwrap(), prepared.bytes);
        input = output; before = after;
    }
}

#[test]
fn exact_expired_retry_is_query_only_and_changed_bytes_do_not_readmit() {
    let root = Directory::new(); let original = first_publication(&root); let prepared = captured(&root);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let missing = root.0.join("missing-credibility.json");
    let profile = SubmitProfile::decode(prepared.json.as_bytes()).unwrap();
    let mut config = configured(&root, 2); config.programs.clear();
    let result = submission::submit(config, Submission { document: &original, profile: &profile,
        peers: None, credibility: Some(&missing) }, || ElapsedTick(200_000)).unwrap();
    assert_executed(&result); assert!(!missing.exists());
    let config = configured(&root, 3); let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(before.revision, prepared.before.revision + 3);
    let output = root.0.join("conflict.anchor");
    let profile = SubmitProfile::decode(profile_json(&before, &prepared.output, Some(&output)).as_bytes()).unwrap();
    let changed = document(1, b"changed", prepared.before.target, 0);
    let mut config = configured(&root, 3); config.programs.clear();
    let result = submission::submit(config, Submission { document: &changed, profile: &profile,
        peers: None, credibility: Some(&missing) }, || ElapsedTick(200_001)).unwrap();
    assert_eq!(result.response.result, Err(WireError::IdempotencyConflict));
    let config = configured(&root, 3); let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 1); assert_eq!(after.payload, b"first");
    assert_eq!(after.control.ledger.charged, 5); assert_eq!(after.control.ledger.stages.len(), 1);
    assert_eq!(read_anchor(&output).revision(), after.revision);
    assert_eq!(fs::read(&prepared.input).unwrap(), prepared.bytes);
    assert!(!config.socket(1).exists()); assert!(!root.0.join("evidence.json").exists());
}

#[test]
fn missing_fresh_source_retains_the_acknowledged_cut_and_corrected_new_work_still_executes() {
    let root = Directory::new(); first_publication(&root); let prepared = captured(&root);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let profile = SubmitProfile::decode(prepared.json.as_bytes()).unwrap();
    let epoch = prepared.before.control.ledger.epoch + 1;
    let new = document(2, b"second", prepared.before.target, epoch);
    let failure = submit(configured(&root, epoch), &profile, &new).err().unwrap();
    assert!(failure.contains("recovery acknowledged"));
    let config = configured(&root, epoch); let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 1); assert_eq!(after.control.ledger.charged, 5);
    assert_eq!(after.control.ledger.stages.len(), 1); assert!(!config.socket(2).exists());
    assert_eq!(read_anchor(&prepared.output).revision(), after.revision);
    assert_eq!(fs::read(&prepared.input).unwrap(), prepared.bytes);
    // The absent source did not forward request 2 or mint a denied/retry record.
    // A corrected caller supplies the next real epoch and fresh original file.
    let config = configured(&root, after.control.ledger.epoch + 1); evidence(&root, &config);
    let corrected = document(2, b"second", after.target, after.control.ledger.epoch + 1);
    let output = root.0.join("corrected.anchor");
    let profile = SubmitProfile::decode(profile_json(&after, &prepared.output, Some(&output)).as_bytes()).unwrap();
    let human = reviewer(&config, 2); let result = submit(config, &profile, &corrected).unwrap();
    human.join().unwrap(); assert_executed(&result);
    assert_eq!(read_anchor(&output).revision(), FileOversight::read_publication(
        root.0.join("store"), &configured(&root, 3).profile).unwrap().revision);
}

#[test]
fn stale_new_proposal_is_never_rewritten_into_the_recovery_epoch() {
    let root = Directory::new(); first_publication(&root); let prepared = captured(&root);
    let profile = SubmitProfile::decode(prepared.json.as_bytes()).unwrap();
    let stale = document(2, b"stale", prepared.before.target, prepared.before.control.ledger.epoch);
    let mut config = configured(&root, 2); config.programs.clear();
    let result = submit(config, &profile, &stale).unwrap();
    assert!(!matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(result.cleanup_pending, 0);
    let config = configured(&root, 2); let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 1); assert_eq!(after.payload, b"first");
    assert_eq!(after.control.ledger.charged, 5); assert_eq!(after.control.ledger.reserved, 0);
    assert!(after.stop.is_none()); assert!(!config.socket(2).exists());
    assert_eq!(read_anchor(&prepared.output).revision(), after.revision);
}

#[test]
fn every_elapsed_replay_cut_cancels_without_fencing_forwarding_or_cleaning_staging() {
    let root = Directory::new(); first_publication(&root); let prepared = captured(&root);
    let profile = SubmitProfile::decode(prepared.json.as_bytes()).unwrap();
    fs::write(root.0.join("store/delivery.pending"), b"retain until acknowledged").unwrap();
    let before = files(&root); let total = prepared.before.revision as usize;
    let new = document(2, b"second", prepared.before.target, prepared.before.control.ledger.epoch + 1);
    for at in 0..=total + 2 {
        let mut config = configured(&root, 2); config.timing.runtime_ms = 50;
        let mut calls = 0; let clocks = Cell::new(0);
        let failure = submission::submit_with_elapsed(config,
            Submission { document: &new, profile: &profile, peers: None, credibility: None },
            || { clocks.set(clocks.get() + 1); ElapsedTick(1_005) }, Instant::now(), &mut || {
                let current = calls; calls += 1;
                if current == at { Duration::from_millis(50) } else { Duration::ZERO }
            }).err().unwrap();
        assert_eq!(failure, "guarded submission recovery replay deadline reached", "cut {at}");
        assert_eq!(calls, at + 1); assert_eq!(clocks.get(), u32::from(at != 0));
        assert_eq!(files(&root), before); assert!(!prepared.output.exists());
        assert_eq!(fs::read(&prepared.input).unwrap(), prepared.bytes);
    }
    let config = configured(&root, 2); let human = reviewer(&config, 2);
    let result = submit(config, &profile, &new).unwrap(); human.join().unwrap(); assert_executed(&result);
    assert!(!root.0.join("store/delivery.pending").exists());
}

#[test]
fn independent_floor_source_and_anchor_mismatches_preserve_history_before_new_work() {
    let root = Directory::new(); first_publication(&root); let prepared = captured(&root);
    let new = document(2, b"second", prepared.before.target, prepared.before.control.ledger.epoch + 1);
    fs::write(root.0.join("store/delivery.pending"), b"staging").unwrap(); let before = files(&root);
    for mode in 0..3 {
        let mut config = configured(&root, 2); let mut json = prepared.json.clone();
        match mode {
            0 => json = json.replace(&format!("\"journal_revision\":{}", prepared.before.revision),
                &format!("\"journal_revision\":{}", prepared.before.revision + 1)),
            1 => config.source_policy.source.generation += 1,
            _ => config.profile.delivery.total += 1,
        }
        let profile = SubmitProfile::decode(json.as_bytes()).unwrap();
        let calls = Cell::new(0);
        let failure = submission::submit(config, Submission { document: &new, profile: &profile,
            peers: None, credibility: None }, || { calls.set(calls.get() + 1); ElapsedTick(1_005) });
        assert!(failure.is_err()); assert_eq!(calls.get(), 1);
        assert_eq!(files(&root), before); assert!(!prepared.output.exists());
    }
    let foreign = Directory::new(); first_publication(&foreign); let other = captured(&foreign);
    let profile = SubmitProfile::decode(profile_json(&prepared.before, &other.input, Some(&prepared.output)).as_bytes()).unwrap();
    assert!(submit(configured(&root, 2), &profile, &new).is_err());
    assert_eq!(files(&root), before); assert!(!prepared.output.exists());
    let profile = SubmitProfile::decode(prepared.json.as_bytes()).unwrap();
    let config = configured(&root, 2); let human = reviewer(&config, 2);
    let result = submit(config, &profile, &new).unwrap(); human.join().unwrap(); assert_executed(&result);
}

#[test]
fn late_export_collision_preserves_the_executed_actor_result_and_all_prior_archives() {
    let root = Directory::new(); first_publication(&root); let prepared = captured(&root);
    let profile = SubmitProfile::decode(prepared.json.as_bytes()).unwrap();
    let new = document(2, b"second", prepared.before.target, prepared.before.control.ledger.epoch + 1);
    let config = configured(&root, 2); let human = reviewer(&config, 2); let mut calls = 0;
    let result = submission::submit(config, Submission { document: &new, profile: &profile,
        peers: None, credibility: None }, || {
            calls += 1;
            // Time 1 precedes custody admission; time 2 follows complete replay.
            if calls == 2 { fs::write(&prepared.output, b"independent winner").unwrap(); }
            ElapsedTick(1_005)
        }).unwrap();
    human.join().unwrap();
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let failure = result.failure.unwrap(); assert!(failure.contains("history anchor retention failed"));
    assert!(failure.contains("recovery acknowledged")); assert_eq!(result.cleanup_pending, 0);
    assert_eq!(fs::read(&prepared.output).unwrap(), b"independent winner");
    assert_eq!(fs::read(&prepared.input).unwrap(), prepared.bytes);
    let config = configured(&root, 2); let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 2); assert_eq!(after.control.ledger.charged, 11);
    let corrected = root.0.join("corrected-retention.anchor");
    let profile = SubmitProfile::decode(profile_json(&prepared.before, &prepared.input, Some(&corrected)).as_bytes()).unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut config = configured(&root, 3); config.programs.clear();
    assert_executed(&submit(config, &profile, &new).unwrap());
    assert_eq!(FileOversight::read_publication(root.0.join("store"), &configured(&root, 3).profile).unwrap().executions, 2);
    assert!(read_anchor(&corrected).revision() > after.revision);
}

#[test]
fn guarded_submission_intent_is_not_a_stop_or_receipt_profile_and_never_creates_a_missing_store() {
    let root = Directory::new(); let config = configured(&root, 1);
    let json = "{\"schema_version\":\"fa.guarded-submit/1\",\"minimum\":{\"journal_revision\":0,\"control_sequence\":0,\"authority_epoch\":0},\"history_anchor\":\"/missing/anchor\"}";
    let profile = SubmitProfile::decode(json.as_bytes()).unwrap();
    assert!(Profile::decode(json.as_bytes()).is_err()); assert!(ResumeProfile::decode(json.as_bytes()).is_err());
    for schema in ["fa.guarded-stop/1", "fa.guarded-resume/1", "fa.guarded-submit/2"] {
        assert!(SubmitProfile::decode(json.replace("fa.guarded-submit/1", schema).as_bytes()).is_err());
    }
    for bad in [json.replace("\"journal_revision\":0,", ""),
        json.replace("\"control_sequence\":0", "\"control_sequence\":-1"),
        json.replace("\"authority_epoch\":0", "\"authority_epoch\":0,\"authority_epoch\":1"),
        json.replace("/missing/anchor", "relative.anchor")] {
        assert!(SubmitProfile::decode(bad.as_bytes()).is_err());
    }
    let new = document(1, b"new", config.profile.delivery.target, 1);
    assert!(submit(config, &profile, &new).is_err()); assert!(!root.0.join("store").exists());
    assert!(submission::submit(configured(&root, 1), Submission { document: b"{}", profile: &profile,
        peers: None, credibility: None }, || panic!("invalid input before clock")).is_err());
    assert!(!root.0.join("store").exists());
}

#[test]
fn unresolved_and_already_irrecoverable_deliveries_prevent_new_admission_on_repeated_recovery() {
    let root = Directory::new(); first_publication(&root);
    let config = configured(&root, 1); let store = config.store.clone(); let bootstrap = config.profile.clone();
    let before = FileOversight::read_publication(&store, &bootstrap).unwrap();
    let original = document(2, b"second", before.target, 1);
    let late = 100_001 + config.profile.delivery.retention_ticks;
    let human = reviewer(&config, 2); let mut expired = false;
    let _result = submit_existing(config, &original, None, None, || {
        if let Ok(state) = FileOversight::read_publication(&store, &bootstrap)
            && state.control.ledger.charged > 5 && state.executions == 1 { expired = true; }
        ElapsedTick(if expired { late } else { 1_005 })
    }).unwrap();
    human.join().unwrap(); assert!(expired);
    let prepared = captured(&root); assert_eq!(prepared.before.executions, 1);
    assert_eq!(prepared.before.control.ledger.charged, 11);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    for attempt in 0..2 {
        let mut config = configured(&root, 4 + attempt); config.programs.clear();
        let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        let next = document(3, b"third", before.target, before.control.ledger.epoch + 1);
        let output = root.0.join(format!("unresolved-{attempt}.anchor"));
        let profile = SubmitProfile::decode(profile_json(&prepared.before, &prepared.input, Some(&output)).as_bytes()).unwrap();
        let error = submission::submit(config, Submission { document: &next, profile: &profile,
            peers: None, credibility: None }, || ElapsedTick(late + 1 + attempt)).err().unwrap();
        assert!(error.contains("unresolved prior deliveries")); assert!(error.contains("no new work admitted"));
        let after = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(after.executions, 1); assert_eq!(after.control.ledger.charged, 11);
        assert_eq!(after.control.ledger.stages.len(), 2); assert_eq!(after.revision, before.revision + 3);
        assert_eq!(read_anchor(&output).revision(), after.revision);
        assert_eq!(fs::read(&prepared.input).unwrap(), prepared.bytes);
    }
}

fn disk_config(root: &Directory, epoch: u64) -> PathBuf {
    let executable = std::env::current_exe().unwrap();
    let arguments = format!("\"arguments\": [\"--exact\", {CHILD:?}, \"--nocapture\"]");
    let mut text = std::str::from_utf8(TEMPLATE).unwrap()
        .replace("/operator/publication-store", root.0.join("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap())
        .replace("/operator/helpers/alpha", executable.to_str().unwrap())
        .replace("/operator/helpers/beta", executable.to_str().unwrap())
        .replace("/operator/helpers", root.0.to_str().unwrap())
        .replace("\"arguments\": []", &arguments);
    for member in ["alpha", "beta"] {
        text = text.replacen("\"environment\": {}", &format!(
            "\"environment\": {{{MEMBER:?}:{member:?}, {EPOCH:?}:\"{epoch}\"}}"), 1);
    }
    let path = root.0.join("config.json"); fs::write(&path, text).unwrap(); path
}

#[test]
fn actual_command_publishes_new_work_then_recovers_exact_receipt_without_live_assets() {
    let root = Directory::new(); first_publication(&root); let prepared = captured(&root);
    let epoch = prepared.before.control.ledger.epoch + 1;
    let config_file = disk_config(&root, epoch); let config = Config::read(&config_file).unwrap();
    let profile_file = root.0.join("guarded-submit.json"); fs::write(&profile_file, &prepared.json).unwrap();
    let fa_reference::action::consequence::oversight::actor_wire::Command::Submit { request, mut proposal } =
        fa_reference::action::consequence::oversight::actor_wire::decode_command(
            &document(2, b"second", prepared.before.target, epoch)).unwrap() else { unreachable!() };
    proposal.deadline = ElapsedTick(workflow::clock().0 + 60_000);
    let original = encode_command(&Command::Submit { request, proposal }).unwrap();
    let document_file = root.0.join("submit.json"); fs::write(&document_file, &original).unwrap();
    let args = vec!["submit".into(), config_file.to_str().unwrap().into(), document_file.to_str().unwrap().into(),
        "--guarded".into(), profile_file.to_str().unwrap().into()];
    let human = reviewer(&config, 2); crate::command(args.clone()).unwrap(); human.join().unwrap();
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 2); assert_eq!(after.payload, b"second"); assert_eq!(after.control.ledger.charged, 11);
    assert_eq!(read_anchor(&prepared.output).revision(), after.revision);
    assert_eq!(fs::read(&document_file).unwrap(), original);
    let recovered = root.0.join("cli-receipt.anchor");
    fs::write(&profile_file, profile_json(&after, &prepared.output, Some(&recovered))).unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let missing = root.0.join("missing-qualification.json");
    let mut retry = args; retry.extend(["--credibility-activation".into(), missing.to_str().unwrap().into()]);
    crate::command(retry).unwrap();
    let repeated = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(repeated.executions, 2); assert_eq!(repeated.control.ledger.charged, 11);
    assert_eq!(repeated.revision, after.revision + 3); assert_eq!(read_anchor(&recovered).revision(), repeated.revision);
    assert!(!missing.exists()); assert!(!config.socket(2).exists());
    assert_eq!(fs::read(&prepared.input).unwrap(), prepared.bytes);
}

#[test]
fn command_rejects_ambiguous_flags_before_io_and_keeps_export_and_output_failures_together() {
    for tail in [vec!["--guarded"], vec!["--guarded", "profile", "--guarded", "other"],
        vec!["--guarded", "--reviewer-profile"], vec!["--guarded", "profile", "--unknown", "other"],
        vec!["--reviewer-profile", "peer"]] {
        let mut args = vec!["submit".to_owned(), "missing-config".into(), "missing-document".into()];
        args.extend(tail.into_iter().map(str::to_owned));
        assert!(submission::command(&args, None, &mut Vec::new()).unwrap_err().contains("usage:"));
    }
    struct Broken;
    impl io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { panic!("failed write cannot flush") }
    }
    let root = Directory::new(); let original = first_publication(&root); let prepared = captured(&root);
    let profile = SubmitProfile::decode(prepared.json.as_bytes()).unwrap();
    let mut config = configured(&root, 2); config.programs.clear(); let mut calls = 0;
    let result = submission::submit(config, Submission { document: &original, profile: &profile,
        peers: None, credibility: None }, || {
            calls += 1; if calls == 2 { fs::write(&prepared.output, b"retain winner").unwrap(); }
            ElapsedTick(1_005)
        }).unwrap();
    assert!(result.failure.as_ref().unwrap().contains("retention failed"));
    let before = files(&root);
    let error = submission::emit(result, &mut Broken).unwrap_err();
    assert!(error.contains("retention failed")); assert!(error.contains("response output failed"));
    assert_eq!(files(&root), before); assert_eq!(fs::read(&prepared.output).unwrap(), b"retain winner");
    assert_eq!(fs::read(&prepared.input).unwrap(), prepared.bytes);
}
