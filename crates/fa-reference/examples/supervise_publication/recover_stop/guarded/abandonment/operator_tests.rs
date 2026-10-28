//! The original source-publication fixture, offline disposition and output custody.
//! Helpers run only while constructing the real dispatched Unknown predecessor.
use super::*;
use crate::recover_stop::guarded::abandonment::{self, tests::run as abandon};
use fa_reference::action::consequence::delivery::persistent::FileAbandonRequest;

fn unknown_case(root: &Directory) -> Case {
    let (config, _) = publish(root, true);
    let (host, _) = FileOversight::open(&config.store, config.profile.clone()).unwrap();
    let case = capture(config, host, root);
    assert_eq!(case.before.control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(case.before.control.ledger.charged, 4);
    assert_eq!(case.before.executions, 0);
    assert!(case.config.programs.is_empty());
    assert!(!root.0.join("evidence.json").exists());
    case
}

fn instruction(case: &Case) -> FileAbandonRequest {
    FileAbandonRequest {
        operation: 71,
        attempt: 1,
        expected_control_sequence: case.before.control.sequence,
        expected_authority_epoch: case.before.control.ledger.epoch,
    }
}

fn profile_document(case: &Case, input: &Path, target: &Path) -> Vec<u8> {
    String::from_utf8(document(case.floor, input, Some(target)))
        .unwrap()
        .replace("fa.guarded-stop/1", "fa.guarded-abandon/1")
        .into_bytes()
}

fn selected_profile(case: &Case, target: &Path) -> Vec<u8> {
    profile_document(case, &case.anchor, target)
}

fn assert_report(stdout: &[u8], revision: u64, retained: bool) {
    let json = output(stdout);
    assert_eq!(
        json.get("schema").unwrap().as_str(),
        Some("fa.abandon-unknown-result/1")
    );
    let known = json.get("knowledge").unwrap();
    assert_eq!(known.get("kind").unwrap().as_str(), Some("known"));
    let basis = known.get("basis").unwrap();
    assert_eq!(basis.get("class").unwrap().as_str(), Some("bounded_model"));
    assert_eq!(
        basis.get("scope").unwrap().as_str(),
        Some("acknowledged_reference_disposition")
    );
    assert_eq!(
        basis.get("authentic_origin").unwrap().as_str(),
        Some("unknown")
    );
    assert_eq!(
        basis.get("source_truth").unwrap().as_str(),
        Some("not_established")
    );
    assert_eq!(
        basis.get("verified_journal_revision").unwrap().as_str(),
        Some(revision.to_string().as_str())
    );
    let value = known.get("value").unwrap();
    assert_eq!(
        value.get("status").unwrap().as_str(),
        Some("irrecoverably_unknown")
    );
    assert_eq!(
        value.get("disposition_recorded").unwrap().as_bool(),
        Some(true)
    );
    assert_eq!(
        value
            .get("effect_outcome")
            .unwrap()
            .get("kind")
            .unwrap()
            .as_str(),
        Some("unknown")
    );
    assert_eq!(
        value
            .get("effect_outcome")
            .unwrap()
            .get("reason")
            .unwrap()
            .as_str(),
        Some("OutcomeUnknown")
    );
    assert_eq!(value.get("operation").unwrap().as_str(), Some("71"));
    assert_eq!(value.get("attempt").unwrap().as_str(), Some("1"));
    assert_eq!(
        value.get("revision").unwrap().as_str(),
        Some(revision.to_string().as_str())
    );
    assert_eq!(value.get("charged").unwrap().as_str(), Some("4"));
    assert_eq!(value.get("abandoned_units").unwrap().as_str(), Some("4"));
    assert_eq!(value.get("executions").unwrap().as_str(), Some("0"));
    assert_eq!(
        value.get("history_anchor_retained").unwrap().as_bool(),
        Some(retained)
    );
    let ids = |name: &str| {
        value
            .get(name)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("unresolved"), vec!["1"]);
    assert_eq!(ids("irrecoverable"), vec!["1"]);
    assert!(!String::from_utf8_lossy(stdout).contains("review this fixture"));
    assert!(!String::from_utf8_lossy(stdout).contains("once"));
}

fn assert_disposed(case: &Case) -> FileDeliverySnapshot {
    let after = FileOversight::read_publication(&case.config.store, &case.config.profile).unwrap();
    assert_eq!(after.revision, case.before.revision + 1);
    assert_eq!(
        after.control.ledger.stages[&1],
        ActionState::IrrecoverablyUnknown
    );
    assert_eq!(
        after.control.ledger.available,
        case.before.control.ledger.available
    );
    assert_eq!(
        after.control.ledger.reserved,
        case.before.control.ledger.reserved
    );
    assert_eq!(after.control.ledger.charged, 4);
    assert_eq!(after.control.sequence, case.before.control.sequence);
    assert_eq!(after.control.ledger.epoch, case.before.control.ledger.epoch);
    assert_eq!(after.payload, case.before.payload);
    assert_eq!(after.executions, 0);
    assert_eq!(after.stop, case.before.stop);
    after
}

#[test]
fn offline_operator_disposes_a_real_expired_unknown_without_live_source_or_helpers() {
    let root = Directory::new();
    let case = unknown_case(&root);
    let target = root.0.join("abandoned.anchor");
    let request = instruction(&case);
    let reads = case.config.source.status().read_attempts;
    let mut stdout = Vec::new();
    abandon(
        &case.config,
        &selected_profile(&case, &target),
        request,
        &mut || Duration::ZERO,
        &mut stdout,
    )
    .unwrap();
    let after = assert_disposed(&case);
    assert_report(&stdout, after.revision, true);
    assert_eq!(read_anchor(&target).revision(), after.revision);
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o077,
        0
    );
    let original = files(&case.config);
    let retry = root.0.join("abandoned-retry.anchor");
    stdout.clear();
    abandon(
        &case.config,
        &profile_document(&case, &target, &retry),
        request,
        &mut || Duration::ZERO,
        &mut stdout,
    )
    .unwrap();
    assert_report(&stdout, after.revision, true);
    assert_eq!(files(&case.config), original);
    assert_eq!(fs::read(&retry).unwrap(), fs::read(&target).unwrap());
    assert_eq!(case.config.source.status().read_attempts, reads);
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    assert!(!case.config.socket(1).exists());
}

