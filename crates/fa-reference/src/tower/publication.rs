//! Actor-scoped Tower dispatch into the original durable publication owner.
//!
//! One instance belongs to one host-authenticated actor session. Its native
//! port retains only weak ownership; the supervisor still owns snapshots,
//! congress, both publication keys, the endpoint and the canonical journal.
//! This adapter has no second ledger or outcome cache. Native request-local
//! provenance is preserved rather than converted into global Tower clocks.

mod capabilities;
#[cfg(target_os = "linux")]
pub mod peer;
mod precheck;

use super::{
    RESPONSE_SCHEMA, ReadArguments, ReasonCode, ResponseStatus, TowerAccess, TowerEncodingError,
    TowerEnvelope, TowerInstallError, TowerRequest, TowerResponse, Verb, validate_address,
};
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileActorPort, FileActorPrecheckPort,
    precheck_wire::{PrecheckWireResponse, discovery::PrecheckCapabilitiesResponse},
};
use crate::action::consequence::oversight::actor::{
    ActorError, ActorProposal, Knowledge as ActorKnowledge, UnknownReason as ActorUnknownReason,
};
use crate::action::consequence::oversight::actor_wire::{
    ActorWire, Command, WireError, WireResponse,
};
use crate::action::{Purpose, Scope};
use crate::address_resolver::{Address, Noun};
use crate::observation_algebra::{AuthorityKind, Knowledge, UnknownReason};
use crate::typestate_cx::PurposeTag;

pub const CONNECTED_PUBLICATION_VERBS: [Verb; 4] =
    [Verb::Propose, Verb::Get, Verb::Precheck, Verb::Capabilities];

/// Response bound for this native profile. Ticket, proposal and durable owner
/// limits remain the original native limits and cannot be enlarged here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationLimits {
    pub max_response_bytes: usize,
}

impl Default for PublicationLimits {
    fn default() -> Self {
        Self {
            max_response_bytes: 16 * 1024,
        }
    }
}

impl PublicationLimits {
    fn valid(self) -> bool {
        (16 * 1024..=1024 * 1024).contains(&self.max_response_bytes)
    }
}

/// The original observation-only response, including its original epistemic
/// variant and request-local basis. A request generation is not a control cut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationResult {
    Request(WireResponse),
    Precheck(PrecheckWireResponse),
    Capabilities(PrecheckCapabilitiesResponse),
    Refused(Knowledge<()>),
}

/// The same sixteen envelope fields with a native publication result profile.
/// No synthetic `Basis` is constructed for the native actor's limited view.
pub type PublicationResponse = TowerResponse<PublicationResult, PublicationLimits>;

/// A live, session-local view of an original `FileOversight` actor gateway.
/// It does not retain the supervisor or prevent the original owner from closing.
///
/// ```compile_fail,E0599
/// use fa_reference::tower::publication::PublicationTower;
/// fn no_supervisor(tower: PublicationTower) { let _ = tower.host_mut(); }
/// ```
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::tower::publication::PublicationResponse;
/// fn no_permit(response: PublicationResponse) -> FilePermit { response }
/// ```
pub struct PublicationTower {
    mount: Address,
    scope: Scope,
    wire: ActorWire<FileActorPort<FileOversight>>,
    precheck: Option<FileActorPrecheckPort>,
    limits: PublicationLimits,
}

impl PublicationTower {
    /// Mount exactly one original actor port. Numeric tenant and run identity
    /// must agree with its native scope; the explicit generation identifies this
    /// host registration, not a native policy epoch or request generation.
    pub fn new(
        mount: Address,
        port: FileActorPort<FileOversight>,
        limits: PublicationLimits,
    ) -> Result<Self, TowerInstallError> {
        validate_address(&mount)?;
        if !limits.valid() {
            return Err(TowerInstallError::InvalidLimits);
        }
        let scope = port.native_scope();
        if mount.kind != Noun::Run
            || mount.generation.is_none_or(|generation| generation == 0)
            || mount.tenant != scope.tenant.to_string()
            || mount.id != scope.run.to_string()
            || scope.purpose != Purpose::Effect
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        Ok(Self {
            mount,
            scope,
            wire: ActorWire::new(port),
            precheck: None,
            limits,
        })
    }

