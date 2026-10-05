//! Original inference, actor tickets, computed congress and real journal files.
//! Synthetic parameters test mechanics, not independent detector effectiveness.
use super::*;
use super::super::{FileDriverPhase, DriverEvidence};
use super::super::super::{FileOversight, FileOversightProfile, FileHumanReviewer};
use super::super::super::decoder::learned::FileLearnedConfig;
use super::super::super::super::{JournalIo, Reconciliation};
use super::super::super::super::requests::actor::{FileActorPort, FileActorTicket};
use crate::action::FrozenAction;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::oversight::actor::{ActorProposal, ActorOutcome, Knowledge};
use crate::action::consequence::oversight::learned_host::sidecar::LearnedSidecarRequest;
use crate::action::consequence::oversight::sidecar::{SidecarCongressBudget, SidecarIdentity};
#[path = "../helpers/learned/tests/fixture.rs"]
mod fixture;
use fixture::*;

type Port = FileActorPort<FileOversight>;
type Ticket = FileActorTicket<FileOversight>;

fn submit(driver: &mut FileSupervisedDriver, port: &Port, request: u64) -> Ticket {
    let (revision, target, epoch) = {
        let host = driver.supervisor().host().unwrap();
        (host.revision(), host.inspect().target, host.inspect().control.ledger.epoch)
    };
    driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
    port.submit(request, &ActorProposal { target, payload: b"visible".to_vec(),
        expected_policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }).unwrap()
}
fn launch(driver: &mut FileSupervisedDriver, request: u64, mode: u8) -> FileDriverLearnedLaunch {
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    let (attempt, _) = admitted(host.request_status(request).unwrap()).unwrap();
    let evidence = host.machine.broker.learned_decoder_evidence(attempt).unwrap().unwrap();
    let options = LearnedSidecarRequest { identity: SidecarIdentity {
        object_id: request + 1000, generation: 1, transform_id: 7 },
        priority: evidence.audit().source().groups().collect(), budget: SidecarCongressBudget::default() };
    let actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let revision = host.revision();
    let sidecar = host.begin_learned_sidecar_plan(revision, attempt, actor_revision, options).unwrap();
    let members = members(&host, &sidecar, mode);
    FileDriverLearnedLaunch { request, sidecar, schedule: schedule(), members, limits: limits() }
}
fn setup(root: &Directory, mode: u8) -> (Port, Ticket, FileSupervisedDriver, FileHumanReviewer) {
    let (mut host, reviewer) = owner(root, &config());
    step(&mut host); step(&mut host);
    let (port, mut driver) = host.into_supervised_driver();
    let ticket = submit(&mut driver, &port, 901);
    let launch = launch(&mut driver, 901, mode);
    driver.start_learned_probe_review(launch, snapshot(), ElapsedTick(1)).unwrap();
    (port, ticket, driver, reviewer)
}
fn advance(driver: &mut FileSupervisedDriver, now: u64) -> Result<FileDriverLearnedEvent, JournalError> {
    let revision = driver.learned_probe_review().unwrap().review().revision();
    driver.advance_learned_probe_review(revision, ElapsedTick(now), snapshot())
}
fn complete(driver: &mut FileSupervisedDriver) -> FileDriverEvent {
    for _ in 0..64 {
        match advance(driver, 1).unwrap() {
            FileDriverLearnedEvent::Progress { .. } => {
                assert!(matches!(driver.phase(), FileDriverPhase::Reviewing { request: 901 }));
            }
            FileDriverLearnedEvent::Completed(event) => return event,
        }
    }
    panic!("original bounded computed review did not complete");
}
fn ordinary(driver: &mut FileSupervisedDriver,
    human: Option<&super::super::super::FileHumanPermit>) -> FileDriverEvent
{
    let input = driver.learned_probe_review().unwrap().review().input().clone();
    driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
        snapshot: snapshot(), inputs: Some(input.clone()),
    }), human).unwrap()
}
fn approve(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer)
    -> super::super::super::FileHumanPermit
{
    let input = driver.learned_probe_review().unwrap().review().input().clone();
    let request = driver.request_human_approval(1001, &input, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn actor_request_refines_through_original_computation_and_reaches_two_key_publication() {
    let root = Directory::new();
    let (port, ticket, mut driver, reviewer) = setup(&root, 1);
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 901 }));
    assert_eq!(driver.next_review_deadline(), Some(ElapsedTick(10)));
    let coarse = driver.learned_probe_review().unwrap().review().input().clone();
    let before = driver.supervisor().host().unwrap().revision();
    // A running computed review cannot fall through to callbacks or sockets.
    assert!(matches!(driver.step_with_evidence(|| panic!("no clock"),
        |_, _| panic!("no substituted input"), None), Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(driver.supervisor().host().unwrap().revision(), before);
    assert!(matches!(complete(&mut driver), FileDriverEvent::ReviewApplied { request: 901, .. }));
    assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 901 });
    let run = driver.learned_probe_review().unwrap().review();
    assert!(run.history().len() > 1);
    assert!(matches!(run.history().first(), Some(FileLearnedSidecarFinish::Refined { .. })));
    assert_ne!(run.input(), &coarse);
    assert_eq!(run.evaluations(), run.history().len() * 2);
    assert!(run.records().values().any(|record| record.work.refined_groups > 0));
    assert_eq!(driver.next_review_deadline(), None);
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert!(matches!(ordinary(&mut driver, None), FileDriverEvent::AwaitingHuman { request: 901 }));
    let human = approve(&mut driver, &reviewer);
    assert!(matches!(ordinary(&mut driver, Some(&human)), FileDriverEvent::Dispatched { request: 901, .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    assert!(matches!(ordinary(&mut driver, None), FileDriverEvent::PublicationChecked { publication, .. }
        if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
    assert!(matches!(ordinary(&mut driver, None), FileDriverEvent::Reconciled {
        request: 901, outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }) }));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let retained = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config()).unwrap();
    assert_eq!(retained.payload, b"visible"); assert_eq!(retained.executions, 1);
    assert_eq!(retained.control.ledger.charged, 16);
    let report = driver.take_learned_probe_review().unwrap().unwrap();
    assert_eq!(report.request(), 901);
    assert_eq!(report.review().status(), FileLearnedProbeStatus::Finished);
    assert!(driver.learned_probe_review().is_none());
}

#[test]
fn native_positive_control_and_computed_alarm_cannot_become_driver_permission() {
    // Execute the same original standalone composition as a positive control.
    let control_root = Directory::new(); let (mut host, _) = owner(&control_root, &config());
    step(&mut host); step(&mut host);
    let (_, sidecar) = propose(&mut host);
    let definitions = members(&host, &sidecar, 0);
    let mut original = host.begin_learned_probe_review(host.revision(), sidecar,
        schedule(), definitions, limits(), snapshot()).unwrap();
    assert_eq!(drive(&mut host, &mut original), FileLearnedProbeStatus::Finished);
    assert_eq!(host.inspect().control.decisions.get(&1), Some(&Consequence::Continue));

    let root = Directory::new(); let (_, _, mut driver, _) = setup(&root, 2);
    let event = complete(&mut driver);
    assert!(matches!(event, FileDriverEvent::ReviewApplied { receipt, .. }
        if receipt.policy.control.decision.consequence != Consequence::Continue));
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(driver.learned_probe_review().unwrap().review().status(), FileLearnedProbeStatus::Finished);
    assert!(driver.request_human_approval(1001, original.input(), ElapsedTick(80), ElapsedTick(1)).is_err());
}

#[test]
fn stale_quantum_does_not_score_and_partial_cancellation_retains_actual_work_and_ticket() {
    let root = Directory::new(); let (port, ticket, mut driver, _) = setup(&root, 0);
    assert!(matches!(driver.advance_learned_probe_review(1, ElapsedTick(1), snapshot()),
        Err(JournalError::Contract(Error::Stale))));
    assert_eq!(driver.learned_probe_review().unwrap().review().evaluations(), 0);
    assert!(matches!(driver.take_learned_probe_review(), Err(Error::WrongState)));
    advance(&mut driver, 1).unwrap(); advance(&mut driver, 1).unwrap();
    let before = driver.learned_probe_review().unwrap().review().records().clone();
    assert!(before.values().any(|record| record.work.evaluated_probes > 0));
    driver.cancel_active().unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    let run = driver.learned_probe_review().unwrap().review();
    assert_eq!(run.status(), FileLearnedProbeStatus::Cancelled);
    for (key, record) in run.records() { assert_eq!(record.work, before[key].work); }
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert!(advance(&mut driver, 1).is_err());
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn changed_original_numerical_source_fails_before_any_more_helper_scoring() {
    let root = Directory::new(); let (_, _, mut driver, _) = setup(&root, 0);
    advance(&mut driver, 1).unwrap(); advance(&mut driver, 1).unwrap();
    let before = driver.learned_probe_review().unwrap().review().records().clone();
    { let mut host = driver.supervisor_mut().host_mut().unwrap(); step(&mut host); }
    assert!(advance(&mut driver, 1).is_err());
    let run = driver.learned_probe_review().unwrap().review();
    assert_eq!(run.status(), FileLearnedProbeStatus::Failed);
    for (key, record) in run.records() { assert_eq!(record.work, before[key].work); }
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn missing_members_end_review_without_refinement_or_an_automatic_key() {
    let root = Directory::new(); let (_, _, mut driver, _) = setup(&root, 1);
    let _ = advance(&mut driver, 10).unwrap();
    if driver.learned_probe_review().unwrap().review().status() == FileLearnedProbeStatus::Running {
        advance(&mut driver, 15).unwrap();
    }
    let run = driver.learned_probe_review().unwrap().review();
    assert_eq!(run.status(), FileLearnedProbeStatus::Finished);
    assert_eq!(run.evaluations(), 0); assert_eq!(run.input_revision(), 1);
    assert_eq!(run.history().len(), 1);
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn all_original_commitment_storage_failures_retain_work_and_recover_without_publication() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (_, _, mut driver, _) = setup(&root, 0);
        driver.supervisor().host().unwrap().store.fail_once(barrier);
        let mut failed = false;
        for _ in 0..64 {
            match advance(&mut driver, 1) {
                Ok(FileDriverLearnedEvent::Progress { .. }) => {}
                Err(JournalError::Io(failure)) => { assert_eq!(failure.operation, barrier); failed = true; break; }
                other => panic!("expected original selected storage failure, got {other:?}"),
            }
        }
        assert!(failed);
        let run = driver.learned_probe_review().unwrap().review();
        assert_eq!(run.status(), FileLearnedProbeStatus::Failed);
        assert!(run.records().values().any(|record| record.work.evaluated_probes > 0));
        assert_eq!(driver.phase(), FileDriverPhase::Idle);
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        drop(driver);
        let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config()).unwrap();
        assert_eq!(recovered.inspect().executions, 0);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert!(recovered.learned_generation_inspection().unwrap().paused);
    }
}

#[test]
fn release_retains_cancelled_computation_and_the_same_original_actor_gateway() {
    let root = Directory::new(); let (port, ticket, mut driver, _) = setup(&root, 0);
    advance(&mut driver, 1).unwrap(); advance(&mut driver, 1).unwrap();
    let before = driver.learned_probe_review().unwrap().review().records().clone();
    let released = driver.release();
    let saved = released.learned_review.unwrap();
    assert_eq!(saved.review().status(), FileLearnedProbeStatus::Cancelled);
    for (key, record) in saved.review().records() { assert_eq!(record.work, before[key].work); }
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 901 }));
    let replacement = FileSupervisedDriver::new(released.supervisor);
    assert_eq!(replacement.phase(), FileDriverPhase::Idle);
    assert!(replacement.learned_probe_review().is_none());
    assert_eq!(replacement.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn dispatched_cancellation_stays_query_only_without_repeating_helpers_or_refunding() {
    let root = Directory::new(); let (port, ticket, mut driver, reviewer) = setup(&root, 0);
    complete(&mut driver);
    let human = approve(&mut driver, &reviewer);
    assert!(matches!(ordinary(&mut driver, Some(&human)), FileDriverEvent::Dispatched { .. }));
    let records = driver.learned_probe_review().unwrap().review().records().clone();
    driver.cancel_active().unwrap();
    assert_eq!(driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 901 });
    assert!(matches!(driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("no evidence during settlement"), None).unwrap(),
        FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    let state = driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.available, 84);
    assert_eq!(driver.learned_probe_review().unwrap().review().records(), &records);
}
