//! Atomic multi-layer capture through the public host-tensor APIs.
#[path = "support/model_kv_fixture.rs"]
mod support;

use support::*;
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorLayout,
};
use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvBudget, KvCapture};
use fa_reference::action::consequence::activation::tensor::kv::model::{
    ModelKvBudget, ModelKvCapture, ModelKvImage, ModelKvProfile,
};
use fa_reference::Error;
use std::collections::BTreeMap;

#[test]
fn every_layer_advances_together_and_old_snapshots_survive_appends() {
    let mut capture = capture();
    let buffers = Buffers::new();
    let receipt = capture.append(0, buffers.requests(0, 1, 1)).unwrap();
    assert_eq!(receipt.layers.len(), 2);
    assert_eq!(receipt.normalized_values, 8);
    assert_eq!(receipt.source_bytes_read, 24);
    assert_eq!(capture.next_position(), 1);
    assert_eq!(capture.next_sequence(), 2);
    let old = capture.snapshot(1).unwrap();
    let old_bytes = old.layer(10).unwrap().encode().unwrap();
    capture.append(1, buffers.requests(1, 1, 2)).unwrap();
    for id in [10, 20] {
        assert_eq!(capture.layer(id).unwrap().revision(), 2);
        assert_eq!(capture.layer(id).unwrap().len(), 2);
        assert_eq!(capture.layer(id).unwrap().next_sequence(), 3);
    }
    assert_eq!(capture.revision(), 2);
    assert_eq!(capture.normalized_values(), 16);
    assert_eq!(capture.source_bytes_read(), 48);
    assert_eq!(old.len(), 1);
    assert_eq!(old.layer(10).unwrap().encode().unwrap(), old_bytes);
    let newest = capture.snapshot(2).unwrap();
    drop(capture);
    drop(buffers);
    assert_eq!(newest.len(), 2);
    assert_eq!(old.layer(10).unwrap().encode().unwrap(), old_bytes);
}

#[test]
fn omitted_or_unregistered_layers_are_not_a_complete_capture() {
    let mut capture = capture();
    let buffers = Buffers::new();
    let mut missing = buffers.requests(0, 2, 1);
    missing.remove(&20);
    assert_eq!(capture.append(0, missing), Err(Error::Binding));
    let mut extra = buffers.requests(0, 2, 1);
    extra.insert(30, extra[&20]);
    assert_eq!(capture.append(0, extra), Err(Error::Binding));
    assert!(capture.is_empty());
    assert_eq!(capture.revision(), 0);
    capture.append(0, buffers.requests(0, 2, 1)).unwrap();
    assert_eq!(capture.len(), 2);
}

