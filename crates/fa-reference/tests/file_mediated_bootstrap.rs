//! First-image topology plus the complete existing numerical/effect profile.
#![cfg(unix)]
#[path = "support/file_mediated.rs"] mod support;
#[path = "support/file_guarded.rs"] mod composed;
#[path = "support/file_credibility.rs"] mod evaluation;
#[path = "support/investigation_decoder.rs"] mod numerical;
#[allow(dead_code)]
#[path = "support/decoder_inputs.rs"] mod data;
use support::ordinary::{self, Directory, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{PolicyUpdate, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::observed::consistency::{FileConsistencyConfig, FileConsistencyParameters};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{FileCredentialRegistration, FileGuardSet};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::{FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement};
use fa_reference::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::decoder_host::HostedStopPolicy;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::identity::IdentityStatus;
use fa_reference::action::consequence::oversight::policy_governance::CampaignDisposition;
use fa_reference::action::consequence::gate::containment::session::policy::Policy;

fn predictor() -> FileConsistencyConfig {
    FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 7, probe_generation: 1, profile: numerical::model().residual_contract(1).unwrap().profile(),
        weights: vec![1.0, 0.0], bias: 0.0, threshold: 1.5,
        forecast: ForecastRegistration { domain: 8, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: BinaryForecast::new(16384, 49152).unwrap(),
            at_threshold: BinaryForecast::new(32768, 32768).unwrap(), positive: BinaryForecast::new(49152, 16384).unwrap() },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 5, max_predictions: 8, max_prediction_age_ticks: 10,
    }).unwrap().with_hosted_residual(1).unwrap()
}
fn numerical_guards() -> FileGuardSet {
    let mut guards = support::guards();
    guards.decoder = Some(FileDecoderConfig::new(numerical::profile(), data::weights(), data::monitor(100.0),
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap());
    guards.decoder_stop = Some(HostedStopPolicy::new(1, 1, 7000).unwrap());
    guards
}
fn force(host: &mut FileOversight, token: u32) {
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, n.position,
        token, numerical::budget()).unwrap().unwrap(), MonitoredStep::Released(_)));
}
fn recapture(host: &mut FileOversight,
    role: &fa_reference::action::consequence::delivery::persistent::observed::mediation::FileMediationObserver,
    generation: u64)
{
    support::update(host, role, generation, Some(support::graph(generation, false)));
    assert!(matches!(support::certify(host, role), CutCheck::Verified(_)));
}
fn message(host: &mut FileOversight, roles: &FileMediatedRoles, p: &FileOversightProfile,
    observation: &EvidenceSnapshot, key: u64, attempt: u64, text: &str) -> EndpointOutcome
{
    let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    roles.consistency_observer.as_ref().unwrap().forecast_hosted_request(host, revision, key, actor).unwrap().unwrap();
    let spec = host.stream_message_spec(text, ElapsedTick(100)).unwrap();
    host.submit_request(host.revision(), key, spec, snapshot()).unwrap();
    let action = host.request_action(key).unwrap().clone();
    let input = observation.inputs_for(&action, &p.committee).unwrap();
    ordinary::review_existing(host, attempt, 100 + attempt, &input);
    evaluation::label(host, roles.evaluator.as_ref().unwrap(), attempt, attempt, GroundTruth::Benign);
    let automatic = host.authorize(host.revision(), attempt, &input, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1000 + attempt, attempt, &input, ElapsedTick(20)).unwrap();
    let revision = host.revision(); let human = roles.oversight.human.approve(host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    let credential = composed::credential(host); let now = host.inspect().control.ledger.elapsed.unwrap();
    let result = host.publish_checked_with_credential(host.revision(), attempt, Some(&input), snapshot(), now, &credential).unwrap();
    assert_eq!(host.reconcile(host.revision(), attempt).unwrap(), Reconciliation::Resolved(result.outcome));
    result.outcome
}

#[test]
fn every_guard_survives_one_recovery_then_new_owned_forecast_and_credentialed_stream_publication() {
    let root = Directory::new(); let mut g = composed::all_guards(); let numeric = numerical_guards();
    g.decoder = numeric.decoder; g.decoder_stop = numeric.decoder_stop;
    let p = composed::profile_for(&g); let inventory = composed::inventory(); let binding = composed::binding();
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), p.clone(), &g,
        Some(FileCredentialRegistration { inventory: &inventory, binding: &binding }),
        support::graph(1, false), Some(predictor()), Some(evaluation::protocol())).unwrap();
    assert_eq!(host.revision(), 10); assert!(!host.clock_ready());
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert!(roles.oversight.identity_observer.is_some() && roles.oversight.policy_governor.is_some());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); force(&mut host, 0);
    assert!(matches!(support::certify(&mut host, &roles.topology_observer), CutCheck::Verified(_)));
    let source_path = root.0.join("evidence.json");
    let evidence = |generation| EvidenceSnapshot::new(EvidenceIdentity { source: 17, generation, scope: p.delivery.scope },
        snapshot(), ordinary::MEMBERS.into_iter().map(|m| (m.to_owned(), b"whole independent source".to_vec())).collect()).unwrap();
    let observation = evidence(1); std::fs::write(&source_path, observation.encode()).unwrap();
    let mut source = FileEvidenceSource::new(source_path.clone(), 17, p.delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    composed::identity::matched(&mut host, roles.oversight.identity_observer.as_ref().unwrap(), 1, 1);
    assert_eq!(message(&mut host, &roles, &p, &observation, 9000, 1, "before restart"),
        EndpointOutcome::Executed { resulting_version: 2 });
    let control = host.inspect().control;
    let update = PolicyUpdate::new(81, control.sequence, control.ledger.epoch,
        Policy::new(2, host.current_policy().unwrap().nodes().to_vec()).unwrap()).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &update).unwrap(); let revision = host.revision();
    let old_key = roles.oversight.policy_governor.as_ref().unwrap().approve(&mut host, revision, &campaign, false).unwrap();
    let expected = FileMediatedRequirements { oversight: composed::requirements(&host, g),
        topology: FileTopologyRequirement { initial: support::graph(1, false),
            current: support::graph(1, false), available: true }, prediction: Some(predictor()), evaluation: Some(evaluation::protocol()) };
    let before = FileOversight::read_mediation(root.store(), &p, &expected).unwrap(); drop(host);

    let (mut host, fresh) = FileOversight::open_mediated_guarded(root.store(), p.clone(), &expected).unwrap();
    assert_eq!(host.revision(), before.journal.revision + 1);
    assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
    assert!(!host.mediation_snapshot().unwrap().available); assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(host.credibility_report().unwrap(), before.credibility.unwrap());
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, before.consistency.unwrap().evidence);
    assert_eq!(host.policy_campaign(81).unwrap().observed_disposition(), CampaignDisposition::Revoked);
    assert!(host.promote_policy_campaign(host.revision(), &old_key).is_err());
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["before restart"]);
    // Topology observation is possible while inference is recovery-paused.
    recapture(&mut host, &fresh.topology_observer, 2);
    assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let observation = evidence(2); std::fs::write(&source_path, observation.encode()).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap(); force(&mut host, 1);
    composed::identity::matched(&mut host, fresh.oversight.identity_observer.as_ref().unwrap(), 2, 2);
    assert_eq!(message(&mut host, &fresh, &p, &observation, 9001, 2, "after fresh recapture"),
        EndpointOutcome::Executed { resulting_version: 3 });
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(),
        vec!["before restart", "after fresh recapture"]);
    assert_eq!(host.credibility_report().unwrap().benign_origins, 2);
    let c = host.inspect().control;
    let next = PolicyUpdate::new(82, c.sequence, c.ledger.epoch,
        Policy::new(2, host.current_policy().unwrap().nodes().to_vec()).unwrap()).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &next).unwrap(); let revision = host.revision();
    fresh.oversight.policy_governor.as_ref().unwrap().approve(&mut host, revision, &campaign, false).unwrap();
    assert_eq!(host.policy_campaign(82).unwrap().observed_disposition(), CampaignDisposition::Approved);
    // Do not promote a new policy into an old predictor's fixed calibration.
    assert!(FileOversight::read_mediation(root.store(), &p, &expected).is_err());
    let current = FileMediatedRequirements { oversight: composed::requirements(&host, expected.oversight.guards),
        topology: FileTopologyRequirement { initial: support::graph(1, false),
            current: support::graph(2, false), available: true }, prediction: Some(predictor()), evaluation: Some(evaluation::protocol()) };
    assert_eq!(FileOversight::read_mediation(root.store(), &p, &current).unwrap().journal, host.inspect());
}

