//! Decode a complete untrusted document before entering any original backend.
//!
//! Access grants are supplied separately by the integrating host. A malformed
//! request produces only bounded public syntax metadata; none of its partially
//! parsed identity, payload, or claimed authority is reflected in the response.

use super::{
    ReasonCode, ReferenceTower, TowerAccess, TowerDecodingError, TowerRequest, TowerResponse, Verb,
    decode_request,
};
use crate::address_resolver::{Address, Noun};

impl ReferenceTower {
    /// Process exactly one bounded `fa.tower_request/1` JSON document.
    ///
    /// Syntax admission finishes before lookup, read expenditure or branch
    /// execution. Successful decoding delegates to the original typed dispatcher,
    /// including its visibility, generation, budget and exact-retry checks.
    /// This method neither authenticates a connection nor accepts caller grants
    /// from the document. Transport framing and deadlines belong to the host.
    pub fn exchange(&mut self, access: &TowerAccess, document: &[u8]) -> TowerResponse {
        match decode_request(document) {
            Ok(request) => self.dispatch(access, request),
            Err(error) => self.malformed_request(error),
        }
    }

    pub(super) fn malformed_request(&self, error: TowerDecodingError) -> TowerResponse {
        let mut response = self.refusal(&invalid_request(), error_reason(error), false);
        response.envelope.retry_safe = false;
        response
    }
}

pub(super) fn invalid_request() -> TowerRequest {
    TowerRequest::new(
        "<invalid-request>",
        Address::new("invalid", Noun::Run, "request", None).expect("constant address"),
        Verb::Get,
    )
}

pub(super) fn error_reason(error: TowerDecodingError) -> ReasonCode {
    match error {
        TowerDecodingError::Capacity => ReasonCode::BudgetExhausted,
        TowerDecodingError::UnsupportedVersion | TowerDecodingError::UnknownVerb => {
            ReasonCode::CapabilityUnsupported
        }
        TowerDecodingError::MalformedRequest => ReasonCode::IntegrityFailure,
    }
}
