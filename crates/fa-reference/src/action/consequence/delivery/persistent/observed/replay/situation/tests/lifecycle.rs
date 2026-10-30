use super::*;

#[test]
fn real_unresolved_work_is_first_and_every_page_retains_whole_cut_accounting() {
    let root = Root::new(); let (mut host, reviewer) = create(&root);
    propose(&mut host, 1, false); propose(&mut host, 2, true); dispatch(&mut host, &reviewer, 9);
    let before = host.inspect(); let files = root.files();
    let report = host.situation().unwrap(); let (first, basis) = page(&report, None, 1);
    assert_eq!(first.attempts[0].attempt, 9); assert_eq!(first.summary.attempts, 3);
    assert_eq!((first.summary.unresolved_attempts, first.summary.pending_attempts, first.summary.terminal_attempts), (1, 1, 1));
    assert_eq!((first.summary.charged_units, first.summary.reserved_units), (16, 0));
    assert_eq!(basis.origin, FileSituationOrigin::AcknowledgedOwner);
    assert_eq!((basis.journal_revision, basis.control_sequence), (host.revision(), before.control.sequence));
    let (second, again) = page(&report, first.next.as_ref(), 1);
    assert_eq!(second.attempts[0].attempt, 2); assert_eq!(second.summary, first.summary); assert_eq!(again, basis);
    let (last, _) = page(&report, second.next.as_ref(), 1);
    assert_eq!(last.attempts[0].attempt, 1); assert!(last.next.is_none());
    assert_eq!(last.attempts[0].stage, ActionState::Denied);
    assert_eq!(root.files(), files); assert_eq!(host.inspect(), before);
    for secret in ["secret-payload", "private-question", "private-profile", "initial-secret"] {
        assert!(!format!("{report:?} {first:?}").contains(secret));
    }
}

#[test]
fn abandoned_unknown_remains_a_charged_unresolved_liability_not_a_terminal_success() {
    let root = Root::new(); let (mut host, reviewer) = create(&root);
    dispatch(&mut host, &reviewer, 1); host.fence(host.revision()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    let control = host.inspect().control;
    host.abandon_unknown(host.revision(), FileAbandonRequest { operation: 7, attempt: 1,
        expected_control_sequence: control.sequence, expected_authority_epoch: control.ledger.epoch }).unwrap();
    let report = FileOversight::read_situation(root.store(), &profile(), None).unwrap();
    let (result, basis) = page(&report, None, 64);
    assert_eq!(result.attempts[0].stage, ActionState::IrrecoverablyUnknown);
    assert_eq!((result.summary.unresolved_attempts, result.summary.irrecoverably_unknown_attempts,
        result.summary.terminal_attempts, result.summary.charged_units), (1, 1, 0, 16));
    assert_eq!(result.summary.executions, 0);
    assert_eq!(basis.origin, FileSituationOrigin::CanonicalImage);
}

#[test]
fn actual_confirmed_publication_is_distinct_from_the_unknown_case() {
    let root = Root::new(); let (mut host, reviewer) = create(&root);
    dispatch(&mut host, &reviewer, 1); host.publish(host.revision(), 1).unwrap();
    host.reconcile(host.revision(), 1).unwrap();
    let files = root.files(); let report = FileOversight::read_situation(root.store(), &profile(), None).unwrap();
    let (result, _) = page(&report, None, 1);
    assert_eq!(result.attempts[0].stage, ActionState::Confirmed);
    assert_eq!((result.summary.executions, result.summary.terminal_attempts, result.summary.unresolved_attempts), (1, 1, 0));
    assert_eq!(result.summary.charged_units, 16); assert_eq!(root.files(), files);
}

