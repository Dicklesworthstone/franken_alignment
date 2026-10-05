//! Real generated output, native helper inference and the original two-key gate.
//! All numerical parameters are synthetic controls, not detector qualifications.
use super::*;
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::delivery::persistent::requests::{FileRequestDisposition,
    actor::{FileLearnedTextActorPort, FileLearnedTextStreamActorPort, LearnedTextRelease}};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit,
    decoder::learned::sidecar::FileLearnedSidecar,
    driver::{FileSupervisedDriver, FileDriverEvent, FileDriverPhase,
        native_learned::{FileNativeDriverLaunch, FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}};
use crate::action::consequence::oversight::{ReviewWindow,
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::{SidecarProbeQuery, SidecarDecisionBasis}}};
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use crate::action::consequence::oversight::actor_wire::{ActorWire, Command, WireError, encode_command};
use std::collections::BTreeSet;

#[path = "../../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn intent() -> LearnedTextProposal { let mut proposal = proposal(); proposal.units = 2; proposal }
fn document(request: u64) -> Vec<u8> {
    encode_command(&Command::Submit { request,
        proposal: FileLearnedTextActorPort::encode_request(request, intent()).unwrap() }).unwrap()
}
fn attempt(host: &FileOversight, request: u64) -> u64 {
    match host.request_status(request).unwrap().disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        _ => panic!("expected original admitted request"),
    }
}
fn sidecar(driver: &mut FileSupervisedDriver, request: u64) -> FileLearnedSidecar {
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    let attempt = attempt(&host, request);
    let evidence = host.machine.broker.learned_decoder_evidence(attempt).unwrap().unwrap();
    let options = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + request,
        generation: 1, transform_id: 7 }, priority: evidence.audit().source().groups().collect(),
        budget: SidecarCongressBudget::default() };
    let actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let revision = host.revision();
    host.begin_learned_sidecar_plan(revision, attempt, actor_revision, options).unwrap()
}
fn launch(driver: &mut FileSupervisedDriver, request: u64, round: u64, spelling: &[u8]) -> FileNativeDriverLaunch {
    let sidecar = sidecar(driver, request);
    let host = driver.supervisor().host().unwrap();
    let original = host.checked_learned_sidecar(&sidecar).unwrap();
    let rows: BTreeSet<_> = original.source().groups().map(|group| group.row).collect();
    let roster = original.round().input().views().iter().enumerate().map(|(index, (name, view))| {
        let queries = rows.iter().map(|row| {
            let (frame, heads, channels) = original.source().row_shape(*row).unwrap();
            SidecarProbeQuery { row: *row, probe: LinearProbe::new(1, 1, frame.profile,
                &vec![0.0; heads * channels], 0.0, 1.0).unwrap() }
        }).collect();
        (name.clone(), NativeReviewMember { evaluator: native_fixture::evaluator(
            view.actual_input().input_profile().clone(), spelling), queries, salt: vec![16 + index as u8; 32] })
    }).collect();
    FileNativeDriverLaunch { journal_revision: host.revision(), request, sidecar,
        rounds: vec![LearnedWorkerRound { round, evidence_root: [7; 32],
            window: ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) } }],
        rosters: [(round, roster)].into_iter().collect(), limits: NativeReviewLimits::default() }
}
fn start(mut driver: FileSupervisedDriver, request: u64, round: u64, spelling: &[u8]) -> FileNativeSupervisedDriver {
    let launch = launch(&mut driver, request, round, spelling);
    driver.start_native_learned_sequence(launch, snapshot(), ElapsedTick(1)).unwrap()
}
fn complete(run: &mut FileNativeSupervisedDriver, source: &mut FileEvidenceSource, now: u64) {
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { break; }
        let report = run.step_from_policy_file(source, || ElapsedTick(now), None);
        assert_eq!(report.observations, vec![Ok(capture(1).identity())]);
        assert_eq!(report.source_updates, vec![Ok(capture(1).identity())]);
        assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Review(_)));
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert!(run.review().records().values().flat_map(|round| round.members.values().filter_map(Option::as_ref))
        .any(|record| record.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer,
    source: &mut FileEvidenceSource, key: u64, now: u64) -> FileHumanPermit
{
    let report = run.request_human_approval_from_policy_file(source, key, ElapsedTick(80), || ElapsedTick(now));
    assert_eq!(report.source_updates, vec![Ok(capture(1).identity())]);
    let request = report.result.unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}
fn tick(run: &mut FileNativeSupervisedDriver, source: &mut FileEvidenceSource,
    human: Option<&FileHumanPermit>, now: u64) -> FileDriverEvent
{
    match run.step_from_policy_file(source, || ElapsedTick(now), human).result.unwrap() {
        FileNativeDriverEvent::Driver(event) => event,
        other => panic!("original completed native review must yield to the effect driver: {other:?}"),
    }
}
struct Raw {
    run: FileNativeSupervisedDriver,
    wire: ActorWire<FileLearnedTextActorPort>,
    port: FileLearnedTextActorPort,
    reviewer: FileHumanReviewer,
    source: FileEvidenceSource,
    config: FileLearnedConfig,
    root: Directory,
}
fn native(spelling: &[u8]) -> Raw {
    let root = Directory::new(); let config = configured(); let (host, reviewer) = owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port.clone()); let mut source = reader(&root);
    publish(&root.store().with_extension("policy"), &capture(1));
    let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire,
        &document(71), &mut source, || ElapsedTick(1)).unwrap();
    assert_eq!(report.response.result, Ok(Knowledge::Pending { request: 71 }));
    assert_eq!(report.intake.unwrap().source_updates, vec![Ok(capture(1).identity())]);
    let run = start(FileSupervisedDriver::new(supervisor), 71, 101, spelling);
    Raw { run, wire, port, reviewer, source, config, root }
}

