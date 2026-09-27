//! Original numerical engines, codecs and monitors; no fabricated case results.
use fa_reference::Error;
use fa_reference::action::consequence::activation::monitor::learned::{
    LearnedMonitorBudget, LearnedRefinementMonitor,
    model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor},
};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{
        DecoderBudget, DecoderIdentity, DecoderLayerWeights, DecoderModel, DecoderProfile,
        DecoderShape, MAX_DECODER_PRODUCTS,
        monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
        sampling::{
            SamplingPolicy, SamplingStart,
            monitored::{GenerationBudget, GenerationSpec, GenerationStatus, GenerationStop, GenerationTelemetryBudget},
            replay::{CheckpointLimits, GenerationCheckpoint, ReplayBudget, ReplayableGeneration,
                archive::ArchiveLimits,
                comparison::{ComparisonLimits, ComparisonStatus, verified::{ComparisonPreparation,
                    campaign::{CampaignBudget, CampaignCase, CampaignCost, CampaignStatus,
                        CaseOutcome, ComparisonCampaign, MAX_CAMPAIGN_CASES}}}},
        },
    },
    experiment::KvSide,
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy},
};
use std::collections::{BTreeMap, BTreeSet};

fn model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
            query_heads: 1, cache_heads: 1, context: 16 }, 0.00001, 10000.0).unwrap();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![0.0; 4],
        keys: vec![0.0; 4], values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}
fn policy(model: &DecoderModel, alarm: bool, count: u64) -> LearnedDecoderPolicy {
    let inference = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let training = model.recompute(11, &[0, 1], inference).unwrap().cache_image().unwrap();
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap();
    let mut taps = BTreeMap::new();
    for (layer, contract) in model.cache_profile().layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let mut probes = Vec::new();
            for id in 1..=count {
                let mut weights = vec![0.0; tensor.dimensions()];
                let threshold = if alarm && *layer == 2 && side == KvSide::Value {
                    weights[1] = 1.0; 0.5
                } else { 1.0 };
                probes.push(LinearProbe::new(id, 1, tensor.profile(), &weights, 0.0, threshold).unwrap());
            }
            taps.insert(KvTap { layer: *layer, side },
                LearnedRefinementMonitor::new(probes, LearnedMonitorBudget::default()).unwrap());
        }
    }
    let monitor = LearnedModelMonitor::new(model.cache_profile().clone(), taps, LearnedAuditBudget::default()).unwrap();
    LearnedDecoderPolicy::new(codec, monitor, LearnedStreamRetention::All,
        LearnedAuditPreparationBudget::default(), inference).unwrap()
}
fn source(model: &DecoderModel, policy: LearnedDecoderPolicy) -> ReplayableGeneration {
    let spec = GenerationSpec::new(vec![0], 3, BTreeSet::new(),
        SamplingStart { policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap();
    model.replayable_monitored_generation(21, 201, spec, policy, GenerationBudget::default(),
        GenerationTelemetryBudget::default()).unwrap()
}
fn prepare(saved: &GenerationCheckpoint, policy: &LearnedDecoderPolicy, limits: ComparisonLimits) -> ComparisonPreparation {
    saved.begin_policy_comparison(policy.clone(), ReplayBudget::default(), limits).unwrap()
}
fn case(id: u64, saved: &GenerationCheckpoint, policy: &LearnedDecoderPolicy, limits: ComparisonLimits) -> CampaignCase {
    CampaignCase::new(id, prepare(saved, policy, limits)).unwrap()
}

#[test]
fn fixed_cohort_retains_both_directions_of_disagreement_and_both_held() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let alarm = policy(&model, true, 1);
    let quiet_source = source(&model, quiet.clone());
    let alarm_source = source(&model, alarm.clone());
    let q = quiet_source.checkpoint(CheckpointLimits::default()).unwrap();
    let a = alarm_source.checkpoint(CheckpointLimits::default()).unwrap();
    let limits = ComparisonLimits::default();
    let mut campaign = ComparisonCampaign::new(vec![case(10, &q, &quiet, limits),
        case(20, &q, &alarm, limits), case(30, &a, &quiet, limits), case(40, &a, &alarm, limits)],
        CampaignBudget::default()).unwrap();
    let reserved = campaign.summary().reserved;
    let summary = campaign.run_to_completion().unwrap();
    assert_eq!(summary.status, CampaignStatus::Complete);
    assert_eq!((summary.completed, summary.pending), (4, 0));
    assert_eq!((summary.matched_stops, summary.decision_differences, summary.both_held), (1, 2, 1));
    assert_eq!(summary.declared_lineages, 1); // Four experiments are not four independent sources.
    assert_eq!(summary.reserved, reserved);
    assert!(matches!(campaign.reports()[1].outcome, CaseOutcome::DecisionDifference {
        position: 1, baseline: GenerationStatus::Generating, candidate: GenerationStatus::Held(_)}));
    assert!(matches!(campaign.reports()[2].outcome, CaseOutcome::DecisionDifference {
        position: 1, baseline: GenerationStatus::Held(_), candidate: GenerationStatus::Generating}));
    assert_eq!(quiet_source.generation().position(), 0);
    assert_eq!(alarm_source.generation().position(), 0);
    assert_eq!(campaign.run_to_completion().unwrap(), summary);
    assert_eq!(campaign.run_next(10), Err(Error::WrongState));
}

#[test]
fn successful_case_matches_independent_original_pair_and_preserves_cost_difference() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let extra = policy(&model, false, 2);
    let mut original = source(&model, quiet);
    for position in 0..3 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let old_work = original.generation().work();
    let old_sampler = original.generation().sampler_state();
    let mut control = prepare(&saved, &extra, ComparisonLimits::default());
    control.advance(usize::MAX).unwrap();
    let original_preparation = control.report();
    let mut control = control.finish().unwrap();
    assert_eq!(control.run_to_stop(), Ok(ComparisonStatus::MatchedStop(GenerationStop::TokenLimit)));
    let mut campaign = ComparisonCampaign::new(vec![case(1, &saved, &extra, ComparisonLimits::default())],
        CampaignBudget::default()).unwrap();
    let report = campaign.run_next(1).unwrap();
    assert_eq!(report.preparation, original_preparation);
    assert_eq!(report.comparison, Some(control.report()));
    let work = report.comparison.unwrap().work;
    assert!(work.candidate_telemetry.monitor_probe_coordinates > work.baseline_telemetry.monitor_probe_coordinates);
    assert_eq!(original.generation().work(), old_work);
    assert_eq!(original.generation().sampler_state(), old_sampler);
    assert_eq!(original.generation().position(), 3);
}

