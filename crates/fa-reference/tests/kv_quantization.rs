//! Actual captured buffers exercise the lossy baseline. No detector or native
//! restart claim follows from these synthetic scalar controls.
use fa_reference::action::consequence::activation::CaptureProfile;
use fa_reference::action::consequence::activation::tensor::*;
use fa_reference::action::consequence::activation::tensor::kv::*;
use fa_reference::action::consequence::activation::tensor::kv::experiment::{KvCell, KvSide};
use fa_reference::action::consequence::activation::tensor::kv::model::*;
use fa_reference::action::consequence::activation::tensor::kv::model::quantized::*;
use fa_reference::Error;
use std::collections::BTreeMap;

fn policy() -> KvQuantization { KvQuantization::new(11, 3).unwrap() }
fn allowance(image: &ModelKvImage) -> QuantizationBudget {
    QuantizationBudget { values: image.normalized_values(), encoded_bytes: image.quantized_len().unwrap() }
}
fn image(keys: &[f32], values: &[f32], heads: usize, query_heads: usize, count: usize,
    first: u64, half_second_layer: bool) -> ModelKvImage
{
    let mut layers = BTreeMap::new(); let mut contracts = BTreeMap::new();
    for id in 1..=2_u64 {
        let encoding = if id == 2 && half_second_layer { ScalarEncoding::Binary16 } else { ScalarEncoding::Binary32 };
        let contract = |tap, width| TensorContract::new(CaptureProfile {
            tenant: 1, model: 2, model_generation: 3, tap, layout_generation: 4,
        }, encoding, ByteOrder::Little, heads, width).unwrap();
        let k = contract(10 + id * 2, keys.len() / heads);
        let v = contract(11 + id * 2, values.len() / heads);
        let kv = KvContract::new(k.clone(), v.clone(), query_heads).unwrap();
        let budget = KvBudget { positions: count.max(1), normalized_values: (keys.len() + values.len()) * count.max(1) };
        let mut captured = KvCapture::new(kv.clone(), 9, 0, first, 1, budget).unwrap();
        if count > 0 {
            let bytes = |source: &[f32]| -> Vec<u8> {
                (0..count).flat_map(|_| source.iter().flat_map(|x| match encoding {
                    ScalarEncoding::Binary32 => x.to_le_bytes().to_vec(),
                    ScalarEncoding::Binary16 => {
                        let bits: u16 = match x.to_bits() {
                            0 => 0, 0x8000_0000 => 0x8000, 0x3f80_0000 => 0x3c00,
                            0xbf80_0000 => 0xbc00, _ => panic!("half fixture only has exact unit values"),
                        }; bits.to_le_bytes().to_vec()
                    }
                    _ => unreachable!(),
                })).collect()
            };
            let layout = |t: &TensorContract| TensorLayout::new([1, count, heads, t.channels()],
                [0, t.dimensions() * encoding.bytes(), t.channels() * encoding.bytes(), encoding.bytes()],
                0, encoding, ByteOrder::Little).unwrap();
            let kb = bytes(keys); let vb = bytes(values); let kl = layout(&k); let vl = layout(&v);
            captured.append(0, KvAppend {
                keys: HostTensor { identity: BufferIdentity { object: k.profile().tap, generation: 1 }, layout: &kl, bytes: &kb },
                values: HostTensor { identity: BufferIdentity { object: v.profile().tap, generation: 1 }, layout: &vl, bytes: &vb },
                first_token: 0, token_count: count, buffer_first_position: first, first_sequence: 1,
            }).unwrap();
        }
        layers.insert(id, captured.snapshot(captured.revision()).unwrap()); contracts.insert(id, kv);
    }
    ModelKvImage::from_layers(ModelKvProfile::new(5, 6, contracts).unwrap(), layers).unwrap()
}
fn cell(side: KvSide, position: u64, head: usize, channel: usize) -> KvCell { KvCell { side, position, head, channel } }
fn value(image: &QuantizedKvImage, layer: u64, side: KvSide, position: u64, head: usize, channel: usize) -> f32 {
    f32::from_bits(image.bits(layer, cell(side, position, head, channel)).unwrap())
}

