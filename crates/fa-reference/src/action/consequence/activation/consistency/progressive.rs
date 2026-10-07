//! Source-certified coarse-to-fine forecasts using the ORIGINAL binary32 codec.
//! Unseen bits remain intervals. No decoded midpoint, MSE, alternate calibration,
//! caller verdict or exact-score fallback can replace an unresolved interval.
use super::{ForecastModel, Prediction, ProbeOutcome, ProgressiveFrame, SourceFrame};
use crate::action::consequence::activation::{HEADER_BYTES, MAX_VALUES};
use crate::Error;

/// At most 24 levels: initial zero bits followed by every mantissa bit. Total
/// data is at most 32 bits per coordinate, plus each header and rounding byte.
/// This bounds logical encoded bytes, not transient copies or physical work.
pub const MAX_PROGRESSIVE_FORECAST_BYTES: usize = 4 * MAX_VALUES + 24 * (HEADER_BYTES + 1);

/// Freeze precision and total packet-byte ceilings before forecasting. A cap
/// below full precision intentionally permits refusal, never an invented band.
/// The source is already held by its capture owner; this is not a neural codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressiveForecastPolicy {
    initial_bits: u8,
    refinement_bits: u8,
    maximum_bits: u8,
    max_encoded_bytes: usize,
}
impl ProgressiveForecastPolicy {
    pub fn new(initial_bits: u8, refinement_bits: u8, maximum_bits: u8,
        max_encoded_bytes: usize) -> Result<Self, Error>
    {
        if initial_bits > maximum_bits || maximum_bits > 23
            || refinement_bits == 0 || refinement_bits > 23 || max_encoded_bytes == 0 {
            return Err(Error::InvalidInput);
        }
        if max_encoded_bytes > MAX_PROGRESSIVE_FORECAST_BYTES { return Err(Error::Limit); }
        Ok(Self { initial_bits, refinement_bits, maximum_bits, max_encoded_bytes })
    }
    pub fn initial_bits(self) -> u8 { self.initial_bits }
    pub fn refinement_bits(self) -> u8 { self.refinement_bits }
    pub fn maximum_bits(self) -> u8 { self.maximum_bits }
    pub fn max_encoded_bytes(self) -> usize { self.max_encoded_bytes }
}

impl ForecastModel {
    /// Immutable per-prediction acquisition policy; lifetime prediction budgets
    /// remain in the original broker. No second probability table is installed.
    /// The exact baseline stays the default, and duplicate selection refuses.
    pub fn with_progressive(mut self, policy: ProgressiveForecastPolicy) -> Result<Self, Error> {
        if self.progressive.is_some() { return Err(Error::Duplicate); }
        self.progressive = Some(policy);
        Ok(self)
    }
    pub fn progressive_policy(&self) -> Option<ProgressiveForecastPolicy> { self.progressive }
}

pub(super) fn predict(model: &ForecastModel, source: &SourceFrame,
    policy: ProgressiveForecastPolicy) -> Result<Prediction, Error>
{
    if source.identity().profile != model.profile() || source.dimensions() != model.dimensions() {
        return Err(Error::Binding);
    }
    let mut from = None;
    let mut bits = policy.initial_bits;
    let mut spent = 0_usize;
    let mut frame: Option<ProgressiveFrame> = None;
    loop {
        // Preflight BEFORE encoding, verification or scoring this level. Count
        // every earlier initial/delta header and padding byte, not only the last
        // packet. Verification re-encodes original bytes and adds physical cost.
        let cost = source.encoded_len(from, bits)?;
        let total = spent.checked_add(cost).ok_or(Error::Overflow)?;
        if total > policy.max_encoded_bytes { return Err(Error::Limit); }
        let encoded = match from {
            None => source.encode_initial(bits)?,
            Some(previous) => source.encode_refinement(previous, bits)?,
        };
        let checked = source.verify_block(&encoded)?;
        match &mut frame {
            None => frame = Some(ProgressiveFrame::from_initial(&checked)?),
            Some(frame) => frame.refine(&checked)?,
        }
        spent = total;
        let observation = model.probe.evaluate(frame.as_ref().expect("initial frame admitted"))?;
        if observation.outcome() != ProbeOutcome::NeedsRefinement {
            // The SAME selector as the exact baseline. A nondegenerate interval
            // strictly on one side is sufficient; equality needs exact [0, 0].
            return model.prediction(observation, spent);
        }
        if bits == policy.maximum_bits { return Err(Error::Incomplete); }
        from = Some(bits);
        bits = bits.saturating_add(policy.refinement_bits).min(policy.maximum_bits);
    }
}

#[cfg(test)]
mod tests;
