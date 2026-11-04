//! The real offline command repairs lost exports only for the exact native
//! lifecycle operation. Historical source custody never resumes publication.
use super::topology_tests::{
    assert_pair, historical, published_with_topology, read_checkpoint, remove_current_inputs,
    retention, topology_arguments,
};
use super::*;

fn repair_args(mut args: Vec<String>) -> Vec<String> {
    args.push("--recover-credential-checkpoint".into());
    args
}

// Both create-new outputs race after admission and the acknowledged Fence.
// The journal may contain the transition or only its Fence; neither result
// licenses fabricating a newer checkpoint from a stale independent anchor.
fn lose_exports(case: &Case, args: &[String], fence_only: bool) {
    let before = case.reader.snapshot().revision;
    let output = PathBuf::from(
        &args[args
            .iter()
            .position(|a| a == "--retain-history-anchor")
            .unwrap()
            + 1],
    );
    let source = PathBuf::from(
        &args[args
            .iter()
            .position(|a| a == "--retain-topology-checkpoint")
            .unwrap()
            + 1],
    );
    let mut injected = false;
    let mut bytes = Vec::new();
    let error = maintenance::command_with(
        args,
        None,
        &mut || {
            if !injected && case.reader.snapshot().revision > before {
                fs::write(&output, b"raced independent history output").unwrap();
                fs::write(&source, b"raced independent source output").unwrap();
                injected = true;
            }
            if injected && fence_only {
                Duration::from_secs(3600)
            } else {
                Duration::ZERO
            }
        },
        &mut bytes,
    )
    .unwrap_err();
    assert!(injected);
    assert!(error.contains("history anchor retention failed"), "{error}");
    assert!(
        error.contains("topology checkpoint retention failed"),
        "{error}"
    );
    let revision = before + if fence_only { 1 } else { 2 };
    assert_eq!(case.reader.snapshot().revision, revision);
    retention(&bytes, revision, false, false);
    let parsed = strict_json::parse(&bytes, Limits::default()).unwrap();
    assert_eq!(
        parsed.get("status").unwrap().as_str(),
        Some(if fence_only { "refused" } else { "applied" })
    );
    no_secret(error.as_bytes());
}

#[test]
fn restricted_checkpoint_repair_restores_both_exports_for_all_forecast_histories() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let case = published_with_topology(&root, mode);
        let before = case.reader.snapshot();
        let original_path = root.0.join("source-1.bin");
        let original = read_checkpoint(&original_path);
        let original_bytes = fs::read(&original_path).unwrap();
        let original_anchor = fs::read(&case.anchor).unwrap();
        remove_current_inputs(&root);
        let mut anchor = case.anchor.clone();
        let mut source = original_path.clone();
        for (index, operation, expected, next, revoked) in
            [(0, 501, 1, Some(2), false), (1, 601, 2, None, true)]
        {
            let profile = historical(&root, &source, &format!("lost-{index}"));
            let lost_history = root.0.join(format!("lost-{index}.anchor"));
            let lost_source = root.0.join(format!("lost-{index}.bin"));
            lose_exports(
                &case,
                &topology_arguments(
                    &case,
                    &anchor,
                    &lost_history,
                    &profile,
                    &lost_source,
                    operation,
                    expected,
                    next,
                ),
                false,
            );
            select(&root, &case.config, 2, revoked);
            let journal = fs::read(case.config.store.join("delivery.bin")).unwrap();
            let output = root.0.join(format!("repaired-{index}.anchor"));
            let repaired_source = root.0.join(format!("repaired-{index}.bin"));
            let args = topology_arguments(
                &case,
                &anchor,
                &output,
                &profile,
                &repaired_source,
                operation,
                expected,
                next,
            );
            let mut bytes = Vec::new();
            assert!(
                maintenance::command_with(&args, None, &mut || Duration::ZERO, &mut bytes).is_err()
            );
            assert!(bytes.is_empty());
            assert_eq!(
                fs::read(case.config.store.join("delivery.bin")).unwrap(),
                journal
            );
            assert!(!output.exists());
            assert!(!repaired_source.exists());
            let prior = case.reader.snapshot();
            let bytes = invoke(&repair_args(args));
            let (fence, revision) = report(
                &bytes,
                "already_applied",
                Some((operation, 2, revoked)),
                (2, revoked, index + 1),
            );
            assert_eq!(fence, prior.revision + 1);
            assert_eq!(revision, fence);
            retention(&bytes, revision, true, true);
            assert_pair(&original, &output, &repaired_source, revision);
            unchanged_work(&before, &case.reader.snapshot());
            assert_eq!(
                case.reader.snapshot().control.ledger.elapsed,
                before.control.ledger.elapsed
            );
            anchor = output;
            source = repaired_source;
        }
        assert_eq!(fs::read(&original_path).unwrap(), original_bytes);
        assert_eq!(fs::read(&case.anchor).unwrap(), original_anchor);
        assert!(!root.0.join("peers").exists());
        assert!(!root.0.join("live-1.json").exists());
        assert!(!root.0.join("inventory-1.json").exists());
    }
}

