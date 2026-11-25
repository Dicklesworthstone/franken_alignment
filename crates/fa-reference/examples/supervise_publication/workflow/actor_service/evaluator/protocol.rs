//! Same closed assessment request; distinct live acknowledgment without a
//! fabricated recovery Fence or an assertion that independent custody was saved.
#[path = "protocol/case.rs"]
pub(crate) mod case;
#[path = "protocol/cases.rs"]
pub(crate) mod cases;
#[path = "protocol/pending.rs"]
pub(crate) mod pending;

use super::{
    Settings,
    assessment::{AssessmentRequest, MAX_ASSESSMENT_BYTES},
};
use crate::config::{debug, hex};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;

pub(super) const REQUEST_BYTES: usize = 20 * 1024;
pub(super) const RESPONSE_BYTES: usize = 24 * 1024;

pub(super) struct Request {
    pub(super) wire: Vec<u8>,
    pub(super) parsed: Json,
    pub(super) assessment: AssessmentRequest,
}
impl Request {
    pub(super) fn prepare(settings: &Settings, bytes: &[u8]) -> Result<Self, String> {
        AssessmentRequest::decode(bytes)?;
        let encoded: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let wire = format!(
            "{{\"schema_version\":\"fa.native-evaluator-request/1\",\"audience\":{},\"assessment_hex\":\"{encoded}\"}}",
            audience(settings)
        );
        Self::accept(settings, wire.as_bytes())
    }

