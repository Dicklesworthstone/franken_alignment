//! Live promotion on the original learned owner with paid history replay.
//! Synthetic model weights and helper verdicts test authority and accounting,
//! not detector effectiveness or organizational independence.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileCredibilityPromotionBlockReason, FileCredibilityPromotionError,
    FileCredibilityPromotionPlan, FileCredibilityPromotionPlanResult,
    FileCredibilityPromotionReceipt, FileCredibilityPromotionRequest,
    FileCredibilityPromotionResult, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::human::HumanDisposition;

const PROMOTION: u64 = 501;

fn started() -> (Directory, FileOversight, FileEvaluatedOversightRoles) {
    let root = Directory::new();
    let (mut host, roles) = create(&root, config(0), joint(JointReplayBudget::default()));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    assert!(
        host.learned_generation_inspection()
            .unwrap()
            .numerical
            .work
            .admitted_tokens
            > 0
    );
    fresh_identity(&mut host, &roles.oversight, 1, 1);
    (root, host, roles)
}

fn qualified() -> (Directory, FileOversight, FileEvaluatedOversightRoles) {
    let (root, mut host, roles) = started();
    for (attempt, round, verdict, truth) in [
        (1, 101, Verdict::Allow, GroundTruth::Benign),
        (2, 102, Verdict::Hold, GroundTruth::Violation),
    ] {
        let (_, input) = staged(&mut host, attempt);
        reviewed(&mut host, attempt, round, &input, verdict).unwrap();
        label(&mut host, &roles, round, 200 + attempt, truth);
    }
    let marginal = host.credibility_report().unwrap();
    assert!(marginal.qualified());
    let report = host.joint_credibility_report(marginal.revision).unwrap();
    assert!(report.qualified());
    assert_eq!(report.work.cases, 2);
    assert_eq!(report.work.member_outcomes, 4);
    (root, host, roles)
}

fn plan(result: FileCredibilityPromotionPlanResult) -> FileCredibilityPromotionPlan {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected the original learned promotion plan: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.can_promote(), value.request().is_some());
    value
}

fn receipt(result: FileCredibilityPromotionResult) -> FileCredibilityPromotionReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected the original learned promotion acknowledgment: {result:?}");
    };
    assert_eq!(value.after(), basis);
    value
}

fn proposal(host: &FileOversight) -> ActorProposal {
    ActorProposal {
        target: host.inspect().target,
        payload: b"visible".to_vec(),
        units: 16,
        deadline: ElapsedTick(100),
        expected_policy_epoch: host.inspect().control.ledger.epoch,
    }
}

