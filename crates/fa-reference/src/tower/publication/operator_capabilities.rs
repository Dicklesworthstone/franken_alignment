//! Discover the original mounted supervisor grants without exercising them.
//! A configured route, a surviving grant and current permission are distinct.

use super::{
    PublicationResponse, PublicationResult, PublicationTower, ReadArguments, ReasonCode,
    TowerAccess, TowerRequest, Verb,
};
use crate::action::consequence::delivery::persistent::requests::actor::FileOperatorCapabilityObservation;
use crate::address_resolver::Address;
use crate::typestate_cx::PurposeTag;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorCapabilityKind {
    Situation,
    JournalTail,
    RequestHistory,
    GovernanceLedger,
    AdmissionExplanation,
    ReviewExplanation,
    HoldDiagnosis,
    Resolution,
    Cancellation,
    Abandonment,
    Narrowing,
    Convening,
    Fence,
    PolicyReplay,
    PolicyPromotion,
    CredibilityPromotion,
}

impl OperatorCapabilityKind {
    pub const ALL: [Self; 16] = [
        Self::Situation,
        Self::JournalTail,
        Self::RequestHistory,
        Self::GovernanceLedger,
        Self::AdmissionExplanation,
        Self::ReviewExplanation,
        Self::HoldDiagnosis,
        Self::Resolution,
        Self::Cancellation,
        Self::Abandonment,
        Self::Narrowing,
        Self::Convening,
        Self::Fence,
        Self::PolicyReplay,
        Self::PolicyPromotion,
        Self::CredibilityPromotion,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Situation => "situation",
            Self::JournalTail => "journal_tail",
            Self::RequestHistory => "request_history",
            Self::GovernanceLedger => "governance_ledger",
            Self::AdmissionExplanation => "admission_explanation",
            Self::ReviewExplanation => "review_explanation",
            Self::HoldDiagnosis => "hold_diagnosis",
            Self::Resolution => "resolution",
            Self::Cancellation => "cancellation",
            Self::Abandonment => "abandonment",
            Self::Narrowing => "narrowing",
            Self::Convening => "convening",
            Self::Fence => "fence",
            Self::PolicyReplay => "policy_replay",
            Self::PolicyPromotion => "policy_promotion",
            Self::CredibilityPromotion => "credibility_promotion",
        }
    }

    pub const fn verbs(self) -> &'static [Verb] {
        match self {
            Self::Situation => &[Verb::Situation],
            Self::JournalTail => &[Verb::Tail],
            Self::RequestHistory => &[Verb::Related],
            Self::GovernanceLedger => &[Verb::Ledger, Verb::Findings],
            Self::AdmissionExplanation | Self::ReviewExplanation => &[Verb::Explain],
            Self::HoldDiagnosis => &[Verb::WhyHeld],
            Self::Resolution | Self::Cancellation | Self::Abandonment => {
                &[Verb::Next, Verb::Resolve]
            }
            Self::Narrowing => &[Verb::Next, Verb::Narrow],
            Self::Convening => &[Verb::Next, Verb::Convene],
            Self::Fence => &[Verb::Fence],
            Self::PolicyReplay => &[Verb::PolicyReplay],
            Self::PolicyPromotion | Self::CredibilityPromotion => &[Verb::Next, Verb::Promote],
        }
    }

    pub const fn required_purpose(self) -> Option<PurposeTag> {
        match self {
            Self::Resolution
            | Self::Cancellation
            | Self::Abandonment
            | Self::Narrowing
            | Self::Convening
            | Self::Fence
            | Self::PolicyPromotion
            | Self::CredibilityPromotion => Some(PurposeTag::Permit),
            Self::PolicyReplay => Some(PurposeTag::Experiment),
            _ => None,
        }
    }

    pub const fn authority_required(self) -> &'static str {
        match self {
            Self::Situation => "supervisor_inspection",
            Self::JournalTail => "supervisor_journal_tail",
            Self::RequestHistory => "supervisor_request_history",
            Self::GovernanceLedger => "supervisor_governance_ledger",
            Self::AdmissionExplanation => "supervisor_explanation",
            Self::ReviewExplanation => "supervisor_review_explanation",
            Self::HoldDiagnosis => "supervisor_hold_diagnosis",
            Self::Resolution => "supervisor_resolution",
            Self::Cancellation => "supervisor_cancellation",
            Self::Abandonment => "supervisor_abandonment",
            Self::Narrowing => "supervisor_narrow",
            Self::Convening => "supervisor_convene",
            Self::Fence => "supervisor_fence",
            Self::PolicyReplay => "supervisor_policy_replay",
            Self::PolicyPromotion => "supervisor_policy_promotion",
            Self::CredibilityPromotion => "supervisor_credibility_promotion",
        }
    }

    fn connected(self, tower: &PublicationTower) -> bool {
        match self {
            Self::Abandonment
            | Self::Narrowing
            | Self::Convening
            | Self::PolicyPromotion
            | Self::CredibilityPromotion => tower.control_operator,
            Self::Cancellation | Self::Fence => {
                !tower.source_observer && (!tower.reconciliation_operator || tower.control_operator)
            }
            Self::Resolution => !tower.source_observer,
            _ => true,
        }
    }
}

