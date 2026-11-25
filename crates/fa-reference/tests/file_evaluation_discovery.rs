//! Independent discovery of real applied reviews through the original live
//! evaluator. These fixtures exercise custody and accounting, not label truth.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/file_credibility.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::{
    FileIndependentEvaluator, MAX_PENDING_EVALUATION_PAGE, PendingEvaluationPage,
    PendingEvaluationStatus,
};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    evaluation::FileEvaluatedOversightRoles,
};
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, FileOversightProfile,
};
use fa_reference::action::consequence::delivery::persistent::{JournalError, PolicyUpdate};
use fa_reference::action::consequence::gate::containment::session::policy::Policy;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::round::Verdict;
use fixture::{Directory, ordinary};

fn profile() -> FileOversightProfile {
    let mut profile = fixture::profile();
    // Select a finite inventory large enough to cross the fixed page boundary.
    // No native limit or production profile is enlarged by this fixture.
    profile.delivery.max_attempts = 32;
    profile.delivery.total = 1024;
    profile
}

fn guards() -> FileGuardSet {
    FileGuardSet {
        stream: None,
        decoder: None,
        decoder_stop: None,
        source: None,
        identity: None,
        campaigns: None,
        credential: None,
    }
}

fn create(root: &Directory) -> (FileOversight, FileEvaluatedOversightRoles) {
    let (mut host, roles) = FileOversight::create_evaluated_guarded(
        root.store(),
        profile(),
        &guards(),
        None,
        fixture::protocol(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles)
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn discover(
    root: &Directory,
    host: &FileOversight,
    evaluator: &FileIndependentEvaluator,
    continuation: Option<(u64, u64)>,
) -> PendingEvaluationPage {
    let before = host.inspect();
    let report = host.credibility_report().unwrap();
    let canonical = image(root);
    let page = evaluator
        .pending_evaluation_cases(host, continuation)
        .unwrap();
    assert_eq!(page.evaluation_revision(), report.revision);
    assert_eq!(host.inspect(), before);
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(image(root), canonical);
    page
}

fn cursor(page: &PendingEvaluationPage) -> (u64, u64) {
    (page.evaluation_revision(), page.next_after_round().unwrap())
}

fn rounds(page: &PendingEvaluationPage) -> Vec<u64> {
    page.cases().iter().map(|case| case.round()).collect()
}

#[test]
fn unapplied_reviews_remain_explicit_and_discovery_never_supplies_the_human_key() {
    let root = Directory::new();
    let (mut host, roles) = create(&root);
    let empty = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(empty.evaluation_revision(), 0);
    assert!(empty.cases().is_empty());
    assert_eq!(empty.total_pending(), 0);
    assert_eq!(empty.unfinished_reviews(), 0);
    assert_eq!(empty.after_round(), None);
    assert_eq!(empty.next_after_round(), None);

    let inputs = fixture::begin(&mut host, 1);
    let incomplete = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(incomplete.evaluation_revision(), 0);
    assert!(incomplete.cases().is_empty());
    assert_eq!(incomplete.total_pending(), 0);
    assert_eq!(incomplete.unfinished_reviews(), 1);
    assert_eq!(incomplete.next_after_round(), None);
    assert_eq!(
        roles.evaluator.evaluation_case(&host, 101).err(),
        Some(JournalError::Contract(Error::Missing))
    );

    ordinary::votes(&mut host, 101, Verdict::Allow);
    host.finish_review(host.revision(), 101, Some(&inputs), fixture::snapshot())
        .unwrap()
        .unwrap();
    let applied = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(rounds(&applied), vec![101]);
    assert_eq!(applied.total_pending(), 1);
    assert_eq!(applied.unfinished_reviews(), 0);
    assert_eq!(
        applied.cases()[0].status(),
        PendingEvaluationStatus::Unassessed
    );
    let original = roles.evaluator.evaluation_case(&host, 101).unwrap();
    let identity = &applied.cases()[0];
    assert_eq!(identity.attempt(), original.case().attempt);
    assert_eq!(
        identity.policy_generation(),
        original.case().policy_generation
    );
    assert_eq!(
        identity.congress_generation(),
        original.case().congress_generation
    );
    assert_eq!(identity.evidence_root(), original.case().evidence_root);

    let automatic = host
        .authorize(host.revision(), 1, &inputs, fixture::snapshot())
        .unwrap();
    assert!(
        host.publish_checked(
            host.revision(),
            1,
            Some(&inputs),
            fixture::snapshot(),
            ElapsedTick(1)
        )
        .is_err()
    );
    assert_eq!(host.inspect().executions, 0);
    let request = host
        .request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31))
        .unwrap();
    let revision = host.revision();
    let human = roles
        .oversight
        .human
        .approve(&mut host, revision, &request)
        .unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        inputs.action(),
        &inputs,
        fixture::snapshot(),
    )
    .unwrap();
    assert_eq!(
        host.publish_checked(
            host.revision(),
            1,
            Some(&inputs),
            fixture::snapshot(),
            ElapsedTick(1)
        )
        .unwrap()
        .outcome,
        EndpointOutcome::Executed {
            resulting_version: 2
        }
    );
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(
        rounds(&discover(&root, &host, &roles.evaluator, None)),
        vec![101]
    );
}

