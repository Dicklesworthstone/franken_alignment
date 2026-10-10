//! Original sampled output, pinned forecast custody, native model review and
//! actual two-key publication after a mediated learned reset/recovery.
//! Synthetic weights, probability tables and graphs are controls, not deployment
//! containment or conditional-calibration qualification.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod numerical;
#[path = "mediated_learned_recovery/support.rs"]
mod custody;
#[path = "mediated_predictive_learned_recovery/native_fixture.rs"]
mod native_fixture;
#[path = "mediated_predictive_learned_recovery/support.rs"]
mod support;
use support::*;
use custody::{graph, protocol, assessment, certify, replacement, update, fresh_identity, snapshot};

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction,
    Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::{FrameIdentity,
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    probe::{LinearProbe, learned::MAX_CHECKED_KV_BYTES},
    tensor::kv::{experiment::KvSide, model::MAX_MODEL_KV_VALUES,
        decoder::{DecoderModel, monitoring::restart::KvRestartBudget,
            sampling::{SampledToken, monitored::GenerationEvent}}},
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{
    FileDeliveryProfile, FilePermit, JournalError, JournalLimits, Reconciliation,
    requests::FileRequestDisposition,
    observed::{FileHumanRequest, FileOversight, FileOversightProfile,
        consistency::{FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
            learned::FileLearnedConsistencyConfig},
        containment::FileResetRequest,
        decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus,
            checkpoint::{FileLearnedCheckpoint, FileLearnedResetIntent}},
        guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
            mediated::{FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement,
                predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements,
                    FilePredictiveMediatedLearnedRecovery}}},
        helpers::learned::native::{NativeReviewMember, NativeReviewLimits, NativeReviewStatus},
    },
}};
use fa_reference::action::consequence::gate::{ReviewBinding,
    containment::{ActorState, RestartGrade, RestartProfile,
        session::policy::{Policy, Predicate}},
};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, credibility::{EvaluationProtocol, GroundTruth},
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_source::{LearnedEvidenceLimits, text::LearnedTextConfig},
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS}},
    sidecar::{SidecarIdentity, SidecarCongressBudget, receiver::native::{SidecarDecisionBasis, SidecarProbeQuery}},
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn atomic_bootstrap_and_cooperative_recovery_keep_all_roles_and_exact_pins() {
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    for mode in [Mode::Raw, Mode::Owned] {
        let config = recipe(&model, source(&model), mode, true);
        for enabled in [false, true] {
            let root = Directory::new();
            let mut g = guards(&config);
            if !enabled { g.identity = None; g.campaigns = None; }
            let evaluation = enabled.then(protocol);
            let (mut host, roles) = FileOversight::create_predictive_mediated_guarded_with_learned_generation(
                root.store(), profile(), &g, None, graph(1, false), prediction(&config),
                evaluation.clone(), config.clone()).unwrap();
            assert_eq!(host.revision(), if enabled { 6 } else { 3 },
                "predictor is installed inside the sole learned Enable");
            assert!(host.publication_guard_required() && host.learned_sidecar_required()
                && host.action_consistency_required());
            assert!(roles.consistency_observer.is_some());
            assert_eq!(roles.evaluator.is_some(), enabled);
            assert_eq!(roles.oversight.identity_observer.is_some(), enabled);
            assert_eq!(roles.oversight.policy_governor.is_some(), enabled);
            assert!(!host.clock_ready());
            assert!(host.mediation_snapshot().unwrap().accepted.is_none());
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            assert!(step(&mut host).sample().is_none());
            assert!(matches!(certify(&mut host, &roles.topology_observer).unwrap(), CutCheck::Verified(_)));
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let mut expected = requirements(&host, &config, evaluation);
            expected.oversight.guards = g;
            let disk = root.bytes();
            drop(host);
            let ordinary = FileMediatedRequirements { oversight: expected.oversight.clone(),
                topology: expected.topology.clone(), prediction: None, evaluation: expected.evaluation.clone() };
            assert_eq!(FileOversight::begin_open_mediated_guarded_with_learned_generation(
                root.store(), profile(), &ordinary, &config).err(), Some(Error::Binding.into()));
            let mut run = FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
                root.store(), profile(), &expected, &config).unwrap();
            assert_eq!(run.progress().replayed_events, 0);
            assert_eq!(run.advance(1, 1).err(), Some(Error::Stale.into()));
            assert_eq!(run.advance(0, 0).err(), Some(Error::InvalidInput.into()));
            assert_eq!(FileOversight::begin_open_with_learned_generation(
                root.store(), profile(), &config).err(), Some(JournalError::Busy));
            run.advance(0, 1).unwrap();
            drop(run);
            assert_eq!(root.bytes(), disk);
            let mut run = FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
                root.store(), profile(), &expected, &config).unwrap();
            drive(&mut run, 3).unwrap();
            assert_eq!(root.bytes(), disk);
            let (host, fresh) = run.finish().unwrap();
            assert_eq!(host.revision(), expected.oversight.minimum.journal_revision + 1);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            assert!(host.learned_generation_inspection().unwrap().paused);
            assert!(!host.clock_ready() && !host.mediation_snapshot().unwrap().available);
            assert!(host.mediation_snapshot().unwrap().accepted.is_none());
            assert!(fresh.consistency_observer.is_some());
            assert_eq!(fresh.evaluator.is_some(), enabled);
            assert_eq!(fresh.oversight.identity_observer.is_some(), enabled);
            assert_eq!(fresh.oversight.policy_governor.is_some(), enabled);
        }
        let root = Directory::new();
        let plain = FileLearnedConfig::new_text(model.clone(), numerical::tokenizer(&model), source(&model),
            LearnedDecoderBindingLimits::default()).unwrap().with_required_sidecar().unwrap();
        assert_eq!(FileOversight::create_predictive_mediated_guarded_with_learned_generation(
            root.store(), profile(), &guards(&plain), None, graph(1, false), prediction(&config),
            None, plain).err(), Some(Error::Binding.into()));
        assert!(!root.store().exists());
        let opposite = recipe(&model, source(&model),
            match mode { Mode::Raw => Mode::Owned, Mode::Owned => Mode::Raw }, true);
        assert_eq!(FileOversight::create_predictive_mediated_guarded_with_learned_generation(
            root.store(), profile(), &guards(&config), None, graph(1, false), prediction(&opposite),
            None, config.clone()).err(), Some(Error::Binding.into()));
        assert!(!root.store().exists());
        let mut invalid = protocol(); invalid.recall_floor.denominator = 0;
        assert!(FileOversight::create_predictive_mediated_guarded_with_learned_generation(
            root.store(), profile(), &guards(&config), None, graph(1, false), prediction(&config),
            Some(invalid), config.clone()).is_err());
        assert!(!root.store().exists());
        assert_eq!(FileOversight::create_mediated_guarded_with_learned_generation(
            root.store(), profile(), &guards(&config), None, graph(1, false), None, config).err(),
            Some(Error::Binding.into()));
        assert!(!root.store().exists());
    }
}

