//! Bounded exact-precheck bytes, separately provisioned from submit/poll/cancel.
//! Complete-document codecs are separate from bounded framing and Unix I/O;
//! Linux peer admission checks kernel credentials, never effect authority.
mod codec;
mod response;
pub mod discovery;
mod channel;
#[cfg(unix)]
mod transport;
#[cfg(target_os = "linux")]
mod peer;
#[cfg(target_os = "linux")]
pub mod client;

pub use codec::{PrecheckRequest, decode_precheck_request, encode_precheck_request};
pub use response::{PrecheckResponseError, PrecheckWireResponse, decode_precheck_response};
pub use channel::FilePrecheckChannel;
#[cfg(unix)]
pub use transport::UnixPrecheckConnection;
#[cfg(target_os = "linux")]
pub use peer::FilePrecheckPeerSession;
use super::{FileActorPrecheckPort, MAX_PRECHECK_QUERIES};
use crate::action::MAX_PAYLOAD_BYTES;
use crate::action::consequence::oversight::actor::ActorError;
use std::fmt;

/// One JSON document, excluding any transport framing.
pub const MAX_PRECHECK_FRAME_BYTES: usize = MAX_PAYLOAD_BYTES * 2 + 1_024;
pub const MAX_PRECHECK_RESPONSE_BYTES: usize = 1_024;
pub const MAX_PRECHECK_EXCHANGES: usize = MAX_PRECHECK_QUERIES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrecheckWireError { MalformedRequest, UnsupportedVersion, Capacity, Unavailable, Withheld }

impl From<ActorError> for PrecheckWireError {
    fn from(error: ActorError) -> Self {
        match error {
            ActorError::MalformedProposal => Self::MalformedRequest,
            ActorError::Capacity => Self::Capacity,
            ActorError::Withheld => Self::Withheld,
            ActorError::Unavailable | ActorError::IdempotencyConflict => Self::Unavailable,
        }
    }
}
impl PrecheckWireError {
    fn name(self) -> &'static str {
        match self {
            Self::MalformedRequest => "malformed_request",
            Self::UnsupportedVersion => "unsupported_version",
            Self::Capacity => "capacity",
            Self::Unavailable => "unavailable",
            Self::Withheld => "withheld",
        }
    }
}

/// The host must explicitly provide a disclosed port. The ordinary actor port
/// cannot create this adapter. It holds no custodian, owner, ticket or permit.
///
/// The frame allowance belongs to this adapter; the evaluation allowance belongs
/// to the original disclosure and is shared with all port clones. Recreating an
/// adapter never renews that evaluation allowance. Neither is a global limiter.
///
/// Exchange consumes one frame slot BEFORE parsing, including malformed bytes.
/// The native port separately charges entered evaluations. There is no framing,
/// socket, I/O, clock read, source refresh or journal write here.
///
/// Precheck is an existing registered rehearsal verb, not submit admission.
pub struct FilePrecheckWire {
    port: FileActorPrecheckPort,
    remaining: usize,
}
impl fmt::Debug for FilePrecheckWire {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePrecheckWire").field("remaining_exchanges", &self.remaining).finish_non_exhaustive()
    }
}
impl FilePrecheckWire {
    pub fn new(port: FileActorPrecheckPort, exchanges: usize) -> Result<Self, PrecheckWireError> {
        if exchanges == 0 || exchanges > MAX_PRECHECK_EXCHANGES {
            return Err(PrecheckWireError::Capacity);
        }
        Ok(Self { port, remaining: exchanges })
    }
    pub fn remaining_exchanges(&self) -> usize { self.remaining }

    /// Process one complete bounded document. Parse failures never disclose a
    /// partially decoded query label. A pass remains review-required observation.
    pub fn exchange(&mut self, document: &[u8]) -> PrecheckWireResponse {
        if self.remaining == 0 {
            return PrecheckWireResponse { query: None, result: Err(PrecheckWireError::Capacity) };
        }
        self.remaining -= 1;
        let request = match decode_precheck_request(document) {
            Ok(request) => request,
            Err(error) => return PrecheckWireResponse { query: None, result: Err(error) },
        };
        PrecheckWireResponse {
            query: Some(request.query),
            result: self.port.check(request.query, &request.proposal).map_err(PrecheckWireError::from),
        }
    }
}

#[cfg(test)]
mod tests;