#[test]
fn every_aggregate_budget_has_an_exact_and_one_less_control() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone());
    for position in 0..2 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let cases = || vec![case(1, &saved, &quiet, ComparisonLimits::default()),
        case(2, &saved, &quiet, ComparisonLimits::default())];
    let required = ComparisonCampaign::required_cost(&cases()).unwrap();
    assert_eq!((required.cases, required.replay_positions, required.paired_positions), (2, 4, 8));
    let single_pair = model.estimate(0, 4).unwrap().scalar_products().unwrap() * 2;
    assert_eq!(required.decoder_products, 2 * (saved.work().reserved_decoder_products + single_pair));
    assert_eq!(required.vocabulary_scores, 2 * (saved.work().reserved_vocabulary_scores + 18));
    for field in 0..5 {
        let mut low = required;
        match field {
            0 => low.cases -= 1,
            1 => low.replay_positions -= 1,
            2 => low.paired_positions -= 1,
            3 => low.decoder_products -= 1,
            4 => low.vocabulary_scores -= 1,
            _ => unreachable!(),
        }
        assert!(matches!(ComparisonCampaign::new(cases(), CampaignBudget(low)), Err(Error::Limit)), "field {field}");
    }
    let mut exact = ComparisonCampaign::new(cases(), CampaignBudget(required)).unwrap();
    assert_eq!(exact.run_to_completion().unwrap().matched_stops, 2);
    assert_eq!(original.generation().position(), 2);
}

#[test]
fn exhaustion_and_comparison_failure_do_not_omit_cases_or_prevent_sibling_execution() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone());
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let mut campaign = ComparisonCampaign::new(vec![
        case(1, &saved, &quiet, ComparisonLimits { positions: 0, ..ComparisonLimits::default() }),
        case(2, &saved, &quiet, ComparisonLimits { positions: 1, ..ComparisonLimits::default() }),
        case(3, &saved, &quiet, ComparisonLimits { state_bytes: 0, ..ComparisonLimits::default() }),
        case(4, &saved, &quiet, ComparisonLimits::default()),
    ], CampaignBudget::default()).unwrap();
    let initial = campaign.summary();
    assert_eq!(campaign.run_next(2), Err(Error::Stale));
    assert_eq!(campaign.summary(), initial);
    assert_eq!(campaign.run_next(1).unwrap().outcome, CaseOutcome::Exhausted);
    let after = campaign.summary();
    assert_eq!(campaign.run_next(1), Err(Error::Stale));
    assert_eq!(campaign.summary(), after);
    let summary = campaign.run_to_completion().unwrap();
    assert_eq!((summary.completed, summary.pending, summary.exhausted, summary.comparison_failures,
        summary.matched_stops), (4, 0, 2, 1, 1));
    let failure = campaign.reports()[2];
    assert_eq!(failure.outcome, CaseOutcome::ComparisonFailed(Error::Limit));
    assert!(failure.preparation.verification_complete);
    assert_eq!(failure.comparison.unwrap().work.baseline.admitted_tokens, 1);
    assert_eq!(summary.reserved, initial.reserved);
}

