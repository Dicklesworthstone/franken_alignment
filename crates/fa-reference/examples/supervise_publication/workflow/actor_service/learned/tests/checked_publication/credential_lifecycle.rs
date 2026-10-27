//! The actual maintenance command consumes independently selected historical
//! contracts and retains a new acknowledged anchor. Fixed models and local
//! fixtures test custody and original accounting, not provider authentication.
use super::super::super::super::credential_lifecycle as maintenance;
use super::*;
use fa_reference::strict_json::{self, Json, Limits};
use std::{io, os::unix::fs::DirBuilderExt};

#[path = "credential_lifecycle/topology.rs"]
mod topology_tests;

const NEXT_SECRET: &[u8] = b"independently-reprovisioned-generation-two-credential-9437";

struct Case {
    config: Config,
    config_path: PathBuf,
    path: PathBuf,
    anchor: PathBuf,
    reader: Reader,
    result: RunResult,
}

fn config_file(root: &Root, config: &Config) -> PathBuf {
    let path = root.0.join("credential-maintenance-config.json");
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    ));
    let text = original
        .replace(
            "/operator/publication-store",
            config.store.to_str().unwrap(),
        )
        .replace(
            "/operator/evidence.json",
            root.0.join("evidence.json").to_str().unwrap(),
        )
        .replace(
            "\"initial_payload_hex\": \"696e697469616c\"",
            "\"initial_payload_hex\": \"\"",
        )
        .replace("\"events\": 256", "\"events\": 1024")
        .replace(
            "\"runtime_ms\": 120000",
            &format!("\"runtime_ms\": {}", config.timing.runtime_ms),
        )
        .replace(
            "\"journal_events\": 4096",
            &format!(
                "\"journal_events\": {}",
                config.profile.delivery.limits.events
            ),
        );
    fs::write(&path, text).unwrap();
    let actual = Config::read(&path).unwrap();
    assert_eq!(actual.source_policy, config.source_policy);
    assert_eq!(
        actual.profile.delivery.limits,
        config.profile.delivery.limits
    );
    assert_eq!(
        actual.profile.delivery.initial_payload,
        config.profile.delivery.initial_payload
    );
    assert_eq!(actual.timing.runtime_ms, config.timing.runtime_ms);
    path
}

