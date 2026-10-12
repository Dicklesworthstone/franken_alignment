//! Original congress/reducer and canonical-file controls, not detector-quality evidence.
use super::*;
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::BaseEvent;

fn finish(mut cursor: FileReviewReplayCursor, quantum: usize) -> FileReviewReplay {
    while cursor.progress().status == FileReviewReplayStatus::Replaying {
        let before = cursor.progress();
        let after = cursor.advance(before.replayed_events, quantum).unwrap();
        assert!(after.replayed_events - before.replayed_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
        assert!(after.replayed_events > before.replayed_events);
    }
    let ready = cursor.progress();
    assert_eq!(ready.status, FileReviewReplayStatus::Ready);
    assert_eq!(ready.attempted_events, ready.total_events);
    assert_eq!(cursor.advance(ready.replayed_events, 1).unwrap(), ready);
    cursor.finish().unwrap()
}

#[test]
fn original_whole_input_application_and_complete_suffix_match_all_quanta() {
    for supplied in 0..3 {
        let root = Directory::new();
        let (mut host, _, inputs, anchor) = prepared(&root);
        let changed = EvidenceSnapshot::new(EvidenceIdentity {
            source: 7, generation: 1, scope: profile().delivery.scope,
        }, snapshot(), BTreeMap::from([("judge".into(), b"changed context".to_vec())]))
            .unwrap().inputs_for(inputs.action(), &profile().committee).unwrap();
        let current = match supplied { 0 => Some(&inputs), 1 => None, _ => Some(&changed) };
        let applied = host.finish_review(host.revision(), 101, current, snapshot()).unwrap();
        match supplied {
            0 => assert!(applied.is_ok()),
            1 => assert_eq!(applied, Err(Error::Incomplete)),
            _ => assert_eq!(applied, Err(Error::Stale)),
        }
        let completed = host.inspect();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let end = host.inspect();
        let oracle = extract(&profile(), &host.events, 101, end.clone()).unwrap();
        let bytes = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();
        for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
            for cursor in [host.begin_review_replay(101).unwrap(),
                FileOversight::begin_read_review_replay(root.store(), &profile(), 101).unwrap()] {
                let result = finish(cursor, quantum);
                assert_eq!(result, oracle);
                assert_eq!(result.application(), &applied);
                assert_eq!(result.completed_snapshot(), &completed);
                assert_eq!(result.journal_snapshot(), &end);
                result.archive().verify(&anchor).unwrap();
            }
        }
        assert_eq!(host.inspect(), end);
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
    }
}

#[test]
fn completing_selected_round_cannot_export_an_unverified_suffix() {
    let root = Directory::new(); let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let completed = host.events.len();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let mut cursor = host.begin_review_replay(101).unwrap();
    let progress = cursor.advance(0, completed).unwrap();
    assert_eq!(progress.status, FileReviewReplayStatus::Replaying);
    assert_eq!(progress.attempted_events, completed);
    assert_eq!(cursor.finish().unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert_eq!(finish(host.begin_review_replay(101).unwrap(), 1).journal_snapshot(), &host.inspect());
}

#[test]
fn audit_keeps_its_frozen_cut_while_live_owner_grows_and_staging_is_untouched() {
    let root = Directory::new(); let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let old = host.inspect();
    let live = host.begin_review_replay(101).unwrap();
    let disk = FileOversight::begin_read_review_replay(root.store(), &profile(), 101).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let current = host.inspect();
    let canonical = root.store().join(storage::CANONICAL);
    let bytes = std::fs::read(&canonical).unwrap();
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"retained failed staging").unwrap();
    for cursor in [live, disk] {
        let result = finish(cursor, 1);
        assert_eq!(result.journal_snapshot(), &old);
        assert_eq!(host.inspect(), current);
        assert_eq!(std::fs::read(&canonical).unwrap(), bytes);
        assert_eq!(std::fs::read(&pending).unwrap(), b"retained failed staging");
    }
    let newer = FileOversight::read_review_replay(root.store(), &profile(), 101).unwrap();
    assert_eq!(newer.journal_snapshot(), &current);
}

