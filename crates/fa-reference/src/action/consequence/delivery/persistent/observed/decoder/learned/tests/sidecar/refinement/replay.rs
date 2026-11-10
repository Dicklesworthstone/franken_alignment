//! Original learned refinement and whole-image audit, with no synthetic receipt.
//! These controls reuse the native numerical source and commit/reveal fixtures.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{
    guarded::anchored::FileHistoryAnchor,
    replay::{
        FileReviewCompletion, FileReviewCompletionAudit, FileReviewReplayCursor,
        FileReviewReplayStatus,
    },
};

fn refine(host: &mut FileOversight, handle: &mut FileLearnedSidecar) -> FileLearnedSidecarFinish {
    completed(host, 101, Some(Verdict::Abstain));
    let result = host
        .finish_learned_sidecar_review(host.revision(), handle, 101, snapshot())
        .unwrap();
    assert!(matches!(&result, FileLearnedSidecarFinish::Refined { .. }));
    result
}

fn read(
    root: &Directory,
    generation: &FileLearnedConfig,
    round: u64,
    anchor: &FileHistoryAnchor,
) -> FileReviewReplayCursor {
    FileOversight::begin_read_review_replay_anchored_with_learned_generation(
        root.store(),
        &profile(),
        generation,
        round,
        anchor,
    )
    .unwrap()
    .include_refinements()
    .unwrap()
}

