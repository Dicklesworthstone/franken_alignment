//! Native generated text observed and controlled through its original owner.
//! Actual inference and two-key publication are exercised; fixture monitors and
//! synthetic helper verdicts are not evidence of detector quality or isolation.
#![cfg(unix)]

#[path = "tower_generated_operator/fixture.rs"]
mod fixture;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        Reconciliation, RecoveryReserve,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            stream::generated::FileTextMessageRequest,
        },
        requests::actor::{
            FileFenceBasis, FileFenceError, FileGeneratedTextActorPort, FileInspectionPage,
            FileResolutionBlockReason, FileResolutionError, MAX_FENCE_REPLAY_EVENTS,
            MAX_RESOLUTION_REPLAY_EVENTS,
        },
    },
    stream::{ReleaseFrame, StreamProfile},
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorOutcome, ActorProposal, Knowledge as ActorKnowledge,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use fixture::{begin, create, generate, profile};
use support::{Directory, Supervisor, observe, oversight, snapshot};

const REQUEST: u64 = 701;
const OPERATION: u64 = 5009;

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(41)).unwrap()
}
fn access(operator: bool, permit: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        if permit {
            vec![PurposeTag::Permit, PurposeTag::Experiment]
        } else {
            Vec::new()
        },
        0,
    )
}
fn situation(tower: &mut PublicationTower) -> PublicationResponse {
    tower.exchange(
        &access(true, false),
        encode_request(&TowerRequest::new(
            "native-observation",
            mount(),
            Verb::Situation,
        ))
        .unwrap()
        .as_bytes(),
    )
}
fn known(response: &PublicationResponse) -> &FileInspectionPage {
    let PublicationResult::Situation(Ok(ActorKnowledge::Known { value, basis })) = &response.result
    else {
        panic!("expected original situation: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.scope, profile().delivery.scope);
    assert_eq!(response.envelope.reason_code, None);
    value
}
fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}
fn private_data_withheld(response: &PublicationResponse) {
    let json = response.to_json().unwrap();
    for forbidden in [
        "PRIVATE-SOURCE-BYTES",
        "\"payload\"",
        "\"logits\"",
        "\"token_ids\"",
        "\"permit\"",
    ] {
        assert!(
            !json.contains(forbidden),
            "operator disclosed {forbidden}: {json}"
        );
    }
}

#[test]
fn native_observation_preserves_real_generation_and_original_two_key_publication() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer) = create(&root);
    let mut tower = PublicationTower::new_generated_observer(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    tower.set_inspection(Some(port)).unwrap();
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let original = image(&root);
    let initial = situation(&mut tower);
    assert_eq!(known(&initial).summary().attempts, 0);
    assert_eq!(image(&root), original);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );

    let source = generate(&mut supervisor);
    let numerical = supervisor.host().unwrap().decoder_inspection().unwrap();
    let original = image(&root);
    let generated = situation(&mut tower);
    assert_eq!(known(&generated).summary().executions, 0);
    assert_eq!(image(&root), original);
    assert_eq!(
        supervisor.host().unwrap().decoder_inspection().unwrap(),
        numerical
    );
    observe(&mut supervisor);
    let ticket = actor.submit(&source).unwrap();
    let pending = situation(&mut tower);
    assert_eq!(known(&pending).summary().pending_attempts, 1);
    assert_eq!(
        known(&pending).attempts()[0].external_request,
        Some(REQUEST)
    );
    assert_eq!(known(&pending).attempts()[0].stage, ActionState::Reviewing);
    assert_eq!(known(&pending).summary().executions, 0);
    let keys = support::ready(&mut supervisor, &reviewer, REQUEST);
    assert_eq!(
        ReleaseFrame::decode(&keys.action.spec().payload)
            .unwrap()
            .message(),
        Some("A")
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let outcome = host
            .publish_checked(
                host.revision(),
                keys.automatic.attempt(),
                Some(&keys.inputs),
                snapshot(),
                ElapsedTick(2),
            )
            .unwrap();
        assert!(matches!(outcome.outcome, EndpointOutcome::Executed { .. }));
        host.reconcile(host.revision(), keys.automatic.attempt())
            .unwrap();
    }
    let original = image(&root);
    let completed = situation(&mut tower);
    assert_eq!(known(&completed).summary().executions, 1);
    assert_eq!(
        known(&completed).attempts()[0].stage,
        ActionState::Confirmed
    );
    assert_eq!(
        known(&completed).summary().charged_units,
        keys.action.spec().units
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(image(&root), original);
    assert!(matches!(
        actor.poll(&ticket),
        ActorKnowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
    assert_eq!(supervisor.host().unwrap().inspect().payload, b"A");
    for response in [&initial, &generated, &pending, &completed] {
        private_data_withheld(response);
    }
}

#[test]
fn all_generated_mounts_keep_actor_intake_and_unapproved_source_references_closed() {
    let root = Directory::new();
    let (actor, mut supervisor, _) = create(&root);
    let source = generate(&mut supervisor);
    observe(&mut supervisor);
    let state = supervisor.host().unwrap().inspect();
    let original = image(&root);
    let constructors = [
        PublicationTower::new_generated_observer,
        PublicationTower::new_generated_reconciliation_operator,
        PublicationTower::new_generated_control_operator,
    ];
    for constructor in constructors {
        let mut tower = constructor(mount(), actor.clone(), PublicationLimits::default()).unwrap();
        let (port, grant) = supervisor.inspection_port(1).unwrap();
        tower.set_inspection(Some(port)).unwrap();
        for payload in [
            b"actor-selected replacement".to_vec(),
            FileGeneratedTextActorPort::encode_message(&source)
                .unwrap()
                .payload,
        ] {
            let mut request = TowerRequest::new("no-native-intake", mount(), Verb::Propose);
            request.idempotency_key = Some(REQUEST.to_string());
            request.arguments = ReadArguments::Propose {
                request: REQUEST,
                proposal: ActorProposal {
                    target: state.target,
                    units: payload.len() as u64,
                    payload,
                    deadline: source.deadline,
                    expected_policy_epoch: source.policy_epoch,
                },
            };
            for response in [
                tower.dispatch(&access(true, true), request.clone()),
                tower.exchange(
                    &access(true, true),
                    encode_request(&request).unwrap().as_bytes(),
                ),
            ] {
                assert_eq!(
                    response.envelope.reason_code,
                    Some(ReasonCode::CapabilityUnsupported)
                );
                assert!(matches!(response.result, PublicationResult::Refused(_)));
            }
        }
        for verb in [Verb::Get, Verb::Precheck, Verb::Capabilities, Verb::Convene] {
            let response = tower.dispatch(
                &access(true, true),
                TowerRequest::new("closed", mount(), verb),
            );
            assert_eq!(
                response.envelope.reason_code,
                Some(ReasonCode::CapabilityUnsupported)
            );
        }
        let denied = tower.dispatch(
            &access(false, true),
            TowerRequest::new("no-operator", mount(), Verb::Situation),
        );
        assert_eq!(
            denied.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
        assert_eq!(grant.remaining_queries(), 1);
        assert_eq!(known(&situation(&mut tower)).summary().attempts, 0);
        assert_eq!(image(&root), original);
        assert_eq!(supervisor.host().unwrap().inspect(), state);
    }
    // Rejected Tower proposals did not consume the independent original source
    // observation or turn its protected result into a different actor request.
    let ticket = actor.submit(&source).unwrap();
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: REQUEST }
    );
}

#[test]
fn generated_observers_share_finite_original_grants_and_cannot_rebind_or_retain_owner() {
    let root = Directory::new();
    let foreign_root = Directory::new();
    let (actor, supervisor, _) = create(&root);
    let (_, foreign, _) = create(&foreign_root);
    let mut first = PublicationTower::new_generated_observer(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    let mut second =
        PublicationTower::new_generated_observer(mount(), actor, PublicationLimits::default())
            .unwrap();
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    first.set_inspection(Some(port.clone())).unwrap();
    second.set_inspection(Some(port)).unwrap();
    assert_eq!(known(&situation(&mut first)).summary().attempts, 0);
    assert_eq!(known(&situation(&mut second)).summary().attempts, 0);
    assert_eq!(grant.remaining_queries(), 0);
    assert!(matches!(
        situation(&mut first).result,
        PublicationResult::Situation(Err(ActorError::Capacity))
    ));
    let (port, revoked) = supervisor.inspection_port(1).unwrap();
    first.set_inspection(Some(port)).unwrap();
    revoked.revoke();
    assert!(matches!(
        situation(&mut first).result,
        PublicationResult::Situation(Ok(ActorKnowledge::Withheld { .. }))
    ));
    let (port, _foreign_grant) = foreign.inspection_port(1).unwrap();
    assert_eq!(
        first.set_inspection(Some(port)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        situation(&mut first).envelope.reason_code,
        Some(ReasonCode::CapabilityUnsupported)
    );
    let (port, retained) = supervisor.inspection_port(1).unwrap();
    second.set_inspection(Some(port)).unwrap();
    drop(supervisor);
    assert!(matches!(
        situation(&mut second).result,
        PublicationResult::Situation(Ok(ActorKnowledge::Unknown { .. }))
    ));
    assert_eq!(retained.remaining_queries(), 1);
}

fn fence_command(supervisor: &Supervisor) -> TowerRequest {
    let host = supervisor.host().unwrap();
    let state = host.inspect();
    let mut request = TowerRequest::new("native-admission-stop", mount(), Verb::Fence);
    request.idempotency_key = Some(OPERATION.to_string());
    request.arguments = ReadArguments::Fence {
        operation: OPERATION,
        expected: FileFenceBasis {
            scope: profile().delivery.scope,
            journal_revision: host.revision(),
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
        },
    };
    request
}

#[test]
fn generated_control_fences_active_native_work_only_with_the_original_complete_numerical_allowance()
{
    let root = Directory::new();
    let (actor, mut supervisor, _) = create(&root);
    begin(&mut supervisor);
    {
        let mut host = supervisor.host_mut().unwrap();
        host.advance_decoder_text(host.revision(), 7, 0).unwrap();
    }
    let command = fence_command(&supervisor);
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor
        .host()
        .unwrap()
        .decoder_inspection()
        .unwrap()
        .numerical;
    let original = image(&root);
    let cost = before.revision as usize + 1;
    let mut tower = PublicationTower::new_generated_control_operator(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    let (port, ordinary) = supervisor.fence_port(MAX_FENCE_REPLAY_EVENTS).unwrap();
    tower.set_fence(Some(port)).unwrap();
    let denied = tower.dispatch(&access(true, true), command.clone());
    let PublicationResult::Fence(denied) = denied.result else {
        panic!("native fence result");
    };
    assert_eq!(denied.native(), &Err(FileFenceError::Unsupported));
    assert_eq!(ordinary.remaining_operations(), 1);
    let (port, short) = supervisor
        .fence_port_with_numerical_budget(MAX_FENCE_REPLAY_EVENTS, cost - 1)
        .unwrap();
    tower.set_fence(Some(port)).unwrap();
    let denied = tower.dispatch(&access(true, true), command.clone());
    let PublicationResult::Fence(denied) = denied.result else {
        panic!("native fence result");
    };
    assert_eq!(denied.native(), &Err(FileFenceError::Capacity));
    assert_eq!(short.remaining_operations(), 1);
    assert_eq!(short.remaining_numerical_replay_events(), Some(cost - 1));
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect(), before);

    let (port, grant) = supervisor
        .fence_port_with_numerical_budget(MAX_FENCE_REPLAY_EVENTS, cost)
        .unwrap();
    tower.set_fence(Some(port.clone())).unwrap();
    for constructor in [
        PublicationTower::new_generated_observer,
        PublicationTower::new_generated_reconciliation_operator,
    ] {
        let mut legacy = constructor(mount(), actor.clone(), PublicationLimits::default()).unwrap();
        legacy.set_fence(Some(port.clone())).unwrap();
        assert_eq!(
            legacy
                .dispatch(&access(true, true), command.clone())
                .envelope
                .reason_code,
            Some(ReasonCode::CapabilityUnsupported)
        );
    }
    assert_eq!(
        tower
            .dispatch(&access(true, false), command.clone())
            .envelope
            .reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    assert_eq!(grant.remaining_operations(), 1);
    let acknowledged = tower.dispatch(&access(true, true), command.clone());
    let PublicationResult::Fence(linked) = &acknowledged.result else {
        panic!("native fence result");
    };
    let Ok(ActorKnowledge::Known { value, .. }) = linked.native() else {
        panic!("{acknowledged:?}");
    };
    assert_eq!(value.after().journal_revision, before.revision + 1);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    let stopped = supervisor.host().unwrap().inspect();
    assert!(stopped.stop.is_some());
    assert_eq!(stopped.executions, 0);
    assert_eq!(stopped.control.ledger.charged, 0);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap()
            .numerical,
        numerical
    );
    let original = image(&root);
    let mut reconnected = PublicationTower::new_generated_control_operator(
        mount(),
        actor,
        PublicationLimits::default(),
    )
    .unwrap();
    reconnected.set_fence(Some(port)).unwrap();
    let repeated = reconnected.dispatch(&access(true, true), command);
    assert_eq!(repeated.result, acknowledged.result);
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect(), stopped);
}

fn next(tower: &mut PublicationTower) -> PublicationResponse {
    let mut request = TowerRequest::new(
        "native-receipt",
        tower.request_address(REQUEST).unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::ResolutionNext {
        operation: OPERATION,
    };
    tower.dispatch(&access(true, true), request)
}

#[test]
fn native_reconciliation_observes_one_real_publication_with_separately_bounded_replay() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer) = create(&root);
    let source = generate(&mut supervisor);
    observe(&mut supervisor);
    let ticket = actor.submit(&source).unwrap();
    let keys = support::ready(&mut supervisor, &reviewer, REQUEST);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let outcome = host
            .publish_checked(
                host.revision(),
                keys.automatic.attempt(),
                Some(&keys.inputs),
                snapshot(),
                ElapsedTick(2),
            )
            .unwrap();
        assert!(matches!(outcome.outcome, EndpointOutcome::Executed { .. }));
    }
    let original = image(&root);
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor
        .host()
        .unwrap()
        .decoder_inspection()
        .unwrap()
        .numerical;
    let cost = before.revision as usize + 1;
    let mut capable = PublicationTower::new_generated_reconciliation_operator(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    let (port, granted) = supervisor
        .resolution_port_with_numerical_budget(4, 1, MAX_RESOLUTION_REPLAY_EVENTS, cost)
        .unwrap();
    capable.set_resolution(Some(port.clone())).unwrap();
    let plan = next(&mut capable);
    let PublicationResult::ResolutionNext(linked) = &plan.result else {
        panic!("native plan");
    };
    let command = linked.action().unwrap().unwrap();
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    for budget in [None, Some(cost - 1)] {
        let mut closed = PublicationTower::new_generated_reconciliation_operator(
            mount(),
            actor.clone(),
            PublicationLimits::default(),
        )
        .unwrap();
        let (port, grant) = match budget {
            None => supervisor
                .resolution_port(2, 1, MAX_RESOLUTION_REPLAY_EVENTS)
                .unwrap(),
            Some(budget) => supervisor
                .resolution_port_with_numerical_budget(2, 1, MAX_RESOLUTION_REPLAY_EVENTS, budget)
                .unwrap(),
        };
        closed.set_resolution(Some(port)).unwrap();
        let response = next(&mut closed);
        let PublicationResult::ResolutionNext(linked) = &response.result else {
            panic!("native plan");
        };
        let Ok(ActorKnowledge::Known { value, .. }) = linked.native() else {
            panic!("{response:?}");
        };
        assert_eq!(
            value.block_reason(),
            Some(if budget.is_some() {
                FileResolutionBlockReason::NumericalReplayLimit
            } else {
                FileResolutionBlockReason::NumericalReplayUnsupported
            })
        );
        let refused = closed.dispatch(&access(true, true), command.clone());
        let PublicationResult::Resolution(linked) = &refused.result else {
            panic!("native resolution");
        };
        assert_eq!(
            linked.native(),
            &Err(if budget.is_some() {
                FileResolutionError::Capacity
            } else {
                FileResolutionError::Unsupported
            })
        );
        assert_eq!(grant.remaining_operations(), 1);
        assert_eq!(grant.remaining_numerical_replay_events(), budget);
    }
    assert_eq!(image(&root), original);
    let acknowledged = capable.dispatch(&access(true, true), command.clone());
    let PublicationResult::Resolution(linked) = &acknowledged.result else {
        panic!("native resolution");
    };
    let Ok(ActorKnowledge::Known { value, .. }) = linked.native() else {
        panic!("{acknowledged:?}");
    };
    assert_eq!(value.stage(), ActionState::Confirmed);
    assert!(matches!(
        value.outcome(),
        Reconciliation::Resolved(EndpointOutcome::Executed { .. })
    ));
    assert_eq!(granted.remaining_operations(), 0);
    assert_eq!(granted.remaining_numerical_replay_events(), Some(0));
    assert!(matches!(
        actor.poll(&ticket),
        ActorKnowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap()
            .numerical,
        numerical
    );
    let original = image(&root);
    let mut reconnected = PublicationTower::new_generated_reconciliation_operator(
        mount(),
        actor,
        PublicationLimits::default(),
    )
    .unwrap();
    reconnected.set_resolution(Some(port)).unwrap();
    assert_eq!(
        reconnected.dispatch(&access(true, true), command).result,
        acknowledged.result
    );
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect(), after);
    private_data_withheld(&acknowledged);
}
