//! Original topology custody, exact target restriction, fresh human approval
//! and actual publication through the public driver and Tower interfaces.
#![cfg(unix)]

#[path = "support/file_evidence_driver.rs"]
mod fixture;
#[path = "file_driver_mediated_narrowing/support.rs"]
mod support;

use fa_reference::Error;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight,
    mediation::source::{TopologySourceRefusal, TOPOLOGY_REFRESH_READ_BYTES},
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileNarrowError;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::tower::publication::PublicationResult;
use support::*;
use std::fs;

#[test]
fn mediated_retained_review_publishes_once_after_original_tower_narrowing_and_fresh_human_key() {
    let (mut file, _) = prepared(2, true);
    let old_human = human(&mut file, 1001);
    let retained_target = target(&file);
    let original_action = file.rig.driver.supervisor().host().unwrap()
        .request_action(REQUEST).unwrap().clone();
    let (mut tower, grant) = operator(&file);
    let before = file.rig.driver.supervisor().host().unwrap().inspect();
    let topology = file.rig.driver.supervisor().host().unwrap().mediation_snapshot().unwrap();
    let bytes = image(&file);
    let reads = file.source.status();
    let spent = work(&file);
    let requested = command(&mut tower, 501, vec![retained_target]);
    assert_eq!(image(&file), bytes);
    assert_eq!(file.source.status(), reads);
    assert_eq!(work(&file), spent);
    let narrowed = apply(&mut tower, &requested);
    let change = receipt(&narrowed);
    assert_eq!(change.effective_targets(), &[retained_target]);
    assert_eq!(change.refunded_units(), 0);
    assert!(change.cancelled().is_empty());
    let after = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.sequence, before.control.sequence + 1);
    assert_eq!(after.control.ledger, before.control.ledger);
    let current = file.rig.driver.supervisor().host().unwrap().mediation_snapshot().unwrap();
    assert_eq!(current.graph, topology.graph);
    assert_eq!(current.available, topology.available);
    assert_eq!(current.accepted, topology.accepted);
    assert_eq!(file.source.status(), reads);
    assert_eq!(work(&file), spent);
    let before_continuation = image(&file);
    assert_eq!(file.rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), Some(501));
    let continuation = file.rig.driver.continue_after_narrowing(REQUEST, 501).unwrap();
    assert_eq!(continuation.narrowing_operations(), &[501]);
    assert_eq!(continuation.authority_epoch(), before.control.ledger.epoch);
    assert_eq!(image(&file), before_continuation);
    assert_eq!(file.source.status(), reads);
    assert_eq!(work(&file), spent);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().human_status(1001)
        .unwrap().disposition, HumanDisposition::Revoked);
    assert!(file.rig.driver.step_from_file(
        &mut file.source, || ElapsedTick(1), Some(&old_human),
    ).result.is_err());
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    let fresh_human = human(&mut file, 1002);
    file.dispatch(&fresh_human);
    assert_eq!(work(&file), spent, "dispatch did not replace the final source check");
    assert!(matches!(publish_and_reconcile(&mut file), EndpointOutcome::Executed { .. }));
    let after = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, b"publication");
    assert_eq!(after.control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!((after.control.ledger.charged, after.control.ledger.reserved), (16, 0));
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().request_action(REQUEST).unwrap(), &original_action);
    assert_eq!(work(&file).checks, 2);
    assert_eq!(work(&file).reserved_read_bytes, 2 * TOPOLOGY_REFRESH_READ_BYTES);
    assert_eq!(file.rig.driver.topology().unwrap().source().last_operation(), 2);
    let replayed = FileOversight::read_publication(file.rig.root.store(), &fixture::driver::profile()).unwrap();
    assert_eq!(replayed, after);
    let bytes = image(&file);
    let spent = work(&file);
    let reads = file.source.status();
    assert_eq!(receipt(&apply(&mut tower, &requested)), receipt(&narrowed));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(image(&file), bytes);
    assert_eq!(work(&file), spent);
    assert_eq!(file.source.status(), reads);
}

