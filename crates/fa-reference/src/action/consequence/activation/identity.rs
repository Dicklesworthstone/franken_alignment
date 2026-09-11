//! Model-space manifests and bounded activation fingerprints (plan 7.10).
//!
//! Manifest commitments and reference stimuli are trusted registration inputs.
//! This module checks structural identity and actual finite binary32 values;
//! it does not authenticate a host, verify signatures, or run model inference.

use super::{CaptureProfile, FrameIdentity, SourceFrame, MAX_VALUES};
use crate::Error;
use std::collections::BTreeMap;

pub const MAX_ANCHORS: usize = 16;
pub const MAX_STIMULUS_TOKENS: usize = 8_192;

/// Supplied commitments, not digests computed or authenticated by this module.
/// Even an empty adapter set has a nonzero, registered commitment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelManifest {
    pub tenant: u64,
    pub model: u64,
    pub model_generation: u64,
    pub host_generation: u64,
    pub tokenizer_generation: u64,
    pub weights: [u8; 32],
    pub adapters: [u8; 32],
    pub tokenizer: [u8; 32],
    pub architecture: [u8; 32],
    pub numeric_profile: [u8; 32],
}

impl ModelManifest {
    fn validate(&self) -> Result<(), Error> {
        if [self.tenant, self.model, self.model_generation, self.host_generation,
            self.tokenizer_generation].contains(&0)
            || [&self.weights, &self.adapters, &self.tokenizer, &self.architecture,
                &self.numeric_profile].iter().any(|digest| **digest == [0; 32])
        {
            return Err(Error::InvalidInput);
        }
        Ok(())
    }
}

/// Inclusive per-coordinate acceptance intervals, registered before capture.
/// Endpoint comparison uses no subtraction, average error, or rounded norm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityAnchor {
    id: u64,
    profile: CaptureProfile,
    stream: u64,
    stimulus: Vec<u32>,
    bounds: Vec<[u32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnchorOutlier {
    pub coordinate: usize,
    pub observed_bits: u32,
    pub lower_bits: u32,
    pub upper_bits: u32,
}

/// Computed evidence only. No constructor accepts an asserted match result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorObservation {
    anchor: u64,
    frame: FrameIdentity,
    coordinates: usize,
    outside: usize,
    first_outlier: Option<AnchorOutlier>,
}

impl AnchorObservation {
    pub fn anchor(&self) -> u64 { self.anchor }
    pub fn frame(&self) -> FrameIdentity { self.frame }
    pub fn coordinates(&self) -> usize { self.coordinates }
    pub fn outside(&self) -> usize { self.outside }
    pub fn first_outlier(&self) -> Option<AnchorOutlier> { self.first_outlier }
}

impl IdentityAnchor {
    pub fn new(
        id: u64, profile: CaptureProfile, stream: u64,
        stimulus: Vec<u32>, bounds: &[[f32; 2]],
    ) -> Result<Self, Error> {
        if [id, profile.tenant, profile.model, profile.model_generation, profile.tap,
            profile.layout_generation, stream].contains(&0)
            || stimulus.is_empty() || bounds.is_empty()
        {
            return Err(Error::InvalidInput);
        }
        if stimulus.len() > MAX_STIMULUS_TOKENS || bounds.len() > MAX_VALUES {
            return Err(Error::Limit);
        }
        if bounds.iter().any(|[lo, hi]| !lo.is_finite() || !hi.is_finite() || lo > hi) {
            return Err(Error::InvalidInput);
        }
        Ok(Self { id, profile, stream, stimulus,
            bounds: bounds.iter().map(|[lo, hi]| [lo.to_bits(), hi.to_bits()]).collect() })
    }

    pub fn id(&self) -> u64 { self.id }
    pub fn profile(&self) -> CaptureProfile { self.profile }
    pub fn stream(&self) -> u64 { self.stream }
    pub fn stimulus(&self) -> &[u32] { &self.stimulus }
    pub fn dimensions(&self) -> usize { self.bounds.len() }
    pub fn bound_bits(&self) -> &[[u32; 2]] { &self.bounds }

