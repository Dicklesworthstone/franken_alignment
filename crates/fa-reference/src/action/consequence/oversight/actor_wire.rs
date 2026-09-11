//! Actor-only byte interface to the existing proposal gateway (FA-107, plan 17.1).
//!
//! The host binds one port to an authenticated connection outside this module.
//! No broker, clock, policy, helper, permit, or endpoint is reachable here.

mod codec;
pub use codec::{Command, MAX_FRAME_BYTES, MAX_RESPONSE_BYTES, WireError, WireResponse, decode_command, encode_command};

use super::actor::{ActorOutcome, ActorPort, ActorTicket, Knowledge, MAX_ACTOR_REQUESTS};
use std::collections::BTreeMap;
use std::fmt;

/// Retain this value across transport reconnection to retain ticket visibility.
/// A new session on the same port can recover a ticket by an exact submit retry;
/// knowing a numeric request ID alone does not import another session's ticket.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::actor_wire::ActorWire;
/// fn escape(wire: ActorWire) { let _broker = wire.broker_mut(); }
/// ```
pub struct ActorWire {
    port: ActorPort,
    tickets: BTreeMap<u64, ActorTicket>,
}

impl fmt::Debug for ActorWire {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActorWire").field("tickets", &self.tickets.len()).finish_non_exhaustive()
    }
}

impl ActorWire {
    pub fn new(port: ActorPort) -> Self { Self { port, tickets: BTreeMap::new() } }

    /// Process exactly one bounded JSON document, with no framing or I/O.
    /// Parse failures disclose no partially decoded request or private diagnostics.
    /// A successful submit/cancel response is an observation, never authorization.
    pub fn exchange(&mut self, document: &[u8]) -> WireResponse {
        let command = match decode_command(document) {
            Ok(command) => command,
            Err(error) => return WireResponse { request: None, result: Err(error) },
        };
        let request = command.request();
        let result = match command {
            Command::Submit { proposal, .. } => {
                if !self.tickets.contains_key(&request) && self.tickets.len() >= MAX_ACTOR_REQUESTS {
                    Err(WireError::Capacity)
                } else {
                    self.port.submit(request, &proposal).map(|ticket| {
                        let observation = self.port.poll(&ticket);
                        self.tickets.insert(request, ticket);
                        observation
                    }).map_err(WireError::from)
                }
            }
            Command::Poll { .. } => Ok(self.poll(request)),
            Command::Cancel { .. } => match self.tickets.get(&request) {
                Some(ticket) => self.port.cancel(ticket).map(|()| self.port.poll(ticket)).map_err(WireError::from),
                None => Err(WireError::Withheld),
            },
        };
        WireResponse { request: Some(request), result }
    }

    fn poll(&self, request: u64) -> Knowledge<ActorOutcome> {
        self.tickets.get(&request).map_or(
            Knowledge::Withheld { authority_required: "own_request" },
            |ticket| self.port.poll(ticket),
        )
    }
}