fn ready(mut cursor: FileReviewReplayCursor, quantum: usize) -> FileReviewReplayCursor {
    while cursor.progress().status == FileReviewReplayStatus::Replaying {
        let before = cursor.progress();
        let after = cursor.advance(before.replayed_events, quantum).unwrap();
        assert!(after.attempted_events - before.attempted_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    let progress = cursor.progress();
    assert_eq!(progress.status, FileReviewReplayStatus::Ready);
    assert_eq!(progress.replayed_events, progress.total_events);
    cursor
}

fn audit(cursor: FileReviewReplayCursor, quantum: usize) -> FileReviewCompletionAudit {
    ready(cursor, quantum)
        .finish_completion_with_effects()
        .unwrap()
}

#[test]
fn opted_in_refinement_audit_retains_the_original_archive_without_applying_it() {
    let root = Directory::new();
    let generation = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &generation);
    let (_, mut handle) = plan(&mut host, 2);
    let coarse = host.current_learned_sidecar(&handle).unwrap().clone();
    let sequence = host.inspect().control.sequence;
    let FileLearnedSidecarFinish::Refined {
        group,
        input_revision,
        archive,
    } = refine(&mut host, &mut handle)
    else {
        unreachable!("actual original refinement");
    };
    let anchor = host.history_anchor().unwrap();
    let before = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap();
    let path = root.store().join(storage::CANONICAL);
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(host.inspect().control.sequence, sequence);
    assert_eq!(archive.inputs.as_ref(), &coarse);
    assert_eq!(input_revision, 2);
    assert_eq!(archive.input_revision, 1);

    for quantum in [1, 3, 4096] {
        let result = audit(read(&root, &generation, 101, &anchor), quantum);
        let FileReviewCompletion::Refined(refined) = result.review() else {
            panic!("a completed refinement is not an application");
        };
        assert_eq!(refined.group(), group);
        assert_eq!(refined.input_revision(), input_revision);
        assert_eq!(refined.reviewed_input_revision(), archive.input_revision);
        assert_eq!(result.review().archive(), &archive);
        assert_eq!(result.review().anchor().policy.round, 101);
        assert_eq!(result.review().anchor().policy.attempt, 1);
        assert_eq!(result.review().anchor().input_revision, 1);
        result
            .review()
            .archive()
            .verify(result.review().anchor())
            .unwrap();
        assert_eq!(result.review().completed_snapshot(), &before);
        assert_eq!(result.review().journal_snapshot(), &before);
        assert!(!result.effect().selected_review_dispatched());
        assert!(result.effect().dispatch().is_none());
        assert!(result.effect().outcome().is_none());
        assert!(result.effect().reconciliation().is_none());
        assert_eq!(host.inspect(), before);
        assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let rich = host.current_learned_sidecar(&handle).unwrap().clone();
    assert!(host.authorize(host.revision(), 1, &rich, snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn refined_round_stays_distinct_from_the_later_applied_round_and_actual_effect() {
    let root = Directory::new();
    let generation = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, reviewer) = owner(&root, &generation);
    let (action, mut handle) = plan(&mut host, 2);
    let original = refine(&mut host, &mut handle);
    let anchor = host.history_anchor().unwrap();
    let refined_cut = host.inspect();
    let frozen = read(&root, &generation, 101, &anchor);
    let rich = host.current_learned_sidecar(&handle).unwrap().clone();
    completed(&mut host, 102, Some(Verdict::Allow));
    let FileLearnedSidecarFinish::Applied {
        receipt: Ok(receipt),
        archive: applied_archive,
        ..
    } = host
        .finish_learned_sidecar_review(host.revision(), &mut handle, 102, snapshot())
        .unwrap()
    else {
        panic!("fresh independent Allow must actually apply");
    };
    let automatic = host
        .authorize(host.revision(), 1, &rich, snapshot())
        .unwrap();
    let request = host
        .request_human_approval(host.revision(), 1001, 1, &rich, ElapsedTick(10))
        .unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &rich, snapshot())
        .unwrap();
    let publication = host
        .publish_checked(host.revision(), 1, Some(&rich), snapshot(), ElapsedTick(2))
        .unwrap();
    assert!(matches!(publication.outcome, EndpointOutcome::Executed { .. }));
    host.reconcile(host.revision(), 1).unwrap();
    let before = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap();
    let path = root.store().join(storage::CANONICAL);
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(before.executions, 1);
    assert_eq!(before.control.ledger.charged, 16);

    // Opening before the live successor fixes the image, not the mutable owner.
    let first = audit(frozen, 1);
    assert_eq!(first.review().journal_snapshot(), &refined_cut);
    assert!(first.effect().dispatch().is_none());
    let current = audit(read(&root, &generation, 101, &anchor), 3);
    assert!(matches!(current.review(), FileReviewCompletion::Refined(_)));
    assert_eq!(current.review().archive(), original.archive());
    assert_eq!(current.review().completed_snapshot(), &refined_cut);
    assert_eq!(current.review().journal_snapshot(), &before);
    assert!(!current.effect().selected_review_dispatched());
    let dispatch = current.effect().dispatch().unwrap();
    assert_eq!(dispatch.authorization().round(), 102);
    assert_eq!(current.effect().outcome().unwrap().outcome(), publication.outcome);
    assert!(current.effect().reconciliation().is_some());

    let applied_anchor = host.history_anchor().unwrap();
    let applied = audit(read(&root, &generation, 102, &applied_anchor), 1);
    let FileReviewCompletion::Applied(review) = applied.review() else {
        panic!("an applied result remains applied");
    };
    assert_eq!(review.archive(), &applied_archive);
    assert_eq!(review.application(), &Ok(receipt));
    assert_eq!(review, &host.review_replay(102).unwrap());
    assert!(applied.effect().selected_review_dispatched());
    assert_eq!(host.inspect(), before);
    assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn legacy_readers_still_refuse_refinement_and_opt_in_cannot_follow_replay() {
    let root = Directory::new();
    let generation = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &generation);
    let (_, mut handle) = plan(&mut host, 2);
    refine(&mut host, &mut handle);
    let anchor = host.history_anchor().unwrap();
    let before = host.inspect();
    let bytes = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();

    assert_eq!(
        host.review_replay(101).unwrap_err(),
        JournalError::Contract(Error::Incomplete)
    );
    let mut legacy = host.begin_review_replay(101).unwrap();
    let total = legacy.progress().total_events;
    assert_eq!(
        legacy.advance(0, total).unwrap_err(),
        JournalError::Contract(Error::Incomplete)
    );
    assert_eq!(
        legacy.finish_completion().unwrap_err(),
        JournalError::Contract(Error::Incomplete)
    );
    let mut late = host.begin_review_replay(101).unwrap();
    late.advance(0, 1).unwrap();
    assert_eq!(
        late.include_refinements().unwrap_err(),
        JournalError::Contract(Error::WrongState)
    );
    let mut late_automatic = host.begin_review_replay(101).unwrap();
    late_automatic.advance(0, 1).unwrap();
    assert_eq!(
        late_automatic.complete_completion().unwrap_err(),
        JournalError::Contract(Error::WrongState)
    );
    assert_eq!(
        read(&root, &generation, 101, &anchor)
            .finish_completion()
            .unwrap_err(),
        JournalError::Contract(Error::Incomplete)
    );
    // Explicitly opting in does not turn either applied-only output into a
    // fabricated refusal or receipt, even after every event has verified.
    assert_eq!(
        ready(read(&root, &generation, 101, &anchor), 1)
            .finish()
            .unwrap_err(),
        JournalError::Contract(Error::Incomplete)
    );
    assert_eq!(
        ready(read(&root, &generation, 101, &anchor), 1)
            .finish_with_effects()
            .unwrap_err(),
        JournalError::Contract(Error::Incomplete)
    );
    assert!(matches!(
        host.begin_review_replay(101)
            .unwrap()
            .complete_completion()
            .unwrap(),
        FileReviewCompletion::Refined(_)
    ));
    assert_eq!(host.inspect(), before);
    assert_eq!(
        std::fs::read(root.store().join(storage::CANONICAL)).unwrap(),
        bytes
    );
}

#[test]
fn refined_audit_requires_the_exact_recipe_and_an_anchor_covering_completion() {
    let root = Directory::new();
    let generation = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &generation);
    let (_, mut handle) = plan(&mut host, 2);
    completed(&mut host, 101, Some(Verdict::Abstain));
    let before_finish = host.history_anchor().unwrap();
    let path = root.store().join(storage::CANONICAL);
    let older = std::fs::read(&path).unwrap();
    host.finish_learned_sidecar_review(host.revision(), &mut handle, 101, snapshot())
        .unwrap();
    let anchor = host.history_anchor().unwrap();
    let bytes = std::fs::read(&path).unwrap();

    assert_eq!(
        FileOversight::begin_read_review_replay_anchored_with_learned_generation(
            root.store(), &profile(), &generation, 101, &before_finish
        )
        .unwrap_err(),
        JournalError::Contract(Error::Incomplete)
    );
    let wrong = config(false, 3).with_required_sidecar().unwrap();
    assert_eq!(
        FileOversight::begin_read_review_replay_anchored_with_learned_generation(
            root.store(), &profile(), &wrong, 101, &anchor
        )
        .unwrap_err(),
        JournalError::Contract(Error::Binding)
    );
    assert!(matches!(
        audit(read(&root, &generation, 101, &anchor), 1).review(),
        FileReviewCompletion::Refined(_)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    let other = Directory::new();
    let (mut second, _) = owner(&other, &generation);
    let (_, mut other_handle) = plan(&mut second, 2);
    refine(&mut second, &mut other_handle);
    assert_eq!(second.revision(), host.revision());
    assert_eq!(
        FileOversight::begin_read_review_replay_anchored_with_learned_generation(
            other.store(), &profile(), &generation, 101, &anchor
        )
        .unwrap_err(),
        JournalError::Contract(Error::Binding)
    );
    let own_anchor = second.history_anchor().unwrap();
    assert!(matches!(
        audit(read(&other, &generation, 101, &own_anchor), 1).review(),
        FileReviewCompletion::Refined(_)
    ));

    drop(host);
    std::fs::write(&path, &older).unwrap();
    assert_eq!(
        FileOversight::begin_read_review_replay_anchored_with_learned_generation(
            root.store(), &profile(), &generation, 101, &anchor
        )
        .unwrap_err(),
        JournalError::Contract(Error::Stale)
    );
    assert_eq!(std::fs::read(&path).unwrap(), older);
}

#[test]
fn a_valid_refinement_anchor_never_excuses_an_unverified_or_invalid_suffix() {
    let root = Directory::new();
    let generation = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &generation);
    let (_, mut handle) = plan(&mut host, 2);
    refine(&mut host, &mut handle);
    let anchor = host.history_anchor().unwrap();
    assert!(matches!(
        audit(read(&root, &generation, 101, &anchor), 1).review(),
        FileReviewCompletion::Refined(_)
    ));
    let mut events = host.events.clone();
    events.push(Event::OpenReveals(999));
    let bytes = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    let path = root.store().join(storage::CANONICAL);
    drop(host);
    std::fs::write(&path, &bytes).unwrap();
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged audit staging").unwrap();

    let mut partial = read(&root, &generation, 101, &anchor);
    partial.advance(0, events.len() - 1).unwrap();
    assert_eq!(partial.progress().status, FileReviewReplayStatus::Replaying);
    assert_eq!(
        partial.finish_completion().unwrap_err(),
        JournalError::Contract(Error::Incomplete)
    );
    let mut cursor = read(&root, &generation, 101, &anchor);
    cursor.advance(0, events.len() - 1).unwrap();
    let failure = cursor.advance(events.len() - 1, 1).unwrap_err();
    assert_eq!(failure, JournalError::Contract(Error::Missing));
    assert_eq!(cursor.advance(events.len() - 1, 1).unwrap_err(), failure);
    assert_eq!(cursor.finish_completion_with_effects().unwrap_err(), failure);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged audit staging");
}

#[test]
fn a_well_framed_corrupt_refinement_witness_cannot_produce_a_completion_report() {
    let root = Directory::new();
    let generation = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &generation);
    let (_, mut handle) = plan(&mut host, 2);
    refine(&mut host, &mut handle);
    let anchor = host.history_anchor().unwrap();
    assert!(matches!(
        audit(read(&root, &generation, 101, &anchor), 1).review(),
        FileReviewCompletion::Refined(_)
    ));
    let mut events = host.events.clone();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(SidecarEvent::Finish {
        expected,
        ..
    }))) = events.last_mut().unwrap()
    else {
        panic!("original completed refinement event");
    };
    let mut witness = expected.to_vec();
    let end = witness.len() - 1;
    witness[end] ^= 1;
    *expected = witness.into();
    let bytes = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    let path = root.store().join(storage::CANONICAL);
    drop(host);
    std::fs::write(&path, &bytes).unwrap();

    assert_eq!(
        FileOversight::begin_read_review_replay_anchored_with_learned_generation(
            root.store(), &profile(), &generation, 101, &anchor
        )
        .unwrap_err(),
        JournalError::Contract(Error::Binding)
    );
    // Even deliberately selecting the unanchored historical API cannot treat
    // valid framing as numerical or completed-review proof.
    let mut cursor = FileOversight::begin_read_review_replay_with_learned_generation(
        root.store(), &profile(), &generation, 101
    )
    .unwrap()
    .include_refinements()
    .unwrap();
    assert_eq!(
        cursor.advance(0, events.len()).unwrap_err(),
        JournalError::Contract(Error::Binding)
    );
    assert_eq!(
        cursor.finish_completion().unwrap_err(),
        JournalError::Contract(Error::Binding)
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn cancelling_refinement_replay_at_every_cut_never_changes_live_work_or_storage() {
    let root = Directory::new();
    let generation = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &generation);
    let (_, mut handle) = plan(&mut host, 2);
    refine(&mut host, &mut handle);
    let anchor = host.history_anchor().unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let before = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap();
    let path = root.store().join(storage::CANONICAL);
    let bytes = std::fs::read(&path).unwrap();
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"retain unrelated staging").unwrap();

    for cut in 0..=host.events.len() {
        let mut cursor = read(&root, &generation, 101, &anchor);
        for position in 0..cut {
            cursor.advance(position, 1).unwrap();
        }
        let progress = cursor.cancel();
        assert_eq!(progress.status, FileReviewReplayStatus::Cancelled);
        assert_eq!(progress.replayed_events, cut);
        assert_eq!(progress.attempted_events, cut);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::read(&pending).unwrap(), b"retain unrelated staging");
    }
    assert!(matches!(
        audit(read(&root, &generation, 101, &anchor), 3).review(),
        FileReviewCompletion::Refined(_)
    ));
    assert_eq!(host.inspect(), before);
}