fn published(root: &Root, mode: Option<bool>, identity: bool) -> Case {
    let config = configured(root);
    let path = native::write(root);
    if let Some(owned) = mode {
        super::super::super::forecast::install(root, &path, owned, 60_000);
    }
    if identity {
        super::super::super::identity_tests::provision(root, &path, true);
    }
    let producer = provision(root, &config, &path, true, false);
    provision_credential(root, &config, &path);
    evidence(root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    let anchor = root.0.join("independent-initial.anchor");
    loaded.anchor_output = Some(anchor.clone());
    let reader = Reader::new(&config, &loaded);
    let config_path = config_file(root, &config);
    let (result, packet) = service(
        root,
        config,
        loaded,
        None,
        None,
        ReviewDecision::Approve,
        &mut |_| ElapsedTick(1000),
    );
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert!(confirmed(&result));
    packet_is_original(&packet.unwrap());
    assert!(anchor_files::load(&anchor).is_ok());
    drop(producer);
    Case {
        config: configured(root),
        config_path,
        path,
        anchor,
        reader,
        result,
    }
}

fn select(root: &Root, config: &Config, generation: u64, revoked: bool) {
    fs::write(
        root.0.join("credential.json"),
        credential_profile(root, config, generation, revoked),
    )
    .unwrap();
}

fn arguments(
    case: &Case,
    input: &Path,
    output: &Path,
    operation: u64,
    expected: u64,
    next: Option<u64>,
) -> Vec<String> {
    let mut args = vec![
        "create-learned-generated".into(),
        case.config_path.to_str().unwrap().into(),
        case.path.to_str().unwrap().into(),
        if next.is_some() {
            "--rotate-credential".into()
        } else {
            "--revoke-credential".into()
        },
        operation.to_string(),
        expected.to_string(),
    ];
    if let Some(next) = next {
        args.push(next.to_string());
    }
    args.extend([
        "--history-anchor".into(),
        input.to_str().unwrap().into(),
        "--retain-history-anchor".into(),
        output.to_str().unwrap().into(),
    ]);
    args
}

fn invoke(args: &[String]) -> Vec<u8> {
    let mut output = Vec::new();
    maintenance::command_with(args, None, &mut || Duration::ZERO, &mut output).unwrap();
    output
}

fn number(value: &Json, key: &str) -> u64 {
    value.get(key).unwrap().as_str().unwrap().parse().unwrap()
}

fn report(
    bytes: &[u8],
    status: &str,
    receipt: Option<(u64, u64, bool)>,
    current: (u64, bool, u64),
) -> (u64, u64) {
    no_secret(bytes);
    assert!(
        !bytes
            .windows(NEXT_SECRET.len())
            .any(|part| part == NEXT_SECRET)
    );
    let parsed = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(
        parsed.get("schema").unwrap().as_str(),
        Some("fa.learned-credential-lifecycle-result/1")
    );
    assert_eq!(parsed.get("status").unwrap().as_str(), Some(status));
    assert_eq!(parsed.get("authority").unwrap().as_str(), Some("none"));
    let knowledge = parsed.get("knowledge").unwrap();
    assert_eq!(knowledge.get("kind").unwrap().as_str(), Some("known"));
    let value = knowledge.get("value").unwrap();
    let selected = value.get("receipt").unwrap();
    if let Some((operation, generation, revoked)) = receipt {
        assert_eq!(number(selected, "operation"), operation);
        assert_eq!(number(selected, "generation"), generation);
        assert_eq!(selected.get("revoked").unwrap().as_bool(), Some(revoked));
    } else {
        assert!(selected.is_null());
    }
    let actual = value.get("current").unwrap();
    assert_eq!(number(actual, "generation"), current.0);
    assert_eq!(actual.get("revoked").unwrap().as_bool(), Some(current.1));
    assert_eq!(number(actual, "retained_changes"), current.2);
    assert_eq!(value.get("history_retained").unwrap().as_bool(), Some(true));
    let revision = number(value, "journal_revision");
    assert_eq!(number(value, "retained_history_revision"), revision);
    assert_eq!(
        number(
            knowledge.get("basis").unwrap(),
            "acknowledged_journal_revision"
        ),
        revision
    );
    (number(value, "recovery_fence_revision"), revision)
}

fn remove_live_assets(root: &Root) -> Vec<(PathBuf, Vec<u8>)> {
    let mut saved = Vec::new();
    for name in [
        "inventory.json",
        "broker.bin",
        "provider.bin",
        "evidence.json",
        "native-roster.json",
        "helper-model.json",
        "helper-weights.safetensors",
        "helper-monitor.json",
        "helper-tokenizer.bbpe",
        "helper-sampling.json",
        "producer/delivery.bin",
    ] {
        let path = root.0.join(name);
        saved.push((path.clone(), fs::read(&path).unwrap()));
        fs::remove_file(path).unwrap();
    }
    fs::remove_dir(root.0.join("peers")).unwrap();
    saved
}

fn restore_live_assets(root: &Root, saved: Vec<(PathBuf, Vec<u8>)>) {
    for (path, bytes) in saved {
        fs::write(path, bytes).unwrap();
    }
    fs::DirBuilder::new()
        .mode(0o750)
        .create(root.0.join("peers"))
        .unwrap();
}

fn unchanged_work(before: &FileDeliverySnapshot, after: &FileDeliverySnapshot) {
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(
        after.control.ledger.available,
        before.control.ledger.available
    );
    assert_eq!(
        after.control.ledger.reserved,
        before.control.ledger.reserved
    );
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.stages, before.control.ledger.stages);
    assert!(after.control.ledger.epoch >= before.control.ledger.epoch);
}

#[test]
fn maintenance_rotation_then_independent_epoch_selection_publishes_the_next_original_message() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let case = published(&root, mode, false);
        let before = case.reader.snapshot();
        let initial = fs::read(&case.anchor).unwrap();
        let original = recipe::load(&case.path, &case.config, false).unwrap();
        let saved = remove_live_assets(&root);
        let output = root.0.join("rotated-two.anchor");
        let result = invoke(&arguments(&case, &case.anchor, &output, 501, 1, Some(2)));
        let (fence, revision) = report(&result, "applied", Some((501, 2, false)), (2, false, 1));
        assert_eq!(fence, before.revision + 1);
        assert_eq!(revision, fence + 1);
        assert_eq!(anchor_files::load(&output).unwrap().revision(), revision);
        assert_eq!(fs::read(&case.anchor).unwrap(), initial);
        unchanged_work(&before, &case.reader.snapshot());
        assert!(!root.0.join("peers").exists());
        restore_live_assets(&root, saved);
        select(&root, &case.config, 2, false);
        // Rotation authorizes no secret. The independent provider expectation
        // and broker input for epoch two must both be supplied for live work.
        fs::write(root.0.join("broker.bin"), NEXT_SECRET).unwrap();
        fs::write(root.0.join("provider.bin"), NEXT_SECRET).unwrap();
        let (path, mut next) = native::next(&root, &case.config, 2);
        assert_eq!(next.generation, original.generation);
        next.recovery_anchor = Some(anchor_files::load(&output).unwrap());
        let retained = root.0.join("continued-two.anchor");
        next.anchor_output = Some(retained.clone());
        let reader = Reader::new(&case.config, &next);
        let (result, packet) = service(
            &root,
            configured(&root),
            next,
            Some(1),
            None,
            ReviewDecision::Approve,
            &mut |_| ElapsedTick(1001),
        );
        assert!(
            result.failure.is_none(),
            "mode={mode:?}: {:?}",
            result.failure
        );
        assert!(confirmed(&result));
        packet_is_original(&packet.unwrap());
        let current = reader.snapshot();
        assert_eq!(current.executions, 2);
        assert_eq!(current.payload, b"AA");
        assert!(current.control.ledger.charged > before.control.ledger.charged);
        assert_eq!(current.control.ledger.reserved, 0);
        let mut exact = recipe::load(&path, &case.config, false).unwrap();
        exact.recovery_anchor = Some(anchor_files::load(&retained).unwrap());
        let (host, _, _, _) = recovery::open_selected(&case.config, &exact).unwrap();
        let status = host.credential_status().unwrap().unwrap();
        assert_eq!(status.generation, 2);
        assert!(!status.revoked);
        assert_eq!(status.retained_changes, 1);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 6);
        assert_eq!(numerical.sampled_draws, 4);
        let journal = fs::read(case.config.store.join("delivery.bin")).unwrap();
        no_secret(&journal);
        assert!(
            !journal
                .windows(NEXT_SECRET.len())
                .any(|part| part == NEXT_SECRET)
        );
    }
}