#[test]
fn bounded_pages_enumerate_pending_and_censored_cases_without_final_labels_or_gaps() {
    let root = Directory::new();
    let (mut host, roles) = create(&root);
    // Insertion order is deliberately different from the declared round order.
    for id in (1..=19).rev() {
        fixture::review(&mut host, id, Verdict::Hold);
    }
    fixture::label(&mut host, &roles.evaluator, 3, 3, GroundTruth::Violation);
    fixture::label(&mut host, &roles.evaluator, 18, 18, GroundTruth::Benign);
    fixture::label(&mut host, &roles.evaluator, 5, 5, GroundTruth::Censored);
    let first = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(MAX_PENDING_EVALUATION_PAGE, 16);
    assert_eq!(first.cases().len(), MAX_PENDING_EVALUATION_PAGE);
    assert_eq!(first.total_pending(), 17);
    assert_eq!(first.unfinished_reviews(), 0);
    assert_eq!(first.next_after_round(), Some(117));
    assert_eq!(
        first
            .cases()
            .iter()
            .filter(|case| case.status() == PendingEvaluationStatus::Censored)
            .map(|case| case.round())
            .collect::<Vec<_>>(),
        vec![105]
    );
    let last = discover(&root, &host, &roles.evaluator, Some(cursor(&first)));
    assert_eq!(last.after_round(), Some(117));
    assert_eq!(last.total_pending(), 17);
    assert_eq!(rounds(&last), vec![119]);
    assert_eq!(last.next_after_round(), None);
    let mut all = rounds(&first);
    all.extend(rounds(&last));
    assert_eq!(
        all,
        (101..=119)
            .filter(|round| ![103, 118].contains(round))
            .collect::<Vec<_>>()
    );
    let end = discover(
        &root,
        &host,
        &roles.evaluator,
        Some((last.evaluation_revision(), 119)),
    );
    assert_eq!(end.after_round(), Some(119));
    assert!(end.cases().is_empty());
    assert_eq!(end.total_pending(), 17);
    assert_eq!(end.next_after_round(), None);
}

#[test]
fn continuation_refuses_every_changed_evaluation_cut_but_not_time_or_exact_label_retry() {
    let root = Directory::new();
    let (mut host, roles) = create(&root);
    for id in 2..=18 {
        fixture::review(&mut host, id, Verdict::Hold);
    }
    let first = discover(&root, &host, &roles.evaluator, None);
    let original = cursor(&first);
    // A new case before the cursor must never disappear from a continued page.
    fixture::review(&mut host, 1, Verdict::Hold);
    assert_eq!(
        roles
            .evaluator
            .pending_evaluation_cases(&host, Some(original)),
        Err(JournalError::Contract(Error::Stale))
    );
    let fresh = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(fresh.cases()[0].round(), 101);
    let before_censoring = cursor(&fresh);
    fixture::label(&mut host, &roles.evaluator, 2, 2, GroundTruth::Censored);
    assert_eq!(
        roles
            .evaluator
            .pending_evaluation_cases(&host, Some(before_censoring)),
        Err(JournalError::Contract(Error::Stale))
    );
    let censored = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(
        censored.cases()[1].status(),
        PendingEvaluationStatus::Censored
    );
    let before_final = cursor(&censored);
    fixture::label(&mut host, &roles.evaluator, 2, 2, GroundTruth::Violation);
    assert_eq!(
        roles
            .evaluator
            .pending_evaluation_cases(&host, Some(before_final)),
        Err(JournalError::Contract(Error::Stale))
    );
    let final_cut = discover(&root, &host, &roles.evaluator, None);
    let unchanged = cursor(&final_cut);
    assert!(!fixture::label(
        &mut host,
        &roles.evaluator,
        2,
        2,
        GroundTruth::Violation
    ));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let continued = discover(&root, &host, &roles.evaluator, Some(unchanged));
    assert_eq!(
        continued.evaluation_revision(),
        final_cut.evaluation_revision()
    );
    assert_eq!(rounds(&continued), vec![118]);
    let _inputs = fixture::begin(&mut host, 19);
    let unfinished = discover(&root, &host, &roles.evaluator, Some(unchanged));
    assert_eq!(rounds(&unfinished), vec![118]);
    assert_eq!(unfinished.unfinished_reviews(), 1);
    assert_eq!(unfinished.total_pending(), 17);
}