#[test]
fn atomic_startup_refuses_bad_contracts_but_records_uncertified_bypass_graphs_without_granting_effects() {
    let invalid = Directory::new(); let mut wrong = profile(); wrong.delivery.scope.run += 1;
    assert!(FileOversight::create_mediated_guarded(invalid.store(), wrong, &support::guards(), None,
        support::graph(1, false), None, None).is_err()); assert!(!invalid.store().exists());
    let invalid = Directory::new(); let mut protocol = evaluation::protocol(); protocol.recall_floor.denominator = 0;
    assert!(FileOversight::create_mediated_guarded(invalid.store(), profile(), &support::guards(), None,
        support::graph(1, false), None, Some(protocol)).is_err()); assert!(!invalid.store().exists());
    let invalid = Directory::new();
    assert!(FileOversight::create_mediated_guarded(invalid.store(), profile(), &support::guards(), None,
        support::graph(1, false), Some(predictor()), None).is_err()); assert!(!invalid.store().exists());
    let invalid = Directory::new(); let g = composed::all_guards();
    assert!(FileOversight::create_mediated_guarded(invalid.store(), composed::profile_for(&g), &g, None,
        support::graph(1, false), None, None).is_err()); assert!(!invalid.store().exists());
    let root = Directory::new();
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), profile(), &support::guards(), None,
        support::graph(1, true), None, None).unwrap();
    assert_eq!(host.revision(), 2); assert!(roles.consistency_observer.is_none() && roles.evaluator.is_none());
    let original = std::fs::read(root.store().join("delivery.bin")).unwrap();
    assert!(FileOversight::create_mediated_guarded(root.store(), profile(), &support::guards(), None,
        support::graph(1, false), None, None).is_err());
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), original);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let cut = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(roles.topology_observer.certify(&mut host, revision, epoch, &cut, MAX_CHECK_EDGE_VISITS).unwrap(),
        Ok(CutCheck::Bypass(_))));
    assert!(host.propose(host.revision(), 1, ordinary::spec(&host, b"blocked"), snapshot()).is_err());
    recapture(&mut host, &roles.topology_observer, 2);
    let keys = ordinary::ready(&mut host, &roles.oversight.human, 1, b"fresh certified publication");
    ordinary::dispatch(&mut host, &keys);
    let outcome = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(outcome.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome.outcome));
}
