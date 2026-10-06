//! Original native review, real policy files and independent pinned recovery.
use super::*;
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{Event, journal, storage,
    decoder::{DecoderEvent, learned::{bind_history, LearnedEvent}}, machine::Machine,
    consistency::FileConsistencyParameters,
    source::FileSourcePolicy, FileHumanPermit,
    driver::{FileSupervisedDriver, FileDriverEvent, FileDriverPhase,
        native_learned::{FileNativeDriverLaunch, FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}};
use crate::action::consequence::delivery::persistent::requests::{FileRequestDisposition,
    actor::{FileActorSupervisor, LearnedTextProposal, LearnedTextRelease}};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::oversight::{ReviewWindow, actor::{ActorOutcome, Knowledge},
    consistency::ConsistencyStopPolicy, decoder_monitoring::LearnedDecoderBindingLimits,
    learned_source::LearnedEvidenceLimits,
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::SidecarProbeQuery},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES},
    policy_state::{StateSource, StateLimits, StateFreshness}};
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::Snapshot;
use std::collections::BTreeSet;
#[path = "../text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn policy() -> FileSourcePolicy {
    FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() }
}
fn exact(crossing: bool, stream: Option<StreamProfile>, policy_first: bool) -> FileLearnedConfig {
    let (model, tokenizer, source) = recipe();
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let forecast = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: model.residual_contract(1).unwrap().profile(),
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral,
            positive: if crossing { BinaryForecast::new(16_384, 49_152).unwrap() } else { neutral } },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap();
    let (config, forecast) = match stream {
        Some(stream) => (
            FileLearnedConfig::new_text_stream(model, tokenizer, source,
                LearnedDecoderBindingLimits::default(), stream).unwrap(),
            forecast.with_stream_messages(stream).unwrap()),
        None => (FileLearnedConfig::new_text(model, tokenizer, source,
            LearnedDecoderBindingLimits::default()).unwrap(), forecast),
    };
    let forecast = forecast.with_pre_output_forecast().unwrap()
        .with_terminal_stop(ConsistencyStopPolicy::new(9, 1, 900).unwrap()).unwrap();
    let config = config.with_required_sidecar().unwrap();
    if policy_first {
        config.with_required_policy_source(policy()).unwrap().with_required_pre_output_forecast(forecast).unwrap()
    } else {
        config.with_required_pre_output_forecast(forecast).unwrap().with_required_policy_source(policy()).unwrap()
    }
}
fn source(root: &Directory) -> FileEvidenceSource {
    let identity = EvidenceIdentity { source: 42, generation: 1, scope: profile().delivery.scope };
    let capture = EvidenceSnapshot::new(identity, snapshot(),
        ["alpha", "beta"].into_iter().map(|name| (name.to_owned(), Vec::new())).collect()).unwrap();
    let path = root.store().with_extension("policy");
    std::fs::write(&path, capture.encode()).unwrap();
    FileEvidenceSource::new(path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap()
}
fn ready(root: &Directory, crossing: bool, stream: Option<StreamProfile>, policy_first: bool)
    -> (FileOversight, FileHumanReviewer, FileConsistencyObserver, FileLearnedConfig, FileOversightProfile, FileEvidenceSource)
{
    let config = exact(crossing, stream, policy_first);
    let mut profile = profile();
    if stream.is_some() { profile.delivery.initial_payload.clear(); profile.delivery.total = 1000; }
    let (mut host, reviewer, observer) = FileOversight::create_with_pre_output_forecast(
        root.store(), profile.clone(), config.clone()).unwrap();
    assert_eq!(host.revision(), if stream.is_some() { 2 } else { 1 });
    assert!(host.policy_only_file_source_required());
    assert_eq!(host.machine.consistency.as_deref(), config.required_pre_output_forecast());
    let mut source = source(root);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    let mut forecast = observer.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).unwrap().unwrap();
    while forecast.numerical().status.is_active() {
        let revision = host.revision(); forecast.advance(&mut host, revision).unwrap().unwrap();
    }
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    (host, reviewer, observer, config, profile, source)
}
fn review(mut driver: FileSupervisedDriver, request: u64, round: u64,
    spelling: &[u8], source: &mut FileEvidenceSource) -> FileNativeSupervisedDriver
{
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("original request must be admitted before review"); };
        let evidence = host.machine.broker.learned_decoder_evidence(attempt).unwrap().unwrap();
        let options = LearnedSidecarRequest { identity: SidecarIdentity {
            object_id: 1000 + request, generation: 1, transform_id: 7 },
            priority: evidence.audit().source().groups().collect(), budget: SidecarCongressBudget::default() };
        let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
        let sidecar = host.begin_learned_sidecar_plan(revision, attempt, n.actor_revision, options).unwrap();
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
    let mut run = driver.start_native_learned_sequence(launch, snapshot(), ElapsedTick(1)).unwrap();
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { break; }
        let report = run.step_from_policy_file(source, || ElapsedTick(1), None);
        assert!(!report.source_updates.is_empty());
        assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Review(_)));
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    run
}
fn effect(run: &mut FileNativeSupervisedDriver, source: &mut FileEvidenceSource,
    human: Option<&FileHumanPermit>) -> FileDriverEvent
{
    match run.step_from_policy_file(source, || ElapsedTick(1), human).result.unwrap() {
        FileNativeDriverEvent::Driver(event) => event,
        other => panic!("expected original publication lifecycle, got {other:?}"),
    }
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer,
    source: &mut FileEvidenceSource, key: u64) -> FileHumanPermit
{
    let report = run.request_human_approval_from_policy_file(source, key, ElapsedTick(80), || ElapsedTick(1));
    assert!(!report.source_updates.is_empty());
    let request = report.result.unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn prepared_owner_bootstrap_issues_once_and_preserves_original_admission_and_observer_custody() {
    let root = Directory::new(); let config = exact(false, None, false);
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let observer = host.enable_learned_generation_with_pre_output_forecast(host.revision(), config.clone()).unwrap();
    let before = host.inspect();
    assert!(host.enable_learned_generation_with_pre_output_forecast(host.revision(), config.clone()).is_err());
    assert_eq!(host.inspect(), before); assert_eq!(host.machine.consistency.as_deref(), config.required_pre_output_forecast());
    let mut source = source(&root);
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap(); step(&mut host);
    let other_root = Directory::new();
    let (_, _, foreign) = FileOversight::create_with_pre_output_forecast(other_root.store(), profile(), config).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    assert_eq!(foreign.forecast_hosted_request(&mut host, revision, 71, n.actor_revision).err(), Some(Error::Binding.into()));
    let mut forecast = observer.begin_pre_output_request(&mut host, revision, 71, n.actor_revision, n.position).unwrap().unwrap();
    let revision = host.revision(); forecast.advance(&mut host, revision).unwrap().unwrap();
    assert_eq!(forecast.numerical().work.sampling_attempts, 1);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn both_wrapper_orders_preserve_real_policy_native_judgment_and_independent_human_approval() {
    for policy_first in [false, true] {
        for (crossing, spelling) in [(false, b"allow".as_slice()), (false, b"deny".as_slice()), (true, b"allow".as_slice())] {
            let root = Directory::new();
            let (host, reviewer, _, config, profile, mut source) = ready(&root, crossing, None, policy_first);
            let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
            let report = supervisor.prepare_learned_policy_intake(&mut source, || ElapsedTick(1));
            assert!(!report.source_updates.is_empty()); report.result.unwrap();
            let ticket = port.submit(71, proposal()).unwrap();
            if crossing {
                let host = supervisor.host().unwrap();
                assert!(host.action_consistency_snapshot().unwrap().evidence.crossed());
                assert!(host.inspect().stop.is_some()); assert_eq!(host.inspect().executions, 0);
                assert!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::NotAdmitted(_)));
                continue;
            }
            let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, spelling, &mut source);
            if spelling == b"deny" {
                assert_eq!(run.progress().phase, FileDriverPhase::Idle);
                assert!(run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).is_err());
                assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0); continue;
            }
            assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::AwaitingHuman { .. }));
            assert_eq!(run.supervisor().host().unwrap().inspect().control.ledger.available, 100);
            let human = approve(&mut run, &reviewer, &mut source, 1001);
            assert!(matches!(effect(&mut run, &mut source, Some(&human)), FileDriverEvent::Dispatched { .. }));
            assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::PublicationChecked { .. }));
            let reads = source.status().read_attempts;
            assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::Reconciled { .. }));
            assert_eq!(source.status().read_attempts, reads);
            assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
            let projection = FileOversight::read_publication_with_learned_generation(root.store(), &profile, &config).unwrap();
            assert_eq!(projection.payload, b"aa"); assert_eq!(projection.executions, 1);
            assert_eq!(projection.control.ledger.charged, 16);
        }
    }
}