    pub fn mount_address(&self) -> &Address {
        &self.mount
    }

    /// Address of this session's projection of an intake request. This explicit
    /// namespace prevents a native request key from impersonating a ledger
    /// attempt number. Possessing this address does not acquire its ticket.
    pub fn request_address(&self, request: u64) -> Result<Address, TowerInstallError> {
        if request == 0 {
            return Err(TowerInstallError::InvalidAddress);
        }
        Address::new(
            self.mount.tenant.clone(),
            Noun::Attempt,
            format!("{}{request}", self.request_prefix()),
            self.mount.generation,
        )
        .map_err(|_| TowerInstallError::InvalidAddress)
    }

    /// Decode one complete bounded Tower document before consulting the native
    /// owner or spending disclosure work. Host access remains a separate input;
    /// malformed bytes never acquire an admission snapshot or query allowance.
    pub fn exchange(&mut self, access: &TowerAccess, document: &[u8]) -> PublicationResponse {
        self.exchange_with_admission(access, document, |_, _, _| Ok(()))
    }

    /// Only original trusted host integrations can acquire source evidence at
    /// the native admission boundary. Complete decoding precedes that handoff.
    pub(crate) fn exchange_with_admission<A>(
        &mut self,
        access: &TowerAccess,
        document: &[u8],
        admission: A,
    ) -> PublicationResponse
    where
        A: FnOnce(&FileActorPort<FileOversight>, u64, &ActorProposal) -> Result<(), ActorError>,
    {
        match super::decode_request(document) {
            Ok(request) => self.dispatch_with_admission(access, request, admission),
            Err(error) => {
                let mut response = self.refusal(
                    &super::intake::invalid_request(),
                    super::intake::error_reason(error),
                    false,
                );
                response.envelope.retry_safe = false;
                response
            }
        }
    }

    /// Submit through native durable intake or observe a ticket already acquired
    /// by this session. Every accepted observation consults the original owner.
    /// A successful submission is not approval or a completed external effect.
    pub fn dispatch(&mut self, access: &TowerAccess, request: TowerRequest) -> PublicationResponse {
        self.dispatch_with_admission(access, request, |_, _, _| Ok(()))
    }

    pub(crate) fn request_port(&self) -> &FileActorPort<FileOversight> {
        self.wire.request_port()
    }

