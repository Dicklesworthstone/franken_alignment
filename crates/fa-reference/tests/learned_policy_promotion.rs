//! Live promotion of the original learned owner requires separately paid replay.
//! Actual decoder output, native helper inference and the independent human key
//! reach the original endpoint. Synthetic model weights are test controls, not
//! evidence of detector effectiveness or organizational independence.

#[path = "support/file_oversight.rs"]
mod legacy;
#[path = "mediated_predictive_learned_recovery/native_fixture.rs"]
mod native_fixture;
#[path = "support/learned_text_model.rs"]
mod numerical;
#[path = "support/learned_policy_promotion.rs"]
mod support;

use fa_reference::Error;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{
    EndpointOutcome, StopRequest,
    persistent::{
        JournalError, RecoveryReserve,
        governance::PolicyUpdate,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            decoder::learned::FileLearnedConfig,
            driver::{
                FileDriverEvent, FileDriverPhase, FileSupervisedDriver,
                native_learned::{
                    FileNativeDriverEvent, FileNativeDriverLaunch, FileNativeSupervisedDriver,
                },
            },
            governance::campaigns::{FilePolicyGovernor, FilePolicyPromotionPermit},
            guarded::{FileCampaignRequirement, FileGuardSet},
            helpers::learned::native::{
                NativeReviewLimits, NativeReviewMember, NativeReviewStatus,
            },
        },
        requests::{
            FileRequestDisposition,
            actor::{
                FileActorSupervisor, FileLearnedTextStreamActorPort,
                FilePolicyPromotionBlockReason, FilePolicyPromotionError, FilePolicyPromotionPlan,
                FilePolicyPromotionPlanResult, FilePolicyPromotionRequest, LearnedTextRelease,
                MAX_POLICY_PROMOTION_REPLAY_EVENTS,
            },
        },
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::containment::{
    ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate},
};
use fa_reference::action::consequence::oversight::{
    CommitteeContract, HelperContract, ReviewWindow,
    actor::{ActorError, ActorOutcome, Knowledge},
    decoder_monitoring::LearnedDecoderBindingLimits,
    learned_host::sidecar::{
        LearnedSidecarRequest,
        workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS},
    },
    learned_source::LearnedEvidenceLimits,
    sidecar::{
        SidecarCongressBudget, SidecarIdentity,
        receiver::native::{SidecarDecisionBasis, SidecarProbeQuery},
    },
};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use std::collections::{BTreeMap, BTreeSet};
use support::*;

const CAMPAIGN: u64 = 901;
const PROMOTION: u64 = 1001;
const SEED: u64 = 900_001;
const REQUEST: u64 = 71;
type Supervisor = FileActorSupervisor<FileOversight>;

