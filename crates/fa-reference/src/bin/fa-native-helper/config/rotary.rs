//! The operator pins numerical semantics independently of model-file metadata.
//! This required /3 object has no implicit/default mode or dynamic fallback.
use super::{Json, LaunchError, count, object, scalar};
use fa_reference::action::consequence::activation::tensor::kv::decoder::RotaryScaling;

pub(super) fn parse(value: &Json) -> Result<RotaryScaling, LaunchError> {
    match value.get("kind").and_then(Json::as_str) {
        Some("none") => {
            object(value, &["kind"], "decoder.rotary")?;
            Ok(RotaryScaling::None)
        }
        Some("linear") => {
            let fields = object(value, &["kind", "factor"], "decoder.rotary")?;
            RotaryScaling::linear(scalar(&fields["factor"], "decoder.rotary.factor")?)
                .map_err(LaunchError::Contract)
        }
        Some("llama3") => {
            let fields = object(value, &["kind", "factor", "low_freq_factor", "high_freq_factor",
                "original_max_position_embeddings"], "decoder.rotary")?;
            RotaryScaling::llama3(scalar(&fields["factor"], "decoder.rotary.factor")?,
                scalar(&fields["low_freq_factor"], "decoder.rotary.low_freq_factor")?,
                scalar(&fields["high_freq_factor"], "decoder.rotary.high_freq_factor")?,
                count(fields, "original_max_position_embeddings")?).map_err(LaunchError::Contract)
        }
        _ => Err(LaunchError::Field("decoder.rotary.kind")),
    }
}
