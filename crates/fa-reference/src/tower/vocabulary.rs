//! The exact registered vocabulary; registration does not imply a connected backend.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerbClass {
    Read,
    Rehearsal,
    Mutation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verb {
    Get,
    Related,
    Capabilities,
    When,
    Invalidate,
    Explain,
    Cost,
    Convene,
    Annotate,
    Situation,
    WhyHeld,
    Next,
    Tail,
    Propose,
    Precheck,
    Hold,
    Narrow,
    Suspend,
    Reset,
    Fence,
    StopFleet,
    Resolve,
    Verify,
    Handoff,
    ReplayPlan,
    Branch,
    Intervene,
    Replay,
    Rehearse,
    Doctor,
    Ledger,
    Findings,
    PolicyReplay,
    Promote,
    Rotate,
    Preregister,
    ServeLearnedGenerated,
    CompareLearnedPrompt,
}

impl Verb {
    pub const ALL: [Self; 38] = [
        Self::Get,
        Self::Related,
        Self::Capabilities,
        Self::When,
        Self::Invalidate,
        Self::Explain,
        Self::Cost,
        Self::Convene,
        Self::Annotate,
        Self::Situation,
        Self::WhyHeld,
        Self::Next,
        Self::Tail,
        Self::Propose,
        Self::Precheck,
        Self::Hold,
        Self::Narrow,
        Self::Suspend,
        Self::Reset,
        Self::Fence,
        Self::StopFleet,
        Self::Resolve,
        Self::Verify,
        Self::Handoff,
        Self::ReplayPlan,
        Self::Branch,
        Self::Intervene,
        Self::Replay,
        Self::Rehearse,
        Self::Doctor,
        Self::Ledger,
        Self::Findings,
        Self::PolicyReplay,
        Self::Promote,
        Self::Rotate,
        Self::Preregister,
        Self::ServeLearnedGenerated,
        Self::CompareLearnedPrompt,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Related => "related",
            Self::Capabilities => "capabilities",
            Self::When => "when",
            Self::Invalidate => "invalidate",
            Self::Explain => "explain",
            Self::Cost => "cost",
            Self::Convene => "convene",
            Self::Annotate => "annotate",
            Self::Situation => "situation",
            Self::WhyHeld => "why-held",
            Self::Next => "next",
            Self::Tail => "tail",
            Self::Propose => "propose",
            Self::Precheck => "precheck",
            Self::Hold => "hold",
            Self::Narrow => "narrow",
            Self::Suspend => "suspend",
            Self::Reset => "reset",
            Self::Fence => "fence",
            Self::StopFleet => "stop-fleet",
            Self::Resolve => "resolve",
            Self::Verify => "verify",
            Self::Handoff => "handoff",
            Self::ReplayPlan => "replay-plan",
            Self::Branch => "branch",
            Self::Intervene => "intervene",
            Self::Replay => "replay",
            Self::Rehearse => "rehearse",
            Self::Doctor => "doctor",
            Self::Ledger => "ledger",
            Self::Findings => "findings",
            Self::PolicyReplay => "policy-replay",
            Self::Promote => "promote",
            Self::Rotate => "rotate",
            Self::Preregister => "preregister",
            Self::ServeLearnedGenerated => "serve-learned-generated",
            Self::CompareLearnedPrompt => "compare-learned-prompt",
        }
    }

    pub const fn class(self) -> VerbClass {
        match self {
            Self::Get
            | Self::Related
            | Self::Capabilities
            | Self::When
            | Self::Explain
            | Self::Cost
            | Self::Situation
            | Self::WhyHeld
            | Self::Next
            | Self::Tail
            | Self::Verify
            | Self::ReplayPlan
            | Self::Doctor
            | Self::Ledger
            | Self::Findings => VerbClass::Read,
            Self::Precheck
            | Self::Branch
            | Self::Intervene
            | Self::Replay
            | Self::Rehearse
            | Self::PolicyReplay
            | Self::CompareLearnedPrompt => VerbClass::Rehearsal,
            Self::Invalidate
            | Self::Convene
            | Self::Annotate
            | Self::Propose
            | Self::Hold
            | Self::Narrow
            | Self::Suspend
            | Self::Reset
            | Self::Fence
            | Self::StopFleet
            | Self::Resolve
            | Self::Handoff
            | Self::Promote
            | Self::Rotate
            | Self::Preregister
            | Self::ServeLearnedGenerated => VerbClass::Mutation,
        }
    }

    /// Operations actually connected by the bounded reference read facade.
    pub const fn is_connected(self) -> bool {
        matches!(
            self,
            Self::Get
                | Self::Related
                | Self::Tail
                | Self::Situation
                | Self::Explain
                | Self::WhyHeld
                | Self::Next
                | Self::Replay
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownVerb;

impl std::str::FromStr for Verb {
    type Err = UnknownVerb;
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|verb| verb.as_str() == raw)
            .ok_or(UnknownVerb)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasonCode {
    DeniedByPolicy,
    MissingRequiredObservation,
    StaleGeneration,
    CapabilityUnsupported,
    BudgetExhausted,
    PermitConsumed,
    DeadlineExpired,
    OutcomeUnknown,
    ReplayClosureUnavailable,
    IntegrityFailure,
    PurposeMismatch,
    LowerLayerUnverified,
}

impl ReasonCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeniedByPolicy => "DeniedByPolicy",
            Self::MissingRequiredObservation => "MissingRequiredObservation",
            Self::StaleGeneration => "StaleGeneration",
            Self::CapabilityUnsupported => "CapabilityUnsupported",
            Self::BudgetExhausted => "BudgetExhausted",
            Self::PermitConsumed => "PermitConsumed",
            Self::DeadlineExpired => "DeadlineExpired",
            Self::OutcomeUnknown => "OutcomeUnknown",
            Self::ReplayClosureUnavailable => "ReplayClosureUnavailable",
            Self::IntegrityFailure => "IntegrityFailure",
            Self::PurposeMismatch => "PurposeMismatch",
            Self::LowerLayerUnverified => "LowerLayerUnverified",
        }
    }
}

pub const ENVELOPE_FIELDS: [&str; 16] = [
    "schema_version",
    "operation_id",
    "status",
    "reason_code",
    "address",
    "generation",
    "control_seq",
    "source_frontiers",
    "missing",
    "limits",
    "budgets_remaining",
    "realized_cost",
    "affordances",
    "retry_safe",
    "idempotency_key",
    "would_change_this",
];
