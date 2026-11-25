//! Select original accounted actor requests before any recovery Fence.
//! Native votes exercise composition; they do not establish evaluator accuracy.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/file_credibility.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::{
    FileEvaluatedOversightRoles,
    andon::{FileAndonRecovery, FileAndonRecoveryRequirements},
};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::recovery::FileGuardedRecoveryStatus;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
};
use fa_reference::action::consequence::delivery::persistent::requests::{
    FileRequestDisposition, FileRequestStatus,
};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};
use fixture::{Directory, ordinary, profile, protocol, snapshot};
use std::fs;

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

fn accounting() -> FileAndonConfig {
    FileAndonConfig::new(
        LedgerIdentity {
            deployment: 101,
            evaluator: 202,
        },
        vec![AndonBudget {
            window: 1,
            start: 0,
            end: 100,
            false_stops_allowed: 0,
            cost_per_stop: 7,
        }],
    )
    .unwrap()
}

fn owner(root: &Directory) -> (FileOversight, FileEvaluatedOversightRoles) {
    let (mut host, roles) = FileOversight::create_andon_guarded_reserved(
        root.store(),
        profile(),
        &guards(),
        None,
        protocol(),
        &accounting(),
        RecoveryReserve::terminal(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles)
}

fn requirements(host: &FileOversight) -> FileAndonRecoveryRequirements {
    let state = host.inspect();
    FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: guards(),
            effective_policy: profile().delivery.policy,
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: state.revision,
                control_sequence: state.control.sequence,
                authority_epoch: state.control.ledger.epoch,
            },
        },
        accounting(),
        accounting().windows().to_vec(),
        host.credibility_report().unwrap().revision,
    )
    .unwrap()
}

fn attempt(status: FileRequestStatus) -> u64 {
    match status.disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("expected an original admitted request: {other:?}"),
    }
}

fn reviewed_request(
    host: &mut FileOversight,
    request: u64,
    verdict: Verdict,
) -> (u64, CommitteeInput) {
    let spec = ordinary::spec(host, b"original accounted publication");
    let status = host
        .submit_request(host.revision(), request, spec, snapshot())
        .unwrap();
    let id = attempt(status);
    let input = ordinary::inputs(
        host.request_action(request).unwrap(),
        b"original frozen evidence",
    );
    host.record_inputs(host.revision(), id, 0, input.clone())
        .unwrap();
    host.begin_review(
        host.revision(),
        id,
        id + 100,
        ordinary::ROOT,
        ordinary::window(host),
        snapshot(),
    )
    .unwrap();
    ordinary::votes(host, id + 100, verdict);
    host.finish_review(host.revision(), id + 100, Some(&input), snapshot())
        .unwrap()
        .unwrap();
    (id, input)
}

fn ready(run: &mut FileAndonRecovery) {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
}

#[test]
fn original_held_allowed_and_refused_requests_require_the_complete_replayed_cut() {
    let root = Directory::new();
    let (mut host, _) = owner(&root);
    reviewed_request(&mut host, 700, Verdict::Hold);
    reviewed_request(&mut host, 701, Verdict::Allow);
    let mut rejected = ordinary::spec(&host, b"unfunded");
    rejected.units = 101;
    let refusal = host
        .submit_request(host.revision(), 702, rejected, snapshot())
        .unwrap();
    assert_eq!(
        refusal.disposition,
        FileRequestDisposition::NotAdmitted(Error::Limit)
    );
    let original = [700, 701, 702].map(|id| host.request_status(id).unwrap());
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    let canonical = fs::read(root.store().join("delivery.bin")).unwrap();
    drop(host);
    let pending = root.store().join("delivery.pending");
    fs::write(&pending, b"keep unacknowledged staging").unwrap();
    let mut run = FileOversight::begin_open_andon_guarded(
        root.store(),
        profile(),
        &expected,
        &protocol(),
        &anchor,
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap();
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let before = run.progress();
        for request in [700, 701, 702, 703] {
            assert_eq!(
                run.recorded_request(request),
                Err(JournalError::Contract(Error::Incomplete))
            );
        }
        assert_eq!(run.progress(), before);
        run.advance(before.replayed_events, 1).unwrap();
        assert_eq!(
            fs::read(root.store().join("delivery.bin")).unwrap(),
            canonical
        );
        assert_eq!(fs::read(&pending).unwrap(), b"keep unacknowledged staging");
    }
    let before = run.progress();
    for expected in original {
        assert_eq!(run.recorded_request(expected.request).unwrap(), expected);
        assert_eq!(run.recorded_request(expected.request).unwrap(), expected);
    }
    for missing in [0, 703, u64::MAX] {
        assert_eq!(
            run.recorded_request(missing),
            Err(JournalError::Contract(Error::Missing))
        );
    }
    assert_eq!(run.progress(), before);
    assert_eq!(run.cancel().status, FileGuardedRecoveryStatus::Cancelled);
    assert_eq!(
        fs::read(root.store().join("delivery.bin")).unwrap(),
        canonical
    );
    assert_eq!(fs::read(pending).unwrap(), b"keep unacknowledged staging");
}

