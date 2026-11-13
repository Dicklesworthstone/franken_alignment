//! Closed custodian documents. Parsing supplies data, never a governor or key.
use crate::config::debug;
use crate::policy_replay::Request as Candidate;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileCampaignRequirement;
use fa_reference::action::consequence::oversight::actor::Knowledge;
use fa_reference::action::{Purpose, Scope};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub(crate) const MAX_REQUEST_BYTES: usize = 2 * 1_048_576;
pub(crate) const MAX_REPORT_BYTES: usize = 1_048_576;
pub(crate) const MAX_RESPONSE_BYTES: usize = 2 * MAX_REPORT_BYTES + 4096;
pub(crate) const REFUSAL_REASONS: &[&str] = &[
    "audience",
    "stale_predecessor",
    "binding",
    "capacity",
    "unavailable",
    "source_unavailable",
    "campaign_not_approved",
    "operator_unavailable",
    "report_mismatch",
    "unsupported",
    "idempotency_conflict",
    "operation_limit",
    "unconfirmed",
];

pub(crate) fn is_refusal_reason(reason: &str) -> bool {
    REFUSAL_REASONS.contains(&reason)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Expected {
    pub(crate) journal_revision: u64,
    pub(crate) control_sequence: u64,
    pub(crate) authority_epoch: u64,
    pub(crate) policy_generation: u64,
}

pub(crate) enum Command {
    Inspect {
        query: u64,
    },
    Review {
        operation: u64,
        expected: Expected,
        candidate: Candidate,
    },
    Approve {
        campaign: u64,
        expected_journal_revision: u64,
        reviewed_report: String,
        accept_newly_reviewable: bool,
    },
}

/// A query identity is not a campaign identity, even when the counters match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Correlation {
    Query(u64),
    Campaign(u64),
}
impl Correlation {
    pub(crate) fn field(self) -> &'static str {
        match self {
            Self::Query(_) => "query",
            Self::Campaign(_) => "campaign",
        }
    }
    pub(crate) fn id(self) -> u64 {
        match self {
            Self::Query(id) | Self::Campaign(id) => id,
        }
    }
}

/// One immutable native cut and the custodian's originally selected bounds.
/// Source interruption is the original latch, not a new source observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Inspection {
    pub(crate) cut: Expected,
    pub(crate) campaigns: FileCampaignRequirement,
    pub(crate) campaign_limit: usize,
    pub(crate) report_bytes: usize,
    pub(crate) stopped: bool,
    pub(crate) source_interrupted: Option<bool>,
    pub(crate) numerical_replay: Option<NumericalReplayInspection>,
}

/// Observed remaining work, never an allowance created by reading this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NumericalReplayInspection {
    pub(crate) selected: super::profile::NumericalReplaySettings,
    pub(crate) remaining_events: usize,
}

pub(crate) struct Request {
    pub(crate) scope: Scope,
    pub(crate) command: Command,
    /// Property order and whitespace do not create another operation identity.
    pub(crate) canonical: Vec<u8>,
}

impl Request {
    pub(crate) fn correlation(&self) -> Correlation {
        match &self.command {
            Command::Inspect { query } => Correlation::Query(*query),
            Command::Review { operation, .. } => Correlation::Campaign(*operation),
            Command::Approve { campaign, .. } => Correlation::Campaign(*campaign),
        }
    }
    pub(crate) fn verb(&self) -> &'static str {
        match self.command {
            Command::Inspect { .. } => "get",
            Command::Review { .. } => "policy-replay",
            Command::Approve { .. } => "promote",
        }
    }
    pub(crate) fn phase(&self) -> &'static str {
        match self.command {
            Command::Inspect { .. } => "inspect",
            Command::Review { .. } => "review",
            Command::Approve { .. } => "approve",
        }
    }
}