#[test]
fn restricted_checkpoint_repair_rejects_changed_operations_epochs_and_unrelated_suffixes_before_fence()
 {
    let root = Root::new();
    let case = published_with_topology(&root, None);
    let original = read_checkpoint(&root.0.join("source-1.bin"));
    let profile = historical(&root, &root.0.join("source-1.bin"), "original");
    remove_current_inputs(&root);
    lose_exports(
        &case,
        &topology_arguments(
            &case,
            &case.anchor,
            &root.0.join("lost.anchor"),
            &profile,
            &root.0.join("lost.bin"),
            501,
            1,
            Some(2),
        ),
        false,
    );
    select(&root, &case.config, 2, false);
    let canonical = case.config.store.join("delivery.bin");
    let journal = fs::read(&canonical).unwrap();
    let pending = case.config.store.join("delivery.pending");
    fs::write(&pending, b"keep rejected recovery staging").unwrap();
    for variant in 0..5 {
        select(&root, &case.config, 2, false);
        let output = root.0.join(format!("rejected-{variant}.anchor"));
        let source = root.0.join(format!("rejected-{variant}.bin"));
        let mut args = repair_args(topology_arguments(
            &case,
            &case.anchor,
            &output,
            &profile,
            &source,
            501,
            1,
            Some(2),
        ));
        match variant {
            0 => args[4] = "502".into(),
            1 => {
                args[5] = "2".into();
                args[6] = "3".into();
            }
            2 => {
                args = repair_args(topology_arguments(
                    &case,
                    &case.anchor,
                    &output,
                    &profile,
                    &source,
                    501,
                    2,
                    None,
                ))
            }
            3 => select(&root, &case.config, 1, false),
            _ => args.push("--recover-credential-checkpoint".into()),
        }
        let mut bytes = Vec::new();
        assert!(
            maintenance::command_with(&args, None, &mut || Duration::ZERO, &mut bytes).is_err(),
            "variant {variant} escaped exact operation custody"
        );
        assert!(bytes.is_empty());
        assert_eq!(fs::read(&canonical).unwrap(), journal);
        assert_eq!(
            fs::read(&pending).unwrap(),
            b"keep rejected recovery staging"
        );
        assert!(!output.exists());
        assert!(!source.exists());
    }
    select(&root, &case.config, 2, false);
    let output = root.0.join("valid-repair.anchor");
    let source = root.0.join("valid-repair.bin");
    let bytes = invoke(&repair_args(topology_arguments(
        &case,
        &case.anchor,
        &output,
        &profile,
        &source,
        501,
        1,
        Some(2),
    )));
    let (_, revision) = report(
        &bytes,
        "already_applied",
        Some((501, 2, false)),
        (2, false, 1),
    );
    assert_pair(&original, &output, &source, revision);
    assert!(!pending.exists());
    let own_profile = historical(&root, &source, "own-cut");
    lose_exports(
        &case,
        &topology_arguments(
            &case,
            &output,
            &root.0.join("lost-next.anchor"),
            &own_profile,
            &root.0.join("lost-next.bin"),
            502,
            2,
            Some(3),
        ),
        false,
    );
    select(&root, &case.config, 3, false);
    let journal = fs::read(&canonical).unwrap();
    for operation in [501, 502] {
        let (expected, next) = if operation == 501 { (1, 2) } else { (2, 3) };
        let args = repair_args(topology_arguments(
            &case,
            &case.anchor,
            &root.0.join(format!("unrelated-{operation}.anchor")),
            &profile,
            &root.0.join(format!("unrelated-{operation}.bin")),
            operation,
            expected,
            Some(next),
        ));
        let mut bytes = Vec::new();
        assert!(
            maintenance::command_with(&args, None, &mut || Duration::ZERO, &mut bytes).is_err()
        );
        assert!(bytes.is_empty());
        assert_eq!(fs::read(&canonical).unwrap(), journal);
    }
    // The later operation succeeds from its own independent pre-operation cut.
    let next_output = root.0.join("own-repair.anchor");
    let next_source = root.0.join("own-repair.bin");
    let bytes = invoke(&repair_args(topology_arguments(
        &case,
        &output,
        &next_output,
        &own_profile,
        &next_source,
        502,
        2,
        Some(3),
    )));
    let (_, revision) = report(
        &bytes,
        "already_applied",
        Some((502, 3, false)),
        (3, false, 2),
    );
    assert_pair(&original, &next_output, &next_source, revision);
}