#[test]
fn mediated_empty_ceiling_refunds_only_the_excluded_reservation_and_keeps_original_unknown_charge() {
    let (mut file, _) = prepared(2, true);
    let mut foreign = fixture::FileRig::new();
    foreign.reviewed();
    let foreign_human = foreign.human();
    assert!(file.rig.driver.step_from_file(
        &mut file.source, || ElapsedTick(1), Some(&foreign_human),
    ).result.is_err());
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 16);
    {
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        let keys = fixture::driver::helper::oversight::ready(
            &mut host, &file.rig.reviewer, 2, b"unknown",
        );
        fixture::driver::helper::oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        assert_eq!(host.reconcile(revision, 2).unwrap(), Reconciliation::AwaitingResolution);
    }
    let before = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!((before.control.ledger.reserved, before.control.ledger.charged), (16, 16));
    let topology = file.rig.driver.supervisor().host().unwrap().mediation_snapshot().unwrap();
    let spent = work(&file);
    let (mut tower, grant) = operator(&file);
    let requested = command(&mut tower, 502, Vec::new());
    let narrowed = apply(&mut tower, &requested);
    assert_eq!(receipt(&narrowed).cancelled(), &[1]);
    assert_eq!(receipt(&narrowed).refunded_units(), 16);
    assert!(receipt(&narrowed).effective_targets().is_empty());
    let after = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(after.control.ledger.stages[&2], ActionState::Unknown);
    assert_eq!((after.control.ledger.reserved, after.control.ledger.charged), (0, 16));
    assert_eq!(after.control.ledger.available, before.control.ledger.available + 16);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(after.executions, 0);
    assert_eq!(work(&file), spent);
    let current = file.rig.driver.supervisor().host().unwrap().mediation_snapshot().unwrap();
    assert_eq!(current.graph, topology.graph);
    assert_eq!(current.accepted, topology.accepted);
    assert!(file.rig.driver.continue_after_narrowing(REQUEST, 502).is_err());
    assert_eq!(receipt(&apply(&mut tower, &requested)), receipt(&narrowed));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(FileOversight::read_publication(file.rig.root.store(), &fixture::driver::profile())
        .unwrap(), after);
}

#[test]
fn a_mediated_continuation_requires_the_original_source_custodian_before_any_new_work() {
    let (mut file, detached) = prepared(2, false);
    let detached = detached.unwrap();
    let target = target(&file);
    let (mut tower, _grant) = operator(&file);
    let requested = command(&mut tower, 503, vec![target]);
    receipt(&apply(&mut tower, &requested));
    let bytes = image(&file);
    let state = file.rig.driver.supervisor().host().unwrap().inspect();
    let reads = file.source.status();
    let spent = detached.source().work();
    assert_eq!(file.rig.driver.pending_narrowing_continuation(REQUEST),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(file.rig.driver.continue_after_narrowing(REQUEST, 503),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(image(&file), bytes);
    assert_eq!(file.source.status(), reads);
    assert_eq!(detached.source().work(), spent);
    assert_eq!(state.executions, 0);
}

#[test]
fn narrowing_never_substitutes_for_the_original_final_topology_refresh() {
    for failure in ["bypass", "missing", "exhausted"] {
        let checks = if failure == "exhausted" { 1 } else { 2 };
        let (mut file, _) = prepared(checks, true);
        let retained = target(&file);
        let (mut tower, _grant) = operator(&file);
        let requested = command(&mut tower, 504, vec![retained]);
        receipt(&apply(&mut tower, &requested));
        file.rig.driver.continue_after_narrowing(REQUEST, 504).unwrap();
        let human = human(&mut file, 1001);
        file.dispatch(&human);
        assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
        match failure {
            "bypass" => fs::write(topology_path(&file), graph(checks, true)).unwrap(),
            "missing" => fs::remove_file(topology_path(&file)).unwrap(),
            _ => {}
        }
        assert!(matches!(publish_and_reconcile(&mut file), EndpointOutcome::NotExecuted { .. }),
            "{failure}");
        let host = file.rig.driver.supervisor().host().unwrap();
        let after = host.inspect();
        assert_eq!(after.executions, 0, "{failure}");
        assert_eq!(after.control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
        assert_eq!((after.control.ledger.reserved, after.control.ledger.charged), (0, 0));
        assert_eq!(after.control.ledger.available, fixture::driver::profile().delivery.total);
        assert!(!host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert_eq!(work(&file).checks, checks);
        assert_eq!(work(&file).reserved_read_bytes, checks * TOPOLOGY_REFRESH_READ_BYTES);
        assert_eq!(file.rig.driver.topology().unwrap().source().last_operation(), 2);
        let refused = file.rig.driver.topology().unwrap().source().last_refusal().unwrap();
        match failure {
            "bypass" => assert_eq!(refused, &TopologySourceRefusal::ChangedInput),
            "missing" => assert!(matches!(refused, TopologySourceRefusal::Input(_))),
            _ => assert_eq!(refused, &TopologySourceRefusal::BudgetExhausted),
        }
    }
}

#[test]
fn another_acknowledged_topology_check_makes_the_planned_narrowing_predecessor_stale() {
    let (mut file, _) = prepared(3, true);
    let retained = target(&file);
    let (mut tower, grant) = operator(&file);
    let requested = command(&mut tower, 505, vec![retained]);
    let revision = file.rig.driver.supervisor().host().unwrap().revision();
    assert!(file.rig.driver.refresh_topology(revision).unwrap().refusal.is_none());
    let bytes = image(&file);
    let state = file.rig.driver.supervisor().host().unwrap().inspect();
    let spent = work(&file);
    let refused = apply(&mut tower, &requested);
    let PublicationResult::Narrowing(linked) = refused.result else {
        panic!("expected an original predecessor refusal");
    };
    assert_eq!(linked.native(), &Err(FileNarrowError::StalePredecessor));
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(image(&file), bytes);
    assert_eq!(work(&file), spent);
    let fresh = command(&mut tower, 505, vec![retained]);
    receipt(&apply(&mut tower, &fresh));
    assert_eq!(grant.remaining_operations(), 1);
}