pub(crate) fn decode(bytes: &[u8], max_report_bytes: usize) -> Result<Request, String> {
    if max_report_bytes == 0 || max_report_bytes > MAX_REPORT_BYTES {
        return Err("invalid governor report bound".into());
    }
    let value = strict_json::parse(
        bytes,
        Limits {
            max_bytes: MAX_REQUEST_BYTES,
            max_depth: 16,
            max_items: 16_512,
            max_string_bytes: MAX_REPORT_BYTES,
        },
    )
    .map_err(debug)?;
    let canonical = json(&value).into_bytes();
    if canonical.len() > MAX_REQUEST_BYTES {
        return Err("governor request exceeds its byte bound".into());
    }
    let mut fields = Fields::new(value)?;
    if fields.text("schema")? != "fa.policy-governor-request/1" {
        return Err("unsupported policy governor request schema".into());
    }
    let scope = decode_scope(fields.take("scope")?)?;
    let verb = fields.text("verb")?;
    let command = match verb.as_str() {
        "get" => {
            let query = fields.decimal("query")?;
            if query == 0 {
                return Err("governor inspection requires an explicit nonzero query".into());
            }
            Command::Inspect { query }
        }
        "policy-replay" => {
            let operation = fields.decimal("operation")?;
            let mut expected = Fields::new(fields.take("expected")?)?;
            let predecessor = Expected {
                journal_revision: expected.decimal("journal_revision")?,
                control_sequence: expected.decimal("control_sequence")?,
                authority_epoch: expected.decimal("authority_epoch")?,
                policy_generation: expected.decimal("policy_generation")?,
            };
            expected.end()?;
            let candidate = Candidate::decode(json(&fields.take("candidate")?).as_bytes())?;
            if operation == 0
                || predecessor.journal_revision == 0
                || predecessor.policy_generation == 0
                || candidate.expected_policy_generation != predecessor.policy_generation
                || candidate.output_bytes > max_report_bytes
            {
                return Err(
                    "governor campaign selection is zero, inconsistent or exceeds report bounds"
                        .into(),
                );
            }
            Command::Review {
                operation,
                expected: predecessor,
                candidate,
            }
        }
        "promote" => {
            if fields.text("phase")? != "approve" {
                return Err("governor Promote requires the explicit approve phase".into());
            }
            let campaign = fields.decimal("campaign")?;
            let expected_journal_revision = fields.decimal("expected_journal_revision")?;
            let reviewed_report = fields.text("reviewed_report")?;
            let accept_newly_reviewable = fields
                .take("accept_newly_reviewable")?
                .as_bool()
                .ok_or("accept_newly_reviewable must be an explicit Boolean")?;
            if campaign == 0
                || expected_journal_revision == 0
                || reviewed_report.is_empty()
                || reviewed_report.len() > max_report_bytes
            {
                return Err("governor approval is zero, empty or exceeds the report bound".into());
            }
            Command::Approve {
                campaign,
                expected_journal_revision,
                reviewed_report,
                accept_newly_reviewable,
            }
        }
        _ => return Err("unsupported governor verb".into()),
    };
    fields.end()?;
    Ok(Request {
        scope,
        command,
        canonical,
    })
}

pub(crate) fn decode_scope(value: Json) -> Result<Scope, String> {
    let mut fields = Fields::new(value)?;
    let scope = Scope {
        tenant: fields.decimal("tenant")?,
        principal: fields.decimal("principal")?,
        run: fields.decimal("run")?,
        branch: fields.decimal("branch")?,
        authority: fields.decimal("authority")?,
        purpose: Purpose::Effect,
    };
    if fields.text("purpose")? != "effect"
        || [
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
        ]
        .contains(&0)
    {
        return Err("governor requires a complete nonzero effect audience".into());
    }
    fields.end()?;
    Ok(scope)
}

/// Observation of the original native owner's acknowledgment. This basis is
/// historical evidence; it is neither an effect permit nor a fresh journal cut.
struct AcknowledgmentBasis {
    scope: Scope,
    correlation: Correlation,
    journal_revision: u64,
}

enum Acknowledgment<'a> {
    Inspected(Inspection),
    Reviewed(&'a str),
    Delegated,
}

fn acknowledged(
    request: &Request,
    revision: u64,
    value: Acknowledgment<'_>,
) -> Result<Vec<u8>, String> {
    response(
        request,
        Ok(Knowledge::Known {
            value,
            basis: AcknowledgmentBasis {
                scope: request.scope,
                correlation: request.correlation(),
                journal_revision: revision,
            },
        }),
    )
}

pub(crate) fn inspected(request: &Request, inspection: Inspection) -> Result<Vec<u8>, String> {
    acknowledged(
        request,
        inspection.cut.journal_revision,
        Acknowledgment::Inspected(inspection),
    )
}

pub(crate) fn review(request: &Request, revision: u64, report: &str) -> Result<Vec<u8>, String> {
    acknowledged(request, revision, Acknowledgment::Reviewed(report))
}

pub(crate) fn approved(request: &Request, revision: u64) -> Result<Vec<u8>, String> {
    acknowledged(request, revision, Acknowledgment::Delegated)
}

pub(crate) fn refused(request: &Request, reason: &str) -> Vec<u8> {
    // Only the closed local reason vocabulary is supplied by this service.
    let reason = if is_refusal_reason(reason) {
        reason
    } else {
        "unavailable"
    };
    response(request, Err(reason)).expect("bounded fixed refusal fields")
}