#[test]
fn raw_and_owned_reset_successors_reach_native_review_and_both_key_publication() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Directory::new();
        let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
        let source = source(&model);
        let expected_samples = original_samples(&model, source.clone());
        let config = recipe(&model, source.clone(), mode, true);
        let (mut host, old) = create(&root, &config, Some(protocol()));
        let saved = checkpoint_before_prompt_end(&mut host);
        certify(&mut host, &old.topology_observer).unwrap();
        let intent = begin_reset(&mut host, &saved, source.policy.allowance());
        let mut expected = requirements(&host, &config, Some(protocol()));
        let anchor = host.history_anchor().unwrap();
        let cut = host.revision();
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let disk = root.bytes();
        drop(host);
        let mut run = FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            root.store(), profile(), &expected, &config, &anchor).unwrap();
        // Mutation of the caller's requirements cannot change this cursor.
        expected.topology.available = false;
        expected.evaluation = None;
        expected.oversight.minimum.journal_revision = u64::MAX;
        drive(&mut run, 3).unwrap();
        assert_eq!(root.bytes(), disk);
        let (mut host, roles) = run.finish_pending_reset(&intent).unwrap();
        assert_eq!(host.revision(), cut + 2);
        assert_eq!(host.history_anchor_after(&anchor).unwrap().revision(), host.revision());
        let receipt = host.learned_reset_result(900).unwrap().unwrap();
        assert!(receipt.control.restored);
        assert_eq!(receipt.resumed_stream, Some(22));
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        let usage = host.learned_recovery_usage().unwrap();
        assert_eq!(usage.restart_attempts, 1);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        assert!(host.pending_learned_reset().unwrap().is_none());
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready() && !host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert_eq!(host.check_learned_checkpoint(&saved), Err(Error::Binding.into()));
        assert_eq!(mode.forecast(&mut host, old.consistency_observer.as_ref().unwrap(), 71).err(),
            Some(Error::Binding.into()));
        let replacement = replacement(&host, 10, Some(graph(2, false)));
        let revision = host.revision();
        assert_eq!(old.topology_observer.update(&mut host, revision, &replacement), Err(Error::Binding.into()));
        resume(&mut host);
        let revision = host.revision();
        assert!(mode.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 71).is_err());
        assert_eq!(host.revision(), revision);
        let accepted = step(&mut host);
        assert!(accepted.sample().is_none());
        assert!(accepted.audit().source().descriptor().layers().values().all(|layer| layer.stream == 22));
        assert_eq!(certify(&mut host, &roles.topology_observer), Err(Error::Incomplete));
        // Topology changes advance authority epoch. Install the new graph
        // before acquiring a forecast bound to that epoch.
        update(&mut host, &roles.topology_observer, 10, Some(graph(2, false)));
        assert!(matches!(certify(&mut host, &roles.topology_observer).unwrap(), CutCheck::Verified(_)));
        let forecast = mode.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 71).unwrap().unwrap();
        assert_eq!((forecast.frame.stream, forecast.frame.sequence), (22, 2));
        assert_eq!(forecast.probability, BinaryForecast::new(32_768, 32_768).unwrap());
        assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
        let (samples, last) = finish_generation(&mut host);
        assert_eq!(samples, expected_samples);
        fresh_identity(&mut host, &roles.oversight, 1, 2);
        submit_generated(&mut host, 71);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        assert!(host.pending_forecast_request().unwrap().is_none());
        let (action, input, automatic, request) = prepared(&mut host, 71, 101, &last);
        assert_eq!(host.inspect().executions, 0);
        assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).is_err());
        let revision = host.revision();
        assert_eq!(old.oversight.human.approve(&mut host, revision, &request).err(), Some(Error::Binding.into()));
        let ticket = host.evaluation_ticket(101).unwrap();
        assert_eq!(old.evaluator.as_ref().unwrap().assess(&mut host, revision, &ticket,
            assessment(GroundTruth::Benign)), Err(Error::Binding.into()));
        assert!(roles.evaluator.as_ref().unwrap().assess(&mut host, revision, &ticket,
            assessment(GroundTruth::Benign)).unwrap());
        assert_eq!(host.credibility_report().unwrap().benign_origins, 1);
        let revision = host.revision();
        let human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
        assert_eq!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap().outcome,
            EndpointOutcome::Executed { resulting_version: 2 });
        host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().payload, b"OK");
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.delivery_mediation(1).unwrap().unwrap().graph(), &graph(2, false));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let expected = requirements(&host, &config, Some(protocol()));
        let revision = host.revision();
        drop(host);
        let (host, _) = recover(&root, &config, &expected, &intent).unwrap();
        assert_eq!(host.revision(), revision + 1, "an exact completed reset retry only fences");
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.inspect().executions, 1);
    }
}

