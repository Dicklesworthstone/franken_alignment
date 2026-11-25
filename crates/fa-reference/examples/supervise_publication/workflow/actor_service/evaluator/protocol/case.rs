//! Read an exact original evaluation case through the separately authenticated
//! evaluator endpoint. The copied identity is an observation, never a ticket,
//! label, approval, fresh clock observation or reserved evaluation predecessor.
use super::{
    Basis, REQUEST_BYTES, RESPONSE_BYTES, Settings, audience, debug, decimal, expected, fields,
    hex, parse,
};
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError,
    observed::{FileOversight, credibility::FileIndependentEvaluator},
};
use fa_reference::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use fa_reference::strict_json::Json;

pub(crate) struct CaseRequest {
    pub(crate) wire: Vec<u8>,
    pub(crate) parsed: Json,
    pub(crate) round: u64,
}

impl CaseRequest {
    pub(crate) fn prepare(settings: &Settings, round: u64) -> Result<Self, String> {
        let wire = format!(
            "{{\"schema_version\":\"fa.native-evaluator-case-request/1\",\"audience\":{},\"round\":\"{round}\"}}",
            audience(settings)
        );
        Self::accept(settings, wire.as_bytes())
    }

    pub(crate) fn accept(settings: &Settings, wire: &[u8]) -> Result<Self, String> {
        let parsed = parse(wire, REQUEST_BYTES)?;
        let root = fields(&parsed, &["schema_version", "audience", "round"])?;
        expected(root, "schema_version", "fa.native-evaluator-case-request/1")?;
        if root["audience"] != parse(audience(settings).as_bytes(), REQUEST_BYTES)? {
            return Err(
                "case request audience does not match this original evaluator owner".into(),
            );
        }
        let round = positive(&root["round"])?;
        Ok(Self {
            wire: wire.to_vec(),
            parsed,
            round,
        })
    }

    /// The evaluator brand is checked BEFORE observing any host basis. In
    /// particular, a replacement owner with identical numeric scope cannot
    /// disclose its cases or revisions through the predecessor's evaluator.
    pub(crate) fn observe(
        &self,
        host: &FileOversight,
        evaluator: &FileIndependentEvaluator,
    ) -> Result<Vec<u8>, String> {
        let knowledge = match evaluator.evaluation_case(host, self.round) {
            Ok(ticket) => {
                let original = ticket.case();
                Knowledge::Known {
                    value: CaseIdentity {
                        round: original.round,
                        attempt: original.attempt,
                        policy: original.policy_generation,
                        congress: original.congress_generation,
                        root: original.evidence_root,
                    },
                    basis: Basis::observe(host)?,
                }
            }
            // Missing native cases do not manufacture a descriptor from a
            // request, incomplete congress, replay or unacknowledged image.
            Err(JournalError::Contract(Error::Missing)) => Knowledge::Unknown {
                reason: UnknownReason::ControllerUnavailable,
            },
            Err(error) => return Err(debug(error)),
        };
        self.encode(&knowledge)
    }

    fn encode(&self, observation: &Knowledge<CaseIdentity, Basis>) -> Result<Vec<u8>, String> {
        let knowledge = match observation {
            Knowledge::Known { value, basis } => {
                let root: String = value
                    .root
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                format!(
                    "{{\"state\":\"known\",\"basis\":{},\"value\":{{\"round\":\"{}\",\"attempt\":\"{}\",\"policy_generation\":\"{}\",\"congress_generation\":\"{}\",\"case_evidence_root_hex\":\"{root}\"}}}}",
                    basis.json(),
                    value.round,
                    value.attempt,
                    value.policy,
                    value.congress,
                )
            }
            Knowledge::Unknown {
                reason: UnknownReason::ControllerUnavailable,
            } => "{\"state\":\"unknown\",\"reason\":\"case_not_recorded\"}".to_owned(),
            _ => return Err("unsupported native case observation".into()),
        };
        let wire = format!(
            "{{\"schema_version\":\"fa.live-evaluator-case/1\",\"request\":{},\"knowledge\":{knowledge},\"effect_authority\":false}}",
            std::str::from_utf8(&self.wire).map_err(debug)?
        )
        .into_bytes();
        validate(self, &wire)?;
        Ok(wire)
    }
}

/// Minimal case identity. Deliberately excludes the original member verdicts,
/// retained input, labels and owner-branded native evaluation ticket.
struct CaseIdentity {
    round: u64,
    attempt: u64,
    policy: u64,
    congress: u64,
    root: [u8; 32],
}

/// Validate the declared request-bound observation, not independent ground
/// truth. A Known revision is historical and can become stale before the caller
/// explicitly submits an assessment. Returning true does not authorize a label.
pub(crate) fn validate(request: &CaseRequest, wire: &[u8]) -> Result<bool, String> {
    let parsed = parse(wire, RESPONSE_BYTES)?;
    let root = fields(
        &parsed,
        &["schema_version", "request", "knowledge", "effect_authority"],
    )?;
    expected(root, "schema_version", "fa.live-evaluator-case/1")?;
    if root["request"] != request.parsed {
        return Err("case response does not bind the submitted evaluator request".into());
    }
    if root["effect_authority"].as_bool() != Some(false) {
        return Err("case observations cannot carry effect authority".into());
    }
    let knowledge = root["knowledge"]
        .as_object()
        .ok_or("case response requires explicit knowledge")?;
    match knowledge.get("state").and_then(Json::as_str) {
        Some("known") => {
            let known = fields(&root["knowledge"], &["state", "basis", "value"])?;
            let basis = Basis::decode(&known["basis"])?;
            if basis.journal == 0 || basis.evaluation == 0 {
                return Err("a recorded native case requires a nonzero acknowledged basis".into());
            }
            let value = fields(
                &known["value"],
                &[
                    "round",
                    "attempt",
                    "policy_generation",
                    "congress_generation",
                    "case_evidence_root_hex",
                ],
            )?;
            if positive(&value["round"])? != request.round {
                return Err("case response names a different original round".into());
            }
            for field in ["attempt", "policy_generation", "congress_generation"] {
                positive(&value[field])?;
            }
            digest(&value["case_evidence_root_hex"])?;
            Ok(true)
        }
        Some("unknown") => {
            let unknown = fields(&root["knowledge"], &["state", "reason"])?;
            expected(unknown, "reason", "case_not_recorded")?;
            Ok(false)
        }
        _ => Err("unsupported evaluator case knowledge state".into()),
    }
}

pub(super) fn positive(value: &Json) -> Result<u64, String> {
    let value = decimal(value)?;
    if value == 0 {
        Err("case identities and generations must be nonzero".into())
    } else {
        Ok(value)
    }
}

pub(super) fn digest(value: &Json) -> Result<[u8; 32], String> {
    let value = value
        .as_str()
        .ok_or("case evidence identity must be lowercase hex")?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("case evidence identity must be exactly 32 lowercase hex bytes".into());
    }
    let digest: [u8; 32] = hex(value)?.try_into().map_err(|_| "case evidence width")?;
    if digest == [0; 32] {
        return Err("case evidence identity must be nonzero".into());
    }
    Ok(digest)
}

#[cfg(test)]
#[path = "../tests/case_protocol.rs"]
mod tests;
