//! Actual durable requests and socket reviews, not live-model qualification.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, encode_command};
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::cell::Cell;

#[test]
fn original_actor_wire_reaches_two_key_publication_and_a_separate_acknowledgment() {
    let mut rig = Rig::new();
    let proposal = rig.proposal();
    let revision = rig.driver.supervisor().host().unwrap().revision();
    rig.driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
    let mut wire = ActorWire::new(rig.port.clone());
    let command = encode_command(&Command::Submit { request: 90, proposal }).unwrap();
    assert!(matches!(wire.exchange(&command).result.unwrap(), Knowledge::Pending { .. }));
    assert_eq!(rig.reviewed(90).policy.control.decision.consequence, Consequence::Continue);
    assert!(matches!(rig.step(None), FileDriverEvent::AwaitingHuman { request: 90 }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
    let human = rig.human(1001, 30);
    let id = match rig.step(Some(&human)) {
        FileDriverEvent::Dispatched { request: 90, attempt } => attempt,
        other => panic!("missing original dispatch: {other:?}"),
    };
    assert_ne!(id, 90);
    let state = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 16);
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingPublication { request: 90 });
    let published = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| panic!("publication requested new evidence"), None).unwrap();
    assert!(matches!(published, FileDriverEvent::Published { outcome: EndpointOutcome::Executed { resulting_version: 2 }, .. }));
    let poll = encode_command(&Command::Poll { request: 90 }).unwrap();
    assert!(matches!(wire.exchange(&poll).result.unwrap(), Knowledge::Unknown { .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.stages[&id], ActionState::Dispatching);
    let settled = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| panic!("reconciliation requested new evidence"), None).unwrap();
    assert!(matches!(settled, FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. }));
    assert!(matches!(wire.exchange(&poll).result.unwrap(), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().payload, b"publication");
    let revision = rig.driver.supervisor().host().unwrap().revision();
    assert!(matches!(rig.driver.step_with_evidence(|| panic!("idle clock"), |_, _| panic!("idle source"), None).unwrap(), FileDriverEvent::Idle));
    assert_eq!(rig.driver.supervisor().host().unwrap().revision(), revision);
}

#[test]
fn an_adverse_completed_review_is_not_automatically_convened_again() {
    let mut rig = Rig::new();
    let ticket = rig.submit(1);
    rig.start(1, 101);
    let event = rig.review_event(Verdict::Hold, false);
    let FileDriverEvent::ReviewApplied { receipt, .. } = event else { panic!("expected restrictive review"); };
    assert_ne!(receipt.policy.control.decision.consequence, Consequence::Continue);
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert!(!matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let revision = rig.driver.supervisor().host().unwrap().revision();
    for _ in 0..3 { assert!(matches!(rig.step(None), FileDriverEvent::Idle)); }
    let host = rig.driver.supervisor().host().unwrap();
    assert_eq!(host.revision(), revision);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
}

#[test]
fn source_loss_at_review_completion_cannot_permit_but_does_not_block_restriction() {
    for verdict in [Verdict::Allow, Verdict::Hold] {
        let mut rig = Rig::new(); rig.submit(1); rig.start(1, 101);
        let event = rig.review_event(verdict, true);
        match (verdict, event) {
            (Verdict::Allow, FileDriverEvent::ReviewRejected { .. }) => {}
            (Verdict::Hold, FileDriverEvent::ReviewApplied { receipt, .. }) => {
                assert_ne!(receipt.policy.control.decision.consequence, Consequence::Continue);
            }
            (_, other) => panic!("incorrect missing-source outcome: {other:?}"),
        }
        assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
    }
}

#[test]
fn a_changed_second_source_read_keeps_the_reservation_but_never_dispatches() {
    let mut rig = Rig::new(); let ticket = rig.submit(1); rig.reviewed(1);
    let human = rig.human(1001, 30);
    let input = rig.inputs.clone().unwrap();
    let changed = helper::inputs(input.action(), b"different complete evidence after reservation");
    let calls = Cell::new(0);
    let error = rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| {
        calls.set(calls.get() + 1);
        Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(if calls.get() == 1 { input.clone() } else { changed.clone() }) })
    }, Some(&human)).unwrap_err();
    assert_eq!(error, JournalError::Contract(Error::Stale));
    assert_eq!(calls.get(), 2);
    let state = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.control.ledger.reserved, 16);
    assert_eq!(state.control.ledger.charged, 0);
    assert_eq!(state.executions, 0);
    assert!(rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
        snapshot: snapshot(), inputs: Some(input.clone()),
    }), Some(&human)).is_err());
    rig.port.cancel(&ticket).unwrap();
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert!(matches!(rig.driver.step_with_evidence(|| panic!("cancelled clock"), |_, _| panic!("cancelled source"), None).unwrap(),
        FileDriverEvent::Stopped { stage: ActionState::Cancelled, .. }));
}

