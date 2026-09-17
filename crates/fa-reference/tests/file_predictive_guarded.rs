//! Original prediction, independently held roles and publication across one recovery.
#![cfg(unix)]
#[path = "support/file_consistency.rs"] mod fixture;
#[path = "support/file_oversight.rs"] mod ordinary;
use fixture::*;
use ordinary::Directory;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::consistency::BinaryForecast;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::consistency::FileConsistencyConfig;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::*;
use fa_reference::action::consequence::oversight::credibility::{Assessment, EvaluationProtocol, Fraction, GroundTruth};
use fa_reference::Error;

fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn evaluation() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 1, period: 1, minimum_violation_origins: 1,
        minimum_benign_origins: 1, precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
fn requirements(host: &FileOversight, evaluated: bool) -> FilePredictiveRequirements {
    let state = host.inspect();
    FilePredictiveRequirements { oversight: FileRecoveryRequirements {
        guards: guards(), effective_policy: host.current_policy().unwrap().clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: state.revision, control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch } }, prediction: configuration(),
        evaluation: evaluated.then(evaluation) }
}
fn create(root: &Directory, evaluated: bool) -> (FileOversight, FilePredictiveRoles) {
    FileOversight::create_predictive_guarded(root.store(), ordinary::profile(), &guards(), None,
        configuration(), evaluated.then(evaluation)).unwrap()
}
fn publish(host: &mut FileOversight, roles: &FilePredictiveRoles, id: u64, sequence: u64) {
    forecast(host, &roles.consistency_observer, id, sequence, -1.0);
    let keys = ordinary::ready(host, &roles.oversight.human, id, b"ordinary");
    ordinary::dispatch(host, &keys);
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let result = host.publish_checked(host.revision(), id, Some(&keys.inputs), ordinary::snapshot(), now).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: id + 1 });
    assert_eq!(host.reconcile(host.revision(), id).unwrap(), Reconciliation::Resolved(result.outcome));
}

#[test]
fn one_recovery_returns_new_observer_and_evaluator_without_resetting_evidence_or_labels() {
    let root = Directory::new(); let (mut host, old) = create(&root, true);
    assert_eq!(host.revision(), 3); assert!(!host.clock_ready());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    publish(&mut host, &old, 1, 1);
    let ticket = host.evaluation_ticket(101).unwrap(); let revision = host.revision();
    let label = Assessment { origin: 1, evidence_id: [9; 32], truth: GroundTruth::Benign };
    old.evaluator.as_ref().unwrap().assess(&mut host, revision, &ticket, label).unwrap();
    let expected = requirements(&host, true); let before = host.action_consistency_snapshot().unwrap();
    drop(host);
    let (mut host, roles) = FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).unwrap();
    assert_eq!(host.revision(), before.journal_revision + 1); assert!(!host.clock_ready());
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence, before.evidence);
    assert_eq!(host.evaluation_history(101).unwrap(), &[label]);
    let current = host.evaluation_ticket(101).unwrap(); let revision = host.revision();
    assert!(matches!(old.evaluator.as_ref().unwrap().assess(&mut host, revision, &current, label),
        Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(roles.evaluator.as_ref().unwrap().assess(&mut host, revision, &ticket, label),
        Err(JournalError::Contract(Error::Binding))));
    assert!(!roles.evaluator.as_ref().unwrap().assess(&mut host, revision, &current, label).unwrap());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let frame = frame(&host, 2, -1.0); let actor = host.actor_snapshot().unwrap().actor_revision;
    let revision = host.revision();
    assert!(matches!(old.consistency_observer.forecast_action(&mut host, revision, 2, actor, &frame),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(host.revision(), revision);
    publish(&mut host, &roles, 2, 2);
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 2);
}

#[test]
fn pending_recovery_preserves_unknown_outcome_and_cannot_rearm_the_lifetime_process() {
    let root = Directory::new(); let (mut host, roles) = create(&root, false);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    forecast(&mut host, &roles.consistency_observer, 1, 1, -1.0);
    let expected = requirements(&host, false); let before = host.action_consistency_snapshot().unwrap();
    let passive = FileOversight::read_predictive_consistency(root.store(), &ordinary::profile(), &expected).unwrap();
    assert_eq!(passive.consistency, before); assert!(!passive.consistency.coverage_lost);
    drop(host);
    let (mut host, roles) = FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).unwrap();
    let after = host.action_consistency_snapshot().unwrap();
    assert!(after.coverage_lost); assert_eq!(after.pending_attempt, Some(1));
    assert_eq!(after.evidence, before.evidence);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let frame = frame(&host, 2, -1.0); let actor = host.actor_snapshot().unwrap().actor_revision;
    let revision = host.revision();
    assert!(roles.consistency_observer.forecast_action(&mut host, revision, 1, actor, &frame).unwrap().is_err());
    assert!(host.propose_consistent(host.revision(), 1, ordinary::spec(&host, b"ordinary"), ordinary::snapshot()).unwrap().is_err());
    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn exact_prediction_and_optional_evaluation_are_checked_without_cleanup_or_fencing() {
    let root = Directory::new(); let (mut host, roles) = create(&root, true);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    publish(&mut host, &roles, 1, 1);
    let expected = requirements(&host, true); drop(host);
    let canonical = root.store().join("delivery.bin"); let bytes = std::fs::read(&canonical).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"keep evidence").unwrap();
    for variant in 0..12 {
        let mut wrong = expected.clone(); let mut p = parameters();
        match variant {
            0 => p.weights[0] = 2.0, 1 => p.bias = -0.0, 2 => p.threshold = 1.0,
            3 => p.forecast.event_prefix.push(b'!'),
            4 => p.forecast.negative = BinaryForecast::new(16000, 49152).unwrap(),
            5 => p.max_predictions += 1, 6 => p.max_prediction_age_ticks += 1,
            7 => p.stream += 1, 8 => wrong.evaluation = None,
            9 => wrong.evaluation.as_mut().unwrap().period += 1,
            10 => wrong.oversight.minimum.journal_revision += 1,
            _ => p.probe_generation += 1,
        }
        wrong.prediction = FileConsistencyConfig::new(p).unwrap();
        assert!(FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &wrong).is_err(), "{variant}");
        assert_eq!(std::fs::read(&canonical).unwrap(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"keep evidence");
    }
    assert!(FileOversight::open_evaluated_guarded(root.store(), ordinary::profile(), &expected.oversight, &evaluation()).is_err());
    assert!(FileOversight::open_guarded(root.store(), ordinary::profile(), &expected.oversight).is_err());
    assert_eq!(std::fs::read(&canonical).unwrap(), bytes); assert!(pending.exists());
    let (host, _) = FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).unwrap();
    assert_eq!(host.inspect().executions, 1); assert!(!pending.exists());
}

