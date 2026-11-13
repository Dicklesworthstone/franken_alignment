//! Governed policy changes through the original running driver and two-key gate.
#![cfg(unix)]
#[path = "support/file_driver.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{
    FileDriverEvent, FileDriverPhase,
};
use fa_reference::action::consequence::delivery::persistent::observed::governance::campaigns::{
    FilePolicyGovernor, FilePolicyPromotionPermit,
};
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, Reconciliation, governance::PolicyUpdate,
};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::helper_client::ClientPhase;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fixture::*;

fn guarded() -> (Rig, FilePolicyGovernor) {
    let mut rig = Rig::new();
    let governor = {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_policy_campaigns(
            revision,
            ReplayLimits {
                cases: 16,
                input_bytes: 1_048_576,
            },
            8,
        )
        .unwrap()
    };
    (rig, governor)
}

fn current_revision(rig: &Rig) -> u64 {
    rig.driver.supervisor().host().unwrap().revision()
}

fn policy(generation: u64) -> Policy {
    Policy::new(generation, profile().delivery.policy.nodes().to_vec()).unwrap()
}

fn approve(
    rig: &mut Rig,
    governor: &FilePolicyGovernor,
    operation: u64,
    candidate: Policy,
) -> FilePolicyPromotionPermit {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let control = host.inspect().control;
    let update =
        PolicyUpdate::new(operation, control.sequence, control.ledger.epoch, candidate).unwrap();
    let revision = host.revision();
    let review = host.request_policy_campaign(revision, &update).unwrap();
    let revision = host.revision();
    governor
        .approve(&mut host, revision, &review, false)
        .unwrap()
}

fn stop_without_observations(rig: &mut Rig) {
    assert!(matches!(
        rig.driver
            .step_with_evidence(
                || panic!("cancelled review sampled a clock"),
                |_, _| panic!("cancelled review fetched new evidence"),
                None,
            )
            .unwrap(),
        FileDriverEvent::Stopped {
            request: 1,
            stage: ActionState::Cancelled,
        }
    ));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
}

fn publish_reviewed(rig: &mut Rig, request: u64) {
    assert!(matches!(
        rig.step(None),
        FileDriverEvent::AwaitingHuman { .. }
    ));
    assert_eq!(
        rig.driver.supervisor().host().unwrap().inspect().executions,
        0
    );
    let human = rig.human(request + 1000, 31);
    assert!(matches!(
        rig.step(Some(&human)),
        FileDriverEvent::Dispatched { .. }
    ));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { .. }));
    assert_eq!(
        rig.driver.supervisor().host().unwrap().inspect().executions,
        1
    );
}

