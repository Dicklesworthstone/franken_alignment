//! Actual original computation and canonical files through the opt-in host loop.
//! The original socket-only refusal and every effect key remain independently tested.
use super::*;
use super::super::super::{FileDriverPhase, admitted};
use super::super::super::learned::FileDriverLearnedLaunch;
use super::super::super::super::{FileOversight, FileHumanReviewer, FileOversightProfile};
use super::super::super::super::decoder::learned::{FileLearnedConfig,
    sidecar::{FileLearnedSidecar, FileLearnedSidecarFinish}};
use super::super::super::super::helpers::learned::FileLearnedProbeReview;
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use crate::action::consequence::delivery::persistent::requests::actor::{FileActorPort, FileActorTicket};
use crate::action::consequence::oversight::{
    actor::{ActorProposal, ActorOutcome, Knowledge},
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerSchedule,
        probes::{ProbeReviewLimits, ProbeReviewMember}}},
    sidecar::{SidecarCongressBudget, SidecarIdentity, probe_helper::ProbeHelperStatus},
};
use crate::{Snapshot, Error};
use crate::round::Verdict;
#[path = "../../../helpers/learned/tests/fixture.rs"]
mod fixture;
use fixture::*;

const REQUEST: u64 = 71;
struct Harness {
    port: FileActorPort<FileOversight>,
    ticket: FileActorTicket<FileOversight>,
    driver: FileSupervisedDriver,
    reviewer: FileHumanReviewer,
}
fn setup(root: &Directory, config: &FileLearnedConfig, mode: u8) -> Harness {
    let (mut host, reviewer) = owner(root, config);
    step(&mut host); step(&mut host);
    let target = host.inspect().target;
    let epoch = host.inspect().control.ledger.epoch;
    let (port, mut driver) = host.into_supervised_driver();
    let revision = driver.supervisor().host().unwrap().revision();
    driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
    let ticket = port.submit(REQUEST, &ActorProposal { target, payload: b"visible".to_vec(),
        expected_policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }).unwrap();
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let attempt = admitted(host.request_status(REQUEST).unwrap()).unwrap().0;
        assert_eq!(attempt, 1);
        let evidence = host.machine.broker.learned_decoder_evidence(attempt).unwrap().unwrap();
        let request = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: evidence.audit().source().groups().collect(), budget: SidecarCongressBudget::default() };
        let actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
        let revision = host.revision();
        let sidecar = host.begin_learned_sidecar_plan(revision, attempt, actor_revision, request).unwrap();
        let definitions = members(&host, &sidecar, mode);
        FileDriverLearnedLaunch { request: REQUEST, sidecar, schedule: schedule(), members: definitions, limits: limits() }
    };
    driver.start_learned_probe_review(launch, snapshot(), ElapsedTick(1)).unwrap();
    Harness { port, ticket, driver, reviewer }
}
fn run(driver: &FileSupervisedDriver) -> &FileLearnedProbeReview {
    driver.learned_probe_review().unwrap().review()
}
fn tick(driver: &mut FileSupervisedDriver, human: Option<&FileHumanPermit>) -> Result<FileDriverEvent, JournalError> {
    let input = run(driver).input().clone();
    driver.step_with_computed_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
        snapshot: snapshot(), inputs: Some(input.clone()),
    }), human)
}
fn finish(h: &mut Harness) -> FileDriverEvent {
    for _ in 0..64 {
        let event = tick(&mut h.driver, None).unwrap();
        if !matches!(event, FileDriverEvent::Workers { .. }) { return event; }
    }
    panic!("computed review exceeded original bounded schedule");
}
fn work(driver: &FileSupervisedDriver) -> usize {
    run(driver).records().values().map(|record| record.work.evaluated_probes).sum()
}
fn approve(h: &mut Harness) -> FileHumanPermit {
    let input = run(&h.driver).input().clone();
    let request = h.driver.request_human_approval(1001, &input, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = h.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision(); h.reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn computed_loop_refines_and_publishes_while_legacy_socket_entry_still_refuses_takeover() {
    let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 1);
    let initial = run(&h.driver).input().clone();
    let revision = h.driver.supervisor().host().unwrap().revision();
    assert!(matches!(h.driver.step_with_evidence(|| panic!("legacy must not observe"),
        |_, _| panic!("legacy must not substitute input"), None), Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(h.driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(work(&h.driver), 0);
    let mut applied = false;
    for _ in 0..64 {
        let before = work(&h.driver);
        let event = tick(&mut h.driver, None).unwrap();
        assert!(work(&h.driver) <= before + 2, "one probe per member per quantum");
        match event {
            FileDriverEvent::Workers { report, .. } => assert!(report.io.is_empty()),
            FileDriverEvent::ReviewApplied { receipt, .. } => {
                assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
                applied = true; break;
            }
            other => panic!("unexpected review event: {other:?}"),
        }
    }
    assert!(applied);
    assert_eq!(run(&h.driver).status(), FileLearnedProbeStatus::Finished);
    assert!(run(&h.driver).history().len() > 1);
    assert!(matches!(run(&h.driver).history().first(), Some(FileLearnedSidecarFinish::Refined { .. })));
    assert!(run(&h.driver).records().values().any(|record|
        record.status == Some(ProbeHelperStatus::Judged(Verdict::Abstain))));
    assert_ne!(run(&h.driver).input(), &initial);
    assert!(h.driver.request_human_approval(1001, &initial, ElapsedTick(80), ElapsedTick(1)).is_err());
    assert!(matches!(tick(&mut h.driver, None).unwrap(), FileDriverEvent::AwaitingHuman { .. }));
    assert_eq!(h.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    let human = approve(&mut h);
    assert!(matches!(tick(&mut h.driver, Some(&human)).unwrap(), FileDriverEvent::Dispatched { attempt: 1, .. }));
    assert_eq!(h.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert!(matches!(tick(&mut h.driver, None).unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
        if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
    assert!(matches!(tick(&mut h.driver, None).unwrap(), FileDriverEvent::Reconciled {
        outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }), .. }));
    assert!(matches!(h.port.poll(&h.ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert!(matches!(tick(&mut h.driver, Some(&human)).unwrap(), FileDriverEvent::Idle));
    let saved = h.driver.take_learned_probe_review().unwrap().unwrap();
    drop(h.driver);
    let stored = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(stored.executions, 1); assert_eq!(stored.payload, b"visible");
    assert_eq!(stored.control.ledger.charged, 16);
    let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(recovered.inspect().executions, 1);
    assert!(recovered.learned_generation_inspection().unwrap().paused);
    assert_eq!(saved.review().status(), FileLearnedProbeStatus::Finished);
}

#[test]
fn computed_loop_alarm_has_no_permitting_phase_and_no_publication() {
    let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 2);
    assert!(matches!(finish(&mut h), FileDriverEvent::ReviewApplied { receipt, .. }
        if receipt.policy.control.decision.consequence != Consequence::Continue));
    assert_eq!(run(&h.driver).status(), FileLearnedProbeStatus::Finished);
    assert!(run(&h.driver).records().values().any(|record|
        record.status == Some(ProbeHelperStatus::Judged(Verdict::Hold))));
    assert_eq!(h.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(h.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn computed_loop_cancellation_retains_work_and_never_reads_another_provider() {
    let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 0);
    tick(&mut h.driver, None).unwrap(); tick(&mut h.driver, None).unwrap();
    let before = work(&h.driver); assert_eq!(before, 2);
    h.port.cancel(&h.ticket).unwrap();
    assert!(matches!(h.driver.step_with_computed_evidence(|| panic!("terminal needs no clock"),
        |_, _| panic!("terminal needs no source"), None).unwrap(), FileDriverEvent::Stopped { stage: crate::action::ActionState::Cancelled, .. }));
    assert_eq!(run(&h.driver).status(), FileLearnedProbeStatus::Cancelled);
    assert_eq!(work(&h.driver), before);
    assert!(matches!(h.port.poll(&h.ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    let mut host = h.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    assert_eq!(host.commit_review(revision, 101, "alpha", 0), Err(Error::WrongState.into()));
}

#[test]
fn computed_loop_source_provider_and_old_refined_input_loss_precede_further_scoring() {
    for loss in 0..3 {
        let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 1);
        let coarse = run(&h.driver).input().clone();
        tick(&mut h.driver, None).unwrap(); tick(&mut h.driver, None).unwrap();
        if loss == 2 {
            for _ in 0..64 {
                if run(&h.driver).input_revision() > 1 { break; }
                tick(&mut h.driver, None).unwrap();
            }
            assert!(run(&h.driver).input_revision() > 1);
            assert_eq!(run(&h.driver).status(), FileLearnedProbeStatus::Running);
        }
        let before = work(&h.driver);
        let result = match loss {
            0 => {
                step(&mut h.driver.supervisor_mut().host_mut().unwrap());
                tick(&mut h.driver, None)
            }
            1 => h.driver.step_with_computed_evidence(|| ElapsedTick(1), |_, _| Err(Error::Missing), None),
            _ => h.driver.step_with_computed_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
                snapshot: snapshot(), inputs: Some(coarse.clone()),
            }), None),
        };
        assert!(result.is_err()); assert_eq!(work(&h.driver), before);
        assert_ne!(run(&h.driver).status(), FileLearnedProbeStatus::Running);
        assert_eq!(h.driver.phase(), FileDriverPhase::Idle);
        let input = run(&h.driver).input().clone();
        let mut host = h.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        assert!(host.authorize(revision, 1, &input, snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn computed_loop_storage_barriers_keep_native_work_and_recover_only_the_original_ledger() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 0);
        h.driver.supervisor().host().unwrap().store.fail_once(barrier);
        let mut failed = false;
        for _ in 0..64 {
            match tick(&mut h.driver, None) {
                Ok(FileDriverEvent::Workers { .. }) => {}
                Err(JournalError::Io(failure)) => { assert_eq!(failure.operation, barrier); failed = true; break; }
                other => panic!("expected selected barrier, got {other:?}"),
            }
        }
        assert!(failed); assert!(work(&h.driver) > 0);
        assert_eq!(run(&h.driver).status(), FileLearnedProbeStatus::Failed);
        let saved = h.driver.take_learned_probe_review().unwrap().unwrap();
        drop(h.driver);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert!(saved.review().history().is_empty());
    }
}

#[test]
fn computed_loop_unknown_dispatch_settlement_does_not_rerun_helpers_or_refund() {
    let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 0);
    finish(&mut h); let human = approve(&mut h);
    assert!(matches!(tick(&mut h.driver, Some(&human)).unwrap(), FileDriverEvent::Dispatched { .. }));
    let before = run(&h.driver).records().clone();
    h.driver.cancel_active().unwrap();
    assert!(matches!(h.driver.step_with_computed_evidence(|| ElapsedTick(2),
        |_, _| panic!("settlement must not observe or score"), None).unwrap(),
        FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    assert_eq!(run(&h.driver).records(), &before);
    assert_eq!(h.driver.supervisor().host().unwrap().inspect().control.ledger.available, 84);
    assert!(matches!(h.port.poll(&h.ticket), Knowledge::Unknown { .. }));
}

#[test]
fn computed_loop_matches_original_standalone_review_without_adopting_its_receipt() {
    let root = Directory::new(); let control_root = Directory::new(); let config = config();
    let mut h = setup(&root, &config, 1); finish(&mut h);
    let (mut control, _) = owner(&control_root, &config); step(&mut control); step(&mut control);
    let (_, sidecar) = propose(&mut control);
    let definitions = members(&control, &sidecar, 1);
    let mut original = control.begin_learned_probe_review(control.revision(), sidecar,
        schedule(), definitions, limits(), snapshot()).unwrap();
    assert_eq!(drive(&mut control, &mut original), FileLearnedProbeStatus::Finished);
    assert_eq!(run(&h.driver).evaluations(), original.evaluations());
    assert_eq!(run(&h.driver).input_revision(), original.input_revision());
    assert_eq!(run(&h.driver).records(), original.records());
    assert_eq!(run(&h.driver).input(), original.input());
    assert_eq!(h.driver.supervisor().host().unwrap().inspect().executions, 0);
}
