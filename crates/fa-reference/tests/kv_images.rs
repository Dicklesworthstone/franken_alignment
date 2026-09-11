//! Frozen and serialized KV values feed the same checked host-buffer writer.

use fa_reference::action::consequence::activation::CaptureProfile;
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorContract, TensorLayout,
};
use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvBudget, KvCapture, KvContract, MAX_KV_POSITIONS, MAX_KV_VALUES};
use fa_reference::action::consequence::activation::tensor::kv::image::{KvImage, KvImageDescriptor, IMAGE_HEADER_BYTES, MAX_IMAGE_BYTES};
use fa_reference::action::consequence::activation::tensor::kv::restore::{HostTensorMut, KvDestination, KvRestoreWindow};
use fa_reference::Error;

fn contract() -> KvContract {
    let profile = CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 };
    KvContract::new(
        TensorContract::new(profile, ScalarEncoding::Binary16, ByteOrder::Big, 1, 2).unwrap(),
        TensorContract::new(CaptureProfile { tap: 6, ..profile }, ScalarEncoding::Binary32, ByteOrder::Little, 1, 1).unwrap(),
        4,
    ).unwrap()
}
fn capture() -> KvCapture {
    KvCapture::new(contract(), 7, 0, 10, 20, KvBudget { positions: 4, normalized_values: 12 }).unwrap()
}
fn kl() -> TensorLayout {
    TensorLayout::new([1, 2, 1, 2], [0, 4, 0, 2], 0, ScalarEncoding::Binary16, ByteOrder::Big).unwrap()
}
fn vl() -> TensorLayout {
    TensorLayout::new([1, 2, 1, 1], [0, 4, 0, 0], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap()
}
fn append(capture: &mut KvCapture, revision: u64, position: u64, sequence: u64, kb: &[u8], vb: &[u8]) {
    capture.append(revision, KvAppend {
        keys: HostTensor { identity: BufferIdentity { object: 1, generation: revision + 1 }, layout: &kl(), bytes: kb },
        values: HostTensor { identity: BufferIdentity { object: 2, generation: revision + 1 }, layout: &vl(), bytes: vb },
        first_token: 0, token_count: 2, buffer_first_position: position, first_sequence: sequence,
    }).unwrap();
}
fn bytes() -> (Vec<u8>, Vec<u8>) {
    ([0x3c00_u16, 0x8000, 1, 0x7bff].into_iter().flat_map(u16::to_be_bytes).collect(),
     [1_u32, 0xff7f_ffff].into_iter().flat_map(u32::to_le_bytes).collect())
}
fn image() -> KvImage {
    let mut c = capture();
    let (kb, vb) = bytes();
    append(&mut c, 0, 10, 20, &kb, &vb);
    c.snapshot(1).unwrap()
}
fn restore(image: &KvImage) -> (Vec<u8>, Vec<u8>) {
    let mut kb = vec![0xee; 8];
    let mut vb = vec![0xee; 8];
    image.prepare_restore(KvDestination::Separate {
        keys: HostTensorMut { identity: BufferIdentity { object: 3, generation: 1 }, layout: &kl(), bytes: &mut kb },
        values: HostTensorMut { identity: BufferIdentity { object: 4, generation: 1 }, layout: &vl(), bytes: &mut vb },
    }, KvRestoreWindow { first_position: 10, token_count: 2, batch: 0, first_token: 0, buffer_first_position: 10 })
        .unwrap().commit();
    (kb, vb)
}

#[test]
fn portable_round_trip_restores_original_scalar_bytes_after_capture_is_gone() {
    let original = image();
    let manifest = original.descriptor().encode().unwrap();
    let encoded = original.encode().unwrap();
    assert_eq!(manifest.len(), IMAGE_HEADER_BYTES);
    assert_eq!(encoded.len(), IMAGE_HEADER_BYTES + 16);
    assert_eq!(&encoded[..IMAGE_HEADER_BYTES], manifest);
    let descriptor = KvImageDescriptor::decode(&manifest).unwrap();
    drop(original);
    let decoded = KvImage::decode(&encoded, &descriptor).unwrap();
    assert_eq!(decoded.encode().unwrap(), encoded);
    assert_eq!(restore(&decoded), bytes());
}

#[test]
fn snapshot_stays_frozen_across_new_pages_and_source_buffer_reuse() {
    let mut c = capture();
    let (mut kb, mut vb) = bytes();
    append(&mut c, 0, 10, 20, &kb, &vb);
    let frozen = c.snapshot(1).unwrap();
    let encoded = frozen.encode().unwrap();
    kb.fill(0);
    vb.fill(0);
    append(&mut c, 1, 12, 22, &kb, &vb);
    assert_eq!(c.len(), 4);
    assert_eq!(frozen.len(), 2);
    assert_eq!(frozen.encode().unwrap(), encoded);
    assert!(frozen.token(12).is_err());
    assert_eq!(c.snapshot(1).unwrap_err(), Error::Stale);
    drop(c);
    assert_eq!(restore(&frozen), bytes());
    let branch = frozen.clone();
    let (mut writable_keys, _) = restore(&branch);
    writable_keys.fill(0x55);
    assert_eq!(restore(&frozen), bytes());
}

#[test]
fn all_truncations_extra_bytes_and_header_substitution_refuse() {
    let image = image();
    let encoded = image.encode().unwrap();
    for end in 0..encoded.len() {
        assert!(KvImage::decode(&encoded[..end], image.descriptor()).is_err(), "end={end}");
    }
    let mut extra = encoded.clone(); extra.push(0);
    assert!(KvImage::decode(&extra, image.descriptor()).is_err());
    for byte in 0..IMAGE_HEADER_BYTES {
        let mut changed = encoded.clone(); changed[byte] ^= 1;
        assert!(KvImage::decode(&changed, image.descriptor()).is_err(), "byte={byte}");
    }
}

#[test]
fn nonfinite_late_values_refuse_without_a_partial_image() {
    let image = image();
    let mut encoded = image.encode().unwrap();
    let end = encoded.len();
    // The last V is binary32 little-endian. Earlier K and V rows are valid.
    encoded[end - 4..].copy_from_slice(&f32::INFINITY.to_le_bytes());
    assert_eq!(KvImage::decode(&encoded, image.descriptor()).unwrap_err(), Error::InvalidInput);
    assert_eq!(restore(&image), bytes());
}

#[test]
fn finite_value_edits_are_not_misrepresented_as_authenticated_corruption_detection() {
    let image = image();
    let mut edited = image.encode().unwrap();
    // This is another finite, exactly representable half. The descriptor is NOT
    // a content commitment, so parsing must not pretend to authenticate it.
    edited[IMAGE_HEADER_BYTES..IMAGE_HEADER_BYTES + 2].copy_from_slice(&0x4000_u16.to_be_bytes());
    let decoded = KvImage::decode(&edited, image.descriptor()).unwrap();
    assert_ne!(restore(&decoded), bytes());
    assert_eq!(decoded.encode().unwrap(), edited);
}

#[test]
fn descriptor_identity_position_and_budget_mismatches_are_rejected() {
    let image = image();
    let encoded = image.encode().unwrap();
    for field in 0..5 {
        let mut expected = image.descriptor().clone();
        match field {
            0 => expected.stream += 1,
            1 => expected.first_position += 1,
            2 => expected.first_sequence += 1,
            3 => expected.source_batch += 1,
            _ => expected.source_revision += 1,
        }
        assert_eq!(KvImage::decode(&encoded, &expected).unwrap_err(), Error::Binding);
    }
    let mut too_many = image.descriptor().clone();
    too_many.token_count = MAX_KV_POSITIONS + 1;
    assert_eq!(too_many.encode(), Err(Error::Limit));
    let mut overflow = image.descriptor().clone();
    overflow.first_position = u64::MAX;
    assert_eq!(overflow.encode(), Err(Error::Overflow));
    assert_eq!(KvImage::decode(&vec![0; MAX_IMAGE_BYTES + 1], image.descriptor()).unwrap_err(), Error::Limit);
    let p = contract().keys().profile();
    let wide = KvContract::new(
        TensorContract::new(p, ScalarEncoding::Binary32, ByteOrder::Little, 1, 65_536).unwrap(),
        TensorContract::new(CaptureProfile { tap: 6, ..p }, ScalarEncoding::Binary32, ByteOrder::Little, 1, 65_536).unwrap(), 1,
    ).unwrap();
    let mut exact = image.descriptor().clone();
    exact.contract = wide;
    exact.token_count = MAX_KV_VALUES / 131_072;
    assert_eq!(exact.encoded_len().unwrap(), MAX_IMAGE_BYTES);
    exact.token_count += 1;
    assert_eq!(exact.encoded_len(), Err(Error::Limit));
}

#[test]
fn empty_snapshot_is_an_empty_prefix_not_restored_or_missing_data() {
    let c = capture();
    let frozen = c.snapshot(0).unwrap();
    assert!(frozen.is_empty());
    let encoded = frozen.encode().unwrap();
    assert_eq!(encoded.len(), IMAGE_HEADER_BYTES);
    let decoded = KvImage::decode(&encoded, frozen.descriptor()).unwrap();
    assert!(decoded.is_empty());
    assert_eq!(decoded.token(10).unwrap_err(), Error::Missing);
}

#[test]
fn imported_values_keep_source_identity_but_not_a_host_capture_receipt() {
    let original = image();
    let decoded = KvImage::decode(&original.encode().unwrap(), original.descriptor()).unwrap();
    for pos in 10..12 {
        let before = original.token(pos).unwrap();
        let after = decoded.token(pos).unwrap();
        for (a, b) in [(before.key(), after.key()), (before.value(), after.value())] {
            assert_eq!(a.identity(), b.identity());
            assert_eq!(a.encode_initial(23).unwrap(), b.encode_initial(23).unwrap());
        }
    }
}