#[test]
fn source_free_revocation_preserves_original_receipts_audit_and_stop_but_refuses_live_work() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let case = published(&root, mode, mode == Some(true));
        let before = case.reader.snapshot();
        select(&root, &case.config, 1, true);
        let revoked_live = recipe::load(&case.path, &case.config, true).unwrap();
        select(&root, &case.config, 1, false);
        let _absent = remove_live_assets(&root);
        let output = root.0.join("revoked-one.anchor");
        let result = invoke(&arguments(&case, &case.anchor, &output, 601, 1, None));
        let (_, revision) = report(&result, "applied", Some((601, 1, true)), (1, true, 1));
        unchanged_work(&before, &case.reader.snapshot());
        assert_eq!(anchor_files::load(&output).unwrap().revision(), revision);
        select(&root, &case.config, 1, true);
        let retry = root.0.join("revoked-exact-retry.anchor");
        report(
            &invoke(&arguments(&case, &output, &retry, 601, 1, None)),
            "already_applied",
            Some((601, 1, true)),
            (1, true, 1),
        );
        let mut exact = recipe::load(&case.path, &case.config, false).unwrap();
        exact.recovery_anchor = Some(anchor_files::load(&retry).unwrap());
        assert!(exact.native.is_none());
        let journal = fs::read(case.config.store.join("delivery.bin")).unwrap();
        let receipt =
            recovery::readonly::read(&case.config, &exact, None, &mut || Duration::ZERO).unwrap();
        assert_eq!(receipt.result, case.result.response.result);
        let mut audit = Vec::new();
        audit_command::run(
            &case.config,
            &exact,
            1,
            exact.recovery_anchor.as_ref().unwrap(),
            &mut audit,
            &mut || Ok(()),
        )
        .unwrap();
        assert!(!audit.is_empty());
        no_secret(&audit);
        assert_eq!(
            fs::read(case.config.store.join("delivery.bin")).unwrap(),
            journal
        );
        assert!(!root.0.join("peers").exists());
        fs::DirBuilder::new()
            .mode(0o750)
            .create(root.0.join("peers"))
            .unwrap();
        let peers = fixture::peers(&root, &case.config);
        let failure = run(configured(&root), revoked_live, &peers, true, || {
            ElapsedTick(1001)
        })
        .err()
        .unwrap();
        assert!(failure.contains("selected credential generation is revoked"));
        assert_eq!(
            fs::read(case.config.store.join("delivery.bin")).unwrap(),
            journal
        );
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        fs::remove_dir(root.0.join("peers")).unwrap();
        let (host, _, _, _) = recovery::open_selected(&case.config, &exact).unwrap();
        let status = host.credential_status().unwrap().unwrap();
        assert_eq!(status.generation, 1);
        assert!(status.revoked);
        assert_eq!(status.retained_changes, 1);
        assert_eq!(
            host.learned_generation_inspection()
                .unwrap()
                .numerical
                .sampled_draws,
            2
        );
        drop(host);
        let stopped_anchor = root.0.join("revoked-stopped.anchor");
        exact.anchor_output = Some(stopped_anchor.clone());
        let mut stop = Vec::new();
        stop_command::run(&case.config, &exact, 801, || ElapsedTick(1001), &mut stop).unwrap();
        let stopped = case.reader.snapshot();
        unchanged_work(&before, &stopped);
        assert!(stopped.stop.is_some());
        assert!(!stop.is_empty());
        assert!(anchor_files::load(&stopped_anchor).is_ok());
    }
}

