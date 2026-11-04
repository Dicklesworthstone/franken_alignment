//! Full original learned publication followed by offline topology credential
//! maintenance. Synthetic models exercise native custody and effect boundaries;
//! they do not establish detector quality or remote-provider authentication.
use super::super::super::super::super::topology as topology_selection;
use super::super::super::super::mediated_topology as original_topology;
use super::*;
use fa_reference::action::consequence::delivery::{
    credential_broker::CredentialRevocationRequest,
    persistent::observed::{
        driver::topology::checkpoint::{MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint},
        guarded::mediated::FileMediatedRequirements,
    },
};

pub(super) fn published_with_topology(root: &Root, mode: Option<bool>) -> Case {
    let (config, path, mut original) = original_topology::selected(root, mode, false);
    if mode == Some(true) {
        super::super::super::super::identity_tests::provision(root, &path, true);
    }
    let producer = provision(root, &config, &path, true, false);
    provision_credential(root, &config, &path);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    loaded.topology = original.topology.take();
    loaded.anchor_output = original.anchor_output.take();
    let anchor = loaded.anchor_output.clone().unwrap();
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
    let saved = read_checkpoint(&root.0.join("source-1.bin"));
    assert!(saved.available());
    assert!(saved.work().checks >= 2);
    assert_eq!(
        saved.history_anchor(),
        &anchor_files::load(&anchor).unwrap()
    );
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

pub(super) fn read_checkpoint(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(
        &mut io::Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap()
}

pub(super) fn historical(root: &Root, source: &Path, name: &str) -> PathBuf {
    let saved = read_checkpoint(source);
    let path = root.0.join(format!("{name}.json"));
    let r = root.0.display();
    fs::write(&path, format!(
        r#"{{"schema_version":"fa.mediated-submit/2","initial":{{"topology_path":"{r}/initial.json","inventory_path":"{r}/initial-inventory.json"}},"next":{{"topology_path":"{r}/missing-next.json","inventory_path":"{r}/missing-next-inventory.json"}},"operation":{},"source_checkpoint":"{}"}}"#,
        saved.last_operation() + 1, source.display(),
    )).unwrap();
    path
}

pub(super) fn topology_arguments(
    case: &Case,
    anchor: &Path,
    history_output: &Path,
    profile: &Path,
    source_output: &Path,
    operation: u64,
    expected: u64,
    next: Option<u64>,
) -> Vec<String> {
    let mut args = arguments(case, anchor, history_output, operation, expected, next);
    args.extend([
        "--topology-profile".into(),
        profile.to_str().unwrap().into(),
        "--retain-topology-checkpoint".into(),
        source_output.to_str().unwrap().into(),
    ]);
    args
}

pub(super) fn remove_current_inputs(root: &Root) {
    drop(remove_live_assets(root));
    fs::remove_file(root.0.join("live-1.json")).unwrap();
    fs::remove_file(root.0.join("inventory-1.json")).unwrap();
    assert!(!root.0.join("missing-next.json").exists());
    assert!(!root.0.join("missing-next-inventory.json").exists());
}

pub(super) fn assert_pair(
    original: &TopologyCheckpoint,
    history: &Path,
    source: &Path,
    revision: u64,
) -> TopologyCheckpoint {
    let saved = read_checkpoint(source);
    assert_eq!(
        saved.history_anchor(),
        &anchor_files::load(history).unwrap()
    );
    assert_eq!(saved.history_anchor().revision(), revision);
    assert_eq!(saved.work(), original.work());
    assert_eq!(saved.last_operation(), original.last_operation());
    assert_eq!(saved.topology_bytes(), original.topology_bytes());
    assert_eq!(saved.inventory_bytes(), original.inventory_bytes());
    assert_eq!(saved.expected(), original.expected());
    assert!(
        !saved.available(),
        "the original recovery Fence withdraws topology"
    );
    assert!(saved.minimum().control_sequence >= original.minimum().control_sequence);
    assert!(saved.minimum().authority_epoch >= original.minimum().authority_epoch);
    assert!(saved.minimum().journal_revision > original.minimum().journal_revision);
    saved
}

pub(super) fn retention(bytes: &[u8], revision: u64, history: bool, source: bool) {
    let parsed = strict_json::parse(bytes, Limits::default()).unwrap();
    let value = parsed.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(number(value, "journal_revision"), revision);
    assert_eq!(
        value.get("history_retained").unwrap().as_bool(),
        Some(history)
    );
    assert_eq!(
        value.get("topology_checkpoint_required").unwrap().as_bool(),
        Some(true)
    );
    assert_eq!(
        value.get("topology_checkpoint_retained").unwrap().as_bool(),
        Some(source)
    );
    let saved = value.get("retained_topology_checkpoint_revision").unwrap();
    if source {
        assert_eq!(saved.as_str().unwrap().parse::<u64>().unwrap(), revision);
    } else {
        assert!(saved.is_null());
    }
}

#[test]
fn topology_maintenance_rotates_revokes_and_retries_original_receipts_without_live_inputs() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let case = published_with_topology(&root, mode);
        let before = case.reader.snapshot();
        let original_source = root.0.join("source-1.bin");
        let original = read_checkpoint(&original_source);
        let original_bytes = fs::read(&original_source).unwrap();
        let original_anchor = fs::read(&case.anchor).unwrap();
        remove_current_inputs(&root);
        let mut input = case.anchor.clone();
        let mut source = original_source.clone();
        // The third request repeats the first operation after a later rotation.
        // Its historical generation MUST differ from the selected current one.
        for (index, operation, expected, next, epoch, revoked, status, receipt, changes) in [
            (0, 501, 1, Some(2), 2, false, "applied", (501, 2, false), 1),
            (1, 502, 2, Some(3), 3, false, "applied", (502, 3, false), 2),
            (
                2,
                501,
                1,
                Some(2),
                3,
                false,
                "already_applied",
                (501, 2, false),
                2,
            ),
            (3, 601, 3, None, 3, true, "applied", (601, 3, true), 3),
            (
                4,
                601,
                3,
                None,
                3,
                true,
                "already_applied",
                (601, 3, true),
                3,
            ),
        ] {
            let prior = case.reader.snapshot();
            let profile = historical(&root, &source, &format!("maintenance-{index}"));
            let output = root.0.join(format!("history-maintained-{index}.anchor"));
            let next_source = root.0.join(format!("source-maintained-{index}.bin"));
            let bytes = invoke(&topology_arguments(
                &case,
                &input,
                &output,
                &profile,
                &next_source,
                operation,
                expected,
                next,
            ));
            let (fence, revision) =
                report(&bytes, status, Some(receipt), (epoch, revoked, changes));
            assert_eq!(fence, prior.revision + 1);
            assert_eq!(revision, fence + u64::from(status == "applied"));
            retention(&bytes, revision, true, true);
            assert_pair(&original, &output, &next_source, revision);
            unchanged_work(&before, &case.reader.snapshot());
            assert_eq!(
                case.reader.snapshot().control.ledger.elapsed,
                before.control.ledger.elapsed
            );
            select(&root, &case.config, epoch, revoked);
            input = output;
            source = next_source;
        }
        assert_eq!(fs::read(&original_source).unwrap(), original_bytes);
        assert_eq!(fs::read(&case.anchor).unwrap(), original_anchor);
        assert!(!root.0.join("peers").exists());
        assert!(!root.0.join("live-1.json").exists());
        assert!(!root.0.join("inventory-1.json").exists());
        // Historical receipts retain the original exact guarded native path.
        let mut loaded = recipe::load(&case.path, &case.config, false).unwrap();
        loaded.recovery_anchor = Some(anchor_files::load(&input).unwrap());
        let profile = historical(&root, &source, "receipt-after-revocation");
        topology_selection::admit(
            &case.config,
            &mut loaded,
            Some(&profile),
            topology_selection::Mode::Historical,
        )
        .unwrap();
        let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
        let receipt =
            recovery::readonly::read(&case.config, &loaded, None, &mut || Duration::ZERO).unwrap();
        assert_eq!(receipt.result, case.result.response.result);
        assert_eq!(
            fs::read(case.config.store.join("delivery.bin")).unwrap(),
            canonical
        );
        no_secret(&canonical);
    }
}

