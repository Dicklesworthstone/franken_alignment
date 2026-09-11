//! Real checked tensor fixtures; values are synthetic, not model execution.
#![allow(dead_code)]

use fa_reference::action::consequence::activation::CaptureProfile;
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorContract, TensorLayout,
};
use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvContract};
use fa_reference::action::consequence::activation::tensor::kv::model::{
    ModelKvBudget, ModelKvCapture, ModelKvImage, ModelKvProfile,
};
use std::collections::BTreeMap;

pub fn profile() -> ModelKvProfile {
    let tensor = |tap, encoding, heads, channels| TensorContract::new(
        CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap, layout_generation: 8 },
        encoding, ByteOrder::Little, heads, channels,
    ).unwrap();
    ModelKvProfile::new(9, 1, BTreeMap::from([
        (10, KvContract::new(tensor(4, ScalarEncoding::Binary16, 1, 2),
            tensor(5, ScalarEncoding::BFloat16, 1, 2), 2).unwrap()),
        (20, KvContract::new(tensor(6, ScalarEncoding::Binary32, 2, 1),
            tensor(7, ScalarEncoding::Binary32, 2, 1), 4).unwrap()),
    ])).unwrap()
}

pub fn capture() -> ModelKvCapture {
    ModelKvCapture::new(profile(), 11, 0, 0, 1,
        ModelKvBudget { positions: 4, normalized_values: 32 }).unwrap()
}

pub struct Buffers {
    pub keys_a: Vec<u8>,
    pub values_a: Vec<u8>,
    pub keys_b: Vec<u8>,
    pub values_b: Vec<u8>,
    pub ka: TensorLayout,
    pub va: TensorLayout,
    pub kb: TensorLayout,
    pub vb: TensorLayout,
}

impl Buffers {
    pub fn new() -> Self {
        let half = |encoding| TensorLayout::new([1, 2, 1, 2], [8, 4, 4, 2], 0,
            encoding, ByteOrder::Little).unwrap();
        let full = || TensorLayout::new([1, 2, 2, 1], [16, 8, 4, 4], 0,
            ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
        Self {
            keys_a: [0x3c00_u16, 0x4000, 0x4200, 0x4400].into_iter().flat_map(u16::to_le_bytes).collect(),
            values_a: [0x40a0_u16, 0x40c0, 0x40e0, 0x4100].into_iter().flat_map(u16::to_le_bytes).collect(),
            keys_b: [9_f32, 10.0, 11.0, 12.0].into_iter().flat_map(f32::to_le_bytes).collect(),
            values_b: [13_f32, 14.0, 15.0, 16.0].into_iter().flat_map(f32::to_le_bytes).collect(),
            ka: half(ScalarEncoding::Binary16), va: half(ScalarEncoding::BFloat16), kb: full(), vb: full(),
        }
    }

    pub fn requests(&self, first_token: usize, token_count: usize, first_sequence: u64) -> BTreeMap<u64, KvAppend<'_>> {
        fn host<'a>(object: u64, layout: &'a TensorLayout, bytes: &'a [u8]) -> HostTensor<'a> {
            HostTensor { identity: BufferIdentity { object, generation: 1 }, layout, bytes }
        }
        BTreeMap::from([
            (10, KvAppend { keys: host(1, &self.ka, self.keys_a.as_slice()),
                values: host(2, &self.va, self.values_a.as_slice()),
                first_token, token_count, first_sequence, buffer_first_position: 0 }),
            (20, KvAppend { keys: host(3, &self.kb, self.keys_b.as_slice()),
                values: host(4, &self.vb, self.values_b.as_slice()),
                first_token, token_count, first_sequence, buffer_first_position: 0 }),
        ])
    }
}

pub fn image() -> ModelKvImage {
    let mut capture = capture();
    capture.append(0, Buffers::new().requests(0, 2, 1)).unwrap();
    capture.snapshot(1).unwrap()
}
