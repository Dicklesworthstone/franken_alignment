//! Original learned numerical work, topology custody, and actual two-key effects.
//! Declared graph/probe fixtures do not qualify operating-system containment.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "mediated_learned_recovery/support.rs"]
mod support;
use support::*;
use fa_reference::Error;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{JournalError, Reconciliation}};
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight,
    decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus,
        checkpoint::FileLearnedResetIntent},
    guarded::FileRecoveryFloor,
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::{
    credibility::GroundTruth, decoder_monitoring::LearnedDecoderBindingLimits,
};
use fa_reference::round::Verdict;

#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod text_fixture;

#[test]
fn atomic_bootstrap_and_cooperative_recovery_preserve_every_declared_role() {
    let c = config(0);
    for mask in 0..8 {
        let root = Directory::new(); let mut g = guards();
        if mask & 1 == 0 { g.identity = None; }
        if mask & 2 == 0 { g.campaigns = None; }
        let evaluation = (mask & 4 != 0).then(protocol);
        let (mut host, roles) = FileOversight::create_mediated_guarded_with_learned_generation(
            root.store(), profile(), &g, None, graph(1, false), evaluation.clone(), c.clone()).unwrap();
        assert_eq!(host.revision(), 3 + u64::from(mask & 1 != 0)
            + u64::from(mask & 2 != 0) + u64::from(mask & 4 != 0));
        assert!(host.publication_guard_required() && host.learned_generation_required());
        assert_eq!(roles.oversight.identity_observer.is_some(), mask & 1 != 0);
        assert_eq!(roles.oversight.policy_governor.is_some(), mask & 2 != 0);
        assert_eq!(roles.evaluator.is_some(), evaluation.is_some());
        assert!(roles.consistency_observer.is_none());
        assert!(!host.clock_ready()); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.work.admitted_tokens, 0);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        assert!(step(&mut host).sample().is_none());
        assert!(matches!(certify(&mut host, &roles.topology_observer).unwrap(), CutCheck::Verified(_)));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let expected = requirements(&host, g, evaluation.clone());
        let report = evaluation.as_ref().map(|_| host.credibility_report().unwrap());
        let bytes = root.bytes(); drop(host);
        assert!(FileOversight::open_mediated_guarded(root.store(), profile(), &expected).is_err());
        assert!(FileOversight::open_guarded_with_learned_generation(
            root.store(), profile(), &expected.oversight, &c).is_err());
        let mut recovery = FileOversight::begin_open_mediated_guarded_with_learned_generation(
            root.store(), profile(), &expected, &c).unwrap();
        assert_eq!(recovery.progress().replayed_events, 0);
        assert_eq!(recovery.advance(1, 1).err(), Some(Error::Stale.into()));
        assert_eq!(recovery.advance(0, 0).err(), Some(Error::InvalidInput.into()));
        assert_eq!(FileOversight::begin_open_with_learned_generation(
            root.store(), profile(), &c).err(), Some(JournalError::Busy));
        recovery.advance(0, 1).unwrap(); drop(recovery);
        assert_eq!(root.bytes(), bytes);
        let mut recovery = FileOversight::begin_open_mediated_guarded_with_learned_generation(
            root.store(), profile(), &expected, &c).unwrap();
        drive(&mut recovery, 2).unwrap(); assert_eq!(root.bytes(), bytes);
        let (host, fresh) = recovery.finish().unwrap();
        assert_eq!(host.revision(), expected.oversight.minimum.journal_revision + 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused); assert!(!host.clock_ready());
        assert!(!host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert_eq!(fresh.oversight.identity_observer.is_some(), mask & 1 != 0);
        assert_eq!(fresh.oversight.policy_governor.is_some(), mask & 2 != 0);
        assert_eq!(fresh.evaluator.is_some(), evaluation.is_some());
        assert!(fresh.consistency_observer.is_none());
        assert_eq!(evaluation.as_ref().map(|_| host.credibility_report().unwrap()), report);
    }
}

