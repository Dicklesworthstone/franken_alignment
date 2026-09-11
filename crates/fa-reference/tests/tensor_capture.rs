//! Public CPU-byte capture, no serving-host or GPU synchronization claims.

use fa_reference::action::consequence::activation::{CaptureProfile, ProgressiveFrame};
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, RefinementBudget, RefinementMonitor};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorCapture, TensorContract,
    TensorLayout, TokenSelection, MAX_HOST_BYTES,
};
use fa_reference::Error;

fn profile() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 }
}
fn contract(heads: usize, channels: usize) -> TensorContract {
    TensorContract::new(profile(), ScalarEncoding::Binary32, ByteOrder::Little, heads, channels).unwrap()
}
fn selection() -> TokenSelection {
    TokenSelection { batch: 0, token: 0, first_position: 100, stream: 6, sequence: 1 }
}
fn buffer<'a>(layout: &'a TensorLayout, bytes: &'a [u8]) -> HostTensor<'a> {
    HostTensor { identity: BufferIdentity { object: 7, generation: 8 }, layout, bytes }
}
fn values(capture: &TensorCapture) -> Vec<f32> {
    let source = capture.source();
    let block = source.verify_block(&source.encode_initial(23).unwrap()).unwrap();
    ProgressiveFrame::from_initial(&block).unwrap().exact_values().unwrap()
}
fn scalar(batch: usize, token: usize, head: usize, channel: usize) -> f32 {
    (1000 * batch + 100 * token + 10 * head + channel) as f32
}

