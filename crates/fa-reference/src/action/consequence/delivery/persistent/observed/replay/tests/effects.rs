//! Original two-key/file effects; congress ballots are controlled test inputs.
use super::*;
use super::super::FileReviewEffectReplay;
use crate::action::{ActionState, FrozenAction};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason,
    persistent::Reconciliation};

fn create(root: &Directory, guarded: bool) -> (FileOversight, FileHumanReviewer) {
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    if guarded { host.enable_publication_guard(host.revision()).unwrap(); }
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, human)
}
fn begin(host: &mut FileOversight, attempt: u64, round: u64, input: &CommitteeInput) {
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    let digest = commitment(round, "judge", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "judge", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "judge", Verdict::Allow, b"salt".to_vec()).unwrap();
    assert_eq!(input.action(), host.machine.actions.get(&attempt).unwrap());
}
fn review(host: &mut FileOversight, attempt: u64, round: u64) -> (FrozenAction, CommitteeInput) {
    let action = host.propose(host.revision(), attempt, ActionSpec { version: VERSION,
        scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"publish".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16,
    }, snapshot()).unwrap();
    let capture = EvidenceSnapshot::new(EvidenceIdentity { source: 7, generation: 1,
        scope: profile().delivery.scope }, snapshot(),
        BTreeMap::from([("judge".into(), b"original context".to_vec())])).unwrap();
    let input = capture.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), attempt, 0, input.clone()).unwrap();
    begin(host, attempt, round, &input);
    host.finish_review(host.revision(), round, Some(&input), snapshot()).unwrap().unwrap();
    (action, input)
}
fn dispatch(host: &mut FileOversight, human: &FileHumanReviewer, attempt: u64,
    action: &FrozenAction, input: &CommitteeInput) -> (u64, u64, u64)
{
    let automatic = host.authorize(host.revision(), attempt, input, snapshot()).unwrap();
    let authorized = host.revision();
    let request = host.request_human_approval(host.revision(), 1000 + attempt, attempt,
        input, ElapsedTick(50)).unwrap();
    let revision = host.revision(); let key = human.approve(host, revision, &request).unwrap();
    let approved = host.revision();
    host.dispatch(host.revision(), &automatic, &key, action, input, snapshot()).unwrap();
    (authorized, approved, host.revision())
}
fn complete(mut cursor: super::super::FileReviewReplayCursor, quantum: usize) -> FileReviewEffectReplay {
    while cursor.progress().status == super::super::FileReviewReplayStatus::Replaying {
        cursor.advance(cursor.progress().replayed_events, quantum).unwrap();
    }
    let progress = cursor.progress();
    assert_eq!(progress.attempted_events, progress.total_events);
    cursor.finish_with_effects().unwrap()
}
fn audit(host: &FileOversight, round: u64) -> FileReviewEffectReplay {
    let expected = complete(host.begin_review_replay(round).unwrap(), 1);
    assert_eq!(expected.review(), &host.review_replay(round).unwrap());
    assert_eq!(expected.review().journal_snapshot(), &host.inspect());
    let anchor = host.history_anchor().unwrap();
    for quantum in [1, 3, 4096] {
        let disk = FileOversight::begin_read_review_replay_anchored(host.store.identity(),
            &host.profile, round, &anchor).unwrap();
        assert_eq!(complete(disk, quantum), expected);
    }
    expected
}

