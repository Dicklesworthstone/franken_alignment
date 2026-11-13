//! Actual mediated Hold -> explicit fresh review -> two-key publication.
//! Each negative changes one original source, custody or predecessor boundary.
#![cfg(unix)]

#[path = "support/file_convene.rs"]
mod fixture;
#[path = "file_driver_mediated_convening/support.rs"]
mod mediated;
#[path = "file_driver_mediated_convening/boundaries.rs"]
mod boundaries;

use fa_reference::Error;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, Reconciliation,
};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{
    FileDriverEvent, FileDriverPhase,
};
use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::{
    TopologySourceRefusal, TOPOLOGY_REFRESH_READ_BYTES,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneOutcome;
use fixture::{NEXT_ROUND, REQUEST};
use std::fs;

#[test]
fn fresh_mediated_successor_uses_original_source_and_two_independent_publication_keys() {
    let mut file = mediated::held(3);
    let action = file.rig.driver.supervisor().host().unwrap().request_action(REQUEST).unwrap().clone();
    let (port, grant) = file.rig.driver.supervisor().convene_port(4, 1).unwrap();
    let request = fixture::command(&port, NEXT_ROUND);
    let before = fixture::image(&file);
    let reads = file.source.status();
    let work = mediated::work(&file);
    assert_eq!(fixture::receipt(&port, &request).outcome(), FileConveneOutcome::Pending);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(file.source.status(), reads);
    assert_eq!(mediated::work(&file), work);
    assert_eq!(fixture::image(&file), before);

    // The original topology check commits a real native journal event. That
    // authorized refresh must not make its own already-checked ticket stale.
    assert_eq!(fixture::launch(&mut file, &request), fixture::document(1).identity());
    assert_eq!(mediated::work(&file).checks, 2);
    assert_eq!(mediated::work(&file).reserved_read_bytes, 2 * TOPOLOGY_REFRESH_READ_BYTES);
    assert_eq!(file.rig.driver.topology().unwrap().source().last_operation(), 2);
    let opened = fixture::receipt(&port, &request);
    assert_eq!(opened.outcome(), FileConveneOutcome::Opened);
    assert!(opened.opened().is_some());
    assert_eq!(file.rig.driver.supervisor().host().unwrap().request_action(REQUEST).unwrap(), &action);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);

    fixture::finish_continue(&mut file);
    fixture::publish(&mut file);
    assert_eq!(mediated::work(&file).checks, 3);
    assert_eq!(mediated::work(&file).reserved_read_bytes, 3 * TOPOLOGY_REFRESH_READ_BYTES);
    assert_eq!(file.rig.driver.topology().unwrap().source().last_operation(), 3);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().request_action(REQUEST).unwrap(), &action);

    let completed = fixture::image(&file);
    let work = mediated::work(&file);
    let reads = file.source.status();
    assert_eq!(fixture::receipt(&port.clone(), &request), opened);
    assert_eq!(fixture::image(&file), completed);
    assert_eq!(mediated::work(&file), work);
    assert_eq!(file.source.status(), reads);
}