#[test]
fn all_physical_axis_orders_preserve_canonical_head_channel_order() {
    let shape = [2, 3, 2, 2];
    let mut permutations = 0;
    for a in 0..4 { for b in 0..4 { for c in 0..4 { for d in 0..4 {
        let order = [a, b, c, d];
        if order.iter().copied().collect::<std::collections::BTreeSet<_>>().len() != 4 { continue; }
        permutations += 1;
        let mut strides = [0; 4];
        let mut span = 4;
        for axis in order { strides[axis] = span; span *= shape[axis]; }
        let layout = TensorLayout::new(shape, strides, 3, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
        let mut bytes = vec![0; layout.byte_range().end];
        for batch in 0..shape[0] { for token in 0..shape[1] {
            for head in 0..shape[2] { for channel in 0..shape[3] {
                let offset = 3 + batch * strides[0] + token * strides[1] + head * strides[2] + channel * strides[3];
                bytes[offset..offset + 4].copy_from_slice(&scalar(batch, token, head, channel).to_le_bytes());
            }}
            let chosen = TokenSelection { batch, token, ..selection() };
            // Fill all coordinates of this token before reading its strided view.
            let capture = contract(2, 2).capture(buffer(&layout, &bytes), chosen).unwrap();
            let expected: Vec<_> = (0..2).flat_map(|h| (0..2).map(move |c| scalar(batch, token, h, c))).collect();
            assert_eq!(values(&capture), expected);
            assert_eq!(capture.source().identity().position, 100 + token as u64);
            assert_eq!(capture.receipt().source_bytes_read(), 16);
        }}
    }}}}
    assert_eq!(permutations, 24);
}

#[test]
fn padded_unaligned_capture_owns_values_after_backing_buffer_is_recycled() {
    let layout = TensorLayout::new([1, 2, 2, 2], [0, 48, 16, 4], 1,
        ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let mut bytes = vec![255; layout.byte_range().end];
    for (offset, value) in [(49, 1.0_f32), (53, -0.0), (65, 3.0), (69, 4.0)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    // Nonfinite contents in the unselected token/padding are not observed values.
    let capture = contract(2, 2).capture(buffer(&layout, &bytes), TokenSelection { token: 1, ..selection() }).unwrap();
    bytes.fill(0);
    assert_eq!(values(&capture).iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        vec![1.0_f32.to_bits(), (-0.0_f32).to_bits(), 3.0_f32.to_bits(), 4.0_f32.to_bits()]);
    assert_eq!(capture.receipt().buffer(), BufferIdentity { object: 7, generation: 8 });
    assert_eq!(capture.receipt().normalized_bytes(), 16);
}

#[test]
fn each_encoding_and_byte_order_produces_the_same_registered_probe_alarm() {
    for encoding in [ScalarEncoding::Binary32, ScalarEncoding::Binary16, ScalarEncoding::BFloat16] {
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let bytes = match (encoding, order) {
                (ScalarEncoding::Binary32, ByteOrder::Little) => 1.5_f32.to_le_bytes().to_vec(),
                (ScalarEncoding::Binary32, ByteOrder::Big) => 1.5_f32.to_be_bytes().to_vec(),
                (ScalarEncoding::Binary16, ByteOrder::Little) => 0x3e00_u16.to_le_bytes().to_vec(),
                (ScalarEncoding::Binary16, ByteOrder::Big) => 0x3e00_u16.to_be_bytes().to_vec(),
                (ScalarEncoding::BFloat16, ByteOrder::Little) => 0x3fc0_u16.to_le_bytes().to_vec(),
                (ScalarEncoding::BFloat16, ByteOrder::Big) => 0x3fc0_u16.to_be_bytes().to_vec(),
            };
            let layout = TensorLayout::new([1; 4], [0; 4], 0, encoding, order).unwrap();
            let contract = TensorContract::new(profile(), encoding, order, 1, 1).unwrap();
            let capture = contract.capture(buffer(&layout, &bytes), selection()).unwrap();
            assert_eq!(values(&capture), vec![1.5]);
            let probe = LinearProbe::new(1, 1, profile(), &[1.0], 0.0, 1.25).unwrap();
            let monitor = RefinementMonitor::new(vec![probe], vec![0, 23], RefinementBudget {
                encoded_bytes: 1000, probe_coordinates: 2,
            }).unwrap();
            assert_eq!(monitor.analyze(capture.source()).unwrap().outcome(), MonitorOutcome::Alarm);
        }
    }
}

#[test]
fn malformed_layouts_aliases_and_checked_arithmetic_refuse() {
    for (shape, strides) in [([1, 1, 2, 2], [0, 0, 4, 4]),
        ([1, 1, 2, 2], [0, 0, 8, 0]), ([1, 0, 2, 2], [0, 0, 8, 4]),
        ([1, 1, 2, 2], [0, 0, 8, 2])]
    {
        assert_eq!(TensorLayout::new(shape, strides, 0, ScalarEncoding::Binary32, ByteOrder::Little), Err(Error::InvalidInput));
    }
    assert_eq!(TensorLayout::new([2; 4], [usize::MAX; 4], 0,
        ScalarEncoding::Binary32, ByteOrder::Little), Err(Error::Overflow));
    assert_eq!(TensorLayout::new([1; 4], [0; 4], MAX_HOST_BYTES - 3,
        ScalarEncoding::Binary32, ByteOrder::Little), Err(Error::Limit));
    assert_eq!(TensorLayout::new([usize::MAX, 2, 1, 1], [8, 4, 0, 0], 0,
        ScalarEncoding::Binary32, ByteOrder::Little), Err(Error::Overflow));
    assert!(TensorLayout::new([1; 4], [0; 4], 0, ScalarEncoding::Binary32, ByteOrder::Little).is_ok());
}

#[test]
fn truncated_storage_invalid_selection_and_wrong_contract_are_not_partial_captures() {
    let layout = TensorLayout::new([1, 2, 1, 1], [0, 8, 0, 0], 0,
        ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let bytes = [0_u8; 12];
    for end in 0..12 { assert_eq!(contract(1, 1).capture(buffer(&layout, &bytes[..end]), selection()).unwrap_err(), Error::Incomplete); }
    assert!(contract(1, 1).capture(buffer(&layout, &bytes), selection()).is_ok());
    for chosen in [TokenSelection { batch: 1, ..selection() }, TokenSelection { token: 2, ..selection() }] {
        assert_eq!(contract(1, 1).capture(buffer(&layout, &bytes), chosen).unwrap_err(), Error::InvalidInput);
    }
    assert_eq!(contract(2, 1).capture(buffer(&layout, &bytes), selection()).unwrap_err(), Error::Binding);
    let wrong = TensorContract::new(profile(), ScalarEncoding::BFloat16, ByteOrder::Little, 1, 1).unwrap();
    assert_eq!(wrong.capture(buffer(&layout, &bytes), selection()).unwrap_err(), Error::Binding);
    assert_eq!(contract(1, 1).frame_identity(TokenSelection { token: 1, first_position: u64::MAX, ..selection() }), Err(Error::Overflow));
}

#[test]
fn nonfinite_selected_values_and_absent_buffer_identity_refuse() {
    let layout = TensorLayout::new([1; 4], [0; 4], 0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(contract(1, 1).capture(buffer(&layout, &value.to_le_bytes()), selection()).unwrap_err(), Error::InvalidInput);
    }
    let bytes = 1_f32.to_le_bytes();
    let invalid = HostTensor { identity: BufferIdentity { object: 7, generation: 0 }, layout: &layout, bytes: &bytes };
    assert_eq!(contract(1, 1).capture(invalid, selection()).unwrap_err(), Error::InvalidInput);
    assert!(contract(1, 1).capture(buffer(&layout, &bytes), selection()).is_ok());
}