    /// Keep source acquisition after every Tower admission check and the
    /// original actor's complete proposal validation and ticket-capacity check.
    /// This hook is never exposed to actors or invoked by a read operation.
    pub(crate) fn dispatch_with_admission<A>(
        &mut self,
        access: &TowerAccess,
        mut request: TowerRequest,
        admission: A,
    ) -> PublicationResponse
    where
        A: FnOnce(&FileActorPort<FileOversight>, u64, &ActorProposal) -> Result<(), ActorError>,
    {
        if !valid_request_identity(&request) {
            let normalized = TowerRequest::new(
                "<invalid-request>",
                Address::new("invalid", Noun::Run, "request", None).expect("constant address"),
                request.verb,
            );
            return self.refusal(&normalized, ReasonCode::IntegrityFailure, false);
        }
        // Source admission precedes ticket lookup, native reads, and every
        // possible durable write. The host, not actor bytes, owns these grants.
        if access.scope.caller_tenant != self.mount.tenant
            || access.scope.is_authorized_for(&request.address).is_err()
        {
            return self.withheld(&request);
        }
        if !CONNECTED_PUBLICATION_VERBS.contains(&request.verb) {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
        }
        if request.verb == Verb::Propose && !access.allowed_purposes.contains(&PurposeTag::Permit) {
            return self.refusal(&request, ReasonCode::PurposeMismatch, false);
        }
        // Generation mismatch is checked before looking up or spending a native
        // admission snapshot. Mutations require an explicit exact predecessor.
        if request
            .address
            .generation
            .is_some_and(|g| Some(g) != self.mount.generation)
            || (request.verb == Verb::Propose && request.address.generation.is_none())
        {
            let mut response = self.refusal(&request, ReasonCode::StaleGeneration, false);
            response.envelope.would_change_this = vec![self.mount.clone()];
            return response;
        }

        if request.verb == Verb::Capabilities {
            return self.dispatch_capabilities(request);
        }

        if request.verb == Verb::Precheck {
            return self.dispatch_precheck(request);
        }

        let arguments = std::mem::replace(&mut request.arguments, ReadArguments::None);
        let command = match (request.verb, arguments) {
            (
                Verb::Propose,
                ReadArguments::Propose {
                    request: key,
                    proposal,
                },
            ) => {
                if request.address.without_generation() != self.mount.without_generation() {
                    return self.refusal(&request, ReasonCode::MissingRequiredObservation, false);
                }
                if key == 0 || request.idempotency_key.as_deref() != Some(key.to_string().as_str())
                {
                    return self.refusal(&request, ReasonCode::IntegrityFailure, false);
                }
                // The native typed executor validates all proposal fields before
                // acquiring a snapshot. Transfer the owned payload directly;
                // no extra clone, hex encoding, JSON parsing or payload cache.
                Command::Submit {
                    request: key,
                    proposal,
                }
            }
            (Verb::Get, ReadArguments::None) => {
                let Some(key) = self.request_key(&request.address) else {
                    return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
                };
                Command::Poll { request: key }
            }
            _ => return self.refusal(&request, ReasonCode::IntegrityFailure, false),
        };
        let native = self.wire.execute_with_admission(command, admission);
        self.native_response(&request, native, true)
    }

    fn request_prefix(&self) -> String {
        format!(
            "request-{}-{}-{}-{}-",
            self.scope.run, self.scope.principal, self.scope.branch, self.scope.authority
        )
    }

    /// Routing reserves the complete native namespace, including invalid key
    /// spellings. Only request_key may interpret a suffix as an actual ticket.
    pub(super) fn owns_request_namespace(&self, address: &Address) -> bool {
        address.tenant == self.mount.tenant
            && address.kind == Noun::Attempt
            && address.id.starts_with(&self.request_prefix())
    }

    fn request_key(&self, address: &Address) -> Option<u64> {
        if address.kind != Noun::Attempt || address.tenant != self.mount.tenant {
            return None;
        }
        let prefix = self.request_prefix();
        let suffix = address.id.strip_prefix(&prefix)?;
        let value = suffix.parse::<u64>().ok()?;
        (value != 0 && value.to_string() == suffix).then_some(value)
    }

    fn native_response(
        &self,
        request: &TowerRequest,
        native: WireResponse,
        retry_safe: bool,
    ) -> PublicationResponse {
        let reason = native_reason(&native);
        self.bounded_response(self.response(
            request,
            PublicationResult::Request(native),
            reason,
            retry_safe,
        ))
    }

    fn response(
        &self,
        request: &TowerRequest,
        result: PublicationResult,
        reason: Option<ReasonCode>,
        retry_safe: bool,
    ) -> PublicationResponse {
        TowerResponse {
            envelope: TowerEnvelope {
                schema_version: RESPONSE_SCHEMA,
                operation_id: request.operation_id.clone(),
                status: if reason.is_some() {
                    ResponseStatus::Error
                } else {
                    ResponseStatus::Ok
                },
                reason_code: reason,
                address: request.address.clone(),
                // Native actor ports intentionally expose none of these global
                // coordinates. Query/request labels cannot stand in for them.
                generation: Knowledge::unknown(UnknownReason::Unavailable),
                control_seq: Knowledge::unknown(UnknownReason::Unavailable),
                source_frontiers: Knowledge::unknown(UnknownReason::Unavailable),
                missing: if reason.is_some() {
                    vec![request.address.clone()]
                } else {
                    Vec::new()
                },
                limits: self.limits,
                budgets_remaining: Knowledge::unknown(UnknownReason::Unavailable),
                realized_cost: Knowledge::unknown(UnknownReason::Unavailable),
                affordances: Knowledge::unknown(UnknownReason::Unavailable),
                retry_safe,
                idempotency_key: request.idempotency_key.clone(),
                would_change_this: if reason.is_some() {
                    vec![request.address.clone()]
                } else {
                    Vec::new()
                },
            },
            result,
        }
    }