#[test]
fn topology_maintenance_requires_ready_guards_exact_paired_v2_custody_and_distinct_new_outputs() {
    let root = Root::new();
    let case = published_with_topology(&root, None);
    let source = root.0.join("source-1.bin");
    let original = read_checkpoint(&source);
    let profile = historical(&root, &source, "admission");
    remove_current_inputs(&root);
    let canonical = case.config.store.join("delivery.bin");
    let journal = fs::read(&canonical).unwrap();
    let pending = case.config.store.join("delivery.pending");
    fs::write(&pending, b"preserve rejected topology maintenance staging").unwrap();
    let mut loaded = recipe::load(&case.path, &case.config, false).unwrap();
    loaded.recovery_anchor = Some(anchor_files::load(&case.anchor).unwrap());
    topology_selection::admit(
        &case.config,
        &mut loaded,
        Some(&profile),
        topology_selection::Mode::Historical,
    )
    .unwrap();
    let requirement = topology_selection::history(&loaded)
        .unwrap()
        .unwrap()
        .requirement
        .clone();
    let expected = FileMediatedRequirements {
        oversight: recovery::requirements(&case.config, &loaded),
        topology: requirement,
        prediction: None,
        evaluation: None,
    };
    let cursor = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        &case.config.store,
        case.config.profile.clone(),
        &expected,
        &loaded.generation,
        loaded.recovery_anchor.as_ref().unwrap(),
    )
    .unwrap();
    assert!(cursor.credential_status().is_err());
    assert!(
        cursor
            .check_credential_rotation(CredentialRotationRequest {
                operation: 501,
                expected_generation: 1,
                next_generation: 2,
            })
            .is_err()
    );
    assert!(
        cursor
            .check_credential_revocation(CredentialRevocationRequest {
                operation: 601,
                expected_generation: 1,
            })
            .is_err()
    );
    drop(cursor);
    let original_profile = fs::read(&profile).unwrap();
    for variant in 0..8 {
        select(&root, &case.config, 1, false);
        fs::write(&profile, &original_profile).unwrap();
        let output = root.0.join(format!("rejected-history-{variant}.anchor"));
        let source_output = root.0.join(format!("rejected-source-{variant}.bin"));
        let mut args = topology_arguments(
            &case,
            &case.anchor,
            &output,
            &profile,
            &source_output,
            501,
            1,
            Some(2),
        );
        match variant {
            0 => select(&root, &case.config, 2, false),
            1 => {
                args[5] = "2".into();
                args[6] = "3".into();
            }
            2 => {
                let at = args
                    .iter()
                    .position(|arg| arg == "--retain-topology-checkpoint")
                    .unwrap();
                drop(args.drain(at..at + 2));
            }
            3 => {
                let at = args
                    .iter()
                    .position(|arg| arg == "--topology-profile")
                    .unwrap();
                drop(args.drain(at..at + 2));
            }
            4 => {
                let at = args
                    .iter()
                    .position(|arg| arg == "--retain-topology-checkpoint")
                    .unwrap();
                args[at + 1] = output.to_str().unwrap().into();
            }
            5 => fs::write(&source_output, b"keep independently existing source output").unwrap(),
            6 => {
                let at = args
                    .iter()
                    .position(|arg| arg == "--retain-topology-checkpoint")
                    .unwrap();
                args[at + 1] = case
                    .config
                    .store
                    .join("unsafe-source.bin")
                    .to_str()
                    .unwrap()
                    .into();
            }
            _ => {
                let r = root.0.display();
                fs::write(&profile, format!(
                    r#"{{"schema_version":"fa.mediated-submit/1","initial":{{"topology_path":"{r}/initial.json","inventory_path":"{r}/initial-inventory.json"}},"current":{{"topology_path":"{r}/initial.json","inventory_path":"{r}/initial-inventory.json"}},"next":{{"topology_path":"{r}/missing-next.json","inventory_path":"{r}/missing-next-inventory.json"}},"available":true,"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
                    original.last_operation() + 1, original.work().checks,
                    original.work().reserved_read_bytes, original.last_operation(),
                )).unwrap();
            }
        }
        let mut bytes = Vec::new();
        let error = maintenance::command_with(&args, None, &mut || Duration::ZERO, &mut bytes)
            .err()
            .unwrap_or_else(|| panic!("variant {variant} admitted incomplete custody"));
        no_secret(error.as_bytes());
        assert!(bytes.is_empty());
        assert_eq!(fs::read(&canonical).unwrap(), journal);
        assert_eq!(
            fs::read(&pending).unwrap(),
            b"preserve rejected topology maintenance staging"
        );
        assert!(!output.exists());
        if variant == 5 {
            assert_eq!(
                fs::read(&source_output).unwrap(),
                b"keep independently existing source output"
            );
        } else {
            assert!(!source_output.exists());
        }
    }
    select(&root, &case.config, 1, false);
    fs::write(&profile, original_profile).unwrap();
    let output = root.0.join("accepted-after-refusals.anchor");
    let next_source = root.0.join("accepted-after-refusals.bin");
    let bytes = invoke(&topology_arguments(
        &case,
        &case.anchor,
        &output,
        &profile,
        &next_source,
        501,
        1,
        Some(2),
    ));
    let (_, revision) = report(&bytes, "applied", Some((501, 2, false)), (2, false, 1));
    assert_pair(&original, &output, &next_source, revision);
    assert!(!pending.exists());
    // A new anchor with old source counters is not a pair; an old complete pair
    // cannot explain even this permitted successor through ordinary recovery.
    select(&root, &case.config, 2, false);
    let advanced = fs::read(&canonical).unwrap();
    for input in [&case.anchor, &output] {
        let refused_history = root.0.join("refused-successor.anchor");
        let refused_source = root.0.join("refused-successor.bin");
        let mut bytes = Vec::new();
        assert!(
            maintenance::command_with(
                &topology_arguments(
                    &case,
                    input,
                    &refused_history,
                    &profile,
                    &refused_source,
                    501,
                    1,
                    Some(2)
                ),
                None,
                &mut || Duration::ZERO,
                &mut bytes,
            )
            .is_err()
        );
        assert!(bytes.is_empty());
        assert!(!refused_history.exists());
        assert!(!refused_source.exists());
        assert_eq!(fs::read(&canonical).unwrap(), advanced);
    }
    // Proper successor custody allows an idempotent original retry.
    let next_profile = historical(&root, &next_source, "accepted-retry");
    let retry = root.0.join("exact-retry.anchor");
    let retry_source = root.0.join("exact-retry.bin");
    let bytes = invoke(&topology_arguments(
        &case,
        &output,
        &retry,
        &next_profile,
        &retry_source,
        501,
        1,
        Some(2),
    ));
    let (_, revision) = report(
        &bytes,
        "already_applied",
        Some((501, 2, false)),
        (2, false, 1),
    );
    assert_pair(&original, &retry, &retry_source, revision);
}