#[test]
fn an_old_exact_rotation_retry_reports_its_original_receipt_separately_from_current_epoch() {
    let root = Root::new();
    let case = published(&root, Some(true), true);
    let _absent = remove_live_assets(&root);
    let before = case.reader.snapshot();
    let second = root.0.join("retry-generation-two.anchor");
    report(
        &invoke(&arguments(&case, &case.anchor, &second, 501, 1, Some(2))),
        "applied",
        Some((501, 2, false)),
        (2, false, 1),
    );
    select(&root, &case.config, 2, false);
    let third = root.0.join("retry-generation-three.anchor");
    report(
        &invoke(&arguments(&case, &second, &third, 502, 2, Some(3))),
        "applied",
        Some((502, 3, false)),
        (3, false, 2),
    );
    select(&root, &case.config, 3, false);
    let journal = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let pending = case.config.store.join("delivery.pending");
    fs::write(&pending, b"retain contradictory lifecycle retry staging").unwrap();
    for (operation, expected, next) in [(501, 2, Some(3)), (501, 1, None), (503, 1, Some(2))] {
        let output = root.0.join(format!(
            "wrong-operation-{operation}-{expected}-{}.anchor",
            next.is_some()
        ));
        let mut bytes = Vec::new();
        assert!(
            maintenance::command_with(
                &arguments(&case, &third, &output, operation, expected, next),
                None,
                &mut || Duration::ZERO,
                &mut bytes
            )
            .is_err()
        );
        assert!(bytes.is_empty());
        assert!(!output.exists());
        assert_eq!(
            fs::read(case.config.store.join("delivery.bin")).unwrap(),
            journal
        );
        assert_eq!(
            fs::read(&pending).unwrap(),
            b"retain contradictory lifecycle retry staging"
        );
    }
    let retry = root.0.join("historical-operation-retry.anchor");
    let prior = case.reader.snapshot();
    let result = invoke(&arguments(&case, &third, &retry, 501, 1, Some(2)));
    let (fence, revision) = report(
        &result,
        "already_applied",
        Some((501, 2, false)),
        (3, false, 2),
    );
    assert_eq!(fence, prior.revision + 1);
    assert_eq!(revision, fence);
    assert!(!pending.exists());
    unchanged_work(&before, &case.reader.snapshot());
    let mut exact = recipe::load(&case.path, &case.config, false).unwrap();
    exact.recovery_anchor = Some(anchor_files::load(&retry).unwrap());
    let receipt =
        recovery::readonly::read(&case.config, &exact, None, &mut || Duration::ZERO).unwrap();
    assert_eq!(receipt.result, case.result.response.result);
    let before_timeout = case.reader.snapshot();
    let timed_retry = root.0.join("historical-retry-after-fence-timeout.anchor");
    let mut result = Vec::new();
    let mut expired = false;
    let failure = maintenance::command_with(
        &arguments(&case, &retry, &timed_retry, 501, 1, Some(2)),
        None,
        &mut || {
            expired |= case.reader.snapshot().revision > before_timeout.revision;
            if expired {
                Duration::from_secs(3600)
            } else {
                Duration::ZERO
            }
        },
        &mut result,
    )
    .err()
    .unwrap();
    assert!(expired);
    assert!(failure.contains("deadline reached"));
    let (fence, revision) = report(
        &result,
        "already_applied",
        Some((501, 2, false)),
        (3, false, 2),
    );
    assert_eq!(fence, before_timeout.revision + 1);
    assert_eq!(revision, fence);
    assert_eq!(
        anchor_files::load(&timed_retry).unwrap().revision(),
        revision
    );
    unchanged_work(&before, &case.reader.snapshot());
}

