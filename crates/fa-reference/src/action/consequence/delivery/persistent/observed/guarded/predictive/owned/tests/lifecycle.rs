//! Same-cut inspection and recovered original identity/native-publication roles.
//! Actual numerical engines and synthetic control weights, not imported verdicts.
use super::*;
use crate::action::consequence::delivery::persistent::{Reconciliation, observed::{
    FileHumanPermit, guarded::FileIdentityRequirement,
    source::FileSourcePolicy, identity::{FileIdentityObserver, FileLearnedIdentityInput},
    driver::{FileSupervisedDriver, FileDriverEvent, native_learned::{
        FileNativeDriverLaunch, FileNativeSupervisedDriver, FileNativeDriverEvent}},
    helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus}},
    requests::{FileRequestDisposition, actor::LearnedTextRelease}};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::activation::{probe::LinearProbe,
    identity::{ModelManifest, ModelPassport, IdentityAnchor},
    tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS}};
use crate::action::consequence::oversight::{ReviewWindow,
    identity::{IdentityPolicy, IdentityOutcome}, decoder_monitoring::LearnedDecoderBindingLimits,
    policy_state::{StateSource, StateLimits, StateFreshness},
    evidence_source::{EvidenceSnapshot, EvidenceIdentity, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES},
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::{SidecarProbeQuery, SidecarDecisionBasis}}};
use std::collections::BTreeSet;
#[path = "../../../../driver/native_learned/native_fixture.rs"]
mod native_fixture;

