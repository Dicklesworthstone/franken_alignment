//! Mandatory all-layer learned monitoring on captured and forward-computed KV.
#[path = "support/learned_kv.rs"] mod tensor_fixture;
#[path = "support/decoder_fixture.rs"] mod decoder_fixture;
use fa_reference::action::consequence::activation::ProgressiveFrame;
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome,
    learned::{LearnedMonitorBudget, LearnedRefinementMonitor,
        model::{KvTap, LearnedAuditBudget, LearnedAuditPreparationBudget, LearnedModelMonitor}}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::probe::learned::{CheckedKvBudget, CheckedLearnedKv, KvGroup, KvRow, ResidualRetention};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderCheckpoint,
    DecoderIdentity, DecoderModel, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::{ModelKvImage, ModelKvProfile,
    learned::{LearnedKvCodec, LearnedKvPolicy, FitBudget, CompressionBudget}};
use fa_reference::Error;
use std::collections::BTreeMap;

fn fitted(training: ModelKvImage) -> LearnedKvCodec {
    LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(), &BTreeMap::from([(101, training)]), FitBudget::default()).unwrap()
}
fn checked(codec: &LearnedKvCodec, source: &ModelKvImage, retention: ResidualRetention) -> CheckedLearnedKv {
    let (image, _) = codec.evaluate_held_out(201, source, CompressionBudget::default()).unwrap();
    CheckedLearnedKv::new(image, source, retention, CheckedKvBudget::default()).unwrap()
}
fn roster(profile: &ModelKvProfile, selected: Option<(KvTap, Vec<f32>, f32)>) -> BTreeMap<KvTap, LearnedRefinementMonitor> {
    let mut monitors = BTreeMap::new();
    for (layer, contract) in profile.layers() {
        for (side, tensor) in [(KvSide::Key, contract.keys()), (KvSide::Value, contract.values())] {
            let tap = KvTap { layer: *layer, side };
            let (weights, threshold) = match &selected {
                Some((which, weights, threshold)) if *which == tap => (weights.clone(), *threshold),
                _ => (vec![0.0; tensor.dimensions()], 1.0),
            };
            let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
            monitors.insert(tap, LearnedRefinementMonitor::new(vec![probe], LearnedMonitorBudget::default()).unwrap());
        }
    }
    monitors
}
fn monitor(profile: &ModelKvProfile, selected: Option<(KvTap, Vec<f32>, f32)>, budget: LearnedAuditBudget) -> LearnedModelMonitor {
    LearnedModelMonitor::new(profile.clone(), roster(profile, selected), budget).unwrap()
}
fn model() -> DecoderModel {
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 3, hidden: 2, intermediate: 2, layers: 2,
            query_heads: 1, cache_heads: 1, context: 64 }, 0.00001, 10000.0).unwrap();
    let mut layers = decoder_fixture::zero_layers(&profile);
    for layer in &mut layers { layer.values = vec![1.0, 0.0, 0.0, 1.0]; }
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], layers, vec![1.0; 2], vec![0.0; 6]).unwrap()
}
fn inference() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn setup(tokens: &[u32]) -> (DecoderModel, LearnedKvCodec, DecoderCheckpoint) {
    let model = model();
    let training = model.recompute(11, &[0, 1], inference()).unwrap().cache_image().unwrap();
    let codec = fitted(training);
    let checkpoint = model.recompute(21, tokens, inference()).unwrap().checkpoint().unwrap();
    (model, codec, checkpoint)
}
fn late_tap() -> KvTap { KvTap { layer: 2, side: KvSide::Value } }

#[test]
fn complete_quiet_prices_the_shared_base_once_and_keeps_every_row_in_the_denominator() {
    let source = tensor_fixture::line(2, 3, 4); let codec = fitted(tensor_fixture::line(1, 3, 5));
    let evidence = checked(&codec, &source, ResidualRetention::All);
    let budget = LearnedAuditBudget { rows: 8, monitoring: LearnedMonitorBudget {
        encoded_bytes: evidence.report().base_encoded_bytes, probe_coordinates: 24,
        reconstruction_products: 0, materialized_values: 0, refinements: 0 } };
    let report = monitor(source.profile(), None, budget).analyze(&evidence).unwrap();
    assert!(report.complete_quiet()); assert_eq!(report.planned_rows(), 8);
    assert_eq!(report.quiet_rows(), 8); assert_eq!(report.examined_rows(), 8); assert_eq!(report.unexamined_rows(), 0);
    assert_eq!(report.first_position(), 0); assert_eq!(report.end_position(), 4);
    assert_eq!(report.work().encoded_bytes, evidence.report().base_encoded_bytes);
    assert!(report.source().report().total_encoded_bytes > report.work().encoded_bytes);
    assert_eq!(report.work().probe_coordinates, 24); assert_eq!(report.work().refinements, 0);
    for (index, row) in report.rows().iter().enumerate() {
        assert_eq!(row.row().position, (index / 2) as u64);
        assert_eq!(row.row().side, if index % 2 == 0 { KvSide::Key } else { KvSide::Value });
        assert_eq!(row.work().encoded_bytes, evidence.report().base_encoded_bytes);
    }
}