#[test]
fn invalid_lifecycle_requests_guards_and_anchor_destinations_cannot_consume_the_recovery_fence() {
    use fa_reference::action::consequence::delivery::credential_broker::CredentialRevocationRequest;
    use std::os::unix::fs::symlink;

    let root = Root::new();
    let case = published(&root, None, false);
    let foreign_root = Root::new();
    let foreign = published(&foreign_root, None, false);
    let _absent = remove_live_assets(&root);
    let canonical = case.config.store.join("delivery.bin");
    let journal = fs::read(&canonical).unwrap();
    let pending = case.config.store.join("delivery.pending");
    fs::write(&pending, b"preserve rejected maintenance staging").unwrap();
    let original = fs::read(root.0.join("credential.json")).unwrap();
    let input = anchor_files::load(&case.anchor).unwrap();
    let loaded = recipe::load(&case.path, &case.config, false).unwrap();
    let expected = recovery::requirements(&case.config, &loaded);
    let cursor = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        &case.config.store,
        case.config.profile.clone(),
        &expected,
        &loaded.generation,
        &input,
    )
    .unwrap();
    assert!(cursor.credential_status().is_err());
    assert!(
        cursor
            .check_credential_rotation(CredentialRotationRequest {
                operation: 501,
                expected_generation: 1,
                next_generation: 2
            })
            .is_err()
    );
    assert!(
        cursor
            .check_credential_revocation(CredentialRevocationRequest {
                operation: 601,
                expected_generation: 1
            })
            .is_err()
    );
    drop(cursor);
    assert_eq!(fs::read(&canonical).unwrap(), journal);
    for variant in 0..14 {
        fs::write(root.0.join("credential.json"), &original).unwrap();
        let output = root.0.join(format!("inadmissible-{variant}.anchor"));
        let mut args = arguments(&case, &case.anchor, &output, 501, 1, Some(2));
        match variant {
            0 => {
                args[5] = "2".into();
                args[6] = "3".into();
            }
            1 => args[6] = "3".into(),
            2 => args[4] = "0".into(),
            3 => args[5] = "0".into(),
            4 => args[4] = "18446744073709551616".into(),
            5 => select(&root, &case.config, 2, false),
            6 => fs::write(
                root.0.join("credential.json"),
                credential_profile(&root, &case.config, 1, false)
                    .replace("broker-token", "foreign-token"),
            )
            .unwrap(),
            7 => {
                let path = root.0.join("malformed.anchor");
                fs::write(&path, b"not an acknowledged archive").unwrap();
                args[8] = path.to_str().unwrap().into();
            }
            8 => args[8] = foreign.anchor.to_str().unwrap().into(),
            9 => fs::write(&output, b"independent existing output must not be replaced").unwrap(),
            10 => {
                args[10] = case
                    .config
                    .store
                    .join("inside.anchor")
                    .to_str()
                    .unwrap()
                    .into()
            }
            11 => {
                let path = case.config.store.join("inside-input.anchor");
                fs::write(&path, fs::read(&case.anchor).unwrap()).unwrap();
                args[8] = path.to_str().unwrap().into();
            }
            12 => args[10] = case.anchor.to_str().unwrap().into(),
            _ => symlink(root.0.join("absent-symlink-target"), &output).unwrap(),
        }
        let mut report = Vec::new();
        let failure = maintenance::command_with(&args, None, &mut || Duration::ZERO, &mut report)
            .err()
            .unwrap_or_else(|| panic!("inadmissible variant {variant} unexpectedly completed"));
        no_secret(failure.as_bytes());
        assert!(report.is_empty(), "variant={variant}");
        assert_eq!(fs::read(&canonical).unwrap(), journal, "variant={variant}");
        assert_eq!(
            fs::read(&pending).unwrap(),
            b"preserve rejected maintenance staging"
        );
        if variant == 9 {
            assert_eq!(
                fs::read(&output).unwrap(),
                b"independent existing output must not be replaced"
            );
        } else if variant == 13 {
            assert!(
                fs::symlink_metadata(&output)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert!(!root.0.join("absent-symlink-target").exists());
        } else {
            assert!(!output.exists());
        }
        assert_eq!(anchor_files::load(&case.anchor).unwrap(), input);
    }
    fs::write(root.0.join("credential.json"), original).unwrap();
    let output = root.0.join("valid-after-rejections.anchor");
    report(
        &invoke(&arguments(&case, &case.anchor, &output, 501, 1, Some(2))),
        "applied",
        Some((501, 2, false)),
        (2, false, 1),
    );
    assert!(!pending.exists());
    assert!(!root.0.join("peers").exists());
}