#[test]
fn mandatory_output_custody_rejects_existing_and_authority_local_targets_before_cleanup() {
    let root = Directory::new();
    let case = unknown_case(&root);
    fs::write(
        case.config.store.join("delivery.pending"),
        b"preserve pending candidate",
    )
    .unwrap();
    let alias = root.0.join("store-alias");
    symlink(&case.config.store, &alias).unwrap();
    let dangling = root.0.join("dangling.anchor");
    symlink(root.0.join("absent"), &dangling).unwrap();
    let directory = root.0.join("existing-directory");
    fs::create_dir(&directory).unwrap();
    let targets = [
        case.anchor.clone(),
        case.config.store.join("not-independent.anchor"),
        alias.join("aliased-inside.anchor"),
        dangling.clone(),
        directory,
    ];
    let before = files(&case.config);
    let reads = case.config.source.status().read_attempts;
    for target in targets {
        let mut stdout = Vec::new();
        let error = abandon(
            &case.config,
            &selected_profile(&case, &target),
            instruction(&case),
            &mut || Duration::ZERO,
            &mut stdout,
        )
        .unwrap_err();
        assert!(
            error.contains("outside the authority store") || error.contains("must be a new file")
        );
        assert!(stdout.is_empty());
        assert_eq!(files(&case.config), before);
    }
    assert!(
        fs::symlink_metadata(&dangling)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    assert_eq!(case.config.source.status().read_attempts, reads);
}

#[test]
fn source_floors_and_immutable_request_predecessors_refuse_before_any_disposition() {
    let root = Directory::new();
    let mut case = unknown_case(&root);
    let request = instruction(&case);
    let target = root.0.join("guarded-disposition.anchor");
    fs::write(
        case.config.store.join("delivery.pending"),
        b"preserve on guard refusal",
    )
    .unwrap();
    let before = files(&case.config);
    let reads = case.config.source.status().read_attempts;
    let original_source = case.config.source_policy;
    case.config.source_policy.source.generation += 1;
    let mut stdout = Vec::new();
    let error = abandon(
        &case.config,
        &selected_profile(&case, &target),
        request,
        &mut || Duration::ZERO,
        &mut stdout,
    )
    .unwrap_err();
    assert!(error.contains("Binding"));
    assert!(stdout.is_empty());
    assert!(!target.exists());
    assert_eq!(files(&case.config), before);
    case.config.source_policy = original_source;

    let original_floor = case.floor;
    for dimension in 0..3 {
        case.floor = original_floor;
        match dimension {
            0 => case.floor.journal_revision = u64::MAX,
            1 => case.floor.control_sequence = u64::MAX,
            _ => case.floor.authority_epoch = u64::MAX,
        }
        let error = abandon(
            &case.config,
            &selected_profile(&case, &target),
            request,
            &mut || Duration::ZERO,
            &mut stdout,
        )
        .unwrap_err();
        assert!(error.contains("Stale"));
        assert!(stdout.is_empty());
        assert!(!target.exists());
        assert_eq!(files(&case.config), before);
    }
    case.floor = original_floor;
    for wrong in [
        FileAbandonRequest {
            expected_control_sequence: request.expected_control_sequence + 1,
            ..request
        },
        FileAbandonRequest {
            expected_authority_epoch: request.expected_authority_epoch + 1,
            ..request
        },
    ] {
        let error = abandon(
            &case.config,
            &selected_profile(&case, &target),
            wrong,
            &mut || Duration::ZERO,
            &mut stdout,
        )
        .unwrap_err();
        assert!(error.contains("Stale"));
        assert!(stdout.is_empty());
        assert!(!target.exists());
        assert_eq!(files(&case.config), before);
    }
    abandon(
        &case.config,
        &selected_profile(&case, &target),
        request,
        &mut || Duration::ZERO,
        &mut stdout,
    )
    .unwrap();
    let after = assert_disposed(&case);
    assert_report(&stdout, after.revision, true);
    assert_eq!(read_anchor(&target).revision(), after.revision);
    assert_eq!(case.config.source.status().read_attempts, reads);
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
    assert!(!case.config.socket(1).exists());
}

#[test]
fn every_preacknowledgment_elapsed_cut_keeps_the_original_journal_and_pending_file() {
    let root = Directory::new();
    let mut case = unknown_case(&root);
    case.config.timing.runtime_ms = 50;
    let target = root.0.join("deadline.anchor");
    let profile = selected_profile(&case, &target);
    fs::write(
        case.config.store.join("delivery.pending"),
        b"preserve on deadline",
    )
    .unwrap();
    let before = files(&case.config);
    let reads = case.config.source.status().read_attempts;
    let limit = Duration::from_millis(50);
    let below = limit.checked_sub(Duration::from_nanos(1)).unwrap();
    let events = case.before.revision as usize;
    // Before custody, before opening, then each original event and final Ready.
    for cut in 0..=events + 2 {
        let mut calls = 0;
        let mut stdout = Vec::new();
        let error = abandon(
            &case.config,
            &profile,
            instruction(&case),
            &mut || {
                let now = calls;
                calls += 1;
                if now == cut { limit } else { below }
            },
            &mut stdout,
        )
        .unwrap_err();
        assert!(error.contains("deadline reached before acknowledgment"));
        assert_eq!(calls, cut + 1);
        assert!(stdout.is_empty());
        assert!(!target.exists());
        assert_eq!(files(&case.config), before);
    }
    assert_eq!(case.config.source.status().read_attempts, reads);
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
}

#[test]
fn lateness_after_native_ack_retention_or_report_still_retains_and_reports_the_same_disposition() {
    for checkpoint in 0..3 {
        let root = Directory::new();
        let mut case = unknown_case(&root);
        case.config.timing.runtime_ms = 50;
        let target = root.0.join("late.anchor");
        let request = instruction(&case);
        let reads = case.config.source.status().read_attempts;
        let first_after_ack = case.before.revision as usize + 3;
        let limit = Duration::from_millis(50);
        let mut calls = 0;
        let mut stdout = Vec::new();
        let error = abandon(
            &case.config,
            &selected_profile(&case, &target),
            request,
            &mut || {
                let now = calls;
                calls += 1;
                if now == first_after_ack + checkpoint {
                    limit
                } else {
                    Duration::ZERO
                }
            },
            &mut stdout,
        )
        .unwrap_err();
        assert!(error.contains("native abandonment acknowledged"));
        assert!(error.contains("deadline reached after native acknowledgment"));
        assert_eq!(calls, first_after_ack + 3);
        let after = assert_disposed(&case);
        assert_report(&stdout, after.revision, true);
        assert_eq!(read_anchor(&target).revision(), after.revision);
        let before_retry = files(&case.config);
        let retry = root.0.join("late-retry.anchor");
        stdout.clear();
        abandon(
            &case.config,
            &selected_profile(&case, &retry),
            request,
            &mut || Duration::ZERO,
            &mut stdout,
        )
        .unwrap();
        assert_report(&stdout, after.revision, true);
        assert_eq!(files(&case.config), before_retry);
        assert_eq!(fs::read(&retry).unwrap(), fs::read(&target).unwrap());
        assert_eq!(case.config.source.status().read_attempts, reads);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        assert!(!case.config.socket(1).exists());
    }
}

struct LostStdout {
    at_flush: bool,
}
impl Write for LostStdout {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.at_flush {
            Ok(bytes.len())
        } else {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::ErrorKind::BrokenPipe.into())
    }
}

