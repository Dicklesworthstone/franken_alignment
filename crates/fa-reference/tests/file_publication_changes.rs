//! Feed notifications exercise the original durable committee/two-key consumer.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick, FrozenAction};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationEvidence, FilePublicationInputs};
use fa_reference::action::consequence::delivery::publication_gate::changes::{ChangeRouting, PublicationChange, PublicationChangePolicy};
use fa_reference::action::consequence::oversight::{CommitteeInput, human::HumanDisposition};
use fa_reference::witness::{DomainProjection, QueryRole, WitnessRequest};
use fa_reference::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use fa_reference::{Error, Snapshot};

const FEED: u64 = 800;
fn policy() -> PublicationChangePolicy {
    PublicationChangePolicy { source: FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_048_576 } }
}
fn tracked(root: &Directory, p: PublicationChangePolicy) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = source_host(root);
    host.enable_publication_changes(host.revision(), p).unwrap();
    (host, reviewer)
}
fn bound(host: &mut FileOversight, id: u64, requests: Vec<WitnessRequest>, opaque: bool)
    -> (FrozenAction, CommitteeInput, FilePublicationInputs)
{
    let (action, committee) = reviewed(host, id, b"visible");
    let supplied = observations(&committee, 1, &[0, 2, 4]);
    let original = FilePublicationInputs::new(supplied.structured().cloned(),
        if opaque { supplied.opaque().cloned() } else { None });
    host.bind_publication_evidence(host.revision(), id,
        FilePublicationEvidence::new(original.clone(), requests).unwrap()).unwrap();
    host.record_publication_inputs(host.revision(), id, 0, Some(original.clone())).unwrap();
    (action, committee, original)
}
fn keys(host: &mut FileOversight, reviewer: &FileHumanReviewer) -> (Keys, FilePublicationInputs) {
    let (action, inputs, original) = bound(host, 1, requests(), false);
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    (Keys { action, inputs, automatic, human, request }, original)
}
fn domain(input: &FilePublicationInputs) -> DomainProjection {
    input.structured().unwrap().snapshot().domain_input().domain()
}
fn notice(sequence: u64, change: WitnessChange) -> PublicationChange { PublicationChange { source: FEED, sequence, change } }
fn record(host: &mut FileOversight, sequence: u64, change: WitnessChange) -> std::rc::Rc<fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangeReport> {
    host.record_publication_change(host.revision(), notice(sequence, change)).unwrap()
}
fn refresh_inputs(host: &mut FileOversight, id: u64, input: &FilePublicationInputs) {
    host.record_publication_inputs(host.revision(), id, host.publication_input_revision(id).unwrap(), Some(input.clone())).unwrap();
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn reverse_index_routes_all_witness_kinds_and_never_narrows_an_opaque_binding() {
    let root = Directory::new(); let (mut host, _) = tracked(&root, policy());
    let req = [WitnessRequest::AbsentKey { key: 1 }, WitnessRequest::EmptyRange { start: 6, end: 9 },
        WitnessRequest::RangeMembers { start: 2, end: 6 }, WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject }];
    let mut d = None;
    for (i, r) in req.into_iter().enumerate() {
        let (_, _, original) = bound(&mut host, i as u64 + 1, vec![r], false); d = Some(domain(&original));
    }
    bound(&mut host, 5, vec![WitnessRequest::AbsentKey { key: 99 }], true);
    for (sequence, (key, expected)) in [(1, vec![1, 5]), (7, vec![2, 5]), (3, vec![3, 5]), (0, vec![4, 5]), (42, vec![5])].into_iter().enumerate() {
        let report = record(&mut host, sequence as u64 + 1, WitnessChange::Key { domain: d.unwrap(), key });
        assert_eq!(report.routing, ChangeRouting::Indexed);
        assert_eq!(report.affected, expected);
        assert!(report.status.complete());
    }
    for id in 1..=4 { assert_eq!(host.publication_input_revision(id).unwrap(), 2); }
    assert_eq!(host.publication_input_revision(5).unwrap(), 6);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn disjoint_notice_preserves_a_permit_and_successful_publication_but_overlap_withdraws_it() {
    for key in [99, 1, 3, 7, 0] {
        let root = Directory::new(); let (mut host, reviewer) = tracked(&root, policy());
        let (keys, original) = keys(&mut host, &reviewer);
        let report = record(&mut host, 1, WitnessChange::Key { domain: domain(&original), key });
        let sent = host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot());
        if key == 99 {
            assert!(report.affected.is_empty()); sent.unwrap();
            let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
            assert_eq!(result.basis, PublicationBasis::Revalidated);
            assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
            host.reconcile(host.revision(), 1).unwrap();
            assert_eq!(host.inspect().executions, 1);
        } else {
            assert_eq!(report.affected, vec![1]);
            assert_eq!(sent, Err(JournalError::Contract(Error::Incomplete)));
            assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
            assert_eq!(host.inspect().control.ledger.reserved, 16);
            assert_eq!(host.inspect().control.ledger.charged, 0);
        }
    }
}

