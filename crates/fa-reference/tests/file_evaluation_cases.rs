//! Bounded discovery over actual applied native reviews, never invented rounds
//! or labels. These tests do not establish independent label truth or alignment.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/file_credibility.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::{
    FileEvaluationCasePage, FileEvaluationCaseStatus, FileEvaluationCasesBasis,
    MAX_EVALUATION_CASE_PAGE,
};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
};
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::round::Verdict;
use fixture::*;
use std::fs;

fn persisted(root: &Directory) -> Vec<u8> {
    fs::read(root.store().join("delivery.bin")).unwrap()
}

fn rounds(page: &FileEvaluationCasePage) -> Vec<u64> {
    page.cases().iter().map(|case| case.round()).collect()
}

#[test]
fn original_applied_case_pages_are_ordered_complete_and_read_only() {
    let root = Directory::new();
    let mut selected = profile();
    selected.delivery.total = 1000;
    selected.delivery.max_attempts = MAX_EVALUATION_CASE_PAGE + 4;
    let (mut host, _) = FileOversight::create(root.store(), selected).unwrap();
    let evaluator = host
        .enable_credibility(host.revision(), protocol())
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    // Insert in reverse order so traversal cannot rely on allocation chronology.
    for id in (1..=MAX_EVALUATION_CASE_PAGE as u64 + 3).rev() {
        review(&mut host, id, Verdict::Hold);
    }
    label(&mut host, &evaluator, 1, 1, GroundTruth::Benign);
    label(&mut host, &evaluator, 2, 2, GroundTruth::Violation);
    label(&mut host, &evaluator, 3, 3, GroundTruth::Censored);
    let before = host.inspect();
    let image = persisted(&root);
    let report = host.credibility_report().unwrap();
    let first = evaluator
        .evaluation_cases(&host, None, None, MAX_EVALUATION_CASE_PAGE)
        .unwrap();
    assert_eq!(first.basis().journal_revision, host.revision());
    assert_eq!(first.basis().evaluation_revision, report.revision);
    assert_eq!(first.total_cases(), MAX_EVALUATION_CASE_PAGE + 3);
    assert_eq!(first.remaining_cases(), first.total_cases());
    assert_eq!(first.unapplied_rounds(), 0);
    assert_eq!(first.after_round(), None);
    assert_eq!(rounds(&first), (101..=116).collect::<Vec<_>>());
    assert_eq!(first.next_after(), Some(116));
    for case in first.cases() {
        let ticket = evaluator.evaluation_case(&host, case.round()).unwrap();
        let original = ticket.case();
        assert_eq!(case.attempt(), original.attempt);
        assert_eq!(case.policy_generation(), original.policy_generation);
        assert_eq!(case.congress_generation(), original.congress_generation);
        assert_eq!(case.evidence_root(), original.evidence_root);
        assert_eq!(
            case.status(),
            match case.round() {
                101 | 102 => FileEvaluationCaseStatus::Resolved,
                103 => FileEvaluationCaseStatus::Censored,
                _ => FileEvaluationCaseStatus::Pending,
            }
        );
    }
    let last = evaluator
        .evaluation_cases(
            &host,
            first.next_after(),
            Some(first.basis()),
            MAX_EVALUATION_CASE_PAGE,
        )
        .unwrap();
    assert_eq!(last.basis(), first.basis());
    assert_eq!(last.after_round(), Some(116));
    assert_eq!(last.total_cases(), first.total_cases());
    assert_eq!(last.remaining_cases(), 3);
    assert_eq!(rounds(&last), vec![117, 118, 119]);
    assert_eq!(last.next_after(), None);
    assert_eq!(host.inspect(), before);
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(persisted(&root), image);
    assert_eq!(host.inspect().executions, 0);
    let visible = format!("{first:?} {last:?}");
    for private in [
        "alpha",
        "beta",
        "verdicts",
        "stopped:",
        "evidence_id",
        "origin:",
        "Benign",
        "Violation",
        "actual frozen helper context",
    ] {
        assert!(!visible.contains(private), "disclosed {private}");
    }
}

