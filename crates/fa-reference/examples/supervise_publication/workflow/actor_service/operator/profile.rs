//! Explicit operator identity and disclosure; never selected by actor bytes.
use super::super::{Config, PeerProfile, Profile};
use crate::config::{debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_ABANDONMENT_OPERATIONS, MAX_ABANDONMENT_QUERIES, MAX_ABANDONMENT_REPLAY_EVENTS,
    MAX_CANCELLATION_OPERATIONS, MAX_CANCELLATION_QUERIES, MAX_CANCELLATION_REPLAY_EVENTS,
    MAX_CONVENE_OPERATIONS, MAX_CONVENE_QUERIES, MAX_EXPLANATION_EVENTS, MAX_EXPLANATION_QUERIES,
    MAX_FENCE_REPLAY_EVENTS, MAX_HOLD_DIAGNOSIS_QUERIES, MAX_INSPECTION_QUERIES,
    MAX_JOURNAL_TAIL_PAGE_EVENTS, MAX_JOURNAL_TAIL_QUERIES, MAX_NARROW_OPERATIONS,
    MAX_NARROW_QUERIES, MAX_NARROW_REPLAY_EVENTS, MAX_POLICY_PROMOTION_QUERIES,
    MAX_POLICY_PROMOTION_REPLAY_EVENTS, MAX_POLICY_REPLAY_PAGE_QUERIES, MAX_POLICY_REPLAY_QUERIES,
    MAX_REQUEST_HISTORY_EVENTS, MAX_REQUEST_HISTORY_QUERIES, MAX_REQUEST_HISTORY_REVIEWS,
    MAX_RESOLUTION_OPERATIONS, MAX_RESOLUTION_QUERIES, MAX_RESOLUTION_REPLAY_EVENTS,
    MAX_REVIEW_EXPLANATION_EVENTS, MAX_REVIEW_EXPLANATION_QUERIES,
};
use fa_reference::action::consequence::oversight::actor_peer::{MAX_PEER_CONNECTIONS, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::MAX_CHANNEL_EXCHANGES;
use fa_reference::action::consequence::policy_campaign::{
    MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, ReplayLimits,
};
use fa_reference::action::{Purpose, Scope};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::tower::publication::PublicationLimits;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) scope: Scope,
    pub(crate) socket: PathBuf,
    pub(crate) supervisor: PeerPolicy,
    pub(crate) operator: PeerPolicy,
    pub(crate) candidate_limit: u64,
    pub(crate) connection_limit: u64,
    pub(crate) exchange_limit: u64,
    pub(crate) response_bytes: usize,
    pub(crate) runtime_ms: u64,
    pub(crate) poll_ms: u64,
    pub(crate) inspection_queries: usize,
    pub(crate) explanation_queries: usize,
    pub(crate) explanation_replay_events: usize,
    /// Cumulative original transitions reserved before numerical admission replay.
    /// Omission keeps numerical admission replay disabled.
    pub(crate) explanation_numerical_replay_events: Option<usize>,
    pub(crate) explanation_disclosure: String,
    pub(crate) journal_tail: Option<JournalTailSettings>,
    pub(crate) request_history: Option<RequestHistorySettings>,
    pub(crate) hold_diagnosis: Option<HoldDiagnosisSettings>,
    pub(crate) resolution: Option<ResolutionSettings>,
    pub(crate) cancellation: Option<CancellationSettings>,
    pub(crate) abandonment: Option<AbandonmentSettings>,
    pub(crate) narrowing: Option<NarrowingSettings>,
    pub(crate) convening: Option<ConveningSettings>,
    pub(crate) fence: Option<FenceSettings>,
    pub(crate) review_explanation: Option<ReviewExplanationSettings>,
    pub(crate) policy_replay: Option<PolicyReplaySettings>,
    pub(crate) policy_promotion: Option<PolicyPromotionSettings>,
}

/// One independently approved campaign, delegated once during this service.
/// Selecting these bounds does not create a governor or an approval key.
#[derive(Clone, Debug)]
pub(crate) struct PolicyPromotionSettings {
    pub(crate) queries: usize,
    pub(crate) max_replay_events: usize,
    /// One separately admitted complete numerical history plus Promote.
    pub(crate) numerical_replay_events: Option<usize>,
    pub(crate) authority: String,
}

