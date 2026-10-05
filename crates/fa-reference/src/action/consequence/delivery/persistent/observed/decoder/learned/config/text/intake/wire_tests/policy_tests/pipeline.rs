//! Real socket intents, real policy files, native helper models and two keys.
//! Synthetic weights are controls, not trained-detector or isolation evidence.
use super::*;
use super::peer::{bound, connect, drain, input, policy, send};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit,
    driver::{FileSupervisedDriver, FileDriverEvent, FileDriverPhase,
        native_learned::{FileNativeDriverLaunch, FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}};
use crate::action::consequence::oversight::{ReviewWindow,
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::{SidecarProbeQuery, SidecarDecisionBasis}}};
use std::collections::BTreeSet;

// Reuse the original native evaluator, never a supplied verdict or a second
// interpretation of helper output. It executes ByteBpe/model/monitor/stop logic.
#[path = "../../../../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn review(mut driver: FileSupervisedDriver, request: u64, round: u64, spelling: &[u8],
    source: &mut FileEvidenceSource) -> FileNativeSupervisedDriver
{
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("original socket request must be admitted before review"); };
        let evidence = host.machine.broker.learned_decoder_evidence(attempt).unwrap().unwrap();
        let options = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + request,
            generation: 1, transform_id: 7 }, priority: evidence.audit().source().groups().collect(),
            budget: SidecarCongressBudget::default() };
        let actor_revision = host.learned_generation_inspection().unwrap().numerical.actor_revision;
        let revision = host.revision();
        let sidecar = host.begin_learned_sidecar_plan(revision, attempt, actor_revision, options).unwrap();
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
    };
    let observed = source.read().unwrap();
    driver.start_native_learned_sequence(launch, observed.snapshot().clone(), ElapsedTick(1)).unwrap()
}
fn next(run: &mut FileNativeSupervisedDriver, source: &mut FileEvidenceSource,
    human: Option<&FileHumanPermit>) -> FileNativeDriverEvent
{
    let report = run.step_from_policy_file(source, || ElapsedTick(1), human);
    assert!(report.source_updates.is_empty());
    report.result.unwrap()
}
fn complete(run: &mut FileNativeSupervisedDriver, source: &mut FileEvidenceSource) {
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { break; }
        assert!(matches!(next(run, source, None), FileNativeDriverEvent::Review(_)));
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert!(run.review().records().values().flat_map(|round| round.members.values().filter_map(Option::as_ref))
        .any(|record| record.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
}
fn effect(run: &mut FileNativeSupervisedDriver, source: &mut FileEvidenceSource,
    human: Option<&FileHumanPermit>) -> FileDriverEvent
{
    match next(run, source, human) {
        FileNativeDriverEvent::Driver(event) => event,
        other => panic!("completed review must yield to original publication: {other:?}"),
    }
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer, key: u64) -> FileHumanPermit {
    let request = run.request_human_approval(key, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn socket_originated_generated_text_requires_actual_native_allow_and_independent_human_key() {
    for spelling in [b"allow".as_slice(), b"deny".as_slice()] {
        let root = Directory::new(); let config = config(); let (host, reviewer) = owner(&root, &config);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        write(&path(&root), &capture(1)); let mut source = source(&root);
        let socket = path(&root).with_extension("actor.sock"); let mut listener = bound(port, &socket, policy());
        let mut client = connect(&socket); send(&mut client, &raw_document(71));
        let report = supervisor.poll_learned_text_listener_from_policy_file(&mut listener,
            &mut source, || ElapsedTick(1), input(true)).unwrap();
        assert_eq!(report.drive.unwrap().unwrap().intakes[0].result, Ok(capture(1).identity()));
        assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 71 }));
        assert_eq!(supervisor.host().unwrap().request_action(71).unwrap().spec().payload, b"aa");
        let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, spelling, &mut source);
        complete(&mut run, &mut source);
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
        if spelling == b"deny" {
            assert_eq!(run.progress().phase, FileDriverPhase::Idle);
            assert!(run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).is_err());
            let reads = source.status().read_attempts;
            send(&mut client, &encode_command(&Command::Poll { request: 71 }).unwrap());
            let report = run.supervisor_mut().poll_learned_text_listener_from_policy_file(&mut listener,
                &mut source, no_clock, input(false)).unwrap();
            assert!(report.drive.unwrap().unwrap().intakes.is_empty());
            assert!(!matches!(drain(&mut listener, &mut client).result,
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
            assert_eq!(source.status().read_attempts, reads); continue;
        }
        assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::AwaitingHuman { request: 71 }));
        assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
        let human = approve(&mut run, &reviewer, 1001);
        assert!(matches!(effect(&mut run, &mut source, Some(&human)), FileDriverEvent::Dispatched { request: 71, .. }));
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
        assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::PublicationChecked { .. }));
        assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::Reconciled { .. }));
        let reads = source.status().read_attempts;
        send(&mut client, &encode_command(&Command::Poll { request: 71 }).unwrap());
        let report = run.supervisor_mut().poll_learned_text_listener_from_policy_file(&mut listener,
            &mut source, no_clock, input(false)).unwrap();
        assert!(report.drive.unwrap().unwrap().intakes.is_empty());
        assert!(matches!(drain(&mut listener, &mut client).result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert_eq!(source.status().read_attempts, reads);
        let stored = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
        assert_eq!(stored.payload, b"aa"); assert_eq!(stored.executions, 1); assert_eq!(stored.control.ledger.charged, 2);
    }
}