#[test]
fn late_nonfinite_value_publishes_no_layer_frontier_or_generation_floor() {
    let mut capture = capture();
    let mut buffers = Buffers::new();
    buffers.values_b[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
    let mut requests = buffers.requests(0, 2, 1);
    for request in requests.values_mut() {
        request.keys.identity.generation = 99;
        request.values.identity.generation = 99;
    }
    assert_eq!(capture.append(0, requests), Err(Error::InvalidInput));
    for id in [10, 20] {
        let layer = capture.layer(id).unwrap();
        assert_eq!(layer.revision(), 0);
        assert!(layer.is_empty());
        assert!(layer.receipts().is_empty());
        assert_eq!(layer.source_bytes_read(), 0);
    }
    assert_eq!(capture.revision(), 0);
    assert_eq!(capture.normalized_values(), 0);
    assert!(capture.receipts().is_empty());
    buffers.values_b[12..16].copy_from_slice(&16_f32.to_le_bytes());
    capture.append(0, buffers.requests(0, 2, 1)).unwrap();
    assert_eq!(capture.source_bytes_read(), 48);
}

#[test]
fn mismatched_layer_windows_sequences_and_stale_predecessors_refuse() {
    let mut capture = capture();
    let buffers = Buffers::new();
    for mutation in 0..3 {
        let mut requests = buffers.requests(0, 2, 1);
        let last = requests.get_mut(&20).unwrap();
        match mutation {
            0 => last.first_sequence += 1,
            1 => last.buffer_first_position += 1,
            _ => last.token_count -= 1,
        }
        assert_eq!(capture.append(0, requests), Err(Error::Stale));
        assert!(capture.is_empty());
    }
    capture.append(0, buffers.requests(0, 1, 1)).unwrap();
    assert_eq!(capture.append(0, buffers.requests(1, 1, 2)), Err(Error::Stale));
    assert_eq!(capture.len(), 1);
    capture.append(1, buffers.requests(1, 1, 2)).unwrap();
}

#[test]
fn buffer_generation_floor_follows_storage_when_it_moves_between_layers() {
    let mut capture = capture();
    let buffers = Buffers::new();
    let mut first = buffers.requests(0, 1, 1);
    first.get_mut(&10).unwrap().keys.identity.generation = 8;
    capture.append(0, first).unwrap();
    let mut next = buffers.requests(1, 1, 2);
    next.get_mut(&10).unwrap().keys.identity.object = 9;
    next.get_mut(&20).unwrap().keys.identity = BufferIdentity { object: 1, generation: 7 };
    assert_eq!(capture.append(1, next), Err(Error::Stale));
    assert_eq!(capture.len(), 1);
    let mut next = buffers.requests(1, 1, 2);
    next.get_mut(&10).unwrap().keys.identity.object = 9;
    next.get_mut(&20).unwrap().keys.identity = BufferIdentity { object: 1, generation: 8 };
    capture.append(1, next).unwrap();
    assert_eq!(capture.len(), 2);
}

#[test]
fn aggregate_budget_cannot_be_evaded_by_splitting_data_across_layers() {
    let buffers = Buffers::new();
    let mut too_small = ModelKvCapture::new(profile(), 11, 0, 0, 1,
        ModelKvBudget { positions: 2, normalized_values: 15 }).unwrap();
    // Each layer alone needs only eight values; their combined sixteen refuse.
    assert_eq!(too_small.append(0, buffers.requests(0, 2, 1)), Err(Error::Limit));
    assert!(too_small.is_empty());
    let mut exact = ModelKvCapture::new(profile(), 11, 0, 0, 1,
        ModelKvBudget { positions: 2, normalized_values: 16 }).unwrap();
    exact.append(0, buffers.requests(0, 2, 1)).unwrap();
    assert_eq!(exact.normalized_values(), 16);
}

#[test]
fn shared_backing_requires_disjoint_layer_spans_and_one_incarnation() {
    let buffers = Buffers::new();
    let bytes: Vec<u8> = [&buffers.keys_a, &buffers.values_a, &buffers.keys_b, &buffers.values_b]
        .into_iter().flat_map(|part| part.iter().copied()).collect();
    let make = |original: &TensorLayout, offset| TensorLayout::new(original.shape(),
        original.byte_strides(), offset, original.encoding(), original.byte_order()).unwrap();
    let ka = make(&buffers.ka, 0);
    let va = make(&buffers.va, 8);
    let kb = make(&buffers.kb, 16);
    let vb = make(&buffers.vb, 32);
    let mut requests = buffers.requests(0, 2, 1);
    for (id, key_layout, value_layout) in [(10, &ka, &va), (20, &kb, &vb)] {
        let request = requests.get_mut(&id).unwrap();
        request.keys = HostTensor { identity: BufferIdentity { object: 77, generation: 1 }, layout: key_layout, bytes: &bytes };
        request.values = HostTensor { identity: BufferIdentity { object: 77, generation: 1 }, layout: value_layout, bytes: &bytes };
    }
    let mut good = capture();
    good.append(0, requests.clone()).unwrap();
    assert_eq!(good.len(), 2);
    let overlapping = make(&buffers.kb, 4);
    requests.get_mut(&20).unwrap().keys.layout = &overlapping;
    let mut bad = capture();
    assert_eq!(bad.append(0, requests), Err(Error::Binding));
    assert!(bad.is_empty());
}

#[test]
fn independently_saved_layers_must_match_the_entire_registered_cut() {
    let mut capture = capture();
    let buffers = Buffers::new();
    capture.append(0, buffers.requests(0, 1, 1)).unwrap();
    let early = capture.layer(20).unwrap().snapshot(1).unwrap();
    capture.append(1, buffers.requests(1, 1, 2)).unwrap();
    let mixed = BTreeMap::from([(10, capture.layer(10).unwrap().snapshot(2).unwrap()), (20, early)]);
    assert_eq!(ModelKvImage::from_layers(profile(), mixed).unwrap_err(), Error::Binding);
    let complete: BTreeMap<_, _> = [10, 20].into_iter().map(|id| (id, capture.layer(id).unwrap().snapshot(2).unwrap())).collect();
    assert_eq!(ModelKvImage::from_layers(profile(), complete.clone()).unwrap().len(), 2);
    let mut missing = complete.clone();
    missing.remove(&10);
    assert_eq!(ModelKvImage::from_layers(profile(), missing).unwrap_err(), Error::Binding);
    let mut swapped = complete;
    let left = swapped.remove(&10).unwrap();
    let right = swapped.remove(&20).unwrap();
    swapped.insert(10, right);
    swapped.insert(20, left);
    assert_eq!(ModelKvImage::from_layers(profile(), swapped).unwrap_err(), Error::Binding);
}

#[test]
fn a_prepared_single_layer_can_be_dropped_without_publishing() {
    let profile = profile();
    let mut layer = KvCapture::new(profile.layers()[&10].clone(), 11, 0, 0, 1,
        KvBudget { positions: 2, normalized_values: 8 }).unwrap();
    let buffers = Buffers::new();
    let request: KvAppend<'_> = buffers.requests(0, 2, 1)[&10];
    let staged = layer.prepare_append(0, request).unwrap();
    assert_eq!(staged.receipt().token_count, 2);
    drop(staged);
    assert!(layer.is_empty());
    assert_eq!(layer.revision(), 0);
    assert_eq!(layer.next_position(), 0);
    assert_eq!(layer.source_bytes_read(), 0);
    let receipt = layer.prepare_append(0, request).unwrap().commit();
    assert_eq!(receipt.source_bytes_read, 16);
    assert_eq!(layer.len(), 2);
}

#[test]
fn registration_refuses_duplicate_taps_and_unrelated_models() {
    let original = profile();
    let duplicate = BTreeMap::from([(10, original.layers()[&10].clone()), (20, original.layers()[&10].clone())]);
    assert_eq!(ModelKvProfile::new(9, 1, duplicate), Err(Error::Duplicate));
    assert_eq!(ModelKvProfile::new(9, 1, BTreeMap::new()), Err(Error::InvalidInput));
    use fa_reference::action::consequence::activation::tensor::TensorContract;
    use fa_reference::action::consequence::activation::tensor::kv::KvContract;
    let changed = |tap| TensorContract::new(
        fa_reference::action::consequence::activation::CaptureProfile {
            model_generation: 99, tap, ..original.layers()[&10].keys().profile()
        }, ScalarEncoding::Binary32, ByteOrder::Little, 1, 1,
    ).unwrap();
    let other = KvContract::new(changed(100), changed(101), 1).unwrap();
    assert_eq!(ModelKvProfile::new(9, 1, BTreeMap::from([
        (10, original.layers()[&10].clone()), (20, other),
    ])), Err(Error::Binding));
}
