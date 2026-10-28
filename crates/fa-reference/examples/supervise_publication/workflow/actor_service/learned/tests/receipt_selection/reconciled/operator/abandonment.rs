//! The real learned operator resolves only original Unknown liabilities and
//! independently retains the acknowledged cut. No fixture substitutes a receipt.
use super::*;
use crate::workflow::actor_service::learned::{abandonment as abandon, anchors as anchor_files};
use fa_reference::action::consequence::delivery::persistent::abandonment::FileAbandonRequest;
use fa_reference::strict_json::{self, Json, Limits};

fn request(case: &Sent) -> FileAbandonRequest {
    let state = state(case);
    FileAbandonRequest {
        operation: 900,
        attempt: 1,
        expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch,
    }
}

fn request_file(root: &Root, request: FileAbandonRequest) -> PathBuf {
    let path = root.0.join("abandon-request.json");
    fs::write(&path, format!(
        r#"{{"schema_version":"fa.abandon-unknown/1","operation":{},"attempt":{},"expected_control_sequence":{},"expected_authority_epoch":{}}}"#,
        request.operation, request.attempt, request.expected_control_sequence, request.expected_authority_epoch
    )).unwrap();
    path
}

fn args(root: &Root, case: &Sent, output: &Path) -> Vec<String> {
    vec![
        "create-learned-generated".into(),
        fixture::config_file(root, &case.config)
            .display()
            .to_string(),
        case.path.display().to_string(),
        "--abandon-unknown".into(),
        request_file(root, request(case)).display().to_string(),
        "--history-anchor".into(),
        fixture::anchor_file(root, case.loaded.recovery_anchor.as_ref().unwrap())
            .display()
            .to_string(),
        "--retain-history-anchor".into(),
        output.display().to_string(),
    ]
}

fn value(bytes: &[u8]) -> Json {
    let parsed = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(
        parsed.get("schema").unwrap().as_str(),
        Some("fa.learned-abandon-unknown-result/1")
    );
    assert_eq!(parsed.get("authority").unwrap().as_str(), Some("none"));
    let knowledge = parsed.get("knowledge").unwrap();
    assert_eq!(knowledge.get("kind").unwrap().as_str(), Some("known"));
    let value = knowledge.get("value").unwrap();
    assert_eq!(
        value.get("status").unwrap().as_str(),
        Some("irrecoverably_unknown")
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
    value.clone()
}

fn assert_disposition(
    case: &Sent,
    before: &fa_reference::action::consequence::delivery::persistent::FileDeliverySnapshot,
) {
    let current = state(case);
    assert_eq!(current.revision, before.revision + 1);
    assert_eq!(current.control.sequence, before.control.sequence);
    assert_eq!(current.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(
        current.control.ledger.elapsed,
        before.control.ledger.elapsed
    );
    assert_eq!(current.dispatcher_epoch, before.dispatcher_epoch);
    assert_eq!(
        current.control.ledger.charged,
        before.control.ledger.charged
    );
    assert_eq!(
        current.control.ledger.available,
        before.control.ledger.available
    );
    assert_eq!(
        current.control.ledger.reserved,
        before.control.ledger.reserved
    );
    assert_eq!(
        current.control.ledger.stages[&1],
        ActionState::IrrecoverablyUnknown
    );
    assert_eq!(current.executions, before.executions);
    assert_eq!(current.payload, before.payload);
    assert!(matches!(
        query::read(&case.config, &case.loaded, None, &mut || Duration::ZERO)
            .unwrap()
            .result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown
        })
    ));
}

#[test]
fn learned_abandonment_retains_one_native_disposition_in_all_predictor_modes_without_live_assets() {
    for mode in [None, Some(false), Some(true)] {
        for published in [false, true] {
            let root = Root::new();
            let case = super::super::unknown(&root, mode, published);
            let before = state(&case);
            assert_eq!(before.control.ledger.stages[&1], ActionState::Unknown);
            let output = root.0.join("abandoned.anchor");
            let args = args(&root, &case, &output);
            let input = fs::read(&args[6]).unwrap();
            remove_live_assets(&root);
            let mut report = Vec::new();
            abandon::command_with(&args, None, &mut || Duration::ZERO, &mut report).unwrap();
            assert_disposition(&case, &before);
            assert_eq!(
                anchor_files::load(&output).unwrap().revision(),
                before.revision + 1
            );
            assert_eq!(fs::read(&args[6]).unwrap(), input);
            let parsed = value(&report);
            assert_eq!(
                parsed.get("history_anchor_retained").unwrap().as_bool(),
                Some(true)
            );
            assert!(
                parsed
                    .get("topology_checkpoint_retained")
                    .unwrap()
                    .is_null()
            );
            assert_eq!(
                parsed.get("executions").unwrap().as_str(),
                Some(if published { "1" } else { "0" })
            );
            let canonical = files(&case.config.store);
            let mut retry = args.clone();
            retry[8] = root.0.join("retried.anchor").display().to_string();
            let mut retried = Vec::new();
            abandon::command_with(&retry, None, &mut || Duration::ZERO, &mut retried).unwrap();
            assert_eq!(files(&case.config.store), canonical);
            assert_eq!(retried, report);
            assert_eq!(
                fs::read(retry[8].as_str()).unwrap(),
                fs::read(output).unwrap()
            );
        }
    }
}