#[test]
fn exact_predictor_topology_evaluator_guards_and_floors_reject_without_writes() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Directory::new();
        let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
        let source = source(&model);
        let config = recipe(&model, source.clone(), mode, true);
        let (mut host, roles) = create(&root, &config, Some(protocol()));
        let saved = checkpoint_before_prompt_end(&mut host);
        update(&mut host, &roles.topology_observer, 1, Some(graph(2, false)));
        certify(&mut host, &roles.topology_observer).unwrap();
        let intent = begin_reset(&mut host, &saved, source.policy.allowance());
        let expected = requirements(&host, &config, Some(protocol()));
        let disk = root.bytes();
        drop(host);
        let pending = root.store().join("delivery.pending");
        std::fs::write(&pending, b"retained staging").unwrap();
        let opposite = recipe(&model, source.clone(),
            match mode { Mode::Raw => Mode::Owned, Mode::Owned => Mode::Raw }, true);
        for field in 0..14 {
            let mut wrong = expected.clone();
            match field {
                0 => wrong.prediction = prediction(&opposite),
                1 => wrong.prediction = changed_prediction(&model, &config, mode),
                2 => wrong.topology.initial = graph(1, true),
                3 => wrong.topology.current = graph(2, true),
                4 => wrong.topology.current = graph(1, false),
                5 => wrong.topology.available = false,
                6 => wrong.evaluation = None,
                7 => wrong.evaluation.as_mut().unwrap().domain += 1,
                8 => wrong.oversight.guards.identity = None,
                9 => wrong.oversight.guards.campaigns = None,
                10 => wrong.oversight.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap(),
                11 => wrong.oversight.minimum.journal_revision += 1,
                12 => wrong.oversight.minimum.control_sequence += 1,
                _ => wrong.oversight.minimum.authority_epoch += 1,
            }
            match FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
                root.store(), profile(), &wrong, &config)
            {
                Err(error) => assert_eq!(error, JournalError::Contract(Error::Binding)),
                Ok(mut run) => {
                    let error = drive(&mut run, 3).unwrap_err();
                    assert!(matches!(run.progress().status, FileLearnedRecoveryStatus::Failed(_)));
                    assert_eq!(run.advance(run.progress().replayed_events, 1).err(), Some(error.clone()));
                    assert_eq!(run.finish_pending_reset(&intent).err(), Some(error));
                }
            }
            assert_eq!(root.bytes(), disk, "field {field}");
            assert_eq!(std::fs::read(&pending).unwrap(), b"retained staging");
        }
        let changed_recipe = recipe(&model, numerical::config(&model), mode, true);
        assert_eq!(FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
            root.store(), profile(), &expected, &changed_recipe).err(), Some(Error::Binding.into()));
        assert_eq!(root.bytes(), disk);
        for field in 0..3 {
            let mut control = intent.control().clone();
            match field { 0 => control.operation += 1, 1 => control.expected_authority_epoch += 1,
                _ => control.binding.evidence_root[0] ^= 1 }
            let changed = FileLearnedResetIntent::for_recovery(intent.checkpoint(), control, intent.budget()).unwrap();
            let mut run = FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
                root.store(), profile(), &expected, &config).unwrap();
            drive(&mut run, 3).unwrap();
            assert_eq!(run.finish_pending_reset(&changed).err(), Some(Error::Binding.into()));
            assert_eq!(root.bytes(), disk);
            assert_eq!(std::fs::read(&pending).unwrap(), b"retained staging");
        }
        let (host, _) = recover(&root, &config, &expected, &intent).unwrap();
        assert_eq!(host.revision(), expected.oversight.minimum.journal_revision + 2);
        assert!(!pending.exists());
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    }
}