#[test]
fn exact_paid_promotion_preserves_unknown_charges_and_fresh_keys_publish_after_reweighting() {
    let (root, mut host, roles) = started();

    // The first original benign review is dispatched without an endpoint
    // outcome. Its charged liability must survive every weight transition.
    let (unknown_action, unknown_input, unknown_key, unknown_request) = prepared(&mut host, 1, 101);
    label(&mut host, &roles, 101, 201, GroundTruth::Benign);
    let original_ticket = host.evaluation_ticket(101).unwrap();
    let revision = host.revision();
    let unknown_human = roles
        .oversight
        .human
        .approve(&mut host, revision, &unknown_request)
        .unwrap();
    host.dispatch(
        host.revision(),
        &unknown_key,
        &unknown_human,
        &unknown_action,
        &unknown_input,
        snapshot(),
    )
    .unwrap();
    assert_eq!(
        host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::AwaitingResolution
    );

    // A second independently labelled benign review holds unused keys. The
    // violation stratum is a separate original review and independent label.
    let (old_action, old_input, old_key, old_request) = prepared(&mut host, 2, 102);
    label(&mut host, &roles, 102, 202, GroundTruth::Benign);
    let revision = host.revision();
    let old_human = roles
        .oversight
        .human
        .approve(&mut host, revision, &old_request)
        .unwrap();
    let (_, violation_input) = staged(&mut host, 3);
    reviewed(&mut host, 3, 103, &violation_input, Verdict::Hold).unwrap();
    label(&mut host, &roles, 103, 203, GroundTruth::Violation);
    let marginal = host.credibility_report().unwrap();
    assert!(marginal.qualified());
    let joint = host.joint_credibility_report(marginal.revision).unwrap();
    assert!(joint.qualified());
    assert_eq!(joint.work.cases, 3);
    assert_eq!(joint.work.member_outcomes, 6);
    assert_eq!(joint.baseline.members["reviewer"].weight, 1);
    assert_eq!(joint.candidate.members["reviewer"].weight, 2);

    let before = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap();
    let accounting = host.andon_ledger().unwrap().clone();
    assert_eq!(before.control.ledger.charged, 16);
    assert_eq!(before.control.ledger.reserved, 16);
    let cost = usize::try_from(before.revision).unwrap() + 1;
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .credibility_promotion_port_with_numerical_budget(
            3,
            MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
            cost,
        )
        .unwrap();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let canonical = root.bytes();
    let offered = plan(port.next(PROMOTION));
    assert!(offered.can_promote());
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(cost));
    assert_eq!(root.bytes(), canonical);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );

    let input = offered.request().unwrap();
    let acknowledged = receipt(port.promote(input));
    assert_eq!(acknowledged.operation(), PROMOTION);
    assert_eq!(acknowledged.before(), offered.basis());
    let after_basis = acknowledged.after();
    assert_eq!(after_basis.journal_revision, before.revision + 1);
    assert_eq!(after_basis.control_sequence, before.control.sequence + 1);
    assert_eq!(after_basis.authority_epoch, before.control.ledger.epoch + 1);
    assert_eq!(after_basis.evaluation_revision, marginal.revision + 1);
    assert_eq!(after_basis.congress_generation, 2);
    assert_eq!(
        after_basis.policy_generation,
        offered.basis().policy_generation
    );
    assert_eq!(after_basis.recorded_at, Some(ElapsedTick(1)));
    assert_eq!(acknowledged.refunded_units(), 16);
    assert!(acknowledged.cancelled().contains(&2));
    assert!(!acknowledged.cancelled().contains(&1));
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    {
        let host = supervisor.host().unwrap();
        let after = host.inspect();
        let current = host.learned_generation_inspection().unwrap();
        assert_eq!(current.numerical, numerical.numerical);
        assert_eq!(current.paused, numerical.paused);
        assert_eq!(current.pending, numerical.pending);
        assert_eq!(after.control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!(after.control.ledger.stages[&2], ActionState::Cancelled);
        assert_eq!(after.control.ledger.charged, 16);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.executions, before.executions);
        assert_eq!(host.andon_ledger().unwrap(), &accounting);
        assert_eq!(host.joint_credibility_promotion(PROMOTION).unwrap(), &joint);
        assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
        assert_eq!(
            host.human_status(1002).unwrap().disposition,
            HumanDisposition::Revoked
        );
        roles.evaluator.check_owner(&host).unwrap();
    }
    let proposed = proposal(&supervisor.host().unwrap());
    assert_eq!(
        actor.submit(701, &proposed).err(),
        Some(ActorError::Unavailable)
    );

    let (action, new_input) = {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(
            host.dispatch(
                revision,
                &old_key,
                &old_human,
                &old_action,
                &old_input,
                snapshot(),
            )
            .is_err()
        );
        // Promotion did not reopen the owner or replace the independent
        // evaluator. Even its original ticket keeps its exact-retry identity.
        let bytes = root.bytes();
        let revision = host.revision();
        assert!(
            !roles
                .evaluator
                .assess(
                    &mut host,
                    revision,
                    &original_ticket,
                    Assessment {
                        origin: 201,
                        evidence_id: [101; 32],
                        truth: GroundTruth::Benign,
                    },
                )
                .unwrap()
        );
        assert_eq!(root.bytes(), bytes);
        fresh_identity(&mut host, &roles.oversight, 2, 1);
        let (action, input) = staged(&mut host, 4);
        let revision = host.revision();
        host.begin_review(
            revision,
            4,
            104,
            [9; 32],
            ReviewWindow {
                commit_by: ElapsedTick(10),
                reveal_by: ElapsedTick(20),
            },
            snapshot(),
        )
        .unwrap();
        (action, input)
    };

    // Retry the historical acknowledgment while a newer original review is
    // open. It cannot replay, clear that review, or consume a fresh observation.
    let current = supervisor.host().unwrap().inspect();
    let current_numerical = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    supervisor
        .set_snapshot(current.revision, Some(snapshot()))
        .unwrap();
    let canonical = root.bytes();
    assert_eq!(receipt(port.clone().promote(input)), acknowledged);
    assert_eq!(supervisor.host().unwrap().inspect(), current);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        current_numerical
    );
    assert_eq!(root.bytes(), canonical);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(
        plan(port.next(PROMOTION)).block_reason(),
        Some(FileCredibilityPromotionBlockReason::OperationAlreadyUsed)
    );
    assert_eq!(
        plan(port.next(PROMOTION + 1)).block_reason(),
        Some(FileCredibilityPromotionBlockReason::OperationLimit)
    );
    assert_eq!(
        port.promote(
            FileCredibilityPromotionRequest::new(PROMOTION + 1, input.expected()).unwrap()
        ),
        Err(FileCredibilityPromotionError::Capacity)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(root.bytes(), canonical);
    let proposed = proposal(&supervisor.host().unwrap());
    let _ticket = actor.submit(702, &proposed).unwrap();

    {
        let mut host = supervisor.host_mut().unwrap();
        let salt = b"independent-fixture-salt";
        let revision = host.revision();
        host.commit_review(
            revision,
            104,
            "reviewer",
            commitment(104, "reviewer", &[9; 32], Verdict::Allow, salt).unwrap(),
        )
        .unwrap();
        let revision = host.revision();
        host.open_reveals(revision, 104).unwrap();
        let revision = host.revision();
        host.reveal_review(revision, 104, "reviewer", Verdict::Allow, salt.to_vec())
            .unwrap();
        let revision = host.revision();
        host.finish_review(revision, 104, Some(&new_input), snapshot())
            .unwrap()
            .unwrap();
        label(&mut host, &roles, 104, 204, GroundTruth::Benign);
        let revision = host.revision();
        let automatic = host.authorize(revision, 4, &new_input, snapshot()).unwrap();
        let revision = host.revision();
        let request = host
            .request_human_approval(revision, 1004, 4, &new_input, ElapsedTick(30))
            .unwrap();
        let revision = host.revision();
        assert!(
            host.publish_checked(revision, 4, Some(&new_input), snapshot(), ElapsedTick(1))
                .is_err()
        );
        assert_eq!(host.inspect().executions, 0);
        let revision = host.revision();
        let human = roles
            .oversight
            .human
            .approve(&mut host, revision, &request)
            .unwrap();
        let revision = host.revision();
        assert!(
            host.dispatch(
                revision,
                &automatic,
                &old_human,
                &action,
                &new_input,
                snapshot()
            )
            .is_err()
        );
        let revision = host.revision();
        assert!(
            host.dispatch(revision, &old_key, &human, &action, &new_input, snapshot())
                .is_err()
        );
        let revision = host.revision();
        host.dispatch(
            revision,
            &automatic,
            &human,
            &action,
            &new_input,
            snapshot(),
        )
        .unwrap();
        assert_eq!(host.inspect().executions, 0);
        let revision = host.revision();
        assert_eq!(
            host.publish_checked(revision, 4, Some(&new_input), snapshot(), ElapsedTick(2))
                .unwrap()
                .outcome,
            EndpointOutcome::Executed {
                resulting_version: 2
            }
        );
        let revision = host.revision();
        host.reconcile(revision, 4).unwrap();
        let after = host.inspect();
        assert_eq!(after.payload, b"visible");
        assert_eq!(after.executions, 1);
        assert_eq!(after.control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!(after.control.ledger.charged, 32);
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical.numerical
        );
        roles.evaluator.check_owner(&host).unwrap();
        assert_eq!(host.evaluation_history(104).unwrap().len(), 1);
        assert_eq!(host.andon_ledger().unwrap(), &accounting);
    }
    let final_state = supervisor.host().unwrap().inspect();
    let canonical = root.bytes();
    assert_eq!(receipt(port.promote(input)), acknowledged);
    assert_eq!(supervisor.host().unwrap().inspect(), final_state);
    assert_eq!(root.bytes(), canonical);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
}

