//! Original process review, native publication and independently retained v2
//! custody. These fixtures exercise enforcement, not helper judgment quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::{
    MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint,
};

fn saved(root: &Directory, name: &str) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(
        &mut io::Cursor::new(fs::read(root.path(name)).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap()
}

fn history(root: &Directory, name: &str) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(
        &mut io::Cursor::new(fs::read(root.path(name)).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap()
}

fn recovery(root: &Directory, input: &str, output: &str) -> SubmitProfile {
    SubmitProfile::decode(
        recovery_text(root, Some(output))
            .replace("first.anchor", input)
            .as_bytes(),
    )
    .unwrap()
}

fn selected(root: &Directory, input: &str, output: &Path, operation: u64) -> Selection {
    Selection::decode(
        format!(
            r#"{{"schema_version":"fa.mediated-submit/2",
            "initial":{{"topology_path":"{}","inventory_path":"{}"}},
            "source_checkpoint":"{}","retain_checkpoint":"{}",
            "next":{{"topology_path":"{}","inventory_path":"{}"}},"operation":{operation}}}"#,
            root.input(1, false).display(),
            root.path("saved-inventory.json").display(),
            root.path(input).display(),
            output.display(),
            root.input(2, true).display(),
            root.path("live-inventory.json").display(),
        )
        .as_bytes(),
    )
    .unwrap()
}

fn paired(root: &Directory, source: &str, anchor: &str) -> TopologyCheckpoint {
    let saved = saved(root, source);
    let anchor = history(root, anchor);
    assert_eq!(saved.history_anchor(), &anchor);
    let config = configured(root, 0);
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(saved.minimum().journal_revision, state.revision);
    assert_eq!(saved.minimum().control_sequence, state.control.sequence);
    assert_eq!(saved.minimum().authority_epoch, state.control.ledger.epoch);
    saved
}

#[test]
fn missing_next_source_keeps_the_pair_for_corrected_publication_and_fresh_human_review() {
    for approve in [true, false] {
        let root = Directory::new();
        seed_retaining(&root, 0, true);
        let original = paired(&root, "first.source", "first.anchor");
        assert_eq!(original.work().checks, 2);
        assert!(original.available());
        fs::remove_file(root.path("live-inventory.json")).unwrap();
        let mut config = configured(&root, 3);
        config.programs.clear();
        let document = document(&config, 2, 3, 2, ElapsedTick(100000));
        let profile = recovery(&root, "first.anchor", "failed.anchor");
        let topology = selected(&root, "first.source", &root.path("failed.source"), 100);
        let error = submit(
            config,
            Submission {
                document: &document,
                profile: &profile,
                peers: None,
                credibility: None,
            },
            &topology,
            || ElapsedTick(1001),
        )
        .err()
        .unwrap();
        assert!(
            error.contains("guarded submission recovery acknowledged"),
            "{error}"
        );
        assert!(!error.contains("retention failed"), "{error}");
        let retained = paired(&root, "failed.source", "failed.anchor");
        assert_eq!(
            retained.history_anchor().revision(),
            original.history_anchor().revision() + 3
        );
        assert_eq!(retained.expected(), original.expected());
        assert_eq!(retained.topology_bytes(), original.topology_bytes());
        assert_eq!(retained.inventory_bytes(), original.inventory_bytes());
        assert_eq!(retained.work(), original.work());
        assert_eq!(retained.last_operation(), original.last_operation());
        assert!(!retained.available());
        let config = configured(&root, 0);
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 1);
        assert_eq!(state.control.ledger.charged, 5);
        assert!(!state.control.ledger.stages.contains_key(&2));

        root.install(2, true);
        // The withdrawn saved cut fences once, then the explicit rebind revokes
        // once. The actor supplies that actual epoch; the command never edits it.
        let epoch = retained.minimum().authority_epoch + 2;
        let config = configured(&root, epoch);
        let next = super::document(&config, 2, epoch, 2, ElapsedTick(100000));
        let profile = recovery(&root, "failed.anchor", "second.anchor");
        let topology = selected(&root, "failed.source", &root.path("second.source"), 200);
        let human = reviewer(
            &config,
            2,
            if approve {
                ReviewDecision::Approve
            } else {
                ReviewDecision::Reject
            },
        );
        let result = submit(
            config,
            Submission {
                document: &next,
                profile: &profile,
                peers: None,
                credibility: None,
            },
            &topology,
            || ElapsedTick(1002),
        )
        .unwrap();
        human.join().unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(executed(&result), approve);
        assert_eq!(result.cleanup_pending, 0);
        let retained = paired(&root, "second.source", "second.anchor");
        let checks = if approve { 4 } else { 3 };
        assert_eq!(retained.work().checks, checks);
        assert_eq!(
            retained.work().reserved_read_bytes,
            checks * TOPOLOGY_REFRESH_READ_BYTES
        );
        assert_eq!(retained.last_operation(), 200 + checks - 2);
        assert_eq!(retained.expected(), &loaded(2));
        let config = configured(&root, epoch);
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, if approve { 2 } else { 1 });
        assert_eq!(state.control.ledger.charged, if approve { 11 } else { 5 });
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(
            state.payload,
            if approve {
                b"second".as_slice()
            } else {
                b"first".as_slice()
            }
        );
    }
}

