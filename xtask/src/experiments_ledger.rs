//! Preregistration ledger validator for `registry/experiments.json` and `registry/claims.json` (FA-129).
//!
//! Enforces plan §§2.4, 20, and 25.7:
//! - Every hypothesis in `registry/claims.json` (`class == "hypothesis"`) has one row in
//!   `registry/experiments.json`.
//! - Preregistered and executed rows bind a protocol file `docs/experiments/H<n>.md` with
//!   all required sections, a matching SHA-256 `protocol_digest`, a valid `frozen_on` date,
//!   and non-empty `bound_identities`.
//! - Executed rows bind a `result_artifact`, a matching `result_digest`, and a
//!   `result_collected_on` date satisfying `frozen_on <= result_collected_on`.
//! - A result whose protocol freeze date postdates data collection (`frozen_on > result_collected_on`)
//!   is exploratory and cannot be marked `executed` or promoted in `registry/claims.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::json::Json;
use crate::source_snapshot;

pub const EXPERIMENTS_SCHEMA: &str = "fa.experiments/0.3";

pub const REQUIRED_PROTOCOL_HEADINGS: [&str; 8] = [
    "## Question",
    "## Treatments",
    "## Outcomes",
    "## Controls",
    "## Sample Size",
    "## Confidence Method",
    "## Kill Criterion",
    "## Bound Identities",
];

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub hypothesis: String,
    pub code: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerReport {
    pub total_hypotheses: usize,
    pub not_preregistered: usize,
    pub preregistered: usize,
    pub executed: usize,
    pub exploratory: usize,
    pub findings: Vec<Finding>,
}

impl LedgerReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    #[must_use]
    pub fn render_json(&self) -> String {
        let mut out = String::new();
        let _ = write!(
            out,
            "{{\"total_hypotheses\":{},\"not_preregistered\":{},\"preregistered\":{},\"executed\":{},\"exploratory\":{},\"findings\":[",
            self.total_hypotheses,
            self.not_preregistered,
            self.preregistered,
            self.executed,
            self.exploratory
        );
        for (idx, finding) in self.findings.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"hypothesis\":\"{}\",\"code\":\"{}\",\"detail\":\"{}\"}}",
                escape_json(&finding.hypothesis),
                finding.code,
                escape_json(&finding.detail)
            );
        }
        out.push_str("]}");
        out
    }
}

