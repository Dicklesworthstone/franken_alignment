//! Portable layer sets and all-layer writes through the original scalar codec.
#[path = "support/model_kv_fixture.rs"]
mod support;

use support::*;
use fa_reference::action::consequence::activation::tensor::{BufferIdentity, TensorLayout};
use fa_reference::action::consequence::activation::tensor::kv::image::IMAGE_HEADER_BYTES;
use fa_reference::action::consequence::activation::tensor::kv::model::{
    LayerRestore, ModelKvDescriptor, ModelKvImage, MODEL_DESCRIPTOR_HEADER_BYTES,
    MODEL_LAYER_DESCRIPTOR_BYTES,
};
use fa_reference::action::consequence::activation::tensor::kv::restore::{
    HostTensorMut, KvDestination, KvRestoreWindow,
};
use fa_reference::Error;
use std::collections::BTreeMap;

fn output() -> [Vec<u8>; 4] { [vec![0xaa; 8], vec![0xaa; 8], vec![0xaa; 16], vec![0xaa; 16]] }

fn destinations<'a>(buffers: &'a Buffers, output: &'a mut [Vec<u8>; 4]) -> BTreeMap<u64, LayerRestore<'a>> {
    let [ka, va, kb, vb] = output;
    fn target<'a>(object: u64, layout: &'a TensorLayout, bytes: &'a mut [u8]) -> HostTensorMut<'a> {
        HostTensorMut { identity: BufferIdentity { object, generation: 1 }, layout, bytes }
    }
    let window = KvRestoreWindow { first_position: 0, token_count: 2, batch: 0, first_token: 0, buffer_first_position: 0 };
    BTreeMap::from([
        (10, LayerRestore { destination: KvDestination::Separate {
            keys: target(101, &buffers.ka, ka), values: target(102, &buffers.va, va),
        }, window }),
        (20, LayerRestore { destination: KvDestination::Separate {
            keys: target(103, &buffers.kb, kb), values: target(104, &buffers.vb, vb),
        }, window }),
    ])
}

#[test]
fn manual_header_and_scalar_order_match_the_portable_encoding() {
    let image = image();
    let descriptor = image.descriptor();
    let header = descriptor.encode().unwrap();
    assert_eq!(&header[..8], b"FAMKVIM\x01");
    assert_eq!(&header[8..16], &9_u64.to_be_bytes());
    assert_eq!(&header[16..24], &1_u64.to_be_bytes());
    assert_eq!(&header[24..28], &2_u32.to_be_bytes());
    assert_eq!(&header[28..36], &10_u64.to_be_bytes());
    assert_eq!(&header[192..200], &20_u64.to_be_bytes());
    assert_eq!(header.len(), 356);
    assert_eq!(ModelKvDescriptor::decode(&header).unwrap(), descriptor);
    let bytes = image.encode().unwrap();
    let buffers = Buffers::new();
    let expected = [
        &buffers.keys_a[..4], &buffers.values_a[..4], &buffers.keys_a[4..], &buffers.values_a[4..],
        &buffers.keys_b[..8], &buffers.values_b[..8], &buffers.keys_b[8..], &buffers.values_b[8..],
    ].concat();
    assert_eq!(&bytes[..356], header.as_slice());
    assert_eq!(&bytes[356..], expected.as_slice());
    assert_eq!(bytes.len(), 404);
    assert_eq!(descriptor.image_len().unwrap(), bytes.len());
    assert_eq!(ModelKvImage::decode(&bytes, &descriptor).unwrap().encode().unwrap(), bytes);
}

#[test]
fn all_layers_restore_exact_bytes_after_the_original_images_are_dropped() {
    let source = image();
    let descriptor = source.descriptor();
    let bytes = source.encode().unwrap();
    drop(source);
    let imported = ModelKvImage::decode(&bytes, &descriptor).unwrap();
    let buffers = Buffers::new();
    let mut output = output();
    let plan = imported.prepare_restore(destinations(&buffers, &mut output)).unwrap();
    assert!(plan.staged_bytes() >= 16 * 4);
    drop(imported);
    let receipt = plan.commit();
    assert_eq!(receipt.source, descriptor);
    assert_eq!(receipt.layers.iter().map(|entry| entry.layer).collect::<Vec<_>>(), vec![10, 20]);
    assert_eq!(receipt.normalized_values, 16);
    assert_eq!(receipt.bytes_written, 48);
    assert_eq!(output, [buffers.keys_a, buffers.values_a, buffers.keys_b, buffers.values_b]);
}

