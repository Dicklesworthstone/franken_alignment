//! Canonical topology evidence and the composed numerical/predictive effect path.
#![cfg(unix)]
#[path = "support/file_mediated.rs"] mod support;
#[path = "support/investigation_decoder.rs"] mod numerical;
#[allow(dead_code)]
#[path = "support/decoder_inputs.rs"] mod data;
use support::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{FileConsistencyConfig, FileConsistencyParameters};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::{FileMediatedRequirements, FileMediatedRoles};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::FilePredictiveRequirements;
use fa_reference::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::Error;

#[test]
fn canonical_history_distinguishes_noop_withdrawal_missing_update_and_faulted_owner() {
    let root = Directory::new();
    let (mut h, roles) = FileOversight::create_mediated_guarded(root.store(), profile(), &guards(), None,
        graph(1, false), None, None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); certify(&mut h, &roles.topology_observer);
    let keys = ordinary::ready(&mut h, &roles.oversight.human, 1, b"visible"); ordinary::dispatch(&mut h, &keys);
    let published = h.publish_checked(h.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let original_cut = h.delivery_mediation(1).unwrap().cloned();
    update(&mut h, &roles.topology_observer, 10, None); let lost_at = h.revision();
    update(&mut h, &roles.topology_observer, 11, None); let noop_at = h.revision();
    let expected = requirements(&h, None);
    let bytes = std::fs::read(root.store().join("delivery.bin")).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"unacknowledged topology").unwrap();
    let request = replacement(&h, 12, Some(graph(2, false))); let rev = h.revision();
    assert!(roles.topology_observer.update(&mut h, rev, &request).is_err());
    assert_eq!(h.mediation_snapshot(), Err(JournalError::Unavailable));
    let disk = FileOversight::read_mediation(root.store(), &profile(), &expected).unwrap();
    assert_eq!(disk.journal.revision, noop_at); assert_eq!(disk.journal.executions, 1);
    assert_eq!(disk.journal.control.ledger.charged, 16); assert!(!disk.topology.available);
    assert!(disk.topology.accepted.is_none()); assert_eq!(disk.updates.len(), 2);
    assert_eq!(disk.updates[0].journal_revision, lost_at); assert!(disk.updates[0].change.is_some());
    assert_eq!(disk.updates[1].journal_revision, noop_at); assert_eq!(disk.updates[1].change, None);
    assert!(!disk.updates.iter().any(|entry| entry.request.operation == 12));
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"unacknowledged topology");
    drop(h);
    let (mut h, fresh) = FileOversight::open_mediated_guarded(root.store(), profile(), &expected).unwrap();
    assert_eq!(h.delivery_mediation(1).unwrap(), original_cut.as_ref());
    h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(h.reconcile(h.revision(), 1).unwrap(), Reconciliation::Resolved(published.outcome));
    assert_eq!(h.inspect().control.ledger.charged, 16);
    update(&mut h, &fresh.topology_observer, 12, Some(graph(2, false))); certify(&mut h, &fresh.topology_observer);
    let keys = ordinary::ready(&mut h, &fresh.oversight.human, 2, b"new graph"); ordinary::dispatch(&mut h, &keys);
    assert_eq!(h.publish_checked(h.revision(), 2, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 3 });
}

fn prediction() -> FileConsistencyConfig {
    FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 7, probe_generation: 1, profile: numerical::model().residual_contract(1).unwrap().profile(),
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 1.5,
        forecast: ForecastRegistration { domain: 8, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: BinaryForecast::new(16384, 49152).unwrap(),
            at_threshold: BinaryForecast::new(32768, 32768).unwrap(), positive: BinaryForecast::new(49152, 16384).unwrap() },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 5, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap()
}
fn complete_request(h: &mut FileOversight, roles: &FileMediatedRoles, request: u64, attempt: u64, text: &[u8]) {
    let actor = h.actor_snapshot().unwrap().actor_revision; let rev = h.revision();
    roles.consistency_observer.as_ref().unwrap().forecast_hosted_request(h, rev, request, actor).unwrap().unwrap();
    h.submit_request(h.revision(), request, ordinary::spec(h, text), snapshot()).unwrap();
    let action = h.request_action(request).unwrap().clone(); let inputs = ordinary::inputs(&action, b"complete input");
    ordinary::review_existing(h, attempt, 100 + attempt, &inputs);
    let automatic = h.authorize(h.revision(), attempt, &inputs, snapshot()).unwrap();
    let now = h.inspect().control.ledger.elapsed.unwrap();
    let request = h.request_human_approval(h.revision(), attempt + 1000, attempt, &inputs, ElapsedTick(now.0 + 30)).unwrap();
    let rev = h.revision(); let human = roles.oversight.human.approve(h, rev, &request).unwrap();
    h.dispatch(h.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let result = h.publish_checked(h.revision(), attempt, Some(&inputs), snapshot(), now).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: attempt + 1 });
    assert_eq!(h.reconcile(h.revision(), attempt).unwrap(), Reconciliation::Resolved(result.outcome));
}