#[test]
fn changed_or_missing_topology_refuses_before_policy_capture_and_never_retries_after_repair() {
    for mutation in 0..5 {
        let mut file = mediated::held(3);
        let (port, grant) = file.rig.driver.supervisor().convene_port(4, 1).unwrap();
        let request = fixture::command(&port, NEXT_ROUND);
        fixture::receipt(&port, &request);
        let ticket = file.rig.driver.take_pending_convene(REQUEST).unwrap().unwrap();
        let launch = fixture::new_launch(&mut file, NEXT_ROUND);
        let reads = file.source.status();
        match mutation {
            0 => fs::remove_file(mediated::topology_path(&file)).unwrap(),
            1 => fs::write(mediated::topology_path(&file), mediated::graph(1, 3, true)).unwrap(),
            2 => fs::write(mediated::topology_path(&file), mediated::graph(2, 3, false)).unwrap(),
            3 => fs::remove_file(mediated::inventory_path(&file)).unwrap(),
            _ => fs::write(
                mediated::inventory_path(&file),
                mediated::INVENTORY.replace("Controlled fixture", "Changed controlled fixture"),
            ).unwrap(),
        }
        assert!(file.rig.driver.start_file_convened_review(
            &mut file.source,
            ticket,
            launch,
            || panic!("refused topology must not reach the review clock"),
        ).is_err(), "mutation {mutation}");
        assert_eq!(file.source.status(), reads, "mutation {mutation}");
        assert_eq!(mediated::work(&file).checks, 2);
        assert_eq!(grant.remaining_operations(), 0);
        let refusal = file.rig.driver.topology().unwrap().source().last_refusal();
        if mutation == 0 || mutation == 3 {
            assert!(matches!(refusal, Some(TopologySourceRefusal::Input(_))));
        } else {
            assert_eq!(refusal, Some(&TopologySourceRefusal::ChangedInput));
        }
        let failed = fixture::receipt(&port, &request);
        assert_eq!(failed.outcome(), FileConveneOutcome::Failed);
        assert!(failed.opened().is_none());
        assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
        {
            let host = file.rig.driver.supervisor().host().unwrap();
            assert!(host.review_replay(NEXT_ROUND).is_err());
            assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().control.ledger.charged, 0);
            assert_eq!(host.inspect().control.ledger.reserved, 0);
            assert!(!host.mediation_snapshot().unwrap().available);
            assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        }

        fs::write(mediated::topology_path(&file), mediated::graph(1, 3, false)).unwrap();
        fs::write(mediated::inventory_path(&file), mediated::INVENTORY).unwrap();
        let after = fixture::image(&file);
        let work = mediated::work(&file);
        assert_eq!(fixture::receipt(&port, &request), failed);
        assert!(file.rig.driver.take_pending_convene(REQUEST).unwrap().is_none());
        assert_eq!(file.source.status(), reads);
        assert_eq!(mediated::work(&file), work);
        assert_eq!(fixture::image(&file), after);
        assert_ne!(file.rig.driver.supervisor().next_review_round(NEXT_ROUND).unwrap(), NEXT_ROUND);
    }
}

#[test]
fn exhausted_original_source_allowance_cannot_be_refilled_by_a_convene_grant() {
    let mut file = mediated::held(1);
    let (port, grant) = file.rig.driver.supervisor().convene_port(3, 1).unwrap();
    let request = fixture::command(&port, NEXT_ROUND);
    fixture::receipt(&port, &request);
    let ticket = file.rig.driver.take_pending_convene(REQUEST).unwrap().unwrap();
    let launch = fixture::new_launch(&mut file, NEXT_ROUND);
    let reads = file.source.status();
    let spent = mediated::work(&file);
    assert!(file.rig.driver.start_file_convened_review(
        &mut file.source,
        ticket,
        launch,
        || panic!("exhausted topology must not reach the review clock"),
    ).is_err());
    assert_eq!(mediated::work(&file), spent);
    assert_eq!(file.source.status(), reads);
    assert_eq!(file.rig.driver.topology().unwrap().source().last_refusal(),
        Some(&TopologySourceRefusal::BudgetExhausted));
    assert_eq!(grant.remaining_operations(), 0);
    let failed = fixture::receipt(&port, &request);
    assert_eq!(failed.outcome(), FileConveneOutcome::Failed);
    assert!(failed.opened().is_none());
    let host = file.rig.driver.supervisor().host().unwrap();
    assert!(host.review_replay(NEXT_ROUND).is_err());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
}

