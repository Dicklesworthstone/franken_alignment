//! Complete messages through original policy, congress, human key and journal.
#![cfg(unix)]
#[path = "support/file_stream.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile, StreamView, STREAM_HEADER_BYTES};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, publication::PublicationBasis};
use fa_reference::{Error, Snapshot};

fn ack(host: &mut FileOversight, id: u64) {
    assert!(matches!(host.reconcile(host.revision(), id).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
}

#[test]
fn message_publication_confirmation_and_finish_are_separate_original_transitions() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    assert!(host.publication_guard_required());
    let first = ready(&mut host, &reviewer, 1, Some("é"));
    let charge = first.action.spec().units;
    assert_eq!(charge, first.action.spec().payload.len() as u64);
    assert_eq!(host.stream_snapshot().unwrap().published.visible(), b"");
    dispatch(&mut host, &first);
    assert_eq!(host.stream_snapshot().unwrap().pending, Some(1));
    assert_eq!(host.stream_message_spec("next", ElapsedTick(100)), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.publish(host.revision(), 1), Err(JournalError::Contract(Error::Incomplete)));
    publish(&mut host, &first);
    let visible = host.stream_snapshot().unwrap();
    assert_eq!(visible.published.visible(), "é".as_bytes());
    assert_eq!(visible.confirmed.visible(), b"");
    assert_eq!(visible.publication.payload, "é".as_bytes());
    assert_eq!(host.inspect().control.ledger.charged, charge);
    let repeated = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(1)).unwrap();
    assert_eq!(repeated.basis, PublicationBasis::PreviouslyResolved);
    assert_eq!(host.inspect().executions, 1);
    ack(&mut host, 1);
    let second = ready(&mut host, &reviewer, 2, Some("世界"));
    let frame = ReleaseFrame::decode(&second.action.spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["é"]);
    assert_eq!(frame.message(), Some("世界"));
    dispatch(&mut host, &second); publish(&mut host, &second); ack(&mut host, 2);
    let finish = ready(&mut host, &reviewer, 3, None);
    assert!(ReleaseFrame::decode(&finish.action.spec().payload).unwrap().is_finish());
    dispatch(&mut host, &finish); publish(&mut host, &finish);
    assert!(!host.stream_snapshot().unwrap().confirmed.finished());
    ack(&mut host, 3);
    let closed = host.stream_snapshot().unwrap();
    assert!(closed.confirmed.finished());
    assert_eq!(closed.published.messages().collect::<Vec<_>>(), vec!["é", "世界"]);
    assert_eq!(closed.confirmed, closed.published);
    assert_eq!(closed.publication.executions, 3);
    assert_eq!(closed.confirmed_target.expected_version, 4);
    assert_eq!(host.stream_finish_spec(ElapsedTick(100)), Err(JournalError::Contract(Error::WrongState)));
}

#[test]
fn changed_evidence_seals_only_the_new_message_and_refunds_only_on_reconciliation() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let first = ready(&mut host, &reviewer, 1, Some("kept"));
    dispatch(&mut host, &first); publish(&mut host, &first); ack(&mut host, 1);
    let charged_prefix = host.inspect().control.ledger.charged;
    let second = ready(&mut host, &reviewer, 2, Some("withheld"));
    dispatch(&mut host, &second);
    let charged = host.inspect().control.ledger.charged;
    let changed = inputs(&second.action, b"different actual helper input");
    let result = host.publish_checked(host.revision(), 2, Some(&changed), snapshot(), ElapsedTick(1)).unwrap();
    assert!(matches!(result.basis, PublicationBasis::Rejected(_)));
    assert_eq!(result.outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    assert_eq!(host.inspect().control.ledger.charged, charged);
    assert_eq!(host.stream_snapshot().unwrap().published.visible(), b"kept");
    assert_eq!(host.stream_snapshot().unwrap().pending, Some(2));
    let retry = host.publish_checked(host.revision(), 2, Some(&second.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(retry.outcome, result.outcome);
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, charged_prefix);
    let third = ready(&mut host, &reviewer, 3, Some("new"));
    dispatch(&mut host, &third); publish(&mut host, &third); ack(&mut host, 3);
    assert_eq!(host.stream_snapshot().unwrap().published.messages().collect::<Vec<_>>(), vec!["kept", "new"]);
}

#[test]
fn reopen_recovers_executed_prefix_without_reissuing_old_keys_or_publishing_twice() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let first = ready(&mut host, &reviewer, 1, Some("visible"));
    dispatch(&mut host, &first); publish(&mut host, &first);
    let charge = host.inspect().control.ledger.charged;
    drop(host);
    let (mut host, reviewer) = FileOversight::open_stream(root.store(), profile(), stream()).unwrap();
    assert!(!host.clock_ready());
    assert_eq!(host.stream_snapshot().unwrap().published.visible(), b"visible");
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"");
    assert_eq!(host.inspect().control.ledger.charged, charge);
    assert_eq!(host.stream_message_spec("next", ElapsedTick(100)), Err(JournalError::Contract(Error::Incomplete)));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(matches!(host.dispatch(host.revision(), &first.automatic, &first.human, &first.action, &first.inputs, snapshot()),
        Err(JournalError::Contract(Error::Binding))));
    let recovered = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(2)).unwrap();
    assert_eq!(recovered.basis, PublicationBasis::PreviouslyResolved);
    ack(&mut host, 1);
    let second = ready(&mut host, &reviewer, 2, Some("next"));
    dispatch(&mut host, &second); publish(&mut host, &second); ack(&mut host, 2);
    assert_eq!(host.stream_snapshot().unwrap().published.messages().collect::<Vec<_>>(), vec!["visible", "next"]);
    assert_eq!(host.inspect().executions, 2);
}