#[test]
fn retained_receipt_recovery_can_repeat_without_next_sources_helpers_or_new_effects() {
    let root = Directory::new();
    let document = seed_retaining(&root, 0, true);
    let original = paired(&root, "first.source", "first.anchor");
    for name in ["evidence.json", "live-topology.json", "live-inventory.json"] {
        fs::remove_file(root.path(name)).unwrap();
    }
    let mut previous = original.clone();
    for (input, source, output, retained) in [
        (
            "first.anchor",
            "first.source",
            "receipt.anchor",
            "receipt.source",
        ),
        (
            "receipt.anchor",
            "receipt.source",
            "again.anchor",
            "again.source",
        ),
    ] {
        let profile = recovery(&root, input, output);
        let topology = selected(&root, source, &root.path(retained), 100);
        let mut config = configured(&root, 0);
        config.programs.clear();
        let missing_credibility = root.path("missing-credibility.json");
        let result = submit(
            config,
            Submission {
                document: &document,
                profile: &profile,
                peers: None,
                credibility: Some(&missing_credibility),
            },
            &topology,
            || ElapsedTick(1_000_000),
        )
        .unwrap();
        assert!(executed(&result));
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert_eq!(result.cleanup_pending, 0);
        let next = paired(&root, retained, output);
        assert_eq!(
            next.history_anchor().revision(),
            previous.history_anchor().revision() + 3
        );
        assert_eq!(next.work(), original.work());
        assert_eq!(next.last_operation(), original.last_operation());
        assert_eq!(next.expected(), original.expected());
        assert!(!next.available());
        previous = next;
    }
    let config = configured(&root, 0);
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"first");
    assert_eq!(state.control.ledger.charged, 5);
    assert_eq!(state.control.ledger.reserved, 0);
    assert!(!config.socket(1).exists());
}

#[test]
fn existing_inside_store_and_canonical_alias_outputs_refuse_before_fence_or_cleanup() {
    let root = Directory::new();
    let document = seed_retaining(&root, 0, true);
    std::os::unix::fs::symlink(&root.0, root.path("alias")).unwrap();
    fs::write(
        root.path("store/delivery.pending"),
        b"untouched staged image",
    )
    .unwrap();
    let before = root.bytes();
    let old_source = fs::read(root.path("first.source")).unwrap();
    for path in [
        root.path("first.source"),
        root.path("store/inside.source"),
        root.path("next.anchor"),
        root.path("alias/next.anchor"),
    ] {
        let profile = recovery(&root, "first.anchor", "next.anchor");
        let topology = selected(&root, "first.source", &path, 100);
        let mut calls = 0;
        let error = submit(
            configured(&root, 3),
            Submission {
                document: &document,
                profile: &profile,
                peers: None,
                credibility: None,
            },
            &topology,
            || {
                calls += 1;
                ElapsedTick(1001)
            },
        )
        .err()
        .unwrap();
        assert!(!error.contains("acknowledged at revision"), "{error}");
        assert_eq!(
            calls, 1,
            "only the original invocation clock may precede custody admission"
        );
        assert_eq!(root.bytes(), before);
        assert_eq!(
            fs::read(root.path("store/delivery.pending")).unwrap(),
            b"untouched staged image"
        );
        assert_eq!(fs::read(root.path("first.source")).unwrap(), old_source);
        assert!(!root.path("next.anchor").exists());
    }
}