#[test]
fn late_restore_failure_and_dropped_plan_leave_every_layer_untouched() {
    let image = image();
    let buffers = Buffers::new();
    let mut output = output();
    output[3].pop();
    let before = output.clone();
    assert_eq!(image.prepare_restore(destinations(&buffers, &mut output)).unwrap_err(), Error::Incomplete);
    assert_eq!(output, before);
    output[3].push(0xaa);
    let before = output.clone();
    let plan = image.prepare_restore(destinations(&buffers, &mut output)).unwrap();
    drop(plan);
    assert_eq!(output, before);
    image.prepare_restore(destinations(&buffers, &mut output)).unwrap().commit();
    assert_eq!(output[3], buffers.values_b);
}

#[test]
fn missing_destinations_and_different_source_windows_cannot_restore_a_partial_model() {
    let image = image();
    let buffers = Buffers::new();
    let mut output = output();
    let before = output.clone();
    let mut missing = destinations(&buffers, &mut output);
    missing.remove(&20);
    assert_eq!(image.prepare_restore(missing).unwrap_err(), Error::Binding);
    assert_eq!(output, before);
    let mut mismatched = destinations(&buffers, &mut output);
    mismatched.get_mut(&20).unwrap().window.token_count = 1;
    assert_eq!(image.prepare_restore(mismatched).unwrap_err(), Error::Binding);
    assert_eq!(output, before);
    image.prepare_restore(destinations(&buffers, &mut output)).unwrap().commit();
    assert_eq!(output[0], buffers.keys_a);
}

#[test]
fn overlapping_declared_destination_objects_refuse_before_any_write() {
    let image = image();
    let buffers = Buffers::new();
    let mut output = output();
    let before = output.clone();
    let mut targets = destinations(&buffers, &mut output);
    if let KvDestination::Separate { keys, .. } = &mut targets.get_mut(&20).unwrap().destination {
        keys.identity.object = 101;
    } else { panic!("fixture requires separate buffers"); }
    assert_eq!(image.prepare_restore(targets).unwrap_err(), Error::Binding);
    assert_eq!(output, before);
}

#[test]
fn layer_specific_page_placement_preserves_padding_and_unselected_tokens() {
    let image = image();
    let original = Buffers::new();
    let make = |layout: &TensorLayout, offset| TensorLayout::new(layout.shape(), layout.byte_strides(),
        offset, layout.encoding(), layout.byte_order()).unwrap();
    let layouts = [make(&original.ka, 3), make(&original.va, 17), make(&original.kb, 5), make(&original.vb, 9)];
    let mut output = [vec![0xaa; 40], vec![0xaa; 40], vec![0xaa; 40], vec![0xaa; 40]];
    let mut expected = output.clone();
    expected[0][7..11].copy_from_slice(&original.keys_a[4..8]);
    expected[1][21..25].copy_from_slice(&original.values_a[4..8]);
    expected[2][5..13].copy_from_slice(&original.keys_b[8..16]);
    expected[3][9..17].copy_from_slice(&original.values_b[8..16]);
    let [ka, va, kb, vb] = &mut output;
    let targets = BTreeMap::from([
        (10, LayerRestore { destination: KvDestination::Separate {
            keys: HostTensorMut { identity: BufferIdentity { object: 101, generation: 1 }, layout: &layouts[0], bytes: ka },
            values: HostTensorMut { identity: BufferIdentity { object: 102, generation: 1 }, layout: &layouts[1], bytes: va },
        }, window: KvRestoreWindow { first_position: 1, token_count: 1, batch: 0, first_token: 1, buffer_first_position: 0 } }),
        (20, LayerRestore { destination: KvDestination::Separate {
            keys: HostTensorMut { identity: BufferIdentity { object: 103, generation: 1 }, layout: &layouts[2], bytes: kb },
            values: HostTensorMut { identity: BufferIdentity { object: 104, generation: 1 }, layout: &layouts[3], bytes: vb },
        }, window: KvRestoreWindow { first_position: 1, token_count: 1, batch: 0, first_token: 0, buffer_first_position: 1 } }),
    ]);
    let receipt = image.prepare_restore(targets).unwrap().commit();
    assert_eq!(receipt.bytes_written, 24);
    assert_eq!(receipt.token_count, 1);
    assert_eq!(output, expected);
}

#[test]
fn every_truncation_and_trailing_byte_refuses_whole_image_import() {
    let image = image();
    let descriptor = image.descriptor();
    let header = descriptor.encode().unwrap();
    for end in 0..header.len() { assert!(ModelKvDescriptor::decode(&header[..end]).is_err(), "descriptor {end}"); }
    let bytes = image.encode().unwrap();
    for end in 0..bytes.len() { assert!(ModelKvImage::decode(&bytes[..end], &descriptor).is_err(), "image {end}"); }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(ModelKvImage::decode(&trailing, &descriptor).unwrap_err(), Error::InvalidInput);
}

