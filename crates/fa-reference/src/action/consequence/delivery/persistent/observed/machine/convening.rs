//! Eligibility for explicit successor reviews on the original publication owner.
//! Native generated streams retain their source-only message contract; learned
//! and predictive histories need their distinct retained-review custody.
use super::Machine;

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
    /// provisioned round. This predicate alone never enables generic review.
    pub(in super::super) fn native_learned_convening_supported(&self) -> bool {
        self.decoder.is_none()
            && self.learned.is_some()
            && self.broker.learned_sidecar_required()
            && self.consistency.is_none()
            && self.learned_consistency.is_none()
    }
}