#[test]
fn undisclosed_recovered_message_is_query_only_and_can_be_sealed_without_erasing_history() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    let first = ready(&mut host, &reviewer, 1, Some("old"));
    dispatch(&mut host, &first); publish(&mut host, &first); ack(&mut host, 1);
    let prefix_charge = host.inspect().control.ledger.charged;
    let second = ready(&mut host, &reviewer, 2, Some("unsent"));
    dispatch(&mut host, &second); drop(host);
    let (mut host, reviewer) = FileOversight::open_stream(root.store(), profile(), stream()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&second.inputs), snapshot(), ElapsedTick(2)),
        Err(JournalError::Contract(Error::Missing)));
    assert_eq!(host.reconcile(host.revision(), 2).unwrap(), Reconciliation::AwaitingResolution);
    host.seal_unexecuted(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, prefix_charge);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"old");
    let finish = ready(&mut host, &reviewer, 3, None);
    dispatch(&mut host, &finish); publish(&mut host, &finish); ack(&mut host, 3);
    assert!(host.stream_snapshot().unwrap().published.finished());
}

#[test]
fn independent_stream_contract_is_checked_before_recovery_and_read_only_inspection() {
    let root = Directory::new(); let (host, _) = create(&root);
    let before = std::fs::read(root.store().join("delivery.bin")).unwrap();
    assert_eq!(FileOversight::read_stream_publication(root.store(), &profile(), stream()).unwrap(), host.stream_snapshot().unwrap());
    let wrong = StreamProfile::new(91, 4, 4, 16, 64).unwrap();
    assert!(matches!(FileOversight::read_stream_publication(root.store(), &profile(), wrong), Err(JournalError::Contract(Error::Binding))));
    drop(host);
    assert!(matches!(FileOversight::open_stream(root.store(), profile(), wrong), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), before);
    let (host, _) = FileOversight::open_stream(root.store(), profile(), stream()).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    let invalid = Directory::new(); let mut p = profile(); p.delivery.initial_payload = b"unreviewed".to_vec();
    assert!(matches!(FileOversight::create_stream(invalid.store(), p, stream()), Err(JournalError::Contract(Error::Binding))));
    assert!(!invalid.store().exists());
    let legacy = Directory::new(); let (host, _) = FileOversight::create(legacy.store(), profile()).unwrap();
    assert!(matches!(host.stream_snapshot(), Err(JournalError::Contract(Error::WrongState))));
}

#[test]
fn same_concatenated_text_cannot_substitute_different_reviewed_message_boundaries() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    for (id, message) in [(1, "ab"), (2, "c")] {
        let keys = ready(&mut host, &reviewer, id, Some(message));
        dispatch(&mut host, &keys); publish(&mut host, &keys); ack(&mut host, id);
    }
    let mut spec = host.stream_message_spec("d", ElapsedTick(100)).unwrap();
    let mut changed = spec.payload[..STREAM_HEADER_BYTES].to_vec();
    for message in ["a", "bc"] {
        changed.extend_from_slice(&(message.len() as u32).to_be_bytes());
        changed.extend_from_slice(message.as_bytes());
    }
    changed.extend_from_slice(b"d");
    assert!(ReleaseFrame::decode(&changed).is_ok());
    spec.payload = changed;
    let before = host.inspect();
    assert_eq!(host.propose(host.revision(), 3, spec, snapshot()), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), before);
    let keys = ready(&mut host, &reviewer, 3, Some("d"));
    dispatch(&mut host, &keys); publish(&mut host, &keys); ack(&mut host, 3);
    assert_eq!(host.stream_snapshot().unwrap().published.messages().collect::<Vec<_>>(), vec!["ab", "c", "d"]);
}

#[test]
fn message_limit_and_expired_human_key_never_disclose_partial_output() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root);
    assert_eq!(host.stream_message_spec(&"x".repeat(17), ElapsedTick(100)), Err(JournalError::Contract(Error::Limit)));
    assert!(host.stream_message_spec(&"x".repeat(16), ElapsedTick(100)).is_ok());
    let keys = ready(&mut host, &reviewer, 1, Some("not shown"));
    dispatch(&mut host, &keys);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(31)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    assert_eq!(host.stream_snapshot().unwrap().published, StreamView::empty(stream()));
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.available, 4096);
    let finish = ready(&mut host, &reviewer, 2, None);
    dispatch(&mut host, &finish); publish(&mut host, &finish); ack(&mut host, 2);
    assert!(host.stream_snapshot().unwrap().published.finished());
    assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
}