#[test]
fn anchored_recovery_rejects_equal_counter_forks_and_truncation_before_cleanup() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Directory::new();
        let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
        let config = recipe(&model, source(&model), mode, true);
        let (mut host, _) = create(&root, &config, None);
        step(&mut host);
        let anchor = host.history_anchor().unwrap();
        let first = host.inspect();
        drop(host);
        // Original valid images at the same storage identity; only time differs.
        std::fs::remove_dir_all(root.store()).unwrap();
        let (mut fork, _) = FileOversight::create_predictive_mediated_guarded_with_learned_generation(
            root.store(), profile(), &guards(&config), None, graph(1, false),
            prediction(&config), None, config.clone()).unwrap();
        let prefix = root.bytes();
        fork.observe_time(fork.revision(), ElapsedTick(2)).unwrap();
        step(&mut fork);
        assert_eq!(fork.revision(), first.revision);
        assert_eq!(fork.inspect().control.sequence, first.control.sequence);
        assert_eq!(fork.inspect().control.ledger.epoch, first.control.ledger.epoch);
        let expected = requirements(&fork, &config, None);
        let valid = fork.history_anchor().unwrap();
        let disk = root.bytes();
        drop(fork);
        let pending = root.store().join("delivery.pending");
        std::fs::write(&pending, b"do not clean").unwrap();
        assert_eq!(FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            root.store(), profile(), &expected, &config, &anchor).err(), Some(Error::Binding.into()));
        assert_eq!(root.bytes(), disk);
        assert_eq!(std::fs::read(&pending).unwrap(), b"do not clean");
        std::fs::write(root.store().join("delivery.bin"), &prefix).unwrap();
        let mut lower = expected.clone();
        lower.oversight.minimum = FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 };
        assert_eq!(FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            root.store(), profile(), &lower, &config, &valid).err(), Some(Error::Stale.into()));
        assert_eq!(root.bytes(), prefix);
        assert_eq!(std::fs::read(&pending).unwrap(), b"do not clean");
        std::fs::write(root.store().join("delivery.bin"), &disk).unwrap();
        let mut run = FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            root.store(), profile(), &expected, &config, &valid).unwrap();
        drive(&mut run, 2).unwrap();
        let (host, _) = run.finish().unwrap();
        assert_eq!(host.history_anchor_after(&valid).unwrap().revision(), host.revision());
        assert!(!pending.exists());
    }
}

