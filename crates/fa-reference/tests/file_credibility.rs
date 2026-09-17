//! Actual applied congresses, independent labels and native weight promotion.
#![cfg(unix)]
#[path = "support/file_credibility.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::credibility::{GroundTruth, QualificationFailure};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn real_cases_change_native_weights_but_new_effects_still_require_both_keys() {
    let root = Directory::new(); let (mut host, human, evaluator) = create(&root);
    qualified(&mut host, &evaluator);
    let old = ordinary::ready(&mut host, &human, 3, b"old keys");
    assert!(!host.credibility_report().unwrap().qualified());
    label(&mut host, &evaluator, 3, 3, GroundTruth::Benign);
    let original = update(&host, 9);
    let promotion = host.promote_credibility(host.revision(), &original).unwrap();
    assert_eq!(promotion.change.previous.members["alpha"].weight, 1);
    assert_eq!(promotion.change.current.members["alpha"].weight, 5);
    assert_eq!(promotion.change.current.caps, promotion.change.previous.caps);
    assert_eq!(promotion.change.current.continue_minimum, 2);
    assert_eq!(host.inspect().control.ledger.stages[&3], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert!(host.dispatch(host.revision(), &old.automatic, &old.human, &old.action, &old.inputs, snapshot()).is_err());
    let before = host.inspect(); let report = host.credibility_report().unwrap();
    assert_eq!(host.promote_credibility(0, &original).unwrap(), promotion);
    assert_eq!(host.inspect(), before); assert_eq!(host.credibility_report().unwrap(), report);
    let mut changed = original; changed.expected_evaluation_revision += 1;
    assert!(matches!(host.promote_credibility(host.revision(), &changed), Err(JournalError::Contract(Error::Binding))));
    let keys = ordinary::ready(&mut host, &human, 4, b"fresh keys");
    ordinary::dispatch(&mut host, &keys);
    assert!(host.publish(host.revision(), 4).is_err());
    let outcome = host.publish_checked(host.revision(), 4, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 4).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.review_replay(104).unwrap().anchor().policy.congress, promotion.change.current);
}

#[test]
fn pending_and_discarded_rounds_cannot_disappear_from_promotion_denominators() {
    let root = Directory::new(); let (mut host, _, evaluator) = create(&root);
    qualified(&mut host, &evaluator);
    let original = update(&host, 1); let inputs = begin(&mut host, 3);
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
    assert!(matches!(host.promote_credibility(host.revision(), &original), Err(JournalError::Contract(Error::Incomplete))));
    ordinary::votes(&mut host, 103, Verdict::Allow);
    let changed = ordinary::inputs(inputs.action(), b"substituted application context");
    assert!(host.finish_review(host.revision(), 103, Some(&changed), snapshot()).unwrap().is_err());
    assert!(host.evaluation_ticket(103).is_err());
    let before = host.credibility_report().unwrap(); assert_eq!(before.pending_cases, 1);
    drop(host);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.credibility_report().unwrap(), before);
    assert_eq!(host.inspect().control.ledger.stages[&3], ActionState::Cancelled);
}

#[test]
fn independent_final_labels_are_immutable_censoring_resolves_and_origins_do_not_multiply() {
    let root = Directory::new(); let (mut host, _, evaluator) = create(&root);
    review(&mut host, 1, Verdict::Hold);
    label(&mut host, &evaluator, 1, 7, GroundTruth::Censored);
    assert_eq!(host.credibility_report().unwrap().censored_cases, 1);
    label(&mut host, &evaluator, 1, 7, GroundTruth::Violation);
    let revision = host.revision();
    assert!(!label(&mut host, &evaluator, 1, 7, GroundTruth::Violation)); assert_eq!(host.revision(), revision);
    assert_eq!(host.evaluation_history(101).unwrap().len(), 2);
    let ticket = host.evaluation_ticket(101).unwrap();
    assert_eq!(evaluator.assess(&mut host, revision, &ticket, assessment(7, GroundTruth::Benign)), Err(JournalError::Contract(Error::Binding)));
    review(&mut host, 2, Verdict::Allow); label(&mut host, &evaluator, 2, 7, GroundTruth::Violation);
    review(&mut host, 3, Verdict::Allow); label(&mut host, &evaluator, 3, 8, GroundTruth::Benign);
    let report = host.credibility_report().unwrap();
    assert_eq!(report.violation_origins, 1); assert_eq!(report.benign_origins, 1);
    assert_eq!(report.members["alpha"].false_negatives, 1);
    assert_eq!(report.members["alpha"].true_positives, 0);
    assert!(report.failures.contains(&QualificationFailure::Recall("alpha".into())));
    let request = update(&host, 1); assert!(host.promote_credibility(host.revision(), &request).is_err());
}

#[test]
fn evaluator_and_ticket_brands_refuse_cross_owner_and_lost_roles() {
    let root = Directory::new(); let other_root = Directory::new();
    let (mut host, _, evaluator) = create(&root); let (mut other, _, other_evaluator) = create(&other_root);
    review(&mut host, 1, Verdict::Hold); review(&mut other, 1, Verdict::Hold);
    let ticket = host.evaluation_ticket(101).unwrap(); let a = assessment(1, GroundTruth::Violation);
    let revision = host.revision();
    assert_eq!(other_evaluator.assess(&mut host, revision, &ticket, a), Err(JournalError::Contract(Error::Binding)));
    let other_ticket = other.evaluation_ticket(101).unwrap();
    assert_eq!(evaluator.assess(&mut host, revision, &other_ticket, a), Err(JournalError::Contract(Error::Binding)));
    assert!(evaluator.assess(&mut host, revision, &ticket, a).unwrap());
    let before = host.credibility_report().unwrap(); drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    let revision = host.revision(); let fresh = host.evaluation_ticket(101).unwrap();
    assert_eq!(evaluator.assess(&mut host, revision, &fresh, a), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.credibility_report().unwrap(), before);
    assert!(!host.clock_ready());
}

#[test]
fn weight_promotion_keeps_sent_charges_and_requires_original_endpoint_reconciliation() {
    let root = Directory::new(); let (mut host, human, evaluator) = create(&root); qualified(&mut host, &evaluator);
    let keys = ordinary::ready(&mut host, &human, 3, b"sent before weight change");
    label(&mut host, &evaluator, 3, 3, GroundTruth::Benign); ordinary::dispatch(&mut host, &keys);
    let request = update(&host, 1); host.promote_credibility(host.revision(), &request).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 0);
    let outcome = host.publish_checked(host.revision(), 3, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
    assert_eq!(outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.reconcile(host.revision(), 3).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn late_enable_and_invalid_protocol_refuse_without_weakening_original_admission() {
    let root = Directory::new(); let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let before = host.inspect(); let mut invalid = protocol(); invalid.precision_floor.denominator = 0;
    assert!(host.enable_credibility(host.revision(), invalid).is_err()); assert_eq!(host.inspect(), before);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.propose(host.revision(), 1, ordinary::spec(&host, b"admitted before evaluation"), snapshot()).unwrap();
    assert!(matches!(host.enable_credibility(host.revision(), protocol()), Err(JournalError::Contract(Error::WrongState))));
    assert!(host.credibility_protocol().unwrap().is_none());
    let valid = Directory::new(); let (mut host, _, _) = create(&valid);
    let receipt = review(&mut host, 1, Verdict::Allow);
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert!(host.enable_credibility(host.revision(), protocol()).is_err());
}
