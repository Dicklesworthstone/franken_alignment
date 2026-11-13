//! Original campaign, real reviews/effects and independent approval custody.
use super::super::{FileRequestDisposition, JournalError};
use super::*;
use crate::Error;
use crate::action::ActionState;
use crate::action::consequence::delivery::persistent::observed::FileHumanReviewer;
use crate::action::consequence::delivery::persistent::observed::governance::campaigns::FilePolicyGovernor;
use crate::action::consequence::delivery::persistent::{
    JournalIo, Reconciliation, governance::PolicyUpdate,
};
use crate::action::consequence::delivery::{EndpointOutcome, StopRequest};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::actor::UnknownReason;
use crate::action::consequence::oversight::human::HumanDisposition;
use crate::action::consequence::policy_campaign::ReplayLimits;

#[path = "../hold_diagnosis/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn governed(root: &Root) -> (FileOversight, FileHumanReviewer, FilePolicyGovernor) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    let governor = host
        .enable_policy_campaigns(
            host.revision(),
            ReplayLimits {
                cases: 16,
                input_bytes: 1_048_576,
            },
            8,
        )
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer, governor)
}

fn submit(host: &mut FileOversight, request: u64, payload: &[u8]) {
    let mut action = spec(payload);
    action.policy_epoch = host.inspect().control.ledger.epoch;
    action.target = Some(host.inspect().target);
    host.submit_request(host.revision(), request, action, snapshot())
        .unwrap();
}

fn approve(
    host: &mut FileOversight,
    governor: &FilePolicyGovernor,
    campaign: u64,
) -> FilePolicyPromotionPermit {
    let cut = host.inspect();
    let policy = Policy::new(
        host.current_policy().unwrap().generation() + 1,
        vec![
            Predicate::ExactValue {
                key: 7,
                value: b"ok".to_vec(),
            },
            Predicate::PayloadAtMost(2),
            Predicate::All(vec![0, 1]),
        ],
    )
    .unwrap();
    let update = PolicyUpdate::new(
        campaign,
        cut.control.sequence,
        cut.control.ledger.epoch,
        policy,
    )
    .unwrap();
    let review = host
        .request_policy_campaign(host.revision(), &update)
        .unwrap();
    let revision = host.revision();
    governor.approve(host, revision, &review, false).unwrap()
}

fn plan(result: FilePolicyPromotionPlanResult) -> FilePolicyPromotionPlan {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original plan: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.can_promote(), value.request().is_some());
    value
}
fn receipt(result: FilePolicyPromotionResult) -> FilePolicyPromotionReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original promotion: {result:?}");
    };
    assert_eq!(value.after(), basis);
    assert_eq!(
        value.before().journal_revision + 1,
        value.after().journal_revision
    );
    assert_eq!(
        value.before().authority_epoch + 1,
        value.after().authority_epoch
    );
    assert_eq!(
        value.before().control_sequence + 1,
        value.after().control_sequence
    );
    assert_eq!(value.before().recorded_at, value.after().recorded_at);
    value
}

