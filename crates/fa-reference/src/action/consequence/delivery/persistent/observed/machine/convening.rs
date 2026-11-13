//! Eligibility for explicit ordinary successor reviews. This is separate from
//! target narrowing: an unchanged mediated owner can review again, but all
//! numerical and text-stream histories retain their existing unsupported scope.
use super::Machine;

impl Machine {
    pub(in super::super) fn convening_supported(&self) -> bool {
        self.decoder.is_none()
            && self.learned.is_none()
            && self.consistency.is_none()
            && self.learned_consistency.is_none()
            && self.broker.stream_state().is_none()
            && !self.generated_text_only
    }

    pub(in super::super) fn convening_requires_topology(&self) -> bool {
        self.mediation.is_some()
    }
}