#[test]
fn exact_two_key_dispatch_outcome_and_reconciliation_are_separate_observations() {
    for guarded in [false, true] { for sweep in [false, true] {
        let root = Directory::new(); let (mut host, human) = create(&root, guarded);
        let (action, input) = review(&mut host, 1, 101); let reviewed = host.revision();
        let record = audit(&host, 101); assert!(record.effect().dispatch().is_none());
        assert!(record.effect().outcome().is_none()); assert!(!record.effect().selected_review_dispatched());
        let (authorized, _, sent) = dispatch(&mut host, &human, 1, &action, &input);
        let record = audit(&host, 101); let link = record.effect().dispatch().unwrap();
        assert_eq!(link.revision(), sent); assert_eq!(link.authorization().revision(), authorized);
        assert_eq!(link.authorization().round(), 101); assert_eq!(link.authorization().review_revision(), reviewed);
        assert_eq!(link.authorization().input_revision(), 1);
        assert_eq!(link.human(), &host.human_status(1001).unwrap());
        assert!(record.effect().selected_review_dispatched()); assert!(record.effect().outcome().is_none());
        if guarded {
            host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).unwrap();
        } else { host.publish(host.revision(), 1).unwrap(); }
        let published = host.revision(); let record = audit(&host, 101);
        let executed = EndpointOutcome::Executed { resulting_version: 2 };
        assert_eq!(record.effect().outcome().unwrap().outcome(), executed);
        assert_eq!(record.effect().outcome().unwrap().revision(), published);
        assert!(record.effect().reconciliation().is_none());
        if sweep { host.reconcile_pending(host.revision()).unwrap(); }
        else { host.reconcile(host.revision(), 1).unwrap(); }
        let record = audit(&host, 101); let settled = record.effect().reconciliation().unwrap();
        assert_eq!(settled.revision(), host.revision()); assert_eq!(settled.result(), Ok(Reconciliation::Resolved(executed)));
        assert_eq!(record.effect().outcome().unwrap().revision(), published);
        assert_eq!(record.effect().stage_at_end(), host.inspect().control.ledger.stages[&1]);
        assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().control.ledger.charged, 16);
    }}
}

#[test]
fn another_attempts_publication_does_not_prove_this_review_executed() {
    let root = Directory::new(); let (mut host, human) = create(&root, false);
    review(&mut host, 1, 101);
    let (action, input) = review(&mut host, 2, 102);
    dispatch(&mut host, &human, 2, &action, &input); host.publish(host.revision(), 2).unwrap();
    host.reconcile_pending(host.revision()).unwrap();
    let first = audit(&host, 101); let second = audit(&host, 102);
    assert_eq!(first.review().journal_snapshot().executions, 1);
    assert_eq!(first.effect().attempt(), 1); assert!(first.effect().dispatch().is_none());
    assert!(first.effect().outcome().is_none()); assert!(first.effect().reconciliation().is_none());
    assert!(!first.effect().selected_review_dispatched());
    assert_eq!(second.effect().attempt(), 2); assert!(second.effect().selected_review_dispatched());
    assert_eq!(second.effect().outcome().unwrap().outcome(), EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn later_applied_review_is_the_consumed_authorization_not_the_selected_earlier_review() {
    let root = Directory::new(); let (mut host, human) = create(&root, false);
    let (action, input) = review(&mut host, 1, 101);
    begin(&mut host, 1, 102, &input);
    host.finish_review(host.revision(), 102, Some(&input), snapshot()).unwrap().unwrap();
    let later = host.revision();
    dispatch(&mut host, &human, 1, &action, &input); host.publish(host.revision(), 1).unwrap();
    let first = audit(&host, 101); let second = audit(&host, 102);
    assert_eq!(first.effect().dispatch(), second.effect().dispatch());
    assert_eq!(first.effect().dispatch().unwrap().authorization().round(), 102);
    assert_eq!(first.effect().dispatch().unwrap().authorization().review_revision(), later);
    assert!(!first.effect().selected_review_dispatched()); assert!(second.effect().selected_review_dispatched());
    assert_eq!(first.effect().outcome(), second.effect().outcome());
}

#[test]
fn refused_later_review_cannot_claim_the_prior_reviews_consumed_key() {
    let root = Directory::new(); let (mut host, human) = create(&root, false);
    let (action, input) = review(&mut host, 1, 101);
    begin(&mut host, 1, 102, &input);
    assert_eq!(host.finish_review(host.revision(), 102, None, snapshot()).unwrap(), Err(Error::Incomplete));
    dispatch(&mut host, &human, 1, &action, &input); host.publish(host.revision(), 1).unwrap();
    let refused = audit(&host, 102); let original = audit(&host, 101);
    assert_eq!(refused.review().application(), &Err(Error::Incomplete));
    assert_eq!(refused.effect().dispatch().unwrap().authorization().round(), 101);
    assert!(!refused.effect().selected_review_dispatched()); assert!(original.effect().selected_review_dispatched());
}

#[test]
fn sealed_nonexecution_is_not_refunded_until_original_receipt_reconciliation() {
    let root = Directory::new(); let (mut host, human) = create(&root, true);
    let (action, input) = review(&mut host, 1, 101); dispatch(&mut host, &human, 1, &action, &input);
    host.publish_checked(host.revision(), 1, None, snapshot(), ElapsedTick(1)).unwrap();
    let first = host.revision(); let record = audit(&host, 101);
    let sealed = EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed };
    assert_eq!(record.effect().outcome().unwrap().outcome(), sealed);
    assert!(record.effect().reconciliation().is_none());
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 0);
    host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(audit(&host, 101).effect().outcome().unwrap().revision(), first);
    host.reconcile(host.revision(), 1).unwrap(); let record = audit(&host, 101);
    assert_eq!(record.effect().reconciliation().unwrap().result(), Ok(Reconciliation::Resolved(sealed)));
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn recovery_and_pending_queries_keep_dispatched_uncertainty_distinct_from_nonexecution() {
    let root = Directory::new(); let (mut host, human) = create(&root, false);
    let (action, input) = review(&mut host, 1, 101); dispatch(&mut host, &human, 1, &action, &input);
    drop(host); let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    let record = audit(&host, 101); assert!(record.effect().outcome().is_none());
    assert_eq!(record.effect().stage_at_end(), ActionState::Unknown);
    assert_eq!(record.effect().reconciliation().unwrap().result(), Ok(Reconciliation::AwaitingResolution));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.seal_unexecuted(host.revision(), 1).unwrap(); let record = audit(&host, 101);
    assert_eq!(record.effect().outcome().unwrap().outcome(), EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    assert_eq!(record.effect().outcome().unwrap().revision(), record.effect().reconciliation().unwrap().revision());
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn ambiguous_publication_reads_actual_canonical_outcome_without_claiming_acknowledgment() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, human) = create(&root, false);
        let (action, input) = review(&mut host, 1, 101); dispatch(&mut host, &human, 1, &action, &input);
        host.store.fail_once(barrier); assert!(host.publish(host.revision(), 1).is_err());
        assert!(matches!(host.begin_review_replay(101), Err(JournalError::Unavailable)));
        let bytes = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();
        let staged = std::fs::read(root.store().join("delivery.pending")).ok();
        let record = complete(FileOversight::begin_read_review_replay(root.store(), &profile(), 101).unwrap(), 1);
        assert!(record.effect().selected_review_dispatched());
        assert_eq!(record.effect().outcome().is_some(), barrier == JournalIo::DirectorySync);
        assert!(record.effect().reconciliation().is_none());
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
        assert_eq!(std::fs::read(root.store().join("delivery.pending")).ok(), staged);
    }
}

