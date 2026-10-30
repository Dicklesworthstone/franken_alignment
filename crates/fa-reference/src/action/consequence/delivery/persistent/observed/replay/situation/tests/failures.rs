use super::*;

#[test]
fn unacknowledged_visible_replacement_is_not_reported_as_acknowledged_owner_state() {
    let root = Root::new(); let (mut host, _) = create(&root);
    let acknowledged = host.revision(); host.store.fail_once(JournalIo::DirectorySync);
    assert!(host.observe_time(acknowledged, ElapsedTick(2)).is_err());
    assert!(matches!(host.situation(), Err(JournalError::Unavailable)));
    let files = root.files(); let report = FileOversight::read_situation(root.store(), &profile(), None).unwrap();
    let (_, basis) = page(&report, None, 1);
    assert_eq!(basis.origin, FileSituationOrigin::CanonicalImage);
    assert_eq!(basis.recorded_at, Some(ElapsedTick(2))); assert_eq!(basis.journal_revision, acknowledged + 1);
    assert_eq!(basis.anchored_through, None); assert_eq!(root.files(), files);
}

#[test]
fn empty_histories_have_a_complete_empty_page_and_bad_limits_never_change_it() {
    let root = Root::new(); let (host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let report = FileOversight::read_situation(root.store(), &profile(), None).unwrap();
    assert!(matches!(report.page(None, 0), Err(JournalError::Contract(Error::InvalidInput))));
    assert!(matches!(report.page(None, MAX_SITUATION_PAGE_ATTEMPTS + 1), Err(JournalError::Contract(Error::Limit))));
    let (empty, basis) = page(&report, None, MAX_SITUATION_PAGE_ATTEMPTS);
    assert!(empty.attempts.is_empty() && empty.next.is_none());
    assert_eq!(empty.summary.available_units, 100); assert_eq!(empty.summary.attempts, 0);
    assert_eq!(basis.recorded_at, None); assert_eq!(basis.journal_revision, 0); drop(host);
}

#[test]
fn missing_wrong_profile_and_malformed_images_never_become_empty_reports() {
    let root = Root::new();
    assert!(FileOversight::read_situation(root.store(), &profile(), None).is_err());
    assert!(!root.store().exists());
    let (mut host, _) = create(&root); propose(&mut host, 1, true);
    let mut wrong = profile(); wrong.delivery.total += 1;
    assert!(FileOversight::read_situation(root.store(), &wrong, None).is_err());
    let path = host.store.identity().join(storage::CANONICAL); let bytes = fs::read(&path).unwrap();
    for invalid in [bytes[..bytes.len() - 1].to_vec(), [bytes, vec![0]].concat()] {
        fs::write(&path, &invalid).unwrap();
        assert!(FileOversight::read_situation(root.store(), &profile(), None).is_err());
        assert_eq!(fs::read(&path).unwrap(), invalid);
    }
}