#[test]
fn stream_bootstrap_keeps_forecast_roles_and_distinct_keys_for_receipt_confirmed_finish() {
    let root = Directory::new(); let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let (host, reviewer, observer, config, profile, mut source) = ready(&root, false, Some(stream), true);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    supervisor.prepare_learned_policy_intake(&mut source, || ElapsedTick(1)).result.unwrap();
    let message = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let message_cost = supervisor.host().unwrap().request_action(71).unwrap().spec().units;
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow", &mut source);
    let human = approve(&mut run, &reviewer, &mut source, 1001);
    assert!(matches!(effect(&mut run, &mut source, Some(&human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::PublicationChecked { .. }));
    run.supervisor_mut().prepare_learned_policy_intake(&mut source, || ElapsedTick(1)).result.unwrap();
    {
        let host = run.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
        assert_eq!(view.published.visible(), b"aa"); assert!(view.confirmed.visible().is_empty());
    }
    assert!(port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).is_err());
    assert_eq!(run.supervisor().host().unwrap().retained_requests(), 1);
    assert_eq!(run.supervisor().host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&message), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let handoff = run.into_handoff().unwrap(); let mut driver = handoff.driver;
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
        observer.forecast_hosted_request(&mut host, revision, 72, n.actor_revision).unwrap().unwrap();
    }
    driver.supervisor_mut().prepare_learned_policy_intake(&mut source, || ElapsedTick(1)).result.unwrap();
    let finish = port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
    let finish_cost = driver.supervisor().host().unwrap().request_action(72).unwrap().spec().units;
    let mut run = review(driver, 72, 201, b"allow", &mut source);
    assert!(run.step_from_policy_file(&mut source, || ElapsedTick(1), Some(&human)).result.is_err());
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 1);
    let finish_human = approve(&mut run, &reviewer, &mut source, 1002);
    assert!(matches!(effect(&mut run, &mut source, Some(&finish_human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, &mut source, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&finish), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(run.supervisor().host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 2);
    let stored = FileOversight::read_stream_publication_with_learned_generation(root.store(), &profile, &config).unwrap();
    assert!(stored.confirmed.finished()); assert_eq!(stored.confirmed.visible(), b"aa");
    assert_eq!(stored.publication.executions, 2);
    assert_eq!(stored.publication.control.ledger.charged, message_cost + finish_cost);
}

#[test]
fn unknown_effect_recovery_keeps_the_pinned_policy_and_forecast_without_source_reads_or_resumption() {
    let root = Directory::new();
    let (host, reviewer, observer, config, profile, mut source) = ready(&root, false, None, false);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    supervisor.prepare_learned_policy_intake(&mut source, || ElapsedTick(1)).result.unwrap();
    let ticket = port.submit(71, proposal()).unwrap();
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow", &mut source);
    let human = approve(&mut run, &reviewer, &mut source, 1001);
    assert!(matches!(effect(&mut run, &mut source, Some(&human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    drop(run); drop(human); drop(reviewer);
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile, &config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.machine.consistency.as_deref(), config.required_pre_output_forecast());
    assert_eq!(host.file_source_status().unwrap().producer.unwrap().generation, 1);
    let (_, mut driver) = host.into_supervised_driver();
    driver.resume_reconciliation(71).unwrap();
    let result = driver.step_with_learned_evidence(|| ElapsedTick(2),
        || panic!("query-only recovery cannot require a policy observation"), None).unwrap();
    assert!(matches!(result, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 84);
    let n = host.learned_generation_inspection().unwrap().numerical; let revision = host.revision();
    assert_eq!(observer.forecast_hosted_request(&mut host, revision, 72, n.actor_revision).err(), Some(Error::Binding.into()));
    assert!(host.enable_learned_generation_with_pre_output_forecast(revision, config).is_err());
}

#[test]
fn all_pinned_installation_write_failures_return_no_observer_and_do_not_weaken_recovery() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = exact(false, None, false);
        let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
        let before = host.inspect(); host.store.fail_once(barrier);
        assert!(matches!(host.enable_learned_generation_with_pre_output_forecast(host.revision(), config.clone()),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert!(host.storage_failure().is_some()); assert_eq!(host.inspect(), before);
        let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        let mut events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
        let installed = events.iter().any(|event| matches!(event, Event::Decoder(DecoderEvent::Learned(LearnedEvent::Enable(_)))));
        if installed {
            bind_history(&mut events, &config).unwrap();
            let candidate = Machine::replay(&host.profile, &events).unwrap();
            assert_eq!(candidate.consistency.as_deref(), config.required_pre_output_forecast());
        }
        drop(host);
        let reopened = FileOversight::open_with_learned_generation(root.store(), profile(), &config);
        if installed {
            let (mut recovered, _) = reopened.unwrap();
            assert!(recovered.learned_generation_inspection().unwrap().paused);
            assert_eq!(recovered.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert!(recovered.enable_learned_generation_with_pre_output_forecast(recovered.revision(), config).is_err());
            assert_eq!(recovered.inspect().executions, 0);
        } else {
            assert!(reopened.is_err(), "a missing mandatory bootstrap cannot become a generic successful recovery");
            assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
        }
    }
}
