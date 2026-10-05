//! Original actor tickets, native model judgments, probes and real publication.
//! These synthetic detector coefficients are controls, not detector qualification.
use super::*;
use super::super::{FileDriverEvent, FileDriverPhase};
use super::super::super::{FileHumanReviewer, FileOversightProfile};
use super::super::super::decoder::learned::FileLearnedConfig;
use super::super::super::super::{JournalIo, requests::actor::{FileActorPort, FileActorTicket}};
use crate::action::FrozenAction;
use crate::action::consequence::oversight::{actor::{ActorProposal, ActorOutcome, Knowledge},
    learned_host::sidecar::LearnedSidecarRequest,
    sidecar::{SidecarCongressBudget, SidecarIdentity, receiver::native::{SidecarProbeQuery, SidecarDecisionBasis}},
    supervised::DriverEvidence};
use super::super::super::helpers::learned::{FileLearnedProbeReview, FileLearnedProbeStatus,
    native::{NativeReviewMember, NativeMemberRecord}};
use crate::action::consequence::oversight::helper_client::native::{NativeEvaluationStatus, NativeEvaluationWork};
use crate::action::consequence::oversight::learned_host::sidecar::workers::{LearnedWorkerSchedule,
    probes::{ProbeReviewMember, ProbeReviewLimits}};
use std::collections::BTreeMap;
#[path = "native_fixture.rs"]
mod native_fixture;

// Reuse the original numerical model and policies, not another success fixture.
#[allow(dead_code)]
#[path = "../../helpers/learned/tests/fixture.rs"]
mod fixture;
use fixture::*;

type Port = FileActorPort<FileOversight>;
type Ticket = FileActorTicket<FileOversight>;

fn pending(root: &Directory, mode: u8, spelling: &[u8])
    -> (Port, Ticket, FileHumanReviewer, FileSupervisedDriver, FileNativeDriverLaunch)
{
    let (mut host, reviewer) = owner(root, &config());
    step(&mut host); step(&mut host);
    let (port, mut driver) = host.into_supervised_driver();
    let proposal = {
        let host = driver.supervisor().host().unwrap();
        ActorProposal { target: host.inspect().target, payload: b"visible".to_vec(), units: 16,
            deadline: ElapsedTick(100), expected_policy_epoch: host.inspect().control.ledger.epoch }
    };
    let revision = driver.supervisor().host().unwrap().revision();
    driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
    let ticket = port.submit(41, &proposal).unwrap();
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let (attempt, _) = admitted(host.request_status(41).unwrap()).unwrap();
        let evidence = host.machine.broker.learned_decoder_evidence(attempt).unwrap().unwrap();
        let request = LearnedSidecarRequest {
            identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: evidence.audit().source().groups().collect(), budget: SidecarCongressBudget::default(),
        };
        let actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
        let revision = host.revision();
        let sidecar = host.begin_learned_sidecar_plan(revision, attempt, actor_revision, request).unwrap();
        let definitions = members(&host, &sidecar, mode);
        let rounds = schedule().rounds;
        let original = host.current_learned_sidecar(&sidecar).unwrap();
        let rosters = rounds.iter().map(|round| {
            let roster = definitions.iter().map(|(name, member)| {
                let profile = original.views()[name].actual_input().input_profile().clone();
                let queries = member.probes.iter().flat_map(|(row, probes)| probes.iter().map(move |probe|
                    SidecarProbeQuery { row: *row, probe: probe.clone() })).collect();
                (name.clone(), NativeReviewMember { evaluator: native_fixture::evaluator(profile, spelling),
                    queries, salt: vec![round.round as u8; 32] })
            }).collect();
            (round.round, roster)
        }).collect();
        FileNativeDriverLaunch { journal_revision: host.revision(), request: 41, sidecar,
            rounds, rosters, limits: NativeReviewLimits::default() }
    };
    (port, ticket, reviewer, driver, launch)
}
fn start(root: &Directory, mode: u8, spelling: &[u8]) -> (Port, Ticket, FileHumanReviewer, FileNativeSupervisedDriver) {
    let (port, ticket, reviewer, driver, launch) = pending(root, mode, spelling);
    let run = driver.start_native_learned_sequence(launch, snapshot(), ElapsedTick(1)).unwrap();
    (port, ticket, reviewer, run)
}
fn complete(run: &mut FileNativeSupervisedDriver) {
    for _ in 0..crate::action::consequence::oversight::learned_host::sidecar::workers::MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { return; }
        run.advance(run.review().revision(), ElapsedTick(1), snapshot()).unwrap();
    }
    panic!("original numerical review exceeded its fixed schedule");
}
fn progress(driver: &mut FileSupervisedDriver, input: &crate::action::consequence::oversight::CommitteeInput,
    human: Option<&super::super::FileHumanPermit>, tick: u64) -> Result<FileDriverEvent, JournalError>
{
    driver.step_with_evidence(|| ElapsedTick(tick), |_, _| Ok(DriverEvidence {
        inputs: Some(input.clone()), snapshot: snapshot(),
    }), human)
}

