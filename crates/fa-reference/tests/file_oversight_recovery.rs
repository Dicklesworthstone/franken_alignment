//! Recovery keeps native obligations, not sendable permissions or helper authority.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::persistent::{FileDelivery, JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::*;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason, StopRequest};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

#[test]
fn recovery_revokes_unspent_human_keys_but_retains_dispatched_liabilities_and_execution_receipts() {
    for mode in ["approved", "dispatched", "published"] {
        let root = Directory::new();
        let (mut host, old_reviewer) = create(&root);
        let keys = ready(&mut host, &old_reviewer, 1, b"one publication");
        if mode != "approved" { dispatch(&mut host, &keys); }
        if mode == "published" { host.publish(host.revision(), 1).unwrap(); }
        drop(host);
        let (mut host, new_reviewer) = FileOversight::open(root.store(), profile()).unwrap();
        assert!(!host.clock_ready());
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        let revision = host.revision();
        assert_eq!(old_reviewer.approve(&mut host, revision, &keys.request).unwrap_err(), JournalError::Contract(Error::Binding));
        assert_eq!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap_err(),
            JournalError::Contract(Error::Binding));
        let recovered_request = host.human_request(1001).unwrap();
        assert_eq!(recovered_request.evidence().inputs(), &keys.inputs);
        if mode == "approved" {
            assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
            assert_eq!(host.inspect().control.ledger.available, 100);
            assert_eq!(host.inspect().control.ledger.charged, 0);
        } else {
            assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
            assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
            assert_eq!(host.inspect().control.ledger.available, 84);
            assert_eq!(host.inspect().control.ledger.charged, 16);
        }
        assert_eq!(host.reconcile_pending(host.revision()).unwrap_err(), JournalError::Contract(Error::Incomplete));
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let revision = host.revision();
        assert!(new_reviewer.approve(&mut host, revision, &recovered_request).is_err());
        let results = host.reconcile_pending(host.revision()).unwrap();
        match mode {
            "approved" => assert!(results.is_empty()),
            "dispatched" => {
                assert_eq!(results[&1], Ok(Reconciliation::AwaitingResolution));
                assert_eq!(host.inspect().control.ledger.charged, 16);
                assert_eq!(host.seal_unexecuted(host.revision(), 1).unwrap(), Reconciliation::Resolved(
                    EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
                assert_eq!(host.inspect().control.ledger.available, 100);
            }
            "published" => {
                assert_eq!(results[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
                assert_eq!(host.inspect().payload, b"one publication");
                assert_eq!(host.inspect().control.ledger.charged, 16);
            }
            _ => unreachable!(),
        }
        assert!(host.publish(host.revision(), 1).is_err());
        assert_eq!(host.inspect().executions, u64::from(mode == "published"));
    }
}

#[test]
fn an_interrupted_helper_round_is_not_resumed_but_a_new_epoch_can_review_new_work() {
    let root = Directory::new();
    let (mut host, _) = create(&root);
    let action = host.propose(host.revision(), 1, spec(&host, b"interrupted"), snapshot()).unwrap();
    let input = inputs(&action, b"complete source");
    host.record_inputs(host.revision(), 1, 0, input).unwrap();
    host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).unwrap();
    commit(&mut host, 101, "alpha", Verdict::Allow);
    drop(host);
    let (mut host, reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert!(host.review_input(101, "alpha").is_err());
    assert_eq!(host.inspect().control.ledger.available, 100);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).unwrap_err(),
        JournalError::Contract(Error::Duplicate));
    let keys = ready(&mut host, &reviewer, 2, b"new approved epoch");
    assert!(keys.action.spec().policy_epoch > action.spec().policy_epoch);
    dispatch(&mut host, &keys);
    host.publish(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().payload, b"new approved epoch");
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn terminal_stop_withdraws_pending_keys_and_drains_only_original_endpoint_receipts_after_reopening() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let reserved = ready(&mut host, &reviewer, 1, b"reserved");
    let unsent = ready(&mut host, &reviewer, 2, b"unsent");
    dispatch(&mut host, &unsent);
    let published = ready(&mut host, &reviewer, 3, b"published");
    dispatch(&mut host, &published);
    host.publish(host.revision(), 3).unwrap();
    let state = host.inspect();
    assert_eq!(state.control.ledger.reserved, 16);
    assert_eq!(state.control.ledger.charged, 32);
    let request = StopRequest { operation: 71, expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch };
    let receipt = host.request_stop(host.revision(), request).unwrap();
    assert_eq!(receipt.refunded_units(), 16);
    assert_eq!(receipt.cancelled(), &[1]);
    assert_eq!(host.human_status(reserved.request.evidence().id()).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert!(!host.stop_progress().unwrap().drained());
    assert!(host.publish(host.revision(), 2).is_err());
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.inspect().stop.as_ref(), Some(&receipt));
    assert!(!host.clock_ready());
    let sweep = host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(sweep.outcomes[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed })));
    assert_eq!(sweep.outcomes[&3], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
    assert!(sweep.progress.drained());
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(host.inspect().control.ledger.available, 84);
    assert_eq!(host.inspect().executions, 1);
    assert!(host.propose(host.revision(), 4, spec(&host, b"forbidden"), snapshot()).is_err());
    assert_eq!(host.request_stop(host.revision(), request).unwrap(), receipt);
    let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
    assert_eq!(disk.stop.as_ref(), Some(&receipt));
    assert_eq!(disk.payload, b"published");
}