fn response(
    request: &Request,
    result: Result<Knowledge<Acknowledgment<'_>, AcknowledgmentBasis>, &str>,
) -> Result<Vec<u8>, String> {
    let result = match result {
        Ok(Knowledge::Known { value, basis }) => {
            let revision = basis.journal_revision;
            let value = match value {
                Acknowledgment::Inspected(inspection) => format!(
                    concat!(
                        "{{\"journal_revision\":\"{}\",\"control_sequence\":\"{}\",",
                        "\"authority_epoch\":\"{}\",\"policy_generation\":\"{}\",",
                        "\"campaigns\":{{\"cases\":\"{}\",\"input_bytes\":\"{}\",\"max_campaigns\":\"{}\"}},",
                        "\"campaign_limit\":\"{}\",\"report_bytes\":\"{}\",",
                        "\"stopped\":{},\"source_interrupted\":{}{}}}"
                    ),
                    revision,
                    inspection.cut.control_sequence,
                    inspection.cut.authority_epoch,
                    inspection.cut.policy_generation,
                    inspection.campaigns.limits.cases,
                    inspection.campaigns.limits.input_bytes,
                    inspection.campaigns.max_campaigns,
                    inspection.campaign_limit,
                    inspection.report_bytes,
                    inspection.stopped,
                    inspection.source_interrupted.map_or("null", |interrupted| {
                        if interrupted { "true" } else { "false" }
                    }),
                    inspection.numerical_replay.map_or_else(String::new, |numerical| format!(
                        ",\"numerical_replay\":{{\"max_replay_events\":\"{}\",\"events\":\"{}\",\"remaining_events\":\"{}\"}}",
                        numerical.selected.max_replay_events,
                        numerical.selected.events,
                        numerical.remaining_events,
                    )),
                ),
                Acknowledgment::Reviewed(report) => format!(
                    "{{\"journal_revision\":\"{revision}\",\"reviewed_report\":{}}}",
                    quote(report)
                ),
                Acknowledgment::Delegated => format!(
                    "{{\"journal_revision\":\"{revision}\",\"approval\":\"delegated\",\"policy_changed\":false,\"effect_authority\":false}}"
                ),
            };
            let basis = format!(
                concat!(
                    "{{\"origin\":\"acknowledged_owner\",\"scope\":{},",
                    "\"{}\":\"{}\",\"journal_revision\":\"{}\"}}"
                ),
                scope_json(basis.scope),
                basis.correlation.field(),
                basis.correlation.id(),
                revision
            );
            format!("\"knowledge\":{{\"state\":\"known\",\"value\":{value},\"basis\":{basis}}}")
        }
        Ok(_) => return Err("governor acknowledgment requires known owner evidence".into()),
        // A command error is separate from epistemic state. In particular,
        // `unconfirmed` must not claim that a native transition did not happen.
        Err(reason) => format!("\"error\":{{\"code\":{}}}", quote(reason)),
    };
    let bytes = format!(
        concat!(
            "{{\"schema\":\"fa.policy-governor-response/1\",",
            "\"scope\":{},\"verb\":{},\"phase\":{},\"{}\":\"{}\",{}}}"
        ),
        scope_json(request.scope),
        quote(request.verb()),
        quote(request.phase()),
        request.correlation().field(),
        request.correlation().id(),
        result
    )
    .into_bytes();
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("governor response exceeds its byte bound".into());
    }
    Ok(bytes)
}

pub(crate) fn scope_json(scope: Scope) -> String {
    format!(
        concat!(
            "{{\"tenant\":\"{}\",\"principal\":\"{}\",\"run\":\"{}\",",
            "\"branch\":\"{}\",\"authority\":\"{}\",\"purpose\":\"effect\"}}"
        ),
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority
    )
}

pub(crate) fn decimal(value: &Json) -> Option<u64> {
    let text = value.as_str()?;
    let number = text.parse::<u64>().ok()?;
    (text == number.to_string()).then_some(number)
}

pub(crate) fn quote(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch <= '\u{1f}' => {
                write!(output, "\\u{:04x}", ch as u32).expect("String write");
            }
            ch => output.push(ch),
        }
    }
    output.push('"');
    output
}

/// Bounded syntax projection into the ORIGINAL candidate decoder, not another
/// policy parser. Parsed number lexemes and all string bytes remain exact.
pub(crate) fn json(value: &Json) -> String {
    match value {
        Json::Null => "null".into(),
        Json::Bool(value) => value.to_string(),
        Json::Number(value) => value.lexeme().into(),
        Json::String(value) => quote(value),
        Json::Array(values) => format!(
            "[{}]",
            values.iter().map(json).collect::<Vec<_>>().join(",")
        ),
        Json::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!("{}:{}", quote(key), json(value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value {
            Json::Object(fields) => Ok(Self(fields)),
            _ => Err("governor document requires an object".into()),
        }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0
            .remove(name)
            .ok_or_else(|| format!("missing governor field {name}"))
    }
    fn text(&mut self, name: &str) -> Result<String, String> {
        match self.take(name)? {
            Json::String(value) => Ok(value),
            _ => Err(format!("governor {name} must be text")),
        }
    }
    fn decimal(&mut self, name: &str) -> Result<u64, String> {
        decimal(&self.take(name)?)
            .ok_or_else(|| format!("governor {name} must be a canonical unsigned decimal string"))
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err("unknown governor field".into())
        }
    }
}