#[test]
fn lost_stdout_retains_native_disposition_and_exact_retry_exports_without_another_event() {
    for at_flush in [false, true] {
        let root = Directory::new();
        let case = unknown_case(&root);
        let target = root.0.join("report-lost.anchor");
        let request = instruction(&case);
        let reads = case.config.source.status().read_attempts;
        let error = abandon(
            &case.config,
            &selected_profile(&case, &target),
            request,
            &mut || Duration::ZERO,
            &mut LostStdout { at_flush },
        )
        .unwrap_err();
        assert!(error.contains("native abandonment acknowledged"));
        assert!(error.contains("report output failed"));
        let after = assert_disposed(&case);
        assert_eq!(read_anchor(&target).revision(), after.revision);
        let before_retry = files(&case.config);
        let retry = root.0.join("report-retry.anchor");
        let mut stdout = Vec::new();
        abandon(
            &case.config,
            &profile_document(&case, &target, &retry),
            request,
            &mut || Duration::ZERO,
            &mut stdout,
        )
        .unwrap();
        assert_report(&stdout, after.revision, true);
        assert_eq!(files(&case.config), before_retry);
        assert_eq!(fs::read(&retry).unwrap(), fs::read(&target).unwrap());
        let conflict = FileAbandonRequest {
            attempt: 2,
            ..request
        };
        let rejected = root.0.join("conflict.anchor");
        assert!(
            abandon(
                &case.config,
                &selected_profile(&case, &rejected),
                conflict,
                &mut || Duration::ZERO,
                &mut Vec::new()
            )
            .unwrap_err()
            .contains("Binding")
        );
        assert!(!rejected.exists());
        assert_eq!(files(&case.config), before_retry);
        assert_eq!(case.config.source.status().read_attempts, reads);
        assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
        assert!(!case.config.socket(1).exists());
    }
}