#[test]
fn refinement_hands_the_original_actor_ticket_to_both_keys_and_one_real_publication() {
    let root = Directory::new();
    let (port, ticket, reviewer, run) = start(&root, 1, b"allow");
    let initial = run.review().input().clone();
    let mut run = *run.into_handoff().unwrap_err();
    assert_eq!(run.progress().polls, 0);
    complete(&mut run);
    assert_eq!(run.progress().status, NativeReviewStatus::Finished);
    assert!(run.review().history().len() > 1);
    assert!(matches!(run.review().history().first(), Some(FileLearnedSidecarFinish::Refined { .. })));
    assert!(native_records(run.review()).any(|r| r.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
    let FileNativeDriverHandoff { mut driver, review } = run.into_handoff().unwrap();
    assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 41 });
    let input = review.input().clone();
    assert_ne!(input, initial);
    assert!(driver.supervisor().host().unwrap().input_revision(1).unwrap() > 1);
    assert!(driver.request_human_approval(1000, &initial, ElapsedTick(80), ElapsedTick(1)).is_err());
    assert!(matches!(progress(&mut driver, &input, None, 1).unwrap(), FileDriverEvent::AwaitingHuman { request: 41 }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    let request = driver.request_human_approval(1001, &input, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let human = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        reviewer.approve(&mut host, revision, &request).unwrap()
    };
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert!(matches!(progress(&mut driver, &input, Some(&human), 1).unwrap(), FileDriverEvent::Dispatched { request: 41, .. }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert!(matches!(progress(&mut driver, &input, None, 2).unwrap(), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(progress(&mut driver, &input, None, 2).unwrap(), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let published = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config()).unwrap();
    assert_eq!(published.executions, 1); assert_eq!(published.payload, b"visible");
    assert_eq!(published.control.ledger.charged, 16);
    assert!(matches!(progress(&mut driver, &input, Some(&human), 2).unwrap(), FileDriverEvent::Idle));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
    assert!(native_records(&review).any(|r| r.progress.completed_probes > 0));
}

#[test]
fn actual_alarm_completes_both_members_but_cannot_handoff_a_ready_job() {
    let root = Directory::new(); let (_, _, _, mut run) = start(&root, 2, b"allow");
    complete(&mut run);
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert!(native_records(run.review()).any(|r| r.progress.basis == Some(SidecarDecisionBasis::NumericalAlarm)));
    let FileNativeDriverHandoff { mut driver, review } = run.into_handoff().unwrap();
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    assert!(driver.request_human_approval(1001, review.input(), ElapsedTick(80), ElapsedTick(1)).is_err());
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    let host = driver.supervisor().host().unwrap();
    assert_ne!(host.inspect().control.decisions.get(&1), Some(&Consequence::Continue));
}

#[test]
fn stale_setup_returns_the_same_live_gateway_without_consuming_its_request() {
    let root = Directory::new(); let (port, ticket, _, driver, mut launch) = pending(&root, 0, b"allow");
    let revision = launch.journal_revision;
    launch.journal_revision -= 1;
    let error = driver.start_native_learned_sequence(launch, snapshot(), ElapsedTick(2)).unwrap_err();
    assert_eq!(error.error, JournalError::Contract(Error::Stale));
    assert_eq!(error.driver.supervisor().host().unwrap().revision(), revision);
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 41 }));
    assert_eq!(error.driver.phase(), FileDriverPhase::Idle);
}

#[test]
fn foreign_plan_refuses_before_time_or_review_and_returns_the_original_owner() {
    let root = Directory::new(); let other = Directory::new();
    let (port, ticket, _, driver, mut launch) = pending(&root, 0, b"allow");
    let (_, _, _, _other_driver, other_launch) = pending(&other, 0, b"allow");
    let revision = launch.journal_revision;
    launch.sidecar = other_launch.sidecar;
    let error = driver.start_native_learned_sequence(launch, snapshot(), ElapsedTick(2)).unwrap_err();
    assert_eq!(error.error, JournalError::Contract(Error::Binding));
    assert_eq!(error.driver.supervisor().host().unwrap().revision(), revision);
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
}

#[test]
fn actor_cancellation_is_seen_before_snapshot_admission_or_more_scoring() {
    let root = Directory::new(); let (port, ticket, _, mut run) = start(&root, 0, b"allow");
    run.advance(0, ElapsedTick(1), snapshot()).unwrap();
    run.advance(1, ElapsedTick(1), snapshot()).unwrap();
    let before = costs(run.review());
    assert_eq!(before.values().map(|r| r.0).sum::<usize>(), 2);
    port.cancel(&ticket).unwrap();
    run.advance(2, ElapsedTick(0), Snapshot::default()).unwrap();
    assert_eq!(run.review().status(), NativeReviewStatus::Cancelled);
    assert_eq!(costs(run.review()), before);
    let parts = run.into_handoff().unwrap();
    assert_eq!(parts.driver.phase(), FileDriverPhase::Idle);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(parts.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn stale_quantum_and_incomplete_snapshot_leave_a_usable_positive_review() {
    let root = Directory::new(); let (_, _, _, mut run) = start(&root, 0, b"allow");
    let revision = run.supervisor().host().unwrap().revision();
    assert_eq!(run.advance(1, ElapsedTick(2), snapshot()).unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(run.advance(0, ElapsedTick(2), Snapshot::default()).unwrap_err(), JournalError::Contract(Error::Incomplete));
    assert_eq!(run.supervisor().host().unwrap().revision(), revision);
    assert!(native_records(run.review()).all(|r| r.progress.completed_probes == 0 && !r.progress.native_started));
    assert_eq!(run.cancel(1), Err(JournalError::Contract(Error::Stale)));
    complete(&mut run);
    assert_eq!(run.progress().phase, FileDriverPhase::AwaitingDispatch { request: 41 });
}

#[test]
fn source_loss_retains_real_probe_work_and_cannot_become_ready() {
    let root = Directory::new(); let (_, _, _, mut run) = start(&root, 0, b"allow");
    run.advance(0, ElapsedTick(1), snapshot()).unwrap();
    run.advance(1, ElapsedTick(1), snapshot()).unwrap();
    let before = costs(run.review());
    { let mut host = run.supervisor_mut().host_mut().unwrap(); step(&mut host); }
    assert!(run.advance(2, ElapsedTick(1), snapshot()).is_err());
    assert_eq!(run.review().status(), NativeReviewStatus::Failed);
    assert_eq!(costs(run.review()), before);
    let parts = run.into_handoff().unwrap();
    assert_eq!(parts.driver.phase(), FileDriverPhase::Idle);
    assert!(parts.review.history().is_empty());
    assert_eq!(parts.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn explicit_cancellation_preserves_costs_and_uses_the_original_request_cancel() {
    let root = Directory::new(); let (port, ticket, _, mut run) = start(&root, 0, b"allow");
    run.advance(0, ElapsedTick(1), snapshot()).unwrap();
    run.advance(1, ElapsedTick(1), snapshot()).unwrap();
    let before = costs(run.review());
    run.cancel(2).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    let parts = run.into_handoff().unwrap();
    assert_eq!(parts.review.status(), NativeReviewStatus::Cancelled);
    assert_eq!(costs(&parts.review), before);
    assert_eq!(parts.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn original_storage_failure_retains_computation_without_ready_or_candidate_receipt() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (_, _, _, mut run) = start(&root, 0, b"allow");
        // Queue the failure before native computation. The next actual original
        // journal write is the commit message, never an invented success seam.
        run.supervisor_mut().host_mut().unwrap().store.fail_once(barrier);
        let error = loop {
            match run.advance(run.review().revision(), ElapsedTick(1), snapshot()) {
                Ok(_) => assert_eq!(run.review().status(), NativeReviewStatus::Running),
                Err(error) => break error,
            }
        };
        let JournalError::Io(failure) = error else { panic!("expected original storage fault"); };
        assert_eq!(failure.operation, barrier);
        let before = costs(run.review());
        assert!(native_records(run.review()).any(|r| r.progress.native_started));
        let parts = run.into_handoff().unwrap();
        assert_eq!(parts.driver.phase(), FileDriverPhase::Idle);
        assert_eq!(costs(&parts.review), before);
        assert!(parts.review.history().is_empty());
        let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config()).unwrap();
        assert_eq!(disk.executions, 0); assert_eq!(disk.payload, b"initial");
        drop(parts);
        let (recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config()).unwrap();
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.inspect().executions, 0);
    }
}

#[test]
fn identical_quiet_probes_do_not_override_a_native_models_deny() {
    for (spelling, ready) in [(b"allow".as_slice(), true), (b"deny".as_slice(), false)] {
        let root = Directory::new(); let (_, _, _, mut run) = start(&root, 0, spelling);
        complete(&mut run);
        assert!(native_records(run.review()).any(|r| r.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
        assert!(native_records(run.review()).all(|r| r.progress.completed_probes == r.progress.declared_probes));
        let parts = run.into_handoff().unwrap();
        assert_eq!(matches!(parts.driver.phase(), FileDriverPhase::AwaitingDispatch { .. }), ready);
        assert_eq!(parts.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

fn native_records(run: &FileNativeSidecarSequence) -> impl Iterator<Item = &NativeMemberRecord> {
    run.records().values().flat_map(|round| round.members.values().filter_map(Option::as_ref))
}

// Cancellation can change status but cannot erase completed numerical work.
type CompletedCosts = BTreeMap<(u64, String), (
    usize, crate::action::consequence::activation::probe::learned::LearnedProbeWork, NativeEvaluationWork,
)>;
fn costs(run: &FileNativeSidecarSequence) -> CompletedCosts {
    run.records().iter().flat_map(|(round, record)| record.members.iter().filter_map(move |(name, value)|
        value.as_ref().map(|record| ((*round, name.clone()), (record.progress.completed_probes, record.progress.completed_probe_work, record.progress.native.work)))
    )).collect()
}

#[test]
fn native_token_cancellation_retains_actual_decoder_work_and_all_future_reservations() {
    let root = Directory::new(); let (port, ticket, _, mut run) = start(&root, 0, b"allow");
    let reserved = run.review().reservation();
    for _ in 0..crate::action::consequence::oversight::learned_host::sidecar::workers::MAX_LEARNED_REVIEW_POLLS {
        if native_records(run.review()).any(|r| r.progress.native.work.decoder.tokens > 0
            && r.progress.native.status == NativeEvaluationStatus::Running) { break; }
        run.advance(run.review().revision(), ElapsedTick(1), snapshot()).unwrap();
    }
    assert!(native_records(run.review()).any(|r| r.progress.native.work.decoder.tokens > 0));
    assert!(native_records(run.review()).all(|r| r.progress.basis != Some(SidecarDecisionBasis::NativeModel)));
    let before = costs(run.review());
    run.cancel(run.review().revision()).unwrap();
    assert_eq!(costs(run.review()), before);
    assert_eq!(run.review().reservation(), reserved);
    assert!(run.review().records().values().any(|round| !round.started));
    let parts = run.into_handoff().unwrap();
    assert_eq!(parts.driver.phase(), FileDriverPhase::Idle);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(costs(&parts.review), before);
}