#[test]
fn closed_lifecycle_grammar_rejects_other_authority_modes_before_any_input_or_replay() {
    let root = Root::new();
    let case = published(&root, None, false);
    let journal = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let pending = case.config.store.join("delivery.pending");
    fs::write(&pending, b"keep closed-grammar staging").unwrap();
    let config = fs::read(&case.config_path).unwrap();
    fs::remove_file(&case.config_path).unwrap();
    let suffixes: &[&[&str]] = &[
        &["--resume"],
        &["--receipts-only"],
        &["--reconcile-receipts"],
        &["--finish-after-message", "1"],
        &["--continue-after-message", "1"],
        &["--recover-stop", "801"],
        &["--stop-admission", "801"],
        &["--drain-stopped", "801"],
        &["--actor-profile", "/missing/actor.json"],
        &["--activate-credibility", "/missing/activation.json"],
        &["--topology-profile", "/missing/topology.json"],
        &["--retain-topology-checkpoint", "/missing/new.checkpoint"],
        &["--recover-terminal-checkpoint"],
        &["--rotate-credential", "502", "1", "2"],
        &["--revoke-credential", "601", "1"],
        &["--history-anchor", "/missing/duplicate.anchor"],
        &[
            "--retain-history-anchor",
            "/missing/duplicate-output.anchor",
        ],
    ];
    for (index, suffix) in suffixes.iter().enumerate() {
        let output = root.0.join(format!("closed-{index}.anchor"));
        let mut args = arguments(&case, &case.anchor, &output, 501, 1, Some(2));
        args.extend(suffix.iter().map(|part| (*part).to_owned()));
        let mut reads = 0;
        let mut bytes = Vec::new();
        let failure = maintenance::command_with(
            &args,
            None,
            &mut || {
                reads += 1;
                Duration::ZERO
            },
            &mut bytes,
        )
        .err()
        .unwrap();
        assert!(
            !failure.contains("No such file"),
            "closed mode reached missing config: {failure}"
        );
        if suffix[0].contains("topology") || suffix[0] == "--recover-terminal-checkpoint" {
            assert!(failure.contains("topology custody"));
        }
        assert_eq!(reads, 0);
        assert!(bytes.is_empty());
        assert!(!output.exists());
        assert_eq!(
            fs::read(case.config.store.join("delivery.bin")).unwrap(),
            journal
        );
        assert_eq!(fs::read(&pending).unwrap(), b"keep closed-grammar staging");
    }
    for omit in ["--history-anchor", "--retain-history-anchor"] {
        let output = root.0.join(format!("omitted-{omit}.anchor"));
        let mut args = arguments(&case, &case.anchor, &output, 501, 1, Some(2));
        let index = args.iter().position(|arg| arg == omit).unwrap();
        drop(args.drain(index..index + 2));
        let mut bytes = Vec::new();
        let failure = maintenance::command_with(
            &args,
            None,
            &mut || panic!("missing mandatory custody reached configuration intake"),
            &mut bytes,
        )
        .err()
        .unwrap();
        assert!(!failure.contains("No such file"));
        assert!(bytes.is_empty());
        assert!(!output.exists());
    }
    let output = root.0.join("activation-refused.anchor");
    let mut bytes = Vec::new();
    let failure = maintenance::command_with(
        &arguments(&case, &case.anchor, &output, 501, 1, Some(2)),
        Some(&root.0.join("missing-activation.json")),
        &mut || panic!("activation was not closed"),
        &mut bytes,
    )
    .err()
    .unwrap();
    assert!(failure.contains("cannot activate credibility"));
    assert!(bytes.is_empty());
    assert_eq!(
        fs::read(case.config.store.join("delivery.bin")).unwrap(),
        journal
    );
    fs::write(&case.config_path, config).unwrap();
    let output = root.0.join("valid-closed-command.anchor");
    report(
        &invoke(&arguments(&case, &case.anchor, &output, 501, 1, Some(2))),
        "applied",
        Some((501, 2, false)),
        (2, false, 1),
    );
    assert!(!pending.exists());
}

struct Output<'a> {
    bytes: Vec<u8>,
    remaining: Option<usize>,
    expire: Option<&'a std::cell::Cell<bool>>,
}

