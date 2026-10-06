//! Recovered custody in original identity, policy, native review and stream paths.
use super::*;
use crate::action::consequence::delivery::persistent::{Reconciliation, observed::{
    FileHumanPermit, guarded::FileIdentityRequirement,
    source::FileSourcePolicy, identity::{FileIdentityObserver, FileLearnedIdentityInput},
    driver::{FileSupervisedDriver, FileDriverEvent, native_learned::{
        FileNativeDriverLaunch, FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}},
    requests::actor::LearnedTextRelease};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::activation::{probe::LinearProbe,
    identity::{ModelManifest, ModelPassport, IdentityAnchor},
    tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS}};
use crate::action::consequence::oversight::{ReviewWindow,
    identity::{IdentityPolicy, IdentityOutcome}, decoder_monitoring::LearnedDecoderBindingLimits,
    policy_state::{StateSource, StateLimits, StateFreshness},
    evidence_source::{EvidenceSnapshot, EvidenceIdentity, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES},
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::SidecarProbeQuery}};
use std::collections::BTreeSet;
#[path = "../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn native_review(mut driver: FileSupervisedDriver, request: u64, round: u64) -> FileNativeSupervisedDriver {
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("original request must be admitted before review"); };
        let evidence = host.machine.broker.learned_decoder_evidence(attempt).unwrap().unwrap();
        let options = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + request,
            generation: 1, transform_id: 7 }, priority: evidence.audit().source().groups().collect(),
            budget: SidecarCongressBudget::default() };
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
                view.actual_input().input_profile().clone(), b"allow"), queries, salt: vec![16 + index as u8; 32] })
        }).collect();
        FileNativeDriverLaunch { journal_revision: host.revision(), request, sidecar,
            rounds: vec![LearnedWorkerRound { round, evidence_root: [7; 32],
                window: ReviewWindow { commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) } }],
            rosters: [(round, roster)].into_iter().collect(), limits: NativeReviewLimits::default() }
    };
    let now = driver.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
    let mut run = driver.start_native_learned_sequence(launch, snapshot(), now).unwrap();
    for _ in 0..MAX_LEARNED_REVIEW_POLLS {
        if run.review().status() != NativeReviewStatus::Running { break; }
        assert!(matches!(run.step(|| now, || Ok(snapshot()), None).unwrap(), FileNativeDriverEvent::Review(_)));
    }
    assert_eq!(run.review().status(), NativeReviewStatus::Finished); run
}
fn effect(run: &mut FileNativeSupervisedDriver, human: Option<&FileHumanPermit>) -> FileDriverEvent {
    let now = run.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
    match run.step(|| now, || Ok(snapshot()), human).unwrap() {
        FileNativeDriverEvent::Driver(event) => event, other => panic!("expected original effect, got {other:?}"),
    }
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer, key: u64) -> FileHumanPermit {
    let now = run.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
    let request = run.request_human_approval(key, ElapsedTick(80), now).unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn passive_read_uses_one_canonical_image_beside_a_locked_or_faulted_owner_without_fencing() {
    let root = Directory::new(); let (mut host, _, config) = at_prompt(&root);
    let expected = expected(&host, &config); let before = bytes(&root);
    let view = FileOversight::read_predictive_consistency_with_learned_generation(
        root.store(), &profile(), &expected, &config).unwrap();
    assert_eq!(view.journal, host.inspect()); assert_eq!(view.consistency, host.action_consistency_snapshot().unwrap());
    assert!(view.credibility.is_none()); assert_eq!(bytes(&root), before);
    assert!(FileOversight::read_predictive_consistency(root.store(), &profile(), &expected).is_err());
    let original = host.inspect(); host.store.fail_once(JournalIo::DirectorySync);
    assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err());
    assert_eq!(host.inspect(), original); assert!(host.storage_failure().is_some());
    let disk = bytes(&root);
    let view = FileOversight::read_predictive_consistency_with_learned_generation(
        root.store(), &profile(), &expected, &config).unwrap();
    assert_eq!(view.journal.revision, original.revision + 1);
    assert_eq!(view.journal.control.ledger.elapsed, Some(ElapsedTick(2)));
    assert_eq!(view.consistency.evidence.samples(), 0);
    assert_eq!(bytes(&root), disk); assert!(host.storage_failure().is_some());
    drop(host);
    let (host, _) = FileOversight::open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    assert_eq!(host.revision(), view.journal.revision + 1); independent(&host, &config);
}