fn review(mut driver: FileSupervisedDriver, request: u64, round: u64, spelling: &[u8])
    -> FileNativeSupervisedDriver
{
    let launch = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
            else { panic!("original request must be admitted before congress"); };
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
                view.actual_input().input_profile().clone(), spelling), queries, salt: vec![16 + index as u8; 32] })
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
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert!(run.review().records().values().flat_map(|round| round.members.values().filter_map(Option::as_ref))
        .any(|record| record.progress.basis == Some(SidecarDecisionBasis::NativeModel)));
    run
}
fn effect(run: &mut FileNativeSupervisedDriver, human: Option<&FileHumanPermit>) -> FileDriverEvent {
    let now = run.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
    match run.step(|| now, || Ok(snapshot()), human).unwrap() {
        FileNativeDriverEvent::Driver(event) => event,
        other => panic!("expected original publication, got {other:?}"),
    }
}
fn approve(run: &mut FileNativeSupervisedDriver, reviewer: &FileHumanReviewer, key: u64) -> FileHumanPermit {
    let now = run.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
    let request = run.request_human_approval(key, ElapsedTick(80), now).unwrap();
    let mut host = run.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn read_only_guarded_inspection_uses_the_same_canonical_cut_even_under_a_faulted_writer_lock() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, _, observer, generation) = at_prompt(&root);
        forecast(&mut host, &observer, 71);
        let required = expected(&host, &generation, predictor(true)); let anchor = host.history_anchor().unwrap();
        let original = host.inspect(); let before = bytes(&root);
        let view = FileOversight::read_predictive_guarded_anchored_with_owned_learned_consistency(
            root.store(), &profile(), &required, &generation, &anchor).unwrap();
        assert_eq!(view.journal.publication, original);
        assert_eq!(view.journal.consistency, host.learned_action_consistency_snapshot().unwrap());
        assert_eq!(view.journal.pending_request, Some(71)); assert!(view.credibility.is_none());
        assert_eq!(bytes(&root), before);
        host.store.fail_once(barrier);
        assert!(matches!(host.observe_time(host.revision(), ElapsedTick(2)),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(host.inspect(), original); assert!(host.storage_failure().is_some());
        let disk = bytes(&root);
        let view = FileOversight::read_predictive_guarded_anchored_with_owned_learned_consistency(
            root.store(), &profile(), &required, &generation, &anchor).unwrap();
        let native = FileOversight::read_publication_with_owned_learned_consistency(
            root.store(), &profile(), &generation, &required.prediction).unwrap();
        assert_eq!(view.journal, native);
        assert!((original.revision..=original.revision + 1).contains(&view.journal.publication.revision));
        assert_eq!(view.journal.pending_request, Some(71));
        assert!(!view.journal.consistency.consistency.coverage_lost, "inspection does not fence");
        assert_eq!(bytes(&root), disk);
        let mut too_new = required.clone(); too_new.oversight.minimum.journal_revision = view.journal.publication.revision + 1;
        assert!(FileOversight::read_predictive_guarded_with_owned_learned_consistency(
            root.store(), &profile(), &too_new, &generation).is_err());
        assert_eq!(bytes(&root), disk); assert!(host.storage_failure().is_some());
    }
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
fn qualify(host: &mut FileOversight, role: &FileIdentityObserver, old: Option<&FileIdentityObserver>,
    id: u64, sequence: u64, now: ElapsedTick) {
    let control = host.inspect().control; let n = host.learned_generation_inspection().unwrap().numerical;
    let challenge = host.begin_identity_check(host.revision(), id, control.sequence, n.actor_revision).unwrap().unwrap();
    let revision = host.revision();
    let input = || FileLearnedIdentityInput { measurement_sequence: sequence,
        budget: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
        observed_manifest: passport().manifest().clone() };
    if let Some(old) = old {
        assert_eq!(old.observe_computed_learned(host, revision, &challenge, input(), || now).err(),
            Some(JournalError::Contract(Error::Binding)));
        assert_eq!(host.revision(), revision);
    }
    let result = role.observe_computed_learned(host, revision, &challenge, input(), || now).unwrap();
    assert_eq!(result.observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
    assert_eq!(result.work.entered_tokens, 1);
    let basis = challenge.evidence();
    host.apply_identity_check(host.revision(), &challenge, basis.control_sequence(), basis.revocation_epoch()).unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
}

#[test]
fn real_identity_and_policy_guards_recover_custody_without_restoring_old_qualification() {
    let root = Directory::new();
    let policy = FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() };
    let generation = config().with_required_computed_identity().unwrap().with_required_policy_source(policy).unwrap();
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let old_identity = host.enable_identity_checks(host.revision(), passport(), identity_policy()).unwrap();
    host.enable_learned_generation(host.revision(), generation.clone()).unwrap();
    let old_forecast = host.enable_learned_action_consistency(host.revision(), predictor(true)).unwrap();
    let capture = EvidenceSnapshot::new(EvidenceIdentity { source: 42, generation: 1, scope: profile().delivery.scope },
        snapshot(), ["alpha", "beta"].into_iter().map(|s| (s.to_owned(), Vec::new())).collect()).unwrap();
    let path = root.store().with_extension("policy"); std::fs::write(&path, capture.encode()).unwrap();
    let mut source = FileEvidenceSource::new(path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    qualify(&mut host, &old_identity, None, 901, 1, ElapsedTick(1)); step(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let mut required = expected(&host, &generation, predictor(true));
    required.oversight.guards.identity = Some(FileIdentityRequirement { passport: passport(), policy: identity_policy() });
    let before = bytes(&root); drop(host);
    for omit_identity in [false, true] {
        let mut missing = required.clone();
        if omit_identity { missing.oversight.guards.identity = None; }
        else { missing.oversight.guards.source = None; }
        assert!(FileOversight::open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile(), &missing, &generation).is_err());
        assert!(FileOversight::read_predictive_guarded_with_owned_learned_consistency(
            root.store(), &profile(), &missing, &generation).is_err());
        assert_eq!(bytes(&root), before);
    }
    let (mut host, roles) = FileOversight::open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile(), &required, &generation).unwrap();
    let identity = roles.oversight.identity_observer.as_ref().unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture.identity()));
    let revision = host.revision();
    assert_eq!(old_forecast.forecast_owned_learned_request(&mut host, revision, 71, n.actor_revision).err(),
        Some(Error::Binding.into()));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2)).unwrap();
    qualify(&mut host, identity, Some(&old_identity), 902, 2, ElapsedTick(2));
    assert!(host.learned_generation_inspection().unwrap().paused);
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    forecast(&mut host, &roles.consistency_observer, 71); step(&mut host); step(&mut host);
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
    independent(&supervisor.host().unwrap(), &generation, &required.prediction);
}

