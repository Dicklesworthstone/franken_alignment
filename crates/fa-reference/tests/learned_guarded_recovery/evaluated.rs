//! Complete the original reset without shedding evaluator or oversight custody.
//! Every fixture executes the public numerical and file-backed control APIs.
use super::*;
use fa_reference::action::consequence::activation::tensor::kv::{
    model::MAX_MODEL_KV_VALUES, decoder::monitoring::restart::KvRestartBudget,
};
use fa_reference::action::consequence::delivery::persistent::observed::{
    containment::FileResetRequest,
    decoder::learned::{FileLearnedRecoveryStatus,
        checkpoint::{FileLearnedCheckpoint, FileLearnedResetIntent}},
    guarded::evaluation::{FileEvaluatedOversightRoles, learned::FileEvaluatedLearnedRecovery},
};
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::action::consequence::oversight::credibility::{
    Assessment, EvaluationProtocol, Fraction, GroundTruth,
};

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 31, stratum: 2, period: 3,
        minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 },
        false_stop_budget: 10 }
}
fn evaluated(root: &Directory, config: &FileLearnedConfig, guards: &FileGuardSet)
    -> (FileOversight, FileEvaluatedOversightRoles)
{
    let (mut host, roles) = FileOversight::create_evaluated_guarded(
        root.store(), profile(), guards, None, protocol()).unwrap();
    host.enable_learned_generation(host.revision(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles)
}
fn capture(host: &mut FileOversight) -> FileLearnedCheckpoint {
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap()
}
fn begin_reset(host: &mut FileOversight, saved: &FileLearnedCheckpoint) -> FileLearnedResetIntent {
    let c = host.inspect().control;
    let intent = FileLearnedResetIntent::for_recovery(saved.id(), FileResetRequest {
        operation: 900, expected_control_sequence: c.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: c.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![host.inspect().target],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: source(0).policy.allowance() }).unwrap();
    host.begin_learned_reset(host.revision(), saved, intent.control().clone(), intent.budget()).unwrap();
    assert_eq!(&host.pending_learned_reset().unwrap().unwrap().intent, &intent);
    intent
}
fn drive(run: &mut FileEvaluatedLearnedRecovery, quantum: usize) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, quantum)?;
        assert!(after.replayed_events - before.replayed_events <= quantum);
    }
    Ok(())
}
fn assessment(truth: GroundTruth) -> Assessment {
    Assessment { origin: 201, evidence_id: [19; 32], truth }
}