#[test]
fn an_observed_journal_cannot_be_opened_as_the_one_key_profile_or_with_changed_review_authority() {
    let root = Directory::new();
    let (host, _) = create(&root);
    assert!(FileDelivery::read_publication(root.store(), &profile().delivery).is_err());
    for change in 0..4 {
        let mut other = profile();
        match change {
            0 => other.human.reviewer_id += 1,
            1 => other.human.max_validity_ticks += 1,
            2 | 3 => {
                let mut members = other.committee.members().clone();
                let helper = members.get("alpha").unwrap();
                let mut binding = helper.profile_at(0);
                let mut question = helper.question().to_vec();
                if change == 2 { binding.model_epoch += 1; } else { question.push(b'?'); }
                let replacement = HelperContract::new(binding, helper.projection_id(), question).unwrap();
                members.insert("alpha".to_owned(), replacement);
                other.committee = CommitteeContract::new(members).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(FileOversight::read_publication(root.store(), &other).unwrap_err(), JournalError::Contract(Error::Binding));
    }
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    drop(host);
    assert!(FileDelivery::open(root.store(), profile().delivery).is_err());
    let plain = Directory::new();
    let host = FileDelivery::create(plain.store(), profile().delivery).unwrap();
    drop(host);
    assert!(FileOversight::open(plain.store(), profile()).is_err());
}

#[test]
fn a_real_staging_collision_returns_no_dispatch_and_recovery_cancels_only_the_unspent_reservation() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = ready(&mut host, &reviewer, 1, b"not sent");
    let before = host.inspect();
    std::fs::write(root.store().join("delivery.pending"), b"inert interrupted stage").unwrap();
    assert!(matches!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()),
        Err(JournalError::Io(_))));
    assert_eq!(host.inspect(), before);
    assert!(!host.clock_ready());
    let revision = host.revision();
    assert_eq!(reviewer.revoke_all(&mut host, revision).unwrap_err(), JournalError::Unavailable);
    let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
    assert_eq!(disk.control.ledger.reserved, 16);
    assert_eq!(disk.control.ledger.charged, 0);
    drop(host);
    let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn expired_retention_does_not_become_a_refund_or_a_successful_stop_drain() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let keys = ready(&mut host, &reviewer, 1, b"unresolved");
    dispatch(&mut host, &keys);
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1002)).unwrap();
    let results = host.reconcile_pending(host.revision()).unwrap();
    assert_eq!(results[&1], Ok(Reconciliation::RetentionExpired));
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    let state = host.inspect();
    host.request_stop(host.revision(), StopRequest { operation: 81,
        expected_control_sequence: state.control.sequence, expected_authority_epoch: state.control.ledger.epoch }).unwrap();
    let sweep = host.progress_stop(host.revision(), ElapsedTick(1003)).unwrap();
    assert_eq!(sweep.outcomes[&1], Ok(Reconciliation::RetentionExpired));
    assert!(!sweep.progress.drained());
    assert_eq!(sweep.progress.unresolved, vec![1]);
    assert_eq!(sweep.progress.charged_units, 16);
    assert_eq!(host.inspect().executions, 0);
}