#[test]
fn corrupted_numerical_witness_fails_both_projection_and_recovery_without_role_or_write() {
    use crate::action::consequence::delivery::persistent::observed::decoder::{DecoderEvent, learned::LearnedEvent};
    let root = Directory::new(); let (host, _, config) = at_prompt(&root);
    let expected = expected(&host, &config); let original = bytes(&root); let mut events = host.events.clone();
    let witness = events.iter_mut().find_map(|event| match event {
        Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step { witness, .. })) => Some(witness), _ => None,
    }).unwrap();
    let mut altered = witness.to_vec(); *altered.last_mut().unwrap() ^= 1; *witness = altered.into();
    let changed = journal::encode(&host.profile, host.store.identity(), &events).unwrap(); drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &changed).unwrap();
    assert!(FileOversight::read_predictive_consistency_with_learned_generation(
        root.store(), &profile(), &expected, &config).is_err());
    let mut run = FileOversight::begin_open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    assert!(run.advance(0, run.progress().total_events).is_err());
    assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
    assert!(run.finish().is_err()); assert_eq!(bytes(&root), changed);
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    assert!(FileOversight::read_predictive_consistency_with_learned_generation(
        root.store(), &profile(), &expected, &config).is_ok());
}

fn passport() -> ModelPassport {
    let (model, _, _) = recipe();
    ModelPassport::new(51, 1, ModelManifest { tenant: 1, model: 2, model_generation: 3, host_generation: 1,
        tokenizer_generation: 4, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32] }, vec![IdentityAnchor::new(10,
            model.residual_contract(1).unwrap().profile(), 5, vec![u32::from(b'p')],
            &[[-2.0, 2.0], [-2.0, 2.0]]).unwrap()]).unwrap()
}
fn identity_policy() -> IdentityPolicy { IdentityPolicy { observer_id: 99, timeout_ticks: 10, validity_ticks: 20, max_checks: 8 } }
fn qualify(host: &mut FileOversight, role: &FileIdentityObserver, id: u64, sequence: u64, now: ElapsedTick) {
    let control = host.inspect().control; let n = host.learned_generation_inspection().unwrap().numerical;
    let challenge = host.begin_identity_check(host.revision(), id, control.sequence, n.actor_revision).unwrap().unwrap();
    let revision = host.revision();
    let result = role.observe_computed_learned(host, revision, &challenge, FileLearnedIdentityInput {
        measurement_sequence: sequence, budget: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
        observed_manifest: passport().manifest().clone() }, || now).unwrap();
    assert_eq!(result.observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
    assert_eq!(result.work.entered_tokens, 1);
    let basis = challenge.evidence();
    host.apply_identity_check(host.revision(), &challenge, basis.control_sequence(), basis.revocation_epoch()).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
}

#[test]
fn recovered_identity_and_predictive_roles_require_original_computed_requalification_and_fresh_policy() {
    let root = Directory::new();
    let source_policy = FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() };
    let config = pinned().with_required_computed_identity().unwrap().with_required_policy_source(source_policy).unwrap();
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let old_identity = host.enable_identity_checks(host.revision(), passport(), identity_policy()).unwrap();
    let old_forecast = host.enable_learned_generation_with_pre_output_forecast(host.revision(), config.clone()).unwrap();
    let capture = EvidenceSnapshot::new(EvidenceIdentity { source: 42, generation: 1, scope: profile().delivery.scope },
        snapshot(), ["alpha", "beta"].into_iter().map(|s| (s.to_owned(), Vec::new())).collect()).unwrap();
    let path = root.store().with_extension("policy"); std::fs::write(&path, capture.encode()).unwrap();
    let mut source = FileEvidenceSource::new(path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    qualify(&mut host, &old_identity, 901, 1, ElapsedTick(1)); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let mut expected = expected(&host, &config);
    expected.oversight.guards.identity = Some(FileIdentityRequirement { passport: passport(), policy: identity_policy() });
    let before = bytes(&root); drop(host);
    let mut missing = expected.clone(); missing.oversight.guards.identity = None;
    assert!(FileOversight::open_predictive_guarded_with_learned_generation(root.store(), profile(), &missing, &config).is_err());
    assert_eq!(bytes(&root), before);
    let (mut host, roles) = FileOversight::open_predictive_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config).unwrap();
    let identity = roles.oversight.identity_observer.as_ref().unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    let revision = host.revision();
    assert_eq!(old_forecast.forecast_hosted_request(&mut host, revision, 71, n.actor_revision).err(), Some(Error::Binding.into()));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2)).unwrap();
    // Actual fresh original-model measurements, not restoring the old installation.
    qualify(&mut host, identity, 902, 2, ElapsedTick(2));
    assert!(host.learned_generation_inspection().unwrap().paused, "identity match does not resume generation");
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    begin_forecast(&mut host, &roles.consistency_observer); step(&mut host); step(&mut host);
    let epoch = host.inspect().control.ledger.epoch;
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let report = supervisor.prepare_learned_policy_intake(&mut source, || ElapsedTick(2));
    assert!(!report.source_updates.is_empty()); report.result.unwrap();
    let mut proposal = proposal(); proposal.expected_policy_epoch = epoch;
    let ticket = port.submit(71, proposal).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
    port.cancel(&ticket).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.available, 100);
    independent(&supervisor.host().unwrap(), &config);
}

