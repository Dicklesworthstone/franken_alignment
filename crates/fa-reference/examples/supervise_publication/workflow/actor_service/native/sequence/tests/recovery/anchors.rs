//! Operator custody composes with actual reviewed native publication and its
//! original cooperative replay. Archives never create source or human evidence.
use super::*;
use super::super::super::anchors as custody;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
use std::{fs, io::Cursor, os::unix::fs::PermissionsExt, path::Path};

fn options(input: Option<&Path>, output: Option<&Path>) -> custody::Options {
    let mut args = [
        "serve-native-generated", "config.json", "sequence.json", "reviewer.json",
    ].map(str::to_owned).to_vec();
    for (flag, path) in [
        ("--history-anchor", input),
        ("--retain-history-anchor", output),
    ] {
        if let Some(path) = path {
            args.extend([flag.to_owned(), path.to_str().unwrap().to_owned()]);
        }
    }
    custody::take_options(&args).unwrap().1
}

fn select(
    config: &Config,
    entries: &[Entry],
    peers: &PeerProfile,
    after: Option<u64>,
    input: Option<&Path>,
    output: Option<&Path>,
) -> custody::Selection {
    options(input, output).admit(
        config,
        &entries.iter().map(|entry| (&entry.actor, entry.precheck.as_ref())).collect::<Vec<_>>(),
        peers, None, after, None,
    ).unwrap()
}

fn archive(path: &Path) -> FileHistoryAnchor {
    assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
    FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()), MAX_HISTORY_ANCHOR_BYTES,
    ).unwrap()
}

struct Retained {
    seed: Seed,
    path: PathBuf,
    earlier: Vec<u8>,
}

fn seed_retained(root: &Root, messages: usize, name: &str, now: u64) -> Retained {
    let (config, mut entries, reviewer) = selected(root, false);
    entries.truncate(2);
    for entry in &mut entries {
        entry.inputs.stream = StreamProfile::new(9, 1, messages, 64, 256).unwrap();
    }
    let reader = Reader::new(&config, &entries[0]);
    let path = root.0.join(name);
    let history = select(&config, &entries, &reviewer, None, None, Some(&path));
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
    let mut earlier = None;
    let result = run_with_history(
        config, entries, &reviewer, None, (None, None), None, Some(&history),
        || {
            let canonical = reader.store.join("delivery.bin");
            if earlier.is_none() && canonical.exists() && reader.publication().executions == 1 {
                earlier = Some(fs::read(&canonical).unwrap());
            }
            humans.poll();
            ElapsedTick(now)
        },
        &mut output,
    );
    assert!(result.is_ok(), "actual retained prefix creation failed: {result:?}");
    assert_eq!(output.documents.len(), 2);
    assert!(output.join().iter().all(executed));
    let packets = humans.join().into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(packets.len(), 2);
    assert_eq!(reader.stream().publication.payload, b"AA");
    assert_eq!(reader.publication().executions, 2);
    assert_eq!(reader.publication().control.ledger.epoch, 0);
    assert_eq!(archive(&path).revision(), reader.publication().revision);
    closed(&actor_profiles, &reviewer);
    let mut config = fixture::configured(root);
    config.programs = fixture::native_programs_at_epoch(root, 1);
    Retained {
        seed: Seed { config, reviewer, reader, packets },
        path,
        earlier: earlier.expect("the actual first confirmed Message was observed"),
    }
}

