//! Original joint-qualified labels, durable effects and finite live delegation.
//! Synthetic helper verdicts test the protocol; they do not measure model skill.
use fa_reference::Error;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileCredibilityPromotionBlockReason, FileCredibilityPromotionError,
    FileCredibilityPromotionPlan, FileCredibilityPromotionPlanResult,
    FileCredibilityPromotionReceipt, FileCredibilityPromotionRequest,
    FileCredibilityPromotionResult, MAX_CREDIBILITY_PROMOTION_QUERIES,
    MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorProposal, Knowledge, UnknownReason,
};
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::joint_credibility::{
    JointFailure, JointReplayBudget,
};
use fa_reference::round::Verdict;

#[path = "support/file_credibility_promotion.rs"]
mod fixture;
use fixture::*;

fn plan(result: FileCredibilityPromotionPlanResult) -> FileCredibilityPromotionPlan {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original promotion plan: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.can_promote(), value.request().is_some());
    value
}

fn receipt(result: FileCredibilityPromotionResult) -> FileCredibilityPromotionReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original promotion receipt: {result:?}");
    };
    assert_eq!(value.after(), basis);
    assert_eq!(value.before().journal_revision + 1, value.after().journal_revision);
    assert_eq!(value.before().control_sequence + 1, value.after().control_sequence);
    assert_eq!(value.before().authority_epoch + 1, value.after().authority_epoch);
    assert_eq!(value.before().evaluation_revision + 1, value.after().evaluation_revision);
    assert_eq!(value.before().congress_generation + 1, value.after().congress_generation);
    assert_eq!(value.before().policy_generation, value.after().policy_generation);
    assert_eq!(value.before().recorded_at, value.after().recorded_at);
    assert_eq!(value.before().scope, value.after().scope);
    value
}

fn proposal(host: &FileOversight) -> ActorProposal {
    ActorProposal {
        target: host.inspect().target,
        payload: b"visible".to_vec(),
        units: 16,
        deadline: ElapsedTick(1000),
        expected_policy_epoch: host.inspect().control.ledger.epoch,
    }
}

