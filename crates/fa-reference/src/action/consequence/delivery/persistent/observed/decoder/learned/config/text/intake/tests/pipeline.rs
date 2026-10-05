//! Generated text reaches native helper models and the ORIGINAL two-key endpoint.
//! Actual original inference supplies both the actor message and helper verdicts;
//! synthetic parameters do not demonstrate detector effectiveness or independence.
use super::*;
use super::stream::{stream_config, stream_delivery_profile, stream_owner};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::delivery::persistent::{Reconciliation,
    requests::{FileRequestDisposition, actor::LearnedTextRelease}};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit,
    driver::{FileSupervisedDriver, FileDriverEvent, FileDriverPhase,
        native_learned::{FileNativeDriverLaunch, FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}};
use crate::action::consequence::delivery::stream::ReleaseFrame;
use crate::action::consequence::oversight::{ReviewWindow,
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::{SidecarProbeQuery, SidecarDecisionBasis}}};
use std::collections::BTreeSet;

#[path = "../../../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn review(mut driver: FileSupervisedDriver, request: u64, round: u64, spelling: &[u8])
    -> FileNativeSupervisedDriver
{
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("source-generated request must enter the original authority"); };
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
                view.actual_input().input_profile().clone(), spelling), queries,
                salt: vec![16 + index as u8; 32] })
        }).collect();
        FileNativeDriverLaunch { journal_revision: host.revision(), request, sidecar,
            rounds: vec![LearnedWorkerRound { round, evidence_root: [7; 32],
                window: ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) } }],
            rosters: [(round, roster)].into_iter().collect(), limits: NativeReviewLimits::default() }
    };
    driver.start_native_learned_sequence(launch, snapshot(), ElapsedTick(1)).unwrap()
}
fn complete(run: &mut FileNativeSupervisedDriver) {
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { break; }
        assert!(matches!(run.step(|| ElapsedTick(1), || Ok(snapshot()), None).unwrap(), FileNativeDriverEvent::Review(_)));
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert!(run.review().records().values().flat_map(|round| round.members.values().filter_map(Option::as_ref))
        .any(|record| record.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer, key: u64) -> FileHumanPermit {
    let request = run.request_human_approval(key, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}
fn tick(run: &mut FileNativeSupervisedDriver, human: Option<&FileHumanPermit>) -> FileDriverEvent {
    match run.step(|| ElapsedTick(1), || Ok(snapshot()), human).unwrap() {
        FileNativeDriverEvent::Driver(event) => event,
        other => panic!("finished native review must yield to the original effect driver: {other:?}"),
    }
}

#[test]
fn actual_source_message_is_published_only_after_native_allow_and_both_keys() {
    for spelling in [b"allow".as_slice(), b"deny".as_slice()] {
        let root = Directory::new(); let config = config(); let (host, reviewer) = owner(&root, &config);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
        let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, spelling);
        complete(&mut run);
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
        if spelling == b"deny" {
            assert_eq!(run.progress().phase, FileDriverPhase::Idle);
            assert!(run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).is_err());
            assert!(!matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
            continue;
        }
        assert!(matches!(tick(&mut run, None), FileDriverEvent::AwaitingHuman { request: 71 }));
        assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
        let human = approve(&mut run, &reviewer, 1001);
        assert!(matches!(tick(&mut run, Some(&human)), FileDriverEvent::Dispatched { request: 71, .. }));
        assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
        assert!(matches!(tick(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
        assert!(matches!(tick(&mut run, None), FileDriverEvent::Reconciled { .. }));
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        let stored = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
        assert_eq!(stored.payload, b"aa"); assert_eq!(stored.executions, 1);
        assert_eq!(port.poll(&port.submit(71, proposal()).unwrap()), port.poll(&ticket));
        assert!(matches!(tick(&mut run, Some(&human)), FileDriverEvent::Idle));
    }
}

#[test]
fn generated_stream_message_and_finish_each_require_native_judgment_and_new_keys() {
    let root = Directory::new(); let config = stream_config(); let (host, reviewer) = stream_owner(&root, &config);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot());
    let message = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let message_cost = supervisor.host().unwrap().request_action(71).unwrap().spec().units;
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow"); complete(&mut run);
    assert!(matches!(tick(&mut run, None), FileDriverEvent::AwaitingHuman { .. }));
    let human = approve(&mut run, &reviewer, 1001);
    assert!(matches!(tick(&mut run, Some(&human)), FileDriverEvent::Dispatched { .. }));
    observe(run.supervisor_mut(), snapshot());
    assert!(matches!(port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)), Err(ActorError::Unavailable)));
    assert_eq!(port.poll(&port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap()), port.poll(&message));
    assert!(matches!(tick(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    let view = run.supervisor().host().unwrap().stream_snapshot().unwrap();
    assert_eq!(view.published.visible(), b"aa"); assert!(view.confirmed.visible().is_empty());
    observe(run.supervisor_mut(), snapshot());
    assert!(matches!(port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)), Err(ActorError::Unavailable)));
    assert!(matches!(tick(&mut run, None), FileDriverEvent::Reconciled { .. }));
    let handoff = run.into_handoff().unwrap(); let mut driver = handoff.driver;
    observe(driver.supervisor_mut(), snapshot());
    assert!(matches!(port.submit(73, LearnedTextRelease::Message, ElapsedTick(100)), Err(ActorError::Unavailable)));
    // The same unconsumed observation now admits the only available release.
    let finish = port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
    let finish_cost = {
        let host = driver.supervisor().host().unwrap(); let spec = host.request_action(72).unwrap().spec();
        let frame = ReleaseFrame::decode(&spec.payload).unwrap();
        assert!(frame.is_finish()); assert_eq!(frame.prior_messages(), &["aa"]);
        assert_eq!(spec.units, spec.payload.len() as u64); spec.units
    };
    let mut run = review(driver, 72, 201, b"allow"); complete(&mut run);
    assert!(matches!(tick(&mut run, None), FileDriverEvent::AwaitingHuman { request: 72 }));
    // The first message's human capability cannot authorize this new frame.
    // The failed dispatch may retain an automatic reservation, never a send.
    assert!(run.step(|| ElapsedTick(1), || Ok(snapshot()), Some(&human)).is_err());
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 1);
    let finish_human = approve(&mut run, &reviewer, 1002);
    assert!(matches!(tick(&mut run, Some(&finish_human)), FileDriverEvent::Dispatched { request: 72, .. }));
    assert!(matches!(tick(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(tick(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&message), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert!(matches!(port.poll(&finish), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let host = run.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
    assert_eq!(view.confirmed.visible(), b"aa"); assert_eq!(view.published.visible(), b"aa");
    assert!(view.confirmed.finished() && view.published.finished());
    assert_eq!(view.publication.executions, 2);
    assert_eq!(view.publication.control.ledger.charged, message_cost + finish_cost);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    let revision = host.revision(); drop(host);
    assert_eq!(port.poll(&port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap()), port.poll(&message));
    assert_eq!(port.poll(&port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap()), port.poll(&finish));
    assert_eq!(run.supervisor().host().unwrap().revision(), revision);
    let retained = FileOversight::read_stream_publication_with_learned_generation(root.store(), &stream_delivery_profile(), &config).unwrap();
    assert_eq!(retained.publication.executions, 2); assert!(retained.published.finished());
    assert_eq!(handoff.review.status(), NativeReviewStatus::Finished);
}

#[test]
fn reopened_unknown_generated_append_retains_charge_without_output_or_numerical_resumption() {
    let root = Directory::new(); let config = stream_config(); let (host, reviewer) = stream_owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let old = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let cost = supervisor.host().unwrap().request_action(71).unwrap().spec().units;
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow"); complete(&mut run);
    let human = approve(&mut run, &reviewer, 1001);
    assert!(matches!(tick(&mut run, Some(&human)), FileDriverEvent::Dispatched { .. }));
    // There is no publication acknowledgment and no permission to retry sending.
    port.cancel(&old).unwrap(); drop(run); drop(human); drop(reviewer);
    let (host, _) = FileOversight::open_with_learned_text_stream(root.store(), stream_delivery_profile(), &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let retry = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert!(matches!(port.poll(&retry), Knowledge::Unknown { .. }));
    observe(&mut supervisor, snapshot());
    assert!(matches!(port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)), Err(ActorError::Unavailable)));
    let mut driver = FileSupervisedDriver::new(supervisor); driver.resume_reconciliation(71).unwrap();
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(2),
        || panic!("query-only settlement cannot need a policy snapshot or numerical source"), None).unwrap(),
        FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    let host = driver.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
    assert!(view.published.visible().is_empty() && !view.published.finished());
    assert_eq!(view.publication.executions, 0);
    assert_eq!(view.publication.control.ledger.available, 1000 - cost);
    assert!(host.learned_generation_inspection().unwrap().paused);
}