#[test]
fn missing_tail_blocks_even_new_captures_and_final_repair_withdraws_them_again() {
    let root = Directory::new(); let (mut host, reviewer) = tracked(&root, policy());
    let (keys, original) = keys(&mut host, &reviewer);
    let change = WitnessChange::Key { domain: domain(&original), key: 99 };
    let gap = record(&mut host, 3, change);
    assert_eq!(gap.routing, ChangeRouting::MissingTail);
    assert_eq!((gap.status.through, gap.status.observed_through), (0, 3));
    assert_eq!(gap.affected, vec![1]);
    for sequence in 1..=3 {
        refresh_inputs(&mut host, 1, &original);
        assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
            Err(JournalError::Contract(Error::Incomplete)));
        let repaired = record(&mut host, sequence, change);
        assert_eq!(repaired.routing, ChangeRouting::RecoveringTail);
        assert_eq!(repaired.affected, vec![1]);
        assert_eq!(repaired.status.complete(), sequence == 3);
    }
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Contract(Error::Incomplete)));
    refresh_inputs(&mut host, 1, &original);
    dispatch(&mut host, &keys);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
}

#[test]
fn notification_after_dispatch_seals_and_only_original_reconciliation_refunds() {
    let root = Directory::new(); let (mut host, reviewer) = tracked(&root, policy());
    let (keys, original) = keys(&mut host, &reviewer); dispatch(&mut host, &keys);
    record(&mut host, 1, WitnessChange::Range { domain: domain(&original), start: 1, end: 5 });
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Incomplete));
    assert_eq!(result.outcome, sealed());
    assert_eq!(host.inspect().control.ledger.charged, 16);
    refresh_inputs(&mut host, 1, &original);
    let retry = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(3)).unwrap();
    assert_eq!(retry.basis, PublicationBasis::PreviouslyResolved); assert_eq!(retry.outcome, sealed());
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(sealed()));
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn budget_or_malformed_notice_conservatively_withdraws_all_without_partial_results() {
    for exhausted in [false, true] {
        let root = Directory::new();
        let p = if exhausted { PublicationChangePolicy { lookup: RoutingBudget::default(), ..policy() } } else { policy() };
        let (mut host, _) = tracked(&root, p);
        let (_, _, original) = bound(&mut host, 1, requests(), false);
        bound(&mut host, 2, requests(), false);
        let change = if exhausted { WitnessChange::Key { domain: domain(&original), key: 99 } }
            else { WitnessChange::Range { domain: domain(&original), start: 5, end: 5 } };
        let report = record(&mut host, 1, change);
        assert_eq!(report.routing, ChangeRouting::Conservative(if exhausted { Error::Incomplete } else { Error::InvalidInput }));
        assert_eq!(report.affected, vec![1, 2]); assert!(report.status.complete());
        assert_eq!(report.spent, RoutingBudget::default());
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    }
}

