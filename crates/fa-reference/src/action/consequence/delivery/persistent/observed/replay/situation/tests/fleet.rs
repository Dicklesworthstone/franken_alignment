//! Real canonical histories; the fleet must not invent a global ledger or cut.
use super::*;
use super::super::fleet::*;

fn reader(root: &Root) -> FileSituationReplay {
    FileOversight::begin_read_situation(root.store(), &profile(), None).unwrap()
}
fn fleet(inputs: Vec<(u64, Option<FileSituationReplay>)>) -> FileSituationFleetReplay {
    FileSituationFleetReplay::new(inputs, FileSituationFleetLimits::default()).unwrap()
}
fn known(page: &FileSituationFleetPage) -> (&FileSituationPage, FileSituationBasis) {
    match &page.observation {
        Knowledge::Known { value, basis } => (value, *basis),
        _ => panic!("verified member"),
    }
}

#[test]
fn one_total_event_budget_round_robins_instead_of_starving_a_short_member() {
    let slow = Root::new(); let (mut host, _) = create(&slow);
    for tick in 2..=9 { host.observe_time(host.revision(), ElapsedTick(tick)).unwrap(); }
    let short = Root::new(); let (other, _) = create(&short);
    let before = (slow.files(), short.files(), host.inspect(), other.inspect());
    let mut replay = fleet(vec![(20, Some(reader(&short))), (10, Some(reader(&slow)))]);
    let initial = replay.progress();
    assert_eq!(replay.advance(0).unwrap(), initial);
    assert_eq!(replay.advance(MAX_SITUATION_REPLAY_EVENTS + 1), Err(Error::Limit.into()));
    assert_eq!(replay.progress(), initial);
    assert_eq!(replay.advance(1).unwrap().known_members, 0);
    let second = replay.advance(1).unwrap();
    assert_eq!(second.checked_events, 2); assert_eq!(second.attempted_events, 2);
    assert_eq!(second.known_members, 1); assert_eq!(second.pending_members, 1);
    let report = replay.complete().unwrap();
    assert!(report.coverage().all_members_known());
    assert_eq!(before, (slow.files(), short.files(), host.inspect(), other.inspect()));
}

#[test]
fn missing_and_semantically_corrupt_members_stay_unknown_without_losing_good_members() {
    let bad = Root::new(); let (mut broken, _) = create(&bad); propose(&mut broken, 1, true);
    let mut events = broken.events.clone(); events.push(Event::Core(BaseEvent::Publish(999)));
    fs::write(broken.store.identity().join(storage::CANONICAL),
        journal::encode(&profile(), broken.store.identity(), &events).unwrap()).unwrap();
    let good = Root::new(); let (mut host, _) = create(&good); propose(&mut host, 7, true);
    let before = (bad.files(), good.files(), host.inspect());
    let mut replay = fleet(vec![(30, Some(reader(&good))), (20, Some(reader(&bad))), (10, None)]);
    while replay.progress().status == FileSituationFleetReplayStatus::Reading { replay.advance(1).unwrap(); }
    let work = replay.progress();
    assert_eq!(work.known_members, 1); assert_eq!(work.unknown_members, 2);
    assert_eq!(work.attempted_events, work.checked_events + 1);
    assert_eq!(replay.advance(64).unwrap(), work);
    let report = replay.finish().unwrap();
    assert_eq!(report.coverage(), FileSituationFleetCoverage {
        declared_members: 3, known_members: 1, unknown_members: 2 });
    let first = report.page(None, 1).unwrap();
    assert_eq!(first.member, 10); assert_eq!(first.failure, Some(FileSituationReadFailure::InputUnavailable));
    assert!(matches!(first.observation, Knowledge::Unknown { .. }));
    let second = report.page(first.next.as_ref(), 1).unwrap();
    assert_eq!(second.member, 20); assert_eq!(second.failure, Some(FileSituationReadFailure::ReplayRejected));
    assert!(matches!(second.observation, Knowledge::Unknown { .. }));
    let last = report.page(second.next.as_ref(), 1).unwrap();
    assert_eq!(last.member, 30); assert!(last.next.is_none());
    assert!(!last.coverage.all_members_known());
    assert_eq!(known(&last).0.attempts[0].attempt, 7);
    assert_eq!(known(&last).0.summary.total_units, 100);
    assert_eq!(before, (bad.files(), good.files(), host.inspect()));
}

#[test]
fn lifecycle_triage_and_nested_pagination_keep_each_original_basis_and_total() {
    let pending = Root::new(); let (mut p, _) = create(&pending);
    propose(&mut p, 2, true); propose(&mut p, 4, true);
    let unresolved = Root::new(); let (mut u, human) = create(&unresolved);
    dispatch(&mut u, &human, 9);
    let empty = Root::new(); let (e, _) = FileOversight::create(empty.store(), profile()).unwrap();
    let before = (pending.files(), unresolved.files(), empty.files());
    let report = fleet(vec![(1, Some(reader(&empty))), (2, Some(reader(&pending))),
        (3, Some(reader(&unresolved)))]).complete().unwrap();
    let a = report.page(None, 1).unwrap();
    assert_eq!(a.member, 3); assert_eq!(known(&a).0.attempts[0].stage, ActionState::Dispatching);
    assert_eq!(known(&a).1.journal_revision, u.revision());
    let b = report.page(a.next.as_ref(), 1).unwrap();
    assert_eq!(b.member, 2); assert_eq!(known(&b).0.attempts[0].attempt, 2);
    assert_eq!(known(&b).0.summary.attempts, 2); assert_eq!(known(&b).0.offset, 0);
    let c = report.page(b.next.as_ref(), 1).unwrap();
    assert_eq!(c.member, 2); assert_eq!(known(&c).0.attempts[0].attempt, 4);
    assert_eq!(known(&c).1, known(&b).1); assert_eq!(known(&c).0.summary, known(&b).0.summary);
    assert_eq!(known(&c).0.offset, 1); assert!(known(&c).0.next.is_none()); assert!(c.next.is_some());
    let d = report.page(c.next.as_ref(), 1).unwrap();
    assert_eq!(d.member, 1); assert!(known(&d).0.attempts.is_empty()); assert!(d.next.is_none());
    assert_eq!(known(&d).1.journal_revision, e.revision());
    assert_eq!(before, (pending.files(), unresolved.files(), empty.files()));
}