fn escape_json(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

#[derive(Clone, Debug)]
struct ClaimHypothesis {
    status: String,
    result_artifacts: Vec<String>,
}

pub fn check_repository(root: &Path, experiments: &Json, claims: &Json) -> LedgerReport {
    check_with_resolver(experiments, claims, |relative| {
        let digest = source_snapshot::operator_file_digest(root, relative)?;
        let text = fs::read_to_string(root.join(relative)).ok();
        Ok((digest, text))
    })
}

pub fn check_with_resolver<F>(experiments: &Json, claims: &Json, resolve_file: F) -> LedgerReport
where
    F: Fn(&str) -> Result<(String, Option<String>), String>,
{
    let mut report = LedgerReport::default();

    let Some(exp_obj) = experiments.as_object() else {
        report.findings.push(Finding {
            hypothesis: "<document>".to_string(),
            code: "invalid_schema",
            detail: "registry/experiments.json root must be a JSON object".to_string(),
        });
        return report;
    };

    if exp_obj.get("schema_version").and_then(Json::as_str) != Some(EXPERIMENTS_SCHEMA) {
        report.findings.push(Finding {
            hypothesis: "<document>".to_string(),
            code: "invalid_schema",
            detail: format!("expected schema_version `{EXPERIMENTS_SCHEMA}`"),
        });
    }

    if exp_obj
        .get("rule")
        .and_then(Json::as_str)
        .is_none_or(|rule| rule.trim().is_empty())
    {
        report.findings.push(Finding {
            hypothesis: "<document>".to_string(),
            code: "missing_rule",
            detail: "registry/experiments.json must declare a non-empty `rule` string".to_string(),
        });
    }

    let mut claim_hypotheses: BTreeMap<String, ClaimHypothesis> = BTreeMap::new();
    if let Some(claim_rows) = claims.get("claims").and_then(Json::as_array) {
        for row in claim_rows {
            let Some(obj) = row.as_object() else {
                continue;
            };
            if obj.get("class").and_then(Json::as_str) != Some("hypothesis") {
                continue;
            }
            let Some(id) = obj.get("id").and_then(Json::as_str) else {
                continue;
            };
            let status = obj
                .get("status")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            let mut result_artifacts = Vec::new();
            if let Some(arr) = obj.get("result_artifacts").and_then(Json::as_array) {
                for item in arr {
                    if let Some(path) = item.as_str() {
                        result_artifacts.push(path.to_string());
                    }
                }
            }
            claim_hypotheses.insert(
                id.to_string(),
                ClaimHypothesis {
                    status,
                    result_artifacts,
                },
            );
        }
    } else {
        report.findings.push(Finding {
            hypothesis: "<claims>".to_string(),
            code: "invalid_claims",
            detail: "registry/claims.json must contain a `claims` array".to_string(),
        });
    }

    let Some(rows) = exp_obj.get("experiments").and_then(Json::as_array) else {
        report.findings.push(Finding {
            hypothesis: "<document>".to_string(),
            code: "missing_experiments",
            detail: "registry/experiments.json must contain an `experiments` array".to_string(),
        });
        return report;
    };

    let mut seen_hypotheses = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let Some(obj) = row.as_object() else {
            report.findings.push(Finding {
                hypothesis: format!("$.experiments[{index}]"),
                code: "malformed_row",
                detail: "experiment row must be a JSON object".to_string(),
            });
            continue;
        };

        let Some(hypothesis) = obj.get("hypothesis").and_then(Json::as_str) else {
            report.findings.push(Finding {
                hypothesis: format!("$.experiments[{index}]"),
                code: "missing_hypothesis_id",
                detail: "experiment row requires string `hypothesis`".to_string(),
            });
            continue;
        };

        if !seen_hypotheses.insert(hypothesis.to_string()) {
            report.findings.push(Finding {
                hypothesis: hypothesis.to_string(),
                code: "duplicate_hypothesis",
                detail: "duplicate hypothesis row in registry/experiments.json".to_string(),
            });
            continue;
        }

        if !claim_hypotheses.contains_key(hypothesis) {
            report.findings.push(Finding {
                hypothesis: hypothesis.to_string(),
                code: "unknown_hypothesis",
                detail: "hypothesis is absent from registry/claims.json".to_string(),
            });
        }

        let status = obj
            .get("hypothesis")
            .and_then(|_| obj.get("status"))
            .and_then(Json::as_str);
        let protocol_path = obj.get("protocol_path");
        let protocol_digest = obj.get("protocol_digest");
        let frozen_on = obj.get("frozen_on");
        let bound_identities = obj.get("bound_identities").and_then(Json::as_array);
        let result_artifact = obj.get("result_artifact");
        let result_digest = obj.get("result_digest");
        let result_collected_on = obj.get("result_collected_on");

        let Some(bound_identities) = bound_identities else {
            report.findings.push(Finding {
                hypothesis: hypothesis.to_string(),
                code: "invalid_bound_identities",
                detail: "`bound_identities` must be an array".to_string(),
            });
            continue;
        };

        match status {
            Some("not_preregistered") => {
                report.not_preregistered += 1;
                if !is_json_null(protocol_path)
                    || !is_json_null(protocol_digest)
                    || !is_json_null(frozen_on)
                    || !is_json_null(result_artifact)
                    || !is_json_null(result_digest)
                    || !bound_identities.is_empty()
                {
                    report.findings.push(Finding {
                        hypothesis: hypothesis.to_string(),
                        code: "inconsistent_not_preregistered",
                        detail: "not_preregistered row must keep protocol/result fields null and bound_identities empty".to_string(),
                    });
                }
            }
            Some("preregistered") | Some("executed") | Some("exploratory") => {
                let status_str = status.unwrap_or_default();
                match status_str {
                    "preregistered" => report.preregistered += 1,
                    "executed" => report.executed += 1,
                    "exploratory" => report.exploratory += 1,
                    _ => {}
                }

                let expected_protocol_path = format!("docs/experiments/{hypothesis}.md");
                let actual_protocol_path = protocol_path.and_then(Json::as_str);
                if actual_protocol_path != Some(expected_protocol_path.as_str()) {
                    report.findings.push(Finding {
                        hypothesis: hypothesis.to_string(),
                        code: "invalid_protocol_path",
                        detail: format!(
                            "expected protocol_path `{expected_protocol_path}`, found {:?}",
                            actual_protocol_path
                        ),
                    });
                }

                let digest_str = protocol_digest.and_then(Json::as_str);
                if !digest_str.is_some_and(is_sha256_hex) {
                    report.findings.push(Finding {
                        hypothesis: hypothesis.to_string(),
                        code: "invalid_protocol_digest",
                        detail:
                            "protocol_digest must be a 64-character lowercase hex SHA-256 string"
                                .to_string(),
                    });
                }

                let frozen_str = frozen_on.and_then(Json::as_str);
                if !frozen_str.is_some_and(is_iso_date) {
                    report.findings.push(Finding {
                        hypothesis: hypothesis.to_string(),
                        code: "invalid_frozen_date",
                        detail: "frozen_on must be a valid YYYY-MM-DD date".to_string(),
                    });
                }

                if bound_identities.is_empty()
                    || !bound_identities.iter().all(|item| {
                        item.as_str()
                            .is_some_and(|s| !s.trim().is_empty() && s.contains(':'))
                    })
                {
                    report.findings.push(Finding {
                        hypothesis: hypothesis.to_string(),
                        code: "invalid_bound_identities",
                        detail: "preregistered/executed row requires non-empty `key:value` strings in bound_identities".to_string(),
                    });
                }

                if let (Some(path), Some(expected_digest)) = (actual_protocol_path, digest_str)
                    && is_sha256_hex(expected_digest)
                {
                    match resolve_file(path) {
                        Ok((actual_digest, maybe_text)) => {
                            if actual_digest != expected_digest {
                                report.findings.push(Finding {
                                    hypothesis: hypothesis.to_string(),
                                    code: "protocol_digest_mismatch",
                                    detail: format!(
                                        "protocol file `{path}` hashed to `{actual_digest}`, ledger records `{expected_digest}`"
                                    ),
                                });
                            }
                            if let Some(text) = maybe_text {
                                check_protocol_sections(hypothesis, &text, &mut report.findings);
                            }
                        }
                        Err(err) => {
                            report.findings.push(Finding {
                                hypothesis: hypothesis.to_string(),
                                code: "missing_protocol_file",
                                detail: format!("cannot verify protocol file `{path}`: {err}"),
                            });
                        }
                    }
                }

                if status_str == "preregistered"
                    && (!is_json_null(result_artifact) || !is_json_null(result_digest))
                {
                    report.findings.push(Finding {
                        hypothesis: hypothesis.to_string(),
                        code: "inconsistent_preregistered_result",
                        detail: "preregistered row without completed execution must keep result_artifact and result_digest null".to_string(),
                    });
                }

                if status_str == "executed" {
                    let artifact_str = result_artifact.and_then(Json::as_str);
                    let res_digest_str = result_digest.and_then(Json::as_str);
                    let collected_str = result_collected_on.and_then(Json::as_str);

                    if artifact_str.is_none_or(|s| s.trim().is_empty()) {
                        report.findings.push(Finding {
                            hypothesis: hypothesis.to_string(),
                            code: "missing_result_artifact",
                            detail: "executed row requires non-empty string `result_artifact`"
                                .to_string(),
                        });
                    }
                    if !res_digest_str.is_some_and(is_sha256_hex) {
                        report.findings.push(Finding {
                            hypothesis: hypothesis.to_string(),
                            code: "invalid_result_digest",
                            detail:
                                "executed row requires 64-character lowercase hex `result_digest`"
                                    .to_string(),
                        });
                    }
                    if !collected_str.is_some_and(is_iso_date) {
                        report.findings.push(Finding {
                            hypothesis: hypothesis.to_string(),
                            code: "missing_result_collected_date",
                            detail:
                                "executed row requires valid YYYY-MM-DD `result_collected_on` date"
                                    .to_string(),
                        });
                    }
                    if let (Some(frozen), Some(collected)) = (frozen_str, collected_str)
                        && is_iso_date(frozen)
                        && is_iso_date(collected)
                        && frozen > collected
                    {
                        report.findings.push(Finding {
                            hypothesis: hypothesis.to_string(),
                            code: "late_protocol_freeze",
                            detail: format!(
                                "protocol frozen_on ({frozen}) postdates result_collected_on ({collected}); result is exploratory and cannot be marked executed"
                            ),
                        });
                    }
                    if let (Some(art_path), Some(expected_res_digest)) =
                        (artifact_str, res_digest_str)
                        && is_sha256_hex(expected_res_digest)
                    {
                        match resolve_file(art_path) {
                            Ok((actual_res_digest, _)) => {
                                if actual_res_digest != expected_res_digest {
                                    report.findings.push(Finding {
                                        hypothesis: hypothesis.to_string(),
                                        code: "result_digest_mismatch",
                                        detail: format!(
                                            "result artifact `{art_path}` hashed to `{actual_res_digest}`, ledger records `{expected_res_digest}`"
                                        ),
                                    });
                                }
                            }
                            Err(err) => {
                                report.findings.push(Finding {
                                    hypothesis: hypothesis.to_string(),
                                    code: "missing_result_artifact",
                                    detail: format!(
                                        "cannot verify result artifact `{art_path}`: {err}"
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            other => {
                report.findings.push(Finding {
                    hypothesis: hypothesis.to_string(),
                    code: "invalid_status",
                    detail: format!(
                        "unsupported experiment status {:?}; expected not_preregistered, preregistered, executed, or exploratory",
                        other
                    ),
                });
            }
        }

        if let Some(claim) = claim_hypotheses.get(hypothesis) {
            let claim_promoted = claim.status != "unmeasured" || !claim.result_artifacts.is_empty();
            if claim_promoted {
                if status != Some("executed") {
                    report.findings.push(Finding {
                        hypothesis: hypothesis.to_string(),
                        code: "unqualified_claim_promotion",
                        detail: format!(
                            "claims.json promotes `{hypothesis}` (status `{}`, artifacts {:?}) while experiments.json status is `{}`",
                            claim.status,
                            claim.result_artifacts,
                            status.unwrap_or("<missing>")
                        ),
                    });
                } else if let Some(ledger_art) = result_artifact.and_then(Json::as_str)
                    && (claim.result_artifacts.is_empty()
                        || claim.result_artifacts.iter().any(|a| a != ledger_art))
                {
                    report.findings.push(Finding {
                        hypothesis: hypothesis.to_string(),
                        code: "claim_result_artifact_mismatch",
                        detail: format!(
                            "claims.json result_artifacts {:?} do not match executed ledger result_artifact `{ledger_art}`",
                            claim.result_artifacts
                        ),
                    });
                }
            }
        }
    }

    for claim_id in claim_hypotheses.keys() {
        if !seen_hypotheses.contains(claim_id) {
            report.findings.push(Finding {
                hypothesis: claim_id.clone(),
                code: "missing_ledger_row",
                detail:
                    "hypothesis in registry/claims.json has no row in registry/experiments.json"
                        .to_string(),
            });
        }
    }

    report.total_hypotheses = seen_hypotheses.len();
    report.findings.sort();
    report
}

fn is_json_null(value: Option<&Json>) -> bool {
    matches!(value, Some(Json::Null))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes[0..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let month: u32 = value[5..7].parse().unwrap_or(0);
    let day: u32 = value[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn check_protocol_sections(hypothesis: &str, text: &str, findings: &mut Vec<Finding>) {
    let mut last_pos = 0usize;
    for heading in REQUIRED_PROTOCOL_HEADINGS {
        let mut found_offset = None;
        for (line_start, line) in line_offsets(&text[last_pos..]) {
            if line.trim_end() == heading {
                found_offset = Some(last_pos + line_start + line.len());
                break;
            }
        }
        if let Some(next_pos) = found_offset {
            last_pos = next_pos;
        } else {
            findings.push(Finding {
                hypothesis: hypothesis.to_string(),
                code: "missing_protocol_section",
                detail: format!(
                    "protocol file `docs/experiments/{hypothesis}.md` is missing required section `{heading}` in canonical order"
                ),
            });
            break;
        }
    }
}

fn line_offsets(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for line in text.split('\n') {
        out.push((start, line));
        start += line.len() + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{Limits, parse};

    fn valid_protocol_markdown(id: &str) -> String {
        format!(
            "# {id} — Test Protocol\n\n\
             ## Question\nQuestion text.\n\n\
             ## Treatments\nTreatments text.\n\n\
             ## Outcomes\nOutcomes text.\n\n\
             ## Controls\nControls text.\n\n\
             ## Sample Size\nSample size text.\n\n\
             ## Confidence Method\nConfidence method text.\n\n\
             ## Kill Criterion\nKill criterion text.\n\n\
             ## Bound Identities\n- `protocol_schema`: `fa.experiment_protocol/1`\n"
        )
    }

    #[test]
    fn repository_experiments_ledger_passes_with_four_preregistered_protocols() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let exp_bytes = fs::read(root.join("registry/experiments.json")).expect("experiments.json");
        let claims_bytes = fs::read(root.join("registry/claims.json")).expect("claims.json");
        let experiments = parse(&exp_bytes, Limits::default()).expect("valid experiments json");
        let claims = parse(&claims_bytes, Limits::default()).expect("valid claims json");

        let report = check_repository(root, &experiments, &claims);
        assert!(
            report.is_clean(),
            "expected clean repository experiments ledger, got findings: {:?}",
            report.findings
        );
        assert_eq!(report.total_hypotheses, 21);
        assert_eq!(report.preregistered, 4);
        assert_eq!(report.not_preregistered, 17);
        assert_eq!(report.executed, 0);
    }

    #[test]
    fn late_protocol_freeze_after_result_collection_is_flagged_and_refused() {
        let claims = parse(
            br#"{
                "claims": [
                    {"id": "H1", "class": "hypothesis", "status": "supported", "result_artifacts": ["artifacts/experiments/h1-result.json"]}
                ]
            }"#,
            Limits::default(),
        )
        .expect("claims");
        let experiments = parse(
            br#"{
                "schema_version": "fa.experiments/0.3",
                "rule": "A result whose protocol digest was recorded after data collection began is exploratory and cannot be promoted.",
                "experiments": [
                    {
                        "hypothesis": "H1",
                        "protocol_path": "docs/experiments/H1.md",
                        "protocol_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "frozen_on": "2026-10-26",
                        "bound_identities": ["model:v1"],
                        "result_artifact": "artifacts/experiments/h1-result.json",
                        "result_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                        "result_collected_on": "2026-10-25",
                        "status": "executed"
                    }
                ]
            }"#,
            Limits::default(),
        )
        .expect("experiments");

        let report = check_with_resolver(&experiments, &claims, |path| match path {
            "docs/experiments/H1.md" => Ok((
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                Some(valid_protocol_markdown("H1")),
            )),
            "artifacts/experiments/h1-result.json" => Ok((
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                None,
            )),
            other => Err(format!("unexpected path {other}")),
        });

        assert!(
            report
                .findings
                .iter()
                .any(|f| f.hypothesis == "H1" && f.code == "late_protocol_freeze"),
            "expected late_protocol_freeze finding, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn exploratory_or_unregistered_hypothesis_promotion_in_claims_registry_is_refused() {
        let claims = parse(
            br#"{
                "claims": [
                    {"id": "H1", "class": "hypothesis", "status": "supported", "result_artifacts": []},
                    {"id": "H2", "class": "hypothesis", "status": "unmeasured", "result_artifacts": ["artifacts/experiments/h2.json"]}
                ]
            }"#,
            Limits::default(),
        )
        .expect("claims");
        let experiments = parse(
            br#"{
                "schema_version": "fa.experiments/0.3",
                "rule": "No exploratory promotion.",
                "experiments": [
                    {
                        "hypothesis": "H1",
                        "protocol_path": null,
                        "protocol_digest": null,
                        "frozen_on": null,
                        "bound_identities": [],
                        "result_artifact": null,
                        "result_digest": null,
                        "status": "not_preregistered"
                    },
                    {
                        "hypothesis": "H2",
                        "protocol_path": "docs/experiments/H2.md",
                        "protocol_digest": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                        "frozen_on": "2026-10-25",
                        "bound_identities": ["model:v1"],
                        "result_artifact": null,
                        "result_digest": null,
                        "status": "preregistered"
                    }
                ]
            }"#,
            Limits::default(),
        )
        .expect("experiments");

        let report = check_with_resolver(&experiments, &claims, |_path| {
            Ok((
                "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string(),
                Some(valid_protocol_markdown("H2")),
            ))
        });

        assert!(
            report
                .findings
                .iter()
                .any(|f| f.hypothesis == "H1" && f.code == "unqualified_claim_promotion"),
            "expected H1 unqualified_claim_promotion, got: {:?}",
            report.findings
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.hypothesis == "H2" && f.code == "unqualified_claim_promotion"),
            "expected H2 unqualified_claim_promotion, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn protocol_digest_mismatch_and_missing_required_section_are_refused() {
        let claims = parse(
            br#"{
                "claims": [
                    {"id": "H15", "class": "hypothesis", "status": "unmeasured", "result_artifacts": []}
                ]
            }"#,
            Limits::default(),
        )
        .expect("claims");
        let experiments = parse(
            br#"{
                "schema_version": "fa.experiments/0.3",
                "rule": "No exploratory promotion.",
                "experiments": [
                    {
                        "hypothesis": "H15",
                        "protocol_path": "docs/experiments/H15.md",
                        "protocol_digest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                        "frozen_on": "2026-10-25",
                        "bound_identities": ["model:v1"],
                        "result_artifact": null,
                        "result_digest": null,
                        "status": "preregistered"
                    }
                ]
            }"#,
            Limits::default(),
        )
        .expect("experiments");

        let report = check_with_resolver(&experiments, &claims, |_path| {
            Ok((
                "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string(),
                Some("# H15\n\n## Question\nOnly question present.\n".to_string()),
            ))
        });

        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "protocol_digest_mismatch"),
            "expected protocol_digest_mismatch, got: {:?}",
            report.findings
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "missing_protocol_section"),
            "expected missing_protocol_section, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn valid_executed_experiment_with_prior_freeze_and_matching_digests_permits_promotion() {
        let claims = parse(
            br#"{
                "claims": [
                    {"id": "H15", "class": "hypothesis", "status": "supported", "result_artifacts": ["artifacts/experiments/h15-result.json"]}
                ]
            }"#,
            Limits::default(),
        )
        .expect("claims");
        let experiments = parse(
            br#"{
                "schema_version": "fa.experiments/0.3",
                "rule": "No exploratory promotion.",
                "experiments": [
                    {
                        "hypothesis": "H15",
                        "protocol_path": "docs/experiments/H15.md",
                        "protocol_digest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "frozen_on": "2026-10-25",
                        "bound_identities": ["model:v1"],
                        "result_artifact": "artifacts/experiments/h15-result.json",
                        "result_digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                        "result_collected_on": "2026-10-26",
                        "status": "executed"
                    }
                ]
            }"#,
            Limits::default(),
        )
        .expect("experiments");

        let report = check_with_resolver(&experiments, &claims, |path| match path {
            "docs/experiments/H15.md" => Ok((
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                Some(valid_protocol_markdown("H15")),
            )),
            "artifacts/experiments/h15-result.json" => Ok((
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                None,
            )),
            other => Err(format!("unexpected path {other}")),
        });

        assert!(
            report.is_clean(),
            "expected clean report for valid executed experiment, got: {:?}",
            report.findings
        );
        assert_eq!(report.executed, 1);
    }
}