#[test]
fn replacement_faults_preserve_acknowledged_vs_visible_refinement_history() {
    for barrier in [
        JournalIo::Stage,
        JournalIo::Write,
        JournalIo::FileSync,
        JournalIo::Rename,
        JournalIo::DirectorySync,
    ] {
        let root = Directory::new();
        let generation = config(false, 1).with_required_sidecar().unwrap();
        let (mut host, _) = owner(&root, &generation);
        let (_, mut handle) = plan(&mut host, 2);
        completed(&mut host, 101, Some(Verdict::Abstain));
        let before = host.inspect();
        host.store.fail_once(barrier);
        let error = host
            .finish_learned_sidecar_review(host.revision(), &mut handle, 101, snapshot())
            .unwrap_err();
        assert!(matches!(error, JournalError::Io(ref failure) if failure.operation == barrier));
        assert_eq!(host.inspect(), before);
        assert_eq!(handle.input_revision(), 1);
        assert_eq!(host.begin_review_replay(101).unwrap_err(), JournalError::Unavailable);
        let path = root.store().join(storage::CANONICAL);
        let bytes = std::fs::read(&path).unwrap();
        let pending = root.store().join("delivery.pending");
        let staged = std::fs::read(&pending).ok();
        let disk = FileOversight::begin_read_review_replay_with_learned_generation(
            root.store(), &profile(), &generation, 101
        );
        if barrier == JournalIo::DirectorySync {
            // A visible replacement is complete historical data; the poisoned
            // live owner did not acknowledge it or acquire the refined handle.
            let report = audit(disk.unwrap().include_refinements().unwrap(), 1);
            let FileReviewCompletion::Refined(refined) = report.review() else {
                panic!("visible original refinement");
            };
            assert_eq!(refined.input_revision(), 2);
            assert_eq!(report.review().completed_snapshot().revision, before.revision + 1);
            assert_eq!(report.review().archive().policy.abstained, ["reviewer".to_owned()]);
            assert!(!report.effect().selected_review_dispatched());
            assert!(report.effect().dispatch().is_none());
        } else {
            assert_eq!(disk.unwrap_err(), JournalError::Contract(Error::Incomplete));
        }
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::read(&pending).ok(), staged);
        assert_eq!(host.inspect(), before);
    }
}
