//! Read-only tracker readiness and stale in-progress diagnostic report (IW-15).
//!
//! Summarizes `.beads/issues.jsonl` against `registry/roadmap.json` so operators
//! and verifiers can inspect open, in-progress, ready, and packet-skewed work
//! without mutating tracker state or claiming execution verification.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::json::{Json, Limits, parse};

/// One causal refusal or skew finding in the tracker readiness report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub code: &'static str,
    pub subject: String,
    pub detail: String,
}

/// Summary of `.beads/issues.jsonl` cross-checked against `registry/roadmap.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessReport {
    pub open_count: usize,
    pub in_progress_count: usize,
    pub closed_count: usize,
    pub ready_ids: Vec<String>,
    pub in_progress_ids: Vec<String>,
    pub pending_packet_in_progress: Vec<(String, String)>,
    pub findings: Vec<Finding>,
}

impl ReadinessReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "TRACKER_READINESS open={} in_progress={} closed={} ready={} pending_packet_skew={}",
            self.open_count,
            self.in_progress_count,
            self.closed_count,
            self.ready_ids.len(),
            self.pending_packet_in_progress.len()
        );
        for id in &self.ready_ids {
            let _ = writeln!(out, "  READY {id}");
        }
        for (bead_id, packet_id) in &self.pending_packet_in_progress {
            let _ = writeln!(
                out,
                "  IN_PROGRESS_ON_PENDING_PACKET bead={bead_id} packet={packet_id}"
            );
        }
        for finding in &self.findings {
            let _ = writeln!(
                out,
                "  FINDING [{}] {}: {}",
                finding.code, finding.subject, finding.detail
            );
        }
        out.push_str(
            "CLAIM_BOUNDARY tracker_readiness: read-only tracker/roadmap diagnostic only. \
             Not a claim of compilation, test execution, or bead acceptance proof.\n",
        );
        out
    }
}

