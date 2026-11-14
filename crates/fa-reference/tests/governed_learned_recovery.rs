//! Recovery keeps the original learned history and issues fresh governance
//! custody only after its complete independent guards and original Fence.
//! Synthetic weights exercise actual decoding and native review; they do not
//! establish detector accuracy, process isolation or production qualification.

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
    EndpointOutcome,
    persistent::{
        JournalError, RecoveryReserve,
        governance::PolicyUpdate,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus},
            driver::{
                FileDriverEvent, FileDriverPhase, FileSupervisedDriver,
                native_learned::{
                    FileNativeDriverEvent, FileNativeDriverLaunch, FileNativeSupervisedDriver,
                },
            },
            governance::campaigns::{FilePolicyGovernor, FilePolicyPromotionPermit},
            guarded::{
                FileCampaignRequirement, FileGuardSet, FileOversightRoles, FileRecoveryFloor,
                FileRecoveryRequirements, anchored::FileHistoryAnchor,
            },
            helpers::learned::native::{
                NativeReviewLimits, NativeReviewMember, NativeReviewStatus,
            },
        },
        requests::{
            FileRequestDisposition,
            actor::{
                FileActorSupervisor, FileLearnedTextStreamActorPort, FilePolicyPromotionPlan,
                FilePolicyPromotionPlanResult, LearnedTextRelease,
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
    policy_governance::CampaignDisposition,
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

const OLD_CAMPAIGN: u64 = 900;
const CAMPAIGN: u64 = 901;
const PROMOTION: u64 = 1001;
const SEED: u64 = 900_001;
const REQUEST: u64 = 71;
type Supervisor = FileActorSupervisor<FileOversight>;

fn requirements(host: &FileOversight) -> FileRecoveryRequirements {
    let state = host.inspect();
    FileRecoveryRequirements {
        guards: guards(),
        effective_policy: host.current_policy().unwrap().clone(),
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: state.revision,
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
        },
    }
}

fn recover(
    root: &Directory,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> Result<(FileOversight, FileOversightRoles), JournalError> {
    let mut cursor = FileOversight::begin_open_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        expected,
        &recipe(),
        anchor,
    )?
    .require_recovery_reserve(RecoveryReserve::terminal())?;
    let canonical = bytes(root);
    while cursor.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = cursor.progress();
        let after = cursor.advance(before.replayed_events, 1)?;
        assert!(after.replayed_events <= before.replayed_events + 1);
        assert_eq!(after.total_events, before.total_events);
        assert_eq!(bytes(root), canonical, "replay must not publish a Fence");
    }
    assert_eq!(cursor.progress().status, FileLearnedRecoveryStatus::Ready);
    assert_eq!(
        cursor.progress().replayed_events,
        cursor.progress().total_events
    );
    cursor.finish()
}

