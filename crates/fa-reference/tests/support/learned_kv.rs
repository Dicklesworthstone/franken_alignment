//! Synthetic bounded F32 tensors through the original checked all-layer capture.
#![allow(dead_code)]
use fa_reference::action::consequence::activation::CaptureProfile;
use fa_reference::action::consequence::activation::tensor::{BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorContract, TensorLayout};
use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvContract};
use fa_reference::action::consequence::activation::tensor::kv::model::{ModelKvBudget, ModelKvCapture, ModelKvImage, ModelKvProfile};
use std::collections::BTreeMap;

pub fn image(stream: u64, heads: usize, channels: usize, rows: &[Vec<f32>]) -> ModelKvImage {
    assert!(!rows.is_empty()); assert!(rows.iter().all(|row| row.len() == heads * channels));
    let tensor = |tap| TensorContract::new(CaptureProfile { tenant: 1, model: 2, model_generation: 3,
        tap, layout_generation: 1 }, ScalarEncoding::Binary32, ByteOrder::Little, heads, channels).unwrap();
    let profile = ModelKvProfile::new(9, 1, BTreeMap::from([(1,
        KvContract::new(tensor(4), tensor(5), heads * 2).unwrap())])).unwrap();
    let n = rows.len(); let width = heads * channels;
    let layout = TensorLayout::new([1, n, heads, channels], [n * width * 4, width * 4, channels * 4, 4],
        0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let keys: Vec<u8> = rows.iter().flatten().flat_map(|value| value.to_le_bytes()).collect();
    let values = keys.clone();
    let mut capture = ModelKvCapture::new(profile, stream, 0, 0, 1,
        ModelKvBudget { positions: n, normalized_values: n * width * 2 }).unwrap();
    capture.append(0, BTreeMap::from([(1, KvAppend {
        keys: HostTensor { identity: BufferIdentity { object: 1, generation: 1 }, layout: &layout, bytes: &keys },
        values: HostTensor { identity: BufferIdentity { object: 2, generation: 1 }, layout: &layout, bytes: &values },
        first_token: 0, token_count: n, first_sequence: 1, buffer_first_position: 0,
    })])).unwrap();
    capture.snapshot(1).unwrap()
}
pub fn line(stream: u64, channels: usize, rows: usize) -> ModelKvImage {
    image(stream, 1, channels, &(0..rows).map(|i| {
        let x = i as f32 - (rows - 1) as f32 / 2.0;
        let mut row = vec![0.0; channels]; row[0] = x;
        if channels > 1 { row[1] = 2.0 * x; }
        row
    }).collect::<Vec<_>>())
}