#[test]
fn an_export_race_reports_unretained_acknowledgment_and_exact_retry_preserves_the_competing_file() {
    let root = Directory::new();
    let case = unknown_case(&root);
    let target = root.0.join("competing.anchor");
    let request = instruction(&case);
    let reads = case.config.source.status().read_attempts;
    let after_ack = case.before.revision as usize + 3;
    let mut calls = 0;
    let mut stdout = Vec::new();
    let error = abandon(
        &case.config,
        &selected_profile(&case, &target),
        request,
        &mut || {
            let now = calls;
            calls += 1;
            if now == after_ack {
                fs::write(&target, b"other custodian").unwrap();
            }
            Duration::ZERO
        },
        &mut stdout,
    )
    .unwrap_err();
    assert!(error.contains("native abandonment acknowledged"));
    assert!(error.contains("history anchor retention failed"));
    let after = assert_disposed(&case);
    assert_report(&stdout, after.revision, false);
    assert_eq!(fs::read(&target).unwrap(), b"other custodian");
    let before_retry = files(&case.config);
    let retry = root.0.join("retained-after-race.anchor");
    stdout.clear();
    abandon(
        &case.config,
        &selected_profile(&case, &retry),
        request,
        &mut || Duration::ZERO,
        &mut stdout,
    )
    .unwrap();
    assert_report(&stdout, after.revision, true);
    assert_eq!(files(&case.config), before_retry);
    assert_eq!(read_anchor(&retry).revision(), after.revision);
    assert_eq!(fs::read(&target).unwrap(), b"other custodian");
    assert_eq!(case.config.source.status().read_attempts, reads);
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
}

