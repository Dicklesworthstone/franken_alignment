//! Boundaries not exercised by the ordinary or initial mediated launch cases.
//! Keep actual source acquisition, helper disclosures and original owner custody
//! observable; a retained diagnosis is never authority for another operation.
use super::{fixture, mediated};
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::{
    DriverTopologyObservation, DriverTopologyOutcome, FileDriverTopology,
};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{
    FileDriverEvent, FileDriverPhase,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneOutcome;
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::evidence_source::EvidenceSnapshot;
use fa_reference::action::ElapsedTick;
use fixture::{HELD_ROUND, NEXT_ROUND, REQUEST};
use std::fs;

fn last(file: &fixture::FileRig) -> DriverTopologyObservation {
    file.rig
        .driver
        .topology()
        .unwrap()
        .last_observation()
        .unwrap()
        .clone()
}

fn assert_no_successor(file: &fixture::FileRig) {
    let host = file.rig.driver.supervisor().host().unwrap();
    assert!(host.review_replay(NEXT_ROUND).is_err());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    assert!(file.rig.clients.values().all(|peer| peer.input().is_none()));
}

fn detach_topology(file: fixture::FileRig) -> (fixture::FileRig, FileDriverTopology) {
    let fixture::FileRig { mut rig, source, path } = file;
    let mut released = rig.driver.release();
    assert!(released.children.is_none());
    assert!(released.learned_review.is_none());
    let topology = released.topology.take().unwrap();
    rig.driver = released.into_driver().unwrap();
    (fixture::FileRig { rig, source, path }, topology)
}

#[test]
fn changed_complete_evidence_reaches_new_helpers_without_rewriting_the_held_review() {
    let mut file = mediated::held(3);
    let (action, held) = {
        let host = file.rig.driver.supervisor().host().unwrap();
        (
            host.request_action(REQUEST).unwrap().clone(),
            host.review_replay(HELD_ROUND).unwrap(),
        )
    };
    let prior_alpha = file.rig.clients["alpha"]
        .input().unwrap().actual_input().submitted_bytes().to_vec();
    let marker = b"INDEPENDENT-NEW-EVIDENCE-FOR-THE-EXPLICIT-SUCCESSOR";
    assert!(!prior_alpha.windows(marker.len()).any(|window| window == marker));
    let original = fixture::document(2);
    let mut contexts = original.contexts().clone();
    contexts.get_mut("alpha").unwrap().extend_from_slice(marker);
    let changed = EvidenceSnapshot::new(
        original.identity(),
        original.snapshot().clone(),
        contexts,
    ).unwrap();
    let expected_inputs = changed.inputs_for(
        &action,
        &fixture::driver::profile().committee,
    ).unwrap();

    let (port, grant) = file.rig.driver.supervisor().convene_port(3, 1).unwrap();
    let request = fixture::command(&port, NEXT_ROUND);
    fixture::receipt(&port, &request);
    fixture::replace(&file.path, &changed);
    assert_eq!(fixture::launch(&mut file, &request), changed.identity());
    assert_eq!(mediated::work(&file).checks, 2);
    let observation = last(&file);
    assert_eq!(observation.request, Some(REQUEST));
    assert_eq!(observation.attempt, Some(request.expected().attempt));
    assert_eq!(observation.started_at_revision, request.expected().journal_revision);
    fixture::finish_continue(&mut file);
    let alpha = file.rig.clients["alpha"].input().unwrap();
    assert_eq!(alpha.round(), NEXT_ROUND);
    assert!(alpha.actual_input().submitted_bytes().windows(marker.len())
        .any(|window| window == marker));
    assert_ne!(alpha.actual_input().submitted_bytes(), prior_alpha.as_slice());
    {
        let host = file.rig.driver.supervisor().host().unwrap();
        let successor = host.review_replay(NEXT_ROUND).unwrap();
        assert_eq!(successor.application_inputs(), Some(&expected_inputs));
        let retained = host.review_replay(HELD_ROUND).unwrap();
        assert_eq!(retained.archive(), held.archive());
        assert_eq!(retained.application(), held.application());
        assert_eq!(retained.application_inputs(), held.application_inputs());
        assert_eq!(host.request_action(REQUEST).unwrap(), &action);
        assert_eq!(host.inspect().executions, 0);
    }
    // The existing path observes AwaitingHuman before acquiring a new independent
    // approval. Changed evidence and a new Continue do not publish on their own.
    fixture::publish(&mut file);
    assert_eq!(mediated::work(&file).checks, 3);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(fixture::receipt(&port, &request).outcome(), FileConveneOutcome::Opened);
}

#[test]
fn failed_policy_capture_retains_successful_topology_work_and_terminal_operation() {
    let mut file = mediated::held(3);
    let (port, grant) = file.rig.driver.supervisor().convene_port(2, 1).unwrap();
    let request = fixture::command(&port, NEXT_ROUND);
    fixture::receipt(&port, &request);
    let ticket = file.rig.driver.take_pending_convene(REQUEST).unwrap().unwrap();
    let launch = fixture::new_launch(&mut file, NEXT_ROUND);
    fs::remove_file(&file.path).unwrap();
    let reads = file.source.status().read_attempts;
    assert!(file.rig.driver.start_file_convened_review(
        &mut file.source,
        ticket,
        launch,
        || ElapsedTick(1),
    ).is_err());
    assert_no_successor(&file);
    assert_eq!(file.source.status().read_attempts, reads + 1);
    assert_eq!(mediated::work(&file).checks, 2);
    let observed = last(&file);
    assert_eq!(observed.request, Some(REQUEST));
    assert_eq!(observed.attempt, Some(request.expected().attempt));
    let DriverTopologyOutcome::Finished(Ok(report)) = &observed.outcome else {
        panic!("successful topology acquisition disappeared after policy failure");
    };
    assert!(report.refusal.is_none());
    assert!(matches!(report.checked, Some(Ok(CutCheck::Verified(_)))));
    assert_eq!(report.work, mediated::work(&file));
    assert_eq!(grant.remaining_operations(), 0);
    let failed = fixture::receipt(&port, &request);
    assert_eq!(failed.outcome(), FileConveneOutcome::Failed);
    assert!(failed.opened().is_none());

    fixture::replace(&file.path, &fixture::document(1));
    let before = fixture::image(&file);
    let reads = file.source.status();
    assert_eq!(fixture::receipt(&port, &request), failed);
    assert!(file.rig.driver.take_pending_convene(REQUEST).unwrap().is_none());
    assert!(matches!(
        file.rig.driver.step_from_file(
            &mut file.source,
            || panic!("restoring policy data cannot rerun a failed successor"),
            None,
        ).result,
        Ok(FileDriverEvent::Idle)
    ));
    assert_eq!(mediated::work(&file).checks, 2);
    assert_eq!(last(&file), observed);
    assert_eq!(file.source.status(), reads);
    assert_eq!(fixture::image(&file), before);
}

#[test]
fn a_separate_acknowledged_topology_refresh_stales_the_entire_convening_ticket() {
    let mut file = mediated::held(4);
    let (port, grant) = file.rig.driver.supervisor().convene_port(2, 1).unwrap();
    let request = fixture::command(&port, NEXT_ROUND);
    fixture::receipt(&port, &request);
    let ticket = file.rig.driver.take_pending_convene(REQUEST).unwrap().unwrap();
    let revision = file.rig.driver.supervisor().host().unwrap().revision();
    let refreshed = file.rig.driver.refresh_topology(revision).unwrap();
    assert!(refreshed.refusal.is_none());
    assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
    assert!(refreshed.journal_revision > request.expected().journal_revision);
    let observed = last(&file);
    assert_eq!((observed.request, observed.attempt), (None, None));
    let launch = fixture::new_launch(&mut file, NEXT_ROUND);
    let before = fixture::image(&file);
    let reads = file.source.status();
    let spent = mediated::work(&file);
    assert!(file.rig.driver.start_file_convened_review(
        &mut file.source,
        ticket,
        launch,
        || panic!("stale topology predecessor reached clock work"),
    ).is_err());
    assert_no_successor(&file);
    assert_eq!(file.source.status(), reads);
    assert_eq!(mediated::work(&file), spent);
    assert_eq!(last(&file), observed);
    assert_eq!(fixture::image(&file), before);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(fixture::receipt(&port, &request).outcome(), FileConveneOutcome::Failed);
}

#[test]
fn custody_lost_after_ticket_handoff_cannot_enable_an_unselected_launch_or_retry() {
    let mut file = mediated::held(3);
    let (port, grant) = file.rig.driver.supervisor().convene_port(2, 1).unwrap();
    let request = fixture::command(&port, NEXT_ROUND);
    fixture::receipt(&port, &request);
    let ticket = file.rig.driver.take_pending_convene(REQUEST).unwrap().unwrap();
    let (mut file, topology) = detach_topology(file);
    let spent = topology.source().work();
    let diagnosis = topology.last_observation().cloned();
    let launch = fixture::new_launch(&mut file, NEXT_ROUND);
    let before = fixture::image(&file);
    let reads = file.source.status();
    assert!(file.rig.driver.start_file_convened_review(
        &mut file.source,
        ticket,
        launch,
        || panic!("missing original topology custody reached capture"),
    ).is_err());
    assert_no_successor(&file);
    assert_eq!(file.source.status(), reads);
    assert_eq!(fixture::image(&file), before);
    assert_eq!(topology.source().work(), spent);
    assert_eq!(topology.last_observation().cloned(), diagnosis);
    assert_eq!(grant.remaining_operations(), 0);
    let failed = fixture::receipt(&port, &request);
    assert_eq!(failed.outcome(), FileConveneOutcome::Failed);
    assert!(failed.opened().is_none());
    mediated::attach(&mut file, topology);
    assert_eq!(fixture::receipt(&port, &request), failed);
    assert!(file.rig.driver.take_pending_convene(REQUEST).unwrap().is_none());
    assert_eq!(mediated::work(&file), spent);
    assert_eq!(file.source.status(), reads);
    assert_eq!(fixture::image(&file), before);
}