#[test]
fn original_promotion_withdraws_old_keys_preserves_unknown_and_allows_fresh_two_key_publication() {
    let root = Root::new();
    let (mut host, reviewer, governor) = governed(&root);
    submit(&mut host, 701, b"ok");
    dispatch_existing(&mut host, &reviewer, 701);
    assert_eq!(
        host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::AwaitingResolution
    );
    submit(&mut host, 702, b"long");
    let (old_action, old_inputs, old_automatic, old_human) = prepare_existing(&mut host, 702);
    let revision = host.revision();
    let old_human = reviewer.approve(&mut host, revision, &old_human).unwrap();
    let key = approve(&mut host, &governor, 91);
    let before = host.inspect();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .policy_promotion_port(key, 4, MAX_POLICY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    assert!(port.belongs_to(&actor));
    assert_eq!(grant.campaign(), 91);
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    let offered = plan(port.next(91, 900));
    assert_eq!(offered.candidate_policy_generation(), 2);
    assert_eq!(offered.basis().policy_generation, 1);
    assert_eq!(root.files(), files);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(grant.remaining_operations(), 1);
    let input = offered.request().unwrap();
    let acknowledged = receipt(port.promote(input));
    assert_eq!(acknowledged.operation(), 900);
    assert_eq!(acknowledged.cancelled(), &[2]);
    assert_eq!(acknowledged.refunded_units(), 8);
    assert_eq!(acknowledged.candidate_policy_generation(), 2);
    assert_eq!(grant.remaining_operations(), 0);
    assert!(supervisor.owner.borrow().snapshot.is_none());
    {
        let mut host = supervisor.host_mut().unwrap();
        let cut = host.inspect();
        assert_eq!(cut.control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!(cut.control.ledger.stages[&2], ActionState::Cancelled);
        assert_eq!(cut.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(cut.control.ledger.reserved, 0);
        assert_eq!(cut.payload, before.payload);
        assert_eq!(
            host.human_status(old_human.request()).unwrap().disposition,
            HumanDisposition::Revoked
        );
        let revision = host.revision();
        assert!(
            host.dispatch(
                revision,
                &old_automatic,
                &old_human,
                &old_action,
                &old_inputs,
                snapshot()
            )
            .is_err()
        );
        submit(&mut host, 703, b"ok");
    }
    let (action, inputs, automatic, human) = {
        let mut host = supervisor.host_mut().unwrap();
        prepare_existing(&mut host, 703)
    };
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let files = root.files();
    assert_eq!(receipt(port.clone().promote(input)), acknowledged);
    assert_eq!(root.files(), files);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(
        supervisor.host().unwrap().inspect().control.ledger.stages[&3],
        ActionState::Authorized
    );
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(
        plan(port.next(91, 900)).block_reason(),
        Some(FilePolicyPromotionBlockReason::OperationAlreadyUsed)
    );
    assert_eq!(
        plan(port.next(91, 901)).block_reason(),
        Some(FilePolicyPromotionBlockReason::OperationLimit)
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &human).unwrap();
        let revision = host.revision();
        host.dispatch(revision, &automatic, &human, &action, &inputs, snapshot())
            .unwrap();
        let revision = host.revision();
        assert_eq!(
            host.publish(revision, 3).unwrap(),
            EndpointOutcome::Executed {
                resulting_version: 2
            }
        );
        let revision = host.revision();
        host.reconcile(revision, 3).unwrap();
        assert_eq!(
            host.inspect().control.ledger.stages[&1],
            ActionState::Unknown
        );
        assert_eq!(
            host.inspect().control.ledger.charged,
            before.control.ledger.charged + 8
        );
        assert_eq!(host.inspect().executions, 1);
    }
}

#[test]
fn exact_expected_cut_and_finite_work_refuse_before_spending_key_or_admission_snapshot() {
    for short in [false, true] {
        let root = Root::new();
        let (mut host, _, governor) = governed(&root);
        submit(&mut host, 701, b"ok");
        let key = approve(&mut host, &governor, 91);
        let before = host.inspect();
        let (_, mut supervisor) = host.into_actor_gateway();
        supervisor
            .set_snapshot(before.revision, Some(snapshot()))
            .unwrap();
        let limit = before.revision as usize + usize::from(!short);
        let (port, grant) = supervisor.policy_promotion_port(key, 1, limit).unwrap();
        let files = root.files();
        assert_eq!(
            port.next(92, 900),
            Ok(Knowledge::Withheld {
                authority_required: "supervisor_policy_promotion"
            })
        );
        assert_eq!(grant.remaining_queries(), 1);
        let offered = plan(port.next(91, 900));
        assert_eq!(port.next(91, 900), Err(FilePolicyPromotionError::Capacity));
        assert_eq!(grant.remaining_queries(), 0);
        let input = FilePolicyPromotionRequest::new(900, offered.basis()).unwrap();
        if short {
            assert_eq!(
                offered.block_reason(),
                Some(FilePolicyPromotionBlockReason::ReplayLimit)
            );
            assert_eq!(port.promote(input), Err(FilePolicyPromotionError::Capacity));
        } else {
            for field in 0..7 {
                let mut changed = offered.basis();
                match field {
                    0 => changed.journal_revision += 1,
                    1 => changed.control_sequence += 1,
                    2 => changed.authority_epoch += 1,
                    3 => changed.policy_generation += 1,
                    4 => changed.recorded_at = None,
                    5 => changed.scope.branch += 1,
                    6 => changed.scope.authority += 1,
                    _ => unreachable!(),
                }
                assert_eq!(
                    port.promote(FilePolicyPromotionRequest::new(900, changed).unwrap()),
                    Err(FilePolicyPromotionError::StalePredecessor)
                );
            }
        }
        assert_eq!(root.files(), files);
        assert!(supervisor.owner.borrow().snapshot.is_some());
        assert_eq!(grant.remaining_operations(), 1);
        if !short {
            receipt(port.promote(input));
            let mut changed = input.expected();
            changed.journal_revision += 1;
            assert_eq!(
                port.promote(FilePolicyPromotionRequest::new(900, changed).unwrap()),
                Err(FilePolicyPromotionError::IdempotencyConflict)
            );
            assert_eq!(
                port.promote(FilePolicyPromotionRequest::new(901, input.expected()).unwrap()),
                Err(FilePolicyPromotionError::Capacity)
            );
        }
    }
}

#[test]
fn complete_native_corpus_stamp_detects_new_denials_without_control_advancement() {
    let root = Root::new();
    let (mut host, _, governor) = governed(&root);
    submit(&mut host, 701, b"ok");
    let key = approve(&mut host, &governor, 91);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .policy_promotion_port(key, 3, MAX_POLICY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    let original = plan(port.next(91, 900)).request().unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let sequence = host.inspect().control.sequence;
        let mut denied = snapshot();
        denied.values.insert(7, b"denied".to_vec());
        let revision = host.revision();
        host.submit_request(revision, 702, spec(b"ok"), denied)
            .unwrap();
        assert_eq!(host.inspect().control.sequence, sequence);
    }
    let before = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    assert_eq!(
        port.promote(original),
        Err(FilePolicyPromotionError::StalePredecessor)
    );
    let stale = plan(port.next(91, 900));
    assert_eq!(
        stale.block_reason(),
        Some(FilePolicyPromotionBlockReason::StaleCampaign)
    );
    assert_eq!(
        port.promote(FilePolicyPromotionRequest::new(900, stale.basis()).unwrap()),
        Err(FilePolicyPromotionError::StalePredecessor)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(root.files(), files);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .current_policy()
            .unwrap()
            .generation(),
        1
    );
}

#[test]
fn independent_key_issuer_revocation_owner_loss_and_terminal_stop_are_preserved() {
    for mode in 0..5 {
        let root = Root::new();
        let (mut host, _, governor) = governed(&root);
        submit(&mut host, 701, b"ok");
        let key = approve(&mut host, &governor, 91);
        let (actor, mut supervisor) = host.into_actor_gateway();
        let foreign_root = Root::new();
        let (mut foreign, _, foreign_governor) = governed(&foreign_root);
        submit(&mut foreign, 701, b"ok");
        let foreign_key = approve(&mut foreign, &foreign_governor, 91);
        assert!(matches!(
            supervisor.policy_promotion_port(foreign_key, 2, MAX_POLICY_PROMOTION_REPLAY_EVENTS),
            Err(JournalError::Contract(Error::Binding))
        ));
        let (port, grant) = supervisor
            .policy_promotion_port(key, 2, MAX_POLICY_PROMOTION_REPLAY_EVENTS)
            .unwrap();
        let input = plan(port.next(91, 900)).request().unwrap();
        assert!(port.belongs_to(&actor));
        match mode {
            0 => {
                grant.revoke();
                assert_eq!(
                    port.promote(input),
                    Ok(Knowledge::Withheld {
                        authority_required: "supervisor_policy_promotion"
                    })
                );
                assert_eq!(
                    port.next(91, 900),
                    Ok(Knowledge::Withheld {
                        authority_required: "supervisor_policy_promotion"
                    })
                );
            }
            1 => {
                drop(supervisor);
                assert_eq!(
                    port.promote(input),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable
                    })
                );
                assert_eq!(
                    port.next(92, 900),
                    Ok(Knowledge::Withheld {
                        authority_required: "supervisor_policy_promotion"
                    })
                );
                assert_eq!(grant.remaining_operations(), 1);
            }
            2 => {
                let before = foreign.inspect();
                *supervisor.host_mut().unwrap() = foreign;
                assert_eq!(
                    port.promote(input),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable
                    })
                );
                assert_eq!(
                    port.next(91, 900),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable
                    })
                );
                assert_eq!(supervisor.host().unwrap().inspect(), before);
                assert_eq!(grant.remaining_operations(), 1);
            }
            3 => {
                let mut host = supervisor.host_mut().unwrap();
                let review = host.policy_campaign(91).unwrap();
                let revision = host.revision();
                governor.revoke(&mut host, revision, &review).unwrap();
                drop(host);
                let offered = plan(port.next(91, 900));
                assert_eq!(
                    offered.block_reason(),
                    Some(FilePolicyPromotionBlockReason::CampaignNotApproved)
                );
                assert_eq!(
                    port.promote(FilePolicyPromotionRequest::new(900, offered.basis()).unwrap()),
                    Err(FilePolicyPromotionError::Unsupported)
                );
                assert_eq!(grant.remaining_operations(), 1);
            }
            4 => {
                let mut host = supervisor.host_mut().unwrap();
                let before = host.inspect();
                let revision = host.revision();
                host.request_stop(
                    revision,
                    StopRequest {
                        operation: 99,
                        expected_control_sequence: before.control.sequence,
                        expected_authority_epoch: before.control.ledger.epoch,
                    },
                )
                .unwrap();
                drop(host);
                let offered = plan(port.next(91, 900));
                assert_eq!(
                    offered.block_reason(),
                    Some(FilePolicyPromotionBlockReason::Stopped)
                );
                assert_eq!(
                    port.promote(FilePolicyPromotionRequest::new(900, offered.basis()).unwrap()),
                    Err(FilePolicyPromotionError::Unsupported)
                );
                assert_eq!(grant.remaining_operations(), 1);
                assert_eq!(
                    supervisor
                        .host()
                        .unwrap()
                        .current_policy()
                        .unwrap()
                        .generation(),
                    1
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn all_original_storage_barriers_spend_once_without_returning_candidate_refunds() {
    for barrier in [
        JournalIo::Stage,
        JournalIo::Write,
        JournalIo::FileSync,
        JournalIo::Rename,
        JournalIo::DirectorySync,
    ] {
        let root = Root::new();
        let (mut host, _, governor) = governed(&root);
        submit(&mut host, 701, b"ok");
        prepare_existing(&mut host, 701);
        let key = approve(&mut host, &governor, 91);
        let before = host.inspect();
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .policy_promotion_port(key, 1, MAX_POLICY_PROMOTION_REPLAY_EVENTS)
            .unwrap();
        let input = plan(port.next(91, 900)).request().unwrap();
        supervisor
            .set_snapshot(before.revision, Some(snapshot()))
            .unwrap();
        supervisor.host().unwrap().fail_storage_once(barrier);
        assert_eq!(
            port.promote(input),
            Err(FilePolicyPromotionError::Unavailable)
        );
        assert_eq!(grant.remaining_operations(), 0);
        assert!(supervisor.owner.borrow().snapshot.is_none());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(
            port.promote(input),
            Ok(Knowledge::Unknown {
                reason: UnknownReason::ControllerUnavailable
            })
        );
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.revision, before.revision + u64::from(replaced));
        assert_eq!(
            disk.control.ledger.stages[&1],
            if replaced {
                ActionState::Cancelled
            } else {
                ActionState::Authorized
            }
        );
        assert_eq!(disk.control.ledger.reserved, if replaced { 0 } else { 8 });
        assert_eq!(disk.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(disk.executions, before.executions);
        assert_eq!(disk.payload, before.payload);
    }
}

#[test]
fn clock_only_progress_requires_a_fresh_predecessor_without_revoking_the_original_approval() {
    let root = Root::new();
    let (mut host, _, governor) = governed(&root);
    submit(&mut host, 701, b"ok");
    let key = approve(&mut host, &governor, 91);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .policy_promotion_port(key, 2, MAX_POLICY_PROMOTION_REPLAY_EVENTS)
        .unwrap();
    let old = plan(port.next(91, 900)).request().unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    assert_eq!(
        port.promote(old),
        Err(FilePolicyPromotionError::StalePredecessor)
    );
    assert_eq!(grant.remaining_operations(), 1);
    let fresh = plan(port.next(91, 900));
    assert_eq!(fresh.basis().recorded_at, Some(ElapsedTick(2)));
    assert_eq!(
        fresh.basis().control_sequence,
        old.expected().control_sequence
    );
    assert_eq!(
        fresh.basis().authority_epoch,
        old.expected().authority_epoch
    );
    assert!(fresh.can_promote());
    receipt(port.promote(fresh.request().unwrap()));
    assert_eq!(grant.remaining_operations(), 0);
}

#[path = "numerical.rs"]
mod numerical;