#[test]
fn an_alarm_in_the_last_forward_computed_layer_and_position_is_not_hidden_by_earlier_quiet_rows() {
    let (_model, codec, checkpoint) = setup(&[0, 0, 2]);
    let frozen = monitor(checkpoint.cache().profile(), Some((late_tap(), vec![0.0, 1.0], 0.5)), LearnedAuditBudget::default());
    let before = checkpoint.cache().encode().unwrap();
    let audit = checkpoint.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All,
        LearnedAuditPreparationBudget::default()).unwrap();
    let report = audit.monitoring(); let last = KvRow { layer: 2, side: KvSide::Value, position: 2 };
    assert_eq!(report.outcome(), MonitorOutcome::Alarm); assert!(!report.complete_quiet());
    assert_eq!(report.planned_rows(), 12); assert_eq!(report.quiet_rows(), 11);
    assert_eq!(report.examined_rows(), 12); assert_eq!(report.unexamined_rows(), 0);
    assert_eq!(report.blocked_row(), Some(last)); assert_eq!(report.work().refinements, 1);
    let row = report.rows().last().unwrap();
    assert_eq!(row.steps().last().unwrap().refinement.unwrap().group, KvGroup { row: last, head: 0 });
    let source = checkpoint.cache().layer(2).unwrap().token(2).unwrap().value();
    let block = source.verify_block(&source.encode_initial(23).unwrap()).unwrap();
    let detector = LinearProbe::new(1, 1, source.identity().profile, &[0.0, 1.0], 0.0, 0.5).unwrap();
    let exact = detector.evaluate(&ProgressiveFrame::from_initial(&block).unwrap()).unwrap();
    assert_eq!(row.steps().last().unwrap().observations[0].exact_score(), Some(&exact.interval().lower));
    assert_eq!(checkpoint.cache().encode().unwrap(), before);
    assert!(!audit.compression().training_source_overlap);
}

#[test]
fn missing_residual_and_exhausted_global_refinement_budget_remain_distinct_holds() {
    let (_model, codec, checkpoint) = setup(&[0, 0, 2]);
    for missing in [false, true] {
        let mut budget = LearnedAuditBudget::default(); if !missing { budget.monitoring.refinements = 0; }
        let frozen = monitor(checkpoint.cache().profile(), Some((late_tap(), vec![0.0, 1.0], 0.5)), budget);
        let audit = checkpoint.audit_learned_cache(201, &codec, &frozen,
            if missing { ResidualRetention::None } else { ResidualRetention::All }, LearnedAuditPreparationBudget::default()).unwrap();
        let report = audit.monitoring();
        assert_eq!(report.outcome(), if missing { MonitorOutcome::Unresolved } else { MonitorOutcome::BudgetExhausted });
        assert_eq!(report.quiet_rows(), 11); assert!(!report.complete_quiet());
        assert_eq!(report.work().refinements, 0); assert_eq!(report.work().materialized_values, 0);
        assert_eq!(report.rows().last().unwrap().steps().len(), 1);
    }
}

#[test]
fn complete_roster_profile_and_capacity_are_admitted_before_a_partial_scan_can_claim_quiet() {
    let source = tensor_fixture::line(2, 3, 4); let codec = fitted(tensor_fixture::line(1, 3, 5));
    let evidence = checked(&codec, &source, ResidualRetention::None);
    let mut missing = roster(source.profile(), None); missing.remove(&KvTap { layer: 1, side: KvSide::Value });
    assert!(matches!(LearnedModelMonitor::new(source.profile().clone(), missing, LearnedAuditBudget::default()), Err(Error::Binding)));
    let mut wrong = roster(source.profile(), None);
    let value = wrong.remove(&KvTap { layer: 1, side: KvSide::Value }).unwrap();
    wrong.insert(KvTap { layer: 2, side: KvSide::Value }, value);
    assert!(matches!(LearnedModelMonitor::new(source.profile().clone(), wrong, LearnedAuditBudget::default()), Err(Error::Binding)));
    let report = monitor(source.profile(), None, LearnedAuditBudget { rows: 7, ..LearnedAuditBudget::default() }).analyze(&evidence).unwrap();
    assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted); assert_eq!(report.planned_rows(), 8);
    assert!(report.rows().is_empty()); assert_eq!(report.unexamined_rows(), 8); assert_eq!(report.work().encoded_bytes, 0);
    let mut budget = LearnedAuditBudget::default(); budget.monitoring.probe_coordinates = 21;
    let report = monitor(source.profile(), None, budget).analyze(&evidence).unwrap();
    assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted); assert_eq!(report.quiet_rows(), 7);
    assert_eq!(report.examined_rows(), 7); assert_eq!(report.unexamined_rows(), 1); assert!(!report.complete_quiet());
    assert!(report.rows().last().unwrap().steps().is_empty());
    assert_eq!(report.blocked_row(), Some(KvRow { layer: 1, side: KvSide::Value, position: 3 }));
}

