//! Original evaluator custody gates immutable discovery before assessment work.
#![cfg(unix)]

#[path = "support/file_credibility.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::round::Verdict;
use fixture::*;
use std::fs;

fn persisted(root: &Directory) -> Vec<u8> {
    fs::read(root.store().join("delivery.bin")).unwrap()
}

#[test]
fn original_evaluator_reads_the_actual_case_without_time_or_journal_work() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    assert_eq!(evaluator.check_owner(&host), Ok(()));
    review(&mut host, 1, Verdict::Hold);
    let expected = host.evaluation_ticket(101).unwrap();
    let revision = host.revision();
    let before = host.inspect();
    let report = host.credibility_report().unwrap();
    let bytes = persisted(&root);

    let ticket = evaluator.evaluation_case(&host, 101).unwrap();
    assert_eq!(ticket.case(), expected.case());
    assert_eq!(
        evaluator.evaluation_case(&host, 101).unwrap().case(),
        expected.case()
    );
    assert_eq!(host.revision(), revision);
    assert_eq!(host.inspect(), before);
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(persisted(&root), bytes);
    assert!(host.evaluation_history(101).unwrap().is_empty());

    assert!(
        evaluator
            .assess(
                &mut host,
                revision,
                &ticket,
                assessment(7, GroundTruth::Violation)
            )
            .unwrap()
    );
    assert_eq!(host.evaluation_history(101).unwrap().len(), 1);
    assert_eq!(
        evaluator.evaluation_case(&host, 101).unwrap().case(),
        expected.case()
    );
}

#[test]
fn unknown_pending_and_unapplied_reviews_do_not_become_cases() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    let inputs = begin(&mut host, 1);
    let revision = host.revision();
    let before = host.inspect();
    let bytes = persisted(&root);

    for round in [0, 101, 999] {
        assert!(evaluator.evaluation_case(&host, round).is_err());
    }
    assert_eq!(host.revision(), revision);
    assert_eq!(host.inspect(), before);
    assert_eq!(persisted(&root), bytes);

    ordinary::votes(&mut host, 101, Verdict::Allow);
    let changed = ordinary::inputs(inputs.action(), b"substituted application context");
    assert!(
        host.finish_review(host.revision(), 101, Some(&changed), snapshot())
            .unwrap()
            .is_err()
    );
    let revision = host.revision();
    let before = host.inspect();
    let report = host.credibility_report().unwrap();
    let bytes = persisted(&root);
    assert!(evaluator.evaluation_case(&host, 101).is_err());
    assert_eq!(host.revision(), revision);
    assert_eq!(host.inspect(), before);
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(report.pending_cases, 1);
    assert_eq!(persisted(&root), bytes);
}

#[test]
fn equal_scope_and_case_numbers_cannot_transfer_evaluator_custody() {
    let root = Directory::new();
    let other_root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    let (mut other, _, other_evaluator) = create(&other_root);
    assert_eq!(evaluator.check_owner(&host), Ok(()));
    assert_eq!(other_evaluator.check_owner(&other), Ok(()));
    assert_eq!(
        evaluator.check_owner(&other),
        Err(JournalError::Contract(Error::Binding))
    );
    review(&mut host, 1, Verdict::Hold);
    review(&mut other, 1, Verdict::Hold);
    assert_eq!(
        host.evaluation_ticket(101).unwrap().case(),
        other.evaluation_ticket(101).unwrap().case()
    );

    let revision = other.revision();
    let before = other.inspect();
    let report = other.credibility_report().unwrap();
    let bytes = persisted(&other_root);
    for round in [101, 999] {
        assert!(matches!(
            evaluator.evaluation_case(&other, round),
            Err(JournalError::Contract(Error::Binding))
        ));
    }
    assert_eq!(other.revision(), revision);
    assert_eq!(other.inspect(), before);
    assert_eq!(other.credibility_report().unwrap(), report);
    assert_eq!(persisted(&other_root), bytes);
    assert!(other_evaluator.evaluation_case(&other, 101).is_ok());
}

#[test]
fn recovery_preserves_the_case_but_retires_the_old_evaluator_role() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    review(&mut host, 1, Verdict::Hold);
    let original = evaluator.evaluation_case(&host, 101).unwrap();
    drop(host);

    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(
        host.evaluation_ticket(101).unwrap().case(),
        original.case()
    );
    let revision = host.revision();
    let before = host.inspect();
    let bytes = persisted(&root);
    assert!(!host.clock_ready());
    assert_eq!(
        evaluator.check_owner(&host),
        Err(JournalError::Contract(Error::Binding))
    );
    assert!(matches!(
        evaluator.evaluation_case(&host, 101),
        Err(JournalError::Contract(Error::Binding))
    ));
    assert_eq!(host.revision(), revision);
    assert_eq!(host.inspect(), before);
    assert_eq!(persisted(&root), bytes);
    assert!(!host.clock_ready());
}

#[test]
fn storage_failure_withholds_cases_and_preserves_unconfirmed_bytes() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    review(&mut host, 1, Verdict::Hold);
    assert!(evaluator.evaluation_case(&host, 101).is_ok());
    let revision = host.revision();
    let bytes = persisted(&root);
    let pending = root.store().join("delivery.pending");
    let unconfirmed = b"independently unresolved staged bytes";
    fs::write(&pending, unconfirmed).unwrap();
    assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
    assert!(host.storage_failure().is_some());

    assert_eq!(evaluator.check_owner(&host), Err(JournalError::Unavailable));
    for round in [101, 999] {
        assert!(matches!(
            evaluator.evaluation_case(&host, round),
            Err(JournalError::Unavailable)
        ));
    }
    assert_eq!(host.revision(), revision);
    assert_eq!(persisted(&root), bytes);
    assert_eq!(
        fs::read(&pending).unwrap().as_slice(),
        unconfirmed.as_slice()
    );
}