#[test]
fn cancellation_at_every_event_and_ready_never_changes_evidence_or_work() {
    let root = Directory::new(); let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let state = host.inspect();
    let canonical = root.store().join(storage::CANONICAL);
    let bytes = std::fs::read(&canonical).unwrap();
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"not a committed image").unwrap();
    for cut in 0..=host.events.len() {
        let mut cursor = host.begin_review_replay(101).unwrap();
        for _ in 0..cut { cursor.advance(cursor.progress().replayed_events, 1).unwrap(); }
        let before = cursor.progress();
        let cancelled = cursor.cancel();
        assert_eq!(cancelled.attempted_events, before.attempted_events);
        assert_eq!(cancelled.replayed_events, cut);
        assert_eq!(cancelled.status, FileReviewReplayStatus::Cancelled);
        assert_eq!(host.inspect(), state);
        assert_eq!(std::fs::read(&canonical).unwrap(), bytes);
        assert_eq!(std::fs::read(&pending).unwrap(), b"not a committed image");
    }
}

#[test]
fn invalid_poll_predecessors_and_quanta_do_not_attempt_an_event() {
    let root = Directory::new(); let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let mut cursor = host.begin_review_replay(101).unwrap();
    let before = cursor.progress();
    for (expected, quantum, error) in [(1, 1, Error::Stale), (0, 0, Error::InvalidInput),
        (0, MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(cursor.advance(expected, quantum), Err(JournalError::Contract(error)));
        assert_eq!(cursor.progress(), before);
    }
    cursor.advance(0, 1).unwrap();
    let next = cursor.progress();
    assert_eq!(cursor.advance(0, 1), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(cursor.progress(), next);
    assert_eq!(finish(cursor, 1), host.review_replay(101).unwrap());
}

#[test]
fn invalid_suffix_discards_already_captured_archive_and_retains_first_error() {
    let root = Directory::new(); let (mut host, _, inputs, _) = prepared(&root);
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let good = host.events.len();
    let mut events = host.events.clone(); events.push(Event::OpenReveals(999));
    assert!(matches!(Machine::replay(&profile(), &events), Err(Error::Missing)));
    let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    let canonical = root.store().join(storage::CANONICAL);
    std::fs::write(&canonical, &bytes).unwrap();
    let mut cursor = FileOversight::begin_read_review_replay(root.store(), &profile(), 101).unwrap();
    assert_eq!(cursor.advance(0, good).unwrap().status, FileReviewReplayStatus::Replaying);
    let error = JournalError::Contract(Error::Missing);
    assert_eq!(cursor.advance(good, 1), Err(error.clone()));
    let failed = cursor.progress();
    assert_eq!(failed.status, FileReviewReplayStatus::Failed(Error::Missing));
    assert_eq!(failed.attempted_events, good + 1);
    assert_eq!(failed.replayed_events, good);
    assert_eq!(cursor.advance(good, 1), Err(error.clone()));
    assert_eq!(cursor.progress(), failed);
    assert_eq!(cursor.finish().unwrap_err(), error);
    assert_eq!(std::fs::read(canonical).unwrap(), bytes);
}

#[test]
fn missing_and_unfinished_rounds_cannot_fabricate_a_completed_review() {
    let root = Directory::new(); let (mut host, _, inputs, _) = prepared(&root);
    for (round, error) in [(0, Error::InvalidInput), (999, Error::Missing), (101, Error::Incomplete)] {
        assert_eq!(host.begin_review_replay(round).unwrap_err(), JournalError::Contract(error));
        assert_eq!(FileOversight::begin_read_review_replay(root.store(), &profile(), round).unwrap_err(),
            JournalError::Contract(error));
    }
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    assert!(finish(host.begin_review_replay(101).unwrap(), 1).application().is_ok());
}

#[test]
fn deadline_closed_missing_votes_use_original_finish_not_an_alternate_reducer() {
    let root = Directory::new(); let (host, _, inputs, anchor) = prepared(&root);
    let begin = host.events.iter().position(|event| matches!(event, Event::Begin(_, 101, ..))).unwrap();
    let mut events = host.events[..=begin].to_vec();
    events.push(Event::Core(BaseEvent::Time(ElapsedTick(8))));
    events.push(Event::Finish(101, Some(inputs.views().clone()), snapshot()));
    let at_end = Machine::replay(&profile(), &events).unwrap().snapshot(events.len());
    let oracle = extract(&profile(), &events, 101, at_end).unwrap();
    let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    std::fs::write(root.store().join(storage::CANONICAL), &bytes).unwrap();
    let result = finish(FileOversight::begin_read_review_replay(root.store(), &profile(), 101).unwrap(), 1);
    assert_eq!(result, oracle);
    result.archive().verify(&anchor).unwrap();
    assert!(result.archive().commit_times.is_empty());
    assert!(result.archive().reveal_times.is_empty());
    assert_eq!(result.archive().reveals_opened_at, ElapsedTick(8));
    assert_eq!(result.journal_snapshot().executions, 0);
}