#[test]
fn captures_and_continuations_do_not_follow_later_live_writes_or_accept_foreign_tokens() {
    let root = Root::new(); let (mut host, _) = create(&root);
    propose(&mut host, 2, true); propose(&mut host, 4, true);
    let selected = fleet(vec![(1, Some(reader(&root)))]);
    let other = fleet(vec![(1, Some(reader(&root)))]).complete().unwrap();
    propose(&mut host, 6, true);
    let report = selected.complete().unwrap();
    let first = report.page(None, 1).unwrap(); let token = first.next.as_ref().unwrap();
    assert!(matches!(other.page(Some(token), 1), Err(JournalError::Contract(Error::Binding))));
    let last = report.page(Some(token), 1).unwrap(); let repeated = report.page(Some(token), 1).unwrap();
    assert_eq!(known(&last).0.attempts, known(&repeated).0.attempts);
    assert_eq!(known(&last).0.summary.attempts, 2); assert_eq!(known(&last).1, known(&first).1);
    assert!(known(&last).1.journal_revision < host.revision()); assert!(last.next.is_none());
    assert!(matches!(report.page(None, 0), Err(JournalError::Contract(Error::InvalidInput))));
    assert!(matches!(report.page(None, 65), Err(JournalError::Contract(Error::Limit))));
}

#[test]
fn every_incomplete_fleet_boundary_refuses_finish_without_mutation() {
    let a = Root::new(); let (mut first, _) = create(&a); propose(&mut first, 1, true);
    let b = Root::new(); let (second, _) = create(&b);
    let events = first.events.len() + second.events.len();
    let before = (a.files(), b.files());
    for cut in 0..events {
        let mut replay = fleet(vec![(1, Some(reader(&a))), (2, Some(reader(&b)))]);
        for _ in 0..cut { replay.advance(1).unwrap(); }
        assert_eq!(replay.progress().attempted_events, cut);
        assert!(matches!(replay.finish(), Err(JournalError::Contract(Error::Incomplete))));
    }
    assert_eq!(before, (a.files(), b.files()));
}

#[test]
fn admission_rejects_duplicate_zero_overlimit_and_preconsumed_inputs_before_replay() {
    let limits = FileSituationFleetLimits::default();
    for (inputs, error) in [(vec![], Error::InvalidInput), (vec![(0, None)], Error::InvalidInput),
        (vec![(1, None), (1, None)], Error::Duplicate),
        ((1..=65).map(|id| (id, None)).collect(), Error::Limit)] {
        assert_eq!(FileSituationFleetReplay::new(inputs, limits).err(), Some(error.into()));
    }
    let root = Root::new(); let (host, _) = create(&root); let before = root.files();
    assert_eq!(FileSituationFleetReplay::new(vec![(1, Some(reader(&root)))],
        FileSituationFleetLimits { events: 0, ..limits }).err(), Some(Error::Limit.into()));
    assert!(FileSituationFleetReplay::new(vec![(1, Some(reader(&root)))],
        FileSituationFleetLimits { events: host.events.len(), ..limits }).unwrap().complete().is_ok());
    let mut used = reader(&root); used.advance(1).unwrap();
    assert_eq!(FileSituationFleetReplay::new(vec![(1, Some(used))], limits).err(), Some(Error::Binding.into()));
    assert_eq!(root.files(), before);
}

#[test]
fn empty_verified_member_and_unreadable_member_are_distinct_even_at_zero_event_budget() {
    let root = Root::new(); let (_host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let report = FileSituationFleetReplay::new(vec![(1, Some(reader(&root))), (2, None)],
        FileSituationFleetLimits { members: 2, events: 0 }).unwrap().finish().unwrap();
    let missing = report.page(None, 64).unwrap();
    assert_eq!(missing.member, 2); assert!(matches!(missing.observation, Knowledge::Unknown { .. }));
    let empty = report.page(missing.next.as_ref(), 64).unwrap();
    assert_eq!(empty.member, 1); assert_eq!(known(&empty).0.summary.attempts, 0);
    assert_eq!(known(&empty).0.summary.available_units, 100); assert!(empty.next.is_none());
    assert!(!empty.coverage.all_members_known());
}

#[test]
fn anchored_rollback_remains_an_unavailable_member_without_unanchored_retry() {
    let root = Root::new(); let (host, _) = create(&root);
    let anchor = host.history_anchor().unwrap();
    fs::write(host.store.identity().join(storage::CANONICAL),
        journal::encode(&profile(), host.store.identity(), &[]).unwrap()).unwrap();
    let before = root.files();
    let input = FileOversight::begin_read_situation(root.store(), &profile(), Some(&anchor)).ok();
    assert!(input.is_none());
    let report = fleet(vec![(1, input)]).complete().unwrap(); let page = report.page(None, 64).unwrap();
    assert!(matches!(page.observation, Knowledge::Unknown { .. }));
    assert_eq!(page.coverage.unknown_members, 1); assert!(page.next.is_none());
    assert_eq!(root.files(), before);
}