#[test]
fn either_export_race_preserves_its_companion_and_the_triggering_failure() {
    for race_source in [true, false] {
        let root = Directory::new();
        seed_retaining(&root, 0, true);
        let original = saved(&root, "first.source");
        fs::remove_file(root.path("live-inventory.json")).unwrap();
        let config = configured(&root, 3);
        let document = document(&config, 2, 3, 2, ElapsedTick(100000));
        let profile = recovery(&root, "first.anchor", "raced.anchor");
        let topology = selected(&root, "first.source", &root.path("raced.source"), 100);
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let mut raced = false;
        let occupied = root.path(if race_source {
            "raced.source"
        } else {
            "raced.anchor"
        });
        let error = submit(
            config,
            Submission {
                document: &document,
                profile: &profile,
                peers: None,
                credibility: None,
            },
            &topology,
            || {
                let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
                if !raced && state.revision > original.history_anchor().revision() {
                    fs::write(&occupied, b"independent racing writer").unwrap();
                    raced = true;
                }
                ElapsedTick(1001)
            },
        )
        .err()
        .unwrap();
        assert!(raced);
        assert!(
            error.contains("guarded submission recovery acknowledged"),
            "{error}"
        );
        assert!(error.contains("retention failed"), "{error}");
        assert!(
            error.contains("No such file") || error.contains("not found"),
            "original input failure was lost: {error}"
        );
        assert_eq!(fs::read(&occupied).unwrap(), b"independent racing writer");
        let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(state.revision, original.history_anchor().revision() + 3);
        assert_eq!(state.executions, 1);
        assert_eq!(state.control.ledger.charged, 5);
        if race_source {
            assert_eq!(history(&root, "raced.anchor").revision(), state.revision);
        } else {
            let retained = saved(&root, "raced.source");
            assert_eq!(retained.history_anchor().revision(), state.revision);
            assert_eq!(retained.work(), original.work());
            assert_eq!(retained.last_operation(), original.last_operation());
            assert!(!retained.available());
        }
    }
}

#[test]
fn deadline_after_rebind_retains_the_new_graph_for_a_corrected_fresh_publication() {
    let root = Directory::new();
    seed_retaining(&root, 0, true);
    let original = saved(&root, "first.source");
    let config = configured(&root, 3);
    let document = document(&config, 2, 3, 2, ElapsedTick(100000));
    let profile = recovery(&root, "first.anchor", "late.anchor");
    let topology = selected(&root, "first.source", &root.path("late.source"), 100);
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let error = submit(
        config,
        Submission {
            document: &document,
            profile: &profile,
            peers: None,
            credibility: None,
        },
        &topology,
        || {
            let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
            ElapsedTick(if state.control.ledger.epoch >= 3 {
                1_000_000
            } else {
                1001
            })
        },
    )
    .err()
    .unwrap();
    assert!(error.contains("deadline reached"), "{error}");
    assert!(!error.contains("retention failed"), "{error}");
    let retained = paired(&root, "late.source", "late.anchor");
    assert_eq!(
        retained.history_anchor().revision(),
        original.history_anchor().revision() + 4
    );
    assert_eq!(retained.expected(), &loaded(2));
    assert_eq!(retained.work(), original.work());
    assert_eq!(retained.last_operation(), 100);
    assert!(retained.available());
    root.install(3, true);
    let epoch = retained.minimum().authority_epoch + 3;
    let config = configured(&root, epoch);
    let next = super::document(&config, 2, epoch, 2, ElapsedTick(100000));
    let profile = recovery(&root, "late.anchor", "done.anchor");
    let topology = selected(&root, "late.source", &root.path("done.source"), 200);
    let human = reviewer(&config, 2, ReviewDecision::Approve);
    let result = submit(
        config,
        Submission {
            document: &next,
            profile: &profile,
            peers: None,
            credibility: None,
        },
        &topology,
        || ElapsedTick(1002),
    )
    .unwrap();
    human.join().unwrap();
    assert!(executed(&result));
    assert!(result.failure.is_none(), "{:?}", result.failure);
    let done = paired(&root, "done.source", "done.anchor");
    assert_eq!(done.expected(), &loaded(3));
    assert_eq!(done.work().checks, 4);
    assert_eq!(done.last_operation(), 202);
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 2);
    assert_eq!(state.control.ledger.charged, 11);
}