#[test]
fn literal_codes_ties_and_full_image_byte_cost_are_explicit() {
    let source = image(&[127.0, 63.5, -63.5, 0.0], &[1.0, -1.0], 1, 4, 2, 7, false);
    let (q, report) = source.quantize(policy(), allowance(&source)).unwrap();
    let mut expected = b"FAKVI8\0\x01".to_vec();
    expected.extend_from_slice(&11_u64.to_be_bytes()); expected.extend_from_slice(&3_u64.to_be_bytes());
    let descriptor = source.descriptor().encode().unwrap();
    expected.extend_from_slice(&(descriptor.len() as u32).to_be_bytes()); expected.extend_from_slice(&0_u32.to_be_bytes());
    expected.extend_from_slice(&56_u64.to_be_bytes()); expected.extend_from_slice(&descriptor);
    for _ in 0..4 {
        expected.extend_from_slice(&127.0_f32.to_bits().to_be_bytes()); expected.extend_from_slice(&[127, 64, 192, 0]);
        expected.extend_from_slice(&1.0_f32.to_bits().to_be_bytes()); expected.extend_from_slice(&[127, 129]);
    }
    assert_eq!(q.encode().unwrap(), expected);
    assert_eq!(q.compact_scalar_bytes(), 56); assert_eq!(q.groups(), 8); assert_eq!(q.values(), 24);
    assert_eq!(report.encoded_bytes, expected.len()); assert_eq!(report.source_scalar_bytes, 96);
    assert_eq!(report.source_image_bytes, source.encode().unwrap().len());
    assert_eq!(report.source_coordinate_visits, 48);
    assert_eq!(value(&q, 1, KvSide::Key, 7, 0, 1), 64.0);
    assert_eq!(value(&q, 2, KvSide::Key, 8, 0, 2), -64.0);
    assert_eq!(report.layers[&1].keys.changed_words, 4);
    assert_eq!(report.layers[&1].keys.squared_error_sum, 1.0);
}

#[test]
fn tiny_signals_and_signed_zero_loss_are_reported_not_hidden() {
    let tiny = f32::from_bits(1);
    let source = image(&[f32::MAX, 1.0, -f32::MAX], &[tiny, -tiny, -0.0], 1, 1, 1, 0, false);
    let (q, report) = source.quantize(policy(), allowance(&source)).unwrap();
    assert_eq!(value(&q, 1, KvSide::Key, 0, 0, 0), f32::MAX);
    assert_eq!(value(&q, 1, KvSide::Key, 0, 0, 1), 0.0);
    assert_eq!(value(&q, 1, KvSide::Value, 0, 0, 0).to_bits(), tiny.to_bits());
    assert_eq!(value(&q, 1, KvSide::Value, 0, 0, 1).to_bits(), (-tiny).to_bits());
    assert_eq!(value(&q, 1, KvSide::Value, 0, 0, 2).to_bits(), 0);
    assert_eq!(report.layers[&1].keys.nonzero_to_zero, 1);
    assert_eq!(report.layers[&1].values.signed_zero_changes, 1);
    assert_eq!(report.layers[&1].keys.max_absolute_error, 1.0);
}

#[test]
fn each_token_and_stored_head_has_its_own_range_not_query_head_copies() {
    let source = image(&[127.0, 1.0, 0.000127, 0.000001], &[1.0, -1.0, 0.0, -0.0], 2, 8, 2, 5, false);
    let (q, report) = source.quantize(policy(), allowance(&source)).unwrap();
    assert_eq!(value(&q, 1, KvSide::Key, 5, 0, 1), 1.0);
    assert!(value(&q, 1, KvSide::Key, 5, 1, 1) > 0.0);
    assert_eq!(q.groups(), 2 * 2 * 2 * 2); assert_eq!(report.values, 32);
    assert_eq!(q.compact_scalar_bytes(), 16 * 6);
    assert_eq!(q.bits(1, cell(KvSide::Key, 5, 2, 0)), Err(Error::InvalidInput));
    assert_eq!(q.bits(1, cell(KvSide::Key, 4, 0, 0)), Err(Error::Missing));
    assert_eq!(q.bits(1, cell(KvSide::Key, 7, 0, 0)), Err(Error::Missing));
    assert_eq!(q.bits(99, cell(KvSide::Key, 5, 0, 0)), Err(Error::Missing));
}

#[test]
fn complete_budget_admission_and_small_group_expansion_leave_source_unchanged() {
    let source = image(&[1.0], &[-1.0], 1, 1, 1, 0, false); let before = source.encode().unwrap();
    let exact = allowance(&source);
    assert!(exact.encoded_bytes > before.len());
    for short in [QuantizationBudget { values: exact.values - 1, ..exact },
        QuantizationBudget { encoded_bytes: exact.encoded_bytes - 1, ..exact }]
    { assert_eq!(source.quantize(policy(), short).unwrap_err(), Error::Limit); }
    assert!(source.quantize(policy(), exact).is_ok()); assert_eq!(source.encode().unwrap(), before);
    assert_eq!(KvQuantization::new(0, 1), Err(Error::InvalidInput));
}

#[test]
fn mixed_precision_source_counts_actual_half_width_payloads() {
    let source = image(&[1.0, -1.0], &[0.0, 1.0, -1.0], 1, 2, 2, 0, true);
    let (q, report) = source.quantize(policy(), allowance(&source)).unwrap();
    assert_eq!(report.values, 20); assert_eq!(report.source_scalar_bytes, 60);
    assert_eq!(report.layers[&1].keys.changed_words, 0); assert_eq!(report.layers[&2].values.changed_words, 0);
    let bytes = q.encode().unwrap();
    let loaded = QuantizedKvImage::decode(&bytes, policy(), &source.descriptor(), allowance(&source)).unwrap();
    assert_eq!(loaded.encode().unwrap(), bytes);
}