#[test]
fn empty_applied_domain_preserves_the_unapplied_round_count() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    let empty = evaluator.evaluation_cases(&host, None, None, 1).unwrap();
    assert_eq!(empty.total_cases(), 0);
    assert_eq!(empty.remaining_cases(), 0);
    assert_eq!(empty.unapplied_rounds(), 0);
    assert_eq!(empty.basis().evaluation_revision, 0);
    assert!(empty.cases().is_empty());
    assert_eq!(empty.next_after(), None);
    let inputs = begin(&mut host, 1);
    let begun = evaluator.evaluation_cases(&host, None, None, 1).unwrap();
    assert_eq!(begun.total_cases(), 0);
    assert_eq!(begun.unapplied_rounds(), 1);
    assert!(begun.cases().is_empty());
    assert_eq!(begun.basis().evaluation_revision, 0);
    assert_eq!(host.credibility_report().unwrap().retained_cases, 1);
    assert_eq!(
        evaluator.evaluation_cases(&host, None, Some(empty.basis()), 1),
        Err(JournalError::Contract(Error::Stale))
    );

    ordinary::votes(&mut host, 101, Verdict::Allow);
    let changed = ordinary::inputs(inputs.action(), b"changed application evidence");
    assert!(
        host.finish_review(host.revision(), 101, Some(&changed), snapshot())
            .unwrap()
            .is_err()
    );
    let before = persisted(&root);
    let unapplied = evaluator.evaluation_cases(&host, None, None, 16).unwrap();
    assert_eq!(unapplied.total_cases(), 0);
    assert_eq!(unapplied.unapplied_rounds(), 1);
    assert_eq!(unapplied.next_after(), None);
    assert_eq!(
        evaluator.evaluation_cases(&host, Some(101), Some(unapplied.basis()), 1),
        Err(JournalError::Contract(Error::Missing))
    );
    assert_eq!(persisted(&root), before);
}

#[test]
fn a_later_lower_round_cannot_be_skipped_by_an_old_continuation() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    review(&mut host, 5, Verdict::Hold);
    review(&mut host, 8, Verdict::Hold);
    let first = evaluator.evaluation_cases(&host, None, None, 1).unwrap();
    assert_eq!(rounds(&first), vec![105]);
    assert_eq!(first.next_after(), Some(105));
    assert_eq!(
        rounds(
            &evaluator
                .evaluation_cases(&host, first.next_after(), Some(first.basis()), 1,)
                .unwrap()
        ),
        vec![108]
    );

    review(&mut host, 1, Verdict::Hold);
    let before = persisted(&root);
    assert_eq!(
        evaluator.evaluation_cases(&host, first.next_after(), Some(first.basis()), 1),
        Err(JournalError::Contract(Error::Stale))
    );
    let fresh = evaluator.evaluation_cases(&host, None, None, 2).unwrap();
    assert_eq!(rounds(&fresh), vec![101, 105]);
    assert_eq!(fresh.total_cases(), 3);
    assert_eq!(fresh.next_after(), Some(105));
    assert_eq!(
        rounds(
            &evaluator
                .evaluation_cases(&host, fresh.next_after(), Some(fresh.basis()), 2,)
                .unwrap()
        ),
        vec![108]
    );
    assert_eq!(persisted(&root), before);
}