#[test]
fn retained_native_creation_authorizes_only_its_actual_append_only_suffix_and_finish() {
    for finish_only in [false, true] {
        let root = Root::new();
        let Retained { seed: Seed { config, reviewer, reader, .. }, path, .. } =
            seed_retained(&root, if finish_only { 2 } else { 4 }, "prefix.anchor", 1000);
        let original = fs::read(&path).unwrap();
        let before = reader.progress(8);
        let entries = if finish_only {
            vec![entry(&root, &config, &reader, 93, 8, Release::Finish)]
        } else {
            suffix(&root, &config, &reader)
        };
        let next = root.0.join("successor.anchor");
        let history = select(&config, &entries, &reviewer, Some(AFTER), Some(&path), Some(&next));
        let mut output = Output::new(&entries);
        let actor_profiles = output.profiles.clone();
        let mut humans = Reviewers::new(&reviewer, &reader, &entries);
        let result = run_with_history(
            config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
            || { humans.poll(); ElapsedTick(1001) },
            &mut output,
        );
        assert!(result.is_ok(), "{result:?}");
        let count = if finish_only { 1 } else { 2 };
        assert_eq!(output.documents.len(), count);
        assert!(output.join().iter().all(executed));
        let packets = humans.join();
        assert_eq!(packets.len(), count);
        assert!(packets.iter().all(|packet| packet.action().spec().policy_epoch == 1));
        let final_state = reader.stream();
        assert!(final_state.confirmed.finished());
        assert_eq!(final_state.confirmed, final_state.published);
        assert_eq!(final_state.publication.executions, 2 + count as u64);
        assert_eq!(final_state.publication.control.ledger.epoch, 1);
        assert!(final_state.publication.stop.is_none());
        if finish_only {
            assert_eq!(reader.progress(8).numerical.numerical, before.numerical.numerical);
            assert_eq!(final_state.publication.payload, b"AA");
            reader.no_generation(9);
        } else {
            assert_eq!(reader.progress(9).numerical.numerical.position, 12);
            assert_eq!(final_state.publication.payload, b"AAA");
        }
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(archive(&next).revision(), final_state.publication.revision);
        assert!(archive(&next).revision() > archive(&path).revision());
        closed(&actor_profiles, &reviewer);
    }
}

