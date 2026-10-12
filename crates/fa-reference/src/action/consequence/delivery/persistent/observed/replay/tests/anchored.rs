//! Original canonical images and independently captured history, not fake receipts.
use super::*;
use super::super::{FileReviewReplayCursor, FileReviewReplayStatus};

fn finish(mut cursor: FileReviewReplayCursor, quantum: usize) -> FileReviewReplay {
    while cursor.progress().status == FileReviewReplayStatus::Replaying {
        let previous = cursor.progress();
        let progress = cursor.advance(previous.replayed_events, quantum).unwrap();
        assert!(progress.attempted_events - previous.attempted_events <= quantum);
    }
    cursor.finish().unwrap()
}

#[test]
fn anchored_review_and_genuine_successors_use_one_frozen_image_without_a_writer() {
    for quantum in [1, 3, 4096] {
        let root = Directory::new();
        let (mut host, _, inputs, _) = prepared(&root);
        host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
        let anchor = host.history_anchor().unwrap();
        let original = host.review_replay(101).unwrap();
        let cursor = FileOversight::begin_read_review_replay_anchored(
            root.store(), &profile(), 101, &anchor).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let current = host.inspect();
        let pending = root.store().join("delivery.pending");
        std::fs::write(&pending, b"unacknowledged staging").unwrap();
        assert_eq!(finish(cursor, quantum), original);
        let successor = host.review_replay(101).unwrap();
        let cursor = FileOversight::begin_read_review_replay_anchored(
            root.store(), &profile(), 101, &anchor).unwrap();
        drop(host);
        assert_eq!(finish(cursor, quantum), successor);
        assert_eq!(std::fs::read(pending).unwrap(), b"unacknowledged staging");
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), current);
    }
}

#[test]
fn an_anchor_before_finish_cannot_claim_the_selected_completed_verdict() {
    let root = Directory::new();
    let (mut host, _, inputs, _) = prepared(&root);
    let before_finish = host.history_anchor().unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    assert_eq!(FileOversight::begin_read_review_replay_anchored(
        root.store(), &profile(), 101, &before_finish).unwrap_err(),
        JournalError::Contract(Error::Incomplete));
    let full = host.history_anchor().unwrap();
    assert_eq!(finish(FileOversight::begin_read_review_replay_anchored(
        root.store(), &profile(), 101, &full).unwrap(), 1), host.review_replay(101).unwrap());
}

#[test]
fn equal_counter_valid_fork_is_not_the_independently_retained_review() {
    let root = Directory::new();
    let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let anchor = host.history_anchor().unwrap();
    let mut events = host.events.clone();
    let Event::Finish(_, _, supplied) = events.last_mut().unwrap() else { panic!("finish"); };
    supplied.values.insert(8, b"different unreviewed metadata".to_vec());
    // Both histories are lawful and have identical counters. Only the exact
    // independently retained bytes can distinguish which one was recorded.
    let fork = Machine::replay(&profile(), &events).unwrap().snapshot(events.len());
    assert_eq!(fork.control.sequence, host.inspect().control.sequence);
    let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    let original = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();
    drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &bytes).unwrap();
    assert!(FileOversight::read_review_replay(root.store(), &profile(), 101).unwrap().application().is_ok());
    assert_eq!(FileOversight::begin_read_review_replay_anchored(
        root.store(), &profile(), 101, &anchor).unwrap_err(), JournalError::Contract(Error::Binding));
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
    std::fs::write(root.store().join(storage::CANONICAL), original).unwrap();
    assert!(FileOversight::begin_read_review_replay_anchored(root.store(), &profile(), 101, &anchor).is_ok());
}

#[test]
fn full_review_anchor_cannot_excuse_an_invalid_unanchored_suffix() {
    let root = Directory::new();
    let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let anchor = host.history_anchor().unwrap();
    let mut events = host.events.clone(); events.push(Event::OpenReveals(999));
    let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &bytes).unwrap();
    let mut cursor = FileOversight::begin_read_review_replay_anchored(
        root.store(), &profile(), 101, &anchor).unwrap();
    cursor.advance(0, events.len() - 1).unwrap();
    assert_eq!(cursor.progress().status, FileReviewReplayStatus::Replaying);
    let error = cursor.advance(events.len() - 1, 1).unwrap_err();
    assert_eq!(cursor.advance(events.len() - 1, 1).unwrap_err(), error);
    assert_eq!(cursor.finish().unwrap_err(), error);
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
}

#[test]
fn foreign_path_and_rollback_behind_anchor_never_start_replay() {
    let root = Directory::new(); let other = Directory::new();
    let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let old = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let anchor = host.history_anchor().unwrap();
    let (mut second, _, input, _) = prepared(&other);
    second.finish_review(second.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    second.observe_time(second.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(anchor.revision(), second.revision());
    assert_eq!(FileOversight::begin_read_review_replay_anchored(
        other.store(), &profile(), 101, &anchor).unwrap_err(), JournalError::Contract(Error::Binding));
    assert!(FileOversight::begin_read_review_replay_anchored(
        other.store(), &profile(), 101, &second.history_anchor().unwrap()).is_ok());
    drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &old).unwrap();
    assert_eq!(FileOversight::begin_read_review_replay_anchored(
        root.store(), &profile(), 101, &anchor).unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), old);
}

#[test]
fn cancellation_at_each_event_cut_returns_no_authority_and_changes_no_bytes() {
    let root = Directory::new();
    let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let anchor = host.history_anchor().unwrap();
    let before = host.inspect();
    let path = root.store().join(storage::CANONICAL);
    let bytes = std::fs::read(&path).unwrap();
    for end in 0..=host.events.len() {
        let mut cursor = FileOversight::begin_read_review_replay_anchored(
            root.store(), &profile(), 101, &anchor).unwrap();
        for position in 0..end { cursor.advance(position, 1).unwrap(); }
        let progress = cursor.cancel();
        assert_eq!(progress.replayed_events, end);
        assert_eq!(progress.status, FileReviewReplayStatus::Cancelled);
        assert_eq!(host.inspect(), before);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
