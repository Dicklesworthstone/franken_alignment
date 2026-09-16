use super::*;

fn manifest() -> ModelManifest {
    ModelManifest { tenant: 1, model: 2, model_generation: 3, host_generation: 4,
        tokenizer_generation: 5, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32] }
}
fn profile() -> CaptureProfile { CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 6, layout_generation: 7 } }
fn passport() -> ModelPassport {
    ModelPassport::new(8, 9, manifest(), vec![IdentityAnchor::new(10, profile(), 11,
        vec![12], &[[-0.0, 0.0]]).unwrap()]).unwrap()
}
fn frame(values: &[f32]) -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: profile(), stream: 11, sequence: 1, position: 0 }, values).unwrap()
}

#[test]
fn manual_passport_vector_preserves_every_registration_field() {
    let mut expected = b"FAIDP\0\0\x01".to_vec();
    for n in [8_u64, 9, 1, 2, 3, 4, 5] { expected.extend_from_slice(&n.to_be_bytes()); }
    for byte in 1..=5 { expected.extend_from_slice(&[byte; 32]); }
    expected.extend_from_slice(&1_u32.to_be_bytes());
    for n in [10_u64, 1, 2, 3, 6, 7, 11] { expected.extend_from_slice(&n.to_be_bytes()); }
    for n in [1_u32, 12, 1, 0x80000000, 0] { expected.extend_from_slice(&n.to_be_bytes()); }
    assert_eq!(encode_passport(&passport()).unwrap(), expected);
    assert_eq!(decode_passport(&expected).unwrap(), passport());
    for end in 0..expected.len() { assert!(decode_passport(&expected[..end]).is_err()); }
    let mut extra = expected.clone(); extra.push(0); assert!(decode_passport(&extra).is_err());
    let mut nan = expected.clone(); let offset = nan.len() - 8;
    nan[offset..offset + 4].copy_from_slice(&f32::NAN.to_bits().to_be_bytes());
    assert_eq!(decode_passport(&nan), Err(Error::InvalidInput));
    // Change only the anchor's model: same dimensions cannot substitute identity.
    let mut foreign = expected; foreign[244..252].copy_from_slice(&99_u64.to_be_bytes());
    assert_eq!(decode_passport(&foreign), Err(Error::Binding));
}

#[test]
fn source_roundtrip_uses_original_full_precision_format_and_native_comparison() {
    let values = [-0.0, 0.0, f32::from_bits(1), -f32::from_bits(1), f32::MAX, -f32::MAX];
    let original = frame(&values);
    let bytes = original.encode_initial(23).unwrap();
    let decoded = decode_frame(&bytes).unwrap();
    assert_eq!(decoded.identity(), original.identity());
    assert_eq!(decoded.encode_initial(23).unwrap(), bytes);
    let anchor = IdentityAnchor::new(10, profile(), 11, vec![12],
        &values.iter().map(|v| [*v, *v]).collect::<Vec<_>>()).unwrap();
    assert_eq!(anchor.compare(&decoded).unwrap().outside(), 0);
    let mut changed = bytes.clone();
    changed[HEADER_BYTES..HEADER_BYTES + 4].copy_from_slice(&1.0_f32.to_bits().to_be_bytes());
    let changed = decode_frame(&changed).unwrap();
    assert_eq!(anchor.compare(&changed).unwrap().outside(), 1);
    assert_eq!(anchor.compare(&changed).unwrap().first_outlier().unwrap().coordinate, 0);
}

#[test]
fn progressive_truncated_nonfinite_and_oversize_frames_are_not_complete_measurements() {
    let source = frame(&[1.0]); let bytes = source.encode_initial(23).unwrap();
    for end in 0..bytes.len() { assert!(decode_frame(&bytes[..end]).is_err()); }
    assert!(matches!(decode_frame(&source.encode_initial(22).unwrap()), Err(Error::Binding)));
    assert!(matches!(decode_frame(&source.encode_refinement(22, 23).unwrap()), Err(Error::Binding)));
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bad = bytes.clone(); bad[HEADER_BYTES..].copy_from_slice(&value.to_bits().to_be_bytes());
        assert!(matches!(decode_frame(&bad), Err(Error::InvalidInput)));
    }
    let mut bad = bytes; bad[72..76].copy_from_slice(&(MAX_VALUES as u32 + 1).to_be_bytes());
    assert!(matches!(decode_frame(&bad), Err(Error::Limit)));
    assert!(matches!(decode_frame(&vec![0; MAX_BLOCK_BYTES + 1]), Err(Error::Limit)));
}

#[test]
fn original_aggregate_caps_and_canonical_anchor_order_are_enforced() {
    let anchors: Vec<_> = (1..=MAX_ANCHORS).map(|id| IdentityAnchor::new(id as u64, profile(), 11,
        vec![12; MAX_STIMULUS_TOKENS / MAX_ANCHORS],
        &vec![[0.0, 1.0]; MAX_VALUES / MAX_ANCHORS]).unwrap()).collect();
    let original = ModelPassport::new(8, 9, manifest(), anchors).unwrap();
    let bytes = encode_passport(&original).unwrap();
    assert_eq!(bytes.len(), MAX_PASSPORT_BYTES);
    assert_eq!(decode_passport(&bytes).unwrap(), original);
    let mut extra = bytes.clone(); extra.push(0); assert_eq!(decode_passport(&extra), Err(Error::Limit));
    let mut count = bytes.clone(); count[224..228].copy_from_slice(&(MAX_ANCHORS as u32 + 1).to_be_bytes());
    assert_eq!(decode_passport(&count), Err(Error::Limit));
    let mut reordered = bytes;
    let width = 64 + 4 * (MAX_STIMULUS_TOKENS / MAX_ANCHORS) + 8 * (MAX_VALUES / MAX_ANCHORS);
    reordered[228..228 + 8].copy_from_slice(&2_u64.to_be_bytes());
    reordered[228 + width..236 + width].copy_from_slice(&1_u64.to_be_bytes());
    assert_eq!(decode_passport(&reordered), Err(Error::Binding));
}

#[test]
fn observed_manifest_mismatch_is_preserved_not_filtered_by_registration_validation() {
    let mut observed = manifest(); observed.model_generation = 0; observed.weights = [0; 32];
    let bytes = encode_manifest(&observed);
    assert_eq!(bytes.len(), MANIFEST_BYTES);
    assert_eq!(decode_manifest(&bytes).unwrap(), observed);
    assert_ne!(decode_manifest(&bytes).unwrap(), *passport().manifest());
    for end in 0..bytes.len() { assert!(decode_manifest(&bytes[..end]).is_err()); }
    let mut extra = bytes; extra.push(0); assert_eq!(decode_manifest(&extra), Err(Error::Limit));
}