#[test]
fn selected_native_history_is_frozen_before_live_replay_and_never_reread_or_substituted() {
    let root = Root::new();
    let Retained { seed: Seed { config, reviewer, reader, .. }, path, .. } =
        seed_retained(&root, 2, "prefix.anchor", 1000);
    let entries = vec![entry(&root, &config, &reader, 93, 8, Release::Finish)];
    let next = root.0.join("successor.anchor");
    let history = select(&config, &entries, &reviewer, Some(AFTER), Some(&path), Some(&next));
    fs::write(&path, b"later pathname bytes are not the selected history").unwrap();
    let mut output = Output::new(&entries);
    let mut humans = Reviewers::new(&reviewer, &reader, &entries);
    let result = run_with_history(
        config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
        || { humans.poll(); ElapsedTick(1001) },
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(output.join().iter().all(executed));
    assert_eq!(humans.join().len(), 1);
    assert!(reader.stream().confirmed.finished());
    assert_eq!(archive(&next).revision(), reader.publication().revision);
    assert_eq!(fs::read(&path).unwrap(), b"later pathname bytes are not the selected history");
}

#[test]
fn retained_native_history_rejects_a_valid_older_cut_before_fence_and_can_retry_the_real_cut() {
    let root = Root::new();
    let Retained { seed: Seed { config, reviewer, reader, .. }, path, earlier } =
        seed_retained(&root, 2, "prefix.anchor", 1000);
    let canonical_path = reader.store.join("delivery.bin");
    let canonical = fs::read(&canonical_path).unwrap();
    let original = fs::read(&path).unwrap();
    let entries = vec![entry(&root, &config, &reader, 93, 8, Release::Finish)];
    let next = root.0.join("successor.anchor");
    let history = select(&config, &entries, &reviewer, Some(AFTER), Some(&path), Some(&next));
    fs::write(&canonical_path, &earlier).unwrap();
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let error = run_with_history(
        config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
        || ElapsedTick(1001), &mut output,
    ).unwrap_err();
    assert!(error.contains("Stale"), "{error}");
    assert_eq!(fs::read(&canonical_path).unwrap(), earlier);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!next.exists());
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    closed(&actor_profiles, &reviewer);

    // Only restore the original acknowledged journal. The same frozen input and
    // still-unused output selection can then finish without a failed-attempt Fence.
    fs::write(&canonical_path, canonical).unwrap();
    let mut config = fixture::configured(&root);
    config.programs = fixture::native_programs_at_epoch(&root, 1);
    let entries = vec![entry(&root, &config, &reader, 93, 8, Release::Finish)];
    let mut output = Output::new(&entries);
    let mut humans = Reviewers::new(&reviewer, &reader, &entries);
    let result = run_with_history(
        config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
        || { humans.poll(); ElapsedTick(1001) }, &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(output.join().iter().all(executed));
    assert_eq!(humans.join().len(), 1);
    assert_eq!(reader.publication().control.ledger.epoch, 1);
    assert_eq!(reader.publication().executions, 3);
    assert_eq!(archive(&next).revision(), reader.publication().revision);
}

#[test]
fn retained_native_history_rejects_an_equal_counter_branch_and_a_foreign_store_unchanged() {
    for foreign in [false, true] {
        let first_root = Root::new();
        let original = seed_retained(&first_root, 4, "first.anchor", 1000);
        let first_bytes = fs::read(&original.path).unwrap();
        let first_revision = archive(&original.path).revision();
        let second_root = Root::new();
        let target = if foreign { &second_root } else { &first_root };
        if !foreign {
            // Keep the original valid store separately. Re-create another valid
            // native history at the SAME canonical identity with a different clock.
            fs::rename(&original.seed.reader.store, first_root.0.join("original-store")).unwrap();
        }
        let Retained { seed: Seed { config, reviewer, reader, .. }, path, .. } =
            seed_retained(target, 4, "other.anchor", 1002);
        assert_eq!(archive(&path).revision(), first_revision);
        let before = reader.publication();
        let canonical_path = reader.store.join("delivery.bin");
        let canonical = fs::read(&canonical_path).unwrap();
        let entries = suffix(target, &config, &reader);
        let next = target.0.join("refused.anchor");
        let history = select(
            &config, &entries, &reviewer, Some(AFTER), Some(&original.path), Some(&next),
        );
        let mut output = Output::new(&entries);
        let actor_profiles = output.profiles.clone();
        let error = run_with_history(
            config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
            || ElapsedTick(1003), &mut output,
        ).unwrap_err();
        assert!(error.contains("Binding"), "{error}");
        assert_eq!(reader.publication(), before);
        assert_eq!(fs::read(&canonical_path).unwrap(), canonical);
        assert_eq!(fs::read(&original.path).unwrap(), first_bytes);
        assert!(!next.exists());
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        closed(&actor_profiles, &reviewer);
    }
}

#[test]
fn retained_native_fence_survives_early_suffix_refusal_without_source_or_numerical_work() {
    let root = Root::new();
    let Retained { seed: Seed { config, reviewer, reader, .. }, path, .. } =
        seed_retained(&root, 4, "prefix.anchor", 1000);
    let before = reader.progress(8);
    let original = fs::read(&path).unwrap();
    let entries = suffix(&root, &config, &reader);
    let next = root.0.join("fenced.anchor");
    let history = select(&config, &entries, &reviewer, Some(91), Some(&path), Some(&next));
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let result = run_with_history(
        config, entries, &reviewer, None, (Some(91), None), None, Some(&history),
        || ElapsedTick(1001), &mut output,
    );
    assert!(result.is_err());
    assert!(!result.unwrap_err().contains("retention failed"));
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    assert_preserved_prefix(&reader, &before);
    let after = reader.publication();
    assert_eq!(after.revision, before.publication.revision + 1);
    assert_eq!(after.control.ledger.epoch, 1);
    assert!(after.stop.is_none());
    assert_eq!(archive(&next).revision(), after.revision);
    assert_eq!(fs::read(&path).unwrap(), original);
    closed(&actor_profiles, &reviewer);
}

#[test]
fn retained_native_output_collision_before_clock_is_read_only_and_remains_retryable() {
    let root = Root::new();
    let Retained { seed: Seed { config, reviewer, reader, .. }, path, .. } =
        seed_retained(&root, 2, "prefix.anchor", 1000);
    let before = reader.publication();
    let canonical = fs::read(reader.store.join("delivery.bin")).unwrap();
    let entries = vec![entry(&root, &config, &reader, 93, 8, Release::Finish)];
    let next = root.0.join("successor.anchor");
    let history = select(&config, &entries, &reviewer, Some(AFTER), Some(&path), Some(&next));
    fs::write(&next, b"independently created custody must not be replaced").unwrap();
    let mut output = Output::new(&entries);
    let error = run_with_history(
        config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
        || panic!("colliding custody acquired the live clock"), &mut output,
    ).unwrap_err();
    assert!(error.contains("must be a new file"));
    assert_eq!(reader.publication(), before);
    assert_eq!(fs::read(reader.store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(fs::read(&next).unwrap(), b"independently created custody must not be replaced");
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());

    fs::remove_file(&next).unwrap();
    let mut config = fixture::configured(&root);
    config.programs = fixture::native_programs_at_epoch(&root, 1);
    let entries = vec![entry(&root, &config, &reader, 93, 8, Release::Finish)];
    let mut output = Output::new(&entries);
    let mut humans = Reviewers::new(&reviewer, &reader, &entries);
    let result = run_with_history(
        config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
        || { humans.poll(); ElapsedTick(1001) }, &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(output.join().iter().all(executed));
    assert_eq!(humans.join().len(), 1);
    assert_eq!(reader.publication().control.ledger.epoch, 1);
    assert_eq!(reader.publication().executions, 3);
}

#[test]
fn late_native_archive_collision_does_not_repeat_an_executed_finish_or_send_stop() {
    let root = Root::new();
    let Retained { seed: Seed { config, reviewer, reader, .. }, path, .. } =
        seed_retained(&root, 2, "prefix.anchor", 1000);
    let original = fs::read(&path).unwrap();
    let entries = vec![entry(&root, &config, &reader, 93, 8, Release::Finish)];
    let next = root.0.join("successor.anchor");
    let history = select(&config, &entries, &reviewer, Some(AFTER), Some(&path), Some(&next));
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let mut humans = Reviewers::new(&reviewer, &reader, &entries);
    let mut collided = false;
    let error = run_with_history(
        config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
        || {
            if !collided {
                fs::write(&next, b"late private custodian").unwrap();
                collided = true;
            }
            humans.poll();
            ElapsedTick(1001)
        },
        &mut output,
    ).unwrap_err();
    assert!(error.starts_with("history anchor retention failed:"), "{error}");
    assert_eq!(output.documents.len(), 1);
    assert!(output.join().iter().all(executed));
    assert_eq!(humans.join().len(), 1);
    assert!(reader.stream().confirmed.finished());
    let after = reader.publication();
    assert_eq!(after.executions, 3);
    assert_eq!(after.control.ledger.epoch, 1);
    assert!(after.stop.is_none());
    assert_eq!(fs::read(&next).unwrap(), b"late private custodian");
    assert_eq!(fs::read(&path).unwrap(), original);
    closed(&actor_profiles, &reviewer);
}

#[test]
fn selected_native_anchor_keeps_the_ready_deadline_and_does_not_export_unacknowledged_history() {
    let root = Root::new();
    let Retained { seed: Seed { config, reviewer, reader, .. }, path, .. } =
        seed_retained(&root, 4, "prefix.anchor", 1000);
    let before = reader.progress(8);
    let canonical = fs::read(reader.store.join("delivery.bin")).unwrap();
    let original = fs::read(&path).unwrap();
    let entries = suffix(&root, &config, &reader);
    let total = replay_length(&config, &entries[0].inputs);
    let next = root.0.join("deadline.anchor");
    let history = select(&config, &entries, &reviewer, Some(AFTER), Some(&path), Some(&next));
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let mut calls = 0;
    let result = run_with_history(
        config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
        || {
            let current = calls;
            calls += 1;
            ElapsedTick(if current == total + 2 { 16_001 } else { 1001 })
        },
        &mut output,
    );
    assert!(result.is_err());
    assert_eq!(calls, total + 3);
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    assert_eq!(fs::read(reader.store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!next.exists());
    assert_preserved_prefix(&reader, &before);
    assert_eq!(reader.publication(), before.publication);
    closed(&actor_profiles, &reviewer);
}

#[test]
fn native_ready_stop_retains_its_acknowledged_archive_and_preserves_ack_on_export_failure() {
    for collide in [false, true] {
        let root = Root::new();
        let Retained { seed: Seed { config, reviewer, reader, .. }, path, .. } =
            seed_retained(&root, 4, "prefix.anchor", 1000);
        let before = reader.progress(8);
        let original = fs::read(&path).unwrap();
        let entries = suffix(&root, &config, &reader);
        let total = replay_length(&config, &entries[0].inputs);
        let next = root.0.join("stopped.anchor");
        let history = select(&config, &entries, &reviewer, Some(AFTER), Some(&path), Some(&next));
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        let mut output = Output::new(&entries);
        let actor_profiles = output.profiles.clone();
        let mut calls = 0;
        let mut controller = None;
        let error = run_with_history(
            config, entries, &reviewer, None, (Some(AFTER), None), None, Some(&history),
            || {
                if collide && calls == 0 {
                    fs::write(&next, b"independent archive wins the name").unwrap();
                }
                if calls == total + 2 {
                    controller = Some(stopping_at(&reviewer, 93));
                }
                calls += 1;
                ElapsedTick(1001)
            },
            &mut output,
        ).unwrap_err();
        assert!(error.starts_with("native admission Stop acknowledged at revision"), "{error}");
        assert_eq!(error.contains("history anchor retention failed:"), collide);
        let receipt = controller.expect("the anchored cursor reached Ready").join().unwrap();
        assert!(receipt.acknowledged() && receipt.drained());
        assert_eq!(receipt.binding.operation, 93);
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        assert_preserved_prefix(&reader, &before);
        let after = reader.publication();
        assert_eq!(after.revision, before.publication.revision + 2);
        assert_eq!(after.control.ledger.elapsed, before.publication.control.ledger.elapsed);
        assert_eq!(after.stop.unwrap().request().operation, 93);
        assert_eq!(fs::read(&path).unwrap(), original);
        if collide {
            assert_eq!(fs::read(&next).unwrap(), b"independent archive wins the name");
        } else {
            assert_eq!(archive(&next).revision(), after.revision);
        }
        closed(&actor_profiles, &reviewer);
    }
}

#[test]
fn resident_history_faulted_final_owner_replaces_the_startup_capture_without_exporting_it() {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, false);
    let native = &entries[0].inputs;
    let selected = bootstrap::selection(&config, native, None).unwrap();
    let (mut host, _) = bootstrap::prepare(&config, native, selected, false).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();

    let previous = root.0.join("previous.anchor");
    let initial = select(&config, &entries, &reviewer, None, None, Some(&previous));
    let mut initial_retention = custody::Retention::new(Some(&initial));
    initial_retention.observe(&host);
    initial_retention.finish(Ok(())).unwrap();
    let prior = fs::read(&previous).unwrap();
    let output = root.0.join("faulted.anchor");
    let history = select(
        &config, &entries, &reviewer, Some(AFTER), Some(&previous), Some(&output),
    );
    let mut retained = custody::Retention::new(Some(&history));
    retained.observe(&host); // The acknowledged startup cut was initially usable.
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    let pending = config.store.join("delivery.pending");
    fs::write(&pending, b"retained ambiguous staging").unwrap();
    let failed = host.observe_time(host.revision(), ElapsedTick(1001)).unwrap_err();
    assert!(matches!(&failed, JournalError::Io(_)));
    let original = format!("original native time observation failed: {failed:?}");

    // The final owner is poisoned. Its failed capture must replace the earlier
    // Ok cut, and retention cannot hide the original work error or clear staging.
    retained.observe(&host);
    let error = retained.finish(Err(original.clone())).unwrap_err();
    assert!(error.starts_with(&original), "{error}");
    assert!(error.contains("Unavailable"), "{error}");
    assert!(error.contains("history anchor retention failed:"), "{error}");
    assert!(!output.exists());
    assert_eq!(fs::read(&previous).unwrap(), prior);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(fs::read(&pending).unwrap(), b"retained ambiguous staging");
}