#[test]
fn selected_unknown_stays_charged_while_a_delayed_original_hold_is_independently_labeled() {
    let root = Directory::new();
    let (mut host, old_roles) = owner(&root);
    let (held, _) = reviewed_request(&mut host, 700, Verdict::Hold);
    let (sent, input) = reviewed_request(&mut host, 701, Verdict::Allow);
    let action = host.request_action(701).unwrap().clone();
    let automatic = host
        .authorize(host.revision(), sent, &input, snapshot())
        .unwrap();
    let request = host
        .request_human_approval(host.revision(), 1701, sent, &input, ElapsedTick(31))
        .unwrap();
    let revision = host.revision();
    let human = old_roles
        .oversight
        .human
        .approve(&mut host, revision, &request)
        .unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &input,
        snapshot(),
    )
    .unwrap();
    // A real original Fence records the lost outcome; no fabricated snapshot or
    // reconciliation is used to create the Unknown fixture.
    host.fence(host.revision()).unwrap();
    let unknown = host.request_status(701).unwrap();
    assert_eq!(
        unknown.disposition,
        FileRequestDisposition::Admitted {
            attempt: sent,
            stage: ActionState::Unknown
        }
    );
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    let state = host.inspect();
    let accounting_before = host.andon_ledger().unwrap().clone();
    let capacity = host.journal_capacity().unwrap();
    assert_eq!(state.control.ledger.charged, 16);
    assert_eq!(state.executions, 0);
    drop(host);
    let mut run = FileOversight::begin_open_andon_guarded(
        root.store(),
        profile(),
        &expected,
        &protocol(),
        &anchor,
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap();
    ready(&mut run);
    assert_eq!(run.recorded_request(701).unwrap(), unknown);
    assert_eq!(attempt(run.recorded_request(700).unwrap()), held);
    let (mut host, roles) = run.finish().unwrap();
    assert_eq!(host.revision(), state.revision + 1);
    assert!(!host.clock_ready());
    assert_eq!(host.request_status(701).unwrap(), unknown);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().payload, state.payload);
    assert_eq!(host.inspect().executions, state.executions);
    assert_eq!(host.andon_ledger().unwrap(), &accounting_before);
    let after_capacity = host.journal_capacity().unwrap();
    assert_eq!(after_capacity.reserve(), Some(RecoveryReserve::terminal()));
    assert_eq!(
        after_capacity.remaining().events + 1,
        capacity.remaining().events
    );
    let ticket = host.evaluation_ticket(held + 100).unwrap();
    let label = fixture::assessment(19, GroundTruth::Benign);
    let revision = host.revision();
    assert_eq!(
        old_roles
            .evaluator
            .assess(&mut host, revision, &ticket, label),
        Err(JournalError::Contract(Error::Binding))
    );
    assert_eq!(
        roles.evaluator.assess(&mut host, revision, &ticket, label),
        Err(JournalError::Contract(Error::Incomplete))
    );
    assert_eq!(host.revision(), revision);
    host.observe_time(host.revision(), ElapsedTick(5)).unwrap();
    let before_label = host.inspect();
    let revision = host.revision();
    assert!(
        roles
            .evaluator
            .assess(&mut host, revision, &ticket, label)
            .unwrap()
    );
    assert_eq!(host.inspect().control, before_label.control);
    assert_eq!(host.request_status(701).unwrap(), unknown);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].total_cost, 7);
    let revision = host.revision();
    assert!(
        !roles
            .evaluator
            .assess(&mut host, revision, &ticket, label)
            .unwrap()
    );
    assert_eq!(host.revision(), revision);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn failed_independent_floor_never_exposes_an_existing_request_or_changes_staging() {
    let root = Directory::new();
    let (mut host, _) = owner(&root);
    reviewed_request(&mut host, 700, Verdict::Hold);
    let expected = requirements(&host);
    let anchor = host.history_anchor().unwrap();
    let canonical = fs::read(root.store().join("delivery.bin")).unwrap();
    let wrong = FileAndonRecoveryRequirements::new(
        expected.base().clone(),
        expected.config().clone(),
        expected.windows().to_vec(),
        expected.minimum_evaluation_revision() + 1,
    )
    .unwrap();
    drop(host);
    let pending = root.store().join("delivery.pending");
    fs::write(&pending, b"preserve failed-floor evidence").unwrap();
    let mut run = FileOversight::begin_open_andon_guarded(
        root.store(),
        profile(),
        &wrong,
        &protocol(),
        &anchor,
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap();
    let failure = loop {
        let progress = run.progress();
        if let Err(error) = run.advance(progress.replayed_events, 1) {
            break error;
        }
    };
    assert_eq!(failure, JournalError::Contract(Error::Stale));
    let before = run.progress();
    assert_eq!(
        before.status,
        FileGuardedRecoveryStatus::Failed(Error::Stale)
    );
    assert_eq!(
        run.recorded_request(700),
        Err(JournalError::Contract(Error::Stale))
    );
    assert_eq!(
        run.recorded_request(701),
        Err(JournalError::Contract(Error::Stale))
    );
    assert_eq!(run.progress(), before);
    drop(run);
    assert_eq!(
        fs::read(root.store().join("delivery.bin")).unwrap(),
        canonical
    );
    assert_eq!(
        fs::read(pending).unwrap(),
        b"preserve failed-floor evidence"
    );
    // The same untouched cut remains usable with the original independent floor.
    let mut control = FileOversight::begin_open_andon_guarded(
        root.store(),
        profile(),
        &expected,
        &protocol(),
        &anchor,
    )
    .unwrap();
    ready(&mut control);
    assert_eq!(control.recorded_request(700).unwrap().request, 700);
}