#[test]
fn pending_forecasts_and_spent_owned_work_never_become_fresh_after_reset() {
    for mode in [Mode::Raw, Mode::Owned] {
        for sampled in [false, true] {
            let root = Directory::new();
            let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
            let source = source(&model);
            let config = recipe(&model, source.clone(), mode, true);
            let (mut host, roles) = create(&root, &config, None);
            let saved = checkpoint_before_prompt_end(&mut host);
            certify(&mut host, &roles.topology_observer).unwrap();
            mode.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 71).unwrap().unwrap();
            let acquired = host.learned_action_consistency_snapshot().ok();
            let evidence = host.action_consistency_snapshot().unwrap();
            if sampled { assert!(step(&mut host).sample().is_some()); }
            let spent = host.learned_generation_inspection().unwrap().numerical.cumulative_work;
            let intent = begin_reset(&mut host, &saved, source.policy.allowance());
            let expected = requirements(&host, &config, None);
            drop(host);
            let (mut host, roles) = recover(&root, &config, &expected, &intent).unwrap();
            let after = host.action_consistency_snapshot().unwrap();
            assert!(after.coverage_lost);
            assert_eq!(after.evidence, evidence.evidence);
            assert_eq!(after.pending_attempt, evidence.pending_attempt);
            assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
            if let Some(acquired) = acquired {
                let after = host.learned_action_consistency_snapshot().unwrap();
                assert_eq!(after.work, acquired.work);
                assert_eq!(after.retained_source_bytes, acquired.retained_source_bytes);
                assert_eq!(after.has_unreported_work, acquired.has_unreported_work);
            }
            let restored = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(restored.work.sampling_attempts, 0);
            assert_eq!(restored.cumulative_work.sampling_attempts, spent.sampling_attempts);
            assert_eq!(spent.sampling_attempts, u64::from(sampled));
            resume(&mut host);
            let n = host.learned_generation_inspection().unwrap().numerical;
            let revision = host.revision();
            let disk = root.bytes();
            assert!(host.advance_learned_generation(revision, n.actor_revision, n.position).is_err());
            assert!(mode.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 72).is_err());
            assert_eq!(host.revision(), revision);
            assert_eq!(root.bytes(), disk);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn answered_sampled_output_and_exhausted_owned_lifetime_remain_terminal() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Directory::new();
        let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
        let source = source(&model);
        let config = recipe(&model, source.clone(), mode, true);
        let (mut host, roles) = create(&root, &config, None);
        let saved = checkpoint_before_prompt_end(&mut host);
        certify(&mut host, &roles.topology_observer).unwrap();
        mode.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 71).unwrap().unwrap();
        assert_eq!(finish_samples(&mut host).len(), 3);
        fresh_identity(&mut host, &roles.oversight, 1, 1);
        submit_generated(&mut host, 71);
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        assert_eq!(evidence.samples(), 1);
        assert!(host.pending_forecast_request().unwrap().is_none());
        let acquired = host.learned_action_consistency_snapshot().ok();
        let intent = begin_reset(&mut host, &saved, source.policy.allowance());
        let expected = requirements(&host, &config, None);
        drop(host);
        let (mut host, roles) = recover(&root, &config, &expected, &intent).unwrap();
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost,
            "isolate abandoned sampled history from unanswered-forecast loss");
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        let restored = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(restored.work.sampling_attempts, 0);
        assert_eq!(restored.cumulative_work.sampling_attempts, 3);
        if let Some(acquired) = acquired {
            let after = host.learned_action_consistency_snapshot().unwrap();
            assert_eq!(after.work, acquired.work);
            assert_eq!(after.retained_source_bytes, acquired.retained_source_bytes);
        }
        resume(&mut host);
        let n = host.learned_generation_inspection().unwrap().numerical;
        let disk = root.bytes();
        assert_eq!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).err(),
            Some(Error::WrongState.into()));
        assert!(mode.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 72).is_err());
        assert_eq!(root.bytes(), disk);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert_eq!(host.inspect().executions, 0);
    }

    let root = Directory::new();
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    let source = source(&model);
    let lifetime = LearnedMonitorBudget { probe_coordinates: 0, ..LearnedMonitorBudget::default() };
    let config = recipe_with_lifetime(&model, source.clone(), Mode::Owned, true, lifetime);
    let (mut host, roles) = create(&root, &config, None);
    let saved = checkpoint_before_prompt_end(&mut host);
    assert!(Mode::Owned.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 71).unwrap().is_err());
    let acquired = host.learned_action_consistency_snapshot().unwrap();
    assert!(acquired.consistency.coverage_lost);
    assert!(acquired.retained_source_bytes > 0);
    let intent = begin_reset(&mut host, &saved, source.policy.allowance());
    let expected = requirements(&host, &config, None);
    drop(host);
    let (mut host, roles) = recover(&root, &config, &expected, &intent).unwrap();
    let after = host.learned_action_consistency_snapshot().unwrap();
    assert!(after.consistency.coverage_lost);
    assert_eq!(after.consistency.evidence, acquired.consistency.evidence);
    assert_eq!(after.work, acquired.work);
    assert_eq!(after.retained_source_bytes, acquired.retained_source_bytes);
    assert_eq!(after.has_unreported_work, acquired.has_unreported_work);
    resume(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let disk = root.bytes();
    assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
    assert!(Mode::Owned.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 72).is_err());
    assert_eq!(root.bytes(), disk);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, acquired.work);
}

