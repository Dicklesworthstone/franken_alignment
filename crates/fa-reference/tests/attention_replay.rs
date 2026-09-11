//! Causal attention from owned KV images and actual checked query captures.
//! Independent formulas and capture mutations, not a native serving-host claim.

#[path = "support/attention_fixture.rs"]
mod support;

use support::{budget, contract, image, query};
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorContract, TensorLayout, TokenSelection,
};
use fa_reference::action::consequence::activation::tensor::kv::attention::{
    AttentionBudget, AttentionContract, AttentionMask, AttentionNumerics, MAX_ATTENTION_HEADS,
};
use fa_reference::action::consequence::activation::tensor::kv::image::KvImage;
use fa_reference::Error;

fn close(left: f64, right: f64) {
    assert!((left - right).abs() <= 1e-12 * right.abs().max(1.0), "{left} != {right}");
}

#[test]
fn zero_query_has_uniform_weights_and_exact_hand_calculated_value_mean() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let cached = image(&c, 0, &[-100.0, 200.0], &[2.0, 6.0]);
    let q = query(&c, 1, &[0.0]);
    let result = cached.replay_attention(&c, &q, budget()).unwrap();
    assert_eq!(result.values.weights(0).unwrap(), &[0.5, 0.5]);
    assert_eq!(result.values.output(), &[4.0]);
    assert_eq!(result.work.scalar_products, 4);
    assert_eq!(result.work.exponentials, 2);
    assert_eq!(result.work.workspace_bytes, 24);
    assert_eq!(result.values.zero_weights(), 0);
    assert_eq!(result.contract.numerics(), AttentionNumerics::Binary64SequentialStableSoftmaxV1);
    assert_eq!(result.query.source().identity(), q.source().identity());
    assert_eq!(result.values.head_output(1), Err(Error::Missing));
}

#[test]
fn two_token_attention_matches_an_independent_sigmoid_identity() {
    let c = contract(1, 1, 1, 2, AttentionMask::FullPrefix);
    let cached = image(&c, 0, &[0.0, 1.0], &[0.0, 4.0, 4.0, 0.0]);
    let result = cached.replay_attention(&c, &query(&c, 1, &[1.0]), budget()).unwrap();
    let p = 1.0 / (1.0 + (-1_f64).exp());
    close(result.values.output()[0], 4.0 * p);
    close(result.values.output()[1], 4.0 * (1.0 - p));
    close(result.values.weights(0).unwrap().iter().sum(), 1.0);
    let reversed = cached.replay_attention(&c, &query(&c, 1, &[-1.0]), budget()).unwrap();
    close(reversed.values.output()[0], result.values.output()[1]);
    close(reversed.values.output()[1], result.values.output()[0]);
}

#[test]
fn grouped_queries_use_cache_head_groups_without_duplicating_cache_values() {
    let c = contract(2, 4, 1, 1, AttentionMask::FullPrefix);
    let cached = image(&c, 0, &[0.0; 4], &[2.0, 20.0, 6.0, 60.0]);
    let result = cached.replay_attention(&c, &query(&c, 1, &[0.0; 4]), budget()).unwrap();
    assert_eq!(result.values.output(), &[4.0, 4.0, 40.0, 40.0]);
    assert_eq!(cached.descriptor().normalized_values().unwrap(), 8);
    let mqa = contract(1, 4, 1, 1, AttentionMask::FullPrefix);
    let shared = image(&mqa, 0, &[0.0; 2], &[2.0, 6.0]);
    assert_eq!(shared.replay_attention(&mqa, &query(&mqa, 1, &[0.0; 4]), budget())
        .unwrap().values.output(), &[4.0; 4]);
    assert_eq!(shared.descriptor().normalized_values().unwrap(), 4);
}

#[test]
fn future_keys_and_values_never_enter_an_earlier_query() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let cached = image(&c, 0, &[0.0, 0.0, f32::MAX], &[2.0, 6.0, f32::MAX]);
    let first = cached.replay_attention(&c, &query(&c, 0, &[1.0]), budget()).unwrap();
    assert_eq!(first.values.output(), &[2.0]);
    assert_eq!(first.values.positions(), 1);
    let second = cached.replay_attention(&c, &query(&c, 1, &[1.0]), budget()).unwrap();
    assert_eq!(second.values.output(), &[4.0]);
    assert_eq!(second.values.weights(0).unwrap(), &[0.5, 0.5]);
}

#[test]
fn an_unobserved_prefix_requires_an_explicit_sufficient_sliding_window() {
    let full = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let cached = image(&full, 3, &[0.0, 0.0], &[2.0, 6.0]);
    let q = query(&full, 4, &[0.0]);
    assert_eq!(cached.replay_attention(&full, &q, budget()).unwrap_err(), Error::Incomplete);
    let sliding = contract(1, 1, 1, 1, AttentionMask::Sliding { tokens: 2 });
    let result = cached.replay_attention(&sliding, &q, budget()).unwrap();
    assert_eq!(result.values.first_position(), 3);
    assert_eq!(result.values.output(), &[4.0]);
    let insufficient = contract(1, 1, 1, 1, AttentionMask::Sliding { tokens: 3 });
    assert_eq!(cached.replay_attention(&insufficient, &q, budget()).unwrap_err(), Error::Incomplete);
    let empty = image(&full, 0, &[], &[]);
    assert_eq!(empty.replay_attention(&full, &query(&full, 0, &[0.0]), budget()).unwrap_err(), Error::Missing);
}