    pub fn compare(&self, source: &SourceFrame) -> Result<AnchorObservation, Error> {
        let frame = source.identity();
        if frame.profile != self.profile || frame.stream != self.stream
            || frame.position != (self.stimulus.len() - 1) as u64
            || source.dimensions() != self.bounds.len()
        {
            return Err(Error::Binding);
        }
        let mut outside = 0;
        let mut first_outlier = None;
        // SourceFrame owns validated finite bits; no mutable host buffer is read.
        for (coordinate, (&word, &[lower_bits, upper_bits])) in source.words.iter()
            .zip(&self.bounds).enumerate()
        {
            let value = f32::from_bits(word);
            if value < f32::from_bits(lower_bits) || value > f32::from_bits(upper_bits) {
                outside += 1;
                first_outlier.get_or_insert(AnchorOutlier {
                    coordinate, observed_bits: word, lower_bits, upper_bits,
                });
            }
        }
        Ok(AnchorObservation { anchor: self.id, frame, coordinates: self.bounds.len(),
            outside, first_outlier })
    }
}

/// An immutable reference passport, NOT a signed production passport. All
/// anchors are mandatory. Its constructor does not establish discriminatory
/// power against arbitrary substitutions or compatibility of restart kernels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelPassport {
    id: u64,
    generation: u64,
    manifest: ModelManifest,
    anchors: BTreeMap<u64, IdentityAnchor>,
}

