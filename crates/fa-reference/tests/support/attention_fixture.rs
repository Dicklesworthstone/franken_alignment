//! Supplied numerical fixtures captured through the actual public tensor API.
//! No native model execution or host qualification is implied.

use fa_reference::action::consequence::activation::CaptureProfile;
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorCapture,
    TensorContract, TensorLayout, TokenSelection,
};
use fa_reference::action::consequence::activation::tensor::kv::{
    KvAppend, KvBudget, KvCapture, KvContract,
};
use fa_reference::action::consequence::activation::tensor::kv::image::KvImage;
use fa_reference::action::consequence::activation::tensor::kv::attention::{
    AttentionBudget, AttentionContract, AttentionMask, MAX_ATTENTION_PRODUCTS,
    MAX_ATTENTION_RESOLUTION_STEPS, MAX_ATTENTION_WORKSPACE_BYTES,
};

pub fn contract(heads: usize, queries: usize, keys: usize, values: usize, mask: AttentionMask) -> AttentionContract {
    let profile = CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 };
    let tensor = |tap, h, channels| TensorContract::new(CaptureProfile { tap, ..profile },
        ScalarEncoding::Binary32, ByteOrder::Little, h, channels).unwrap();
    let cache = KvContract::new(tensor(4, heads, keys), tensor(6, heads, values), queries).unwrap();
    AttentionContract::new(9, 1, tensor(8, queries, keys), cache, mask, 1.0).unwrap()
}

pub fn image(contract: &AttentionContract, first: u64, keys: &[f32], values: &[f32]) -> KvImage {
    let k = contract.cache().keys();
    let v = contract.cache().values();
    assert_eq!(keys.len() % k.dimensions(), 0);
    let count = keys.len() / k.dimensions();
    assert_eq!(values.len(), count * v.dimensions());
    let mut capture = KvCapture::new(contract.cache().clone(), 7, 0, first, first + 1,
        KvBudget { positions: 4096, normalized_values: 1_048_576 }).unwrap();
    if count > 0 {
        let layout = |c: &TensorContract| TensorLayout::new([1, count, c.heads(), c.channels()],
            [count * c.dimensions() * 4, c.dimensions() * 4, c.channels() * 4, 4],
            0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
        let kl = layout(k);
        let vl = layout(v);
        let kb: Vec<_> = keys.iter().flat_map(|x| x.to_le_bytes()).collect();
        let vb: Vec<_> = values.iter().flat_map(|x| x.to_le_bytes()).collect();
        capture.append(0, KvAppend {
            keys: HostTensor { identity: BufferIdentity { object: 1, generation: 1 }, layout: &kl, bytes: &kb },
            values: HostTensor { identity: BufferIdentity { object: 2, generation: 1 }, layout: &vl, bytes: &vb },
            first_token: 0, token_count: count, buffer_first_position: first, first_sequence: first + 1,
        }).unwrap();
    }
    capture.snapshot(capture.revision()).unwrap()
}

pub fn query(contract: &AttentionContract, position: u64, values: &[f32]) -> TensorCapture {
    let q = contract.queries();
    assert_eq!(values.len(), q.dimensions());
    let layout = TensorLayout::new([1, 1, q.heads(), q.channels()],
        [q.dimensions() * 4, q.dimensions() * 4, q.channels() * 4, 4],
        0, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let bytes: Vec<_> = values.iter().flat_map(|x| x.to_le_bytes()).collect();
    q.capture(HostTensor { identity: BufferIdentity { object: 3, generation: 1 }, layout: &layout, bytes: &bytes },
        TokenSelection { batch: 0, token: 0, first_position: position, stream: 7, sequence: position + 1 }).unwrap()
}

pub fn budget() -> AttentionBudget {
    AttentionBudget { scalar_products: MAX_ATTENTION_PRODUCTS,
        resolution_steps: MAX_ATTENTION_RESOLUTION_STEPS, workspace_bytes: MAX_ATTENTION_WORKSPACE_BYTES }
}
