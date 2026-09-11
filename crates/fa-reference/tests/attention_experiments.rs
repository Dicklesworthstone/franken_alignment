//! Nonlinear downstream calculations for sparse KV interventions.
//! The fixed-query counterfactual is not a real resumed transformer rollout.

#[path = "support/attention_fixture.rs"]
mod support;

use support::{budget, contract, image, query};
use fa_reference::action::consequence::activation::tensor::kv::attention::{AttentionBudget, AttentionContract, AttentionMask};
use fa_reference::action::consequence::activation::tensor::kv::experiment::{
    KvBranchKind, KvCell, KvEdit, KvEditScope, KvExperiment, KvExperimentLimits, KvSide,
};
use fa_reference::action::consequence::activation::tensor::kv::image::KvImage;
use fa_reference::Error;

fn experiment(c: &AttentionContract, keys: &[f32], values: &[f32]) -> KvExperiment {
    let source = image(c, 0, keys, values);
    let count = source.len();
    KvExperiment::new(1, source, KvEditScope { first_position: 0, token_count: count, keys: true, values: true },
        KvExperimentLimits { branches: 16, retained_edits: 32, resolution_depth: 4 }).unwrap()
}

fn edit(side: KvSide, position: u64, head: usize, from: f32, to: f32) -> KvEdit {
    KvEdit { cell: KvCell { side, position, head, channel: 0 }, expected_bits: from.to_bits(), replacement_bits: to.to_bits() }
}

#[test]
fn key_intervention_changes_softmax_and_downstream_value_not_only_a_probe_score() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let mut experiments = experiment(&c, &[0.0, 0.0], &[0.0, 10.0]);
    let baseline = experiments.baseline();
    let changed = experiments.fork(1, &baseline, &[edit(KvSide::Key, 1, 0, 0.0, 2.0)]).unwrap();
    let compared = changed.compare_attention(&baseline, &c, &query(&c, 1, &[1.0]), budget()).unwrap();
    assert_eq!(compared.reference_values.output(), &[5.0]);
    let expected = 10.0 / (1.0 + (-2_f64).exp());
    assert!((compared.candidate_values.output()[0] - expected).abs() < 1e-12);
    assert!(compared.candidate_values.weights(0).unwrap()[1] > 0.88);
    assert_eq!(compared.changed_output_coordinates(), 1);
    assert!(compared.maximum_absolute_output_change() > 3.8);
    assert_eq!(baseline.bits(KvCell { side: KvSide::Key, position: 1, head: 0, channel: 0 }).unwrap(), 0_f32.to_bits());
}

#[test]
fn value_only_intervention_preserves_weights_while_changing_mixed_output() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let mut experiments = experiment(&c, &[0.0, 0.0], &[0.0, 10.0]);
    let baseline = experiments.baseline();
    let changed = experiments.fork(1, &baseline, &[edit(KvSide::Value, 1, 0, 10.0, 20.0)]).unwrap();
    let compared = changed.compare_attention(&baseline, &c, &query(&c, 1, &[1.0]), budget()).unwrap();
    assert_eq!(compared.reference_values.weights(0).unwrap(), compared.candidate_values.weights(0).unwrap());
    assert_eq!(compared.reference_values.output(), &[5.0]);
    assert_eq!(compared.candidate_values.output(), &[10.0]);
}

#[test]
fn unchanged_controls_and_undone_interventions_recover_the_original_calculation() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let mut experiments = experiment(&c, &[0.0, 1.0], &[2.0, 6.0]);
    let baseline = experiments.baseline();
    let control = experiments.fork(1, &baseline, &[]).unwrap();
    let changed = experiments.fork(2, &baseline, &[edit(KvSide::Value, 1, 0, 6.0, 12.0)]).unwrap();
    let undone = experiments.fork(3, &changed, &[edit(KvSide::Value, 1, 0, 12.0, 6.0)]).unwrap();
    let q = query(&c, 1, &[1.0]);
    for branch in [&control, &undone] {
        let compared = branch.compare_attention(&baseline, &c, &q, budget()).unwrap();
        assert_eq!(compared.reference_values, compared.candidate_values);
        assert_eq!(compared.changed_output_coordinates(), 0);
        assert_eq!(compared.maximum_absolute_output_change(), 0.0);
    }
    assert_eq!(experiments.branch_count(), 3);
    assert_eq!(experiments.retained_edit_count(), 2);
}

#[test]
fn future_and_outside_window_interventions_do_not_change_the_query() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let mut experiments = experiment(&c, &[0.0; 3], &[2.0, 6.0, 8.0]);
    let baseline = experiments.baseline();
    let future = experiments.fork(1, &baseline, &[
        edit(KvSide::Key, 2, 0, 0.0, f32::MAX), edit(KvSide::Value, 2, 0, 8.0, f32::MAX),
    ]).unwrap();
    let compared = future.compare_attention(&baseline, &c, &query(&c, 1, &[1.0]), budget()).unwrap();
    assert_eq!(compared.reference_values, compared.candidate_values);
    assert_eq!(compared.candidate_values.output(), &[4.0]);
    let old = experiments.fork(2, &baseline, &[edit(KvSide::Value, 0, 0, 2.0, 100.0)]).unwrap();
    let window = contract(1, 1, 1, 1, AttentionMask::Sliding { tokens: 2 });
    let compared = old.compare_attention(&baseline, &window, &query(&window, 2, &[1.0]), budget()).unwrap();
    assert_eq!(compared.reference_values, compared.candidate_values);
    assert_eq!(compared.candidate_values.output(), &[7.0]);
    let full = old.compare_attention(&baseline, &c, &query(&c, 2, &[1.0]), budget()).unwrap();
    assert_eq!(full.changed_output_coordinates(), 1);
}

