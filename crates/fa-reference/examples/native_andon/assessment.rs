//! One bounded assessment contract shared by offline and live evaluator intake.
//! The caller supplies a label, never a replacement case or a saved score. Only
//! the separately retained native evaluator role can attach it to the original
//! acknowledged review. This module owns no socket, recovery or effect path.

use crate::config::{debug, hex};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, credibility::FileIndependentEvaluator,
};
use fa_reference::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;

pub(crate) const MAX_ASSESSMENT_BYTES: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssessmentRequest {
    pub(crate) round: u64,
    pub(crate) attempt: u64,
    pub(crate) policy: u64,
    pub(crate) congress: u64,
    pub(crate) root: [u8; 32],
    pub(crate) expected_revision: u64,
    pub(crate) assessment: Assessment,
}

impl AssessmentRequest {
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let Json::Object(mut fields) = strict_json::parse(
            bytes,
            Limits {
                max_bytes: MAX_ASSESSMENT_BYTES,
                max_depth: 4,
                max_items: 64,
                max_string_bytes: 256,
            },
        )
        .map_err(debug)?
        else {
            return Err("expected native assessment object".into());
        };
        if take(&mut fields, "schema_version")?.as_str() != Some("fa.native-assessment/1") {
            return Err("wrong native assessment schema".into());
        }
        let round = number(&mut fields, "round")?;
        let attempt = number(&mut fields, "attempt")?;
        let policy = number(&mut fields, "policy_generation")?;
        let congress = number(&mut fields, "congress_generation")?;
        let root = digest(&mut fields, "case_evidence_root_hex")?;
        let expected_revision = number(&mut fields, "expected_evaluation_revision")?;
        let origin = number(&mut fields, "origin")?;
        let evidence_id = digest(&mut fields, "evidence_id_hex")?;
        let truth = match take(&mut fields, "truth")?.as_str() {
            Some("benign") => GroundTruth::Benign,
            Some("violation") => GroundTruth::Violation,
            Some("censored") => GroundTruth::Censored,
            _ => return Err("truth must be benign, violation or censored".into()),
        };
        if !fields.is_empty() {
            return Err("unknown native assessment fields".into());
        }
        if [round, attempt, policy, congress, origin].contains(&0) {
            return Err("assessment case and origin identities must be nonzero".into());
        }
        Ok(Self {
            round,
            attempt,
            policy,
            congress,
            root,
            expected_revision,
            assessment: Assessment {
                origin,
                evidence_id,
                truth,
            },
        })
    }

    /// Preserve the original ticket, label history and native accounting law.
    /// Exact retries do not read a new clock or incur another false-stop charge.
    /// An acknowledged Time before a failed label remains real journal history.
    pub(crate) fn apply(
        &self,
        host: &mut FileOversight,
        evaluator: &FileIndependentEvaluator,
        checkpoint: &mut impl FnMut() -> Result<(), String>,
        clock: &mut impl FnMut() -> Result<ElapsedTick, String>,
    ) -> Result<bool, String> {
        checkpoint()?;
        // A foreign role must refuse before even a benign label's Time event.
        // Numeric scope/case equality does not establish original owner custody.
        evaluator.check_owner(host).map_err(debug)?;
        let ticket = host.evaluation_ticket(self.round).map_err(debug)?;
        let case = ticket.case();
        if case.attempt != self.attempt
            || case.policy_generation != self.policy
            || case.congress_generation != self.congress
            || case.evidence_root != self.root
        {
            return Err("assessment does not bind the original applied case".into());
        }
        let repeated =
            host.evaluation_history(self.round).map_err(debug)?.last() == Some(&self.assessment);
        if !repeated {
            if host.credibility_report().map_err(debug)?.revision != self.expected_revision {
                return Err("stale expected evaluation revision".into());
            }
            if self.assessment.truth == GroundTruth::Benign {
                checkpoint()?;
                let tick = clock()?;
                checkpoint()?;
                host.observe_time(host.revision(), tick).map_err(debug)?;
                checkpoint()?;
            }
        }
        let revision = host.revision();
        evaluator
            .assess(host, revision, &ticket, self.assessment)
            .map_err(debug)
    }
}

fn take(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<Json, String> {
    fields
        .remove(key)
        .ok_or_else(|| format!("missing native assessment field {key}"))
}

fn number(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<u64, String> {
    take(fields, key)?
        .as_u64()
        .ok_or_else(|| format!("{key} must be an unsigned integer"))
}

fn digest(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<[u8; 32], String> {
    let value = take(fields, key)?;
    let bytes = hex(value
        .as_str()
        .ok_or("evidence identity must be lowercase hex")?)?;
    let digest: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "evidence identity must be 32 bytes")?;
    if digest == [0; 32] {
        return Err("evidence identity must be nonzero".into());
    }
    Ok(digest)
}