#[test]
fn gap_and_original_bindings_survive_reopen_without_reviving_old_keys() {
    let root = Directory::new(); let (mut host, reviewer) = tracked(&root, policy());
    let (keys, original) = keys(&mut host, &reviewer);
    let expected = record(&mut host, 2, WitnessChange::All);
    drop(reviewer); drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.publication_change_status().unwrap(), expected.status);
    assert_eq!(host.publication_change_report().unwrap().unwrap(), expected);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
    let (_, committee, new_input) = bound(&mut host, 2, requests(), false);
    assert!(matches!(host.authorize(host.revision(), 2, &committee, snapshot()), Err(JournalError::Contract(Error::Incomplete))));
    for sequence in 1..=2 { record(&mut host, sequence, WitnessChange::Key { domain: domain(&original), key: 99 }); }
    refresh_inputs(&mut host, 2, &new_input);
    assert!(host.authorize(host.revision(), 2, &committee, snapshot()).is_ok());
}

#[test]
fn failed_durable_notice_quarantines_the_owner_instead_of_retaining_old_eligibility() {
    let root = Directory::new(); let (mut host, reviewer) = tracked(&root, policy());
    let (keys, original) = keys(&mut host, &reviewer);
    let before = host.inspect();
    std::fs::write(root.store().join("delivery.pending"), b"inert staged data").unwrap();
    assert!(matches!(host.record_publication_change(host.revision(), notice(1, WitnessChange::Domain { domain: domain(&original) })), Err(JournalError::Io(_))));
    assert!(host.storage_failure().is_some()); assert_eq!(host.inspect(), before);
    assert_eq!(host.publication_change_status(), Err(JournalError::Unavailable));
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()), Err(JournalError::Unavailable));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), before);
    drop(reviewer); drop(host);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn stale_or_foreign_notices_cannot_change_state_or_reopen_configuration() {
    let root = Directory::new(); let (mut host, _) = tracked(&root, policy());
    let (_, _, original) = bound(&mut host, 1, requests(), false);
    record(&mut host, 1, WitnessChange::Key { domain: domain(&original), key: 99 });
    let before = host.inspect();
    assert_eq!(host.record_publication_change(host.revision(), notice(1, WitnessChange::All)), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.record_publication_change(host.revision(), PublicationChange { source: FEED + 1, ..notice(2, WitnessChange::All) }), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.record_publication_change(host.revision() - 1, notice(2, WitnessChange::All)), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.enable_publication_changes(host.revision(), policy()), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_none());
}

#[test]
fn source_bound_freshness_is_withdrawn_without_erasing_producer_history_or_spending_keys() {
    let root = Directory::new(); let (mut host, reviewer) = tracked(&root, policy());
    let keys = source_keys(&mut host, &reviewer, &root, 1);
    refresh(&mut host, &root, 1);
    assert!(host.publication_source(1).unwrap().unwrap().fresh);
    record(&mut host, 1, WitnessChange::All);
    let source_state = host.publication_source(1).unwrap().unwrap();
    assert!(!source_state.fresh); assert_eq!(source_state.generation, 1);
    assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()), Err(JournalError::Contract(Error::Incomplete)));
    refresh(&mut host, &root, 1); dispatch(&mut host, &keys);
    refresh(&mut host, &root, 1);
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap().basis, PublicationBasis::Revalidated);
}

#[test]
fn executed_receipt_wins_even_when_a_new_change_gap_remains_unrepaired() {
    let root = Directory::new(); let (mut host, reviewer) = tracked(&root, policy());
    let (keys, _) = keys(&mut host, &reviewer); dispatch(&mut host, &keys);
    let prior = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    record(&mut host, 5, WitnessChange::All);
    let again = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(3)).unwrap();
    assert_eq!(again.outcome, prior.outcome); assert_eq!(again.basis, PublicationBasis::PreviouslyResolved);
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 1);
    assert!(!host.publication_change_status().unwrap().complete());
}