#[test]
fn recovered_native_reviewed_dispatch_is_query_only_whether_publication_became_visible_or_not() {
    for published in [false, true] {
        let root = Directory::new(); let config = pinned();
        let (mut host, reviewer, observer) = FileOversight::create_with_pre_output_forecast(root.store(), profile(), config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
        begin_forecast(&mut host, &observer); step(&mut host); step(&mut host);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot()); port.submit(71, proposal()).unwrap();
        let mut run = native_review(FileSupervisedDriver::new(supervisor), 71, 101);
        let human = approve(&mut run, &reviewer, 1001);
        assert!(matches!(effect(&mut run, Some(&human)), FileDriverEvent::Dispatched { .. }));
        if published { assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. })); }
        let expected = expected(&run.supervisor().host().unwrap(), &config);
        let view = FileOversight::read_predictive_consistency_with_learned_generation(
            root.store(), &profile(), &expected, &config).unwrap();
        assert_eq!(view.journal.executions, u64::from(published)); assert_eq!(view.consistency.evidence.samples(), 1);
        drop(run);
        let (host, _) = FileOversight::open_predictive_guarded_with_learned_generation(
            root.store(), profile(), &expected, &config).unwrap();
        assert!(host.learned_generation_inspection().unwrap().paused);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let (_, mut driver) = host.into_supervised_driver(); driver.resume_reconciliation(71).unwrap();
        let event = driver.step_with_learned_evidence(|| ElapsedTick(2),
            || panic!("query-only settlement must not read policy or resume inference"), None).unwrap();
        if published {
            assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. }));
        } else { assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. })); }
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(published)); assert_eq!(host.inspect().control.ledger.available, 84);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, view.consistency.evidence);
        independent(&host, &config);
    }
}

#[test]
fn recovered_observer_can_forecast_a_receipt_confirmed_stream_finish_without_repeating_output() {
    let root = Directory::new(); let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let (model, tokenizer, source) = recipe();
    let config = FileLearnedConfig::new_text_stream(model, tokenizer, source,
        LearnedDecoderBindingLimits::default(), stream).unwrap().with_required_sidecar().unwrap()
        .with_required_pre_output_forecast(predictor().with_stream_messages(stream).unwrap()).unwrap();
    let mut profile = profile(); profile.delivery.initial_payload.clear(); profile.delivery.total = 1000;
    let (mut host, reviewer, observer) = FileOversight::create_with_pre_output_forecast(root.store(), profile.clone(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    begin_forecast(&mut host, &observer); step(&mut host); step(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let mut run = native_review(FileSupervisedDriver::new(supervisor), 71, 101);
    let human = approve(&mut run, &reviewer, 1001);
    assert!(matches!(effect(&mut run, Some(&human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    let expected = expected(&run.supervisor().host().unwrap(), &config); drop(run);
    let (mut host, roles) = FileOversight::open_predictive_guarded_with_learned_generation(
        root.store(), profile.clone(), &expected, &config).unwrap();
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    let revision = host.revision();
    let forecast = roles.consistency_observer.forecast_hosted_request(&mut host, revision, 72, numerical.actor_revision).unwrap().unwrap();
    assert!(forecast.observation().frame().sequence > 1);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let ticket = port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
    let mut run = native_review(FileSupervisedDriver::new(supervisor), 72, 201);
    let request = run.request_human_approval(1002, ElapsedTick(80), ElapsedTick(2)).unwrap();
    let next_human = {
        let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        assert_eq!(reviewer.approve(&mut host, revision, &request).err(), Some(Error::Binding.into()));
        roles.oversight.human.approve(&mut host, revision, &request).unwrap()
    };
    assert!(run.step(|| ElapsedTick(2), || Ok(snapshot()), Some(&human)).is_err());
    assert!(matches!(effect(&mut run, Some(&next_human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let host = run.supervisor().host().unwrap(); let stream = host.stream_snapshot().unwrap();
    assert!(stream.confirmed.finished()); assert_eq!(stream.confirmed.visible(), b"aa");
    assert_eq!(host.inspect().executions, 2); assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    let view = FileOversight::read_predictive_consistency_with_learned_generation(
        root.store(), &profile, &expected, &config).unwrap();
    assert_eq!(view.journal, host.inspect()); independent(&host, &config);
}