#[test]
fn unknown_publications_stay_charged_and_old_roles_cannot_publish() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Directory::new();
        let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
        let config = recipe(&model, source(&model), mode, true);
        let (mut host, old) = create(&root, &config, Some(protocol()));
        step(&mut host); step(&mut host);
        certify(&mut host, &old.topology_observer).unwrap();
        mode.forecast(&mut host, old.consistency_observer.as_ref().unwrap(), 71).unwrap().unwrap();
        let (_, last) = finish_generation(&mut host);
        fresh_identity(&mut host, &old.oversight, 1, 1);
        submit_generated(&mut host, 71);
        let (action, input, automatic, request) = prepared(&mut host, 71, 101, &last);
        let revision = host.revision();
        let human = old.oversight.human.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
        let accepted_cut = host.delivery_mediation(1).unwrap().cloned();
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().executions, 0);
        let expected = requirements(&host, &config, Some(protocol()));
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let acquired = host.learned_action_consistency_snapshot().ok();
        drop(host);
        let (mut host, fresh) = FileOversight::open_predictive_mediated_guarded_with_learned_generation(
            root.store(), profile(), &expected, &config).unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.delivery_mediation(1).unwrap(), accepted_cut.as_ref());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        if let Some(acquired) = acquired {
            let after = host.learned_action_consistency_snapshot().unwrap();
            assert_eq!(after.work, acquired.work);
            assert_eq!(after.retained_source_bytes, acquired.retained_source_bytes);
        }
        let disk = root.bytes();
        assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
        let revision = host.revision();
        assert_eq!(old.oversight.human.approve(&mut host, revision, &request).err(), Some(Error::Binding.into()));
        assert_eq!(mode.forecast(&mut host, old.consistency_observer.as_ref().unwrap(), 72).err(),
            Some(Error::Binding.into()));
        assert_eq!(root.bytes(), disk);
        assert!(fresh.consistency_observer.is_some() && fresh.evaluator.is_some());
        // Settlement does not require a new forecast, native vote or live cut.
        host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        host.seal_unexecuted(host.revision(), 1).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.inspect().executions, 0);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.mediation_snapshot().unwrap().available);
    }
}