#[test]
fn learned_abandonment_rejects_changed_predictor_predecessor_and_nonunknown_without_cleanup() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let case = super::super::unknown(&root, mode, false);
        let output = root.0.join("refused.anchor");
        let args = args(&root, &case, &output);
        let exact = request(&case);
        remove_live_assets(&root);
        fs::write(
            case.config.store.join("delivery.pending"),
            b"diagnostic staging",
        )
        .unwrap();
        let before = files(&case.config.store);
        for wrong in [
            FileAbandonRequest {
                attempt: 2,
                ..exact
            },
            FileAbandonRequest {
                expected_control_sequence: exact.expected_control_sequence + 1,
                ..exact
            },
            FileAbandonRequest {
                expected_authority_epoch: exact.expected_authority_epoch + 1,
                ..exact
            },
        ] {
            request_file(&root, wrong);
            let mut report = Vec::new();
            assert!(
                abandon::command_with(&args, None, &mut || Duration::ZERO, &mut report).is_err()
            );
            assert!(report.is_empty());
            assert!(!output.exists());
            assert_eq!(files(&case.config.store), before);
        }
        request_file(&root, exact);
        if mode.is_some() {
            let path = root.0.join("forecast.json");
            let original = fs::read_to_string(&path).unwrap();
            assert!(original.contains("1065353216,2147483648"));
            fs::write(
                &path,
                original.replace("1065353216,2147483648", "1065353216,0"),
            )
            .unwrap();
            let changed = recipe::load(&case.path, &case.config, false).unwrap();
            assert_ne!(changed.generation, case.loaded.generation);
            assert!(
                abandon::command_with(&args, None, &mut || Duration::ZERO, &mut Vec::new())
                    .is_err()
            );
            assert_eq!(files(&case.config.store), before);
            assert!(!output.exists());
            fs::write(path, original).unwrap();
        }
        abandon::command_with(&args, None, &mut || Duration::ZERO, &mut Vec::new()).unwrap();
        assert!(!case.config.store.join("delivery.pending").exists());
    }
    let root = Root::new();
    let case = sent(&root, None, false);
    let output = root.0.join("dispatching.anchor");
    let args = args(&root, &case, &output);
    fs::write(
        case.config.store.join("delivery.pending"),
        b"dispatching must remain",
    )
    .unwrap();
    let before = files(&case.config.store);
    assert!(abandon::command_with(&args, None, &mut || Duration::ZERO, &mut Vec::new()).is_err());
    assert_eq!(files(&case.config.store), before);
    assert!(!output.exists());
}