#[test]
fn paid_promotion_keeps_original_numerics_and_new_native_review_reaches_both_keys() {
    let Rig {
        root,
        actor,
        mut supervisor,
        human,
        governor,
    } = ready();
    let permit = approve(&mut supervisor, &governor);
    let before = supervisor.host().unwrap().inspect();
    let numerical_before = supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let cost = before.revision as usize + 1;
    let (port, grant) = supervisor
        .policy_promotion_port_with_numerical_budget(
            permit,
            8,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS,
            cost,
        )
        .unwrap();
    observe(&mut supervisor);
    let planned = plan(port.next(CAMPAIGN, PROMOTION));
    assert!(planned.can_promote());
    assert_eq!(planned.basis().scope, profile().delivery.scope);
    assert_eq!(planned.basis().journal_revision, before.revision);
    assert_eq!(planned.basis().recorded_at, Some(ElapsedTick(1)));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(cost));
    let request = planned.request().unwrap();
    let receipt = port.promote(request).unwrap();
    let Knowledge::Known { value, basis } = &receipt else {
        panic!("paid original promotion did not acknowledge: {receipt:?}");
    };
    assert_eq!(value.before(), planned.basis());
    assert_eq!(value.after(), *basis);
    assert_eq!(basis.policy_generation, 2);
    assert_eq!(basis.control_sequence, before.control.sequence + 1);
    assert_eq!(basis.authority_epoch, before.control.ledger.epoch + 1);
    assert_eq!(basis.journal_revision, before.revision + 1);
    assert!(
        value.cancelled().is_empty(),
        "the retained corpus case was denied"
    );
    assert_eq!(value.refunded_units(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(grant.remaining_operations(), 0);
    {
        let host = supervisor.host().unwrap();
        let after = host.inspect();
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.executions, before.executions);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(
            after.control.ledger.reserved,
            before.control.ledger.reserved
        );
        let current = host.learned_generation_inspection().unwrap();
        assert_eq!(current.numerical, numerical_before.numerical);
        assert_eq!(current.paused, numerical_before.paused);
        assert_eq!(current.pending, numerical_before.pending);
        assert_eq!(host.current_policy().unwrap(), &candidate());
    }
    // The consumed pre-promotion observation cannot admit new work in epoch 2.
    assert_eq!(
        actor
            .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
            .err(),
        Some(ActorError::Unavailable)
    );
    observe(&mut supervisor);
    let ticket = actor
        .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    assert!(matches!(
        actor.poll(&ticket),
        Knowledge::Pending { request: REQUEST }
    ));
    let mut run = start_review(FileSupervisedDriver::new(supervisor), REQUEST, 101);
    assert_eq!(run.progress().status, NativeReviewStatus::Running);
    let during = run.progress();
    let current = run.supervisor().host().unwrap().inspect();
    let canonical = bytes(&root);
    // The old acknowledgment is historical. It cannot cancel this newer
    // original review, clear its work records, or reconstruct history again.
    assert_eq!(port.clone().promote(request).unwrap(), receipt);
    assert_eq!(run.progress(), during);
    assert_eq!(run.supervisor().host().unwrap().inspect(), current);
    assert_eq!(bytes(&root), canonical);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    complete(&mut run);
    assert_eq!(
        run.progress().phase,
        FileDriverPhase::AwaitingDispatch { request: REQUEST }
    );
    assert!(matches!(
        tick(&mut run, None),
        FileDriverEvent::AwaitingHuman { .. }
    ));
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    let human_request = run
        .request_human_approval(2001, ElapsedTick(31), ElapsedTick(1))
        .unwrap();
    let key = {
        let mut host = run.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        human.approve(&mut host, revision, &human_request).unwrap()
    };
    assert!(matches!(
        tick(&mut run, Some(&key)),
        FileDriverEvent::Dispatched { .. }
    ));
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    let publication = tick(&mut run, None);
    assert!(
        matches!(
            publication,
            FileDriverEvent::PublicationChecked {
                request: REQUEST,
                publication: ref checked,
                source_failure: None,
            } if checked.outcome == EndpointOutcome::Executed { resulting_version: 2 }
        ),
        "expected the original guarded publication, received {publication:?}"
    );
    assert!(matches!(
        tick(&mut run, None),
        FileDriverEvent::Reconciled { .. }
    ));
    let host = run.supervisor().host().unwrap();
    assert_eq!(host.stream_snapshot().unwrap().published.visible(), b"OK");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(
        host.inspect().control.ledger.charged,
        host.request_action(REQUEST).unwrap().spec().units
    );
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        numerical_before.numerical
    );
    assert!(matches!(
        actor.poll(&ticket),
        Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
}

#[test]
fn absent_or_one_short_numerical_allowance_refuses_before_effects_and_snapshot_loss() {
    for paid in [false, true] {
        let Rig {
            root,
            actor,
            mut supervisor,
            governor,
            ..
        } = ready();
        let permit = approve(&mut supervisor, &governor);
        let cost = supervisor.host().unwrap().revision() as usize + 1;
        let (port, grant) = if paid {
            supervisor.policy_promotion_port_with_numerical_budget(
                permit,
                4,
                MAX_POLICY_PROMOTION_REPLAY_EVENTS,
                cost - 1,
            )
        } else {
            supervisor.policy_promotion_port(permit, 4, MAX_POLICY_PROMOTION_REPLAY_EVENTS)
        }
        .unwrap();
        observe(&mut supervisor);
        let before = supervisor.host().unwrap().inspect();
        let numerical_before = supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap();
        let canonical = bytes(&root);
        let next = plan(port.next(CAMPAIGN, PROMOTION));
        assert_eq!(
            next.block_reason(),
            Some(if paid {
                FilePolicyPromotionBlockReason::NumericalReplayLimit
            } else {
                FilePolicyPromotionBlockReason::NumericalReplayUnsupported
            })
        );
        assert!(next.request().is_none());
        let input = FilePolicyPromotionRequest::new(PROMOTION, next.basis()).unwrap();
        assert_eq!(
            port.promote(input),
            Err(if paid {
                FilePolicyPromotionError::Capacity
            } else {
                FilePolicyPromotionError::Unsupported
            })
        );
        assert_eq!(grant.remaining_operations(), 1);
        assert_eq!(grant.remaining_queries(), 3);
        assert_eq!(
            grant.remaining_numerical_replay_events(),
            paid.then_some(cost - 1)
        );
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(
            supervisor
                .host()
                .unwrap()
                .learned_generation_inspection()
                .unwrap(),
            numerical_before
        );
        assert_eq!(bytes(&root), canonical);
        // This real admission, without refreshing the one-shot snapshot,
        // distinguishes preflight refusal from an entered failed mutation.
        let ticket = actor
            .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
            .unwrap();
        assert!(matches!(
            actor.poll(&ticket),
            Knowledge::Pending { request: REQUEST }
        ));
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
}

#[test]
fn entered_storage_failure_keeps_numerical_work_spent_and_never_retries_the_transition() {
    let Rig {
        root,
        mut supervisor,
        governor,
        ..
    } = ready();
    let permit = approve(&mut supervisor, &governor);
    let cost = supervisor.host().unwrap().revision() as usize + 1;
    let (port, grant) = supervisor
        .policy_promotion_port_with_numerical_budget(
            permit,
            4,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS,
            cost,
        )
        .unwrap();
    let input = plan(port.next(CAMPAIGN, PROMOTION)).request().unwrap();
    let before = supervisor.host().unwrap().inspect();
    let canonical = bytes(&root);
    let pending = root.store().join("delivery.pending");
    std::fs::create_dir(&pending).unwrap();
    assert_eq!(
        port.promote(input),
        Err(FilePolicyPromotionError::Unavailable)
    );
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(bytes(&root), canonical);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert!(pending.is_dir());
    std::fs::remove_dir(&pending).unwrap();
    // Repairing the pathname does not reissue an entered operation or budget.
    // This owner may conservatively remain unavailable after its I/O failure.
    assert!(matches!(
        port.clone().promote(input),
        Ok(Knowledge::Unknown { .. }) | Err(FilePolicyPromotionError::Unavailable)
    ));
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(bytes(&root), canonical);
}

#[test]
fn paid_delegation_keeps_original_issuer_scope_and_shared_weak_custody() {
    let Rig {
        root,
        actor,
        mut supervisor,
        governor,
        ..
    } = ready();
    let Rig {
        root: _other_root,
        supervisor: mut other,
        governor: other_governor,
        ..
    } = ready();
    let foreign = approve(&mut other, &other_governor);
    let canonical = bytes(&root);
    assert_eq!(
        supervisor
            .policy_promotion_port_with_numerical_budget(
                foreign,
                8,
                MAX_POLICY_PROMOTION_REPLAY_EVENTS,
                MAX_POLICY_PROMOTION_REPLAY_EVENTS,
            )
            .err(),
        Some(JournalError::Contract(Error::Binding))
    );
    assert_eq!(bytes(&root), canonical);
    let permit = approve(&mut supervisor, &governor);
    let cost = supervisor.host().unwrap().revision() as usize + 1;
    let (port, grant) = supervisor
        .policy_promotion_port_with_numerical_budget(
            permit,
            4,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS,
            cost,
        )
        .unwrap();
    let clone = port.clone();
    assert!(matches!(
        clone.next(CAMPAIGN + 1, PROMOTION),
        Ok(Knowledge::Withheld { .. })
    ));
    assert_eq!(grant.remaining_queries(), 4);
    let next = plan(clone.next(CAMPAIGN, PROMOTION));
    assert_eq!(grant.remaining_queries(), 3);
    let mut wrong = next.basis();
    wrong.scope.branch += 1;
    observe(&mut supervisor);
    let before = supervisor.host().unwrap().inspect();
    let canonical = bytes(&root);
    assert_eq!(
        port.promote(FilePolicyPromotionRequest::new(PROMOTION, wrong).unwrap()),
        Err(FilePolicyPromotionError::StalePredecessor)
    );
    {
        let held = supervisor.host().unwrap();
        assert!(matches!(
            port.promote(next.request().unwrap()),
            Ok(Knowledge::Unknown { .. })
        ));
        assert_eq!(held.inspect(), before);
    }
    assert_eq!(grant.remaining_numerical_replay_events(), Some(cost));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(bytes(&root), canonical);
    grant.revoke();
    assert!(matches!(
        port.next(CAMPAIGN, PROMOTION),
        Ok(Knowledge::Withheld { .. })
    ));
    assert!(matches!(
        clone.promote(next.request().unwrap()),
        Ok(Knowledge::Withheld { .. })
    ));
    assert_eq!(bytes(&root), canonical);
    // Rejected scope, a busy original owner and withdrawing the sole custodian
    // did not consume the separately supplied admission observation.
    assert!(
        actor
            .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
            .is_ok()
    );
}

#[test]
fn stop_remains_terminal_even_with_an_exact_paid_original_approval() {
    let Rig {
        root,
        mut supervisor,
        governor,
        ..
    } = ready();
    let permit = approve(&mut supervisor, &governor);
    let (port, grant) = supervisor
        .policy_promotion_port_with_numerical_budget(
            permit,
            4,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS,
        )
        .unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let state = host.inspect();
        host.request_stop(
            state.revision,
            StopRequest {
                operation: 3001,
                expected_control_sequence: state.control.sequence,
                expected_authority_epoch: state.control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let before = supervisor.host().unwrap().inspect();
    let canonical = bytes(&root);
    let next = plan(port.next(CAMPAIGN, PROMOTION));
    assert_eq!(
        next.block_reason(),
        Some(FilePolicyPromotionBlockReason::Stopped)
    );
    assert_eq!(
        port.promote(FilePolicyPromotionRequest::new(PROMOTION, next.basis()).unwrap()),
        Err(FilePolicyPromotionError::Unsupported)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(MAX_POLICY_PROMOTION_REPLAY_EVENTS)
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(bytes(&root), canonical);
}