#[test]
fn absent_and_one_short_numerical_allowances_preserve_original_work_and_snapshot() {
    for paid in [false, true] {
        let (root, host, roles) = qualified();
        let before = host.inspect();
        let numerical = host.learned_generation_inspection().unwrap();
        let cost = usize::try_from(before.revision).unwrap() + 1;
        let (actor, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = if paid {
            supervisor.credibility_promotion_port_with_numerical_budget(
                1,
                MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
                cost - 1,
            )
        } else {
            supervisor.credibility_promotion_port(1, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS)
        }
        .unwrap();
        supervisor
            .set_snapshot(before.revision, Some(snapshot()))
            .unwrap();
        let canonical = root.bytes();
        let offered = plan(port.next(PROMOTION));
        assert!(!offered.can_promote());
        assert_eq!(
            offered.block_reason(),
            Some(if paid {
                FileCredibilityPromotionBlockReason::NumericalReplayLimit
            } else {
                FileCredibilityPromotionBlockReason::NumericalReplayUnsupported
            })
        );
        let expected = if paid {
            FileCredibilityPromotionError::Capacity
        } else {
            FileCredibilityPromotionError::Unsupported
        };
        let input = FileCredibilityPromotionRequest::new(PROMOTION, offered.basis()).unwrap();
        assert_eq!(port.promote(input), Err(expected));
        assert_eq!(port.clone().promote(input), Err(expected));
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(grant.remaining_operations(), 1);
        assert_eq!(
            grant.remaining_numerical_replay_events(),
            paid.then_some(cost - 1)
        );
        assert_eq!(root.bytes(), canonical);
        {
            let host = supervisor.host().unwrap();
            assert_eq!(host.inspect(), before);
            assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
            assert_eq!(
                host.credibility_promotion(PROMOTION).err(),
                Some(JournalError::Contract(Error::Missing))
            );
            roles.evaluator.check_owner(&host).unwrap();
        }
        // Both the query and refused mutation preserved the one admission
        // observation, the existing identity, and the original learned gate.
        let proposed = proposal(&supervisor.host().unwrap());
        let _ticket = actor.submit(701, &proposed).unwrap();
    }
}

#[test]
fn eligible_next_preserves_snapshot_and_later_actor_work_stales_the_paid_predecessor() {
    let (root, host, roles) = qualified();
    let before = host.inspect();
    let numerical = host.learned_generation_inspection().unwrap();
    let cost = usize::try_from(before.revision).unwrap() + 1;
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .credibility_promotion_port_with_numerical_budget(
            1,
            MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
            cost,
        )
        .unwrap();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let canonical = root.bytes();
    let offered = plan(port.next(PROMOTION));
    assert!(offered.can_promote());
    assert_eq!(root.bytes(), canonical);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(cost));
    assert_eq!(
        port.clone().next(PROMOTION),
        Err(FileCredibilityPromotionError::Capacity)
    );
    let proposed = proposal(&supervisor.host().unwrap());
    let _ticket = actor.submit(701, &proposed).unwrap();
    let current = supervisor.host().unwrap().inspect();
    assert!(current.revision > before.revision);
    let canonical = root.bytes();
    assert_eq!(
        port.promote(offered.request().unwrap()),
        Err(FileCredibilityPromotionError::StalePredecessor)
    );
    assert_eq!(supervisor.host().unwrap().inspect(), current);
    assert_eq!(root.bytes(), canonical);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(cost));
    let host = supervisor.host().unwrap();
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        numerical.numerical
    );
    roles.evaluator.check_owner(&host).unwrap();
}