impl PolicyPromotionSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            max_replay_events: usize::try_from(fields.number("max_replay_events")?)
                .map_err(debug)?,
            numerical_replay_events: match fields.optional("numerical_replay_events") {
                None => None,
                Some(value) => Some(
                    usize::try_from(value.as_u64().ok_or(
                        "policy promotion numerical replay events must be an unsigned integer",
                    )?)
                    .map_err(debug)?,
                ),
            },
            authority: fields.text("authority")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_POLICY_PROMOTION_QUERIES
            || self.max_replay_events == 0
            || self.max_replay_events > MAX_POLICY_PROMOTION_REPLAY_EVENTS
            || self
                .numerical_replay_events
                .is_some_and(|events| events == 0 || events > self.max_replay_events)
            || self.authority != "promote_approved_policy"
        {
            return Err("policy promotion requires explicit approved-policy authority and bounded queries and replay".into());
        }
        Ok(())
    }
}

/// Direct original request-to-review relationships. This metadata selection
/// grants neither review-input disclosure nor numerical reconstruction work.
#[derive(Clone, Debug)]
pub(crate) struct RequestHistorySettings {
    pub(crate) queries: usize,
    pub(crate) max_history_events: usize,
    pub(crate) max_reviews: usize,
    pub(crate) disclosure: String,
}