#[test]
fn live_reweighting_keeps_unknown_charges_and_requires_both_fresh_publication_keys() {
    let (root, mut host, human, evaluator) = ready(0, JointReplayBudget::default());
    calibrate(&mut host, &evaluator);
    let (unknown_action, unknown_inputs) = review(&mut host, 4, Verdict::Allow, Verdict::Allow);
    assess(&mut host, &evaluator, 4, 4, GroundTruth::Benign);
    dispatch(&mut host, &human, 4, &unknown_action, &unknown_inputs);
    assert_eq!(
        host.reconcile(host.revision(), 4).unwrap(),
        Reconciliation::AwaitingResolution
    );
    let (old_action, old_inputs) = review(&mut host, 5, Verdict::Allow, Verdict::Allow);
    assess(&mut host, &evaluator, 5, 5, GroundTruth::Benign);
    let automatic = host.authorize(host.revision(), 5, &old_inputs, snapshot()).unwrap();
    let human_request = host.request_human_approval(
        host.revision(), 1005, 5, &old_inputs, ElapsedTick(50),
    ).unwrap();
    let revision = host.revision();
    let old_human = human.approve(&mut host, revision, &human_request).unwrap();
    let joint = host.joint_credibility_report(host.credibility_report().unwrap().revision).unwrap();
    assert!(joint.qualified());
    let before = host.inspect();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .credibility_promotion_port(4, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let bytes = root.bytes();
    let offered = plan(port.next(900));
    assert!(offered.can_promote());
    assert_eq!(root.bytes(), bytes);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    let input = offered.request().unwrap();
    let acknowledged = receipt(port.promote(input));
    assert_eq!(acknowledged.operation(), 900);
    assert_eq!(acknowledged.refunded_units(), 16);
    assert!(acknowledged.cancelled().contains(&5));
    assert_eq!(grant.remaining_operations(), 0);
    {
        let host = supervisor.host().unwrap();
        let after = host.inspect();
        assert_eq!(after.control.ledger.stages[&4], ActionState::Unknown);
        assert_eq!(after.control.ledger.stages[&5], ActionState::Cancelled);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.control.ledger.charged, 16);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.executions, before.executions);
        assert_eq!(host.human_status(1005).unwrap().disposition, HumanDisposition::Revoked);
        assert_eq!(host.joint_credibility_promotion(900).unwrap(), &joint);
        let change = &host.credibility_promotion(900).unwrap().change;
        assert_eq!(change.current.members["a"].weight, 5);
        assert_eq!(change.current.members["b"].weight, 5);
    }
    let fresh_proposal = proposal(&supervisor.host().unwrap());
    assert!(matches!(actor.submit(701, &fresh_proposal), Err(ActorError::Unavailable)));
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.dispatch(
            revision, &automatic, &old_human, &old_action, &old_inputs, snapshot(),
        ).is_err());
        let (action, inputs) = review(&mut host, 6, Verdict::Allow, Verdict::Allow);
        let revision = host.revision();
        let automatic = host.authorize(revision, 6, &inputs, snapshot()).unwrap();
        let revision = host.revision();
        let request = host.request_human_approval(
            revision, 1006, 6, &inputs, ElapsedTick(60),
        ).unwrap();
        let revision = host.revision();
        let fresh_human = human.approve(&mut host, revision, &request).unwrap();
        let revision = host.revision();
        assert!(host.dispatch(
            revision, &automatic, &old_human, &action, &inputs, snapshot(),
        ).is_err());
        let revision = host.revision();
        host.dispatch(revision, &automatic, &fresh_human, &action, &inputs, snapshot()).unwrap();
        let revision = host.revision();
        assert_eq!(host.publish_checked(
            revision, 6, Some(&inputs), snapshot(), ElapsedTick(25),
        ).unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
        let revision = host.revision();
        host.reconcile(revision, 6).unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&4], ActionState::Unknown);
        assert_eq!(host.inspect().control.ledger.charged, 32);
        assert_eq!(host.inspect().executions, 1);
    }
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let bytes = root.bytes();
    assert_eq!(receipt(port.clone().promote(input)), acknowledged);
    assert_eq!(root.bytes(), bytes);
    assert_eq!(grant.remaining_operations(), 0);
    let next_proposal = proposal(&supervisor.host().unwrap());
    actor.submit(702, &next_proposal).unwrap();
}

#[test]
fn every_predecessor_coordinate_is_checked_before_the_operation_or_journal_changes() {
    let (root, mut host, _, evaluator) = ready(0, JointReplayBudget::default());
    calibrate(&mut host, &evaluator);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .credibility_promotion_port(3, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(900));
    let bytes = root.bytes();
    for dimension in 0..12 {
        let mut expected = offered.basis();
        match dimension {
            0 => expected.journal_revision += 1,
            1 => expected.control_sequence += 1,
            2 => expected.authority_epoch += 1,
            3 => expected.evaluation_revision += 1,
            4 => expected.congress_generation += 1,
            5 => expected.policy_generation += 1,
            6 => expected.recorded_at = None,
            7 => expected.scope.tenant += 1,
            8 => expected.scope.principal += 1,
            9 => expected.scope.run += 1,
            10 => expected.scope.branch += 1,
            11 => expected.scope.authority += 1,
            _ => unreachable!(),
        }
        let result = port.promote(FileCredibilityPromotionRequest::new(900, expected).unwrap());
        if dimension >= 7 {
            assert_eq!(result, Ok(Knowledge::Withheld {
                authority_required: "supervisor_credibility_promotion",
            }));
        } else {
            assert_eq!(result, Err(FileCredibilityPromotionError::StalePredecessor));
        }
        assert_eq!(root.bytes(), bytes);
        assert_eq!(grant.remaining_operations(), 1);
    }
    let input = offered.request().unwrap();
    let acknowledged = receipt(port.promote(input));
    let after = root.bytes();
    assert_eq!(receipt(port.clone().promote(input)), acknowledged);
    let mut conflicting = input.expected();
    conflicting.evaluation_revision += 1;
    assert_eq!(port.clone().promote(FileCredibilityPromotionRequest::new(900, conflicting).unwrap()),
        Err(FileCredibilityPromotionError::IdempotencyConflict));
    assert_eq!(port.promote(FileCredibilityPromotionRequest::new(901, input.expected()).unwrap()),
        Err(FileCredibilityPromotionError::Capacity));
    assert_eq!(plan(port.next(900)).block_reason(),
        Some(FileCredibilityPromotionBlockReason::OperationAlreadyUsed));
    assert_eq!(plan(port.next(901)).block_reason(),
        Some(FileCredibilityPromotionBlockReason::OperationLimit));
    assert_eq!(root.bytes(), after);
    grant.revoke();
    assert_eq!(port.promote(input), Ok(Knowledge::Withheld {
        authority_required: "supervisor_credibility_promotion",
    }));
    assert_eq!(root.bytes(), after);
}