#[test]
fn cooperative_recovery_keeps_all_declared_roles_and_original_numerical_pause() {
    for mask in 0..4 {
        let root = Directory::new(); let c = config(0); let mut g = guards();
        if mask & 1 == 0 { g.identity = None; }
        if mask & 2 == 0 { g.campaigns = None; }
        let (mut host, _) = evaluated(&root, &c, &g); step(&mut host).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let report = host.credibility_report().unwrap();
        let expected = requirements(&host, g); let bytes = root.bytes(); drop(host);
        let mut run = FileOversight::begin_open_evaluated_guarded_with_learned_generation(
            root.store(), profile(), &expected, &protocol(), &c).unwrap();
        assert_eq!(run.progress().replayed_events, 0);
        assert_eq!(run.advance(1, 1).err(), Some(Error::Stale.into()));
        assert_eq!(run.advance(0, 0).err(), Some(Error::InvalidInput.into()));
        assert_eq!(FileOversight::begin_open_with_learned_generation(
            root.store(), profile(), &c).err(), Some(JournalError::Busy));
        drive(&mut run, 1).unwrap(); assert_eq!(root.bytes(), bytes);
        let (host, roles) = run.finish().unwrap();
        assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
        assert_eq!(roles.oversight.identity_observer.is_some(), mask & 1 != 0);
        assert_eq!(roles.oversight.policy_governor.is_some(), mask & 2 != 0);
        assert_eq!(host.credibility_report().unwrap(), report);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready()); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn anchored_reset_retains_labels_and_returns_fresh_evaluator_identity_and_both_keys() {
    let root = Directory::new(); let c = config(0); let g = guards();
    let (mut host, old) = evaluated(&root, &c, &g);
    step(&mut host).unwrap(); let saved = capture(&mut host); step(&mut host).unwrap();
    fresh_identity(&mut host, &old.oversight, 1, 1);
    let (old_action, old_input, old_key, old_request) = prepared(&mut host, 1, 101);
    let revision = host.revision();
    let old_human = old.oversight.human.approve(&mut host, revision, &old_request).unwrap();
    let old_ticket = host.evaluation_ticket(101).unwrap(); let revision = host.revision();
    old.evaluator.assess(&mut host, revision, &old_ticket, assessment(GroundTruth::Censored)).unwrap();
    let report = host.credibility_report().unwrap(); assert_eq!(report.censored_cases, 1);
    let intent = begin_reset(&mut host, &saved);
    let mut expected = requirements(&host, g); let cut = host.revision();
    let mut evaluation = protocol(); let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes(); drop(host);
    let mut run = FileOversight::begin_open_evaluated_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &evaluation, &c, &anchor).unwrap();
    // The recovery owns exact copies, not mutable references to caller policy.
    expected.guards.identity = None; expected.minimum.journal_revision = u64::MAX;
    evaluation.false_stop_budget = 0;
    assert_ne!(evaluation, protocol());
    drive(&mut run, 2).unwrap(); assert_eq!(root.bytes(), bytes);
    let (mut host, fresh) = run.finish_pending_reset(&intent).unwrap();
    assert_eq!(host.revision(), cut + 2); assert_eq!(anchor.revision(), cut);
    assert_eq!(host.history_anchor_after(&anchor).unwrap().revision(), host.revision());
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(host.learned_generation_inspection().unwrap().paused); assert!(!host.clock_ready());
    assert!(host.check_learned_checkpoint(&saved).is_err());
    assert!(host.dispatch(host.revision(), &old_key, &old_human, &old_action, &old_input, snapshot()).is_err());
    let ticket = host.evaluation_ticket(101).unwrap(); let revision = host.revision();
    assert_eq!(old.evaluator.assess(&mut host, revision, &ticket, assessment(GroundTruth::Benign)),
        Err(Error::Binding.into()));
    assert_eq!(fresh.evaluator.assess(&mut host, revision, &old_ticket, assessment(GroundTruth::Benign)),
        Err(Error::Binding.into()));
    assert!(fresh.evaluator.assess(&mut host, revision, &ticket, assessment(GroundTruth::Benign)).unwrap());
    assert!(!host.clock_ready()); assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.credibility_report().unwrap().retained_cases, 1);
    assert_eq!(host.credibility_report().unwrap().benign_origins, 1);
    resume(&mut host, 2); step(&mut host).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    fresh_identity(&mut host, &fresh.oversight, 2, 2);
    let (action, input, automatic, request) = prepared(&mut host, 2, 102);
    let revision = host.revision();
    assert!(old.oversight.human.approve(&mut host, revision, &request).is_err());
    assert!(host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(2)).is_err());
    let revision = host.revision();
    let human = fresh.oversight.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(3)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn exact_protocol_guard_policy_and_floors_refuse_without_replaying_or_cleaning_away_mismatch() {
    let root = Directory::new(); let c = config(0); let g = guards();
    let (mut host, _) = evaluated(&root, &c, &g);
    step(&mut host).unwrap(); let saved = capture(&mut host);
    let intent = begin_reset(&mut host, &saved); let expected = requirements(&host, g);
    let bytes = root.bytes(); drop(host);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"unacknowledged staging").unwrap();
    for field in 0..12 {
        let mut wrong = protocol();
        match field {
            0 => wrong.domain += 1, 1 => wrong.stratum += 1, 2 => wrong.period += 1,
            3 => wrong.minimum_violation_origins += 1, 4 => wrong.minimum_benign_origins += 1,
            5 => wrong.precision_floor.numerator += 1, 6 => wrong.precision_floor.denominator += 1,
            7 => wrong.recall_floor.numerator += 1, 8 => wrong.recall_floor.denominator += 1,
            9 => wrong.false_positive_ceiling.numerator += 1, 10 => wrong.false_positive_ceiling.denominator += 1,
            _ => wrong.false_stop_budget += 1,
        }
        assert_eq!(FileOversight::begin_open_evaluated_guarded_with_learned_generation(
            root.store(), profile(), &expected, &wrong, &c).err(), Some(Error::Binding.into()));
        assert_eq!(root.bytes(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging");
    }
    let mut changed = source(0); changed.telemetry.source_check_values -= 1;
    let wrong = FileLearnedConfig::new(numerical::model(), changed, LearnedDecoderBindingLimits::default()).unwrap();
    assert_eq!(FileOversight::begin_open_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &protocol(), &wrong).err(), Some(Error::Binding.into()));
    for field in 0..7 {
        let mut wrong = expected.clone();
        match field {
            0 => wrong.guards.identity = None, 1 => wrong.guards.campaigns = None,
            2 => wrong.guards.identity.as_mut().unwrap().policy.observer_id += 1,
            3 => wrong.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            4 => wrong.minimum.journal_revision += 1, 5 => wrong.minimum.control_sequence += 1,
            _ => wrong.minimum.authority_epoch += 1,
        }
        let mut run = FileOversight::begin_open_evaluated_guarded_with_learned_generation(
            root.store(), profile(), &wrong, &protocol(), &c).unwrap();
        let error = drive(&mut run, 3).unwrap_err();
        assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
        assert_eq!(run.advance(run.progress().replayed_events, 1).err(), Some(error.clone()));
        assert_eq!(run.finish_pending_reset(&intent).err(), Some(error));
        assert_eq!(root.bytes(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging");
    }
    let mut run = FileOversight::begin_open_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &protocol(), &c).unwrap();
    drive(&mut run, 1).unwrap(); let (host, roles) = run.finish_pending_reset(&intent).unwrap();
    assert!(roles.oversight.identity_observer.is_some() && roles.oversight.policy_governor.is_some());
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1); assert!(!pending.exists());
}