#[test]
fn recovered_generation_needs_fresh_topology_identity_and_both_keys_to_publish() {
    let root = Directory::new(); let c = config(0);
    let (mut host, old) = create(&root, &c, Some(protocol()));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    // Quiet original numerics alone do not satisfy the initially unverified cut.
    assert!(host.propose(host.revision(), 99, spec(&host), snapshot()).is_err());
    assert!(matches!(certify(&mut host, &old.topology_observer).unwrap(), CutCheck::Verified(_)));
    fresh_identity(&mut host, &old.oversight, 1, 1);
    let (old_action, old_input, old_key, old_request) = prepared(&mut host, 1, 101);
    let revision = host.revision();
    let old_human = old.oversight.human.approve(&mut host, revision, &old_request).unwrap();
    let old_ticket = host.evaluation_ticket(101).unwrap(); let revision = host.revision();
    old.evaluator.as_ref().unwrap().assess(&mut host, revision, &old_ticket,
        assessment(GroundTruth::Censored)).unwrap();
    let report = host.credibility_report().unwrap();
    let expected = requirements(&host, guards(), Some(protocol())); drop(host);
    let (mut host, fresh) = FileOversight::open_mediated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &c).unwrap();
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert!(host.dispatch(host.revision(), &old_key, &old_human, &old_action, &old_input, snapshot()).is_err());
    let request = replacement(&host, 10, Some(graph(2, false))); let revision = host.revision();
    assert_eq!(old.topology_observer.update(&mut host, revision, &request), Err(Error::Binding.into()));
    assert_eq!(host.revision(), revision);
    let ticket = host.evaluation_ticket(101).unwrap();
    assert_eq!(old.evaluator.as_ref().unwrap().assess(&mut host, revision, &ticket,
        assessment(GroundTruth::Benign)), Err(Error::Binding.into()));
    assert_eq!(fresh.evaluator.as_ref().unwrap().assess(&mut host, revision, &old_ticket,
        assessment(GroundTruth::Benign)), Err(Error::Binding.into()));
    assert!(fresh.evaluator.as_ref().unwrap().assess(&mut host, revision, &ticket,
        assessment(GroundTruth::Benign)).unwrap());
    assert_eq!(host.credibility_report().unwrap().benign_origins, 1);
    resume(&mut host, 2);
    assert!(step(&mut host).sample().is_some(), "original output sampling continues after recovery");
    assert_eq!(certify(&mut host, &fresh.topology_observer), Err(Error::Incomplete));
    assert!(host.propose(host.revision(), 98, spec(&host), snapshot()).is_err());
    update(&mut host, &fresh.topology_observer, 10, Some(graph(2, false)));
    assert!(matches!(certify(&mut host, &fresh.topology_observer).unwrap(), CutCheck::Verified(_)));
    let (action, input) = staged(&mut host, 2);
    assert_eq!(reviewed(&mut host, 2, 102, &input, Verdict::Allow).err(), Some(Error::Incomplete.into()));
    assert!(host.authorize(host.revision(), 2, &input, snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
    fresh_identity(&mut host, &fresh.oversight, 2, 2);
    reviewed(&mut host, 2, 103, &input, Verdict::Allow).unwrap();
    let automatic = host.authorize(host.revision(), 2, &input, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1002, 2, &input, ElapsedTick(30)).unwrap();
    assert!(host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(2)).is_err());
    let revision = host.revision();
    assert_eq!(old.oversight.human.approve(&mut host, revision, &request).err(), Some(Error::Binding.into()));
    let human = fresh.oversight.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 2).is_err());
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.delivery_mediation(2).unwrap().unwrap().graph(), &graph(2, false));
}