#[test]
fn time_and_unapplied_review_events_invalidate_the_exact_journal_cut() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    review(&mut host, 1, Verdict::Hold);
    review(&mut host, 3, Verdict::Hold);
    let first = evaluator.evaluation_cases(&host, None, None, 1).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(
        host.credibility_report().unwrap().revision,
        first.basis().evaluation_revision
    );
    assert_eq!(
        evaluator.evaluation_cases(&host, first.next_after(), Some(first.basis()), 1),
        Err(JournalError::Contract(Error::Stale))
    );
    let fresh = evaluator.evaluation_cases(&host, None, None, 1).unwrap();
    assert_eq!(
        rounds(
            &evaluator
                .evaluation_cases(&host, fresh.next_after(), Some(fresh.basis()), 1,)
                .unwrap()
        ),
        vec![103]
    );
    begin(&mut host, 2);
    assert_eq!(
        host.credibility_report().unwrap().revision,
        fresh.basis().evaluation_revision
    );
    assert_eq!(
        evaluator.evaluation_cases(&host, fresh.next_after(), Some(fresh.basis()), 1),
        Err(JournalError::Contract(Error::Stale))
    );
    let current = evaluator.evaluation_cases(&host, None, None, 16).unwrap();
    assert_eq!(rounds(&current), vec![101, 103]);
    assert_eq!(current.total_cases(), 2);
    assert_eq!(current.unapplied_rounds(), 1);
}

#[test]
fn resolving_a_censored_label_requires_a_fresh_page_basis_but_exact_retries_do_not() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    for id in [1, 2] {
        review(&mut host, id, Verdict::Hold);
    }
    label(&mut host, &evaluator, 2, 7, GroundTruth::Censored);
    let first = evaluator.evaluation_cases(&host, None, None, 1).unwrap();
    let next = evaluator
        .evaluation_cases(&host, first.next_after(), Some(first.basis()), 1)
        .unwrap();
    assert_eq!(next.cases()[0].status(), FileEvaluationCaseStatus::Censored);
    assert!(!label(&mut host, &evaluator, 2, 7, GroundTruth::Censored));
    assert_eq!(
        evaluator
            .evaluation_cases(&host, first.next_after(), Some(first.basis()), 1)
            .unwrap(),
        next
    );
    assert!(label(&mut host, &evaluator, 2, 7, GroundTruth::Violation));
    let before = persisted(&root);
    assert_eq!(
        evaluator.evaluation_cases(&host, first.next_after(), Some(first.basis()), 1),
        Err(JournalError::Contract(Error::Stale))
    );
    let fresh = evaluator.evaluation_cases(&host, None, None, 16).unwrap();
    assert_eq!(
        fresh.cases()[1].status(),
        FileEvaluationCaseStatus::Resolved
    );
    assert_eq!(persisted(&root), before);
}

#[test]
fn invalid_bounds_missing_boundaries_and_each_wrong_revision_refuse_without_mutation() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    review(&mut host, 1, Verdict::Hold);
    let initial = evaluator.evaluation_cases(&host, None, None, 16).unwrap();
    let basis = initial.basis();
    let before = persisted(&root);
    for (limit, error) in [
        (0, Error::InvalidInput),
        (17, Error::Limit),
        (usize::MAX, Error::Limit),
    ] {
        assert_eq!(
            evaluator.evaluation_cases(&host, None, None, limit),
            Err(JournalError::Contract(error))
        );
    }
    for (after, expected, error) in [
        (Some(0), Some(basis), Error::InvalidInput),
        (Some(101), None, Error::InvalidInput),
        (Some(100), Some(basis), Error::Missing),
        (Some(u64::MAX), Some(basis), Error::Missing),
        (
            Some(101),
            Some(FileEvaluationCasesBasis {
                journal_revision: basis.journal_revision + 1,
                ..basis
            }),
            Error::Stale,
        ),
        (
            Some(101),
            Some(FileEvaluationCasesBasis {
                evaluation_revision: basis.evaluation_revision + 1,
                ..basis
            }),
            Error::Stale,
        ),
    ] {
        assert_eq!(
            evaluator.evaluation_cases(&host, after, expected, 1),
            Err(JournalError::Contract(error))
        );
    }
    let end = evaluator
        .evaluation_cases(&host, Some(101), Some(basis), 1)
        .unwrap();
    assert_eq!(end.after_round(), Some(101));
    assert_eq!(end.total_cases(), 1);
    assert_eq!(end.remaining_cases(), 0);
    assert!(end.cases().is_empty());
    assert_eq!(end.next_after(), None);
    assert_eq!(persisted(&root), before);
}