#[test]
fn stale_mixed_and_foreign_pairs_never_export_or_clean_staging() {
    let root = Directory::new();
    let document = seed_retaining(&root, 0, true);
    let foreign = Directory::new();
    seed_retaining(&foreign, 0, true);
    fs::copy(foreign.path("first.source"), root.path("foreign.source")).unwrap();
    let profile = recovery(&root, "first.anchor", "current.anchor");
    let topology = selected(&root, "first.source", &root.path("current.source"), 100);
    let result = submit(
        configured(&root, 0),
        Submission {
            document: &document,
            profile: &profile,
            peers: None,
            credibility: None,
        },
        &topology,
        || ElapsedTick(1001),
    )
    .unwrap();
    assert!(executed(&result));
    assert!(result.failure.is_none());
    paired(&root, "current.source", "current.anchor");
    fs::write(
        root.path("store/delivery.pending"),
        b"keep independent staging",
    )
    .unwrap();
    let before = root.bytes();
    for (anchor, source) in [
        ("first.anchor", "first.source"),
        ("current.anchor", "first.source"),
        ("first.anchor", "current.source"),
        ("current.anchor", "foreign.source"),
    ] {
        let profile = recovery(&root, anchor, "refused.anchor");
        let topology = selected(&root, source, &root.path("refused.source"), 100);
        let error = submit(
            configured(&root, 0),
            Submission {
                document: &document,
                profile: &profile,
                peers: None,
                credibility: None,
            },
            &topology,
            || ElapsedTick(1002),
        )
        .err()
        .unwrap();
        assert!(!error.contains("acknowledged at revision"), "{error}");
        assert_eq!(root.bytes(), before);
        assert_eq!(
            fs::read(root.path("store/delivery.pending")).unwrap(),
            b"keep independent staging"
        );
        assert!(!root.path("refused.source").exists());
        assert!(!root.path("refused.anchor").exists());
    }
}

#[test]
fn poisoned_rebind_cannot_export_the_earlier_recovery_cut_as_current() {
    let root = Directory::new();
    seed_retaining(&root, 0, true);
    let original = saved(&root, "first.source");
    let config = configured(&root, 3);
    let document = document(&config, 2, 3, 2, ElapsedTick(100000));
    let profile = recovery(&root, "first.anchor", "poisoned.anchor");
    let topology = selected(&root, "first.source", &root.path("poisoned.source"), 100);
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let mut injected = false;
    let error = submit_with_elapsed(
        config,
        Submission {
            document: &document,
            profile: &profile,
            peers: None,
            credibility: None,
        },
        &topology,
        || ElapsedTick(1001),
        Instant::now(),
        &mut || {
            let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
            if !injected && state.revision > original.history_anchor().revision() {
                fs::create_dir(root.path("store/delivery.pending")).unwrap();
                injected = true;
            }
            Duration::ZERO
        },
    )
    .err()
    .unwrap();
    assert!(injected);
    assert!(
        error.contains("guarded submission recovery acknowledged"),
        "{error}"
    );
    assert!(
        error.contains("retention failed") || error.contains("quarantin"),
        "{error}"
    );
    assert!(!root.path("poisoned.anchor").exists());
    assert!(!root.path("poisoned.source").exists());
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.revision, original.history_anchor().revision() + 3);
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.charged, 5);
}

#[cfg(target_os = "linux")]
#[test]
fn human_and_stop_socket_destinations_refuse_before_native_recovery() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let root = Directory::new();
    let document = seed_retaining(&root, 0, true);
    fs::create_dir(root.path("peers")).unwrap();
    fs::set_permissions(root.path("peers"), fs::Permissions::from_mode(0o700)).unwrap();
    let metadata = fs::metadata(root.path("peers")).unwrap();
    let peers = PeerProfile::decode(format!(
        r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5}},
        "reviewer_id":77,"socket_directory":"{}","supervisor":{{"uid":{},"gid":{},"pid":null}},
        "reviewer":{{"uid":{},"gid":{},"pid":null}},"candidate_limit":4,"runtime_ms":15000,"poll_ms":1}}"#,
        root.path("peers").display(), metadata.uid(), metadata.gid(), metadata.uid(), metadata.gid()
    ).as_bytes()).unwrap();
    let before = root.bytes();
    for (source, anchor) in [
        (root.path("peers/review-1.sock"), "safe.anchor"),
        (root.path("peers/stop-1.sock"), "safe.anchor"),
        (root.path("safe.source"), "peers/review-1.sock"),
        (root.path("safe.source"), "peers/stop-1.sock"),
    ] {
        let profile = recovery(&root, "first.anchor", anchor);
        let topology = selected(&root, "first.source", &source, 100);
        let error = submit(
            configured(&root, 0),
            Submission {
                document: &document,
                profile: &profile,
                peers: Some(&peers),
                credibility: None,
            },
            &topology,
            || ElapsedTick(1001),
        )
        .err()
        .unwrap();
        assert!(error.contains("sockets must be distinct"), "{error}");
        assert_eq!(root.bytes(), before);
        assert!(!source.exists());
        assert!(!root.path(anchor).exists());
    }
}