/// Host configuration and caller-purpose facts are sealed separately from the
/// original grant observation. Even all three being present does not establish
/// execution eligibility. None means this grant was not installed on the mount.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperatorCapabilityEntry {
    kind: OperatorCapabilityKind,
    route_connected: bool,
    purpose_allowed: bool,
    native: Option<FileOperatorCapabilityObservation>,
}

impl OperatorCapabilityEntry {
    pub fn kind(&self) -> OperatorCapabilityKind {
        self.kind
    }
    pub fn route_connected(&self) -> bool {
        self.route_connected
    }
    pub fn purpose_allowed(&self) -> bool {
        self.purpose_allowed
    }
    pub fn configured(&self) -> bool {
        self.native.is_some()
    }
    pub fn native(&self) -> Option<&FileOperatorCapabilityObservation> {
        self.native.as_ref()
    }
}

/// Original session-local selection and native custody observations, not a
/// signed assurance profile, a source capture, an approval or an effect permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedOperatorCapabilities {
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
    entries: [OperatorCapabilityEntry; 16],
}

impl LinkedOperatorCapabilities {
    pub fn registration(&self) -> &Address {
        &self.registration
    }
    pub fn query_address(&self) -> &Address {
        &self.query_address
    }
    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }
    pub fn query_idempotency_key(&self) -> Option<&str> {
        self.query_idempotency_key.as_deref()
    }
    pub fn entries(&self) -> &[OperatorCapabilityEntry; 16] {
        &self.entries
    }
}

impl PublicationTower {
    pub(super) fn dispatch_operator_capabilities(
        &self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        if !access.operator {
            return self.withheld(&request);
        }
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        }
        if request.arguments != ReadArguments::OperatorCapabilities {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        }
        let observations = [
            self.inspection
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.journal_tail
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.request_history
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.governance_ledger
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.explanation
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.review_explanation
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.hold_diagnosis
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.resolution
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.cancellation
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.abandonment
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.narrowing
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.convening
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.fence.as_ref().map(|port| port.capability_snapshot()),
            self.policy_replay
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.policy_promotion
                .as_ref()
                .map(|port| port.capability_snapshot()),
            self.credibility_promotion
                .as_ref()
                .map(|port| port.capability_snapshot()),
        ];
        let mut index = 0;
        let entries = observations.map(|native| {
            let kind = OperatorCapabilityKind::ALL[index];
            index += 1;
            OperatorCapabilityEntry {
                kind,
                route_connected: kind.connected(self),
                purpose_allowed: kind
                    .required_purpose()
                    .is_none_or(|purpose| access.allowed_purposes.contains(&purpose)),
                native,
            }
        });
        let linked = LinkedOperatorCapabilities {
            registration: self.mount.clone(),
            query_address: request.address.clone(),
            query_operation_id: request.operation_id.clone(),
            query_idempotency_key: request.idempotency_key.clone(),
            entries,
        };
        self.bounded_response(self.response(
            &request,
            PublicationResult::OperatorCapabilities(linked),
            None,
            true,
        ))
    }
}