#[test]
fn anchored_pending_reset_retains_unknown_liability_and_exact_retry_counts_once() {
    let root = Directory::new(); let c = config(0);
    let (mut host, old) = create(&root, &c, Some(protocol()));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let saved = capture(&mut host); let abandoned = step(&mut host).sample().unwrap().clone();
    certify(&mut host, &old.topology_observer).unwrap(); fresh_identity(&mut host, &old.oversight, 1, 1);
    let (old_action, old_input, old_key, request) = prepared(&mut host, 1, 101);
    let revision = host.revision();
    let old_human = old.oversight.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &old_key, &old_human, &old_action, &old_input, snapshot()).unwrap();
    let dispatch_cut = host.delivery_mediation(1).unwrap().cloned();
    let intent = begin_reset(&mut host, &saved);
    let mut expected = requirements(&host, guards(), Some(protocol()));
    let cut = host.revision(); let anchor = host.history_anchor().unwrap(); let bytes = root.bytes(); drop(host);
    let mut recovery = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &c, &anchor).unwrap();
    // Independent expectations are frozen; later caller mutations cannot relax
    // the private candidate or replace its required evaluator/topology contract.
    expected.topology.available = false; expected.oversight.minimum.journal_revision = u64::MAX;
    expected.evaluation.as_mut().unwrap().false_stop_budget = 0;
    drive(&mut recovery, 3).unwrap(); assert_eq!(root.bytes(), bytes);
    let (host, _) = recovery.finish_pending_reset(&intent).unwrap();
    assert_eq!(host.revision(), cut + 2);
    assert_eq!(host.history_anchor_after(&anchor).unwrap().revision(), host.revision());
    let receipt = host.learned_reset_result(900).unwrap().unwrap();
    assert!(receipt.control.restored); assert_eq!(receipt.position, 1); assert_eq!(receipt.sampled_draws, 0);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    let usage = host.learned_recovery_usage().unwrap(); assert_eq!(usage.restart_attempts, 1);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.delivery_mediation(1).unwrap(), dispatch_cut.as_ref());
    assert!(!host.mediation_snapshot().unwrap().available);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(host.pending_learned_reset().unwrap().is_none());
    let exact = requirements(&host, guards(), Some(protocol())); let revision = host.revision(); drop(host);
    let mut retry = FileOversight::begin_open_mediated_guarded_with_learned_generation(
        root.store(), profile(), &exact, &c).unwrap(); drive(&mut retry, 2).unwrap();
    let (mut host, fresh) = retry.finish_pending_reset(&intent).unwrap();
    assert_eq!(host.revision(), revision + 1);
    let retried = host.learned_reset_result(900).unwrap().unwrap();
    assert_eq!(retried.control, receipt.control);
    assert_eq!((retried.actor_revision, retried.resumed_stream, retried.position, retried.sampled_draws),
        (receipt.actor_revision, receipt.resumed_stream, receipt.position, receipt.sampled_draws));
    assert_eq!(host.learned_recovery_usage().unwrap(), usage);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.check_learned_checkpoint(&saved).is_err());
    resume(&mut host, 2);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.seal_unexecuted(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(step(&mut host).sample(), Some(&abandoned));
    assert!(host.propose(host.revision(), 99, spec(&host), snapshot()).is_err());
    update(&mut host, &fresh.topology_observer, 10, Some(graph(2, false)));
    certify(&mut host, &fresh.topology_observer).unwrap(); fresh_identity(&mut host, &fresh.oversight, 2, 2);
    let (action, input, automatic, request) = prepared(&mut host, 2, 102);
    assert!(host.dispatch(host.revision(), &old_key, &old_human, &old_action, &old_input, snapshot()).is_err());
    let revision = host.revision(); let human = fresh.oversight.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 1);
}