#[test]
fn owned_predictions_independent_labels_and_mediation_survive_one_composed_recovery() {
    let root = Directory::new(); let mut g = guards();
    g.decoder = Some(FileDecoderConfig::new(numerical::profile(), data::weights(), data::monitor(100.0),
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap());
    let prediction = prediction(); let evaluation = protocol();
    let (mut h, roles) = FileOversight::create_mediated_guarded(root.store(), profile(), &g, None,
        graph(1, false), Some(prediction.clone()), Some(evaluation.clone())).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); certify(&mut h, &roles.topology_observer);
    let n = h.decoder_inspection().unwrap().numerical;
    h.advance_decoder_forced(h.revision(), n.actor_revision, n.position, 0, numerical::budget()).unwrap().unwrap();
    complete_request(&mut h, &roles, 9000, 1, b"first");
    let ticket = h.evaluation_ticket(101).unwrap(); let rev = h.revision();
    roles.evaluator.as_ref().unwrap().assess(&mut h, rev, &ticket, Assessment {
        origin: 1, evidence_id: [9; 32], truth: GroundTruth::Benign }).unwrap();
    let mut expected: FileMediatedRequirements = requirements(&h, Some(evaluation));
    expected.oversight.guards = g; expected.prediction = Some(prediction.clone());
    let disk = FileOversight::read_mediation(root.store(), &profile(), &expected).unwrap();
    assert_eq!(disk.consistency.as_ref().unwrap().evidence.samples(), 1);
    assert_eq!(disk.credibility.as_ref().unwrap().pending_cases, 0);
    let mut bad = expected.clone(); bad.prediction = None;
    assert!(FileOversight::read_mediation(root.store(), &profile(), &bad).is_err());
    let mut bad = expected.clone(); bad.evaluation = None;
    assert!(FileOversight::read_mediation(root.store(), &profile(), &bad).is_err());
    let mut bad = expected.clone(); bad.oversight.guards.decoder = None;
    assert!(FileOversight::read_mediation(root.store(), &profile(), &bad).is_err());
    let revision = h.revision(); drop(h);
    let old_profile = FilePredictiveRequirements { oversight: expected.oversight.clone(), prediction,
        evaluation: expected.evaluation.clone() };
    assert!(matches!(FileOversight::open_predictive_guarded(root.store(), profile(), &old_profile), Err(JournalError::Contract(Error::Binding))));
    let (mut h, fresh) = FileOversight::open_mediated_guarded(root.store(), profile(), &expected).unwrap();
    assert_eq!(h.revision(), revision + 1); assert!(!h.clock_ready());
    assert_eq!(h.action_consistency_snapshot().unwrap().evidence, disk.consistency.unwrap().evidence);
    assert_eq!(h.credibility_report().unwrap(), disk.credibility.unwrap());
    let old_request = replacement(&h, 10, Some(graph(2, false))); let rev = h.revision();
    assert!(roles.topology_observer.update(&mut h, rev, &old_request).is_err());
    update(&mut h, &fresh.topology_observer, 10, Some(graph(2, false))); certify(&mut h, &fresh.topology_observer);
    let actor = h.actor_snapshot().unwrap().actor_revision; let rev = h.revision();
    assert!(fresh.consistency_observer.as_ref().unwrap().forecast_hosted_request(&mut h, rev, 9001, actor).is_err());
    h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
    let n = h.decoder_inspection().unwrap().numerical;
    h.resume_decoder(h.revision(), n.actor_revision, n.position).unwrap();
    let rev = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    assert_eq!(fresh.consistency_observer.as_ref().unwrap().forecast_hosted_request(&mut h, rev, 9001, actor).unwrap(), Err(Error::Stale));
    let n = h.decoder_inspection().unwrap().numerical;
    assert!(matches!(h.advance_decoder_sampled(h.revision(), n.actor_revision, n.position, SampleBudget {
        decoder: numerical::budget(), sampling: SamplingBudget { vocabulary: 2 },
    }).unwrap().unwrap(), MonitoredSampledStep::Released(_)));
    complete_request(&mut h, &fresh, 9001, 2, b"second");
    assert_eq!(h.action_consistency_snapshot().unwrap().evidence.samples(), 2);
    expected.topology.current = graph(2, false);
    let observed = FileOversight::read_mediation(root.store(), &profile(), &expected).unwrap();
    assert_eq!(observed.journal.executions, 2); assert!(observed.topology.accepted.is_some());
    assert_eq!(observed.updates.len(), 1); assert_eq!(observed.updates[0].request.operation, 10);
}
