//! Original native incident -> durable cleanup -> endpoint reconciliation.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_consistency.rs"] mod prediction;
#[path = "support/file_guarded.rs"] mod composed;
use ordinary::{Directory, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, PolicyUpdate, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyConfig;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::*;
use fa_reference::action::consequence::gate::containment::session::policy::Policy;
use fa_reference::action::consequence::oversight::consistency::{ConsistencyStopCause, ConsistencyStopPolicy};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::policy_governance::CampaignDisposition;
use fa_reference::Error;

fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn policy() -> ConsistencyStopPolicy { ConsistencyStopPolicy::new(11, 12, 7000).unwrap() }
fn config(stopping: bool) -> FileConsistencyConfig {
    let c = prediction::configuration(); if stopping { c.with_terminal_stop(policy()).unwrap() } else { c }
}
fn expected(h: &FileOversight, guards: FileGuardSet, prediction: FileConsistencyConfig) -> FilePredictiveRequirements {
    FilePredictiveRequirements { oversight: composed::requirements(h, guards), prediction, evaluation: None }
}
fn start(root: &Directory, stopping: bool) -> (FileOversight, FilePredictiveRoles) {
    let (mut h, roles) = FileOversight::create_predictive_guarded(root.store(), profile(), &guards(), None,
        config(stopping), None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); (h, roles)
}
fn forecast_request(h: &mut FileOversight, roles: &FilePredictiveRoles, key: u64, sequence: u64) {
    let frame = prediction::frame(h, sequence, -1.0); let revision = h.revision();
    let actor = h.actor_snapshot().unwrap().actor_revision;
    roles.consistency_observer.forecast_request(h, revision, key, actor, &frame).unwrap().unwrap();
}

#[test]
fn native_crossing_stops_and_cleans_up_but_the_legacy_mode_remains_hold_only() {
    for stopping in [false, true] {
        let root = Directory::new(); let (mut h, roles) = start(&root, stopping);
        prediction::forecast(&mut h, &roles.consistency_observer, 1, 1, -1.0);
        let first = ordinary::ready(&mut h, &roles.oversight.human, 1, b"risk first");
        ordinary::dispatch(&mut h, &first);
        forecast_request(&mut h, &roles, 9000, 2);
        let proposal = ordinary::spec(&h, b"risk second");
        let status = h.submit_request(h.revision(), 9000, proposal.clone(), snapshot()).unwrap();
        assert_eq!(h.action_consistency_snapshot().unwrap().evidence.first_crossing(), Some(2));
        assert_eq!(h.inspect().control.ledger.charged, 16);
        let before = h.inspect();
        assert_eq!(h.submit_request(0, 9000, proposal, snapshot()).unwrap(), status);
        assert_eq!(h.inspect(), before); assert_eq!(h.action_consistency_snapshot().unwrap().evidence.samples(), 2);
        if stopping {
            assert!(matches!(status.disposition, FileRequestDisposition::NotAdmitted(_)));
            let incident = h.consistency_stop_incident().unwrap().unwrap().clone();
            assert_eq!(incident.cause, ConsistencyStopCause::ThresholdCrossed { first_sample: 2 });
            assert_eq!(incident.observed_samples, 2); assert_eq!(incident.policy, policy());
            assert_eq!(incident.receipt.as_ref().unwrap().request().operation, 7000);
            assert!(h.inspect().control.suspended); assert!(h.inspect().stop.is_some());
            assert_eq!(h.reconcile(h.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
            let sweep = h.progress_stop(h.revision(), ElapsedTick(2)).unwrap(); assert!(sweep.progress.drained());
            assert_eq!(h.inspect().control.ledger.charged, 0); assert_eq!(h.inspect().executions, 0);
            let req = expected(&h, guards(), config(true));
            let disk = FileOversight::read_predictive_stop(root.store(), &profile(), &req).unwrap();
            assert_eq!(disk.incident, Some(incident)); assert_eq!(disk.history.journal, h.inspect());
        } else {
            assert!(matches!(status.disposition, FileRequestDisposition::Admitted { .. }));
            assert!(h.consistency_stop_policy().unwrap().is_none()); assert!(h.inspect().stop.is_none());
            assert!(h.consistency_stop_incident().unwrap().is_none());
            let denied = h.publish_checked(h.revision(), 1, Some(&first.inputs), snapshot(), ElapsedTick(1)).unwrap();
            assert!(!matches!(denied.outcome, EndpointOutcome::Executed { .. }));
            assert_eq!(h.inspect().control.ledger.charged, 16);
            h.reconcile(h.revision(), 1).unwrap(); assert_eq!(h.inspect().control.ledger.charged, 0);
        }
    }
}

#[test]
fn missing_capture_withdraws_human_and_campaign_keys_without_refunding_executed_or_unknown_effects() {
    let root = Directory::new(); let mut g = guards(); g.campaigns = composed::guards().campaigns;
    let (mut h, roles) = FileOversight::create_predictive_guarded(root.store(), profile(), &g, None, config(true), None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    prediction::forecast(&mut h, &roles.consistency_observer, 1, 1, -1.0);
    let executed = ordinary::ready(&mut h, &roles.oversight.human, 1, b"one"); ordinary::dispatch(&mut h, &executed);
    let outcome = h.publish_checked(h.revision(), 1, Some(&executed.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(outcome.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    prediction::forecast(&mut h, &roles.consistency_observer, 2, 2, -1.0);
    let unresolved = ordinary::ready(&mut h, &roles.oversight.human, 2, b"two"); ordinary::dispatch(&mut h, &unresolved);
    prediction::forecast(&mut h, &roles.consistency_observer, 3, 3, -1.0);
    let unsent = ordinary::ready(&mut h, &roles.oversight.human, 3, b"three");
    let c = h.inspect().control;
    let update = PolicyUpdate::new(81, c.sequence, c.ledger.epoch,
        Policy::new(2, h.current_policy().unwrap().nodes().to_vec()).unwrap()).unwrap();
    let campaign = h.request_policy_campaign(h.revision(), &update).unwrap(); let r = h.revision();
    let campaign_key = roles.oversight.policy_governor.as_ref().unwrap().approve(&mut h, r, &campaign, false).unwrap();
    let r = h.revision(); roles.consistency_observer.unavailable(&mut h, r).unwrap();
    assert_eq!(h.consistency_stop_incident().unwrap().unwrap().cause, ConsistencyStopCause::CoverageLost);
    assert_eq!(h.inspect().control.ledger.charged, 32); assert_eq!(h.inspect().control.ledger.reserved, 0);
    assert_eq!(h.inspect().control.ledger.stages[&3], ActionState::Cancelled);
    assert_eq!(h.human_status(1003).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(h.policy_campaign(81).unwrap().observed_disposition(), CampaignDisposition::Revoked);
    assert!(h.promote_policy_campaign(h.revision(), &campaign_key).is_err());
    assert!(h.dispatch(h.revision(), &unsent.automatic, &unsent.human, &unsent.action, &unsent.inputs, snapshot()).is_err());
    let report = h.progress_stop(h.revision(), ElapsedTick(2)).unwrap(); assert!(report.progress.drained());
    assert_eq!(h.inspect().control.ledger.charged, 16); assert_eq!(h.inspect().executions, 1);
    let req = expected(&h, g, config(true)); let incident = h.consistency_stop_incident().unwrap().cloned(); drop(h);
    let (mut h, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &req).unwrap();
    assert_eq!(h.consistency_stop_incident().unwrap().cloned(), incident);
    assert_eq!(h.inspect().control.ledger.charged, 16); assert!(!h.clock_ready());
    h.observe_time(h.revision(), ElapsedTick(3)).unwrap();
    assert!(h.propose(h.revision(), 4, ordinary::spec(&h, b"blocked"), snapshot()).is_err());
}

#[test]
fn clean_recovery_allows_fresh_review_but_an_unanswered_forecast_stops_on_the_same_fence() {
    for pending in [false, true] {
        let root = Directory::new(); let (mut h, roles) = start(&root, true);
        prediction::forecast(&mut h, &roles.consistency_observer, 1, 1, -1.0);
        let first = ordinary::ready(&mut h, &roles.oversight.human, 1, b"one"); ordinary::dispatch(&mut h, &first);
        let outcome = h.publish_checked(h.revision(), 1, Some(&first.inputs), snapshot(), ElapsedTick(1)).unwrap();
        h.reconcile(h.revision(), 1).unwrap(); assert_eq!(outcome.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        if pending { forecast_request(&mut h, &roles, 9000, 2); }
        let req = expected(&h, guards(), config(true)); let revision = h.revision(); drop(h);
        let (mut h, fresh) = FileOversight::open_predictive_guarded(root.store(), profile(), &req).unwrap();
        assert_eq!(h.revision(), revision + 1); assert!(!h.clock_ready());
        assert_eq!(h.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        let r = h.revision(); assert!(roles.consistency_observer.unavailable(&mut h, r).is_err());
        h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
        if pending {
            let incident = h.consistency_stop_incident().unwrap().unwrap();
            assert_eq!(incident.cause, ConsistencyStopCause::CoverageLost); assert_eq!(incident.pending_attempt, Some(2));
            assert_eq!(incident.observed_samples, 1); assert!(h.inspect().stop.is_some());
            assert_eq!(h.pending_forecast_request().unwrap(), Some(9000));
        } else {
            assert!(h.inspect().stop.is_none());
            prediction::forecast(&mut h, &fresh.consistency_observer, 2, 2, -1.0);
            let second = ordinary::ready(&mut h, &fresh.oversight.human, 2, b"two"); ordinary::dispatch(&mut h, &second);
            assert_eq!(h.publish_checked(h.revision(), 2, Some(&second.inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
                EndpointOutcome::Executed { resulting_version: 3 });
        }
        assert_eq!(h.inspect().control.ledger.charged, if pending { 16 } else { 32 });
    }
}

#[test]
fn stale_frames_do_not_trigger_but_actual_prediction_exhaustion_durably_stops() {
    let root = Directory::new(); let mut parameters = prediction::parameters(); parameters.max_predictions = 1;
    let raw = FileConsistencyConfig::new(parameters).unwrap();
    let cfg = raw.clone().with_terminal_stop(policy()).unwrap();
    let (mut h, roles) = FileOversight::create_predictive_guarded(root.store(), profile(), &guards(), None, cfg.clone(), None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    prediction::forecast(&mut h, &roles.consistency_observer, 1, 1, -1.0);
    h.propose(h.revision(), 1, ordinary::spec(&h, b"one"), snapshot()).unwrap();
    let frame = prediction::frame(&h, 1, -1.0); let r = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    assert_eq!(roles.consistency_observer.forecast_action(&mut h, r, 2, actor, &frame).unwrap(), Err(Error::Stale));
    assert!(h.inspect().stop.is_none()); assert!(h.consistency_stop_incident().unwrap().is_none());
    let frame = prediction::frame(&h, 2, -1.0); let r = h.revision();
    assert_eq!(roles.consistency_observer.forecast_action(&mut h, r, 2, actor, &frame).unwrap(), Err(Error::Limit));
    let incident = h.consistency_stop_incident().unwrap().unwrap();
    assert_eq!(incident.cause, ConsistencyStopCause::CoverageLost); assert_eq!(incident.prediction_jobs, 1);
    assert!(incident.receipt.is_some()); assert_eq!(h.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    let req = expected(&h, guards(), cfg); let before = h.inspect(); let path = root.store(); drop(h);
    std::fs::write(path.join("delivery.pending"), b"preserve wrong configuration evidence").unwrap();
    let canonical = std::fs::read(path.join("delivery.bin")).unwrap();
    let candidates = [raw.clone(),
        raw.clone().with_terminal_stop(ConsistencyStopPolicy::new(12, 12, 7000).unwrap()).unwrap(),
        raw.clone().with_terminal_stop(ConsistencyStopPolicy::new(11, 13, 7000).unwrap()).unwrap(),
        raw.with_terminal_stop(ConsistencyStopPolicy::new(11, 12, 7001).unwrap()).unwrap()];
    for candidate in candidates {
        let mut wrong = req.clone(); wrong.prediction = candidate;
        assert!(matches!(FileOversight::open_predictive_guarded(&path, profile(), &wrong),
            Err(JournalError::Contract(Error::Binding))));
        assert_eq!(std::fs::read(path.join("delivery.bin")).unwrap(), canonical);
        assert_eq!(std::fs::read(path.join("delivery.pending")).unwrap(), b"preserve wrong configuration evidence");
    }
    assert_eq!(FileOversight::read_predictive_stop(&path, &profile(), &req).unwrap().history.journal, before);
    FileOversight::open_predictive_guarded(&path, profile(), &req).unwrap();
}

#[test]
fn loss_on_an_already_stopped_owner_preserves_the_original_stop_operation() {
    let root = Directory::new(); let (mut h, roles) = start(&root, true); let c = h.inspect().control;
    let receipt = h.request_stop(h.revision(), StopRequest { operation: 77,
        expected_control_sequence: c.sequence, expected_authority_epoch: c.ledger.epoch }).unwrap();
    let r = h.revision(); roles.consistency_observer.unavailable(&mut h, r).unwrap();
    let incident = h.consistency_stop_incident().unwrap().unwrap();
    assert_eq!(incident.policy.operation(), 7000); assert_eq!(incident.receipt, Some(receipt.clone()));
    assert_eq!(h.inspect().stop, Some(receipt)); assert!(incident.last_stop_error.is_none());
    let control = h.inspect().control; let r = h.revision();
    roles.consistency_observer.unavailable(&mut h, r).unwrap(); assert_eq!(h.inspect().control, control);
}

#[path = "support/investigation_decoder.rs"] mod numerical;
#[allow(dead_code)]
#[path = "support/decoder_inputs.rs"] mod data;

#[test]
fn actual_owned_inference_and_identity_allow_publication_but_loss_stops_numerical_resume() {
    use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
    use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
    use fa_reference::action::consequence::oversight::identity::IdentityStatus;
    let root = Directory::new(); let mut g = guards(); g.identity = composed::guards().identity;
    g.decoder = Some(FileDecoderConfig::new(numerical::profile(), data::weights(), data::monitor(100.0),
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap());
    let mut parameters = prediction::parameters();
    parameters.profile = numerical::model().residual_contract(1).unwrap().profile();
    parameters.weights = vec![1.0, 0.0]; parameters.threshold = 1.5; parameters.stream = 5;
    let cfg = FileConsistencyConfig::new(parameters).unwrap().with_terminal_stop(policy()).unwrap().with_hosted_residual(1).unwrap();
    let (mut h, roles) = FileOversight::create_predictive_guarded(root.store(), profile(), &g, None, cfg.clone(), None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); let n = h.decoder_inspection().unwrap().numerical;
    h.advance_decoder_forced(h.revision(), n.actor_revision, n.position, 0, numerical::budget()).unwrap().unwrap();
    // The identity fixture supplies registered measurements; the forecast itself
    // comes from the actual owned numerical residual, not those fixture values.
    composed::identity::matched(&mut h, roles.oversight.identity_observer.as_ref().unwrap(), 1, 1);
    let r = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    roles.consistency_observer.forecast_hosted_request(&mut h, r, 9000, actor).unwrap().unwrap();
    h.submit_request(h.revision(), 9000, ordinary::spec(&h, b"ordinary"), snapshot()).unwrap();
    let action = h.request_action(9000).unwrap().clone(); let input = ordinary::inputs(&action, b"complete actual inputs");
    ordinary::review_existing(&mut h, 1, 101, &input);
    let automatic = h.authorize(h.revision(), 1, &input, snapshot()).unwrap();
    let request = h.request_human_approval(h.revision(), 1001, 1, &input, ElapsedTick(20)).unwrap(); let r = h.revision();
    let human = roles.oversight.human.approve(&mut h, r, &request).unwrap();
    h.dispatch(h.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(h.publish_checked(h.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    let n = h.decoder_inspection().unwrap().numerical; let r = h.revision();
    roles.consistency_observer.unavailable(&mut h, r).unwrap();
    assert!(h.decoder_inspection().unwrap().paused); assert_eq!(h.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(h.advance_decoder_forced(h.revision(), n.actor_revision, n.position, 1, numerical::budget()).is_err());
    assert_eq!(h.decoder_inspection().unwrap().numerical, n);
    assert!(h.progress_stop(h.revision(), ElapsedTick(2)).unwrap().progress.drained());
    assert_eq!(h.inspect().control.ledger.charged, 16);
    let req = expected(&h, g, cfg); let incident = h.consistency_stop_incident().unwrap().cloned(); drop(h);
    let (mut h, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &req).unwrap();
    h.observe_time(h.revision(), ElapsedTick(3)).unwrap();
    assert_eq!(h.consistency_stop_incident().unwrap().cloned(), incident); assert!(h.decoder_inspection().unwrap().paused);
    assert!(h.resume_decoder(h.revision(), n.actor_revision, n.position).is_err());
    assert_eq!(h.inspect().executions, 1); assert_eq!(h.inspect().control.ledger.charged, 16);
}