#[test]
fn effect_export_waits_for_every_suffix_event_and_cancellation_changes_nothing() {
    let root = Directory::new(); let (mut host, human) = create(&root, false);
    let (action, input) = review(&mut host, 1, 101); dispatch(&mut host, &human, 1, &action, &input);
    host.publish(host.revision(), 1).unwrap(); let published = host.revision();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap(); let final_revision = host.revision();
    let bytes = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();
    for cut in 0..=final_revision {
        let mut cursor = host.begin_review_replay(101).unwrap();
        if cut != 0 { cursor.advance(0, cut as usize).unwrap(); }
        if cut == published { assert!(matches!(cursor.finish_with_effects(), Err(JournalError::Contract(Error::Incomplete)))); }
        else { cursor.cancel(); }
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
        assert_eq!(host.revision(), final_revision);
    }
    let mut invalid = host.events.clone(); invalid.push(Event::Core(super::super::super::BaseEvent::Time(ElapsedTick(0))));
    let bad = journal::encode(&host.profile, host.store.identity(), &invalid).unwrap();
    std::fs::write(root.store().join(storage::CANONICAL), &bad).unwrap();
    let mut cursor = FileOversight::begin_read_review_replay(root.store(), &profile(), 101).unwrap();
    assert!(cursor.advance(0, 4096).is_err());
    assert!(cursor.finish_with_effects().is_err());
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), bad);
}
