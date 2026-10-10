//! Public original-generation regressions for opt-in forecast source successors.
//! The tiny model and neutral probability table are controls, not calibration.
#![cfg(unix)]

#[path = "support/learned_text_model.rs"]
#[allow(dead_code)]
mod numerical;
#[path = "forecast_reset_successors/support.rs"]
mod support;
use support::*;

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::{FrameIdentity,
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::{experiment::KvSide, model::MAX_MODEL_KV_VALUES,
        decoder::{DecoderModel, monitoring::restart::KvRestartBudget,
            sampling::{SampledToken, monitored::GenerationEvent}}},
};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::persistent::{
    FileDeliveryProfile, JournalError, JournalLimits,
    observed::{FileOversight, FileOversightProfile,
        consistency::{FileConsistencyConfig, FileConsistencyObserver, FileConsistencyParameters,
            learned::FileLearnedConsistencyConfig},
        containment::FileResetRequest,
        decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus,
            checkpoint::{FileLearnedCheckpoint, FileLearnedResetIntent}},
        guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
            predictive::{FilePredictiveRequirements, FilePredictiveRoles,
                owned::FileOwnedPredictiveRequirements}},
    },
};
use fa_reference::action::consequence::gate::{ReviewBinding,
    containment::{ActorState, RestartGrade, RestartProfile,
        session::policy::{Policy, Predicate}},
};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract,
    decoder_monitoring::LearnedDecoderBindingLimits, human::HumanReviewPolicy,
    learned_source::{LearnedEvidenceLimits, text::LearnedTextConfig},
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn completed_pending_reset_allows_raw_and_owned_forecasts_then_original_sampling() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Directory::new();
        let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
        let source = source(&model);
        let expected_samples = original_samples(&model, source.clone());
        let config = recipe(&model, source.clone(), mode, true);
        let (mut host, old_observer) = owner(&root, &config);
        let saved = checkpoint_before_prompt_end(&mut host);
        let intent = begin_reset(&mut host, &saved, source.policy.allowance());
        let required = requirements(&host, &config);
        let cut = host.revision();
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let disk = root.bytes();
        drop(host);

        let (mut host, roles) = recover(&root, &config, mode, &required, &intent).unwrap();
        assert_ne!(root.bytes(), disk);
        assert_eq!(host.revision(), cut + 2, "one original reset and one recovery fence");
        let reset = host.learned_reset_result(900).unwrap().unwrap();
        assert!(reset.control.restored);
        assert_eq!(reset.resumed_stream, Some(22));
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        assert!(host.pending_learned_reset().unwrap().is_none());
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready());
        assert_eq!(host.check_learned_checkpoint(&saved), Err(Error::Binding.into()));
        assert!(mode.forecast(&mut host, &old_observer, 71).is_err());

        resume(&mut host);
        let revision = host.revision();
        assert!(mode.forecast(&mut host, &roles.consistency_observer, 71).is_err(),
            "a reset receipt and new role do not replace accepted source evidence");
        assert_eq!(host.revision(), revision);
        let accepted = step(&mut host);
        assert!(accepted.sample().is_none());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 2);
        assert!(accepted.audit().source().descriptor().layers().values()
            .all(|layer| layer.stream == 22));
        let forecast = mode.forecast(&mut host, &roles.consistency_observer, 71).unwrap().unwrap();
        assert_eq!(forecast.frame.stream, 22);
        assert_eq!(forecast.frame.sequence, 2);
        assert_eq!(forecast.probability, BinaryForecast::new(32_768, 32_768).unwrap());
        assert_eq!(host.pending_forecast_request().unwrap(), Some(71));
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert_eq!(finish_samples(&mut host), expected_samples,
            "accepted restart retains the original sampler state and numerical path");
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
        submit_generated(&mut host, 71);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        assert!(host.pending_forecast_request().unwrap().is_none());
        assert_eq!(host.inspect().executions, 0);
        assert!(config.follows_forecast_reset_successors());
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let required = requirements(&host, &config);
        let cut = host.revision();
        drop(host);
        let (host, _) = recover(&root, &config, mode, &required, &intent).unwrap();
        assert_eq!(host.revision(), cut + 1, "completed-reset retry only fences");
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        assert!(host.pending_forecast_request().unwrap().is_none());
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    }
}