#[test]
fn stable_softmax_handles_large_scores_and_reports_zero_numerical_weights() {
    let original = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let c = AttentionContract::new(9, 1, original.queries().clone(), original.cache().clone(),
        AttentionMask::FullPrefix, 1_000_000.0).unwrap();
    let cached = image(&c, 0, &[f32::MAX, -f32::MAX], &[3.0, 7.0]);
    let result = cached.replay_attention(&c, &query(&c, 1, &[f32::MAX]), budget()).unwrap();
    assert_eq!(result.values.weights(0).unwrap(), &[1.0, 0.0]);
    assert_eq!(result.values.output(), &[3.0]);
    assert_eq!(result.values.zero_weights(), 1);
    let tied = image(&c, 0, &[f32::MAX; 2], &[3.0, 7.0]);
    assert_eq!(tied.replay_attention(&c, &query(&c, 1, &[f32::MAX]), budget())
        .unwrap().values.output(), &[5.0]);
}

#[test]
fn same_shaped_foreign_query_contract_and_temporal_identity_are_rejected() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let cached = image(&c, 0, &[0.0; 2], &[2.0, 6.0]);
    let wrong_queries = TensorContract::new(
        fa_reference::action::consequence::activation::CaptureProfile { tap: 9, ..c.queries().profile() },
        ScalarEncoding::Binary32, ByteOrder::Little, 1, 1).unwrap();
    let other = AttentionContract::new(9, 1, wrong_queries, c.cache().clone(), AttentionMask::FullPrefix, 1.0).unwrap();
    assert_eq!(cached.replay_attention(&c, &query(&other, 1, &[0.0]), budget()).unwrap_err(), Error::Binding);
    let layout = TensorLayout::new([2, 1, 1, 1], [4, 0, 0, 0], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let bytes = [0_u8; 8];
    for (batch, stream, sequence) in [(1, 7, 2), (0, 8, 2), (0, 7, 1)] {
        let q = c.queries().capture(HostTensor {
            identity: BufferIdentity { object: 3, generation: 1 }, layout: &layout, bytes: &bytes,
        }, TokenSelection { batch, token: 0, first_position: 1, stream, sequence }).unwrap();
        assert_eq!(cached.replay_attention(&c, &q, budget()).unwrap_err(), Error::Binding);
    }
}

#[test]
fn exact_cost_limits_succeed_and_each_one_less_refuses_without_changing_sources() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let cached = image(&c, 0, &[0.0; 2], &[2.0, 6.0]);
    let q = query(&c, 1, &[0.0]);
    let before = cached.encode().unwrap();
    let measured = cached.replay_attention(&c, &q, budget()).unwrap().work;
    let exact = AttentionBudget { scalar_products: measured.scalar_products,
        resolution_steps: measured.resolution_step_bound, workspace_bytes: measured.workspace_bytes };
    assert_eq!(cached.replay_attention(&c, &q, exact).unwrap().values.output(), &[4.0]);
    for invalid in [AttentionBudget { scalar_products: exact.scalar_products - 1, ..exact },
        AttentionBudget { resolution_steps: exact.resolution_steps - 1, ..exact },
        AttentionBudget { workspace_bytes: exact.workspace_bytes - 1, ..exact }]
    { assert_eq!(cached.replay_attention(&c, &q, invalid).unwrap_err(), Error::Limit); }
    assert_eq!(cached.encode().unwrap(), before);
}

#[test]
fn portable_image_replay_matches_the_original_captured_values() {
    let c = contract(1, 2, 2, 1, AttentionMask::FullPrefix);
    let original = image(&c, 0, &[1.0, 0.0, 0.0, 1.0], &[2.0, 6.0]);
    let descriptor = original.descriptor().clone();
    let bytes = original.encode().unwrap();
    let q = query(&c, 1, &[1.0, 0.0, 0.0, 1.0]);
    let expected = original.replay_attention(&c, &q, budget()).unwrap();
    drop(original);
    let imported = KvImage::decode(&bytes, &descriptor).unwrap();
    assert_eq!(imported.replay_attention(&c, &q, budget()).unwrap().values, expected.values);
}

#[test]
fn registration_rejects_invalid_masks_scales_and_head_contracts() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, 1_000_001.0] {
        assert!(AttentionContract::new(9, 1, c.queries().clone(), c.cache().clone(), AttentionMask::FullPrefix, scale).is_err());
    }
    for tokens in [0, 4097] {
        assert!(AttentionContract::new(9, 1, c.queries().clone(), c.cache().clone(), AttentionMask::Sliding { tokens }, 1.0).is_err());
    }
    let mismatched = TensorContract::new(c.queries().profile(), ScalarEncoding::Binary32, ByteOrder::Little, 1, 2).unwrap();
    assert_eq!(AttentionContract::new(9, 1, mismatched, c.cache().clone(), AttentionMask::FullPrefix, 1.0).unwrap_err(), Error::Binding);
    let many = MAX_ATTENTION_HEADS + 1;
    let large_cache = fa_reference::action::consequence::activation::tensor::kv::KvContract::new(
        c.cache().keys().clone(), c.cache().values().clone(), many).unwrap();
    let large_queries = TensorContract::new(c.queries().profile(), ScalarEncoding::Binary32, ByteOrder::Little, many, 1).unwrap();
    assert_eq!(AttentionContract::new(9, 1, large_queries, large_cache, AttentionMask::FullPrefix, 1.0).unwrap_err(), Error::Limit);
}