#[test]
fn socket_stream_finish_waits_for_receipt_then_requires_new_native_review_and_human_key() {
    let root = Directory::new(); let config = stream_config(); let (host, reviewer) = stream_owner(&root, &config);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    write(&path(&root), &capture(1)); let mut source = source(&root);
    let socket = path(&root).with_extension("actor.sock"); let mut listener = bound(port, &socket, policy());
    let mut client = connect(&socket); send(&mut client, &stream_document(71, LearnedTextRelease::Message));
    let report = supervisor.poll_learned_text_stream_listener_from_policy_file(&mut listener,
        &mut source, || ElapsedTick(1), input(true)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 71 }));
    let message_cost = supervisor.host().unwrap().request_action(71).unwrap().spec().units;
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow", &mut source);
    complete(&mut run, &mut source); let human = approve(&mut run, &reviewer, 1001);
    assert!(matches!(effect(&mut run, &mut source, Some(&human)), FileDriverEvent::Dispatched { request: 71, .. }));
    for published in [false, true] {
        if published {
            assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::PublicationChecked { .. }));
            let view = run.supervisor().host().unwrap().stream_snapshot().unwrap();
            assert_eq!(view.published.visible(), b"aa"); assert!(view.confirmed.visible().is_empty());
        }
        let reads = source.status().read_attempts;
        send(&mut client, &stream_document(72, LearnedTextRelease::Finish));
        let report = run.supervisor_mut().poll_learned_text_stream_listener_from_policy_file(&mut listener,
            &mut source, no_clock, input(false)).unwrap();
        assert!(report.drive.unwrap().unwrap().intakes.is_empty());
        assert_eq!(drain(&mut listener, &mut client).result, Err(WireError::Unavailable));
        assert_eq!(source.status().read_attempts, reads);
    }
    assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::Reconciled { .. }));
    let handoff = run.into_handoff().unwrap(); let mut driver = handoff.driver;
    send(&mut client, &stream_document(72, LearnedTextRelease::Finish));
    let report = driver.supervisor_mut().poll_learned_text_stream_listener_from_policy_file(&mut listener,
        &mut source, || ElapsedTick(1), input(false)).unwrap();
    assert_eq!(report.drive.unwrap().unwrap().intakes.len(), 1);
    assert_eq!(drain(&mut listener, &mut client).result, Ok(Knowledge::Pending { request: 72 }));
    let finish_cost = {
        let host = driver.supervisor().host().unwrap(); let spec = host.request_action(72).unwrap().spec();
        let frame = ReleaseFrame::decode(&spec.payload).unwrap();
        assert!(frame.is_finish()); assert_eq!(frame.prior_messages(), &["aa"]);
        assert_eq!(spec.units, spec.payload.len() as u64); spec.units
    };
    let mut run = review(driver, 72, 201, b"allow", &mut source); complete(&mut run, &mut source);
    assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::AwaitingHuman { request: 72 }));
    assert!(run.step_from_policy_file(&mut source, || ElapsedTick(1), Some(&human)).result.is_err());
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 1);
    let finish_human = approve(&mut run, &reviewer, 1002);
    assert!(matches!(effect(&mut run, &mut source, Some(&finish_human)), FileDriverEvent::Dispatched { request: 72, .. }));
    assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::Reconciled { .. }));
    let reads = source.status().read_attempts;
    for (request, release) in [(71, LearnedTextRelease::Message), (72, LearnedTextRelease::Finish)] {
        send(&mut client, &stream_document(request, release));
        let report = run.supervisor_mut().poll_learned_text_stream_listener_from_policy_file(&mut listener,
            &mut source, no_clock, input(false)).unwrap();
        assert!(report.drive.unwrap().unwrap().intakes.is_empty());
        assert!(matches!(drain(&mut listener, &mut client).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    }
    assert_eq!(source.status().read_attempts, reads);
    let host = run.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
    assert!(view.confirmed.finished() && view.published.finished()); assert_eq!(view.confirmed.visible(), b"aa");
    assert_eq!(view.publication.executions, 2); assert_eq!(view.publication.control.ledger.charged, message_cost + finish_cost);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(handoff.review.status(), NativeReviewStatus::Finished);
}
