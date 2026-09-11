//! Sparse comparison checked through actual paired CPU writes and recapture.
//! Recaptured edited buffers remain synthetic fixture data, not model output.

#[path = "support/attention_fixture.rs"]
mod support;

use support::{budget, contract, image, query};
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorLayout,
};
use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvBudget, KvCapture};
use fa_reference::action::consequence::activation::tensor::kv::attention::AttentionMask;
use fa_reference::action::consequence::activation::tensor::kv::experiment::{
    KvCell, KvEdit, KvEditScope, KvExperiment, KvExperimentLimits, KvSide,
};
use fa_reference::action::consequence::activation::tensor::kv::restore::{HostTensorMut, KvDestination, KvRestoreWindow};

#[test]
fn paired_buffer_restoration_and_recapture_reproduce_both_attention_results() {
    let c = contract(1, 1, 1, 1, AttentionMask::FullPrefix);
    let original = image(&c, 0, &[0.0, 0.0], &[2.0, 6.0]);
    let original_bytes = original.encode().unwrap();
    let mut experiments = KvExperiment::new(1, original.clone(),
        KvEditScope { first_position: 0, token_count: 2, keys: true, values: true },
        KvExperimentLimits { branches: 2, retained_edits: 4, resolution_depth: 2 }).unwrap();
    let baseline = experiments.baseline();
    let changed = experiments.fork(1, &baseline, &[
        KvEdit { cell: KvCell { side: KvSide::Key, position: 1, head: 0, channel: 0 },
            expected_bits: 0_f32.to_bits(), replacement_bits: 2_f32.to_bits() },
        KvEdit { cell: KvCell { side: KvSide::Value, position: 1, head: 0, channel: 0 },
            expected_bits: 6_f32.to_bits(), replacement_bits: 12_f32.to_bits() },
    ]).unwrap();
    let q = query(&c, 1, &[1.0]);
    let compared = changed.compare_attention(&baseline, &c, &q, budget()).unwrap();
    let layout = TensorLayout::new([1, 2, 1, 1], [8, 4, 4, 4], 0,
        ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let mut left_keys = [0xcc; 8];
    let mut left_values = [0xcc; 8];
    let mut right_keys = [0xcc; 8];
    let mut right_values = [0xcc; 8];
    let window = KvRestoreWindow { first_position: 0, token_count: 2, batch: 0,
        first_token: 0, buffer_first_position: 0 };
    let restored = changed.prepare_twin_restore(&baseline,
        KvDestination::Separate {
            keys: HostTensorMut { identity: BufferIdentity { object: 10, generation: 1 }, layout: &layout, bytes: &mut left_keys },
            values: HostTensorMut { identity: BufferIdentity { object: 11, generation: 1 }, layout: &layout, bytes: &mut left_values },
        },
        KvDestination::Separate {
            keys: HostTensorMut { identity: BufferIdentity { object: 12, generation: 1 }, layout: &layout, bytes: &mut right_keys },
            values: HostTensorMut { identity: BufferIdentity { object: 13, generation: 1 }, layout: &layout, bytes: &mut right_values },
        }, window).unwrap().commit();
    assert_eq!(restored.reference.restoration.bytes_written, 16);
    assert_eq!(restored.candidate.restoration.bytes_written, 16);
    assert_ne!(left_keys, right_keys);
    assert_ne!(left_values, right_values);
    for (keys, values, expected, object) in [
        (&left_keys, &left_values, &compared.reference_values, 10),
        (&right_keys, &right_values, &compared.candidate_values, 12),
    ] {
        let mut captured = KvCapture::new(c.cache().clone(), 7, 0, 0, 1,
            KvBudget { positions: 2, normalized_values: 4 }).unwrap();
        captured.append(0, KvAppend {
            keys: HostTensor { identity: BufferIdentity { object, generation: 1 }, layout: &layout, bytes: keys },
            values: HostTensor { identity: BufferIdentity { object: object + 1, generation: 1 }, layout: &layout, bytes: values },
            first_token: 0, token_count: 2, buffer_first_position: 0, first_sequence: 1,
        }).unwrap();
        let image = captured.snapshot(1).unwrap();
        assert_eq!(&image.replay_attention(&c, &q, budget()).unwrap().values, expected);
    }
    assert_eq!(original.encode().unwrap(), original_bytes);
    assert_eq!(experiments.branch_count(), 1);
    assert_eq!(experiments.retained_edit_count(), 2);
}