#[test]
fn missing_driver_source_custody_defers_handoff_until_the_same_spent_source_is_attached() {
    let (mut file, selected) = mediated::held_without_selection(3);
    let (port, grant) = file.rig.driver.supervisor().convene_port(3, 1).unwrap();
    let request = fixture::command(&port, NEXT_ROUND);
    let pending = fixture::receipt(&port, &request);
    assert_eq!(pending.outcome(), FileConveneOutcome::Pending);
    let before = fixture::image(&file);
    let reads = file.source.status();
    let spent = selected.source().work();
    assert!(matches!(
        file.rig.driver.take_pending_convene(REQUEST),
        Err(JournalError::Contract(Error::Incomplete))
    ));
    assert_eq!(fixture::receipt(&port, &request), pending);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(selected.source().work(), spent);
    assert_eq!(file.source.status(), reads);
    assert_eq!(fixture::image(&file), before);

    // Reattach only the original nonclone custodian. Its previously spent
    // bootstrap check remains spent; no new grant or topology is inferred.
    mediated::attach(&mut file, selected);
    assert_eq!(mediated::work(&file), spent);
    assert_eq!(fixture::image(&file), before);
    fixture::launch(&mut file, &request);
    assert_eq!(mediated::work(&file).checks, 2);
    fixture::finish_continue(&mut file);
    fixture::publish(&mut file);
    assert_eq!(mediated::work(&file).checks, 3);
    assert_eq!(grant.remaining_operations(), 0);
}

#[test]
fn taken_mediated_ticket_cannot_cross_stop_expiry_or_input_changes_before_any_source_work() {
    for mutation in 0..3 {
        let mut file = mediated::held(3);
        let (port, grant) = file.rig.driver.supervisor().convene_port(3, 1).unwrap();
        let request = fixture::command(&port, NEXT_ROUND);
        fixture::receipt(&port, &request);
        let ticket = file.rig.driver.take_pending_convene(REQUEST).unwrap().unwrap();
        let launch = fixture::new_launch(&mut file, NEXT_ROUND);
        if mutation == 0 {
            let before = file.rig.driver.supervisor().host().unwrap().inspect();
            file.rig.driver.request_stop(StopRequest {
                operation: 700,
                expected_control_sequence: before.control.sequence,
                expected_authority_epoch: before.control.ledger.epoch,
            }).unwrap();
        } else {
            let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            if mutation == 1 {
                host.observe_time(revision, request.expected().deadline).unwrap();
            } else {
                let input = host.input_revision(1).unwrap();
                host.inputs_unavailable(revision, 1, input).unwrap();
            }
        }
        let before = fixture::image(&file);
        let work = mediated::work(&file);
        let reads = file.source.status();
        assert!(file.rig.driver.start_file_convened_review(
            &mut file.source,
            ticket,
            launch,
            || panic!("stale original held predecessor reached clock"),
        ).is_err(), "mutation {mutation}");
        assert_eq!(mediated::work(&file), work);
        assert_eq!(file.source.status(), reads);
        assert_eq!(fixture::image(&file), before);
        assert_eq!(fixture::receipt(&port, &request).outcome(), FileConveneOutcome::Failed);
        assert_eq!(grant.remaining_operations(), 0);
        let host = file.rig.driver.supervisor().host().unwrap();
        assert!(host.review_replay(NEXT_ROUND).is_err());
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.request_action(REQUEST).unwrap().spec().deadline, request.expected().deadline);
    }
}

#[test]
fn successful_successor_does_not_replace_the_original_final_topology_gate_or_receipt_accounting() {
    let mut file = mediated::held(3);
    let (port, grant) = file.rig.driver.supervisor().convene_port(3, 1).unwrap();
    let request = fixture::command(&port, NEXT_ROUND);
    fixture::receipt(&port, &request);
    fixture::launch(&mut file, &request);
    fixture::finish_continue(&mut file);
    let human = file.human();
    file.dispatch(&human);
    assert_eq!(mediated::work(&file).checks, 2);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);

    fs::remove_file(mediated::topology_path(&file)).unwrap();
    let event = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None)
        .result.unwrap();
    assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
        if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
    assert_eq!(mediated::work(&file).checks, 3);
    assert!(matches!(file.rig.driver.topology().unwrap().source().last_refusal(),
        Some(TopologySourceRefusal::Input(_))));
    {
        let host = file.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    }
    let work = mediated::work(&file);
    let reads = file.source.status();
    assert!(matches!(file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None)
        .result.unwrap(), FileDriverEvent::Reconciled {
            outcome: Reconciliation::Resolved(EndpointOutcome::NotExecuted { .. }), ..
        }));
    assert_eq!(mediated::work(&file), work);
    assert_eq!(file.source.status(), reads);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(fixture::receipt(&port, &request).outcome(), FileConveneOutcome::Opened);
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().payload, b"initial");
}
