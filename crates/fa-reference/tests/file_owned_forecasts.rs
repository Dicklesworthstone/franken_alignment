//! Actual owned decoder -> keyed pre-action prediction -> original effect gate.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/investigation_decoder.rs"] mod numerical;
#[allow(dead_code)]
#[path = "support/decoder_inputs.rs"] mod data;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::{SourceFrame, FrameIdentity};
use fa_reference::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{
    FileConsistencyConfig, FileConsistencyParameters, FileConsistencyObserver,
};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    predictive::{FilePredictiveRequirements, FilePredictiveRoles},
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::oversight::actor::{ActorProposal, ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::decoder_host::HostedStopPolicy;
use fa_reference::Error;
use ordinary::Directory;

fn raw_config() -> FileConsistencyConfig {
    FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 7, probe_generation: 1, profile: numerical::model().residual_contract(1).unwrap().profile(),
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 1.5,
        forecast: ForecastRegistration { domain: 8, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: BinaryForecast::new(16384, 49152).unwrap(),
            at_threshold: BinaryForecast::new(32768, 32768).unwrap(), positive: BinaryForecast::new(49152, 16384).unwrap() },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 5, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap()
}
fn guards(alarm: f32, stopping: bool) -> FileGuardSet {
    FileGuardSet { stream: None,
        decoder: Some(FileDecoderConfig::new(numerical::profile(), data::weights(), data::monitor(alarm),
            data::sampling(), 5, DecoderBindingLimits::default()).unwrap()),
        decoder_stop: stopping.then(|| HostedStopPolicy::new(1, 1, 7000).unwrap()),
        source: None, identity: None, campaigns: None, credential: None }
}
fn requirements(g: FileGuardSet) -> FilePredictiveRequirements {
    FilePredictiveRequirements { oversight: FileRecoveryRequirements {
        guards: g, effective_policy: ordinary::profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 },
    }, prediction: raw_config().with_hosted_residual(1).unwrap(), evaluation: None }
}
fn create(root: &Directory, alarm: f32, stopping: bool)
    -> (FileOversight, FilePredictiveRoles, FilePredictiveRequirements)
{
    let expected = requirements(guards(alarm, stopping));
    let (mut h, roles) = FileOversight::create_predictive_guarded(root.store(), ordinary::profile(),
        &expected.oversight.guards, None, expected.prediction.clone(), None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    (h, roles, expected)
}
fn force(h: &mut FileOversight, token: u32) -> MonitoredStep {
    let n = h.decoder_inspection().unwrap().numerical;
    h.advance_decoder_forced(h.revision(), n.actor_revision, n.position, token, numerical::budget()).unwrap().unwrap()
}
fn forecast(h: &mut FileOversight, observer: &FileConsistencyObserver, key: u64) {
    let revision = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    observer.forecast_hosted_request(h, revision, key, actor).unwrap().unwrap();
}
fn approve(h: &mut FileOversight, human: &FileHumanReviewer, request: u64, attempt: u64) -> ordinary::Keys {
    let action = h.request_action(request).unwrap().clone(); let inputs = ordinary::inputs(&action, b"complete evidence");
    ordinary::review_existing(h, attempt, attempt + 100, &inputs);
    let automatic = h.authorize(h.revision(), attempt, &inputs, ordinary::snapshot()).unwrap();
    let request = h.request_human_approval(h.revision(), attempt + 1000, attempt, &inputs,
        ElapsedTick(h.inspect().control.ledger.elapsed.unwrap().0 + 30)).unwrap();
    let revision = h.revision(); let human = human.approve(h, revision, &request).unwrap();
    ordinary::Keys { action, inputs, automatic, human, request }
}
fn fake(h: &FileOversight) -> SourceFrame {
    let n = h.decoder_inspection().unwrap().numerical;
    SourceFrame::capture(FrameIdentity { profile: numerical::model().residual_contract(1).unwrap().profile(),
        stream: 5, position: n.position - 1, sequence: n.position }, &[-100.0, 0.0]).unwrap()
}

#[test]
fn actual_actor_gateway_uses_owned_residual_and_still_requires_both_keys_and_reconciliation() {
    let root = Directory::new(); let (mut h, roles, expected) = create(&root, 100.0, false);
    assert_eq!(h.hosted_consistency_layer().unwrap(), Some(1));
    force(&mut h, 0); let numeric = h.decoder_inspection().unwrap().numerical;
    forecast(&mut h, &roles.consistency_observer, 9000);
    assert_eq!(h.decoder_inspection().unwrap().numerical, numeric);
    assert_eq!(h.pending_forecast_request().unwrap(), Some(9000));
    let raw = ordinary::spec(&h, b"first");
    let proposal = ActorProposal { target: raw.target.unwrap(), payload: raw.payload.clone(),
        expected_policy_epoch: raw.policy_epoch, deadline: raw.deadline, units: raw.units };
    let (actor, mut supervisor) = h.into_actor_gateway();
    let rev = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(rev, Some(ordinary::snapshot())).unwrap();
    let ticket = actor.submit(9000, &proposal).unwrap();
    assert!(matches!(actor.poll(&ticket), Knowledge::Pending { .. }));
    {
        let mut h = supervisor.host_mut().unwrap();
        assert_eq!(h.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        assert_eq!(h.action_consistency_observation(1).unwrap().prediction().forecast().null_numerator(), 16384);
        let keys = approve(&mut h, &roles.oversight.human, 9000, 1);
        ordinary::dispatch(&mut h, &keys);
        let revision = h.revision();
        let published = h.publish_checked(revision, 1, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(1)).unwrap();
        assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    }
    assert!(matches!(actor.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
    {
        let mut h = supervisor.host_mut().unwrap();
        let revision = h.revision();
        assert_eq!(h.reconcile(revision, 1).unwrap(), Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    }
    assert!(matches!(actor.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let retried = actor.submit(9000, &proposal).unwrap();
    assert!(matches!(actor.poll(&retried), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(supervisor.host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 1);
    let canonical = FileOversight::read_predictive_consistency(root.store(), &ordinary::profile(), &expected).unwrap();
    assert_eq!(canonical.consistency.evidence.samples(), 1); assert_eq!(canonical.journal.executions, 1);
}

#[test]
fn raw_frames_refuse_even_with_matching_metadata_and_mode_mismatch_cannot_recover() {
    let root = Directory::new(); let (mut h, roles, expected) = create(&root, 100.0, false);
    force(&mut h, 1); let frame = fake(&h); let actor = h.actor_snapshot().unwrap().actor_revision;
    let revision = h.revision();
    assert_eq!(roles.consistency_observer.forecast_action(&mut h, revision, 1, actor, &frame).unwrap(), Err(Error::Binding));
    let revision = h.revision();
    assert_eq!(roles.consistency_observer.forecast_request(&mut h, revision, 9000, actor, &frame).unwrap(), Err(Error::Binding));
    assert_eq!(h.pending_forecast_request().unwrap(), None);
    let revision = h.revision();
    let observed = roles.consistency_observer.forecast_hosted_request(&mut h, revision, 9000, actor).unwrap().unwrap();
    assert_eq!(observed.forecast().null_numerator(), 49152);
    let canonical = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retain evidence").unwrap();
    drop(h);
    for prediction in [raw_config(), raw_config().with_hosted_residual(2).unwrap()] {
        let mut wrong = expected.clone(); wrong.prediction = prediction;
        assert!(matches!(FileOversight::read_predictive_consistency(root.store(), &ordinary::profile(), &wrong),
            Err(JournalError::Contract(Error::Binding))));
        assert!(matches!(FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &wrong),
            Err(JournalError::Contract(Error::Binding))));
        assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), canonical);
        assert_eq!(std::fs::read(&pending).unwrap(), b"retain evidence");
    }
    let (h, _) = FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).unwrap();
    assert_eq!(h.hosted_consistency_layer().unwrap(), Some(1));
    assert!(h.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(h.pending_forecast_request().unwrap(), Some(9000));
}

#[test]
fn clean_recovery_requires_new_role_time_resume_and_new_actual_token_before_second_publication() {
    let root = Directory::new(); let (mut h, roles, expected) = create(&root, 100.0, false);
    force(&mut h, 0); forecast(&mut h, &roles.consistency_observer, 9000);
    h.submit_request(h.revision(), 9000, ordinary::spec(&h, b"first"), ordinary::snapshot()).unwrap();
    let keys = approve(&mut h, &roles.oversight.human, 9000, 1); ordinary::dispatch(&mut h, &keys);
    h.publish_checked(h.revision(), 1, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(1)).unwrap();
    h.reconcile(h.revision(), 1).unwrap(); let evidence = h.action_consistency_snapshot().unwrap().evidence;
    drop(h);
    let (mut h, fresh) = FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).unwrap();
    assert_eq!(h.hosted_consistency_layer().unwrap(), Some(1));
    assert_eq!(h.action_consistency_snapshot().unwrap().evidence, evidence);
    let actor = h.actor_snapshot().unwrap().actor_revision; let revision = h.revision();
    assert_eq!(roles.consistency_observer.forecast_hosted_request(&mut h, revision, 9001, actor), Err(JournalError::Contract(Error::Binding)));
    let revision = h.revision();
    assert!(fresh.consistency_observer.forecast_hosted_request(&mut h, revision, 9001, actor).is_err());
    h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
    let n = h.decoder_inspection().unwrap().numerical;
    h.resume_decoder(h.revision(), n.actor_revision, n.position).unwrap();
    let actor = h.actor_snapshot().unwrap().actor_revision;
    let revision = h.revision();
    assert_eq!(fresh.consistency_observer.forecast_hosted_request(&mut h, revision, 9001, actor).unwrap(), Err(Error::Stale));
    let n = h.decoder_inspection().unwrap().numerical;
    let step = h.advance_decoder_sampled(h.revision(), n.actor_revision, n.position, SampleBudget {
        decoder: numerical::budget(), sampling: SamplingBudget { vocabulary: 2 },
    }).unwrap().unwrap();
    assert!(matches!(step, MonitoredSampledStep::Released(_)));
    assert_eq!(h.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    forecast(&mut h, &fresh.consistency_observer, 9001);
    h.submit_request(h.revision(), 9001, ordinary::spec(&h, b"second"), ordinary::snapshot()).unwrap();
    assert_eq!(h.action_consistency_observation(2).unwrap().prediction().forecast().null_numerator(), 49152);
    let keys = approve(&mut h, &fresh.oversight.human, 9001, 2); ordinary::dispatch(&mut h, &keys);
    assert_eq!(h.publish_checked(h.revision(), 2, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 3 });
    h.reconcile(h.revision(), 2).unwrap();
    assert_eq!(h.action_consistency_snapshot().unwrap().evidence.samples(), 2);
}

#[test]
fn crossed_owned_evidence_keeps_refused_request_and_seals_without_premature_refund() {
    let root = Directory::new(); let (mut h, roles, _) = create(&root, 100.0, false);
    force(&mut h, 0); forecast(&mut h, &roles.consistency_observer, 9000);
    h.submit_request(h.revision(), 9000, ordinary::spec(&h, b"risk first"), ordinary::snapshot()).unwrap();
    let keys = approve(&mut h, &roles.oversight.human, 9000, 1); ordinary::dispatch(&mut h, &keys);
    force(&mut h, 0); forecast(&mut h, &roles.consistency_observer, 9001);
    let raw = ordinary::spec(&h, b"risk second");
    let status = h.submit_request(h.revision(), 9001, raw.clone(), ordinary::snapshot()).unwrap();
    assert!(matches!(status.disposition, FileRequestDisposition::NotAdmitted(_)));
    assert_eq!(h.action_consistency_snapshot().unwrap().evidence.first_crossing(), Some(2));
    let revision = h.revision();
    assert_eq!(h.submit_request(0, 9001, raw, Default::default()).unwrap(), status);
    assert_eq!(h.revision(), revision);
    let sealed = h.publish_checked(h.revision(), 1, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(1)).unwrap();
    assert!(matches!(sealed.basis, PublicationBasis::Rejected(_)));
    assert_eq!(h.inspect().executions, 0); assert_eq!(h.inspect().control.ledger.charged, 16);
    assert_eq!(h.reconcile(h.revision(), 1).unwrap(), Reconciliation::Resolved(sealed.outcome));
    assert_eq!(h.inspect().control.ledger.charged, 0);
    assert_eq!(h.action_consistency_snapshot().unwrap().evidence.samples(), 2);
}

#[test]
fn held_or_stopped_owner_cannot_forecast_and_faulted_storage_exposes_no_candidate_binding() {
    for stopping in [false, true] {
        let root = Directory::new(); let (mut h, roles, expected) = create(&root, 1.5, stopping);
        force(&mut h, 0); assert!(matches!(force(&mut h, 1), MonitoredStep::Held(_)));
        let revision = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
        let result = roles.consistency_observer.forecast_hosted_request(&mut h, revision, 9000, actor);
        assert!(!matches!(result, Ok(Ok(_))));
        assert_eq!(h.pending_forecast_request().unwrap(), None);
        assert_eq!(h.inspect().stop.is_some(), stopping);
        let report = FileOversight::read_predictive_consistency(root.store(), &ordinary::profile(), &expected).unwrap();
        assert_eq!(report.consistency.evidence.samples(), 0); assert_eq!(report.journal.executions, 0);
    }
    let root = Directory::new(); let (mut h, roles, expected) = create(&root, 100.0, false);
    force(&mut h, 0); let before = h.inspect();
    std::fs::write(root.store().join("delivery.pending"), b"keep").unwrap();
    let revision = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    assert!(roles.consistency_observer.forecast_hosted_request(&mut h, revision, 9000, actor).is_err());
    assert_eq!(h.inspect(), before); assert!(h.storage_failure().is_some());
    assert!(matches!(h.hosted_consistency_layer(), Err(JournalError::Unavailable)));
    let report = FileOversight::read_predictive_consistency(root.store(), &ordinary::profile(), &expected).unwrap();
    assert_eq!(report.consistency.pending_attempt, None); assert!(!report.consistency.coverage_lost);
    assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"keep");
}

#[test]
fn invalid_owned_contract_refuses_before_initial_publication_and_legacy_source_mode_stays_explicit() {
    for variant in 0..3 {
        let root = Directory::new(); let mut g = guards(100.0, false);
        let layer = if variant == 1 { 2 } else { 1 };
        if variant == 0 { g.decoder = None; }
        let prediction = if variant == 2 {
            // A valid predictor, but bound to a different actual source stream.
            let p = FileConsistencyParameters { probe_id: 1, probe_generation: 1,
                profile: numerical::model().residual_contract(1).unwrap().profile(), weights: vec![1.0, 0.0],
                bias: 0.0, threshold: 1.5, forecast: ForecastRegistration { domain: 8, generation: 1, policy_generation: 1,
                    event_prefix: b"risk".to_vec(), negative: BinaryForecast::new(1, 2).unwrap(),
                    at_threshold: BinaryForecast::new(1, 2).unwrap(), positive: BinaryForecast::new(1, 2).unwrap() },
                alpha: ErrorBudget::new(1, 4).unwrap(), stream: 99, max_predictions: 8, max_prediction_age_ticks: 10 };
            FileConsistencyConfig::new(p).unwrap()
        } else { raw_config() }.with_hosted_residual(layer).unwrap();
        assert!(FileOversight::create_predictive_guarded(root.store(), ordinary::profile(), &g, None, prediction, None).is_err());
        assert!(!root.store().exists());
    }
    let root = Directory::new();
    let (mut h, roles) = FileOversight::create_predictive_guarded(root.store(), ordinary::profile(),
        &guards(100.0, false), None, raw_config(), None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); force(&mut h, 0);
    assert_eq!(h.hosted_consistency_layer().unwrap(), None);
    let actor = h.actor_snapshot().unwrap().actor_revision; let revision = h.revision();
    assert_eq!(roles.consistency_observer.forecast_hosted_request(&mut h, revision, 9000, actor).unwrap(), Err(Error::Incomplete));
    let frame = fake(&h); let revision = h.revision();
    assert!(roles.consistency_observer.forecast_request(&mut h, revision, 9000, actor, &frame).unwrap().is_ok());
}