#[test]
fn actual_abandonment_command_parses_external_request_and_guards_before_the_single_native_event() {
    let root = Directory::new();
    let case = unknown_case(&root);
    let request = instruction(&case);
    let config = root.0.join("config.json");
    let text = include_str!("../../../../../fixtures/supervised_publication.json")
        .replace(
            "/operator/publication-store",
            case.config.store.to_str().unwrap(),
        )
        .replace(
            "/operator/evidence.json",
            root.0.join("evidence.json").to_str().unwrap(),
        );
    fs::write(&config, text).unwrap();
    let parsed = Config::read(&config).unwrap();
    crate::config::assert_oversight_profiles_eq(&parsed.profile, &case.config.profile);
    let request_path = root.0.join("abandon.json");
    fs::write(
        &request_path,
        format!(
            concat!(
                "{{\"schema_version\":\"fa.abandon-unknown/1\",\"operation\":{},\"attempt\":{},",
                "\"expected_control_sequence\":{},\"expected_authority_epoch\":{}}}"
            ),
            request.operation,
            request.attempt,
            request.expected_control_sequence,
            request.expected_authority_epoch
        ),
    )
    .unwrap();
    let target = root.0.join("cli-abandoned.anchor");
    let profile_path = root.0.join("abandon-profile.json");
    fs::write(&profile_path, selected_profile(&case, &target)).unwrap();
    let args = vec![
        "abandon-unknown".into(),
        config.to_str().unwrap().into(),
        request_path.to_str().unwrap().into(),
        "--guarded".into(),
        profile_path.to_str().unwrap().into(),
    ];
    let mut stdout = Vec::new();
    abandonment::command(&args, &mut stdout).unwrap();
    let after = assert_disposed(&case);
    assert_report(&stdout, after.revision, true);
    assert_eq!(read_anchor(&target).revision(), after.revision);
    let completed = files(&case.config);
    let retry = root.0.join("cli-abandoned-retry.anchor");
    fs::write(&profile_path, profile_document(&case, &target, &retry)).unwrap();
    // Exercise the executable's early routing, including its closed grammar,
    // without repeating the acknowledged native event.
    crate::command(args).unwrap();
    assert_eq!(files(&case.config), completed);
    assert_eq!(fs::read(&retry).unwrap(), fs::read(&target).unwrap());
    assert!(!root.0.join("evidence.json").exists());
    assert!(!case.config.socket(1).exists());
    assert_eq!(fs::read(&case.anchor).unwrap(), case.archive);
}