#[test]
fn native_reviewed_unknown_dispatches_recover_query_only_without_forecast_or_generation_resume() {
    for published in [false, true] {
        let root = Directory::new(); let (mut host, human, observer, generation) = at_prompt(&root);
        forecast(&mut host, &observer, 71); step(&mut host); step(&mut host);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
        let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow");
        let permit = approve(&mut run, &human, 1001);
        assert!(matches!(effect(&mut run, Some(&permit)), FileDriverEvent::Dispatched { request: 71, .. }));
        if published { assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. })); }
        let host = run.supervisor().host().unwrap();
        let required = expected(&host, &generation, predictor(true));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let costs = host.learned_action_consistency_snapshot().unwrap(); drop(host);
        let view = FileOversight::read_predictive_guarded_with_owned_learned_consistency(
            root.store(), &profile(), &required, &generation).unwrap();
        assert_eq!(view.journal.publication.executions, u64::from(published));
        assert_eq!(view.journal.consistency, costs);
        drop(run); assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
        let (host, _) = FileOversight::open_predictive_guarded_with_owned_learned_consistency(
            root.store(), profile(), &required, &generation).unwrap();
        assert!(host.learned_generation_inspection().unwrap().paused);
        let (port, supervisor) = host.into_learned_text_actor_gateway().unwrap();
        let ticket = port.submit(71, proposal()).unwrap(); // exact historical key, no observation
        let mut driver = FileSupervisedDriver::new(supervisor); driver.resume_reconciliation(71).unwrap();
        let event = driver.step_with_evidence(|| ElapsedTick(2), |_, _| panic!("query-only recovery must not read evidence"), None).unwrap();
        if published {
            assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(_), .. }));
            assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        } else {
            assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
            assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
        }
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(published));
        assert_eq!(host.inspect().control.ledger.available, 84);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, costs.work);
        independent(&host, &generation, &required.prediction);
    }
}

#[test]
fn receipt_confirmed_stream_finish_after_guarded_recovery_requires_a_later_forecast_and_fresh_human_key() {
    let root = Directory::new(); let stream = StreamProfile::new(61, 1, 8, 32, 128).unwrap();
    let (model, tokenizer, source) = recipe(); let mut profile = profile();
    profile.delivery.initial_payload.clear(); profile.delivery.total = 1000;
    let generation = FileLearnedConfig::new_text_stream(model, tokenizer, source,
        LearnedDecoderBindingLimits::default(), stream).unwrap().with_required_sidecar().unwrap();
    let raw = predictor(false).consistency().clone().with_stream_messages(stream).unwrap();
    let prediction = FileLearnedConsistencyConfig::new(raw, 1, KvSide::Value,
        LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
        .unwrap().with_owned_generation().unwrap().with_pre_output_forecast().unwrap();
    let (mut host, human) = FileOversight::create_with_learned_text_stream(root.store(), profile.clone(), generation.clone()).unwrap();
    let observer = host.enable_learned_action_consistency(host.revision(), prediction.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    forecast(&mut host, &observer, 71); step(&mut host); step(&mut host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let message = port.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let message_cost = supervisor.host().unwrap().request_action(71).unwrap().spec().units;
    let mut run = review(FileSupervisedDriver::new(supervisor), 71, 101, b"allow");
    let old_human = approve(&mut run, &human, 1001);
    assert!(matches!(effect(&mut run, Some(&old_human)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&message), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let required = expected(&run.supervisor().host().unwrap(), &generation, prediction);
    drop(run);
    let (mut host, roles) = FileOversight::open_predictive_guarded_with_owned_learned_consistency(
        root.store(), profile.clone(), &required, &generation).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"aa");
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_learned_generation(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    forecast(&mut host, &roles.consistency_observer, 72);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let finish = port.submit(72, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
    let finish_cost = supervisor.host().unwrap().request_action(72).unwrap().spec().units;
    assert!(!supervisor.host().unwrap().learned_action_consistency_observation(2).unwrap().event());
    let mut run = review(FileSupervisedDriver::new(supervisor), 72, 201, b"allow");
    assert!(matches!(effect(&mut run, None), FileDriverEvent::AwaitingHuman { request: 72 }));
    assert!(run.step(|| ElapsedTick(2), || Ok(snapshot()), Some(&old_human)).is_err());
    let fresh = approve(&mut run, &roles.oversight.human, 1002);
    assert!(matches!(effect(&mut run, Some(&fresh)), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(effect(&mut run, None), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&finish), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let host = run.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(host.inspect().control.ledger.charged, message_cost + finish_cost);
    assert!(host.stream_snapshot().unwrap().confirmed.finished());
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"aa");
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
    independent(&host, &generation, &required.prediction);
}