#[test]
fn passive_inspection_survives_faulted_owner_but_rejects_invalid_suffixes() {
    let root = Directory::new(); let (mut host, _) = create(&root, false);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let expected = requirements(&host, false); let before = host.inspect();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"keep").unwrap();
    assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err());
    assert!(host.storage_failure().is_some());
    let canonical = root.store().join("delivery.bin"); let bytes = std::fs::read(&canonical).unwrap();
    let read = FileOversight::read_predictive_consistency(root.store(), &ordinary::profile(), &expected).unwrap();
    assert_eq!(read.journal, before); assert!(read.credibility.is_none());
    assert_eq!(read.consistency.evidence.samples(), 0);
    let mut corrupt = bytes.clone(); corrupt.push(0); std::fs::write(&canonical, &corrupt).unwrap();
    assert!(FileOversight::read_predictive_consistency(root.store(), &ordinary::profile(), &expected).is_err());
    assert_eq!(std::fs::read(&canonical).unwrap(), corrupt); assert_eq!(std::fs::read(&pending).unwrap(), b"keep");
    std::fs::write(&canonical, &bytes).unwrap();
    assert_eq!(FileOversight::read_predictive_consistency(root.store(), &ordinary::profile(), &expected).unwrap(), read);
    assert_eq!(host.inspect(), before); assert!(host.storage_failure().is_some());
}

#[test]
fn invalid_bootstrap_never_creates_storage_and_prediction_is_not_optional_on_recovery() {
    for variant in 0..3 {
        let root = Directory::new(); let mut p = parameters();
        match variant { 0 => p.profile.tenant += 1, 1 => p.profile.model_generation += 1,
            _ => p.forecast.policy_generation += 1 }
        assert!(FileOversight::create_predictive_guarded(root.store(), ordinary::profile(), &guards(), None,
            FileConsistencyConfig::new(p).unwrap(), None).is_err());
        assert!(!root.store().exists());
    }
    let root = Directory::new(); let (host, roles) = create(&root, false);
    assert_eq!(host.revision(), 2); assert!(roles.evaluator.is_none());
    let mut expected = requirements(&host, false); expected.evaluation = Some(evaluation()); drop(host);
    assert!(FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).is_err());
    expected.evaluation = None;
    let (host, _) = FileOversight::open_predictive_guarded(root.store(), ordinary::profile(), &expected).unwrap();
    assert!(!host.clock_ready());
}