#[test]
fn legacy_raw_and_owned_recipes_keep_their_acknowledged_stream_refusal_on_replay() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Directory::new();
        let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
        let source = source(&model);
        let config = recipe(&model, source.clone(), mode, false);
        assert!(!config.follows_forecast_reset_successors());
        let (mut host, _) = owner(&root, &config);
        let saved = checkpoint_before_prompt_end(&mut host);
        let intent = begin_reset(&mut host, &saved, source.policy.allowance());
        let required = requirements(&host, &config);
        drop(host);
        let (mut host, roles) = recover(&root, &config, mode, &required, &intent).unwrap();
        resume(&mut host);
        step(&mut host);
        let revision = host.revision();
        assert_eq!(mode.forecast(&mut host, &roles.consistency_observer, 71).unwrap().err(),
            Some(Error::Binding));
        assert_eq!(host.revision(), revision + 1, "retain the original acknowledged refusal");
        assert!(host.pending_forecast_request().unwrap().is_none());
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        assert_eq!(evidence.samples(), 0);
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        let required = requirements(&host, &config);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        drop(host);
        // The old raw event has no saved outcome witness. Replaying its original
        // versioned recipe must still compute Binding, never create a forecast.
        let (host, _) = recover(&root, &config, mode, &required, &intent).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert!(host.pending_forecast_request().unwrap().is_none());
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    }
}

#[test]
fn reset_successor_selection_cannot_be_added_or_removed_at_recovery() {
    for mode in [Mode::Raw, Mode::Owned] {
        for successors in [false, true] {
            let root = Directory::new();
            let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
            let source = source(&model);
            let config = recipe(&model, source.clone(), mode, successors);
            let changed = recipe(&model, source.clone(), mode, !successors);
            assert_ne!(config, changed);
            assert_eq!(config.required_pre_output_forecast(), changed.required_pre_output_forecast());
            assert_eq!(config.required_owned_pre_output_forecast(), changed.required_owned_pre_output_forecast());
            let (mut host, _) = owner(&root, &config);
            let saved = checkpoint_before_prompt_end(&mut host);
            let intent = begin_reset(&mut host, &saved, source.policy.allowance());
            let required = requirements(&host, &config);
            let disk = root.bytes();
            drop(host);
            assert_eq!(recover(&root, &changed, mode, &required, &intent).err(),
                Some(JournalError::Contract(Error::Binding)));
            assert_eq!(root.bytes(), disk, "a different recipe cannot replace canonical history");
            let (host, _) = recover(&root, &config, mode, &required, &intent).unwrap();
            assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
            assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        }
    }
}

