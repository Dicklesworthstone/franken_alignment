//! Actual CPU byte writes through the checked reference capture/restore path.

use fa_reference::action::consequence::activation::{CaptureProfile, SourceFrame};
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorContract, TensorLayout, TokenSelection,
};
use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvBudget, KvCapture, KvContract};
use fa_reference::action::consequence::activation::tensor::kv::restore::{HostTensorMut, KvDestination, KvRestoreWindow};
use fa_reference::Error;

fn contract(tap: u64, encoding: ScalarEncoding) -> TensorContract {
    TensorContract::new(CaptureProfile {
        tenant: 1, model: 2, model_generation: 3, tap, layout_generation: 5,
    }, encoding, ByteOrder::Little, 1, 2).unwrap()
}
fn layout(encoding: ScalarEncoding, tokens: usize, offset: usize) -> TensorLayout {
    let width = encoding.bytes();
    TensorLayout::new([1, tokens, 1, 2], [0, 2 * width, 0, width], offset,
        encoding, ByteOrder::Little).unwrap()
}
fn buffer(object: u64) -> BufferIdentity { BufferIdentity { object, generation: 1 } }
fn selection() -> TokenSelection {
    TokenSelection { batch: 0, token: 0, first_position: 10, stream: 8, sequence: 9 }
}
fn captured() -> (KvCapture, Vec<u8>, Vec<u8>) {
    let keys = contract(4, ScalarEncoding::Binary16);
    let values = contract(6, ScalarEncoding::BFloat16);
    let kl = layout(ScalarEncoding::Binary16, 3, 0);
    let vl = layout(ScalarEncoding::BFloat16, 3, 0);
    let kb: Vec<_> = [0x3c00_u16, 0x8000, 1, 0x7bff, 0xc000, 0x4200]
        .into_iter().flat_map(u16::to_le_bytes).collect();
    let vb: Vec<_> = [0x3f80_u16, 0x8000, 1, 0x7f7f, 0xc000, 0x4040]
        .into_iter().flat_map(u16::to_le_bytes).collect();
    let mut capture = KvCapture::new(KvContract::new(keys, values, 4).unwrap(), 8, 0, 10, 9,
        KvBudget { positions: 3, normalized_values: 12 }).unwrap();
    capture.append(0, KvAppend {
        keys: HostTensor { identity: buffer(1), layout: &kl, bytes: &kb },
        values: HostTensor { identity: buffer(2), layout: &vl, bytes: &vb },
        first_token: 0, token_count: 3, buffer_first_position: 10, first_sequence: 9,
    }).unwrap();
    (capture, kb, vb)
}
fn window() -> KvRestoreWindow {
    KvRestoreWindow { first_position: 10, token_count: 3, batch: 0, first_token: 0, buffer_first_position: 10 }
}

#[test]
fn restores_exact_mixed_precision_kv_without_duplicating_query_heads() {
    let (capture, kb, vb) = captured();
    let kl = layout(ScalarEncoding::Binary16, 3, 1);
    let vl = layout(ScalarEncoding::BFloat16, 3, 3);
    let mut kd = vec![0xaa; 16];
    let mut vd = vec![0xbb; 18];
    let plan = capture.prepare_restore(1, KvDestination::Separate {
        keys: HostTensorMut { identity: buffer(7), layout: &kl, bytes: &mut kd },
        values: HostTensorMut { identity: buffer(8), layout: &vl, bytes: &mut vd },
    }, window()).unwrap();
    assert!(plan.staged_bytes() >= 24);
    let receipt = plan.commit();
    assert_eq!(receipt.bytes_written, 24);
    assert_eq!(receipt.normalized_values, 12);
    assert_eq!(&kd[1..13], kb);
    assert_eq!(&vd[3..15], vb);
    assert_eq!(kd[0], 0xaa);
    assert_eq!(&kd[13..], &[0xaa; 3]);
    assert_eq!(&vd[..3], &[0xbb; 3]);
    assert_eq!(&vd[15..], &[0xbb; 3]);
    assert_eq!(capture.revision(), 1);
    assert_eq!(capture.next_position(), 13);
}

#[test]
fn late_value_destination_failure_and_dropped_plan_leave_both_buffers_unchanged() {
    let (capture, _, _) = captured();
    let kl = layout(ScalarEncoding::Binary16, 3, 0);
    let vl = layout(ScalarEncoding::BFloat16, 3, 0);
    let mut kd = vec![0xaa; 12];
    let mut short = vec![0xbb; 11];
    assert_eq!(capture.prepare_restore(1, KvDestination::Separate {
        keys: HostTensorMut { identity: buffer(7), layout: &kl, bytes: &mut kd },
        values: HostTensorMut { identity: buffer(8), layout: &vl, bytes: &mut short },
    }, window()).unwrap_err(), Error::Incomplete);
    assert_eq!(kd, vec![0xaa; 12]);
    assert_eq!(short, vec![0xbb; 11]);
    let mut vd = vec![0xbb; 12];
    drop(capture.prepare_restore(1, KvDestination::Separate {
        keys: HostTensorMut { identity: buffer(7), layout: &kl, bytes: &mut kd },
        values: HostTensorMut { identity: buffer(8), layout: &vl, bytes: &mut vd },
    }, window()).unwrap());
    assert_eq!(kd, vec![0xaa; 12]);
    assert_eq!(vd, vec![0xbb; 12]);
}