#[test]
fn fresh_recovered_governor_can_promote_and_publish_without_reviving_old_custody() {
    let Rig {
        root,
        actor: old_actor,
        mut supervisor,
        human: old_human,
        governor: old_governor,
    } = ready();
    let (old_review, old_approval) = {
        let mut host = supervisor.host_mut().unwrap();
        let state = host.inspect();
        let update = PolicyUpdate::new(
            OLD_CAMPAIGN,
            state.control.sequence,
            state.control.ledger.epoch,
            candidate(),
        )
        .unwrap();
        let revision = host.revision();
        let review = host.request_policy_campaign(revision, &update).unwrap();
        let revision = host.revision();
        let approval = old_governor
            .approve(&mut host, revision, &review, false)
            .unwrap();
        (review, approval)
    };
    let (expected, anchor, before, numerical_before) = {
        let host = supervisor.host().unwrap();
        (
            requirements(&host),
            host.history_anchor().unwrap(),
            host.inspect(),
            host.learned_generation_inspection().unwrap().numerical,
        )
    };
    drop(supervisor);

    let (mut host, roles) = recover(&root, &expected, &anchor).unwrap();
    assert_eq!(host.revision(), before.revision + 1);
    assert!(host.inspect().control.ledger.epoch > before.control.ledger.epoch);
    assert_eq!(host.current_policy().unwrap(), &expected.effective_policy);
    assert_eq!(host.inspect().executions, before.executions);
    assert_eq!(host.inspect().payload, before.payload);
    assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical_before);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert_eq!(
        host.policy_campaign(OLD_CAMPAIGN).unwrap().observed_disposition(),
        CampaignDisposition::Revoked
    );
    let governor = roles.policy_governor.expect("fresh original governor role");
    let fenced = host.inspect();
    let canonical = bytes(&root);
    assert_eq!(
        host.promote_policy_campaign(host.revision(), &old_approval),
        Err(JournalError::Contract(Error::Binding))
    );
    let revision = host.revision();
    assert_eq!(
        old_governor.approve(&mut host, revision, &old_review, false).err(),
        Some(JournalError::Contract(Error::Binding))
    );
    let revoked = host.policy_campaign(OLD_CAMPAIGN).unwrap();
    assert_eq!(
        old_governor.approve(&mut host, revision, &revoked, false).err(),
        Some(JournalError::Contract(Error::Binding))
    );
    assert_eq!(
        governor.approve(&mut host, revision, &revoked, false).err(),
        Some(JournalError::Contract(Error::WrongState))
    );
    assert_eq!(
        old_actor.submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100)).err(),
        Some(ActorError::Unavailable)
    );
    assert_eq!(host.inspect(), fenced);
    assert_eq!(bytes(&root), canonical);

    // Independent current time and explicit resume re-enable the original
    // completed draft, with no new sampled draw or numerical work allowance.
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.resume_learned_generation(
        host.revision(), numerical_before.actor_revision, numerical_before.position,
    ).unwrap();
    let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let permit = approve(&mut supervisor, &governor);
    let cost = supervisor.host().unwrap().revision() as usize + 1;
    let (port, grant) = supervisor.policy_promotion_port_with_numerical_budget(
        permit, 8, MAX_POLICY_PROMOTION_REPLAY_EVENTS, cost,
    ).unwrap();
    observe(&mut supervisor);
    let selected = plan(port.next(CAMPAIGN, PROMOTION));
    assert!(selected.can_promote());
    let request = selected.request().unwrap();
    let acknowledgment = port.promote(request).unwrap();
    assert!(matches!(&acknowledgment, Knowledge::Known { .. }));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(supervisor.host().unwrap().current_policy().unwrap(), &candidate());
    assert_eq!(
        supervisor.host().unwrap().learned_generation_inspection().unwrap().numerical,
        numerical_before
    );
    assert_eq!(
        actor.submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100)).err(),
        Some(ActorError::Unavailable),
        "promotion cannot preserve an old admission snapshot"
    );
    observe(&mut supervisor);
    let ticket = actor.submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let mut run = start_review(FileSupervisedDriver::new(supervisor), REQUEST, 101);
    complete(&mut run);
    assert_eq!(run.progress().phase, FileDriverPhase::AwaitingDispatch { request: REQUEST });
    assert!(matches!(tick(&mut run, None), FileDriverEvent::AwaitingHuman { .. }));
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
    let human_request = run.request_human_approval(2001, ElapsedTick(31), ElapsedTick(1)).unwrap();
    let key = {
        let mut host = run.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        let before = host.inspect();
        assert_eq!(
            old_human.approve(&mut host, revision, &human_request).err(),
            Some(JournalError::Contract(Error::Binding))
        );
        assert_eq!(host.inspect(), before);
        roles.human.approve(&mut host, revision, &human_request).unwrap()
    };
    assert!(matches!(tick(&mut run, Some(&key)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(
        tick(&mut run, None),
        FileDriverEvent::PublicationChecked {
            request: REQUEST, publication, source_failure: None,
        } if publication.outcome == EndpointOutcome::Executed { resulting_version: 2 }
    ));
    assert!(matches!(tick(&mut run, None), FileDriverEvent::Reconciled { .. }));
    let host = run.supervisor().host().unwrap();
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"OK");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, host.request_action(REQUEST).unwrap().spec().units);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical_before);
    assert!(matches!(actor.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let after = host.inspect();
    let canonical = bytes(&root);
    drop(host);
    assert_eq!(port.promote(request).unwrap(), acknowledgment);
    assert_eq!(run.supervisor().host().unwrap().inspect(), after);
    assert_eq!(bytes(&root), canonical);
}

#[test]
fn incomplete_or_changed_independent_recovery_requirements_never_publish_a_fence() {
    let Rig { root, supervisor, .. } = ready();
    let (expected, anchor) = {
        let host = supervisor.host().unwrap();
        (requirements(&host), host.history_anchor().unwrap())
    };
    drop(supervisor);
    let canonical = bytes(&root);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged staging remains intact").unwrap();
    for variation in 0..6 {
        let mut changed = expected.clone();
        match variation {
            0 => changed.guards.campaigns = None,
            1 => changed.guards.campaigns.as_mut().unwrap().max_campaigns += 1,
            2 => changed.guards.campaigns.as_mut().unwrap().limits.cases += 1,
            3 => changed.effective_policy = Policy::new(
                expected.effective_policy.generation(),
                vec![Predicate::PayloadAtMost(4096)],
            ).unwrap(),
            4 => changed.minimum.authority_epoch += 1,
            5 => changed.minimum.journal_revision += 1,
            _ => unreachable!(),
        }
        assert!(recover(&root, &changed, &anchor).is_err(), "variation {variation}");
        assert_eq!(bytes(&root), canonical, "variation {variation}");
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging remains intact");
    }
    let foreign = ready();
    let foreign_anchor = foreign.supervisor.host().unwrap().history_anchor().unwrap();
    assert!(recover(&root, &expected, &foreign_anchor).is_err());
    assert_eq!(bytes(&root), canonical);
    assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging remains intact");

    // The same complete expectations admit their unchanged original history;
    // refusal did not poison the store or spend its reserved recovery capacity.
    let (host, roles) = recover(&root, &expected, &anchor).unwrap();
    assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
    assert!(roles.policy_governor.is_some());
    assert!(!pending.exists());
    assert_eq!(host.inspect().executions, 0);
}