#[test]
fn all_header_substitutions_and_mixed_layer_revisions_are_rejected() {
    let image = image();
    let descriptor = image.descriptor();
    let bytes = image.encode().unwrap();
    for index in 0..descriptor.descriptor_len() {
        let mut changed = bytes.clone();
        changed[index] ^= 1;
        assert!(ModelKvImage::decode(&changed, &descriptor).is_err(), "header byte {index}");
    }
    let mut header = descriptor.encode().unwrap();
    let mut late = descriptor.layers()[&20].clone();
    late.source_revision = 2;
    let start = MODEL_DESCRIPTOR_HEADER_BYTES + MODEL_LAYER_DESCRIPTOR_BYTES + 8;
    header[start..start + IMAGE_HEADER_BYTES].copy_from_slice(&late.encode().unwrap());
    assert_eq!(ModelKvDescriptor::decode(&header).unwrap_err(), Error::Binding);
}

#[test]
fn finite_payload_changes_are_not_falsely_reported_as_authenticated() {
    let image = image();
    let descriptor = image.descriptor();
    let original = image.encode().unwrap();
    let mut changed = original.clone();
    let length = changed.len();
    changed[length - 4..].copy_from_slice(&17_f32.to_le_bytes());
    let parsed = ModelKvImage::decode(&changed, &descriptor).unwrap();
    assert_eq!(parsed.encode().unwrap(), changed);
    assert_ne!(parsed.layer(20).unwrap().encode().unwrap(), image.layer(20).unwrap().encode().unwrap());
    assert_eq!(image.encode().unwrap(), original);
}

#[test]
fn a_nonfinite_value_in_the_final_layer_rejects_the_entire_import() {
    let image = image();
    let descriptor = image.descriptor();
    let mut bytes = image.encode().unwrap();
    let length = bytes.len();
    bytes[length - 4..].copy_from_slice(&f32::INFINITY.to_le_bytes());
    assert_eq!(ModelKvImage::decode(&bytes, &descriptor).unwrap_err(), Error::InvalidInput);
    assert_eq!(image.len(), 2);
}

#[test]
fn aggregate_decode_limit_is_checked_using_only_the_descriptor() {
    use fa_reference::action::consequence::activation::CaptureProfile;
    use fa_reference::action::consequence::activation::tensor::{ByteOrder, ScalarEncoding, TensorContract};
    use fa_reference::action::consequence::activation::tensor::kv::KvContract;
    use fa_reference::action::consequence::activation::tensor::kv::image::KvImageDescriptor;
    let mut header = b"FAMKVIM\x01".to_vec();
    header.extend_from_slice(&9_u64.to_be_bytes());
    header.extend_from_slice(&1_u64.to_be_bytes());
    header.extend_from_slice(&17_u32.to_be_bytes());
    for id in 1..=17_u64 {
        let tensor = |tap| TensorContract::new(CaptureProfile {
            tenant: 1, model: 2, model_generation: 3, tap, layout_generation: 1,
        }, ScalarEncoding::Binary32, ByteOrder::Little, 1, 128).unwrap();
        let descriptor = KvImageDescriptor {
            contract: KvContract::new(tensor(2 * id), tensor(2 * id + 1), 1).unwrap(),
            stream: 1, source_batch: 0, first_position: 0, first_sequence: 1,
            source_revision: 1, token_count: 4096,
        };
        assert_eq!(descriptor.normalized_values().unwrap(), 1_048_576);
        header.extend_from_slice(&id.to_be_bytes());
        header.extend_from_slice(&descriptor.encode().unwrap());
    }
    assert!(header.len() < 4096);
    assert_eq!(ModelKvDescriptor::decode(&header).unwrap_err(), Error::Limit);
}

#[test]
fn empty_prefix_round_trip_does_not_invent_restorable_rows() {
    let image = capture().snapshot(0).unwrap();
    let descriptor = image.descriptor();
    let bytes = image.encode().unwrap();
    assert_eq!(bytes.len(), descriptor.descriptor_len());
    let image = ModelKvImage::decode(&bytes, &descriptor).unwrap();
    assert!(image.is_empty());
    let buffers = Buffers::new();
    let mut output = output();
    let before = output.clone();
    assert_eq!(image.prepare_restore(destinations(&buffers, &mut output)).unwrap_err(), Error::Missing);
    assert_eq!(output, before);
}
