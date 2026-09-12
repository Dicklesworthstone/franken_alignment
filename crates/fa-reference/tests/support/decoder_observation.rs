//! Read complete source values through the existing public verification path.
use fa_reference::action::consequence::activation::{ProgressiveFrame, SourceFrame};

pub fn words(source: &SourceFrame) -> Vec<u32> {
    let bytes = source.encode_initial(23).unwrap();
    let verified = source.verify_block(&bytes).unwrap();
    ProgressiveFrame::from_initial(&verified).unwrap().exact_values().unwrap()
        .into_iter().map(f32::to_bits).collect()
}
