//! History custody and one-time credibility activation share the same resident
//! owner. These synthetic fixtures establish workflow contracts, not evaluator
//! authentication, detector quality or hostile-process containment.
use super::*;
use std::path::Path;
use crate::workflow::actor_service::learned::anchors as archive;
use crate::workflow::actor_service::native::sequence::anchors as custody;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::FileHistoryAnchor;
use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::checked::source::{
    FileGeneratedSourceRecoveryStatus, GeneratedSourceProfile,
};

/// Preserve the incoming fixture unchanged. Capture a later acknowledged
/// original cut, retaining its valid predecessor solely for the rollback test.
/// The capsule names the next recovery's actual sequence and successor epoch.
fn retained_seed(root: &Root) -> (Seed, FileHistoryAnchor, Vec<u8>) {
    let mut seeded = seed(root);
    let predecessor = fs::read(seeded.config.store.join("delivery.bin")).unwrap();
    let native = inputs(root, &seeded.config);
    let selected = bootstrap::selection(&seeded.config, &native, None).unwrap();
    let (host, _) = bootstrap::prepare(&seeded.config, &native, selected, true).unwrap();
    let control = host.inspect().control;
    seeded.fresh = evidence::activation(
        &seeded.config,
        control.sequence,
        control.ledger.epoch + 1,
        21,
        3,
        2,
    );
    let retained = host.history_anchor().unwrap();
    drop(host);
    seeded.config.programs =
        fixture::native_programs_at_epoch(root, seeded.fresh.expected_epoch + 1);
    seeded.before = seeded.reader.progress(8);
    assert_eq!(retained.revision(), seeded.before.publication.revision);
    (seeded, retained, predecessor)
}

fn selection(
    config: &Config,
    entries: &[Entry],
    peers: &PeerProfile,
    input: &Path,
    output: &Path,
    credibility: &Path,
) -> custody::Selection {
    let args = vec![
        "serve-native-generated".to_owned(),
        "config".to_owned(),
        "sequence".to_owned(),
        "reviewer".to_owned(),
        "--after-message".to_owned(),
        AFTER.to_string(),
        "--history-anchor".to_owned(),
        input.to_str().unwrap().to_owned(),
        "--retain-history-anchor".to_owned(),
        output.to_str().unwrap().to_owned(),
    ];
    let (_, options) = custody::take_options(&args).unwrap();
    options
        .admit(
            config,
            &entries
                .iter()
                .map(|entry| (&entry.actor, entry.precheck.as_ref()))
                .collect::<Vec<_>>(),
            peers,
            None,
            Some(AFTER),
            Some(credibility),
        )
        .unwrap()
}

