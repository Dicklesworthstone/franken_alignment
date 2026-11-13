//! Projection of the native publication port through the common envelope.
//!
//! Native request observations retain their original wire representation and
//! request-local basis. Neither a request ID nor an intake generation becomes
//! a global Tower generation, control sequence, or proof of effect admission.

mod abandonment;
mod cancellation;
mod convening;
mod explanation;
mod fence;
mod hold_diagnosis;
mod inspection;
mod journal_tail;
mod narrowing;
mod policy_promotion;
mod policy_replay;
mod request_history;
mod resolution;
mod review_explanation;

use super::{Encoder, KnowledgeKind, TowerEncodingError, object_json, quote};
use crate::action::consequence::delivery::persistent::requests::actor::{
    MAX_EXPLANATION_EVENTS, MAX_EXPLANATION_QUERIES, MAX_INSPECTION_ATTEMPTS,
    MAX_INSPECTION_QUERIES, MAX_PRECHECK_QUERIES, precheck_wire::PrecheckResponseError,
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
            PublicationResult::Situation(native) => {
                ("situation", inspection::encode(native, &encoder)?)
            }
            PublicationResult::SituationLinks(linked) => (
                "situation",
                inspection::encode_linked(linked, &self.envelope.address, &encoder)?,
            ),
            PublicationResult::JournalTail(linked) => (
                "journal-tail",
                journal_tail::encode(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::RequestHistory(linked) => (
                "request-history",
                request_history::encode(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::Explanation(native) => (
                "explanation",
                explanation::encode(native, &self.envelope.address, &encoder)?,
            ),
            PublicationResult::HoldDiagnosis(linked) => (
                "hold-diagnosis",
                hold_diagnosis::encode(linked, &self.envelope.address, &encoder)?,
            ),
            PublicationResult::ResolutionNext(linked) => (
                "resolution-next",
                resolution::encode_next(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::Resolution(linked) => (
                "resolution",
                resolution::encode_resolution(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::CancellationNext(linked) => (
                "cancellation-next",
                cancellation::encode_next(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::Cancellation(linked) => (
                "cancellation",
                cancellation::encode_cancellation(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::AbandonmentNext(linked) => (
                "abandonment-next",
                abandonment::encode_next(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::Abandonment(linked) => (
                "abandonment",
                abandonment::encode_abandonment(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::ConveneNext(linked) => (
                "convene-next",
                convening::encode_next(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::Convene(linked) => (
                "convene",
                convening::encode_convene(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::PolicyPromotionNext(linked) => (
                "policy-promotion-next",
                policy_promotion::encode_next(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::PolicyPromotion(linked) => (
                "policy-promotion",
                policy_promotion::encode_promotion(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::NarrowingNext(linked) => (
                "narrowing-next",
                narrowing::encode_next(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::Narrowing(linked) => (
                "narrowing",
                narrowing::encode_narrowing(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::Fence(linked) => (
                "fence",
                fence::encode(
                    linked,
                    &self.envelope.address,
                    &self.envelope.operation_id,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::ReviewExplanation(native) => (
                "review-explanation",
                review_explanation::encode(native, &self.envelope.address, &encoder)?,
            ),
            PublicationResult::RefinementExplanation(native) => (
                "refinement-explanation",
                review_explanation::encode_refinement(native, &self.envelope.address, &encoder)?,
            ),
            PublicationResult::PolicyReplay(native) => (
                "policy-replay",
                policy_replay::encode(
                    native,
                    &self.envelope.address,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
            PublicationResult::PolicyReplayPage(native) => (
                "policy-replay",
                policy_replay::encode_page(
                    native,
                    &self.envelope.address,
                    self.envelope.idempotency_key.as_deref(),
                    &encoder,
                )?,
            ),
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
        ("max_inspection_queries", MAX_INSPECTION_QUERIES.to_string()),
        (
            "max_explanation_queries",
            MAX_EXPLANATION_QUERIES.to_string(),
        ),
        ("max_explanation_events", MAX_EXPLANATION_EVENTS.to_string()),
        (
            "max_inspection_attempts",
            MAX_INSPECTION_ATTEMPTS.to_string(),
        ),
        ("max_frame_bytes", MAX_FRAME_BYTES.to_string()),
        ("max_response_bytes", limits.max_response_bytes.to_string()),
    ])
}