#[test]
fn native_allow_and_valid_forecasts_do_not_override_a_bypass_graph() {
    for mode in [Mode::Raw, Mode::Owned] {
        let mut common_epoch = None;
        for bypass in [false, true] {
            let root = Directory::new();
            let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
            let config = recipe(&model, source(&model), mode, true);
            let mut g = guards(&config);
            // Both controls use the same optional-guard inventory. Isolate the
            // cut from identity renewal and any approval from an older epoch.
            g.identity = None; g.campaigns = None;
            let (mut host, roles) = FileOversight::create_predictive_mediated_guarded_with_learned_generation(
                root.store(), profile(), &g, None, graph(1, false), prediction(&config),
                None, config.clone()).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            step(&mut host); step(&mut host);
            update(&mut host, &roles.topology_observer, 10, Some(graph(2, bypass)));
            let epoch = host.inspect().control.ledger.epoch;
            if let Some(expected) = common_epoch { assert_eq!(epoch, expected); }
            common_epoch = Some(epoch);
            let cut = certify(&mut host, &roles.topology_observer).unwrap();
            assert_eq!(matches!(cut, CutCheck::Verified(_)), !bypass);
            // The only graph difference is the direct actor-to-sink edge.
            // Both acquire their forecast AFTER the same epoch transition.
            let forecast = mode.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 71)
                .unwrap().unwrap();
            assert_eq!(forecast.probability, BinaryForecast::new(32_768, 32_768).unwrap());
            let (_, last) = finish_generation(&mut host);
            let payload = host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes().to_vec();
            assert_eq!(payload, b"OK");
            let spec = ActionSpec { version: VERSION, scope: profile().delivery.scope,
                target: Some(host.inspect().target), payload, required_witnesses: Vec::new(),
                policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 };
            host.submit_request(host.revision(), 71, spec, snapshot()).unwrap();
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
            assert!(!host.action_consistency_snapshot().unwrap().evidence.crossed());
            if bypass {
                assert!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::NotAdmitted(_)));
                assert!(host.request_action(71).is_err());
                assert!(host.mediation_snapshot().unwrap().accepted.is_none());
                assert_eq!(host.inspect().executions, 0);
                assert_eq!(host.inspect().control.ledger.charged, 0);
                assert_eq!(host.inspect().payload, b"initial");
            } else {
                let (action, input, automatic, request) = prepared(&mut host, 71, 101, &last);
                let revision = host.revision();
                let human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
                host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
                assert_eq!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2))
                    .unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
                host.reconcile(host.revision(), 1).unwrap();
                assert_eq!(host.inspect().executions, 1);
                assert_eq!(host.inspect().payload, b"OK");
                assert_eq!(host.delivery_mediation(1).unwrap().unwrap().graph(), &graph(2, false));
            }
        }
    }
}