#[test]
fn foreign_faulted_or_invalid_cursor_reads_cannot_reuse_a_saved_original_inventory() {
    let root = Directory::new();
    let other_root = Directory::new();
    let (mut host, roles) = create(&root);
    let (mut other, other_roles) = create(&other_root);
    fixture::review(&mut host, 1, Verdict::Hold);
    fixture::review(&mut other, 1, Verdict::Hold);
    let own = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(
        own,
        discover(&other_root, &other, &other_roles.evaluator, None)
    );
    let canonical = image(&root);
    assert_eq!(
        other_roles.evaluator.pending_evaluation_cases(&host, None),
        Err(JournalError::Contract(Error::Binding))
    );
    // Brand mismatch precedes even a malformed/stale continuation.
    assert_eq!(
        other_roles
            .evaluator
            .pending_evaluation_cases(&host, Some((u64::MAX, 0))),
        Err(JournalError::Contract(Error::Binding))
    );
    assert_eq!(
        roles
            .evaluator
            .pending_evaluation_cases(&host, Some((own.evaluation_revision(), 0))),
        Err(JournalError::Contract(Error::InvalidInput))
    );
    assert_eq!(
        roles
            .evaluator
            .pending_evaluation_cases(&host, Some((own.evaluation_revision(), 999))),
        Err(JournalError::Contract(Error::Binding))
    );
    assert_eq!(image(&root), canonical);
    fixture::label(&mut host, &roles.evaluator, 1, 1, GroundTruth::Violation);
    let revision = host.credibility_report().unwrap().revision;
    assert_eq!(
        roles
            .evaluator
            .pending_evaluation_cases(&host, Some((revision, 101))),
        Err(JournalError::Contract(Error::Binding))
    );
    assert!(
        discover(&root, &host, &roles.evaluator, None)
            .cases()
            .is_empty()
    );
    let canonical = image(&root);
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err());
    assert_eq!(
        roles.evaluator.pending_evaluation_cases(&host, None),
        Err(JournalError::Unavailable)
    );
    assert_eq!(image(&root), canonical);
}

#[test]
fn guarded_recovery_reissues_custody_and_keeps_pending_older_policy_cases_discoverable() {
    let root = Directory::new();
    let (mut host, roles) = create(&root);
    fixture::review(&mut host, 1, Verdict::Hold);
    fixture::review(&mut host, 2, Verdict::Allow);
    fixture::label(&mut host, &roles.evaluator, 2, 2, GroundTruth::Censored);
    let control = host.inspect().control;
    let update = PolicyUpdate::new(
        7,
        control.sequence,
        control.ledger.epoch,
        Policy::new(2, host.current_policy().unwrap().nodes().to_vec()).unwrap(),
    )
    .unwrap();
    host.replace_policy(host.revision(), &update).unwrap();
    fixture::review(&mut host, 3, Verdict::Allow);
    assert_eq!(host.credibility_report().unwrap().scoped_cases, 1);
    let original = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(rounds(&original), vec![101, 102, 103]);
    assert_eq!(
        original
            .cases()
            .iter()
            .map(|case| case.policy_generation())
            .collect::<Vec<_>>(),
        vec![1, 1, 2]
    );
    let state = host.inspect();
    let expected = FileRecoveryRequirements {
        guards: guards(),
        effective_policy: host.current_policy().unwrap().clone(),
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: state.revision,
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
        },
    };
    drop(host);
    let (mut host, fresh) = FileOversight::open_evaluated_guarded(
        root.store(),
        profile(),
        &expected,
        &fixture::protocol(),
    )
    .unwrap();
    assert!(!host.clock_ready());
    assert_eq!(
        roles.evaluator.pending_evaluation_cases(&host, None),
        Err(JournalError::Contract(Error::Binding))
    );
    assert_eq!(discover(&root, &host, &fresh.evaluator, None), original);
    assert!(!host.clock_ready());
    // Discovery gives the fresh independent evaluator the original case to
    // resolve, without reconstructing old live roles or refreshing a clock.
    fixture::label(&mut host, &fresh.evaluator, 2, 2, GroundTruth::Benign);
    assert_eq!(
        rounds(&discover(&root, &host, &fresh.evaluator, None)),
        vec![101, 103]
    );
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn terminal_round_identity_has_a_complete_empty_suffix_without_counter_overflow() {
    let root = Directory::new();
    let (mut host, roles) = create(&root);
    fixture::review(&mut host, u64::MAX - 100, Verdict::Hold);
    let first = discover(&root, &host, &roles.evaluator, None);
    assert_eq!(rounds(&first), vec![u64::MAX]);
    assert_eq!(first.next_after_round(), None);
    let end = discover(
        &root,
        &host,
        &roles.evaluator,
        Some((first.evaluation_revision(), u64::MAX)),
    );
    assert!(end.cases().is_empty());
    assert_eq!(end.total_pending(), 1);
    assert_eq!(end.next_after_round(), None);
}