#[test]
fn each_position_spends_the_same_global_refinement_allowance_instead_of_resetting_it() {
    let (_model, codec, checkpoint) = setup(&[2, 2, 2]);
    let selected = Some((late_tap(), vec![0.0, -1.0], 0.5));
    let mut limited = LearnedAuditBudget::default(); limited.monitoring.refinements = 1;
    let frozen = monitor(checkpoint.cache().profile(), selected.clone(), limited);
    let audit = checkpoint.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All, LearnedAuditPreparationBudget::default()).unwrap();
    let report = audit.monitoring();
    assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted); assert_eq!(report.quiet_rows(), 7);
    assert_eq!(report.examined_rows(), 8); assert_eq!(report.unexamined_rows(), 4); assert_eq!(report.work().refinements, 1);
    assert_eq!(report.blocked_row(), Some(KvRow { layer: 2, side: KvSide::Value, position: 1 }));
    let frozen = monitor(checkpoint.cache().profile(), selected, LearnedAuditBudget::default());
    let audit = checkpoint.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All, LearnedAuditPreparationBudget::default()).unwrap();
    assert!(audit.monitoring().complete_quiet()); assert_eq!(audit.monitoring().work().refinements, 3);
    assert_eq!(audit.monitoring().work().materialized_values, 6);
    assert_eq!(audit.monitoring().quiet_rows(), 12);
}

#[test]
fn preparation_budgets_and_declared_held_out_identity_remain_enforced_without_modifying_the_checkpoint() {
    let (model, codec, checkpoint) = setup(&[0, 0, 2]);
    let frozen = monitor(checkpoint.cache().profile(), None, LearnedAuditBudget::default());
    let before = checkpoint.cache().encode().unwrap();
    assert_eq!(checkpoint.audit_learned_cache(101, &codec, &frozen, ResidualRetention::All, LearnedAuditPreparationBudget::default()).unwrap_err(), Error::Duplicate);
    let same_stream = model.recompute(11, &[0, 0, 2], inference()).unwrap().checkpoint().unwrap();
    assert_eq!(same_stream.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All, LearnedAuditPreparationBudget::default()).unwrap_err(), Error::Duplicate);
    let audit = checkpoint.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All, LearnedAuditPreparationBudget::default()).unwrap();
    let report = audit.monitoring().source().report();
    let mut insufficient = LearnedAuditPreparationBudget::default();
    insufficient.source_check.encoded_bytes = report.total_encoded_bytes - 1;
    assert_eq!(checkpoint.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All, insufficient).unwrap_err(), Error::Limit);
    let mut insufficient = LearnedAuditPreparationBudget::default();
    insufficient.compression.work_units = audit.compression().work_units_reserved - 1;
    assert_eq!(checkpoint.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All, insufficient).unwrap_err(), Error::Limit);
    assert_eq!(checkpoint.cache().encode().unwrap(), before);
    let empty = model.session(99).unwrap().checkpoint().unwrap();
    assert_eq!(empty.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All, LearnedAuditPreparationBudget::default()).unwrap_err(), Error::Incomplete);
}

#[test]
fn completed_audit_keeps_original_evidence_after_decoder_and_training_owners_are_dropped() {
    let (model, codec, checkpoint) = setup(&[0, 0, 2]);
    let frozen = monitor(checkpoint.cache().profile(), Some((late_tap(), vec![0.0, 1.0], 0.5)), LearnedAuditBudget::default());
    let audit = checkpoint.audit_learned_cache(201, &codec, &frozen, ResidualRetention::All, LearnedAuditPreparationBudget::default()).unwrap();
    let bytes = audit.monitoring().source().encode().unwrap();
    drop(frozen); drop(checkpoint); drop(codec); drop(model);
    assert_eq!(audit.monitoring().source().encode().unwrap(), bytes);
    assert_eq!(audit.monitoring().outcome(), MonitorOutcome::Alarm);
    assert_eq!(audit.monitoring().monitor().taps().len(), 4);
    let row = audit.monitoring().rows().last().unwrap();
    let group = KvGroup { row: row.row(), head: 0 };
    let interval = row.view().interval(group, 1).unwrap();
    assert_eq!(interval[0], interval[1]); assert!(interval[0] > 0.5);
}
