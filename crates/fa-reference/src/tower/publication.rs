//! Scope-bound Tower dispatch into the original durable publication owner.
//!
//! One instance belongs to one host-authenticated actor or operator session.
//! Its native port retains only weak ownership; the supervisor still owns snapshots,
//! congress, both publication keys, the endpoint and the canonical journal.
//! This adapter has no second ledger or outcome cache. Native request-local
//! provenance is preserved rather than converted into global Tower clocks.

mod capabilities;
mod fence;
pub use fence::LinkedFence;
mod hold_diagnosis;
pub use hold_diagnosis::LinkedHoldDiagnosis;
mod resolution;
pub use resolution::{LinkedResolution, LinkedResolutionNext};
mod explanation;
mod review_explanation;
pub use review_explanation::{LinkedRefinementExplanation, LinkedReviewExplanation};
mod inspection;
pub use inspection::LinkedSituation;
#[cfg(target_os = "linux")]
pub mod peer;
mod policy_replay;
pub use policy_replay::{LinkedPolicyReplay, LinkedPolicyReplayPage};
mod precheck;

use super::{
    RESPONSE_SCHEMA, ReadArguments, ReasonCode, ResponseStatus, TowerAccess, TowerEncodingError,
    TowerEnvelope, TowerInstallError, TowerRequest, TowerResponse, Verb, validate_address,
};
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileActorPort, FileActorPrecheckPort, FileExplanationResult, FileInspectionCursor,
    FileInspectionResult, FileLearnedTextStreamActorPort, FilePolicyReplayCursor,
    FileSupervisorExplanationPort, FileSupervisorHoldDiagnosisPort, FileSupervisorInspectionPort,
    FileSupervisorPolicyReplayPort, FileSupervisorResolutionPort, FileSupervisorFencePort,
    FileSupervisorReviewExplanationPort,
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
use std::collections::BTreeMap;

pub const CONNECTED_PUBLICATION_VERBS: [Verb; 11] = [
    Verb::Propose,
    Verb::Get,
    Verb::Precheck,
    Verb::Capabilities,
    Verb::Situation,
    Verb::Explain,
    Verb::WhyHeld,
    Verb::Next,
    Verb::Resolve,
    Verb::Fence,
    Verb::PolicyReplay,
];

/// Smallest native response allowance; also the compatibility default.
pub const MIN_PUBLICATION_RESPONSE_BYTES: usize = 16 * 1024;
/// Largest complete response accepted by the native publication facade.
pub const MAX_PUBLICATION_RESPONSE_BYTES: usize = 1024 * 1024;

/// Response bound for this native profile. Ticket, proposal and durable owner
/// limits remain the original native limits and cannot be enlarged here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationLimits {
    pub max_response_bytes: usize,
}

impl Default for PublicationLimits {
    fn default() -> Self {
        Self {
            max_response_bytes: MIN_PUBLICATION_RESPONSE_BYTES,
        }
    }
}

impl PublicationLimits {
    /// Validate a host-selected allowance before any endpoint or owner setup.
    /// This changes output capacity only, never a native operation's authority.
    pub fn valid(self) -> bool {
        (MIN_PUBLICATION_RESPONSE_BYTES..=MAX_PUBLICATION_RESPONSE_BYTES)
            .contains(&self.max_response_bytes)
    }
}

/// Original request and reconciliation responses preserve each native epistemic variant
/// and basis. A request generation is not a supervisor situation's control cut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicationResult {
    Request(WireResponse),
    Precheck(PrecheckWireResponse),
    Capabilities(PrecheckCapabilitiesResponse),
    Situation(FileInspectionResult),
    SituationLinks(LinkedSituation),
    Explanation(FileExplanationResult),
    HoldDiagnosis(LinkedHoldDiagnosis),
    ResolutionNext(LinkedResolutionNext),
    Resolution(LinkedResolution),
    Fence(LinkedFence),
    ReviewExplanation(LinkedReviewExplanation),
    RefinementExplanation(LinkedRefinementExplanation),
    PolicyReplay(LinkedPolicyReplay),
    PolicyReplayPage(LinkedPolicyReplayPage),
    Refused(Knowledge<()>),
}

