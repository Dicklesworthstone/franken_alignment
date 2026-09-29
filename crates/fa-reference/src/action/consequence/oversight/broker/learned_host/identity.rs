//! Actual owned-model stimuli through the original numerical identity engine.
//! Measurement is observation, never source eligibility or effect authority.
use super::OversightBroker;
use crate::action::consequence::activation::identity::{
    ModelPassport, decoder::DecoderIdentityProbe,
};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use crate::Error;

impl OversightBroker {
    /// Probe THIS learned owner's immutable parameters, not a caller-selected
    /// surrogate. Original all-anchor admission runs before any token work.
    /// Each original probe advance uses a fresh private anchor cache and has
    /// separately reported costs; it cannot spend/refill live generation budgets.
    ///
    /// This is also a diagnostic operation on a held or paused owner. It does
    /// not clear that condition, install an identity, resume inference, or issue
    /// a permit. A consumer must bind an original current identity challenge;
    /// a detached measurement is historical if the actor advances or resets.
    /// Manifest commitments and discriminatory strength remain independent host
    /// obligations. Exact residual matching is not cryptographic attestation.
    ///
    /// ```compile_fail,E0308
    /// use fa_reference::action::{Permit, consequence::oversight::OversightBroker};
    /// use fa_reference::action::consequence::activation::{identity::ModelPassport,
    ///     tensor::kv::decoder::DecoderBudget};
    /// fn grant(owner: &OversightBroker, passport: &ModelPassport, budget: DecoderBudget) -> Permit {
    ///     owner.hosted_learned_identity_probe(owner.actor_revision(), passport, 1, budget).unwrap()
    /// }
    /// ```
    pub fn hosted_learned_identity_probe(&self, expected_actor_revision: u64,
        passport: &ModelPassport, measurement_sequence: u64, budget: DecoderBudget)
        -> Result<DecoderIdentityProbe, Error>
    {
        if expected_actor_revision != self.actor_revision() { return Err(Error::Stale); }
        self.hosted_learned_original()?.host_identity_probe(passport, measurement_sequence, budget)
    }
}
