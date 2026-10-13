//! Actual first publication, private export and guarded continuation. Synthetic
//! helpers exercise enforcement, not model calibration or custodian identity.
use super::*;
use crate::workflow::continuation::{creation as initial, guarded as resumed};
use crate::recover_stop::guarded::{CreationOutput, SubmitProfile};
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements, FileRecoveryFloor,
    anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES}, recovery::FileGuardedRecoveryStatus};
use std::{fs, io::Cursor, path::Path, os::unix::fs::{PermissionsExt, symlink}};

fn create<F>(config: Config, doc: &[u8], output: &Path, clock: F) -> Result<workflow::RunResult, String>
where F: FnMut() -> ElapsedTick {
    initial::run(config, initial::Creation { document: doc, output, peers: None, publication: None }, clock)
}
fn load(path: &Path) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(&mut Cursor::new(fs::read(path).unwrap()), MAX_HISTORY_ANCHOR_BYTES).unwrap()
}
fn state(config: &Config) -> FileDeliverySnapshot {
    FileOversight::read_publication(&config.store, &config.profile).unwrap()
}
fn verify_anchor(config: &Config, output: &Path) -> FileDeliverySnapshot {
    let state = state(config); let anchor = load(output); let before = fs::read(config.store.join("delivery.bin")).unwrap();
    assert_eq!(anchor.revision(), state.revision); assert_eq!(anchor.retained_bytes(), before.len());
    assert_eq!(fs::metadata(output).unwrap().permissions().mode() & 0o077, 0);
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: state.revision,
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch },
    };
    // Matching the FULL independently exported prefix checks exact bytes with
    // the original journal codec, not just equal counters or archive lengths.
    let mut recovery = FileOversight::begin_open_guarded_anchored(&config.store,
        config.profile.clone(), &expected, &anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while recovery.progress().status == FileGuardedRecoveryStatus::Replaying {
        recovery.advance(recovery.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(recovery.cancel().status, FileGuardedRecoveryStatus::Cancelled);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    state
}
fn submit_profile(config: &Config, input: &Path, output: &Path) -> SubmitProfile {
    let state = state(config);
    SubmitProfile::decode(format!(r#"{{"schema_version":"fa.guarded-submit/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":"{}","retain_history_anchor":"{}"}}"#,
        state.revision, state.control.sequence, state.control.ledger.epoch, input.display(), output.display()).as_bytes()).unwrap()
}

#[test]
fn initial_export_bootstraps_successive_guarded_publication_and_offline_exact_retry() {
    let root = Directory::new(); let config = configured(&root, 0); evidence(&root, &config);
    let doc = document(1, b"first", config.profile.delivery.target, 0); let output = root.0.join("first.anchor");
    let human = reviewer(&config, 1);
    let result = create(config, &doc, &output, || ElapsedTick(1000)).unwrap();
    let packet = human.join().unwrap(); assert_executed(&result);
    assert_eq!(packet.action().spec().policy_epoch, 0, "creation must not add a recovery fence");
    let config = configured(&root, 1); let before = verify_anchor(&config, &output);
    assert_eq!(before.executions, 1); assert_eq!(before.control.ledger.charged, 5);
    let original_archive = fs::read(&output).unwrap(); let next = root.0.join("next.anchor");
    let profile = submit_profile(&config, &output, &next);
    let second = document(2, b"second", before.target, before.control.ledger.epoch + 1);
    let human = reviewer(&config, 2);
    let result = resumed::submit(config, resumed::Submission {
        document: &second, profile: &profile, peers: None, credibility: None }, || ElapsedTick(1001)).unwrap();
    let packet = human.join().unwrap(); assert_executed(&result);
    assert_eq!(packet.action().spec().payload, b"second");
    let config = configured(&root, 2); let after = verify_anchor(&config, &next);
    assert_eq!(after.executions, 2); assert_eq!(after.control.ledger.charged, 11);
    assert_eq!(fs::read(&output).unwrap(), original_archive);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut config = config; config.programs.clear(); let retry_output = root.0.join("retry.anchor");
    let profile = submit_profile(&config, &next, &retry_output);
    let result = resumed::submit(config, resumed::Submission {
        document: &second, profile: &profile, peers: None, credibility: None }, || ElapsedTick(200_000)).unwrap();
    assert_executed(&result);
    let after = verify_anchor(&configured(&root, 3), &retry_output);
    assert_eq!(after.executions, 2); assert_eq!(after.control.ledger.charged, 11);
}

#[test]
fn unavailable_first_source_retains_the_healthy_prefix_then_allows_corrected_guarded_work() {
    let root = Directory::new(); let config = configured(&root, 0);
    let doc = document(1, b"first", config.profile.delivery.target, 0); let output = root.0.join("withdrawn.anchor");
    assert!(create(config, &doc, &output, || ElapsedTick(1000)).is_err());
    let config = configured(&root, 1); let before = verify_anchor(&config, &output);
    assert_eq!(before.executions, 0); assert!(before.control.ledger.stages.is_empty());
    assert_eq!(before.control.ledger.charged, 0); assert!(!config.socket(1).exists());
    evidence(&root, &config); let next = root.0.join("corrected.anchor");
    let profile = submit_profile(&config, &output, &next);
    let corrected = document(1, b"first", before.target, before.control.ledger.epoch + 1);
    let human = reviewer(&config, 1);
    let result = resumed::submit(config, resumed::Submission {
        document: &corrected, profile: &profile, peers: None, credibility: None }, || ElapsedTick(1001)).unwrap();
    human.join().unwrap(); assert_executed(&result);
    assert_eq!(verify_anchor(&configured(&root, 2), &next).executions, 1);
}

#[test]
fn bad_output_custody_is_refused_before_the_first_store_or_clock() {
    let root = Directory::new(); let config = configured(&root, 0);
    let doc = document(1, b"first", config.profile.delivery.target, 0);
    let old = root.0.join("existing.anchor"); fs::write(&old, b"independent old archive").unwrap();
    let dangling = root.0.join("dangling.anchor"); symlink(root.0.join("missing"), &dangling).unwrap();
    for output in [old.clone(), dangling, root.0.clone(), PathBuf::from("relative.anchor"),
        root.0.join("absent-parent/new.anchor"), config.store.join("inside.anchor")] {
        assert!(create(configured(&root, 0), &doc, &output, || panic!("custody refusal precedes clock")).is_err());
        assert!(!config.store.exists()); assert_eq!(fs::read(&old).unwrap(), b"independent old archive");
    }
    let good = root.0.join("accepted.anchor");
    assert!(CreationOutput::admit(&config, &good).is_ok()); assert!(!good.exists()); assert!(!config.store.exists());
    fs::create_dir(&config.store).unwrap(); let alias = root.0.join("alias"); symlink(&config.store, &alias).unwrap();
    for output in [config.store.join("inside.anchor"), alias.join("inside.anchor")] {
        assert!(CreationOutput::admit(&config, &output).err().unwrap().contains("outside the authority store"));
    }
    assert_eq!(fs::read_dir(&config.store).unwrap().count(), 0);
}

#[test]
fn invalid_document_or_expired_original_deadline_never_creates_or_exports() {
    let root = Directory::new(); let output = root.0.join("not-created.anchor");
    for doc in [b"{}".to_vec(), encode_command(&Command::Poll { request: 1 }).unwrap()] {
        assert!(create(configured(&root, 0), &doc, &output, || panic!("parse before clock")).is_err());
        assert!(!output.exists()); assert!(!root.0.join("store").exists());
    }
    let config = configured(&root, 0); let doc = document(1, b"first", config.profile.delivery.target, 0);
    assert!(create(config, &doc, &output, || ElapsedTick(100_000)).err().unwrap().contains("deadline"));
    assert!(!output.exists()); assert!(!root.0.join("store").exists());
}

#[test]
fn a_late_archive_collision_keeps_the_original_executed_response_and_never_reopens_creation() {
    let root = Directory::new(); let config = configured(&root, 0); evidence(&root, &config);
    let doc = document(1, b"first", config.profile.delivery.target, 0); let output = root.0.join("race.anchor");
    let human = reviewer(&config, 1); let mut raced = false;
    let result = create(config, &doc, &output, || {
        // Admission has completed, but exclusive export must still check again.
        if !raced { fs::write(&output, b"another custodian").unwrap(); raced = true; }
        ElapsedTick(1000)
    }).unwrap();
    human.join().unwrap(); assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert!(result.failure.as_ref().unwrap().contains("history anchor retention failed"));
    let expected_response = result.response.encode(); let mut stdout = Vec::new();
    assert!(resumed::emit(result, &mut stdout).unwrap_err().contains("retention failed"));
    assert_eq!(stdout, [expected_response, vec![b'\n']].concat());
    assert_eq!(fs::read(&output).unwrap(), b"another custodian");
    let config = configured(&root, 0); let before = fs::read(config.store.join("delivery.bin")).unwrap();
    let replacement = root.0.join("must-not-recreate.anchor");
    assert!(create(config, &doc, &replacement, || ElapsedTick(1001)).is_err());
    assert!(!replacement.exists()); assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
    assert_eq!(state(&configured(&root, 0)).executions, 1);
}

struct Broken { flush: bool }
impl io::Write for Broken {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.flush { Ok(bytes.len()) } else { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
}
#[test]
fn lost_stdout_preserves_export_and_reports_simultaneous_retention_failure() {
    for collision in [false, true] {
        for flush in [false, true] {
            let root = Directory::new(); let config = configured(&root, 0); evidence(&root, &config);
            let doc = document(1, b"first", config.profile.delivery.target, 0); let output = root.0.join("reply.anchor");
            let human = reviewer(&config, 1); let mut raced = false;
            let result = create(config, &doc, &output, || {
                if collision && !raced { fs::write(&output, b"do not overwrite").unwrap(); raced = true; }
                ElapsedTick(1000)
            }).unwrap();
            human.join().unwrap(); let config = configured(&root, 0); let before = state(&config);
            let error = resumed::emit(result, &mut Broken { flush }).unwrap_err();
            assert!(error.contains("response output failed"));
            assert_eq!(error.contains("history anchor retention failed"), collision);
            assert_eq!(state(&config), before); assert_eq!(before.executions, 1);
            if collision { assert_eq!(fs::read(&output).unwrap(), b"do not overwrite"); }
            else { assert_eq!(verify_anchor(&config, &output), before); }
        }
    }
}

#[test]
fn creation_flags_are_closed_before_io_and_do_not_consume_recovery_intents() {
    let root = Directory::new(); let output = root.0.join("unwritten.anchor");
    let base = vec!["create".into(), "absent-config".into(), "absent-document".into(),
        "--retain-history-anchor".into(), output.to_str().unwrap().into()];
    for extra in [vec!["--retain-history-anchor", "/other"], vec!["--guarded", "/resume"],
        vec!["--history-anchor", "/old"], vec!["--credibility-activation", "/capsule"],
        vec!["--reviewer-profile"], vec!["--reviewer-profile", "--bad"]] {
        let mut args = base.clone(); args.extend(extra.into_iter().map(str::to_owned));
        assert!(crate::command(args).unwrap_err().contains("usage:")); assert!(!output.exists());
    }
    for mode in ["create", "create-checked"] {
        let mut args = vec![mode.into(), "absent-config".into(), "absent-document".into()];
        if mode == "create-checked" { args.push("absent-witnesses".into()); }
        args.extend(["--retain-history-anchor".into(), output.to_str().unwrap().into()]);
        // A valid grammar reaches the original config reader, not legacy usage.
        let error = crate::command(args).unwrap_err(); assert!(!error.contains("usage:"));
        assert!(!output.exists()); assert!(!root.0.join("store").exists());
    }
}

#[test]
fn stale_actor_epoch_is_not_repaired_and_its_refusal_still_has_independent_history() {
    let root = Directory::new(); let config = configured(&root, 0); evidence(&root, &config);
    let doc = document(1, b"not approved", config.profile.delivery.target, 99);
    let output = root.0.join("refused.anchor");
    let result = create(config, &doc, &output, || ElapsedTick(1000)).unwrap();
    assert!(!matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(result.cleanup_pending, 0);
    let config = configured(&root, 0); let observed = verify_anchor(&config, &output);
    assert_eq!(observed.executions, 0); assert_eq!(observed.control.ledger.charged, 0);
    assert_eq!(observed.control.ledger.reserved, 0); assert!(!config.socket(1).exists());
}

fn checked_capture(root: &Directory, config: &Config, phantom: bool) -> crate::publication::PublicationProfile {
    use fa_reference::action::{ActionSpec, VERSION};
    use fa_reference::action::consequence::delivery::persistent::observed::publication::{
        capture::{FileCaptureIdentity, FilePublicationCapture}, witnesses::{FilePublicationInputs, FileWitnessInput}};
    use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
    use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
    let (mut host, _) = FileOversight::create(root.0.join("capture-owner"), config.profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec {
        version: VERSION, scope: config.profile.delivery.scope, target: Some(config.profile.delivery.target),
        payload: b"checked".to_vec(), units: 7, required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100_000),
    }, Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }).unwrap();
    drop(host);
    for (name, generation, keys) in [("original.bin", 1, vec![0]),
        ("current.bin", 2, if phantom { vec![0, 1] } else { vec![0] })] {
        let key = ProjectionKey { source: 40, branch: action.spec().scope.branch, projection: 7, source_epoch: 1 };
        let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
        let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
        frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
        let structured = FileWitnessInput::new(generation, generation, 20,
            AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
            keys.into_iter().map(|key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap();
        let capture = FilePublicationCapture::new(1, FileCaptureIdentity { source: 91, generation }, &action,
            FilePublicationInputs::new(Some(structured), None)).unwrap();
        fs::write(root.0.join(name), capture.to_bytes().unwrap()).unwrap();
    }
    crate::publication::PublicationProfile::decode(format!(r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}/original.bin","current":"{}/current.bin","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), root.0.display()).as_bytes()).unwrap()
}

#[test]
fn checked_creation_retains_exact_history_without_bypassing_a_changed_negative_witness() {
    for phantom in [false, true] {
        let root = Directory::new(); let config = configured(&root, 0); evidence(&root, &config);
        let profile = checked_capture(&root, &config, phantom);
        let doc = document(1, b"checked", config.profile.delivery.target, 0);
        let output = root.0.join("checked.anchor"); let human = reviewer(&config, 1);
        let result = initial::run(config, initial::Creation { document: &doc, output: &output,
            peers: None, publication: Some(&profile) }, || ElapsedTick(1000)).unwrap();
        human.join().unwrap(); assert_eq!(result.cleanup_pending, 0);
        assert_eq!(matches!(&result.response.result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), !phantom);
        let config = configured(&root, 0); let observed = verify_anchor(&config, &output);
        assert_eq!(observed.executions, u64::from(!phantom));
        assert_eq!(observed.control.ledger.charged, if phantom { 0 } else { 7 });
        let (host, _) = FileOversight::open_with_publication_validation(
            &config.store, config.profile, profile.limits).unwrap();
        assert_eq!(host.publication_validation_profile().unwrap(), Some(profile.limits));
        assert_eq!(host.retained_publication_evidence(1).unwrap().requests().len(), 1);
    }
}

#[test]
fn a_poisoned_creation_owner_cannot_export_old_memory_as_an_acknowledged_cut() {
    let root = Directory::new(); let config = configured(&root, 0); let output = root.0.join("poisoned.anchor");
    let admitted = CreationOutput::admit(&config, &output).unwrap();
    let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.enable_file_source(host.revision(), config.source_policy).unwrap();
    // A real failing native staging operation, not an injected successful result.
    fs::create_dir(config.store.join("delivery.pending")).unwrap();
    assert!(host.observe_time(host.revision(), ElapsedTick(1000)).is_err());
    assert!(host.storage_failure().is_some());
    assert!(admitted.retain(&host).unwrap_err().contains("retention failed")); assert!(!output.exists());
    drop(host); fs::remove_dir(config.store.join("delivery.pending")).unwrap();
    let (healthy, _) = FileOversight::open(&config.store, config.profile.clone()).unwrap();
    admitted.retain(&healthy).unwrap();
    assert_eq!(load(&output).revision(), healthy.revision());
    assert_eq!(healthy.inspect().executions, 0);
}

fn cli_config(root: &Directory) -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let mut text = String::from_utf8(TEMPLATE.to_vec()).unwrap()
        .replace("/operator/publication-store", root.0.join("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap())
        .replace("/operator/helpers/alpha", exe.to_str().unwrap())
        .replace("/operator/helpers/beta", exe.to_str().unwrap())
        .replace("/operator/helpers", root.0.to_str().unwrap())
        .replace("\"arguments\": []", &format!("\"arguments\": [\"--exact\",\"{CHILD}\",\"--nocapture\"]"));
    for member in ["alpha", "beta"] {
        text = text.replacen("\"environment\": {}", &format!(
            "\"environment\": {{\"{MEMBER}\":\"{member}\",\"{EPOCH}\":\"0\"}}"), 1);
    }
    let path = root.0.join("config.json"); fs::write(&path, text).unwrap(); path
}

#[test]
fn actual_create_command_exports_the_first_anchor_without_library_only_custody_setup() {
    let root = Directory::new(); let path = cli_config(&root); let config = Config::read(&path).unwrap();
    evidence(&root, &config); let output = root.0.join("cli-first.anchor");
    let doc = encode_command(&Command::Submit { request: 1, proposal: ActorProposal {
        target: config.profile.delivery.target, payload: b"first".to_vec(), units: 5,
        deadline: ElapsedTick(workflow::clock().0.checked_add(90_000).unwrap()), expected_policy_epoch: 0,
    }}).unwrap();
    let doc_path = root.0.join("submit.json"); fs::write(&doc_path, &doc).unwrap();
    let human = reviewer(&config, 1);
    crate::command(vec!["create".into(), path.to_str().unwrap().into(), doc_path.to_str().unwrap().into(),
        "--retain-history-anchor".into(), output.to_str().unwrap().into()]).unwrap();
    human.join().unwrap(); let before = verify_anchor(&config, &output);
    assert_eq!(before.executions, 1); assert_eq!(before.control.ledger.charged, 5);
    let archive = fs::read(&output).unwrap(); fs::remove_file(root.0.join("evidence.json")).unwrap();
    let next = root.0.join("cli-receipt.anchor");
    let receipt = root.0.join("receipt.json");
    fs::write(&receipt, format!(r#"{{"schema_version":"fa.guarded-resume/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":"{}","retain_history_anchor":"{}"}}"#,
        before.revision, before.control.sequence, before.control.ledger.epoch, output.display(), next.display())).unwrap();
    crate::command(vec!["resume".into(), path.to_str().unwrap().into(), doc_path.to_str().unwrap().into(),
        "--guarded".into(), receipt.to_str().unwrap().into()]).unwrap();
    let after = verify_anchor(&config, &next); assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, 5); assert_eq!(fs::read(&output).unwrap(), archive);
}