#[test]
fn exact_graphs_availability_evaluation_guards_and_floors_refuse_without_writes() {
    let root = Directory::new(); let c = config(0);
    let (mut host, roles) = create(&root, &c, Some(protocol()));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    let saved = capture(&mut host); step(&mut host);
    update(&mut host, &roles.topology_observer, 1, Some(graph(2, false)));
    certify(&mut host, &roles.topology_observer).unwrap();
    let intent = begin_reset(&mut host, &saved);
    let expected = requirements(&host, guards(), Some(protocol())); let bytes = root.bytes(); drop(host);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retained staging").unwrap();
    for field in 0..12 {
        let mut wrong = expected.clone();
        match field {
            0 => wrong.topology.initial = graph(1, true),
            1 => wrong.topology.current = graph(2, true),
            2 => wrong.topology.current = graph(1, false),
            3 => wrong.topology.available = false,
            4 => wrong.evaluation = None,
            5 => wrong.evaluation.as_mut().unwrap().domain += 1,
            6 => wrong.oversight.guards.identity = None,
            7 => wrong.oversight.guards.campaigns = None,
            8 => wrong.oversight.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            9 => wrong.oversight.minimum.journal_revision += 1,
            10 => wrong.oversight.minimum.control_sequence += 1,
            _ => wrong.oversight.minimum.authority_epoch += 1,
        }
        match FileOversight::begin_open_mediated_guarded_with_learned_generation(
            root.store(), profile(), &wrong, &c)
        {
            Err(_) => {}
            Ok(mut recovery) => {
                let error = drive(&mut recovery, 3).unwrap_err();
                assert!(matches!(recovery.progress().status, FileLearnedRecoveryStatus::Failed(_)));
                assert_eq!(recovery.advance(recovery.progress().replayed_events, 1).err(), Some(error.clone()));
                assert_eq!(recovery.finish_pending_reset(&intent).err(), Some(error));
            }
        }
        assert_eq!(root.bytes(), bytes, "field {field}");
        assert_eq!(std::fs::read(&pending).unwrap(), b"retained staging");
    }
    let mut wrong_source = source(0); wrong_source.telemetry.source_check_values -= 1;
    let wrong = FileLearnedConfig::new(numerical::model(), wrong_source, LearnedDecoderBindingLimits::default()).unwrap();
    assert_eq!(FileOversight::begin_open_mediated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &wrong).err(), Some(Error::Binding.into()));
    assert!(FileOversight::open_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &expected.oversight, &protocol(), &c).is_err());
    for field in 0..3 {
        let mut control = intent.control().clone();
        match field { 0 => control.operation += 1, 1 => control.expected_authority_epoch += 1,
            _ => control.binding.evidence_root[0] ^= 1 }
        let changed = FileLearnedResetIntent::for_recovery(intent.checkpoint(), control, intent.budget()).unwrap();
        let mut recovery = FileOversight::begin_open_mediated_guarded_with_learned_generation(
            root.store(), profile(), &expected, &c).unwrap(); drive(&mut recovery, 3).unwrap();
        assert_eq!(recovery.finish_pending_reset(&changed).err(), Some(Error::Binding.into()));
        assert_eq!(root.bytes(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"retained staging");
    }
    let mut recovery = FileOversight::begin_open_mediated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &c).unwrap(); drive(&mut recovery, 3).unwrap();
    let (host, _) = recovery.finish_pending_reset(&intent).unwrap();
    assert_eq!(host.revision(), expected.oversight.minimum.journal_revision + 2);
    assert!(!pending.exists()); assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
}

#[test]
fn exact_anchor_rejects_equal_counter_fork_and_truncation_before_cleanup() {
    let root = Directory::new(); let c = config(0);
    let (mut first, _) = create(&root, &c, None);
    first.observe_time(first.revision(), ElapsedTick(1)).unwrap(); step(&mut first);
    let anchor = first.history_anchor().unwrap(); let first_state = first.inspect(); drop(first);
    // Build a second fully valid original history at the same storage identity.
    // Its only different event is the clock input; counters and recipe coincide.
    std::fs::remove_dir_all(root.store()).unwrap();
    let (mut fork, _) = create(&root, &c, None); let prefix = root.bytes();
    fork.observe_time(fork.revision(), ElapsedTick(2)).unwrap(); step(&mut fork);
    let expected = requirements(&fork, guards(), None); let valid = fork.history_anchor().unwrap();
    assert_eq!(fork.revision(), first_state.revision);
    assert_eq!(fork.inspect().control.sequence, first_state.control.sequence);
    assert_eq!(fork.inspect().control.ledger.epoch, first_state.control.ledger.epoch);
    let bytes = root.bytes(); drop(fork);
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"do not clean").unwrap();
    assert_eq!(FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &c, &anchor).err(), Some(Error::Binding.into()));
    assert_eq!(root.bytes(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"do not clean");
    std::fs::write(root.store().join("delivery.bin"), &prefix).unwrap();
    let mut lower = expected.clone(); lower.oversight.minimum = FileRecoveryFloor {
        journal_revision: 0, control_sequence: 0, authority_epoch: 0 };
    assert_eq!(FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        root.store(), profile(), &lower, &c, &valid).err(), Some(Error::Stale.into()));
    assert_eq!(root.bytes(), prefix); assert_eq!(std::fs::read(&pending).unwrap(), b"do not clean");
    std::fs::write(root.store().join("delivery.bin"), &bytes).unwrap();
    let mut recovery = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        root.store(), profile(), &expected, &c, &valid).unwrap(); drive(&mut recovery, 2).unwrap();
    let (host, _) = recovery.finish().unwrap();
    assert_eq!(host.history_anchor_after(&valid).unwrap().revision(), host.revision());
    assert!(!pending.exists());
}