#[test]
fn foreign_or_pre_recovery_evaluators_cannot_list_equal_numeric_history() {
    let root = Directory::new();
    let other_root = Directory::new();
    let guards = FileGuardSet {
        stream: None,
        decoder: None,
        decoder_stop: None,
        source: None,
        identity: None,
        campaigns: None,
        credential: None,
    };
    let (mut host, roles) =
        FileOversight::create_evaluated_guarded(root.store(), profile(), &guards, None, protocol())
            .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    review(&mut host, 1, Verdict::Hold);
    let original = roles
        .evaluator
        .evaluation_cases(&host, None, None, 16)
        .unwrap();
    let (mut foreign, _, foreign_evaluator) = create(&other_root);
    review(&mut foreign, 1, Verdict::Hold);
    assert_eq!(
        rounds(
            &foreign_evaluator
                .evaluation_cases(&foreign, None, None, 16)
                .unwrap()
        ),
        rounds(&original)
    );
    let before_foreign = persisted(&other_root);
    assert_eq!(
        roles.evaluator.evaluation_cases(&foreign, None, None, 16),
        Err(JournalError::Contract(Error::Binding))
    );
    assert_eq!(persisted(&other_root), before_foreign);

    let state = host.inspect();
    let expected = FileRecoveryRequirements {
        guards,
        effective_policy: profile().delivery.policy,
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: state.revision,
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
        },
    };
    drop(host);
    let (recovered, fresh_roles) =
        FileOversight::open_evaluated_guarded(root.store(), profile(), &expected, &protocol())
            .unwrap();
    let bytes = persisted(&root);
    assert_eq!(
        roles.evaluator.evaluation_cases(&recovered, None, None, 16),
        Err(JournalError::Contract(Error::Binding))
    );
    assert_eq!(
        fresh_roles
            .evaluator
            .evaluation_cases(&recovered, None, Some(original.basis()), 16),
        Err(JournalError::Contract(Error::Stale))
    );
    let resumed = fresh_roles
        .evaluator
        .evaluation_cases(&recovered, None, None, 16)
        .unwrap();
    assert_eq!(resumed.cases(), original.cases());
    assert_eq!(resumed.total_cases(), original.total_cases());
    assert_eq!(
        resumed.basis().evaluation_revision,
        original.basis().evaluation_revision
    );
    assert!(resumed.basis().journal_revision > original.basis().journal_revision);
    assert_eq!(persisted(&root), bytes);
    assert!(!recovered.clock_ready());
}

#[test]
fn healthy_stopped_history_is_readable_but_faulted_storage_is_never_a_cached_page() {
    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    review(&mut host, 1, Verdict::Hold);
    let original = evaluator.evaluation_cases(&host, None, None, 16).unwrap();
    let control = host.inspect().control;
    host.request_stop(
        host.revision(),
        StopRequest {
            operation: 77,
            expected_control_sequence: control.sequence,
            expected_authority_epoch: control.ledger.epoch,
        },
    )
    .unwrap();
    let stopped = evaluator.evaluation_cases(&host, None, None, 16).unwrap();
    assert_eq!(stopped.cases(), original.cases());
    assert_eq!(stopped.total_cases(), original.total_cases());
    assert!(host.inspect().stop.is_some());
    assert_eq!(host.inspect().executions, 0);

    let revision = host.revision();
    let bytes = persisted(&root);
    let pending = root.store().join("delivery.pending");
    let unconfirmed = b"independently unresolved staged bytes";
    fs::write(&pending, unconfirmed).unwrap();
    assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
    assert!(host.storage_failure().is_some());
    assert_eq!(
        evaluator.evaluation_cases(&host, None, None, 16),
        Err(JournalError::Unavailable)
    );
    assert_eq!(
        evaluator.evaluation_cases(&host, Some(101), Some(stopped.basis()), 1),
        Err(JournalError::Unavailable)
    );
    assert_eq!(host.revision(), revision);
    assert_eq!(persisted(&root), bytes);
    assert_eq!(
        fs::read(&pending).unwrap().as_slice(),
        unconfirmed.as_slice()
    );
}