impl io::Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(expire) = self.expire {
            expire.set(true);
        }
        let count = self
            .remaining
            .map_or(bytes.len(), |remaining| remaining.min(bytes.len()));
        if count == 0 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.bytes.extend_from_slice(&bytes[..count]);
        if let Some(remaining) = self.remaining.as_mut() {
            *remaining -= count;
        }
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn lifecycle_deadlines_and_output_failures_preserve_the_latest_acknowledged_cut_and_exact_retry() {
    for boundary in 0..4 {
        let root = Root::new();
        let case = published(&root, Some(true), true);
        let _absent = remove_live_assets(&root);
        let before = case.reader.snapshot();
        let journal = fs::read(case.config.store.join("delivery.bin")).unwrap();
        let initial = fs::read(&case.anchor).unwrap();
        let output = root.0.join("deadline-cut.anchor");
        let mut expired = false;
        let mut bytes = Vec::new();
        let failure = maintenance::command_with(
            &arguments(&case, &case.anchor, &output, 501, 1, Some(2)),
            None,
            &mut || {
                expired |= if boundary == 3 {
                    output.exists()
                } else {
                    case.reader.snapshot().revision >= before.revision + boundary
                };
                if expired {
                    Duration::from_secs(3600)
                } else {
                    Duration::ZERO
                }
            },
            &mut bytes,
        )
        .err()
        .unwrap();
        assert!(expired);
        assert!(failure.contains("deadline reached"), "{failure}");
        assert_eq!(fs::read(&case.anchor).unwrap(), initial);
        unchanged_work(&before, &case.reader.snapshot());
        assert_eq!(
            case.reader.snapshot().control.ledger.elapsed,
            before.control.ledger.elapsed
        );
        if boundary == 0 {
            assert!(bytes.is_empty());
            assert!(!output.exists());
            assert_eq!(
                fs::read(case.config.store.join("delivery.bin")).unwrap(),
                journal
            );
        } else {
            let applied = boundary >= 2;
            let (fence, revision) = report(
                &bytes,
                if applied { "applied" } else { "refused" },
                applied.then_some((501, 2, false)),
                (if applied { 2 } else { 1 }, false, u64::from(applied)),
            );
            assert_eq!(fence, before.revision + 1);
            assert_eq!(revision, fence + u64::from(applied));
            assert_eq!(anchor_files::load(&output).unwrap().revision(), revision);
        }
        let applied = boundary >= 2;
        select(&root, &case.config, if applied { 2 } else { 1 }, false);
        let retry = root.0.join("retry-after-deadline.anchor");
        let input = if output.exists() {
            &output
        } else {
            &case.anchor
        };
        report(
            &invoke(&arguments(&case, input, &retry, 501, 1, Some(2))),
            if applied {
                "already_applied"
            } else {
                "applied"
            },
            Some((501, 2, false)),
            (2, false, 1),
        );
        unchanged_work(&before, &case.reader.snapshot());
        assert!(!root.0.join("peers").exists());
    }
    for variant in 0..3 {
        let root = Root::new();
        let case = published(&root, None, false);
        let _absent = remove_live_assets(&root);
        let before = case.reader.snapshot();
        let output = root.0.join("output-failed-after-commit.anchor");
        let expired = std::cell::Cell::new(false);
        let mut writer = Output {
            bytes: Vec::new(),
            remaining: match variant {
                0 => Some(0),
                1 => Some(19),
                _ => None,
            },
            expire: (variant == 2).then_some(&expired),
        };
        let failure = maintenance::command_with(
            &arguments(&case, &case.anchor, &output, 501, 1, Some(2)),
            None,
            &mut || {
                if expired.get() {
                    Duration::from_secs(3600)
                } else {
                    Duration::ZERO
                }
            },
            &mut writer,
        )
        .err()
        .unwrap();
        if variant == 2 {
            assert!(expired.get());
            assert!(failure.contains("deadline reached"));
            report(
                &writer.bytes,
                "applied",
                Some((501, 2, false)),
                (2, false, 1),
            );
        } else {
            assert!(failure.contains("result output failed"));
        }
        assert_eq!(case.reader.snapshot().revision, before.revision + 2);
        assert_eq!(
            anchor_files::load(&output).unwrap().revision(),
            before.revision + 2
        );
        unchanged_work(&before, &case.reader.snapshot());
        select(&root, &case.config, 2, false);
        let retry = root.0.join("retry-after-output-failure.anchor");
        report(
            &invoke(&arguments(&case, &output, &retry, 501, 1, Some(2))),
            "already_applied",
            Some((501, 2, false)),
            (2, false, 1),
        );
        unchanged_work(&before, &case.reader.snapshot());
        no_secret(&writer.bytes);
    }
    let root = Root::new();
    let case = published(&root, None, false);
    let _absent = remove_live_assets(&root);
    let before = case.reader.snapshot();
    let output = root.0.join("late-existing-output.anchor");
    let initial = fs::read(&case.anchor).unwrap();
    let mut injected = false;
    let mut bytes = Vec::new();
    let failure = maintenance::command_with(
        &arguments(&case, &case.anchor, &output, 501, 1, Some(2)),
        None,
        &mut || {
            if !injected && case.reader.snapshot().revision > before.revision {
                fs::write(&output, b"independently appeared after output admission").unwrap();
                injected = true;
            }
            Duration::ZERO
        },
        &mut bytes,
    )
    .err()
    .unwrap();
    assert!(injected);
    assert!(failure.contains("history anchor retention failed"));
    assert_eq!(
        fs::read(&output).unwrap(),
        b"independently appeared after output admission"
    );
    assert_eq!(fs::read(&case.anchor).unwrap(), initial);
    let parsed = strict_json::parse(&bytes, Limits::default()).unwrap();
    assert_eq!(parsed.get("status").unwrap().as_str(), Some("applied"));
    let value = parsed.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(
        value.get("history_retained").unwrap().as_bool(),
        Some(false)
    );
    assert!(value.get("retained_history_revision").unwrap().is_null());
    assert_eq!(number(value.get("receipt").unwrap(), "generation"), 2);
    assert_eq!(case.reader.snapshot().revision, before.revision + 2);
    select(&root, &case.config, 2, false);
    let retry = root
        .0
        .join("retry-with-new-output-after-retention-failure.anchor");
    report(
        &invoke(&arguments(&case, &case.anchor, &retry, 501, 1, Some(2))),
        "already_applied",
        Some((501, 2, false)),
        (2, false, 1),
    );
    unchanged_work(&before, &case.reader.snapshot());
}