#[test]
fn promotion_retires_original_socket_workers_before_new_observations_and_new_work_publishes() {
    let (mut rig, governor) = guarded();
    let old = rig.submit(1);
    rig.start(1, 101);
    // Both actual HelperClients consume their complete original request, then
    // wait for inference without supplying any synthetic success to the host.
    for _ in 0..32 {
        assert!(matches!(rig.step(None), FileDriverEvent::Workers { .. }));
        for client in rig.clients.values_mut() {
            client.step().unwrap();
        }
        if rig
            .clients
            .values()
            .all(|client| client.phase() == ClientPhase::NeedsInference)
        {
            break;
        }
    }
    assert!(
        rig.clients
            .values()
            .all(|client| client.phase() == ClientPhase::NeedsInference)
    );
    let key = approve(&mut rig, &governor, 10, policy(2));
    let revision = current_revision(&rig);
    let receipt = rig.driver.promote_policy_campaign(revision, &key).unwrap();
    assert_eq!(receipt.change().cancelled, vec![1]);
    assert!(matches!(
        rig.port.poll(&old),
        Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            ..
        }
    ));
    // Cleanup happened in promotion, before the caller requests another step.
    for (member, client) in &mut rig.clients {
        client
            .respond(Verdict::Allow, &helper::oversight::salt(member))
            .unwrap();
        assert!(client.step().is_err());
    }
    stop_without_observations(&mut rig);
    let current = rig.submit(2);
    rig.reviewed(2);
    publish_reviewed(&mut rig, 2);
    assert!(matches!(
        rig.port.poll(&current),
        Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
    assert_eq!(
        rig.driver
            .supervisor()
            .host()
            .unwrap()
            .current_policy()
            .unwrap(),
        &policy(2)
    );
}

#[test]
fn promotion_withdraws_the_retained_reservation_and_independent_human_key_exactly_once() {
    let (mut rig, governor) = guarded();
    rig.submit(1);
    rig.reviewed(1);
    let rightful = rig.human(1001, 31);
    let (mut foreign, _) = guarded();
    foreign.submit(1);
    foreign.reviewed(1);
    let wrong = foreign.human(1001, 31);
    let inputs = rig.inputs.clone();
    assert_eq!(
        rig.driver
            .step_with_evidence(
                || ElapsedTick(1),
                |_, _| Ok(DriverEvidence {
                    snapshot: snapshot(),
                    inputs: inputs.clone()
                }),
                Some(&wrong),
            )
            .unwrap_err(),
        JournalError::Contract(Error::Binding)
    );
    assert_eq!(
        rig.driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .control
            .ledger
            .reserved,
        16
    );
    let key = approve(&mut rig, &governor, 10, policy(2));
    let revision = current_revision(&rig);
    let receipt = rig.driver.promote_policy_campaign(revision, &key).unwrap();
    assert_eq!(receipt.change().refunded_units, 16);
    {
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(
            host.human_status(rightful.request()).unwrap().disposition,
            HumanDisposition::Revoked
        );
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
    let after = current_revision(&rig);
    assert_eq!(
        rig.driver.promote_policy_campaign(0, &key).unwrap(),
        receipt
    );
    assert_eq!(current_revision(&rig), after);
    assert!(matches!(
        rig.step(Some(&rightful)),
        FileDriverEvent::Stopped { .. }
    ));
    let current = rig.submit(2);
    rig.reviewed(2);
    let inputs = rig.inputs.clone();
    assert!(
        rig.driver
            .step_with_evidence(
                || ElapsedTick(1),
                |_, _| Ok(DriverEvidence {
                    snapshot: snapshot(),
                    inputs: inputs.clone()
                }),
                Some(&rightful),
            )
            .is_err()
    );
    assert_eq!(
        rig.driver.supervisor().host().unwrap().inspect().executions,
        0
    );
    publish_reviewed(&mut rig, 2);
    assert!(matches!(
        rig.port.poll(&current),
        Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
}

#[test]
fn historical_promotion_retry_preserves_a_newer_live_review_and_its_fresh_keys() {
    let (mut rig, governor) = guarded();
    rig.submit(1);
    rig.start(1, 101);
    let key = approve(&mut rig, &governor, 10, policy(2));
    let revision = current_revision(&rig);
    let receipt = rig.driver.promote_policy_campaign(revision, &key).unwrap();
    assert_eq!(receipt.change().cancelled, vec![1]);
    stop_without_observations(&mut rig);
    rig.submit(2);
    rig.start(2, 102);
    let before = current_revision(&rig);
    assert_eq!(
        rig.driver.promote_policy_campaign(0, &key).unwrap(),
        receipt
    );
    assert_eq!(current_revision(&rig), before);
    assert_eq!(
        rig.driver.phase(),
        FileDriverPhase::Reviewing { request: 2 }
    );
    assert!(matches!(
        rig.review_event(Verdict::Allow, false),
        FileDriverEvent::ReviewApplied { .. }
    ));
    publish_reviewed(&mut rig, 2);
}

#[test]
fn stale_revision_foreign_and_revoked_keys_cannot_cancel_the_original_healthy_review() {
    for refusal in ["revision", "foreign", "revoked"] {
        let (mut rig, governor) = guarded();
        rig.submit(1);
        rig.start(1, 101);
        let original = approve(&mut rig, &governor, 10, policy(2));
        let (mut foreign, foreign_governor) = guarded();
        foreign.submit(1);
        let wrong = approve(&mut foreign, &foreign_governor, 10, policy(2));
        if refusal == "revoked" {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let review = host.policy_campaign(10).unwrap();
            let revision = host.revision();
            governor.revoke(&mut host, revision, &review).unwrap();
        }
        let before = rig.driver.supervisor().host().unwrap().inspect();
        let revision = current_revision(&rig);
        let error = rig
            .driver
            .promote_policy_campaign(
                if refusal == "revision" {
                    revision - 1
                } else {
                    revision
                },
                if refusal == "foreign" {
                    &wrong
                } else {
                    &original
                },
            )
            .unwrap_err();
        assert_eq!(
            error,
            JournalError::Contract(match refusal {
                "revision" => Error::Stale,
                "foreign" => Error::Binding,
                _ => Error::Missing,
            })
        );
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
        assert_eq!(
            rig.driver.phase(),
            FileDriverPhase::Reviewing { request: 1 }
        );
        assert!(matches!(
            rig.review_event(Verdict::Allow, false),
            FileDriverEvent::ReviewApplied { .. }
        ));
        publish_reviewed(&mut rig, 1);
        assert_eq!(
            rig.driver
                .supervisor()
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
fn new_exact_denial_invalidates_campaign_without_changing_sequence_or_killing_healthy_workers() {
    let (mut rig, governor) = guarded();
    rig.submit(1);
    rig.start(1, 101);
    let key = approve(&mut rig, &governor, 10, policy(2));
    let sequence = {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let sequence = host.inspect().control.sequence;
        let mut denied = snapshot();
        denied.values.insert(7, b"bad".to_vec());
        let spec = helper::spec(&host, b"new exact denial");
        let revision = host.revision();
        host.propose(revision, 2, spec, denied).unwrap();
        assert_eq!(host.inspect().control.sequence, sequence);
        assert_eq!(
            host.inspect().control.ledger.stages[&2],
            ActionState::Denied
        );
        sequence
    };
    let before = rig.driver.supervisor().host().unwrap().inspect();
    let revision = current_revision(&rig);
    assert_eq!(
        rig.driver.promote_policy_campaign(revision, &key),
        Err(JournalError::Contract(Error::Stale))
    );
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(before.control.sequence, sequence);
    assert!(matches!(
        rig.review_event(Verdict::Allow, false),
        FileDriverEvent::ReviewApplied { .. }
    ));
    publish_reviewed(&mut rig, 1);
}

#[test]
fn dispatched_and_unknown_effects_keep_their_original_obligations_and_charges() {
    for unknown in [false, true] {
        let (mut rig, governor) = guarded();
        rig.submit(1);
        rig.reviewed(1);
        let human = rig.human(1001, 31);
        assert!(matches!(
            rig.step(Some(&human)),
            FileDriverEvent::Dispatched { .. }
        ));
        if unknown {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            assert_eq!(
                host.reconcile(revision, 1).unwrap(),
                Reconciliation::AwaitingResolution
            );
            assert_eq!(
                host.inspect().control.ledger.stages[&1],
                ActionState::Unknown
            );
        }
        let key = approve(
            &mut rig,
            &governor,
            10,
            Policy::new(2, vec![Predicate::PayloadAtMost(0)]).unwrap(),
        );
        let revision = current_revision(&rig);
        let receipt = rig.driver.promote_policy_campaign(revision, &key).unwrap();
        assert!(receipt.change().cancelled.is_empty());
        assert_eq!(receipt.change().refunded_units, 0);
        assert_eq!(
            rig.driver.phase(),
            FileDriverPhase::AwaitingPublication { request: 1 }
        );
        assert_eq!(
            rig.driver
                .supervisor()
                .host()
                .unwrap()
                .inspect()
                .control
                .ledger
                .charged,
            16
        );
        let event = rig
            .driver
            .step_with_evidence(
                || ElapsedTick(1),
                |_, _| panic!("admitted obligation reacquired evidence"),
                None,
            )
            .unwrap();
        if unknown {
            assert!(matches!(
                event,
                FileDriverEvent::Reconciled {
                    outcome: Reconciliation::AwaitingResolution,
                    ..
                }
            ));
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().control.ledger.charged, 16);
            let revision = host.revision();
            assert_eq!(
                host.seal_unexecuted(revision, 1).unwrap(),
                Reconciliation::Resolved(EndpointOutcome::NotExecuted {
                    reason: NonExecutionReason::Sealed
                })
            );
            assert_eq!(host.inspect().control.ledger.available, 100);
        } else {
            assert!(matches!(event, FileDriverEvent::Published { .. }));
            assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { .. }));
            let host = rig.driver.supervisor().host().unwrap();
            assert_eq!(host.inspect().executions, 1);
            assert_eq!(host.inspect().control.ledger.charged, 16);
        }
    }
}