#[test]
fn planning_leaves_the_original_actor_snapshot_available_but_does_not_pin_the_owner() {
    let (root, mut host, _, evaluator) = ready(0, JointReplayBudget::default());
    calibrate(&mut host, &evaluator);
    let before = host.inspect();
    let (actor, mut supervisor) = host.into_actor_gateway();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let (port, grant) = supervisor
        .credibility_promotion_port(1, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
    let bytes = root.bytes();
    let input = plan(port.next(900)).request().unwrap();
    assert_eq!(root.bytes(), bytes);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.clone().next(901), Err(FileCredibilityPromotionError::Capacity));
    let input_proposal = proposal(&supervisor.host().unwrap());
    actor.submit(701, &input_proposal).unwrap();
    let bytes = root.bytes();
    assert_eq!(port.promote(input), Err(FileCredibilityPromotionError::StalePredecessor));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn unfinished_pending_and_censored_cases_resolve_only_through_the_original_evaluator() {
    for mode in 0..3 {
        let (root, mut host, _, evaluator) = ready(0, JointReplayBudget::default());
        calibrate(&mut host, &evaluator);
        let unfinished = if mode == 0 {
            Some(begin(&mut host, 4).1)
        } else {
            review(&mut host, 4, Verdict::Allow, Verdict::Allow);
            if mode == 2 {
                assess(&mut host, &evaluator, 4, 4, GroundTruth::Censored);
            }
            None
        };
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .credibility_promotion_port(2, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
        let bytes = root.bytes();
        let blocked = plan(port.next(900));
        // The native marginal report includes unapplied current-policy rounds
        // as well as completed cases still lacking an independent final label.
        assert_eq!(blocked.block_reason(),
            Some(FileCredibilityPromotionBlockReason::MarginalUnqualified));
        assert!(blocked.request().is_none());
        assert_eq!(grant.remaining_operations(), 1);
        assert_eq!(root.bytes(), bytes);
        {
            let mut host = supervisor.host_mut().unwrap();
            if let Some(inputs) = unfinished {
                finish(&mut host, 4, &inputs, Verdict::Allow, Verdict::Allow);
            }
            assess(&mut host, &evaluator, 4, 4, GroundTruth::Benign);
            assert_eq!(host.evaluation_history(104).unwrap().len(), if mode == 2 { 2 } else { 1 });
        }
        let fresh = plan(port.next(900));
        assert!(fresh.can_promote());
        assert!(fresh.basis().evaluation_revision > blocked.basis().evaluation_revision);
        let bytes = root.bytes();
        assert_eq!(port.promote(FileCredibilityPromotionRequest::new(900, blocked.basis()).unwrap()),
            Err(FileCredibilityPromotionError::StalePredecessor));
        assert_eq!(root.bytes(), bytes);
        assert_eq!(grant.remaining_operations(), 1);
        receipt(port.promote(fresh.request().unwrap()));
    }
}

#[test]
fn equal_marginal_success_cannot_hide_a_joint_regression_or_repeat_failed_work() {
    for hold_maximum in [5, 0] {
        let (root, mut host, _, evaluator) = ready(hold_maximum, JointReplayBudget::default());
        calibrate(&mut host, &evaluator);
        let evaluation = host.credibility_report().unwrap();
        assert!(evaluation.qualified());
        let joint = host.joint_credibility_report(evaluation.revision).unwrap();
        if hold_maximum == 5 {
            assert!(joint.failures.contains(&JointFailure::RegressedOrigin(1)));
            assert_eq!(joint.candidate_counts.escaped_violation_origins, 2);
        } else {
            assert!(joint.qualified());
            assert_eq!(joint.candidate_counts.escaped_violation_origins, 0);
        }
        let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .credibility_promotion_port(2, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
        let offered = plan(port.next(900));
        let input = FileCredibilityPromotionRequest::new(900, offered.basis()).unwrap();
        let before = supervisor.host().unwrap().inspect();
        let bytes = root.bytes();
        if hold_maximum == 5 {
            assert_eq!(offered.block_reason(), Some(FileCredibilityPromotionBlockReason::JointUnqualified));
            assert_eq!(port.promote(input), Err(FileCredibilityPromotionError::Unsupported));
            assert_eq!(grant.remaining_operations(), 0);
            assert_eq!(port.clone().promote(input), Err(FileCredibilityPromotionError::Unsupported));
            assert_eq!(port.promote(FileCredibilityPromotionRequest::new(901, input.expected()).unwrap()),
                Err(FileCredibilityPromotionError::Capacity));
            assert_eq!(root.bytes(), bytes);
            assert_eq!(supervisor.host().unwrap().inspect(), before);
        } else {
            assert!(offered.can_promote());
            receipt(port.promote(input));
        }
    }
}

#[test]
fn original_joint_work_budget_has_an_exact_positive_boundary_and_no_marginal_fallback() {
    for member_outcomes in [11, 12] {
        let (root, mut host, _, evaluator) = ready(0, JointReplayBudget { cases: 3, member_outcomes });
        calibrate(&mut host, &evaluator);
        let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .credibility_promotion_port(1, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
        let bytes = root.bytes();
        let offered = plan(port.next(900));
        assert_eq!(root.bytes(), bytes);
        let input = FileCredibilityPromotionRequest::new(900, offered.basis()).unwrap();
        if member_outcomes == 11 {
            assert_eq!(offered.block_reason(), Some(FileCredibilityPromotionBlockReason::EvaluationWorkLimit));
            assert_eq!(port.promote(input), Err(FileCredibilityPromotionError::Capacity));
            assert_eq!(grant.remaining_operations(), 0);
            assert_eq!(port.clone().promote(input), Err(FileCredibilityPromotionError::Capacity));
            assert_eq!(root.bytes(), bytes);
        } else {
            assert!(offered.can_promote());
            receipt(port.promote(input));
        }
    }
}

#[test]
fn complete_history_allowance_and_query_clones_cannot_be_replenished() {
    for enough in [false, true] {
        let (root, mut host, _, evaluator) = ready(0, JointReplayBudget::default());
        calibrate(&mut host, &evaluator);
        let limit = usize::try_from(host.revision()).unwrap() + usize::from(enough);
        let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.credibility_promotion_port(1, limit).unwrap();
        let clone = port.clone();
        let offered = plan(port.next(900));
        assert_eq!(grant.max_replay_events(), limit);
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(clone.next(900), Err(FileCredibilityPromotionError::Capacity));
        assert_eq!(grant.remaining_numerical_replay_events(), None);
        let bytes = root.bytes();
        let input = FileCredibilityPromotionRequest::new(900, offered.basis()).unwrap();
        if enough {
            assert!(offered.can_promote());
            let acknowledgment = receipt(clone.promote(input));
            assert_eq!(receipt(port.promote(input)), acknowledgment);
            assert_eq!(grant.remaining_operations(), 0);
        } else {
            assert_eq!(offered.block_reason(), Some(FileCredibilityPromotionBlockReason::ReplayLimit));
            assert_eq!(clone.promote(input), Err(FileCredibilityPromotionError::Capacity));
            assert_eq!(port.promote(input), Err(FileCredibilityPromotionError::Capacity));
            assert_eq!(grant.remaining_operations(), 1);
            assert_eq!(root.bytes(), bytes);
        }
    }
}

#[test]
fn revoked_missing_borrowed_and_replaced_owner_never_becomes_a_fresh_promotion_source() {
    for mode in 0..4 {
        let (root, mut host, _, evaluator) = ready(0, JointReplayBudget::default());
        calibrate(&mut host, &evaluator);
        let (actor, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .credibility_promotion_port(3, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
        let input = plan(port.next(900)).request().unwrap();
        let bytes = root.bytes();
        match mode {
            0 => {
                grant.revoke();
                assert_eq!(port.next(900), Ok(Knowledge::Withheld {
                    authority_required: "supervisor_credibility_promotion",
                }));
                assert_eq!(port.clone().promote(input), Ok(Knowledge::Withheld {
                    authority_required: "supervisor_credibility_promotion",
                }));
            }
            1 => {
                drop(supervisor);
                // Retaining the actor, grant and all weak clones cannot retain
                // the original mutable owner or mint a replacement one.
                let _retained_actor = actor;
                assert_eq!(port.next(900), Ok(Knowledge::Unknown {
                    reason: UnknownReason::ControllerUnavailable,
                }));
                assert_eq!(port.clone().promote(input), Ok(Knowledge::Unknown {
                    reason: UnknownReason::ControllerUnavailable,
                }));
                assert_eq!(grant.remaining_operations(), 1);
                assert_eq!(grant.remaining_queries(), 2);
            }
            2 => {
                let owner = supervisor.host_mut().unwrap();
                assert_eq!(port.next(900), Ok(Knowledge::Unknown {
                    reason: UnknownReason::ControllerUnavailable,
                }));
                assert_eq!(port.promote(input), Ok(Knowledge::Unknown {
                    reason: UnknownReason::ControllerUnavailable,
                }));
                assert_eq!(grant.remaining_operations(), 1);
                assert_eq!(grant.remaining_queries(), 2);
                drop(owner);
                assert_eq!(root.bytes(), bytes);
                receipt(port.promote(input));
                continue;
            }
            3 => {
                let (foreign_root, mut foreign, _, foreign_evaluator) = ready(0, JointReplayBudget::default());
                calibrate(&mut foreign, &foreign_evaluator);
                assert_eq!(foreign.inspect(), supervisor.host().unwrap().inspect());
                let before = foreign.inspect();
                let foreign_bytes = foreign_root.bytes();
                *supervisor.host_mut().unwrap() = foreign;
                assert_eq!(port.next(900), Ok(Knowledge::Unknown {
                    reason: UnknownReason::ControllerUnavailable,
                }));
                assert_eq!(port.promote(input), Ok(Knowledge::Unknown {
                    reason: UnknownReason::ControllerUnavailable,
                }));
                assert_eq!(supervisor.host().unwrap().inspect(), before);
                assert_eq!(foreign_root.bytes(), foreign_bytes);
                assert_eq!(grant.remaining_operations(), 1);
            }
            _ => unreachable!(),
        }
        assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn marginal_only_configuration_and_invalid_grants_cannot_install_the_live_joint_role() {
    for marginal in [false, true] {
        let root = Directory::new();
        let (mut host, _) = FileOversight::create(&root.0, profile(0)).unwrap();
        if marginal {
            let revision = host.revision();
            let evaluator = host.enable_credibility(revision, protocol()).unwrap();
            calibrate(&mut host, &evaluator);
        }
        let before = host.inspect();
        let bytes = root.bytes();
        let (_, supervisor) = host.into_actor_gateway();
        assert!(matches!(supervisor.credibility_promotion_port(1, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS),
            Err(JournalError::Contract(Error::Incomplete))));
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(root.bytes(), bytes);
    }
    let (root, host, _, _) = ready(0, JointReplayBudget::default());
    let (_, supervisor) = host.into_actor_gateway();
    let bytes = root.bytes();
    for (queries, events) in [(0, 1), (1, 0), (MAX_CREDIBILITY_PROMOTION_QUERIES + 1, 1),
        (1, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS + 1)] {
        assert!(supervisor.credibility_promotion_port(queries, events).is_err());
    }
    for (events, numerical) in [(1, 0), (1, 2)] {
        assert!(supervisor.credibility_promotion_port_with_numerical_budget(1, events, numerical).is_err());
    }
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn a_recovery_fence_requires_original_time_observation_but_never_erases_labels() {
    let (root, mut host, _, evaluator) = ready(0, JointReplayBudget::default());
    calibrate(&mut host, &evaluator);
    let evaluation = host.credibility_report().unwrap();
    host.fence(host.revision()).unwrap();
    assert!(!host.clock_ready());
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .credibility_promotion_port(2, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
    let bytes = root.bytes();
    let blocked = plan(port.next(900));
    assert_eq!(blocked.block_reason(), Some(FileCredibilityPromotionBlockReason::ClockUnavailable));
    assert_eq!(port.promote(FileCredibilityPromotionRequest::new(900, blocked.basis()).unwrap()),
        Err(FileCredibilityPromotionError::Unsupported));
    assert_eq!(root.bytes(), bytes);
    assert_eq!(grant.remaining_operations(), 1);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(50)).unwrap();
        assert_eq!(host.credibility_report().unwrap(), evaluation);
    }
    let offered = plan(port.next(900));
    assert!(offered.can_promote());
    receipt(port.promote(offered.request().unwrap()));
}

#[test]
fn terminal_stop_and_native_suspension_cannot_be_cleared_by_reweighting() {
    for suspended in [false, true] {
        let root = Directory::new();
        let mut p = profile(0);
        p.delivery.congress.narrow_at = 15;
        p.delivery.congress.suspend_at = 20;
        let (mut host, _) = FileOversight::create(&root.0, p).unwrap();
        let evaluator = host.enable_joint_credibility(
            host.revision(), protocol(), guard(JointReplayBudget::default()),
        ).unwrap();
        calibrate(&mut host, &evaluator);
        if suspended {
            review(&mut host, 4, Verdict::Hold, Verdict::Hold);
            assess(&mut host, &evaluator, 4, 4, GroundTruth::Violation);
            assert!(host.inspect().control.suspended);
        } else {
            let cut = host.inspect();
            host.request_stop(host.revision(), StopRequest {
                operation: 99,
                expected_control_sequence: cut.control.sequence,
                expected_authority_epoch: cut.control.ledger.epoch,
            }).unwrap();
            assert!(host.inspect().stop.is_some());
        }
        let before = host.inspect();
        let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .credibility_promotion_port(1, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS).unwrap();
        let bytes = root.bytes();
        let offered = plan(port.next(900));
        assert_eq!(offered.block_reason(), Some(if suspended {
            FileCredibilityPromotionBlockReason::Suspended
        } else {
            FileCredibilityPromotionBlockReason::Stopped
        }));
        assert_eq!(port.promote(FileCredibilityPromotionRequest::new(900, offered.basis()).unwrap()),
            Err(FileCredibilityPromotionError::Unsupported));
        assert_eq!(grant.remaining_operations(), 1);
        assert_eq!(root.bytes(), bytes);
        assert_eq!(supervisor.host().unwrap().inspect(), before);
    }
}