/// Evaluate `.beads/issues.jsonl` text and parsed `registry/roadmap.json`.
pub fn evaluate(issues_jsonl: &str, roadmap: &Json) -> ReadinessReport {
    let mut findings = Vec::new();
    let mut packet_statuses: BTreeMap<String, String> = BTreeMap::new();

    if let Some(packets) = roadmap.get("packets").and_then(Json::as_array) {
        for (index, item) in packets.iter().enumerate() {
            let Some(obj) = item.as_object() else {
                findings.push(Finding {
                    code: "malformed_roadmap_packet",
                    subject: format!("$.packets[{index}]"),
                    detail: "packet entry is not a JSON object".to_string(),
                });
                continue;
            };
            let id = obj.get("id").and_then(Json::as_str);
            let status = obj.get("status").and_then(Json::as_str);
            match (id, status) {
                (Some(id), Some(status)) => {
                    packet_statuses.insert(id.to_string(), status.to_string());
                }
                _ => findings.push(Finding {
                    code: "malformed_roadmap_packet",
                    subject: format!("$.packets[{index}]"),
                    detail: "packet entry requires string `id` and `status`".to_string(),
                }),
            }
        }
    } else {
        findings.push(Finding {
            code: "missing_roadmap_packets",
            subject: "$.packets".to_string(),
            detail: "roadmap.json must contain a `packets` array".to_string(),
        });
    }

    let mut statuses: BTreeMap<String, String> = BTreeMap::new();
    let mut blockers_by_issue: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut packet_refs: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (line_index, raw_line) in issues_jsonl.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let line_label = format!("line {}", line_index + 1);
        let parsed = match parse(line.as_bytes(), Limits::default()) {
            Ok(value) => value,
            Err(error) => {
                findings.push(Finding {
                    code: "invalid_issues_jsonl",
                    subject: line_label,
                    detail: format!("cannot parse JSONL record: {error}"),
                });
                continue;
            }
        };
        let Some(issue) = parsed.as_object() else {
            findings.push(Finding {
                code: "invalid_issues_jsonl",
                subject: line_label,
                detail: "JSONL record must be an object".to_string(),
            });
            continue;
        };
        if issue
            .get("tombstone")
            .and_then(Json::as_bool)
            .unwrap_or(false)
            || issue.get("status").and_then(Json::as_str) == Some("tombstone")
        {
            continue;
        }
        let Some(id) = issue.get("id").and_then(Json::as_str) else {
            findings.push(Finding {
                code: "missing_issue_id",
                subject: line_label,
                detail: "live issue record is missing string `id`".to_string(),
            });
            continue;
        };
        let Some(status) = issue.get("status").and_then(Json::as_str) else {
            findings.push(Finding {
                code: "missing_issue_status",
                subject: id.to_string(),
                detail: "live issue record is missing string `status`".to_string(),
            });
            continue;
        };
        statuses.insert(id.to_string(), status.to_string());

        let mut blockers = Vec::new();
        if let Some(deps) = issue.get("dependencies").and_then(Json::as_array) {
            for dep in deps {
                if let Some(dep_obj) = dep.as_object() {
                    let dep_type = dep_obj
                        .get("type")
                        .or_else(|| dep_obj.get("dependency_type"))
                        .and_then(Json::as_str)
                        .unwrap_or("blocks");
                    if dep_type == "blocks"
                        && let Some(depends_on) =
                            dep_obj.get("depends_on_id").and_then(Json::as_str)
                    {
                        blockers.push(depends_on.to_string());
                    }
                }
            }
        }
        blockers_by_issue.insert(id.to_string(), blockers);

        let mut refs = Vec::new();
        if let Some(description) = issue.get("description").and_then(Json::as_str) {
            for desc_line in description.lines() {
                if let Some(rest) = desc_line.trim().strip_prefix("- Roadmap packet:") {
                    let packet = rest.trim();
                    if !packet.is_empty() {
                        refs.push(packet.to_string());
                    }
                }
            }
        }
        packet_refs.insert(id.to_string(), refs);
    }

    let mut open_count = 0usize;
    let mut in_progress_count = 0usize;
    let mut closed_count = 0usize;
    let mut ready_ids = Vec::new();
    let mut in_progress_ids = Vec::new();
    let mut pending_packet_in_progress = Vec::new();
    let mut seen_skew = BTreeSet::new();

    for (id, status) in &statuses {
        match status.as_str() {
            "open" => {
                open_count += 1;
                let blocked = blockers_by_issue.get(id).is_some_and(|blockers| {
                    blockers.iter().any(|blocker| {
                        statuses
                            .get(blocker)
                            .is_none_or(|blocker_status| blocker_status != "closed")
                    })
                });
                if !blocked {
                    ready_ids.push(id.clone());
                }
            }
            "in_progress" => {
                in_progress_count += 1;
                in_progress_ids.push(id.clone());
            }
            "closed" => {
                closed_count += 1;
            }
            other => {
                findings.push(Finding {
                    code: "unknown_issue_status",
                    subject: id.clone(),
                    detail: format!("unrecognized issue status `{other}`"),
                });
            }
        }

        if let Some(refs) = packet_refs.get(id) {
            for packet_id in refs {
                match packet_statuses.get(packet_id) {
                    None => findings.push(Finding {
                        code: "unknown_roadmap_packet",
                        subject: id.clone(),
                        detail: format!(
                            "issue references roadmap packet `{packet_id}` absent from roadmap.json"
                        ),
                    }),
                    Some(packet_status) => {
                        if status == "in_progress"
                            && packet_status == "pending"
                            && seen_skew.insert((id.clone(), packet_id.clone()))
                        {
                            pending_packet_in_progress.push((id.clone(), packet_id.clone()));
                        }
                    }
                }
            }
        }
    }

    ReadinessReport {
        open_count,
        in_progress_count,
        closed_count,
        ready_ids,
        in_progress_ids,
        pending_packet_in_progress,
        findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_roadmap() -> Json {
        parse(
            br#"{"packets":[
                {"id":"FA-001","status":"complete"},
                {"id":"FA-002","status":"pending"}
            ]}"#,
            Limits::default(),
        )
        .expect("valid sample roadmap")
    }

    #[test]
    fn readiness_report_computes_ready_in_progress_and_pending_packet_skew() {
        let issues = concat!(
            r#"{"id":"bead-1","status":"closed","description":"- Roadmap packet: FA-001"}"#,
            "\n",
            r#"{"id":"bead-2","status":"open","dependencies":[{"depends_on_id":"bead-1","type":"blocks"}],"description":"- Roadmap packet: FA-002"}"#,
            "\n",
            r#"{"id":"bead-3","status":"in_progress","description":"- Roadmap packet: FA-002"}"#,
            "\n",
            r#"{"id":"bead-4","status":"open","dependencies":[{"depends_on_id":"bead-3","type":"blocks"}]}"#,
            "\n"
        );
        let report = evaluate(issues, &sample_roadmap());
        assert!(
            report.is_clean(),
            "unexpected findings: {:?}",
            report.findings
        );
        assert_eq!(report.open_count, 2);
        assert_eq!(report.in_progress_count, 1);
        assert_eq!(report.closed_count, 1);
        assert_eq!(report.ready_ids, vec!["bead-2".to_string()]);
        assert_eq!(
            report.pending_packet_in_progress,
            vec![("bead-3".to_string(), "FA-002".to_string())]
        );
        let rendered = report.render();
        assert!(rendered.contains("READY bead-2"));
        assert!(rendered.contains("IN_PROGRESS_ON_PENDING_PACKET bead=bead-3 packet=FA-002"));
        assert!(
            rendered
                .contains("Not a claim of compilation, test execution, or bead acceptance proof")
        );
    }

    #[test]
    fn malformed_jsonl_and_unknown_roadmap_packet_produce_causal_findings() {
        let issues = concat!(
            "not-json\n",
            r#"{"id":"bead-bad-packet","status":"open","description":"- Roadmap packet: FA-999"}"#,
            "\n"
        );
        let report = evaluate(issues, &sample_roadmap());
        assert!(!report.is_clean());
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "invalid_issues_jsonl" && f.subject == "line 1"),
            "missing invalid_issues_jsonl finding: {:?}",
            report.findings
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "unknown_roadmap_packet" && f.subject == "bead-bad-packet"),
            "missing unknown_roadmap_packet finding: {:?}",
            report.findings
        );
    }
}