/// The same sixteen envelope fields with a native publication result profile.
/// No synthetic global `Basis` is constructed for native observations.
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
    // A learned stream exposes source-derived release intents only. Its
    // observation facade may bind original supervisor grants, but must never
    // turn the private weak gateway into ordinary payload intake or tickets.
    learned_observer: bool,
    // Operator reconciliation grants never widen this service into actor intake.
    reconciliation_operator: bool,
    // Explicit original-domain control mount; legacy observation and
    // reconciliation constructors never gain the fence route implicitly.
    control_operator: bool,
    wire: ActorWire<FileActorPort<FileOversight>>,
    precheck: Option<FileActorPrecheckPort>,
    inspection: Option<FileSupervisorInspectionPort>,
    inspection_cursor: Option<FileInspectionCursor>,
    explanation: Option<FileSupervisorExplanationPort>,
    hold_diagnosis: Option<FileSupervisorHoldDiagnosisPort>,
    resolution: Option<FileSupervisorResolutionPort>,
    fence: Option<FileSupervisorFencePort>,
    review_explanation: Option<FileSupervisorReviewExplanationPort>,
    policy_replay: Option<FileSupervisorPolicyReplayPort>,
    policy_replay_cursors: BTreeMap<u64, FilePolicyReplayCursor>,
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
            learned_observer: false,
            reconciliation_operator: false,
            control_operator: false,
            wire: ActorWire::new(port),
            precheck: None,
            inspection: None,
            inspection_cursor: None,
            explanation: None,
            hold_diagnosis: None,
            resolution: None,
            fence: None,
            review_explanation: None,
            policy_replay: None,
            policy_replay_cursors: BTreeMap::new(),
            limits,
        })
    }

    /// Mount an observation-only view of the ORIGINAL learned stream owner.
    /// The host must separately install and retain its supervisor read grants.
    /// This does not expose an ordinary actor port, accept a caller's payload,
    /// acquire a release ticket or retain the supervisor's authority lock.
    ///
    /// Only an operator may use this facade, even when the caller is otherwise
    /// authorized for the same run. Situation, recorded hold diagnosis and
    /// explicitly granted historical explanation/policy comparison remain their
    /// original native operations.
    /// Unsupported numerical replay stays Unknown under those grants' original
    /// work contracts. Permit/Experiment tags cannot enable ordinary submission,
    /// cancellation, precheck, request polling or any other mutation here.
    pub fn new_learned_observer(
        mount: Address,
        port: FileLearnedTextStreamActorPort,
        limits: PublicationLimits,
    ) -> Result<Self, TowerInstallError> {
        let mut tower = Self::new(mount, port.into_tower_observation_port(), limits)?;
        tower.learned_observer = true;
        Ok(tower)
    }

    /// Mount an ordinary owner for separately granted operator reconciliation.
    /// Even Permit and Experiment purposes cannot enable actor submission,
    /// cancellation, ticket polling or policy precheck through this facade.
    /// The host must independently install and retain the finite resolution
    /// grant. Learned observation constructors keep their closed read profile.
    pub fn new_reconciliation_operator(
        mount: Address,
        port: FileActorPort<FileOversight>,
        limits: PublicationLimits,
    ) -> Result<Self, TowerInstallError> {
        let mut tower = Self::new(mount, port, limits)?;
        tower.reconciliation_operator = true;
        Ok(tower)
    }

    /// Mount the ORIGINAL learned stream owner for explicitly granted receipt
    /// reconciliation. This does not expose its private ordinary actor gateway:
    /// the operator profile admits only the same closed observation vocabulary
    /// and Next/Resolve, under their separate native grants and caller purpose.
    ///
    /// The constructor supplies no resolution authority or numerical work.
    /// Learned history requires an independently retained numerical resolution
    /// grant; the ordinary grant keeps its original refusal. Reconciliation
    /// observes an already dispatched effect through its original endpoint. It
    /// never generates, submits, publishes, seals or abandons an effect.
    pub fn new_learned_reconciliation_operator(
        mount: Address,
        port: FileLearnedTextStreamActorPort,
        limits: PublicationLimits,
    ) -> Result<Self, TowerInstallError> {
        Self::new_reconciliation_operator(mount, port.into_tower_observation_port(), limits)
    }

    /// Mount one original owner for explicitly granted control operations.
    /// Ordinary actor intake stays closed even when Permit is allowed. This
    /// constructor supplies no fence or reconciliation grant of its own.
    pub fn new_control_operator(
        mount: Address,
        port: FileActorPort<FileOversight>,
        limits: PublicationLimits,
    ) -> Result<Self, TowerInstallError> {
        let mut tower = Self::new_reconciliation_operator(mount, port, limits)?;
        tower.control_operator = true;
        Ok(tower)
    }

    /// The original learned owner with the same restricted operator surface.
    /// A separate finite numerical grant is required to enter a learned fence;
    /// neither this mount nor a caller's payload can drive generation.
    pub fn new_learned_control_operator(
        mount: Address,
        port: FileLearnedTextStreamActorPort,
        limits: PublicationLimits,
    ) -> Result<Self, TowerInstallError> {
        Self::new_control_operator(mount, port.into_tower_observation_port(), limits)
    }

    pub fn mount_address(&self) -> &Address {
        &self.mount
    }

    /// Address of this session's projection of an intake request. This explicit
    /// namespace prevents a native request key from impersonating a ledger
    /// attempt number. Possessing this address does not acquire its ticket.
    pub fn request_address(&self, request: u64) -> Result<Address, TowerInstallError> {
        request_address_for(self.scope, &self.mount, request)
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
        if self.learned_observer {
            if !access.operator {
                return self.withheld(&request);
            }
            if !matches!(
                request.verb,
                Verb::Situation | Verb::Explain | Verb::WhyHeld | Verb::PolicyReplay
            ) {
                return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
            }
        }
        if self.reconciliation_operator {
            if !access.operator {
                return self.withheld(&request);
            }
            if !matches!(
                request.verb,
                Verb::Situation
                    | Verb::Explain
                    | Verb::WhyHeld
                    | Verb::Next
                    | Verb::Resolve
                    | Verb::PolicyReplay
            ) && !(self.control_operator && request.verb == Verb::Fence) {
                return self.refusal(&request, ReasonCode::CapabilityUnsupported, false);
            }
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
            || (matches!(
                request.verb,
                Verb::Propose | Verb::PolicyReplay | Verb::Resolve | Verb::Fence
            ) && request.address.generation.is_none())
        {
            let mut response = self.refusal(&request, ReasonCode::StaleGeneration, false);
            response.envelope.would_change_this = vec![self.mount.clone()];
            return response;
        }

        if request.verb == Verb::Situation {
            return self.dispatch_situation(access, request);
        }

        if request.verb == Verb::Explain {
            return self.dispatch_explanation(access, request);
        }

        if request.verb == Verb::WhyHeld {
            return self.dispatch_hold_diagnosis(access, request);
        }

        if matches!(request.verb, Verb::Next | Verb::Resolve) {
            return self.dispatch_resolution(access, request);
        }

        if request.verb == Verb::Fence {
            return self.dispatch_fence(access, request);
        }

        if request.verb == Verb::PolicyReplay {
            return self.dispatch_policy_replay(access, request);
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
        request_prefix(self.scope)
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
                // Any coordinates disclosed by a native result retain their
                // own basis. None is promoted to a global Tower coordinate.
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

/// Canonical navigation address for an external request in one publication
/// scope and registration. This pure spelling helper supplies no ticket, read
/// grant, native owner or evidence that the named request exists.
pub fn request_address_for(
    scope: Scope,
    registration: &Address,
    request: u64,
) -> Result<Address, TowerInstallError> {
    if request == 0
        || [
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
        ]
        .contains(&0)
        || scope.purpose != Purpose::Effect
        || validate_address(registration).is_err()
        || registration.kind != Noun::Run
        || registration
            .generation
            .is_none_or(|generation| generation == 0)
        || registration.tenant != scope.tenant.to_string()
        || registration.id != scope.run.to_string()
    {
        return Err(TowerInstallError::InvalidAddress);
    }
    Address::new(
        registration.tenant.clone(),
        Noun::Attempt,
        format!("{}{request}", request_prefix(scope)),
        registration.generation,
    )
    .map_err(|_| TowerInstallError::InvalidAddress)
}

fn request_prefix(scope: Scope) -> String {
    format!(
        "request-{}-{}-{}-{}-",
        scope.run, scope.principal, scope.branch, scope.authority
    )
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
