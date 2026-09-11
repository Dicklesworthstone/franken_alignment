//! Incremental paired KV observation over real borrowed fixture bytes.

use fa_reference::action::consequence::activation::{CaptureProfile, ProgressiveFrame};
use fa_reference::action::consequence::activation::tensor::{BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorCapture, TensorContract, TensorLayout};
use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvBudget, KvCapture, KvContract};
use fa_reference::Error;

fn tensor_contract(tap: u64, heads: usize, channels: usize) -> TensorContract {
    TensorContract::new(CaptureProfile { tenant: 1, model: 2, model_generation: 3,
        tap, layout_generation: 1 }, ScalarEncoding::Binary32, ByteOrder::Little, heads, channels).unwrap()
}
fn host<'a>(layout: &'a TensorLayout, bytes: &'a [u8], object: u64, generation: u64) -> HostTensor<'a> {
    HostTensor { identity: BufferIdentity { object, generation }, layout, bytes }
}
fn values(capture: &TensorCapture) -> Vec<f32> {
    let source = capture.source();
    let checked = source.verify_block(&source.encode_initial(23).unwrap()).unwrap();
    ProgressiveFrame::from_initial(&checked).unwrap().exact_values().unwrap()
}
fn layout(heads: usize, channels: usize) -> TensorLayout {
    TensorLayout::new([2, 4, heads, channels], [4 * heads * channels * 4,
        heads * channels * 4, channels * 4, 4], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap()
}
fn data(layout: &TensorLayout, bias: f32) -> Vec<u8> {
    let shape = layout.shape(); let strides = layout.byte_strides();
    let mut bytes = vec![0; layout.byte_range().end];
    for b in 0..shape[0] { for t in 0..shape[1] { for h in 0..shape[2] { for c in 0..shape[3] {
        let offset = layout.byte_range().start + b * strides[0] + t * strides[1] + h * strides[2] + c * strides[3];
        let value = bias + (100 * b + 10 * t + 3 * h + c) as f32;
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }}}}
    bytes
}
fn cache(heads: usize, key_channels: usize, value_channels: usize, query_heads: usize, budget: KvBudget) -> KvCapture {
    let contract = KvContract::new(tensor_contract(10, heads, key_channels),
        tensor_contract(11, heads, value_channels), query_heads).unwrap();
    KvCapture::new(contract, 6, 1, 40, 10, budget).unwrap()
}
fn request<'a>(keys: HostTensor<'a>, values: HostTensor<'a>, first: usize, count: usize) -> KvAppend<'a> {
    KvAppend { keys, values, first_token: first, token_count: count,
        buffer_first_position: 40, first_sequence: 10 + first as u64 }
}

#[test]
fn gqa_prefill_and_decode_copy_only_new_positions_and_keep_prior_values_immutable() {
    let keys_layout = layout(2, 3);
    let values_layout = TensorLayout::new([2, 4, 2, 1], [32, 4, 16, 0], 0,
        ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let mut keys = data(&keys_layout, 0.0);
    let mut vals = data(&values_layout, 1000.0);
    let mut cache = cache(2, 3, 1, 8, KvBudget { positions: 4, normalized_values: 32 });
    assert_eq!((0..8).map(|q| cache.contract().cache_head_for(q).unwrap()).collect::<Vec<_>>(), vec![0, 0, 0, 0, 1, 1, 1, 1]);
    let first = cache.append(0, request(host(&keys_layout, &keys, 1, 1), host(&values_layout, &vals, 2, 1), 0, 2)).unwrap();
    assert_eq!(first.source_bytes_read, 64);
    assert_eq!(first.normalized_values, 16);
    let old_keys = values(cache.token(40).unwrap().key());
    let old_vals = values(cache.token(40).unwrap().value());
    // Recycle only the already captured positions; new token bytes remain ready.
    keys[96..144].fill(0);
    vals[32..40].fill(0); vals[48..56].fill(0);
    let next = cache.append(1, request(host(&keys_layout, &keys, 1, 2), host(&values_layout, &vals, 2, 2), 2, 1)).unwrap();
    assert_eq!(next.source_bytes_read, 32);
    assert_eq!(cache.source_bytes_read(), 96);
    assert_eq!(cache.normalized_values(), 24);
    assert_eq!(cache.next_position(), 43);
    assert_eq!(cache.next_sequence(), 13);
    assert_eq!(values(cache.token(40).unwrap().key()), old_keys);
    assert_eq!(values(cache.token(40).unwrap().value()), old_vals);
    assert_eq!(values(cache.token(42).unwrap().key()), vec![120.0, 121.0, 122.0, 123.0, 124.0, 125.0]);
    assert_eq!(values(cache.token(42).unwrap().value()), vec![1120.0, 1123.0]);
    assert_eq!(cache.token(42).unwrap().key().source().identity().sequence, 12);
    assert_eq!(cache.token(43).unwrap_err(), Error::Missing);
}

#[test]
fn mqa_retains_one_cache_head_instead_of_duplicating_for_each_query_head() {
    let layout = layout(1, 2);
    let keys = data(&layout, 0.0); let vals = data(&layout, 1000.0);
    let mut cache = cache(1, 2, 2, 16, KvBudget { positions: 1, normalized_values: 4 });
    for query in 0..16 { assert_eq!(cache.contract().cache_head_for(query).unwrap(), 0); }
    assert_eq!(cache.contract().cache_head_for(16), Err(Error::InvalidInput));
    cache.append(0, request(host(&layout, &keys, 1, 1), host(&layout, &vals, 2, 1), 0, 1)).unwrap();
    assert_eq!(cache.normalized_values(), 4);
    assert_eq!(cache.token(40).unwrap().key().source().dimensions(), 2);
    assert_eq!(cache.source_bytes_read(), 16);
}

#[test]
fn a_nonfinite_value_after_valid_keys_cannot_publish_half_a_pair_or_window() {
    let layout = layout(1, 1);
    let keys = data(&layout, 0.0); let mut vals = data(&layout, 1000.0);
    let mut cache = cache(1, 1, 1, 4, KvBudget { positions: 4, normalized_values: 8 });
    cache.append(0, request(host(&layout, &keys, 1, 1), host(&layout, &vals, 2, 1), 0, 1)).unwrap();
    let before = (cache.revision(), cache.next_position(), cache.next_sequence(), cache.len(), cache.source_bytes_read());
    vals[24..28].copy_from_slice(&f32::NAN.to_le_bytes());
    // The first new pair is valid; the second pair fails after its key capture.
    assert_eq!(cache.append(1, request(host(&layout, &keys, 1, 1), host(&layout, &vals, 2, 1), 1, 2)), Err(Error::InvalidInput));
    assert_eq!((cache.revision(), cache.next_position(), cache.next_sequence(), cache.len(), cache.source_bytes_read()), before);
    assert_eq!(cache.token(41).unwrap_err(), Error::Missing);
    vals[24..28].copy_from_slice(&1120.0_f32.to_le_bytes());
    cache.append(1, request(host(&layout, &keys, 1, 1), host(&layout, &vals, 2, 1), 1, 2)).unwrap();
    assert_eq!(cache.len(), 3);
}

#[test]
fn gaps_duplicate_positions_and_out_of_order_sequences_never_advance_the_prefix() {
    let layout = layout(1, 1); let bytes = data(&layout, 0.0);
    let mut cache = cache(1, 1, 1, 1, KvBudget { positions: 4, normalized_values: 8 });
    let base = request(host(&layout, &bytes, 1, 1), host(&layout, &bytes, 2, 1), 0, 1);
    cache.append(0, base).unwrap();
    assert_eq!(cache.append(0, base), Err(Error::Stale));
    let mut wrong = base; wrong.first_sequence = 11;
    assert_eq!(cache.append(1, wrong), Err(Error::Stale));
    wrong.first_token = 2;
    assert_eq!(cache.append(1, wrong), Err(Error::Stale));
    wrong.first_token = 1; wrong.first_sequence = 12;
    assert_eq!(cache.append(1, wrong), Err(Error::Stale));
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.next_position(), 41);
    wrong.first_sequence = 11;
    cache.append(1, wrong).unwrap();
    assert_eq!(cache.next_position(), 42);
}

#[test]
fn shared_buffer_views_require_matching_incarnation_and_disjoint_declared_spans() {
    let k = TensorLayout::new([1; 4], [0; 4], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let v = TensorLayout::new([1; 4], [0; 4], 4, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let bytes: Vec<_> = [1.0_f32, 2.0].into_iter().flat_map(f32::to_le_bytes).collect();
    let contract = KvContract::new(tensor_contract(10, 1, 1), tensor_contract(11, 1, 1), 1).unwrap();
    let mut cache = KvCapture::new(contract, 6, 0, 40, 10, KvBudget { positions: 1, normalized_values: 2 }).unwrap();
    let alias = request(host(&k, &bytes, 1, 1), host(&k, &bytes, 1, 1), 0, 1);
    assert_eq!(cache.append(0, alias), Err(Error::Binding));
    let mixed = request(host(&k, &bytes, 1, 1), host(&v, &bytes, 1, 2), 0, 1);
    assert_eq!(cache.append(0, mixed), Err(Error::Binding));
    assert!(cache.is_empty());
    cache.append(0, request(host(&k, &bytes, 1, 1), host(&v, &bytes, 1, 1), 0, 1)).unwrap();
    assert_eq!(values(cache.token(40).unwrap().key()), vec![1.0]);
    assert_eq!(values(cache.token(40).unwrap().value()), vec![2.0]);
}

#[test]
fn budgets_generations_and_cache_head_contracts_are_checked_before_publication() {
    assert_eq!(KvContract::new(tensor_contract(10, 2, 1), tensor_contract(11, 2, 1), 3).unwrap_err(), Error::InvalidInput);
    assert_eq!(KvContract::new(tensor_contract(10, 2, 1), tensor_contract(11, 1, 1), 4).unwrap_err(), Error::Binding);
    let layout = layout(1, 1); let bytes = data(&layout, 0.0);
    let mut cache = cache(1, 1, 1, 1, KvBudget { positions: 4, normalized_values: 4 });
    cache.append(0, request(host(&layout, &bytes, 1, 2), host(&layout, &bytes, 2, 2), 0, 1)).unwrap();
    assert_eq!(cache.append(1, request(host(&layout, &bytes, 1, 1), host(&layout, &bytes, 2, 2), 1, 1)), Err(Error::Stale));
    assert_eq!(cache.append(1, request(host(&layout, &bytes, 1, 2), host(&layout, &bytes, 2, 2), 1, 2)), Err(Error::Limit));
    assert_eq!(cache.len(), 1);
    cache.append(1, request(host(&layout, &bytes, 1, 3), host(&layout, &bytes, 2, 3), 1, 1)).unwrap();
    assert_eq!(cache.normalized_values(), 4);
}

#[test]
fn mismatched_batch_token_extents_and_truncated_value_buffer_refuse_atomically() {
    let keys_layout = layout(1, 1); let keys = data(&keys_layout, 0.0);
    let values_layout = TensorLayout::new([2, 3, 1, 1], [12, 4, 0, 0], 0,
        ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let vals = data(&values_layout, 1000.0);
    let mut cache = cache(1, 1, 1, 1, KvBudget { positions: 4, normalized_values: 8 });
    assert_eq!(cache.append(0, request(host(&keys_layout, &keys, 1, 1), host(&values_layout, &vals, 2, 1), 0, 1)), Err(Error::Binding));
    assert_eq!(cache.append(0, request(host(&keys_layout, &keys, 1, 1), host(&keys_layout, &keys[..31], 2, 1), 0, 1)), Err(Error::Incomplete));
    assert!(cache.is_empty());
    assert_eq!(cache.revision(), 0);
    assert!(cache.receipts().is_empty());
    cache.append(0, request(host(&keys_layout, &keys, 1, 1), host(&keys_layout, &keys, 2, 1), 0, 1)).unwrap();
}

#[test]
fn page_rollover_uses_absolute_positions_without_copying_the_previous_page() {
    let first = TensorLayout::new([1, 2, 1, 1], [0, 4, 0, 0], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let next = TensorLayout::new([1; 4], [0; 4], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let mut page: Vec<_> = [1.0_f32, 2.0].into_iter().flat_map(f32::to_le_bytes).collect();
    let contract = KvContract::new(tensor_contract(10, 1, 1), tensor_contract(11, 1, 1), 4).unwrap();
    let mut cache = KvCapture::new(contract, 6, 0, 0, 1, KvBudget { positions: 3, normalized_values: 6 }).unwrap();
    cache.append(0, KvAppend { keys: host(&first, &page, 1, 1), values: host(&first, &page, 2, 1),
        first_token: 0, token_count: 2, buffer_first_position: 0, first_sequence: 1 }).unwrap();
    page.fill(0);
    let tail = 3.0_f32.to_le_bytes();
    let receipt = cache.append(1, KvAppend { keys: host(&next, &tail, 1, 2), values: host(&next, &tail, 2, 2),
        first_token: 0, token_count: 1, buffer_first_position: 2, first_sequence: 3 }).unwrap();
    assert_eq!(receipt.source_bytes_read, 8);
    assert_eq!(values(cache.token(0).unwrap().key()), vec![1.0]);
    assert_eq!(values(cache.token(1).unwrap().key()), vec![2.0]);
    assert_eq!(values(cache.token(2).unwrap().key()), vec![3.0]);
    assert_eq!(cache.token(2).unwrap().key().receipt().selection().token, 0);
    assert_eq!(cache.token(2).unwrap().key().source().identity().position, 2);
    assert_eq!(cache.source_bytes_read(), 24);
}
