//! Real file acquisition around native two-key dispatch, with one sink cut.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::FileCaptureError;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::completion::CapturedCompletionKeys;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::Error;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn keys(keys: &Keys) -> CapturedCompletionKeys<'_> {
    CapturedCompletionKeys { automatic: &keys.automatic, human: &keys.human, credential: None }
}
fn evidence(keys: &Keys) -> Result<DriverEvidence, Error> {
    Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(keys.inputs.clone()) })
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

#[test]
fn two_fresh_reads_complete_one_effect_and_replay_the_same_settled_cut() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let ready = source_keys(&mut host, &reviewer, &root, 1);
    let before = host.revision();
    let mut samples = 0;
    let report = host.complete_publication_from_source(before, keys(&ready), &source(&root), || ElapsedTick(2), |_, _| {
        samples += 1;
        let visible = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(visible.executions, 0);
        assert_eq!(visible.control.ledger.stages[&1], ActionState::Authorized);
        assert_eq!(visible.control.ledger.reserved, 16);
        assert_eq!(visible.control.ledger.charged, 0);
        evidence(&ready)
    });
    assert_eq!(samples, 2);
    assert_eq!(report.reads, vec![Ok(packet(1, &ready.action, &ready.inputs, 1, &[0, 2, 4]).identity()); 2]);
    assert_eq!(report.evidence_failure, None);
    let publication = report.result.unwrap();
    assert_eq!(publication.basis, PublicationBasis::Revalidated);
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.revision(), before + 8);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().executions, 1);
    assert!(!host.publication_source(1).unwrap().unwrap().fresh);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(publication.outcome));
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn phantoms_seen_after_staged_dispatch_seal_instead_of_rebasing_original_requirements() {
    for rows in [vec![2, 4], vec![0, 1, 2, 4], vec![0, 2, 4, 7], vec![0, 2, 3, 4], vec![0, 2, 4, 99]] {
        let root = Directory::new();
        let (mut host, reviewer) = source_host(&root);
        let ready = source_keys(&mut host, &reviewer, &root, 1);
        let mut samples = 0;
        let report = host.complete_publication_from_source(host.revision(), keys(&ready), &source(&root), || ElapsedTick(2), |_, _| {
            samples += 1;
            if samples == 2 { replace_source(&root, &packet(1, &ready.action, &ready.inputs, 2, &rows)); }
            evidence(&ready)
        });
        assert_eq!(samples, 2);
        let publication = report.result.unwrap();
        let permitted = rows.contains(&99);
        assert_eq!(publication.basis, if permitted { PublicationBasis::Revalidated } else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(publication.outcome, if permitted { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(host.inspect().executions, u64::from(permitted));
        assert_eq!(host.inspect().control.ledger.charged, if permitted { 16 } else { 0 });
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.publication_source(1).unwrap().unwrap().generation, 2);
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
        let repeated = host.complete_publication_from_source(host.revision(), keys(&ready), &source(&root),
            || panic!("completed keys cannot sample a clock"), |_, _| panic!("completed keys cannot reacquire"));
        assert!(repeated.reads.is_empty());
        assert_eq!(repeated.result, Err(JournalError::Contract(Error::WrongState)));
    }
}

#[test]
fn first_read_loss_retains_both_original_keys_and_a_real_retry_can_succeed() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let ready = source_keys(&mut host, &reviewer, &root, 1);
    std::fs::remove_file(root.0.join("witness-input.bin")).unwrap();
    let before = host.revision();
    let report = host.complete_publication_from_source(before, keys(&ready), &source(&root),
        || panic!("failed first read cannot reach dispatch clock"), |_, _| evidence(&ready));
    assert_eq!(report.reads, vec![Err(FileCaptureError::Io(std::io::ErrorKind::NotFound))]);
    assert_eq!(report.result, Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), before + 1);
    assert!(host.storage_failure().is_none());
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    replace_source(&root, &packet(1, &ready.action, &ready.inputs, 1, &[0, 2, 4]));
    let retry = host.complete_publication_from_source(host.revision(), keys(&ready), &source(&root), || ElapsedTick(2), |_, _| evidence(&ready));
    assert_eq!(retry.result.unwrap().basis, PublicationBasis::Revalidated);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn second_read_or_provider_loss_produces_native_nonexecution_and_settlement_together() {
    for provider_loss in [false, true] {
        let root = Directory::new();
        let (mut host, reviewer) = source_host(&root);
        let ready = source_keys(&mut host, &reviewer, &root, 1);
        let mut samples = 0;
        let report = host.complete_publication_from_source(host.revision(), keys(&ready), &source(&root), || ElapsedTick(2), |_, _| {
            samples += 1;
            if samples == 2 {
                if provider_loss { return Err(Error::Incomplete); }
                std::fs::remove_file(root.0.join("witness-input.bin")).unwrap();
            }
            evidence(&ready)
        });
        assert_eq!(report.evidence_failure, if provider_loss { Some(Error::Incomplete) } else { None });
        assert_eq!(report.reads.len(), if provider_loss { 1 } else { 2 });
        let publication = report.result.unwrap();
        assert_eq!(publication.basis, PublicationBasis::Rejected(Error::Incomplete));
        assert_eq!(publication.outcome, sealed());
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn caught_second_provider_panic_cannot_expose_speculative_dispatch_or_old_eligibility() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let ready = source_keys(&mut host, &reviewer, &root, 1);
    let before = host.revision();
    let mut samples = 0;
    assert!(catch_unwind(AssertUnwindSafe(|| {
        host.complete_publication_from_source(before, keys(&ready), &source(&root), || ElapsedTick(2), |_, _| {
            samples += 1;
            assert!(samples != 2, "provider interrupted after staged dispatch");
            evidence(&ready)
        })
    })).is_err());
    assert_eq!(samples, 2);
    assert!(host.storage_failure().is_some());
    assert_eq!(host.revision(), before + 1);
    let published = FileOversight::read_publication(root.store(), &profile()).unwrap();
    assert_eq!(published.executions, 0);
    assert_eq!(published.control.ledger.stages[&1], ActionState::Authorized);
    assert_eq!(published.control.ledger.reserved, 16);
    assert_eq!(host.authorize(host.revision(), 1, &ready.inputs, snapshot()).unwrap_err(), JournalError::Unavailable);
    drop(host); drop(reviewer);
    let (host, _) = FileOversight::open_with_publication_validation(root.store(), profile(), limits()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn producer_equivocation_refuses_installation_without_returning_a_fabricated_seal() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let ready = source_keys(&mut host, &reviewer, &root, 1);
    let before = host.revision();
    let mut samples = 0;
    let report = host.complete_publication_from_source(before, keys(&ready), &source(&root), || ElapsedTick(2), |_, _| {
        samples += 1;
        if samples == 2 { replace_source(&root, &packet(1, &ready.action, &ready.inputs, 1, &[0, 1, 2, 4])); }
        evidence(&ready)
    });
    assert_eq!(report.reads.len(), 2);
    assert!(report.reads.iter().all(Result::is_ok));
    assert_eq!(report.result, Err(JournalError::Contract(Error::Binding)));
    assert!(host.storage_failure().is_some());
    assert_eq!(host.revision(), before + 1);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap().executions, 0);
    assert_eq!(host.inspect().control.ledger.reserved, 16);
}

#[test]
fn second_acquisition_duration_cannot_extend_human_execution_deadline() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let ready = source_keys(&mut host, &reviewer, &root, 1);
    let calls = Cell::new(0);
    let report = host.complete_publication_from_source(host.revision(), keys(&ready), &source(&root), || {
        calls.set(calls.get() + 1);
        ElapsedTick(if calls.get() == 1 { 2 } else { 31 })
    }, |_, _| evidence(&ready));
    assert_eq!(calls.get(), 2);
    let result = report.result.unwrap();
    assert_eq!(result.basis, PublicationBasis::DeadlineElapsed);
    assert_eq!(result.outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed });
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn final_replacement_failure_exposes_only_the_pre_read_withdrawal_and_requires_recovery() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let ready = source_keys(&mut host, &reviewer, &root, 1);
    let before = host.revision();
    let calls = Cell::new(0);
    let report = host.complete_publication_from_source(before, keys(&ready), &source(&root), || {
        calls.set(calls.get() + 1);
        if calls.get() == 2 { std::fs::write(root.store().join("delivery.pending"), b"inert stage obstruction").unwrap(); }
        ElapsedTick(2)
    }, |_, _| evidence(&ready));
    assert_eq!(report.reads.len(), 2);
    assert!(matches!(report.result, Err(JournalError::Io(_))));
    assert!(host.storage_failure().is_some());
    assert_eq!(host.revision(), before + 1);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap().executions, 0);
    drop(host); drop(reviewer);
    let (host, _) = FileOversight::open_with_publication_validation(root.store(), profile(), limits()).unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn foreign_keys_and_full_event_capacity_refuse_before_observation_or_withdrawal() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    let ready = source_keys(&mut host, &reviewer, &root, 1);
    let other_root = Directory::new();
    let (mut other, other_reviewer) = source_host(&other_root);
    let other_ready = source_keys(&mut other, &other_reviewer, &other_root, 1);
    let before = host.inspect();
    let foreign = host.complete_publication_from_source(host.revision(), CapturedCompletionKeys {
        automatic: &ready.automatic, human: &other_ready.human, credential: None,
    }, &source(&root), || panic!("foreign keys cannot read time"), |_, _| panic!("foreign keys cannot read evidence"));
    assert_eq!(foreign.result, Err(JournalError::Contract(Error::Binding)));
    assert!(foreign.reads.is_empty());
    assert_eq!(host.inspect(), before);
    let small_root = Directory::new();
    let mut p = profile(); p.delivery.limits.events = before.revision as usize + 7;
    let (mut small, small_reviewer) = FileOversight::create_with_publication_validation(small_root.store(), p, limits()).unwrap();
    small.observe_time(small.revision(), ElapsedTick(1)).unwrap();
    let small_ready = source_keys(&mut small, &small_reviewer, &small_root, 1);
    let before = small.inspect();
    let refused = small.complete_publication_from_source(small.revision(), keys(&small_ready), &source(&small_root),
        || panic!("capacity refused before time"), |_, _| panic!("capacity refused before capture"));
    assert_eq!(refused.result, Err(JournalError::Contract(Error::Limit)));
    assert!(refused.reads.is_empty());
    assert_eq!(small.inspect(), before);
}