#[test]
fn one_cache_head_intervention_changes_only_its_group_of_query_heads() {
    let c = contract(2, 4, 1, 1, AttentionMask::FullPrefix);
    let mut experiments = experiment(&c, &[0.0; 4], &[2.0, 20.0, 6.0, 60.0]);
    let baseline = experiments.baseline();
    let changed = experiments.fork(1, &baseline, &[edit(KvSide::Value, 1, 1, 60.0, 100.0)]).unwrap();
    let compared = changed.compare_attention(&baseline, &c, &query(&c, 1, &[0.0; 4]), budget()).unwrap();
    assert_eq!(compared.reference_values.output(), &[4.0, 4.0, 40.0, 40.0]);
    assert_eq!(compared.candidate_values.output(), &[4.0, 4.0, 60.0, 60.0]);
    assert_eq!(compared.changed_output_coordinates(), 2);
}

#[test]
fn rebase_changes_lookup_cost_without_changing_attention_or_erasing_lineage() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let mut experiments = experiment(&c, &[0.0, 1.0], &[2.0, 6.0]);
    let baseline = experiments.baseline();
    let first = experiments.fork(1, &baseline, &[edit(KvSide::Key, 1, 0, 1.0, 2.0)]).unwrap();
    let second = experiments.fork(2, &first, &[edit(KvSide::Value, 1, 0, 6.0, 12.0)]).unwrap();
    let rebased = experiments.rebase(3, &second).unwrap();
    let q = query(&c, 1, &[1.0]);
    let deep = second.compare_attention(&baseline, &c, &q, budget()).unwrap();
    let shallow = rebased.compare_attention(&baseline, &c, &q, budget()).unwrap();
    assert_eq!(deep.candidate_values, shallow.candidate_values);
    assert_eq!(deep.reference_values, shallow.reference_values);
    assert!(shallow.work.resolution_step_bound < deep.work.resolution_step_bound);
    assert_eq!(rebased.lineage().last(), Some(&(3, KvBranchKind::Rebase)));
    assert_eq!(rebased.lineage().len(), 4);
}

#[test]
fn paired_budget_includes_both_outputs_and_both_sparse_resolution_paths() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let source = image(&c, 0, &[0.0; 2], &[2.0, 6.0]);
    let q = query(&c, 1, &[0.0]);
    let single = source.replay_attention(&c, &q, budget()).unwrap();
    let mut experiments = experiment(&c, &[0.0; 2], &[2.0, 6.0]);
    let baseline = experiments.baseline();
    let changed = experiments.fork(1, &baseline, &[edit(KvSide::Value, 1, 0, 6.0, 8.0)]).unwrap();
    let pair = changed.compare_attention(&baseline, &c, &q, budget()).unwrap();
    assert_eq!(pair.work.scalar_products, 2 * single.work.scalar_products);
    assert_eq!(pair.work.workspace_bytes, 2 * single.work.workspace_bytes);
    let exact = AttentionBudget { scalar_products: pair.work.scalar_products,
        resolution_steps: pair.work.resolution_step_bound, workspace_bytes: pair.work.workspace_bytes };
    changed.compare_attention(&baseline, &c, &q, exact).unwrap();
    let before = (experiments.branch_count(), experiments.retained_edit_count(), changed.delta().unwrap());
    for insufficient in [AttentionBudget { scalar_products: single.work.scalar_products, ..exact },
        AttentionBudget { resolution_steps: exact.resolution_steps - 1, ..exact },
        AttentionBudget { workspace_bytes: single.work.workspace_bytes, ..exact }]
    { assert_eq!(changed.compare_attention(&baseline, &c, &q, insufficient).unwrap_err(), Error::Limit); }
    assert_eq!((experiments.branch_count(), experiments.retained_edit_count(), changed.delta().unwrap()), before);
}

#[test]
fn equal_descriptor_foreign_experiment_cannot_substitute_the_control_leg() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let mut first = experiment(&c, &[0.0; 2], &[2.0, 6.0]);
    let second = experiment(&c, &[0.0; 2], &[2.0, 6.0]);
    assert_eq!(first.baseline().basis().source, second.baseline().basis().source);
    let baseline = first.baseline();
    let changed = first.fork(1, &baseline, &[edit(KvSide::Value, 1, 0, 6.0, 8.0)]).unwrap();
    let q = query(&c, 1, &[0.0]);
    assert_eq!(changed.compare_attention(&second.baseline(), &c, &q, budget()).unwrap_err(), Error::Binding);
    changed.compare_attention(&baseline, &c, &q, budget()).unwrap();
}

#[test]
fn imported_bases_and_dropped_owners_preserve_the_same_numerical_experiment() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let original = image(&c, 0, &[0.0, 1.0], &[2.0, 6.0]);
    let q = query(&c, 1, &[1.0]);
    let expected = original.replay_attention(&c, &q, budget()).unwrap().values;
    let imported = KvImage::decode(&original.encode().unwrap(), original.descriptor()).unwrap();
    drop(original);
    let mut experiments = KvExperiment::new(1, imported,
        KvEditScope { first_position: 0, token_count: 2, keys: true, values: true },
        KvExperimentLimits { branches: 2, retained_edits: 2, resolution_depth: 2 }).unwrap();
    let baseline = experiments.baseline();
    let changed = experiments.fork(1, &baseline, &[edit(KvSide::Value, 1, 0, 6.0, 12.0)]).unwrap();
    drop(experiments);
    let compared = changed.compare_attention(&baseline, &c, &q, budget()).unwrap();
    assert_eq!(compared.reference_values, expected);
    assert!(compared.maximum_absolute_output_change() > 4.0);
}