#[test]
fn shared_storage_restores_disjoint_spans_and_refuses_aliases() {
    let (capture, kb, vb) = captured();
    let kl = layout(ScalarEncoding::Binary16, 3, 0);
    let vl = layout(ScalarEncoding::BFloat16, 3, 16);
    let mut storage = vec![0xcc; 32];
    capture.prepare_restore(1, KvDestination::Shared {
        identity: buffer(3), keys: &kl, values: &vl, bytes: &mut storage,
    }, window()).unwrap().commit();
    assert_eq!(&storage[..12], kb);
    assert_eq!(&storage[16..28], vb);
    assert_eq!(&storage[12..16], &[0xcc; 4]);
    let before = storage.clone();
    let overlap = layout(ScalarEncoding::BFloat16, 3, 10);
    assert_eq!(capture.prepare_restore(1, KvDestination::Shared {
        identity: buffer(3), keys: &kl, values: &overlap, bytes: &mut storage,
    }, window()).unwrap_err(), Error::Binding);
    assert_eq!(storage, before);
}

#[test]
fn restores_a_later_source_page_into_a_permuted_padded_destination() {
    let (capture, kb, vb) = captured();
    // Physical channel-major, with a gap between the two channels.
    let kl = TensorLayout::new([1, 2, 1, 2], [0, 2, 0, 8], 1,
        ScalarEncoding::Binary16, ByteOrder::Little).unwrap();
    let vl = TensorLayout::new([1, 2, 1, 2], [0, 2, 0, 8], 1,
        ScalarEncoding::BFloat16, ByteOrder::Little).unwrap();
    let mut kd = vec![0xee; 16];
    let mut vd = vec![0xee; 16];
    let w = KvRestoreWindow { first_position: 11, token_count: 2, buffer_first_position: 11, ..window() };
    capture.prepare_restore(1, KvDestination::Separate {
        keys: HostTensorMut { identity: buffer(7), layout: &kl, bytes: &mut kd },
        values: HostTensorMut { identity: buffer(8), layout: &vl, bytes: &mut vd },
    }, w).unwrap().commit();
    for (actual, original) in [(&kd, &kb), (&vd, &vb)] {
        let mut expected = vec![0xee; 16];
        for token in 0..2 { for channel in 0..2 {
            let from = (token + 1) * 4 + channel * 2;
            let to = 1 + token * 2 + channel * 8;
            expected[to..to + 2].copy_from_slice(&original[from..from + 2]);
        } }
        assert_eq!(actual, &expected);
    }
}

#[test]
fn stale_revision_missing_source_and_wrong_absolute_destination_do_not_write() {
    let (capture, _, _) = captured();
    let kl = layout(ScalarEncoding::Binary16, 3, 0);
    let vl = layout(ScalarEncoding::BFloat16, 3, 0);
    for (revision, w, error) in [
        (0, window(), Error::Stale),
        (1, KvRestoreWindow { first_position: 9, buffer_first_position: 9, ..window() }, Error::Missing),
        (1, KvRestoreWindow { buffer_first_position: 9, ..window() }, Error::Binding),
        (1, KvRestoreWindow { token_count: 0, ..window() }, Error::InvalidInput),
        (1, KvRestoreWindow { token_count: 4, ..window() }, Error::Missing),
    ] {
        let mut kd = vec![0xaa; 12];
        let mut vd = vec![0xbb; 12];
        assert_eq!(capture.prepare_restore(revision, KvDestination::Separate {
            keys: HostTensorMut { identity: buffer(7), layout: &kl, bytes: &mut kd },
            values: HostTensorMut { identity: buffer(8), layout: &vl, bytes: &mut vd },
        }, w).unwrap_err(), error);
        assert_eq!(kd, vec![0xaa; 12]);
        assert_eq!(vd, vec![0xbb; 12]);
    }
}

#[test]
fn single_tensor_refuses_late_rounding_and_source_identity_substitution() {
    let contract = contract(4, ScalarEncoding::Binary16);
    let layout = layout(ScalarEncoding::Binary16, 1, 0);
    let mut destination = vec![0xaa; 4];
    let selected = selection();
    let source = SourceFrame::capture(contract.frame_identity(selected).unwrap(), &[1.0, f32::from_bits(0x3f80_0001)]).unwrap();
    assert_eq!(contract.prepare_restore(&source, HostTensorMut {
        identity: buffer(7), layout: &layout, bytes: &mut destination,
    }, selected).unwrap_err(), Error::Binding);
    assert_eq!(destination, vec![0xaa; 4]);
    let source = SourceFrame::capture(contract.frame_identity(selected).unwrap(), &[1.0, -0.0]).unwrap();
    assert_eq!(contract.prepare_restore(&source, HostTensorMut {
        identity: buffer(7), layout: &layout, bytes: &mut destination,
    }, TokenSelection { sequence: 10, ..selected }).unwrap_err(), Error::Binding);
    contract.prepare_restore(&source, HostTensorMut {
        identity: buffer(7), layout: &layout, bytes: &mut destination,
    }, selected).unwrap().commit();
    assert_eq!(destination, [0, 0x3c, 0, 0x80]);
}