impl ModelPassport {
    pub fn new(
        id: u64, generation: u64, manifest: ModelManifest, anchors: Vec<IdentityAnchor>,
    ) -> Result<Self, Error> {
        manifest.validate()?;
        if id == 0 || generation == 0 || anchors.is_empty() { return Err(Error::InvalidInput); }
        if anchors.len() > MAX_ANCHORS { return Err(Error::Limit); }
        let mut retained = BTreeMap::new();
        let mut coordinates = 0_usize;
        let mut tokens = 0_usize;
        for anchor in anchors {
            let profile = anchor.profile();
            if profile.tenant != manifest.tenant || profile.model != manifest.model
                || profile.model_generation != manifest.model_generation
            {
                return Err(Error::Binding);
            }
            coordinates = coordinates.checked_add(anchor.dimensions()).ok_or(Error::Limit)?;
            tokens = tokens.checked_add(anchor.stimulus().len()).ok_or(Error::Limit)?;
            if coordinates > MAX_VALUES || tokens > MAX_STIMULUS_TOKENS { return Err(Error::Limit); }
            if retained.insert(anchor.id(), anchor).is_some() { return Err(Error::Duplicate); }
        }
        Ok(Self { id, generation, manifest, anchors: retained })
    }
    pub fn id(&self) -> u64 { self.id }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn manifest(&self) -> &ModelManifest { &self.manifest }
    pub fn anchors(&self) -> &BTreeMap<u64, IdentityAnchor> { &self.anchors }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> CaptureProfile {
        CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 }
    }
    fn manifest() -> ModelManifest {
        ModelManifest { tenant: 1, model: 2, model_generation: 3, host_generation: 1,
            tokenizer_generation: 1, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
            architecture: [4; 32], numeric_profile: [5; 32] }
    }
    fn anchor(bounds: &[[f32; 2]]) -> IdentityAnchor {
        IdentityAnchor::new(1, profile(), 6, vec![10, 20], bounds).unwrap()
    }
    fn source(values: &[f32]) -> SourceFrame {
        SourceFrame::capture(FrameIdentity { profile: profile(), stream: 6, sequence: 1, position: 1 }, values).unwrap()
    }

    #[test]
    fn inclusive_bounds_allow_registered_drift_but_not_an_average_that_hides_an_outlier() {
        let anchor = anchor(&[[1.0, 2.0], [-2.0, -1.0]]);
        for values in [[1.0, -2.0], [2.0, -1.0], [1.5, -1.5]] {
            let observation = anchor.compare(&source(&values)).unwrap();
            assert_eq!(observation.outside(), 0);
            assert_eq!(observation.coordinates(), 2);
            assert_eq!(observation.first_outlier(), None);
        }
        let observation = anchor.compare(&source(&[0.5, -2.5])).unwrap();
        assert_eq!(observation.outside(), 2);
        assert_eq!(observation.first_outlier().unwrap().observed_bits, 0.5_f32.to_bits());
        assert_eq!(observation.first_outlier().unwrap().coordinate, 0);
    }

    #[test]
    fn signed_zero_subnormals_and_maximum_values_need_no_tolerance_arithmetic() {
        let smallest = f32::from_bits(1);
        let anchor = anchor(&[[-0.0, 0.0], [-smallest, smallest], [f32::MAX, f32::MAX]]);
        for zero in [0.0, -0.0] {
            assert_eq!(anchor.compare(&source(&[zero, smallest, f32::MAX])).unwrap().outside(), 0);
        }
        let observation = anchor.compare(&source(&[smallest, smallest, f32::MAX])).unwrap();
        assert_eq!(observation.outside(), 1);
        assert_eq!(observation.first_outlier().unwrap().observed_bits, 1);
    }

    #[test]
    fn matching_dimensions_do_not_substitute_for_capture_identity() {
        let anchor = anchor(&[[0.0, 2.0]]);
        let valid = source(&[1.0]);
        for field in 0..6 {
            let mut identity = valid.identity();
            match field {
                0 => identity.profile.model += 1,
                1 => identity.profile.model_generation += 1,
                2 => identity.profile.tap += 1,
                3 => identity.profile.layout_generation += 1,
                4 => identity.stream += 1,
                _ => identity.position += 1,
            }
            assert_eq!(anchor.compare(&SourceFrame::capture(identity, &[1.0]).unwrap()), Err(Error::Binding));
        }
        assert_eq!(anchor.compare(&source(&[1.0, 1.0])), Err(Error::Binding));
        assert_eq!(anchor.compare(&valid).unwrap().outside(), 0);
    }

    #[test]
    fn passports_bind_every_manifest_component_and_unique_anchor() {
        let baseline = manifest();
        let passport = ModelPassport::new(1, 1, baseline.clone(), vec![anchor(&[[0.0, 1.0]])]).unwrap();
        assert_eq!(passport.manifest(), &baseline);
        for index in 0..5 {
            let mut changed = baseline.clone();
            match index {
                0 => changed.weights[0] ^= 1,
                1 => changed.adapters[0] ^= 1,
                2 => changed.tokenizer[0] ^= 1,
                3 => changed.architecture[0] ^= 1,
                _ => changed.numeric_profile[0] ^= 1,
            }
            assert_ne!(passport.manifest(), &changed);
        }
        assert_eq!(ModelPassport::new(1, 1, baseline, vec![anchor(&[[0.0, 1.0]]), anchor(&[[0.0, 1.0]])]), Err(Error::Duplicate));
        let mut changed = manifest(); changed.model += 1;
        assert_eq!(ModelPassport::new(1, 1, changed, vec![anchor(&[[0.0, 1.0]])]), Err(Error::Binding));
    }

    #[test]
    fn invalid_registration_and_aggregate_capacity_are_rejected() {
        for bounds in [vec![], vec![[f32::NAN, 1.0]], vec![[0.0, f32::INFINITY]], vec![[2.0, 1.0]]] {
            assert!(IdentityAnchor::new(1, profile(), 6, vec![10], &bounds).is_err());
        }
        let mut invalid = manifest(); invalid.numeric_profile = [0; 32];
        assert_eq!(ModelPassport::new(1, 1, invalid, vec![anchor(&[[0.0, 1.0]])]), Err(Error::InvalidInput));
        let large = anchor(&vec![[0.0, 1.0]; MAX_VALUES]);
        assert!(ModelPassport::new(1, 1, manifest(), vec![large.clone()]).is_ok());
        let extra = IdentityAnchor::new(2, profile(), 6, vec![10], &[[0.0, 1.0]]).unwrap();
        assert_eq!(ModelPassport::new(1, 1, manifest(), vec![large, extra]), Err(Error::Limit));
        assert!(IdentityAnchor::new(1, profile(), 6, vec![10; MAX_STIMULUS_TOKENS + 1], &[[0.0, 1.0]]).is_err());
    }
}