#[test]
fn an_advanced_verifier_cannot_be_adopted_after_seeing_baseline_results() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone());
    for position in 0..3 { original.advance(position).unwrap(); }
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let mut late = prepare(&saved, &quiet, ComparisonLimits::default());
    late.advance(1).unwrap();
    assert!(matches!(CampaignCase::new(1, late), Err(Error::WrongState)));
    let mut zero = prepare(&saved, &quiet, ComparisonLimits::default());
    zero.advance(0).unwrap();
    let mut campaign = ComparisonCampaign::new(vec![CampaignCase::new(1, zero).unwrap()], CampaignBudget::default()).unwrap();
    assert_eq!(campaign.run_to_completion().unwrap().matched_stops, 1);
}

#[test]
fn roster_ids_and_cardinality_are_validated_before_work() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let original = source(&model, quiet.clone());
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    assert!(matches!(CampaignCase::new(0, prepare(&saved, &quiet, ComparisonLimits::default())), Err(Error::InvalidInput)));
    assert!(matches!(ComparisonCampaign::new(vec![], CampaignBudget::default()), Err(Error::InvalidInput)));
    assert!(matches!(ComparisonCampaign::new(vec![case(1, &saved, &quiet, ComparisonLimits::default()),
        case(1, &saved, &quiet, ComparisonLimits::default())], CampaignBudget::default()), Err(Error::Duplicate)));
    let too_many = (1..=MAX_CAMPAIGN_CASES + 1).map(|id|
        case(id as u64, &saved, &quiet, ComparisonLimits { positions: 0, ..ComparisonLimits::default() })).collect();
    assert!(matches!(ComparisonCampaign::new(too_many, CampaignBudget::default()), Err(Error::Limit)));
    let excessive = CampaignBudget(CampaignCost { cases: MAX_CAMPAIGN_CASES + 1, ..CampaignBudget::default().0 });
    assert!(matches!(ComparisonCampaign::new(vec![case(1, &saved, &quiet, ComparisonLimits::default())], excessive), Err(Error::Limit)));
    assert_eq!(original.generation().work().admitted_tokens, 0);
}

#[test]
fn archive_preparations_use_the_same_original_verifier_and_never_mutate_the_intended_owner() {
    let model = model();
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, quiet.clone());
    original.run_to_stop().unwrap();
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let bytes = saved.encode_archive(ArchiveLimits::default()).unwrap();
    let intended = source(&model, quiet.clone());
    let archive = intended.decode_archive(&bytes, ArchiveLimits::default()).unwrap();
    let preparation = archive.begin_policy_comparison(quiet,
        ReplayBudget::default(), ComparisonLimits::default()).unwrap();
    let mut campaign = ComparisonCampaign::new(vec![CampaignCase::new(1, preparation).unwrap()], CampaignBudget::default()).unwrap();
    let row = campaign.run_next(1).unwrap();
    assert!(row.preparation.verification_complete);
    assert_eq!(row.preparation.compared_positions, 4);
    assert_eq!(row.comparison.unwrap().baseline_replay.unwrap().recomputation, original.generation().work());
    assert_eq!(row.outcome, CaseOutcome::MatchedStop(GenerationStop::TokenLimit));
    assert_eq!(intended.generation().position(), 0);
}

#[test]
fn a_later_live_hold_is_neither_cleared_nor_reclassified_by_experiments() {
    let model = model();
    let alarm = policy(&model, true, 1);
    let quiet = policy(&model, false, 1);
    let mut original = source(&model, alarm);
    let saved = original.checkpoint(CheckpointLimits::default()).unwrap();
    let held = original.run_to_stop().unwrap();
    let spend = original.generation().work();
    assert!(matches!(held, GenerationStatus::Held(_)));
    let mut campaign = ComparisonCampaign::new(vec![case(1, &saved, &quiet, ComparisonLimits::default())], CampaignBudget::default()).unwrap();
    assert_eq!(campaign.run_to_completion().unwrap().decision_differences, 1);
    assert_eq!(original.generation().status(), held);
    assert_eq!(original.generation().work(), spend);
    assert_eq!(original.advance(1).err(), Some(Error::WrongState));
}

#[path = "learned_comparison_campaign/stepped.rs"]
mod stepped;