#[test]
fn bypass_topology_stops_effects_despite_quiet_generation_and_both_old_keys() {
    let root = Directory::new(); let c = config(0);
    let (mut host, roles) = create(&root, &c, None);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    certify(&mut host, &roles.topology_observer).unwrap(); fresh_identity(&mut host, &roles.oversight, 1, 1);
    let (old_action, old_input, old_key, request) = prepared(&mut host, 1, 101);
    let revision = host.revision(); let old_human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
    update(&mut host, &roles.topology_observer, 10, Some(graph(2, true)));
    assert!(!matches!(certify(&mut host, &roles.topology_observer).unwrap(), CutCheck::Verified(_)));
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert!(host.propose(host.revision(), 99, spec(&host), snapshot()).is_err());
    assert!(host.dispatch(host.revision(), &old_key, &old_human, &old_action, &old_input, snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
    update(&mut host, &roles.topology_observer, 11, Some(graph(3, false)));
    assert!(matches!(certify(&mut host, &roles.topology_observer).unwrap(), CutCheck::Verified(_)));
    fresh_identity(&mut host, &roles.oversight, 2, 1);
    let (action, input, automatic, request) = prepared(&mut host, 2, 102);
    let revision = host.revision(); let human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(1)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.delivery_mediation(2).unwrap().unwrap().graph(), &graph(3, false));
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn invalid_native_bootstraps_and_predictor_pins_never_create_partial_storage() {
    use fa_reference::action::consequence::activation::{
        consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
        monitor::learned::LearnedMonitorBudget, probe::learned::MAX_CHECKED_KV_BYTES,
        tensor::kv::experiment::KvSide,
    };
    use fa_reference::action::consequence::delivery::persistent::observed::consistency::{
        FileConsistencyConfig, FileConsistencyParameters, learned::FileLearnedConsistencyConfig,
    };
    let root = Directory::new(); let mut invalid = protocol(); invalid.recall_floor.denominator = 0;
    assert!(FileOversight::create_mediated_guarded_with_learned_generation(root.store(), profile(),
        &guards(), None, graph(1, false), Some(invalid), config(0)).is_err());
    assert!(!root.store().exists());
    let mut foreign = graph(1, false).spec().clone();
    foreign.scope.tenant += 1; foreign.family.scope.tenant += 1;
    let foreign = fa_reference::action::consequence::mediation::AuthorityGraph::new(foreign).unwrap();
    assert!(FileOversight::create_mediated_guarded_with_learned_generation(root.store(), profile(),
        &guards(), None, foreign, None, config(0)).is_err());
    assert!(!root.store().exists());
    let mut invalid = guards(); invalid.identity.as_mut().unwrap().policy.observer_id = 0;
    assert!(FileOversight::create_mediated_guarded_with_learned_generation(root.store(), profile(),
        &invalid, None, graph(1, false), None, config(0)).is_err());
    assert!(!root.store().exists());
    let model = text_fixture::model(&[u32::from(b'a'), text_fixture::END]);
    let plain = FileLearnedConfig::new_text(model.clone(), text_fixture::tokenizer(&model),
        text_fixture::config(&model), LearnedDecoderBindingLimits::default()).unwrap()
        .with_required_sidecar().unwrap();
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let prediction = |owned: bool| FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1,
        profile: if owned { model.cache_profile().layers()[&1].values().profile() }
            else { model.residual_contract(1).unwrap().profile() },
        weights: vec![1.0, 0.0, 0.0, 0.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"a".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: 50,
    }).unwrap();
    let raw = plain.clone().with_required_pre_output_forecast(prediction(false)
        .with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap()).unwrap();
    let owned = plain.with_required_owned_pre_output_forecast(FileLearnedConsistencyConfig::new(
        prediction(true), 1, KvSide::Value, LearnedMonitorBudget::default(), LearnedMonitorBudget::default(),
        MAX_CHECKED_KV_BYTES).unwrap().with_owned_generation().unwrap().with_pre_output_forecast().unwrap()).unwrap();
    for pinned in [raw, owned] {
        assert_eq!(FileOversight::create_mediated_guarded_with_learned_generation(root.store(), profile(),
            &guards(), None, graph(1, false), None, pinned.clone()).err(), Some(Error::Binding.into()));
        assert!(!root.store().exists());
    }
    let (host, _) = create(&root, &config(0), None);
    let expected = requirements(&host, guards(), None); let bytes = root.bytes(); drop(host);
    let mut wrong = expected.clone(); wrong.prediction = Some(prediction(false));
    assert_eq!(FileOversight::begin_open_mediated_guarded_with_learned_generation(
        root.store(), profile(), &wrong, &config(0)).err(), Some(Error::Binding.into()));
    assert_eq!(root.bytes(), bytes);
    let (host, _) = FileOversight::open_mediated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &config(0)).unwrap();
    assert_eq!(host.revision(), expected.oversight.minimum.journal_revision + 1);
}