#[test]
fn revocation_cannot_refund_or_resend_an_original_unknown_credentialed_publication() {
    use fa_reference::action::consequence::oversight::actor::UnknownReason;

    let root = Root::new();
    let mut config = configured(&root);
    let path = native::write(&root);
    provision_credential(&root, &config, &path);
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let reader = Reader::new(&config, &loaded);
    let config_path = config_file(&root, &config);
    let (host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    let credential = bind(&host, &config);
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let deadline = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(
        generate(
            &mut driver,
            &reviewer,
            &mut config,
            &mut control,
            &deadline,
            &mut || ElapsedTick(1000)
        )
        .unwrap()
    );
    submit(
        &mut driver,
        &mut wire,
        &mut config,
        &loaded,
        &deadline,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let (mut driver, history) =
        native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
    let human = native::approve(&mut driver, &mut config, &reviewer, 1);
    assert!(matches!(
        driver
            .step_computed_from_policy_file_with_credential(
                &mut config.source,
                || ElapsedTick(1000),
                Some(&human),
                &credential
            )
            .result
            .unwrap(),
        FileDriverEvent::Dispatched { request: 1, .. }
    ));
    assert!(matches!(
        driver
            .step_computed_from_policy_file(&mut config.source, || ElapsedTick(1000), None)
            .result
            .unwrap(),
        FileDriverEvent::PublicationUnknown {
            error: JournalError::Contract(Error::Incomplete),
            ..
        }
    ));
    assert!(matches!(
        driver
            .step_computed_from_policy_file_with_credential(
                &mut config.source,
                || ElapsedTick(1000),
                None,
                &credential
            )
            .result
            .unwrap(),
        FileDriverEvent::Reconciled {
            outcome: Reconciliation::AwaitingResolution,
            ..
        }
    ));
    let host = driver.supervisor().host().unwrap();
    let admitted = host.inspect();
    assert!(admitted.control.ledger.charged > 0);
    assert_eq!(admitted.executions, 0);
    let anchor = root.0.join("independent-unknown.anchor");
    anchor_files::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
    drop(host);
    drop(wire);
    drop(history);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    for name in [
        "inventory.json",
        "broker.bin",
        "provider.bin",
        "evidence.json",
        "native-roster.json",
        "helper-model.json",
        "helper-weights.safetensors",
        "helper-monitor.json",
        "helper-tokenizer.bbpe",
        "helper-sampling.json",
    ] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::remove_dir(root.0.join("peers")).unwrap();
    let output = root.0.join("revoked-unknown.anchor");
    let args = vec![
        "create-learned-generated".into(),
        config_path.to_str().unwrap().into(),
        path.to_str().unwrap().into(),
        "--revoke-credential".into(),
        "601".into(),
        "1".into(),
        "--history-anchor".into(),
        anchor.to_str().unwrap().into(),
        "--retain-history-anchor".into(),
        output.to_str().unwrap().into(),
    ];
    report(
        &invoke(&args),
        "applied",
        Some((601, 1, true)),
        (1, true, 1),
    );
    select(&root, &config, 1, true);
    let mut exact = recipe::load(&path, &config, false).unwrap();
    exact.recovery_anchor = Some(anchor_files::load(&output).unwrap());
    let receipt = recovery::readonly::read(&config, &exact, None, &mut || Duration::ZERO).unwrap();
    assert_eq!(
        receipt.result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown
        })
    );
    let current = reader.snapshot();
    assert_eq!(current.executions, 0);
    assert!(current.payload.is_empty());
    assert_eq!(
        current.control.ledger.charged,
        admitted.control.ledger.charged
    );
    assert_eq!(
        current.control.ledger.available,
        admitted.control.ledger.available
    );
    assert_eq!(current.control.ledger.reserved, 0);
    let journal = fs::read(config.store.join("delivery.bin")).unwrap();
    no_secret(&journal);
    let (host, _, _, _) = recovery::open_selected(&config, &exact).unwrap();
    let status = host.credential_status().unwrap().unwrap();
    assert!(status.revoked);
    assert_eq!(status.retained_changes, 1);
    assert_eq!(
        host.learned_generation_inspection()
            .unwrap()
            .numerical
            .sampled_draws,
        2
    );
    assert_eq!(
        host.inspect().control.ledger.charged,
        admitted.control.ledger.charged
    );
    assert_eq!(host.inspect().executions, 0);
}
