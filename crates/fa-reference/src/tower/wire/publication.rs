//! Projection of the native publication port through the common envelope.
//!
//! Native request observations retain their original wire representation and
//! request-local basis. Neither a request ID nor an intake generation becomes
//! a global Tower generation, control sequence, or proof of effect admission.

use super::{Encoder, KnowledgeKind, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::requests::actor::{
    MAX_PRECHECK_QUERIES, precheck_wire::PrecheckResponseError,
};
use crate::action::consequence::oversight::actor::MAX_ACTOR_REQUESTS;
use crate::action::consequence::oversight::actor_wire::MAX_FRAME_BYTES;
use crate::tower::publication::{PublicationLimits, PublicationResponse, PublicationResult};

struct PublicationRefusal;
impl KnowledgeKind for PublicationRefusal {
    const NAME: &'static str = "PublicationRefusal";
}

struct RemainingPublicationBudget;
impl KnowledgeKind for RemainingPublicationBudget {
    const NAME: &'static str = "RemainingPublicationBudget";
}

struct RealizedPublicationCost;
impl KnowledgeKind for RealizedPublicationCost {
    const NAME: &'static str = "RealizedPublicationCost";
}

impl PublicationResponse {
    /// Preserve the native request result as structured JSON, including every
    /// epistemic variant and its original basis. Encoding never polls, submits,
    /// or retries a request, and an output refusal never truncates an outcome.
    pub fn to_json(&self) -> Result<String, TowerEncodingError> {
        let encoder = Encoder {
            max_bytes: self.envelope.limits.max_response_bytes,
        };
        let envelope = encoder.envelope::<_, RemainingPublicationBudget, RealizedPublicationCost>(
            &self.envelope,
            publication_limits_json(self.envelope.limits),
        )?;
        let (kind, value) = match &self.result {
            PublicationResult::Request(response) => (
                "publication-request",
                encoder.bounded(
                    String::from_utf8(response.encode())
                        .map_err(|_| TowerEncodingError::InvalidKnowledge)?,
                )?,
            ),
            PublicationResult::Precheck(response) => {
                // The original encoder validates correlation and allowed read
                // states, retaining its disclosure basis and review-required
                // marker. None of those fields becomes a request admission.
                let bytes = response.encode().map_err(|error| match error {
                    PrecheckResponseError::Capacity => TowerEncodingError::OutputLimit,
                    PrecheckResponseError::Malformed
                    | PrecheckResponseError::UnsupportedVersion
                    | PrecheckResponseError::Binding => TowerEncodingError::InvalidKnowledge,
                })?;
                (
                    "precheck",
                    encoder.bounded(
                        String::from_utf8(bytes)
                            .map_err(|_| TowerEncodingError::InvalidKnowledge)?,
                    )?,
                )
            }
            PublicationResult::Capabilities(response) => {
                // Discovery retains the native grant-local basis, remaining
                // shared work, and explicit absence of effect authority. It is
                // never encoded as an exact-policy pass or a signed profile.
                let bytes = response.encode().map_err(|error| match error {
                    PrecheckResponseError::Capacity => TowerEncodingError::OutputLimit,
                    PrecheckResponseError::Malformed
                    | PrecheckResponseError::UnsupportedVersion
                    | PrecheckResponseError::Binding => TowerEncodingError::InvalidKnowledge,
                })?;
                (
                    "capabilities",
                    encoder.bounded(
                        String::from_utf8(bytes)
                            .map_err(|_| TowerEncodingError::InvalidKnowledge)?,
                    )?,
                )
            }
            PublicationResult::Refused(refusal) => (
                "refused",
                encoder.knowledge::<_, PublicationRefusal>(refusal, |_| {
                    // A refusal cannot acquire a success payload merely because
                    // public response fields were reconstructed by a caller.
                    Err(TowerEncodingError::InvalidKnowledge)
                })?,
            ),
        };
        let result = encoder.bounded(object_json(&[("kind", quote(kind)), ("value", value)]))?;
        encoder.bounded(object_json(&[("envelope", envelope), ("result", result)]))
    }
}

fn publication_limits_json(limits: PublicationLimits) -> String {
    object_json(&[
        ("max_actor_requests", MAX_ACTOR_REQUESTS.to_string()),
        ("max_precheck_queries", MAX_PRECHECK_QUERIES.to_string()),
        ("max_frame_bytes", MAX_FRAME_BYTES.to_string()),
        ("max_response_bytes", limits.max_response_bytes.to_string()),
    ])
}