#[test]
fn recipe_owned_policy_source_matches_the_guard_and_installs_once() {
    use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
    use fa_reference::action::consequence::oversight::{
        policy_state::{StateSource, StateLimits, StateFreshness},
        evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES},
    };
    let policy = FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() };
    let model = text_fixture::model(&[u32::from(b'a'), text_fixture::END]);
    let c = FileLearnedConfig::new_text(model.clone(), text_fixture::tokenizer(&model),
        text_fixture::config(&model), LearnedDecoderBindingLimits::default()).unwrap()
        .with_required_sidecar().unwrap().with_required_policy_source(policy).unwrap();
    let mut g = guards(); g.identity = None; g.campaigns = None; g.source = Some(policy);
    for field in 0..3 {
        let root = Directory::new(); let mut wrong = g.clone();
        match field { 0 => wrong.source = None, 1 => wrong.source.as_mut().unwrap().source.source += 1,
            _ => wrong.source.as_mut().unwrap().freshness = StateFreshness::new(6).unwrap() }
        assert_eq!(FileOversight::create_mediated_guarded_with_learned_generation(root.store(), profile(),
            &wrong, None, graph(1, false), None, c.clone()).err(), Some(Error::Binding.into()));
        assert!(!root.store().exists());
    }
    let root = Directory::new();
    let (mut host, roles) = FileOversight::create_mediated_guarded_with_learned_generation(root.store(), profile(),
        &g, None, graph(1, false), None, c.clone()).unwrap();
    assert_eq!(host.revision(), 3); assert!(host.policy_only_file_source_required());
    assert_eq!(host.file_source_status().unwrap().policy, policy);
    let observation = EvidenceSnapshot::new(EvidenceIdentity {
        source: 42, generation: 1, scope: profile().delivery.scope,
    }, snapshot(), std::collections::BTreeMap::from([("reviewer".to_owned(), Vec::new())])).unwrap();
    let path = root.store().with_extension("policy.json"); std::fs::write(&path, observation.encode()).unwrap();
    let mut reader = FileEvidenceSource::new(path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
    assert_eq!(host.file_source_status().unwrap().producer, Some(observation.identity()));
    assert!(step(&mut host).sample().is_none());
    certify(&mut host, &roles.topology_observer).unwrap();
    let expected = requirements(&host, g, None); let numerical = host.learned_generation_inspection().unwrap().numerical;
    drop(host);
    let (host, _) = FileOversight::open_mediated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &c).unwrap();
    assert!(host.policy_only_file_source_required() && host.learned_sidecar_required());
    assert_eq!(host.file_source_status().unwrap().policy, policy);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(!host.mediation_snapshot().unwrap().available); assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 0);
}