#[test]
fn durable_wire_intake_and_native_publication_require_both_original_keys() {
    for spelling in [b"allow".as_slice(), b"deny".as_slice()] {
        let mut s = native(spelling); complete(&mut s.run, &mut s.source, 1);
        assert_eq!(s.run.supervisor().host().unwrap().inspect().executions, 0);
        if spelling == b"deny" {
            assert_eq!(s.run.progress().phase, FileDriverPhase::Idle);
            let reads = s.source.status().read_attempts;
            assert!(s.run.request_human_approval_from_policy_file(&mut s.source, 1001,
                ElapsedTick(80), || panic!("denied review cannot read a clock or request a key")).result.is_err());
            assert_eq!(s.source.status().read_attempts, reads); continue;
        }
        assert!(matches!(tick(&mut s.run, &mut s.source, None, 1), FileDriverEvent::AwaitingHuman { request: 71 }));
        let human = approve(&mut s.run, &s.reviewer, &mut s.source, 1001, 1);
        let reads = s.source.status().read_attempts;
        let report = s.run.step_from_policy_file(&mut s.source, || ElapsedTick(1), Some(&human));
        assert_eq!(report.source_updates, vec![Ok(capture(1).identity()); 2]);
        assert_eq!(s.source.status().read_attempts, reads + 2, "reservation cannot reuse its earlier snapshot");
        assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::Dispatched { .. })));
        assert_eq!(s.run.supervisor().host().unwrap().inspect().executions, 0);
        assert!(matches!(tick(&mut s.run, &mut s.source, None, 1), FileDriverEvent::PublicationChecked { .. }));
        let reads = s.source.status().read_attempts;
        assert!(matches!(tick(&mut s.run, &mut s.source, None, 1), FileDriverEvent::Reconciled { .. }));
        assert_eq!(s.source.status().read_attempts, reads);
        let poll = encode_command(&Command::Poll { request: 71 }).unwrap();
        assert!(matches!(s.wire.exchange(&poll).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let retried = s.port.submit(71, intent()).unwrap();
        assert!(matches!(s.port.poll(&retried), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert_eq!(s.source.status().read_attempts, reads);
        let stored = FileOversight::read_publication_with_learned_generation(s.root.store(), &profile(), &s.config).unwrap();
        assert_eq!(stored.payload, b"aa"); assert_eq!(stored.executions, 1); assert_eq!(stored.control.ledger.charged, 2);
        assert_eq!(s.run.supervisor().host().unwrap().file_source_status().unwrap().producer, Some(capture(1).identity()));
    }
}

#[test]
fn expired_policy_can_be_refreshed_during_native_review_and_human_request_without_rebuilding_evidence() {
    let mut s = native(b"allow");
    let (attempt, revision, numerical) = {
        let host = s.run.supervisor().host().unwrap(); let attempt = attempt(&host, 71);
        (attempt, host.input_revision(attempt).unwrap(), host.learned_generation_inspection().unwrap().numerical)
    };
    { let mut host = s.run.supervisor_mut().host_mut().unwrap(); let cut = host.revision(); host.observe_time(cut, ElapsedTick(6)).unwrap(); }
    assert!(s.run.supervisor().host().unwrap().machine.checked_learned_sidecar(attempt).is_err());
    complete(&mut s.run, &mut s.source, 6);
    assert_eq!(s.run.supervisor().host().unwrap().input_revision(attempt).unwrap(), revision);
    { let mut host = s.run.supervisor_mut().host_mut().unwrap(); let cut = host.revision(); host.observe_time(cut, ElapsedTick(12)).unwrap(); }
    assert!(s.run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(12)).is_err());
    let human = approve(&mut s.run, &s.reviewer, &mut s.source, 1001, 12);
    assert!(matches!(tick(&mut s.run, &mut s.source, Some(&human), 12), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(tick(&mut s.run, &mut s.source, None, 12), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(tick(&mut s.run, &mut s.source, None, 12), FileDriverEvent::Reconciled { .. }));
    let host = s.run.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.input_revision(attempt).unwrap(), revision);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
}

#[test]
fn expired_post_read_intake_cannot_return_a_ticket_or_spend_an_older_snapshot() {
    let root = Directory::new(); let (host, _) = owner(&root, &configured());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port.clone()); let mut source = reader(&root);
    publish(&root.store().with_extension("policy"), &capture(1));
    observe(&mut supervisor, snapshot());
    let mut calls = 0;
    let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire, &document(71),
        &mut source, || { calls += 1; ElapsedTick(if calls == 1 { 2 } else { 7 }) }).unwrap();
    assert_eq!(calls, 2); assert_eq!(report.response.result, Err(WireError::Unavailable));
    let intake = report.intake.unwrap(); assert!(intake.result.is_err());
    assert_eq!(intake.source_updates, vec![Ok(capture(1).identity())]);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    assert!(port.submit(71, intent()).is_err(), "no stale or partially prepared snapshot");
    let report = supervisor.exchange_learned_text_actor_from_policy_file(&mut wire, &document(71),
        &mut source, || ElapsedTick(8)).unwrap();
    assert_eq!(report.response.result, Ok(Knowledge::Pending { request: 71 }));
    assert!(report.intake.unwrap().result.is_ok());
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn a_file_context_packet_cannot_register_required_learned_provenance() {
    let root = Directory::new(); let (host, _) = owner(&root, &configured());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    publish(&root.store().with_extension("policy"), &capture(1));
    supervisor.prepare_learned_policy_intake(&mut reader(&root), || ElapsedTick(1)).result.unwrap();
    port.submit(71, intent()).unwrap();
    let mut host = supervisor.host_mut().unwrap(); let attempt = attempt(&host, 71);
    let action = host.request_action(71).unwrap().clone();
    let replacement = capture(1).inputs_for(&action, &profile().committee).unwrap();
    let revision = host.revision();
    host.record_inputs(revision, attempt, 0, replacement.clone()).unwrap();
    let revision = host.revision();
    assert!(matches!(host.begin_review(revision, attempt, 101, [7; 32],
        ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) }, snapshot()),
        Err(JournalError::Contract(Error::Incomplete))));
    let revision = host.revision();
    assert!(host.authorize(revision, attempt, &replacement, snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    assert!(host.machine.broker.capture_policy_state().is_ok(), "failure is not stale policy");
}

mod failures;
mod stream;
mod probes;