#[test]
fn second_read_failure_does_not_refund_or_recreate_the_original_reservation() {
    let mut rig = Rig::new(); rig.submit(1); rig.reviewed(1);
    let human = rig.human(1001, 30); let input = rig.inputs.clone();
    let calls = Cell::new(0);
    assert_eq!(rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 { Err(Error::Incomplete) }
        else { Ok(DriverEvidence { snapshot: snapshot(), inputs: input.clone() }) }
    }, Some(&human)).unwrap_err(), JournalError::Contract(Error::Incomplete));
    let state = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.control.ledger.available, 84);
    assert_eq!(state.control.ledger.reserved, 16);
    assert_eq!(state.control.ledger.charged, 0);
    assert_eq!(state.executions, 0);
    assert!(rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
        snapshot: snapshot(), inputs: input.clone(),
    }), Some(&human)).is_err());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), state);
}

#[test]
fn expiry_after_reservation_needs_a_new_independent_human_key_not_another_automatic_permit() {
    let mut rig = Rig::new(); rig.submit(1); rig.reviewed(1);
    let expired = rig.human(1001, 30); let input = rig.inputs.clone();
    let ticks = Cell::new(0);
    assert!(rig.driver.step_with_evidence(|| {
        ticks.set(ticks.get() + 1);
        ElapsedTick(if ticks.get() < 3 { 1 } else { 30 })
    }, |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: input.clone() }), Some(&expired)).is_err());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 16);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    let fresh = rig.human(1002, 50);
    assert!(matches!(rig.step(Some(&fresh)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { outcome: EndpointOutcome::Executed { .. }, .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
}

#[test]
fn a_foreign_human_key_cannot_publish_despite_identical_numeric_attempts() {
    let mut first = Rig::new(); first.submit(1); first.reviewed(1);
    let mut second = Rig::new(); second.submit(1); second.reviewed(1);
    let foreign = second.human(1001, 30); let input = first.inputs.clone();
    assert_eq!(first.driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
        snapshot: snapshot(), inputs: input.clone(),
    }), Some(&foreign)).unwrap_err(), JournalError::Contract(Error::Binding));
    assert_eq!(first.driver.supervisor().host().unwrap().inspect().executions, 0);
    let own = first.human(1001, 30);
    assert!(matches!(first.step(Some(&own)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(first.step(None), FileDriverEvent::Published { .. }));
    assert_eq!(first.driver.supervisor().host().unwrap().inspect().executions, 1);
    assert_eq!(second.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn bad_roster_preflight_leaves_the_request_available_for_one_valid_launch() {
    let mut rig = Rig::new(); rig.submit(1);
    let mut launch = rig.launch(1, 101); launch.workers.remove(MEMBERS[0]);
    let before = rig.driver.supervisor().host().unwrap().inspect();
    assert!(rig.driver.start_review(launch, snapshot(), || panic!("bad roster sampled clock")).is_err());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    rig.reviewed(1);
    let key = rig.human(1001, 30);
    assert!(matches!(rig.step(Some(&key)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
}
