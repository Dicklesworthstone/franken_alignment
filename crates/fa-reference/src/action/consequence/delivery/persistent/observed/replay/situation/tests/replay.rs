use super::*;

#[test]
fn replay_yields_at_each_event_and_never_exports_an_unchecked_prefix() {
    let root = Root::new(); let (mut host, _) = create(&root);
    propose(&mut host, 1, true); propose(&mut host, 2, false);
    let files = root.files(); let expected = page(&host.situation().unwrap(), None, 64).0;
    for cut in 0..host.events.len() {
        let mut replay = FileOversight::begin_read_situation(root.store(), &profile(), None).unwrap();
        for _ in 0..cut { replay.advance(1).unwrap(); }
        assert_eq!(replay.progress().checked_events, cut);
        assert!(matches!(replay.finish(), Err(JournalError::Contract(Error::Incomplete))));
    }
    let mut replay = FileOversight::begin_read_situation(root.store(), &profile(), None).unwrap();
    let before = replay.progress(); assert_eq!(replay.advance(0).unwrap(), before);
    assert_eq!(replay.advance(MAX_SITUATION_REPLAY_EVENTS + 1), Err(Error::Limit.into()));
    while replay.progress().status == FileSituationReplayStatus::Reading {
        let before = replay.progress().checked_events; let after = replay.advance(1).unwrap();
        assert_eq!(after.checked_events, before + 1);
    }
    let ready = replay.progress(); assert_eq!(replay.advance(1).unwrap(), ready);
    let (actual, _) = page(&replay.finish().unwrap(), None, 64);
    assert_eq!(actual.summary, expected.summary); assert_eq!(actual.attempts, expected.attempts);
    assert_eq!(root.files(), files);
}

#[test]
fn immutable_capture_and_branded_pages_prevent_mixed_live_revisions_or_equal_number_forks() {
    let root = Root::new(); let (mut host, _) = create(&root);
    propose(&mut host, 2, true); propose(&mut host, 4, true);
    let old = host.situation().unwrap(); let equal_revision = host.situation().unwrap();
    let (first, basis) = page(&old, None, 1); let token = first.next.unwrap();
    assert!(matches!(equal_revision.page(Some(&token), 1), Err(JournalError::Contract(Error::Binding))));
    let replay = FileOversight::begin_read_situation(root.store(), &profile(), None).unwrap();
    propose(&mut host, 3, true);
    let newer = host.situation().unwrap(); assert!(newer.page(Some(&token), 1).is_err());
    let (last, again) = page(&old, Some(&token), 1); assert_eq!(again, basis);
    assert_eq!(last.attempts[0].attempt, 4); assert!(last.next.is_none());
    let (retry, _) = page(&old, Some(&token), 1); assert_eq!(retry.attempts, last.attempts);
    let (frozen, frozen_basis) = page(&replay.complete().unwrap(), None, 64);
    assert_eq!(frozen.summary.attempts, 2); assert_eq!(frozen_basis.journal_revision, basis.journal_revision);
    assert_eq!(page(&newer, None, 64).0.summary.attempts, 3);
}

#[test]
fn semantic_corruption_after_a_valid_prefix_permanently_disqualifies_replay() {
    let root = Root::new(); let (mut host, _) = create(&root); propose(&mut host, 1, true);
    let mut events = host.events.clone();
    events.push(Event::Core(BaseEvent::Publish(999)));
    let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    fs::write(host.store.identity().join(storage::CANONICAL), bytes).unwrap();
    let files = root.files();
    let mut replay = FileOversight::begin_read_situation(root.store(), &profile(), None).unwrap();
    for _ in 0..host.events.len() { replay.advance(1).unwrap(); }
    assert!(replay.advance(1).is_err());
    assert_eq!(replay.progress().status, FileSituationReplayStatus::Failed);
    assert_eq!(replay.advance(1), Err(JournalError::Unavailable));
    assert!(matches!(replay.finish(), Err(JournalError::Unavailable)));
    assert!(FileOversight::read_situation(root.store(), &profile(), None).is_err());
    assert_eq!(root.files(), files);
}

#[test]
fn exact_anchors_accept_successors_but_reject_equal_counter_forks_and_rollback() {
    let root = Root::new(); let (mut host, _) = create(&root);
    let anchor = host.history_anchor().unwrap(); let anchored = anchor.revision();
    propose(&mut host, 1, true);
    let original = host.store.read(profile().delivery.limits.bytes).unwrap();
    let report = FileOversight::read_situation(root.store(), &profile(), Some(&anchor)).unwrap();
    let (_, basis) = page(&report, None, 64);
    assert_eq!(basis.anchored_through, Some(anchored)); assert!(basis.journal_revision > anchored);
    let mut fork = host.events.clone();
    fork[0] = Event::Core(BaseEvent::Time(ElapsedTick(2)));
    fs::write(host.store.identity().join(storage::CANONICAL), journal::encode(&profile(), host.store.identity(), &fork).unwrap()).unwrap();
    assert!(FileOversight::read_situation(root.store(), &profile(), None).is_ok());
    assert!(matches!(FileOversight::begin_read_situation(root.store(), &profile(), Some(&anchor)),
        Err(JournalError::Contract(Error::Binding))));
    let empty = journal::encode(&profile(), host.store.identity(), &[]).unwrap();
    fs::write(host.store.identity().join(storage::CANONICAL), empty).unwrap();
    assert!(matches!(FileOversight::begin_read_situation(root.store(), &profile(), Some(&anchor)),
        Err(JournalError::Contract(Error::Stale))));
    fs::write(host.store.identity().join(storage::CANONICAL), original).unwrap();
}