    pub(super) fn accept(settings: &Settings, wire: &[u8]) -> Result<Self, String> {
        let parsed = parse(wire, REQUEST_BYTES)?;
        let root = fields(&parsed, &["schema_version", "audience", "assessment_hex"])?;
        expected(root, "schema_version", "fa.native-evaluator-request/1")?;
        if root["audience"] != parse(audience(settings).as_bytes(), REQUEST_BYTES)? {
            return Err("evaluator request audience does not match this original owner".into());
        }
        let encoded = root["assessment_hex"]
            .as_str()
            .ok_or("assessment_hex must be text")?;
        if encoded.len() > 2 * MAX_ASSESSMENT_BYTES {
            return Err("assessment exceeds the original byte bound".into());
        }
        let assessment = AssessmentRequest::decode(&hex(encoded)?)?;
        Ok(Self {
            wire: wire.to_vec(),
            parsed,
            assessment,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Basis {
    pub(super) journal: u64,
    pub(super) evaluation: u64,
    pub(super) control: u64,
    pub(super) epoch: u64,
}
impl Basis {
    pub(super) fn observe(host: &FileOversight) -> Result<Self, String> {
        let report = host.credibility_report().map_err(debug)?;
        let state = host.inspect();
        Ok(Self {
            journal: host.revision(),
            evaluation: report.revision,
            control: state.control.sequence,
            epoch: state.control.ledger.epoch,
        })
    }

    fn json(self) -> String {
        format!(
            "{{\"journal_revision\":\"{}\",\"evaluation_revision\":\"{}\",\"control_sequence\":\"{}\",\"authority_epoch\":\"{}\"}}",
            self.journal, self.evaluation, self.control, self.epoch
        )
    }

    fn decode(value: &Json) -> Result<Self, String> {
        let fields = fields(
            value,
            &[
                "journal_revision",
                "evaluation_revision",
                "control_sequence",
                "authority_epoch",
            ],
        )?;
        Ok(Self {
            journal: decimal(&fields["journal_revision"])?,
            evaluation: decimal(&fields["evaluation_revision"])?,
            control: decimal(&fields["control_sequence"])?,
            epoch: decimal(&fields["authority_epoch"])?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Applied,
    AlreadyApplied,
    Refused,
    Unconfirmed,
}
impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::AlreadyApplied => "already_applied",
            Self::Refused => "refused",
            Self::Unconfirmed => "unconfirmed",
        }
    }

    pub(super) fn completed(self) -> bool {
        matches!(self, Self::Applied | Self::AlreadyApplied)
    }
}

pub(super) fn receipt(
    request: &Request,
    outcome: Outcome,
    before: Basis,
    after: Option<Basis>,
) -> Result<Vec<u8>, String> {
    let after = after.map_or_else(|| "null".to_owned(), Basis::json);
    let wire = format!("{{\"schema_version\":\"fa.live-evaluator-receipt/1\",\"request\":{},\"status\":\"{}\",\"before\":{},\"after\":{after}}}",
        std::str::from_utf8(&request.wire).map_err(debug)?, outcome.name(), before.json()).into_bytes();
    validate(request, &wire)?;
    Ok(wire)
}

pub(super) fn validate(expected_request: &Request, wire: &[u8]) -> Result<Outcome, String> {
    let value = parse(wire, RESPONSE_BYTES)?;
    let root = fields(
        &value,
        &["schema_version", "request", "status", "before", "after"],
    )?;
    expected(root, "schema_version", "fa.live-evaluator-receipt/1")?;
    if root["request"] != expected_request.parsed {
        return Err("live evaluator receipt does not bind the submitted request".into());
    }
    let outcome = match root["status"].as_str() {
        Some("applied") => Outcome::Applied,
        Some("already_applied") => Outcome::AlreadyApplied,
        Some("refused") => Outcome::Refused,
        Some("unconfirmed") => Outcome::Unconfirmed,
        _ => return Err("unknown live evaluation acknowledgment".into()),
    };
    let before = Basis::decode(&root["before"])?;
    if root["after"].is_null() {
        return if outcome == Outcome::Unconfirmed {
            Ok(outcome)
        } else {
            Err("a completed or refused native assessment requires acknowledged state".into())
        };
    }
    let after = Basis::decode(&root["after"])?;
    let benign = expected_request.assessment.assessment.truth == GroundTruth::Benign;
    let expected_revision = expected_request.assessment.expected_revision;
    if outcome == Outcome::Unconfirmed
        || after.journal < before.journal
        || after.control != before.control
        || after.epoch != before.epoch
        || match outcome {
            Outcome::Applied => {
                before.evaluation != expected_revision
                    || before.journal.checked_add(if benign { 2 } else { 1 }) != Some(after.journal)
                    || before.evaluation.checked_add(1) != Some(after.evaluation)
            }
            Outcome::AlreadyApplied => before != after,
            Outcome::Refused => {
                after.evaluation != before.evaluation
                    || (after.journal != before.journal
                        && !(benign
                            && before.evaluation == expected_revision
                            && before.journal.checked_add(1) == Some(after.journal)))
            }
            Outcome::Unconfirmed => true,
        }
    {
        return Err("contradictory live native assessment acknowledgment".into());
    }
    Ok(outcome)
}

fn audience(settings: &Settings) -> String {
    let scope = settings.scope;
    format!(
        "{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{},\"deployment\":{},\"evaluator\":{}}}",
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        settings.deployment,
        settings.evaluator_id
    )
}

fn parse(bytes: &[u8], maximum: usize) -> Result<Json, String> {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: maximum,
            max_depth: 6,
            max_items: 128,
            max_string_bytes: MAX_ASSESSMENT_BYTES * 2,
        },
    )
    .map_err(debug)
}

fn fields<'a>(value: &'a Json, keys: &[&str]) -> Result<&'a BTreeMap<String, Json>, String> {
    let fields = value.as_object().ok_or("expected live evaluator object")?;
    if fields.len() != keys.len() || keys.iter().any(|key| !fields.contains_key(*key)) {
        return Err("missing or unsupported live evaluator field".into());
    }
    Ok(fields)
}

fn expected(fields: &BTreeMap<String, Json>, key: &str, value: &str) -> Result<(), String> {
    if fields[key].as_str() == Some(value) {
        Ok(())
    } else {
        Err(format!("invalid {key}"))
    }
}

fn decimal(value: &Json) -> Result<u64, String> {
    let text = value
        .as_str()
        .ok_or("revision must be canonical decimal text")?;
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err("revision must be canonical decimal text".into());
    }
    text.parse().map_err(debug)
}