fn verify_retained(
    reader: &Reader,
    selected: GeneratedSourceProfile,
    anchor: &FileHistoryAnchor,
) {
    let canonical = fs::read(reader.store.join("delivery.bin")).unwrap();
    assert_eq!(anchor.revision(), reader.publication().revision);
    let mut recovery = FileOversight::begin_open_generated_text_stream_from_source_anchored(
        &reader.store,
        reader.profile.clone(),
        &reader.decoder,
        &reader.tokenizer,
        selected,
        anchor,
    )
    .unwrap();
    while recovery.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
        recovery.advance(recovery.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(
        recovery.progress().status,
        FileGeneratedSourceRecoveryStatus::Ready
    );
    recovery.cancel();
    assert_eq!(
        fs::read(reader.store.join("delivery.bin")).unwrap(),
        canonical,
        "verification must not append another Fence"
    );
}

#[test]
fn anchored_resident_requalification_retains_one_activation_and_every_confirmed_suffix_release() {
    let root = Root::new();
    let (Seed { config, reviewer, reader, before, fresh, .. }, prior, _) =
        retained_seed(&root);
    let epoch = fresh.expected_epoch + 1;
    let input = root.0.join("qualified-prefix.anchor");
    let retained_path = root.0.join("qualified-suffix.anchor");
    archive::export_acknowledged(&prior, &input).unwrap();
    let original_archive = fs::read(&input).unwrap();
    let capsule = root.0.join("fresh-qualification.bin");
    fs::write(&capsule, fresh.encode_reference().unwrap()).unwrap();
    let mut entries = suffix(&root, &config, &reader, false);
    let sockets = precheck_tests::select(&root, &mut entries, epoch);
    let selected = bootstrap::selection(&config, &entries[0].inputs, None).unwrap();
    let actor_profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
    let history = selection(&config, &entries, &reviewer, &input, &retained_path, &capsule);
    let mut humans = Reviewers::new(&reviewer, &reader, &entries, None);
    let mut output = OnceOutput {
        inner: precheck_tests::CheckedOutput::new(&entries, &reader, &reviewer, epoch),
        capsule: capsule.clone(),
        removed: false,
    };
    let result = run_with_history(
        config,
        entries,
        &reviewer,
        None,
        (Some(AFTER), None),
        Some(&capsule),
        Some(&history),
        || {
            humans.poll();
            ElapsedTick(1000)
        },
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(output.removed && !capsule.exists());
    assert_eq!(output.inner.documents.len(), 3);
    let replies = output.inner.join();
    assert_eq!(replies.len(), 3);
    assert!(replies.iter().all(executed));
    let packets = humans.join();
    assert_eq!(packets.len(), 3);
    assert!(packets.iter().all(Option::is_some));
    let mut sessions = BTreeSet::new();
    let mut approved = 0;
    for packet in packets.iter().flatten() {
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.action().spec().policy_epoch, epoch);
        approved += packet.action().spec().units;
    }
    let stream = reader.stream();
    assert_eq!(stream.publication.control.ledger.epoch, epoch);
    assert_eq!(stream.publication.executions, 5);
    assert_eq!(stream.publication.payload, b"AAAA");
    assert_eq!(
        stream.publication.control.ledger.charged,
        before.publication.control.ledger.charged + approved
    );
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.confirmed.finished());
    let progress = reader.progress(10);
    assert_eq!(progress.numerical.numerical.position, 16);
    assert_eq!(progress.numerical.numerical.sampled_draws, 8);
    reader.no_generation(11);
    let retained = archive::load(&retained_path).unwrap();
    assert!(retained.revision() > prior.revision());
    assert_eq!(retained.revision(), stream.publication.revision);
    verify_retained(&reader, selected, &retained);
    assert_eq!(fs::read(&input).unwrap(), original_archive);
    assert!(sockets.iter().all(|socket| socket.as_ref().is_none_or(|path| !path.exists())));
    closed(&actor_profiles, &reviewer);
}

#[test]
fn anchored_resident_requalification_rejects_rollback_before_capsule_read_or_recovery_fence() {
    let root = Root::new();
    let (Seed { config, reviewer, reader, .. }, prior, predecessor) =
        retained_seed(&root);
    let input = root.0.join("required-qualified-prefix.anchor");
    let retained_path = root.0.join("must-not-be-retained.anchor");
    archive::export_acknowledged(&prior, &input).unwrap();
    let original_archive = fs::read(&input).unwrap();
    let entries = suffix(&root, &config, &reader, false);
    let actor_profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
    let missing = root.0.join("must-not-read-qualification.bin");
    let history = selection(&config, &entries, &reviewer, &input, &retained_path, &missing);
    assert!(!missing.exists());
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let canonical = config.store.join("delivery.bin");
    let pending = config.store.join("delivery.pending");
    fs::write(&canonical, &predecessor).unwrap();
    fs::write(&pending, b"retain qualified rollback evidence").unwrap();
    let before = reader.progress(8);
    assert!(prior.revision() > before.publication.revision);
    let mut clocks = 0;
    let mut document = Vec::new();
    let error = run_with_history(
        config,
        entries,
        &reviewer,
        None,
        (Some(AFTER), None),
        Some(&missing),
        Some(&history),
        || {
            clocks += 1;
            ElapsedTick(1000)
        },
        &mut document,
    )
    .unwrap_err();
    assert!(error.contains("Stale"), "{error}");
    assert_eq!(clocks, 2, "invalid history must refuse before replay checkpoints");
    assert!(document.is_empty());
    assert!(!retained_path.exists());
    assert!(!missing.exists());
    assert_eq!(fs::read(&canonical).unwrap(), predecessor);
    assert_eq!(fs::read(&pending).unwrap(), b"retain qualified rollback evidence");
    assert_eq!(fs::read(&input).unwrap(), original_archive);
    assert_eq!(reader.publication(), before.publication);
    preserved(&reader, &before);
    closed(&actor_profiles, &reviewer);
}