#[test]
fn learned_abandonment_expiry_before_ready_preserves_files_and_late_expiry_retains_acknowledgment()
{
    let root = Root::new();
    let case = super::super::unknown(&root, None, false);
    let output = root.0.join("late.anchor");
    let args = args(&root, &case, &output);
    remove_live_assets(&root);
    fs::write(
        case.config.store.join("delivery.pending"),
        b"deadline staging",
    )
    .unwrap();
    let before = files(&case.config.store);
    for expire_at in [
        0,
        1,
        case.revision as usize / 2 + 1,
        case.revision as usize + 1,
    ] {
        let mut calls = 0;
        let result = recovery::abandon_selected_with_checkpoints(
            &case.config,
            &case.loaded,
            request(&case),
            &mut |_| {
                let at = calls;
                calls += 1;
                assert_eq!(files(&case.config.store), before);
                if at == expire_at {
                    Err("test replay deadline".into())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err(), "test replay deadline");
        assert_eq!(calls, expire_at + 1);
        assert_eq!(files(&case.config.store), before);
    }
    let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let mut report = Vec::new();
    let limit = Duration::from_secs(3600);
    let error = abandon::command_with(
        &args,
        None,
        &mut || {
            if fs::read(case.config.store.join("delivery.bin")).unwrap() == canonical {
                Duration::ZERO
            } else {
                limit
            }
        },
        &mut report,
    )
    .unwrap_err();
    assert!(error.contains(&format!("acknowledged at revision {}", case.revision + 1)));
    assert!(error.contains("deadline reached after native acknowledgment"));
    assert_eq!(
        anchor_files::load(&output).unwrap().revision(),
        case.revision + 1
    );
    assert_eq!(
        value(&report)
            .get("history_anchor_retained")
            .unwrap()
            .as_bool(),
        Some(true)
    );
}

#[test]
fn learned_abandonment_export_race_and_stdout_failure_remain_exactly_retryable() {
    let root = Root::new();
    let case = super::super::unknown(&root, Some(true), true);
    let output = root.0.join("raced.anchor");
    let mut args = args(&root, &case, &output);
    let input = fs::read(&args[6]).unwrap();
    remove_live_assets(&root);
    let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let mut raced = false;
    let mut report = Vec::new();
    let error = abandon::command_with(
        &args,
        None,
        &mut || {
            if !raced && fs::read(case.config.store.join("delivery.bin")).unwrap() != canonical {
                fs::write(&output, b"independent winner").unwrap();
                raced = true;
            }
            Duration::ZERO
        },
        &mut report,
    )
    .unwrap_err();
    assert!(error.contains(&format!("acknowledged at revision {}", case.revision + 1)));
    assert_eq!(fs::read(&output).unwrap(), b"independent winner");
    assert_eq!(
        value(&report)
            .get("history_anchor_retained")
            .unwrap()
            .as_bool(),
        Some(false)
    );
    assert_eq!(fs::read(&args[6]).unwrap(), input);
    let abandoned = files(&case.config.store);
    args[8] = root.0.join("retry.anchor").display().to_string();
    struct Broken {
        retained: PathBuf,
    }
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            assert!(
                self.retained.exists(),
                "report preceded independent retention"
            );
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed stdout"))
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("failed write must not flush")
        }
    }
    let error = abandon::command_with(
        &args,
        None,
        &mut || Duration::ZERO,
        &mut Broken {
            retained: PathBuf::from(&args[8]),
        },
    )
    .unwrap_err();
    assert!(error.contains(&format!("acknowledged at revision {}", case.revision + 1)));
    assert!(error.contains("report output failed"));
    assert_eq!(files(&case.config.store), abandoned);
    assert_eq!(
        anchor_files::load(Path::new(&args[8])).unwrap().revision(),
        case.revision + 1
    );
}

#[test]
fn learned_abandonment_closed_grammar_rejects_stripped_flags_before_missing_config() {
    let base: Vec<String> = [
        "create-learned-generated",
        "/missing/config",
        "/missing/recipe",
        "--abandon-unknown",
        "/missing/request",
        "--history-anchor",
        "/missing/input",
        "--retain-history-anchor",
        "/missing/output",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for extra in [
        vec!["--resume"],
        vec!["--reconcile-receipts"],
        vec!["--actor-profile", "/missing/actor"],
        vec!["--rotate-credential", "1", "1", "2"],
        vec!["--recover-stop", "1"],
        vec!["--topology-profile", "/missing/topology"],
        vec!["--retain-topology-checkpoint", "/missing/source"],
        vec!["--history-anchor", "/missing/duplicate"],
        vec!["--audit-review", "1", "/missing/audit"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        let error = abandon::command_with(
            &args,
            None,
            &mut || panic!("grammar reached elapsed input"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(
            !error.contains("No such file"),
            "grammar reached file intake: {error}"
        );
        assert!(
            command_with_clock(&args, None, || panic!("abandonment acquired effect clock"))
                .is_err()
        );
    }
    for index in [4, 6, 8] {
        let mut args = base.clone();
        args.remove(index);
        assert!(
            abandon::command_with(
                &args,
                None,
                &mut || panic!("missing path reached intake"),
                &mut Vec::new()
            )
            .is_err()
        );
    }
    assert!(
        abandon::command_with(
            &base,
            Some(Path::new("/missing/credibility")),
            &mut || panic!("credibility reached intake"),
            &mut Vec::new()
        )
        .is_err()
    );
}