#[test]
fn restricted_checkpoint_repair_retains_fence_only_and_post_acknowledgment_failures() {
    for initially_applied in [false, true] {
        let root = Root::new();
        let case = published_with_topology(&root, Some(true));
        let before = case.reader.snapshot();
        let original = read_checkpoint(&root.0.join("source-1.bin"));
        let profile = historical(&root, &root.0.join("source-1.bin"), "interrupted");
        remove_current_inputs(&root);
        lose_exports(
            &case,
            &topology_arguments(
                &case,
                &case.anchor,
                &root.0.join("lost.anchor"),
                &profile,
                &root.0.join("lost.bin"),
                501,
                1,
                Some(2),
            ),
            !initially_applied,
        );
        select(
            &root,
            &case.config,
            if initially_applied { 2 } else { 1 },
            false,
        );
        let prior = case.reader.snapshot();
        let output = root.0.join("deadline.anchor");
        let source = root.0.join("deadline.bin");
        let mut expired = false;
        let mut bytes = Vec::new();
        let error = maintenance::command_with(
            &repair_args(topology_arguments(
                &case,
                &case.anchor,
                &output,
                &profile,
                &source,
                501,
                1,
                Some(2),
            )),
            None,
            &mut || {
                expired |= case.reader.snapshot().revision > prior.revision;
                if expired {
                    Duration::from_secs(3600)
                } else {
                    Duration::ZERO
                }
            },
            &mut bytes,
        )
        .unwrap_err();
        assert!(expired);
        assert!(error.contains("deadline reached"), "{error}");
        let (fence, revision) = report(
            &bytes,
            if initially_applied {
                "already_applied"
            } else {
                "refused"
            },
            initially_applied.then_some((501, 2, false)),
            (
                if initially_applied { 2 } else { 1 },
                false,
                u64::from(initially_applied),
            ),
        );
        assert_eq!(fence, prior.revision + 1);
        assert_eq!(revision, fence);
        retention(&bytes, revision, true, true);
        assert_pair(&original, &output, &source, revision);
        let next_profile = historical(&root, &source, "report-failure");
        let next_output = root.0.join("reported.anchor");
        let next_source = root.0.join("reported.bin");
        let mut writer = Output {
            bytes: Vec::new(),
            remaining: Some(0),
            expire: None,
        };
        let error = maintenance::command_with(
            &repair_args(topology_arguments(
                &case,
                &output,
                &next_output,
                &next_profile,
                &next_source,
                501,
                1,
                Some(2),
            )),
            None,
            &mut || Duration::ZERO,
            &mut writer,
        )
        .unwrap_err();
        assert!(error.contains("result output failed"), "{error}");
        assert!(writer.bytes.is_empty());
        let revision = revision + 1 + u64::from(!initially_applied);
        assert_eq!(case.reader.snapshot().revision, revision);
        assert_pair(&original, &next_output, &next_source, revision);
        unchanged_work(&before, &case.reader.snapshot());
        assert_eq!(
            case.reader.snapshot().control.ledger.elapsed,
            before.control.ledger.elapsed
        );
        assert!(!root.0.join("peers").exists());
    }
}