#[test]
fn pending_forecast_and_spent_acquisition_survive_reset_successor_selection() {
    for mode in [Mode::Raw, Mode::Owned] {
        for sampled in [false, true] {
            let root = Directory::new();
            let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
            let source = source(&model);
            let config = recipe(&model, source.clone(), mode, true);
            let (mut host, observer) = owner(&root, &config);
            let saved = checkpoint_before_prompt_end(&mut host);
            mode.forecast(&mut host, &observer, 71).unwrap().unwrap();
            let acquired = host.learned_action_consistency_snapshot().ok();
            let evidence = host.action_consistency_snapshot().unwrap();
            if sampled { assert!(step(&mut host).sample().is_some()); }
            let spent = host.learned_generation_inspection().unwrap().numerical.cumulative_work;
            let intent = begin_reset(&mut host, &saved, source.policy.allowance());
            let required = requirements(&host, &config);
            drop(host);
            let (mut host, roles) = recover(&root, &config, mode, &required, &intent).unwrap();
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
            assert!(mode.forecast(&mut host, &roles.consistency_observer, 72).is_err());
            assert_eq!(host.revision(), revision);
            assert_eq!(root.bytes(), disk);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, n);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn exhausted_owned_lifetime_is_not_refilled_by_the_original_reset() {
    let root = Directory::new();
    let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
    let source = source(&model);
    let lifetime = LearnedMonitorBudget { probe_coordinates: 0, ..LearnedMonitorBudget::default() };
    let config = recipe_with_lifetime(&model, source.clone(), Mode::Owned, true, lifetime);
    let (mut host, observer) = owner(&root, &config);
    let saved = checkpoint_before_prompt_end(&mut host);
    assert!(Mode::Owned.forecast(&mut host, &observer, 71).unwrap().is_err());
    let acquired = host.learned_action_consistency_snapshot().unwrap();
    assert!(acquired.consistency.coverage_lost);
    assert!(acquired.retained_source_bytes > 0);
    assert!(host.pending_forecast_request().unwrap().is_none());
    let intent = begin_reset(&mut host, &saved, source.policy.allowance());
    let required = requirements(&host, &config);
    drop(host);
    let (mut host, roles) = recover(&root, &config, Mode::Owned, &required, &intent).unwrap();
    let after = host.learned_action_consistency_snapshot().unwrap();
    assert!(after.consistency.coverage_lost);
    assert_eq!(after.consistency.evidence, acquired.consistency.evidence);
    assert_eq!(after.work, acquired.work);
    assert_eq!(after.retained_source_bytes, acquired.retained_source_bytes);
    assert_eq!(after.has_unreported_work, acquired.has_unreported_work);
    assert_eq!(config.required_owned_pre_output_forecast().unwrap().lifetime_budget(), lifetime);
    resume(&mut host);
    let n = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision();
    let disk = root.bytes();
    assert!(host.advance_learned_generation(revision, n.actor_revision, n.position).is_err());
    assert!(Mode::Owned.forecast(&mut host, &roles.consistency_observer, 72).is_err());
    assert_eq!(root.bytes(), disk);
    assert_eq!(host.revision(), revision);
    assert_eq!(host.learned_action_consistency_snapshot().unwrap().work, acquired.work);
    assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
}

#[test]
fn answered_forecast_does_not_allow_reset_to_erase_sampled_output_history() {
    for mode in [Mode::Raw, Mode::Owned] {
        let root = Directory::new();
        let model = numerical::model(&[b'O' as u32, b'K' as u32, numerical::END]);
        let source = source(&model);
        let config = recipe(&model, source.clone(), mode, true);
        let (mut host, observer) = owner(&root, &config);
        let saved = checkpoint_before_prompt_end(&mut host);
        mode.forecast(&mut host, &observer, 71).unwrap().unwrap();
        assert_eq!(finish_samples(&mut host).len(), 3);
        submit_generated(&mut host, 71);
        assert!(host.pending_forecast_request().unwrap().is_none());
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        assert_eq!(evidence.samples(), 1);
        let acquired = host.learned_action_consistency_snapshot().ok();
        let intent = begin_reset(&mut host, &saved, source.policy.allowance());
        let required = requirements(&host, &config);
        drop(host);
        let (mut host, roles) = recover(&root, &config, mode, &required, &intent).unwrap();
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost,
            "this isolates sampled-history refusal from the pending-forecast latch");
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert!(host.pending_forecast_request().unwrap().is_none());
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
        let revision = host.revision();
        let disk = root.bytes();
        assert_eq!(host.advance_learned_generation(revision, n.actor_revision, n.position).err(),
            Some(JournalError::Contract(Error::WrongState)));
        assert!(mode.forecast(&mut host, &roles.consistency_observer, 72).is_err());
        assert_eq!(host.revision(), revision);
        assert_eq!(root.bytes(), disk);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert_eq!(host.inspect().executions, 0);
    }
}