impl RequestHistorySettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            max_history_events: usize::try_from(fields.number("max_history_events")?)
                .map_err(debug)?,
            max_reviews: usize::try_from(fields.number("max_reviews")?).map_err(debug)?,
            disclosure: fields.text("disclosure")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_REQUEST_HISTORY_QUERIES
            || self.max_history_events == 0
            || self.max_history_events > MAX_REQUEST_HISTORY_EVENTS
            || self.max_reviews == 0
            || self.max_reviews > MAX_REQUEST_HISTORY_REVIEWS
            || self.disclosure != "recorded_request_relationships"
        {
            return Err(
                "request history requires explicit recorded-request-relationships disclosure and bounded queries, history and reviews"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Explicit bounded disclosure of original journal event metadata. This does
/// not disclose payloads, helper ballots, model inputs or current authority.
#[derive(Clone, Debug)]
pub(crate) struct JournalTailSettings {
    pub(crate) queries: usize,
    pub(crate) events_per_page: usize,
    pub(crate) disclosure: String,
}

impl JournalTailSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            events_per_page: usize::try_from(fields.number("events_per_page")?).map_err(debug)?,
            disclosure: fields.text("disclosure")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_JOURNAL_TAIL_QUERIES
            || self.events_per_page == 0
            || self.events_per_page > MAX_JOURNAL_TAIL_PAGE_EVENTS
            || self.disclosure != "redacted_journal_events"
        {
            return Err(
                "journal tail requires explicit redacted-journal-events disclosure and bounded queries and pages"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Explicit reduction of the original ordinary run's target ceiling. This
/// grants neither publication nor learned-stream policy or checkpoint changes.
#[derive(Clone, Debug)]
pub(crate) struct NarrowingSettings {
    pub(crate) queries: usize,
    pub(crate) operations: usize,
    pub(crate) max_replay_events: usize,
    pub(crate) authority: String,
}

impl NarrowingSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            operations: usize::try_from(fields.number("operations")?).map_err(debug)?,
            max_replay_events: usize::try_from(fields.number("max_replay_events")?)
                .map_err(debug)?,
            authority: fields.text("authority")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_NARROW_QUERIES
            || self.operations == 0
            || self.operations > MAX_NARROW_OPERATIONS
            || self.max_replay_events == 0
            || self.max_replay_events > MAX_NARROW_REPLAY_EVENTS
            || self.authority != "narrow_original_targets"
        {
            return Err(
                "narrowing requires explicit original-target authority and bounded queries, operations and replay"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Explicit additional congress rounds for an original ordinary held request.
/// This allowance never changes the roster, grants either effect key or extends
/// the action deadline. Each successor requires a separate operator request.
#[derive(Clone, Debug)]
pub(crate) struct ConveningSettings {
    pub(crate) queries: usize,
    pub(crate) operations: usize,
    pub(crate) authority: String,
}

impl ConveningSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            operations: usize::try_from(fields.number("operations")?).map_err(debug)?,
            authority: fields.text("authority")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_CONVENE_QUERIES
            || self.operations == 0
            || self.operations > MAX_CONVENE_OPERATIONS
            || self.authority != "convene_held_review"
        {
            return Err(
                "convening requires explicit held-review authority and bounded queries and successor rounds"
                    .into(),
            );
        }
        Ok(())
    }
}

/// One explicitly selected permanent admission stop in the original domain.
/// Endpoint drain and remote actor halt require independent acknowledgments.
#[derive(Clone, Debug)]
pub(crate) struct FenceSettings {
    pub(crate) max_replay_events: usize,
    pub(crate) numerical_replay_events: Option<usize>,
    pub(crate) authority: String,
}

impl FenceSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            max_replay_events: usize::try_from(fields.number("max_replay_events")?)
                .map_err(debug)?,
            numerical_replay_events: fields
                .optional_number("numerical_replay_events")?
                .map(usize::try_from)
                .transpose()
                .map_err(debug)?,
            authority: fields.text("authority")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.max_replay_events == 0
            || self.max_replay_events > MAX_FENCE_REPLAY_EVENTS
            || self.authority != "stop_original_admission"
            || self
                .numerical_replay_events
                .is_some_and(|events| events == 0 || events > self.max_replay_events)
        {
            return Err(
                "fence requires explicit original-admission-stop authority and bounded replay"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Explicit permission for one original owner's bounded receipt reconciliation.
/// This is distinct from observation disclosure and never enables publication.
#[derive(Clone, Debug)]
pub(crate) struct ResolutionSettings {
    pub(crate) queries: usize,
    pub(crate) operations: usize,
    pub(crate) max_replay_events: usize,
    /// Cumulative complete-cut reservations for original numerical histories,
    /// including the new reconciliation event; not a measured CPU/work metric.
    pub(crate) numerical_replay_events: Option<usize>,
    pub(crate) authority: String,
}

impl ResolutionSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            operations: usize::try_from(fields.number("operations")?).map_err(debug)?,
            max_replay_events: usize::try_from(fields.number("max_replay_events")?)
                .map_err(debug)?,
            numerical_replay_events: fields
                .optional_number("numerical_replay_events")?
                .map(usize::try_from)
                .transpose()
                .map_err(debug)?,
            authority: fields.text("authority")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_RESOLUTION_QUERIES
            || self.operations == 0
            || self.operations > MAX_RESOLUTION_OPERATIONS
            || self.max_replay_events == 0
            || self.max_replay_events > MAX_RESOLUTION_REPLAY_EVENTS
            || self.authority != "reconcile_existing_receipts"
        {
            return Err(
                "resolution requires explicit receipt-reconciliation authority and bounded reads, operations and replay"
                    .into(),
            );
        }
        if self.numerical_replay_events.is_some_and(|events| {
            events == 0
                || self
                    .operations
                    .checked_mul(self.max_replay_events)
                    .is_none_or(|maximum| events > maximum)
        }) {
            return Err(
                "numerical resolution requires a positive cumulative event budget within the selected operation and replay bounds"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Separate authority to cancel an original request before dispatch. This does
/// not authorize receipt reconciliation, domain stop, actor intake or a Hold.
#[derive(Clone, Debug)]
pub(crate) struct CancellationSettings {
    pub(crate) queries: usize,
    pub(crate) operations: usize,
    pub(crate) max_replay_events: usize,
    /// Complete-cut reservations for numerical replay, including native Cancel.
    pub(crate) numerical_replay_events: Option<usize>,
    pub(crate) authority: String,
}

impl CancellationSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            operations: usize::try_from(fields.number("operations")?).map_err(debug)?,
            max_replay_events: usize::try_from(fields.number("max_replay_events")?)
                .map_err(debug)?,
            numerical_replay_events: fields
                .optional_number("numerical_replay_events")?
                .map(usize::try_from)
                .transpose()
                .map_err(debug)?,
            authority: fields.text("authority")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_CANCELLATION_QUERIES
            || self.operations == 0
            || self.operations > MAX_CANCELLATION_OPERATIONS
            || self.max_replay_events == 0
            || self.max_replay_events > MAX_CANCELLATION_REPLAY_EVENTS
            || self.authority != "cancel_undispatched_requests"
        {
            return Err(
                "cancellation requires explicit undispatched-request authority and bounded reads, operations and replay"
                    .into(),
            );
        }
        if self.numerical_replay_events.is_some_and(|events| {
            events == 0
                || self
                    .operations
                    .checked_mul(self.max_replay_events)
                    .is_none_or(|maximum| events > maximum)
        }) {
            return Err(
                "numerical cancellation requires a positive cumulative event budget within the selected operation and replay bounds"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Separate authority to abandon one original Unknown delivery. Its full charge
/// remains a liability; this grants no reconciliation, cancellation or admission.
#[derive(Clone, Debug)]
pub(crate) struct AbandonmentSettings {
    pub(crate) queries: usize,
    pub(crate) operations: usize,
    pub(crate) max_replay_events: usize,
    /// Complete-cut reservations for numerical replay, including native abandonment.
    pub(crate) numerical_replay_events: Option<usize>,
    pub(crate) authority: String,
}

impl AbandonmentSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            operations: usize::try_from(fields.number("operations")?).map_err(debug)?,
            max_replay_events: usize::try_from(fields.number("max_replay_events")?)
                .map_err(debug)?,
            numerical_replay_events: fields
                .optional_number("numerical_replay_events")?
                .map(usize::try_from)
                .transpose()
                .map_err(debug)?,
            authority: fields.text("authority")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_ABANDONMENT_QUERIES
            || self.operations == 0
            || self.operations > MAX_ABANDONMENT_OPERATIONS
            || self.max_replay_events == 0
            || self.max_replay_events > MAX_ABANDONMENT_REPLAY_EVENTS
            || self.authority != "abandon_unknown_requests"
        {
            return Err(
                "abandonment requires explicit unknown-delivery authority and bounded reads, operations and replay"
                    .into(),
            );
        }
        if self.numerical_replay_events.is_some_and(|events| {
            events == 0
                || self
                    .operations
                    .checked_mul(self.max_replay_events)
                    .is_none_or(|maximum| events > maximum)
        }) {
            return Err(
                "numerical abandonment requires a positive cumulative event budget within the selected operation and replay bounds"
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HoldDiagnosisSettings {
    pub(crate) queries: usize,
    pub(crate) disclosure: String,
}

impl HoldDiagnosisSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            disclosure: fields.text("disclosure")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_HOLD_DIAGNOSIS_QUERIES
            || self.disclosure != "recorded_publication_state"
        {
            return Err(
                "hold diagnosis requires explicit recorded-publication-state disclosure and bounded queries"
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ReviewExplanationSettings {
    pub(crate) queries: usize,
    pub(crate) max_replay_events: usize,
    /// Cumulative original transitions for numerical histories. Each admitted
    /// cut pays its full length; model work retains the original recipe limits.
    pub(crate) numerical_replay_events: Option<usize>,
    pub(crate) disclosure: String,
}

impl ReviewExplanationSettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            max_replay_events: usize::try_from(fields.number("max_replay_events")?)
                .map_err(debug)?,
            numerical_replay_events: fields
                .optional_number("numerical_replay_events")?
                .map(usize::try_from)
                .transpose()
                .map_err(debug)?,
            disclosure: fields.text("disclosure")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_REVIEW_EXPLANATION_QUERIES
            || self.max_replay_events == 0
            || self.max_replay_events > MAX_REVIEW_EXPLANATION_EVENTS
            || self.disclosure != "policy_and_congress_trace"
        {
            return Err("review explanation requires explicit policy-and-congress-trace disclosure and bounded replay".into());
        }
        if self.numerical_replay_events.is_some_and(|events| {
            events == 0
                || self
                    .queries
                    .checked_mul(self.max_replay_events)
                    .is_none_or(|maximum| events > maximum)
        }) {
            return Err("numerical review replay requires a positive cumulative event budget within the selected query and replay bounds".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PolicyReplaySettings {
    pub(crate) queries: usize,
    pub(crate) page_queries: usize,
    pub(crate) limits: ReplayLimits,
    pub(crate) disclosure: String,
}

impl PolicyReplaySettings {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = Fields::new(value)?;
        let selected = Self {
            queries: usize::try_from(fields.number("queries")?).map_err(debug)?,
            page_queries: match fields.optional_number("page_queries")? {
                None => 0,
                Some(0) => {
                    return Err("explicit policy replay page_queries must be positive".into());
                }
                Some(value) => usize::try_from(value).map_err(debug)?,
            },
            limits: ReplayLimits {
                cases: usize::try_from(fields.number("cases")?).map_err(debug)?,
                input_bytes: usize::try_from(fields.number("input_bytes")?).map_err(debug)?,
            },
            disclosure: fields.text("disclosure")?,
        };
        fields.end()?;
        selected.validate()?;
        Ok(selected)
    }

    fn validate(&self) -> Result<(), String> {
        if self.queries == 0
            || self.queries > MAX_POLICY_REPLAY_QUERIES
            || self.page_queries > MAX_POLICY_REPLAY_PAGE_QUERIES
            || self.limits.cases == 0
            || self.limits.cases > MAX_REPLAY_CASES
            || self.limits.input_bytes == 0
            || self.limits.input_bytes > MAX_REPLAY_INPUT_BYTES
            || self.disclosure != "policy_and_case_traces"
        {
            return Err("policy replay requires explicit policy-and-case-traces disclosure and bounded work".into());
        }
        Ok(())
    }
}

impl Settings {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, 16_384)?)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut fields = Fields::new(
            strict_json::parse(
                bytes,
                Limits {
                    max_bytes: 16_384,
                    max_depth: 4,
                    max_items: 128,
                    max_string_bytes: 4_096,
                },
            )
            .map_err(debug)?,
        )?;
        if fields.text("schema")? != "fa.tower-operator-service/1" {
            return Err("unsupported Tower operator service schema".into());
        }
        let mut audience = Fields::new(fields.take("scope")?)?;
        let scope = Scope {
            tenant: audience.number("tenant")?,
            principal: audience.number("principal")?,
            run: audience.number("run")?,
            branch: audience.number("branch")?,
            authority: audience.number("authority")?,
            purpose: Purpose::Effect,
        };
        audience.end()?;
        let settings = Self {
            scope,
            socket: PathBuf::from(fields.text("socket")?),
            supervisor: identity(fields.take("supervisor")?)?,
            operator: identity(fields.take("operator")?)?,
            candidate_limit: fields.number("candidate_limit")?,
            connection_limit: fields.number("connection_limit")?,
            exchange_limit: fields.number("exchange_limit")?,
            response_bytes: fields
                .optional_number("response_bytes")?
                .map(usize::try_from)
                .transpose()
                .map_err(debug)?
                .unwrap_or_else(|| PublicationLimits::default().max_response_bytes),
            runtime_ms: fields.number("runtime_ms")?,
            poll_ms: fields.number("poll_ms")?,
            inspection_queries: usize::try_from(fields.number("inspection_queries")?)
                .map_err(debug)?,
            explanation_queries: usize::try_from(fields.number("explanation_queries")?)
                .map_err(debug)?,
            explanation_replay_events: usize::try_from(fields.number("explanation_replay_events")?)
                .map_err(debug)?,
            explanation_numerical_replay_events: fields
                .optional_number("explanation_numerical_replay_events")?
                .map(usize::try_from)
                .transpose()
                .map_err(debug)?,
            explanation_disclosure: fields.text("explanation_disclosure")?,
            journal_tail: match fields.optional("journal_tail") {
                None | Some(Json::Null) => None,
                Some(value) => Some(JournalTailSettings::decode(value)?),
            },
            request_history: match fields.optional("request_history") {
                None | Some(Json::Null) => None,
                Some(value) => Some(RequestHistorySettings::decode(value)?),
            },
            hold_diagnosis: match fields.optional("hold_diagnosis") {
                None | Some(Json::Null) => None,
                Some(value) => Some(HoldDiagnosisSettings::decode(value)?),
            },
            resolution: match fields.optional("resolution") {
                None | Some(Json::Null) => None,
                Some(value) => Some(ResolutionSettings::decode(value)?),
            },
            cancellation: match fields.optional("cancellation") {
                None | Some(Json::Null) => None,
                Some(value) => Some(CancellationSettings::decode(value)?),
            },
            abandonment: match fields.optional("abandonment") {
                None | Some(Json::Null) => None,
                Some(value) => Some(AbandonmentSettings::decode(value)?),
            },
            narrowing: match fields.optional("narrowing") {
                None | Some(Json::Null) => None,
                Some(value) => Some(NarrowingSettings::decode(value)?),
            },
            convening: match fields.optional("convening") {
                None | Some(Json::Null) => None,
                Some(value) => Some(ConveningSettings::decode(value)?),
            },
            fence: match fields.optional("fence") {
                None | Some(Json::Null) => None,
                Some(value) => Some(FenceSettings::decode(value)?),
            },
            review_explanation: match fields.optional("review_explanation") {
                None | Some(Json::Null) => None,
                Some(value) => Some(ReviewExplanationSettings::decode(value)?),
            },
            policy_replay: match fields.optional("policy_replay") {
                None | Some(Json::Null) => None,
                Some(value) => Some(PolicyReplaySettings::decode(value)?),
            },
            policy_promotion: match fields.optional("policy_promotion") {
                None | Some(Json::Null) => None,
                Some(value) => Some(PolicyPromotionSettings::decode(value)?),
            },
        };
        fields.end()?;
        settings.validate()?;
        Ok(settings)
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if !(PublicationLimits {
            max_response_bytes: self.response_bytes,
        })
        .valid()
        {
            return Err(
                "operator response_bytes exceeds the native publication response bounds".into(),
            );
        }
        if self.scope.purpose != Purpose::Effect
            || [
                self.scope.tenant,
                self.scope.principal,
                self.scope.run,
                self.scope.branch,
                self.scope.authority,
            ]
            .contains(&0)
        {
            return Err("operator audience identifiers must be nonzero in the effect scope".into());
        }
        let normalized: PathBuf = self.socket.components().collect();
        if !self.socket.is_absolute()
            || self.socket.file_name().is_none()
            || normalized.as_os_str() != self.socket.as_os_str()
            || self
                .socket
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || self.socket.as_os_str().as_encoded_bytes().contains(&0)
            || self.socket.as_os_str().as_encoded_bytes().len() > 100
        {
            return Err(
                "operator socket must be an absolute normalized path of at most 100 bytes".into(),
            );
        }
        if self.candidate_limit == 0
            || self.candidate_limit > MAX_PEER_CONNECTIONS
            || self.connection_limit == 0
            || self.connection_limit > self.candidate_limit
            || self.exchange_limit == 0
            || self.exchange_limit > MAX_CHANNEL_EXCHANGES
            || self.runtime_ms == 0
            || self.runtime_ms > 3_600_000
            || self.poll_ms == 0
            || self.poll_ms > 1_000
            || self.poll_ms > self.runtime_ms
            || self.inspection_queries > MAX_INSPECTION_QUERIES
            || self.explanation_queries > MAX_EXPLANATION_QUERIES
            || (self.inspection_queries == 0
                && self.explanation_queries == 0
                && self.journal_tail.is_none()
                && self.request_history.is_none()
                && self.hold_diagnosis.is_none()
                && self.resolution.is_none()
                && self.cancellation.is_none()
                && self.abandonment.is_none()
                && self.narrowing.is_none()
                && self.convening.is_none()
                && self.fence.is_none()
                && self.review_explanation.is_none()
                && self.policy_replay.is_none()
                && self.policy_promotion.is_none())
        {
            return Err("operator service work or lifetime limit is out of range".into());
        }
        if self.explanation_queries == 0 {
            if self.explanation_replay_events != 0
                || self.explanation_numerical_replay_events.is_some()
                || self.explanation_disclosure != "none"
            {
                return Err(
                    "disabled explanation requires no replay work and disclosure none".into(),
                );
            }
        } else if self.explanation_replay_events == 0
            || self.explanation_replay_events > MAX_EXPLANATION_EVENTS
            || self.explanation_disclosure != "policy_and_selected_witnesses"
        {
            return Err("explanation requires explicit policy-and-selected-witness disclosure and bounded replay".into());
        }
        if self
            .explanation_numerical_replay_events
            .is_some_and(|events| {
                events == 0
                    || self
                        .explanation_queries
                        .checked_mul(self.explanation_replay_events)
                        .is_none_or(|maximum| events > maximum)
            })
        {
            return Err("numerical admission explanation requires a positive cumulative event budget within the selected query and replay bounds".into());
        }
        if let Some(selected) = &self.policy_replay {
            selected.validate()?;
        }
        if let Some(selected) = &self.policy_promotion {
            selected.validate()?;
        }
        if let Some(selected) = &self.journal_tail {
            selected.validate()?;
        }
        if let Some(selected) = &self.request_history {
            selected.validate()?;
        }
        if let Some(selected) = &self.hold_diagnosis {
            selected.validate()?;
        }
        if let Some(selected) = &self.review_explanation {
            selected.validate()?;
        }
        if let Some(selected) = &self.resolution {
            selected.validate()?;
        }
        if let Some(selected) = &self.cancellation {
            selected.validate()?;
        }
        if let Some(selected) = &self.abandonment {
            selected.validate()?;
        }
        if let Some(selected) = &self.narrowing {
            selected.validate()?;
        }
        if let Some(selected) = &self.convening {
            selected.validate()?;
        }
        if let Some(selected) = &self.fence {
            selected.validate()?;
        }
        Ok(())
    }

    /// Learned observation retains its original read-only defaults. Selecting
    /// reconciliation, request cancellation, unknown abandonment or admission stop
    /// requires explicit
    /// numerical work admission before clock/source acquisition, owner startup
    /// or listener creation.
    pub(crate) fn check_learned_operator(&self) -> Result<(), String> {
        self.check_numerical_operator("learned", false)
    }

    /// Only the separately selected learned governor composition may delegate
    /// a paid original approval into this learned operator.
    pub(crate) fn check_governed_learned_operator(&self) -> Result<(), String> {
        if self
            .policy_promotion
            .as_ref()
            .is_none_or(|selected| selected.numerical_replay_events.is_none())
        {
            return Err(
                "governed learned promotion requires an explicit numerical replay budget".into(),
            );
        }
        self.check_numerical_operator("learned", true)
    }

    /// Native decoder histories require the same independent numerical work
    /// reservations. An ordinary allowance cannot silently fund reconstruction.
    pub(crate) fn check_generated_operator(&self) -> Result<(), String> {
        self.check_numerical_operator("native text", false)
    }

    fn check_numerical_operator(&self, profile: &str, governed: bool) -> Result<(), String> {
        self.validate()?;
        if self.policy_promotion.is_some() && !governed {
            return Err(
                "numerical policy promotion requires the separately selected learned governor"
                    .into(),
            );
        }
        if self.convening.is_some() {
            return Err(
                "held-review convening supports only original ordinary publication runs".into(),
            );
        }
        if self.narrowing.is_some() {
            return Err(
                "target-ceiling narrowing supports only original ordinary publication runs".into(),
            );
        }
        if self
            .resolution
            .as_ref()
            .is_some_and(|selected| selected.numerical_replay_events.is_none())
        {
            return Err(format!(
                "{profile} receipt reconciliation requires an explicit numerical replay budget"
            ));
        }
        if self
            .cancellation
            .as_ref()
            .is_some_and(|selected| selected.numerical_replay_events.is_none())
        {
            return Err(format!(
                "{profile} request cancellation requires an explicit numerical replay budget"
            ));
        }
        if self
            .abandonment
            .as_ref()
            .is_some_and(|selected| selected.numerical_replay_events.is_none())
        {
            return Err(format!(
                "{profile} unknown abandonment requires an explicit numerical replay budget"
            ));
        }
        if self
            .fence
            .as_ref()
            .is_some_and(|selected| selected.numerical_replay_events.is_none())
        {
            return Err(format!(
                "{profile} admission stop requires an explicit numerical replay budget"
            ));
        }
        Ok(())
    }

    /// Audience and declared server identity must match the original service.
    /// The operator identity is independently selected, never inherited from
    /// the actor or reviewer and never widened by connecting to another socket.
    pub(super) fn check_actor(&self, actor: &Profile) -> Result<(), String> {
        self.validate()?;
        if self.scope != actor.scope || self.supervisor != actor.supervisor {
            return Err(
                "operator audience/server identity does not match the original actor service"
                    .into(),
            );
        }
        if self.socket == actor.socket {
            return Err("operator and actor sockets must be distinct".into());
        }
        self.check_directory()
    }

    /// Preflight before source, time or persistent setup. Existing files,
    /// sockets and symlinks are never replaced to make the sidecar start.
    pub(crate) fn check_host(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        additional_sockets: &[&Path],
    ) -> Result<(), String> {
        actor.check_host(config)?;
        reviewer.check_host(config)?;
        self.check_actor(actor)?;
        if self.scope != config.profile.delivery.scope
            || self.socket == reviewer.socket(actor.request)
            || self.socket == super::super::super::control::socket_path(reviewer, actor.request)
            || additional_sockets
                .iter()
                .any(|path| self.socket.as_path() == *path)
        {
            return Err("operator audience or socket conflicts with the original service".into());
        }
        self.check_unbound()
    }

    pub(super) fn check_unbound(&self) -> Result<(), String> {
        match fs::symlink_metadata(&self.socket) {
            Ok(_) => Err("operator socket path already exists; no replacement is permitted".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(debug(error)),
        }
    }

    /// Public client-side check: no private Config, source or journal is read.
    pub(crate) fn check_directory(&self) -> Result<(), String> {
        self.validate()?;
        let metadata = fs::symlink_metadata(
            self.socket
                .parent()
                .ok_or("missing operator socket directory")?,
        )
        .map_err(debug)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.mode() & 0o022 != 0
            || metadata.uid() != self.supervisor.uid()
            || metadata.gid() != self.supervisor.gid()
        {
            return Err(
                "operator socket directory must be supervisor-owned and not group/other writable"
                    .into(),
            );
        }
        Ok(())
    }

    pub(crate) fn secure_socket(&self) -> Result<(), String> {
        let metadata = fs::symlink_metadata(&self.socket).map_err(debug)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != self.supervisor.uid()
            || metadata.gid() != self.supervisor.gid()
        {
            return Err("operator socket has unexpected ownership or type".into());
        }
        fs::set_permissions(&self.socket, fs::Permissions::from_mode(0o660)).map_err(debug)
    }
}

fn identity(value: Json) -> Result<PeerPolicy, String> {
    let mut fields = Fields::new(value)?;
    let uid = u32::try_from(fields.number("uid")?).map_err(debug)?;
    let gid = u32::try_from(fields.number("gid")?).map_err(debug)?;
    let pid = match fields.take("pid")? {
        Json::Null => None,
        value => Some(
            u32::try_from(
                value
                    .as_u64()
                    .ok_or("pid must be null or an unsigned integer")?,
            )
            .map_err(debug)?,
        ),
    };
    fields.end()?;
    PeerPolicy::new(uid, gid, pid).map_err(debug)
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value {
            Json::Object(fields) => Ok(Self(fields)),
            _ => Err("expected operator profile object".into()),
        }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0
            .remove(name)
            .ok_or_else(|| format!("missing operator field {name}"))
    }
    fn optional(&mut self, name: &str) -> Option<Json> {
        self.0.remove(name)
    }
    fn text(&mut self, name: &str) -> Result<String, String> {
        self.take(name)?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{name} must be text"))
    }
    fn number(&mut self, name: &str) -> Result<u64, String> {
        self.take(name)?
            .as_u64()
            .ok_or_else(|| format!("{name} must be an unsigned integer"))
    }
    fn optional_number(&mut self, name: &str) -> Result<Option<u64>, String> {
        self.optional(name)
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| format!("{name} must be an unsigned integer"))
            })
            .transpose()
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err("unknown operator service field".into())
        }
    }
}