#[test]
fn topology_maintenance_deadlines_retain_the_acknowledged_fence_or_transition_pair() {
    for boundary in 1..=3 {
        let root = Root::new();
        let case = published_with_topology(&root, Some(true));
        let original_source = root.0.join("source-1.bin");
        let original = read_checkpoint(&original_source);
        let profile = historical(&root, &original_source, "deadline-input");
        let before = case.reader.snapshot();
        remove_current_inputs(&root);
        let output = root.0.join("deadline.anchor");
        let source = root.0.join("deadline.bin");
        let mut expired = false;
        let mut bytes = Vec::new();
        let error = maintenance::command_with(
            &topology_arguments(
                &case,
                &case.anchor,
                &output,
                &profile,
                &source,
                501,
                1,
                Some(2),
            ),
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
        assert!(error.contains("deadline reached"), "{error}");
        let applied = boundary >= 2;
        let current = if applied { 2 } else { 1 };
        let (fence, revision) = report(
            &bytes,
            if applied { "applied" } else { "refused" },
            applied.then_some((501, 2, false)),
            (current, false, u64::from(applied)),
        );
        assert_eq!(fence, before.revision + 1);
        assert_eq!(revision, fence + u64::from(applied));
        retention(&bytes, revision, true, true);
        assert_pair(&original, &output, &source, revision);
        unchanged_work(&before, &case.reader.snapshot());
        select(&root, &case.config, current, false);
        let next_profile = historical(&root, &source, "deadline-retry");
        let retry = root.0.join("deadline-retry.anchor");
        let retry_source = root.0.join("deadline-retry.bin");
        let bytes = invoke(&topology_arguments(
            &case,
            &output,
            &retry,
            &next_profile,
            &retry_source,
            501,
            1,
            Some(2),
        ));
        let (_, revision) = report(
            &bytes,
            if applied {
                "already_applied"
            } else {
                "applied"
            },
            Some((501, 2, false)),
            (2, false, 1),
        );
        assert_pair(&original, &retry, &retry_source, revision);
        assert!(!root.0.join("peers").exists());
    }
}

#[test]
fn topology_maintenance_attempts_both_exports_and_preserves_acknowledgment_on_report_failure() {
    for failure in 0..3 {
        let root = Root::new();
        let case = published_with_topology(&root, None);
        let original_source = root.0.join("source-1.bin");
        let original = read_checkpoint(&original_source);
        let original_bytes = fs::read(&original_source).unwrap();
        let profile = historical(&root, &original_source, "retention-input");
        let before = case.reader.snapshot();
        remove_current_inputs(&root);
        let output = root.0.join("retention.anchor");
        let source = root.0.join("retention.bin");
        let mut injected = false;
        let mut writer = Output {
            bytes: Vec::new(),
            remaining: (failure == 2).then_some(0),
            expire: None,
        };
        let error = maintenance::command_with(
            &topology_arguments(
                &case,
                &case.anchor,
                &output,
                &profile,
                &source,
                501,
                1,
                Some(2),
            ),
            None,
            &mut || {
                if failure < 2 && !injected && case.reader.snapshot().revision > before.revision {
                    fs::write(
                        if failure == 0 { &output } else { &source },
                        b"independent archive appeared after admission",
                    )
                    .unwrap();
                    injected = true;
                }
                Duration::ZERO
            },
            &mut writer,
        )
        .err()
        .unwrap();
        no_secret(error.as_bytes());
        let revision = before.revision + 2;
        assert_eq!(case.reader.snapshot().revision, revision);
        unchanged_work(&before, &case.reader.snapshot());
        assert_eq!(fs::read(&original_source).unwrap(), original_bytes);
        assert!(!root.0.join("peers").exists());
        match failure {
            0 => {
                assert!(injected);
                assert!(error.contains("history anchor retention failed"), "{error}");
                assert_eq!(
                    fs::read(&output).unwrap(),
                    b"independent archive appeared after admission"
                );
                let saved = read_checkpoint(&source);
                assert_eq!(saved.work(), original.work());
                assert_eq!(saved.history_anchor().revision(), revision);
                retention(&writer.bytes, revision, false, true);
                // The independently retained checkpoint already contains the
                // acknowledged anchor; its custodian can export that exact cut.
                let repaired = root.0.join("independently-exported-anchor.bin");
                anchor_files::export_acknowledged(saved.history_anchor(), &repaired).unwrap();
                assert_pair(&original, &repaired, &source, revision);
            }
            1 => {
                assert!(injected);
                assert!(
                    error.contains("topology checkpoint retention failed"),
                    "{error}"
                );
                assert_eq!(
                    fs::read(&source).unwrap(),
                    b"independent archive appeared after admission"
                );
                assert_eq!(anchor_files::load(&output).unwrap().revision(), revision);
                retention(&writer.bytes, revision, true, false);
                // Ordinary recovery cannot relabel the old source checkpoint
                // as current just because the independent history export worked.
                select(&root, &case.config, 2, false);
                let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
                let mut bytes = Vec::new();
                assert!(
                    maintenance::command_with(
                        &topology_arguments(
                            &case,
                            &output,
                            &root.0.join("invalid-pair.anchor"),
                            &profile,
                            &root.0.join("invalid-pair.bin"),
                            501,
                            1,
                            Some(2)
                        ),
                        None,
                        &mut || Duration::ZERO,
                        &mut bytes,
                    )
                    .is_err()
                );
                assert!(bytes.is_empty());
                assert_eq!(
                    fs::read(case.config.store.join("delivery.bin")).unwrap(),
                    canonical
                );
            }
            _ => {
                assert!(error.contains("result output failed"), "{error}");
                assert!(writer.bytes.is_empty());
                assert_pair(&original, &output, &source, revision);
                select(&root, &case.config, 2, false);
                let profile = historical(&root, &source, "report-failure-retry");
                let retry = root.0.join("report-failure-retry.anchor");
                let retry_source = root.0.join("report-failure-retry.bin");
                let bytes = invoke(&topology_arguments(
                    &case,
                    &output,
                    &retry,
                    &profile,
                    &retry_source,
                    501,
                    1,
                    Some(2),
                ));
                let (_, revision) = report(
                    &bytes,
                    "already_applied",
                    Some((501, 2, false)),
                    (2, false, 1),
                );
                assert_pair(&original, &retry, &retry_source, revision);
            }
        }
    }
}