    fn refusal(
        &self,
        request: &TowerRequest,
        reason: ReasonCode,
        retry_safe: bool,
    ) -> PublicationResponse {
        self.response(
            request,
            PublicationResult::Refused(Knowledge::unknown(UnknownReason::Unavailable)),
            Some(reason),
            retry_safe,
        )
    }

    fn withheld(&self, request: &TowerRequest) -> PublicationResponse {
        let mut response = self.response(
            request,
            PublicationResult::Refused(Knowledge::withheld(AuthorityKind::Verify)),
            Some(ReasonCode::DeniedByPolicy),
            false,
        );
        response.envelope.generation = Knowledge::withheld(AuthorityKind::Verify);
        response.envelope.control_seq = Knowledge::withheld(AuthorityKind::Verify);
        response.envelope.source_frontiers = Knowledge::withheld(AuthorityKind::Verify);
        response.envelope.budgets_remaining = Knowledge::withheld(AuthorityKind::Verify);
        response.envelope.realized_cost = Knowledge::withheld(AuthorityKind::Verify);
        response.envelope.affordances = Knowledge::withheld(AuthorityKind::Verify);
        response
    }

    fn bounded_response(&self, mut response: PublicationResponse) -> PublicationResponse {
        if let Err(error) = response.to_json() {
            // A native mutation may already be durably admitted. Keep its exact
            // key retry-safe, and never pretend encoding failure rolled it back.
            response.envelope.status = ResponseStatus::Error;
            response.envelope.reason_code = Some(match error {
                TowerEncodingError::OutputLimit => ReasonCode::BudgetExhausted,
                TowerEncodingError::InvalidKnowledge => ReasonCode::IntegrityFailure,
            });
            response.result =
                PublicationResult::Refused(Knowledge::unknown(UnknownReason::Incomplete));
            response.envelope.missing = vec![response.envelope.address.clone()];
            response.envelope.would_change_this = vec![response.envelope.address.clone()];
        }
        response
    }
}

pub(super) fn valid_request_identity(request: &TowerRequest) -> bool {
    !request.operation_id.is_empty()
        && request.operation_id.len() <= 128
        && request
            .idempotency_key
            .as_ref()
            .is_none_or(|key| !key.is_empty() && key.len() <= 128)
        && validate_address(&request.address).is_ok()
}

fn native_reason(response: &WireResponse) -> Option<ReasonCode> {
    match &response.result {
        Err(
            WireError::MalformedRequest
            | WireError::UnsupportedVersion
            | WireError::IdempotencyConflict,
        ) => Some(ReasonCode::IntegrityFailure),
        Err(WireError::Capacity) => Some(ReasonCode::BudgetExhausted),
        Err(WireError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
        Err(WireError::Withheld) => Some(ReasonCode::DeniedByPolicy),
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::ControllerUnavailable,
        }) => Some(ReasonCode::LowerLayerUnverified),
        Ok(ActorKnowledge::Unknown {
            reason: ActorUnknownReason::OutcomeUnknown,
        }) => Some(ReasonCode::OutcomeUnknown),
        Ok(ActorKnowledge::Withheld { .. }) => Some(ReasonCode::DeniedByPolicy),
        Ok(ActorKnowledge::Stale { .. }) => Some(ReasonCode::StaleGeneration),
        Ok(
            ActorKnowledge::Known { .. }
            | ActorKnowledge::Pending { .. }
            | ActorKnowledge::Absent { .. },
        ) => None,
    }
}
