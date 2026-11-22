//! Eligibility for explicit successor reviews on the original publication owner.
//! Native generated streams retain their source-only message contract; learned
//! and predictive histories need their distinct retained-review custody.
use super::Machine;
use crate::Error;

impl Machine {
    pub(in super::super) fn convening_supported(&self) -> bool {
        let ordinary = self.decoder.is_none()
            && self.broker.stream_state().is_none()
            && !self.generated_text_only;
        let native_text = self.decoder.is_some()
            && self.broker.stream_state().is_some()
            && self.generated_text_only;
        (ordinary || native_text)
            && self.learned.is_none()
            && self.consistency.is_none()
            && self.learned_consistency.is_none()
    }

    pub(in super::super) fn convening_requires_topology(&self) -> bool {
        self.mediation.is_some()
    }

    /// A retained native evaluator must separately offer its exact unused
    /// provisioned round. Ordinary absence and required pre-output prediction
    /// remain distinct original recipes. Optional, legacy or mismatched lanes
    /// cannot borrow either contract. This admits configuration only; each
    /// predictive attempt must still retain its exact settled observation.
    pub(in super::super) fn native_learned_convening_supported(&self) -> bool {
        if self.decoder.is_some() || !self.broker.learned_sidecar_required() {
            return false;
        }
        let Some(recipe) = self.learned_contract() else {
            return false;
        };
        match (
            recipe.required_pre_output_forecast(),
            recipe.required_owned_pre_output_forecast(),
        ) {
            (None, None) => {
                self.consistency.is_none()
                    && self.learned_consistency.is_none()
                    && !self.broker.action_consistency_required()
            }
            (Some(required), None) => {
                required.requires_pre_output_forecast()
                    && required.hosted_residual_layer().is_some()
                    && self.consistency.as_deref() == Some(required)
                    && self.learned_consistency.is_none()
                    && self.broker.action_consistency_required()
                    && !self.broker.learned_action_consistency_required()
                    && self.broker.hosted_consistency_layer() == required.hosted_residual_layer()
            }
            (None, Some(required)) => {
                required.uses_owned_generation()
                    && required.requires_pre_output_forecast()
                    && self.learned_consistency.as_deref() == Some(required)
                    && self.consistency.as_deref() == Some(required.consistency())
                    && self.broker.owned_learned_consistency_required()
                    && self.broker.hosted_consistency_layer().is_none()
            }
            (Some(_), Some(_)) => false,
        }
    }

    /// Recheck the SAME request's observation without replacing the predictor
    /// or opening another forecast. Message and Finish have distinct attempts;
    /// an earlier message's sample cannot qualify a later held release.
    pub(in super::super) fn check_native_learned_convening(
        &self,
        attempt: u64,
    ) -> Result<(), Error> {
        if !self.native_learned_convening_supported() {
            return Err(Error::Binding);
        }
        if self.consistency_request.is_some() {
            return Err(Error::Incomplete);
        }
        self.broker.check_consistency(attempt)
    }
}