#[test]
fn every_truncation_suffix_header_or_source_substitution_refuses() {
    let source = image(&[1.0, 0.5], &[0.0, -1.0], 1, 2, 1, 0, false);
    let (q, _) = source.quantize(policy(), allowance(&source)).unwrap(); let bytes = q.encode().unwrap();
    let descriptor = source.descriptor(); let budget = allowance(&source);
    for length in 0..bytes.len() { assert!(QuantizedKvImage::decode(&bytes[..length], policy(), &descriptor, budget).is_err()); }
    let mut tail = bytes.clone(); tail.push(0); assert!(QuantizedKvImage::decode(&tail, policy(), &descriptor, budget).is_err());
    for offset in [0, 7, 15, 23, 27, 31, 39, QUANTIZED_HEADER_BYTES + 15] {
        let mut changed = bytes.clone(); changed[offset] ^= 1;
        assert!(QuantizedKvImage::decode(&changed, policy(), &descriptor, budget).is_err(), "offset {offset}");
    }
    let other = image(&[1.0, 0.5], &[0.0, -1.0], 1, 2, 1, 1, false);
    assert!(QuantizedKvImage::decode(&bytes, policy(), &other.descriptor(), budget).is_err());
    assert!(QuantizedKvImage::decode(&bytes, KvQuantization::new(11, 4).unwrap(), &descriptor, budget).is_err());
}

#[test]
fn malformed_last_group_never_returns_a_partial_image() {
    let source = image(&[1.0, 0.5], &[0.0, -1.0], 1, 1, 1, 0, false);
    let (q, _) = source.quantize(policy(), allowance(&source)).unwrap(); let bytes = q.encode().unwrap();
    let last = bytes.len() - 6;
    for peak in [f32::NAN, f32::INFINITY, -1.0, -0.0, 0.0] {
        let mut changed = bytes.clone(); changed[last..last + 4].copy_from_slice(&peak.to_bits().to_be_bytes());
        assert!(QuantizedKvImage::decode(&changed, policy(), &source.descriptor(), allowance(&source)).is_err());
    }
    let mut changed = bytes.clone(); changed[last + 5] = 128;
    assert!(QuantizedKvImage::decode(&changed, policy(), &source.descriptor(), allowance(&source)).is_err());
    changed[last + 5] = 1;
    assert!(QuantizedKvImage::decode(&changed, policy(), &source.descriptor(), allowance(&source)).is_err());
    assert_eq!(q.encode().unwrap(), bytes);
}

#[test]
fn valid_numeric_tampering_is_not_mislabeled_as_authenticated_source() {
    let source = image(&[127.0, 1.0], &[0.0, -1.0], 1, 1, 1, 0, false);
    let (q, _) = source.quantize(policy(), allowance(&source)).unwrap(); let mut bytes = q.encode().unwrap();
    let body = QUANTIZED_HEADER_BYTES + source.descriptor().descriptor_len(); bytes[body + 5] = 2;
    let edited = QuantizedKvImage::decode(&bytes, policy(), &source.descriptor(), allowance(&source)).unwrap();
    assert_eq!(value(&q, 1, KvSide::Key, 0, 0, 1), 1.0);
    assert_eq!(value(&edited, 1, KvSide::Key, 0, 0, 1), 2.0);
}

#[test]
fn empty_and_zero_images_remain_well_defined_without_hidden_raw_owners() {
    let (empty, report) = { let source = image(&[0.0, -0.0], &[0.0], 1, 1, 0, 4, false);
        source.quantize(policy(), allowance(&source)).unwrap() };
    assert_eq!(empty.values(), 0); assert_eq!(empty.compact_scalar_bytes(), 0);
    assert_eq!(report.groups, 0); assert_eq!(report.source_scalar_bytes, 0);
    let cloned = empty.clone(); drop(empty);
    let bytes = cloned.encode().unwrap();
    assert!(QuantizedKvImage::decode(&bytes, policy(), cloned.source_descriptor(),
        QuantizationBudget { values: 0, encoded_bytes: bytes.len() }).is_ok());
    let source = image(&[0.0, -0.0], &[-0.0], 1, 1, 1, 4, false);
    let (q, _) = source.quantize(policy(), allowance(&source)).unwrap();
    assert_eq!(value(&q, 1, KvSide::Value, 4, 0, 0).to_bits(), 0);
}

#[test]
fn wider_heads_save_bytes_with_all_scale_and_descriptor_costs_included() {
    let row: Vec<f32> = (0..128).map(|n| n as f32 - 63.0).collect();
    let source = image(&row, &row, 1, 8, 8, 0, false);
    let (q, report) = source.quantize(policy(), allowance(&source)).unwrap();
    assert_eq!(q.compact_scalar_bytes(), 2 * 8 * 2 * (4 + 128));
    assert_eq!(report.source_scalar_bytes, 2 * 8 * 2 * 128 * 4);
    assert!(report.encoded_bytes * 3 < report.source_image_bytes);
    assert_eq!(report.encoded_bytes, q.encode().unwrap().len());
}
