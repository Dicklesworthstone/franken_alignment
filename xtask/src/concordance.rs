//! Fail-closed validation of the founding-ideas concordance.
//!
//! This module deliberately validates only direct, typed concordance edges.
//! It does not infer coverage through invariant roots, roadmap dependencies,
//! parent headings, or prose.

use std::collections::BTreeSet;

use crate::json::Json;

const CONCORDANCE_FILE: &str = "registry/founding_concordance.json";
const INVARIANTS_FILE: &str = "registry/invariants.json";
const CLAIMS_FILE: &str = "registry/claims.json";
const ROADMAP_FILE: &str = "registry/roadmap.json";
const PLAN_FILE: &str = "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md";

const MAX_ROWS: usize = 4_096;
const MAX_REFS_PER_ROW: usize = 256;
const MAX_TOTAL_REFS: usize = 32_768;
const MAX_PLAN_BYTES: usize = 8 * 1024 * 1024;
const MAX_PLAN_LINES: usize = 32_768;
const MAX_FINDINGS: usize = 8_192;

/// An exact, deterministic checker finding.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Finding {
    pub file: String,
    pub id: String,
    pub code: String,
    pub detail: String,
}

/// Counts of the checked, typed target universes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    pub headings: usize,
    pub invariants: usize,
    pub hypotheses: usize,
    pub packets: usize,
}

/// Deterministic concordance-check result.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    pub missing: Vec<Finding>,
    pub dangling: Vec<Finding>,
    pub counts: Counts,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.dangling.is_empty()
    }

    /// Render the required report with a deliberately small, shared JSON serializer.
    pub fn render_json(&self) -> String {
        let mut rendered = String::new();
        rendered.push_str("{\"missing\":[");
        render_findings(&mut rendered, &self.missing);
        rendered.push_str("],\"dangling\":[");
        render_findings(&mut rendered, &self.dangling);
        rendered.push_str("],\"counts\":{\"headings\":");
        rendered.push_str(&self.counts.headings.to_string());
        rendered.push_str(",\"invariants\":");
        rendered.push_str(&self.counts.invariants.to_string());
        rendered.push_str(",\"hypotheses\":");
        rendered.push_str(&self.counts.hypotheses.to_string());
        rendered.push_str(",\"packets\":");
        rendered.push_str(&self.counts.packets.to_string());
        rendered.push_str("}}");
        rendered
    }
}

/// Check direct, typed coverage from the concordance against the four registries.
pub fn check(
    concordance: &Json,
    invariants: &Json,
    claims: &Json,
    roadmap: &Json,
    plan: &str,
) -> Report {
    let mut checker = Checker::default();
    let plan_targets = checker.plan_targets(plan);
    let invariant_targets = checker.registry_ids(
        invariants,
        INVARIANTS_FILE,
        "invariants",
        is_invariant_id,
        "invariant",
    );
    let hypothesis_targets = checker.hypothesis_ids(claims);
    let packet_targets =
        checker.registry_ids(roadmap, ROADMAP_FILE, "packets", is_packet_id, "packet");

    checker.report.counts = Counts {
        headings: plan_targets.required.len(),
        invariants: invariant_targets.len(),
        hypotheses: hypothesis_targets.len(),
        packets: packet_targets.len(),
    };
    checker.concordance(
        concordance,
        TargetUniverses {
            headings: &plan_targets.known,
            required_headings: &plan_targets.required,
            invariants: &invariant_targets,
            hypotheses: &hypothesis_targets,
            packets: &packet_targets,
        },
    );
    checker.finish();
    checker.report
}

#[derive(Default)]
struct Checker {
    report: Report,
    covered_headings: BTreeSet<String>,
    covered_invariants: BTreeSet<String>,
    covered_hypotheses: BTreeSet<String>,
    covered_packets: BTreeSet<String>,
    references_seen: usize,
    reference_limit_reported: bool,
    finding_limit_reported: bool,
}

struct PlanTargets {
    known: BTreeSet<String>,
    required: BTreeSet<String>,
}

#[derive(Clone, Copy)]
struct TargetUniverses<'a> {
    headings: &'a BTreeSet<String>,
    required_headings: &'a BTreeSet<String>,
    invariants: &'a BTreeSet<String>,
    hypotheses: &'a BTreeSet<String>,
    packets: &'a BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum CoverageKind {
    Heading,
    Invariant,
    Hypothesis,
    Packet,
}

impl Checker {
    fn dangling(
        &mut self,
        file: &str,
        id: impl Into<String>,
        code: &str,
        detail: impl Into<String>,
    ) {
        self.finding(false, file, id.into(), code, detail.into());
    }

    fn missing(&mut self, file: &str, id: impl Into<String>, detail: impl Into<String>) {
        self.finding(true, file, id.into(), "uncovered", detail.into());
    }

    fn finding(&mut self, missing: bool, file: &str, id: String, code: &str, detail: String) {
        let findings = if missing {
            &mut self.report.missing
        } else {
            &mut self.report.dangling
        };
        if findings.len() < MAX_FINDINGS {
            findings.push(Finding {
                file: file.to_owned(),
                id,
                code: code.to_owned(),
                detail,
            });
        } else if !self.finding_limit_reported {
            self.finding_limit_reported = true;
            findings.push(Finding {
                file: file.to_owned(),
                id: "findings".to_owned(),
                code: "input_limit".to_owned(),
                detail: format!("more than {MAX_FINDINGS} findings"),
            });
        }
    }

    fn reference_permitted(&mut self) -> bool {
        self.references_seen = self.references_seen.saturating_add(1);
        if self.references_seen <= MAX_TOTAL_REFS {
            return true;
        }
        if !self.reference_limit_reported {
            self.reference_limit_reported = true;
            self.dangling(
                CONCORDANCE_FILE,
                "references",
                "input_limit",
                format!("more than {MAX_TOTAL_REFS} direct references"),
            );
        }
        false
    }

    fn plan_targets(&mut self, plan: &str) -> PlanTargets {
        let mut known = BTreeSet::new();
        let mut required = BTreeSet::new();
        let mut section: Option<(String, u32)> = None;
        let mut fence = None;
        let bounded = bounded_prefix(plan, MAX_PLAN_BYTES);
        if bounded.len() != plan.len() {
            self.dangling(
                PLAN_FILE,
                "plan",
                "input_limit",
                format!("plan exceeds {MAX_PLAN_BYTES} bytes"),
            );
        }

        for (index, line) in bounded.lines().enumerate() {
            if index >= MAX_PLAN_LINES {
                self.dangling(
                    PLAN_FILE,
                    "plan",
                    "input_limit",
                    format!("plan exceeds {MAX_PLAN_LINES} lines"),
                );
                break;
            }
            let trimmed = line.trim_start();
            if let Some(open) = fence {
                if closes_fence(trimmed, open) {
                    fence = None;
                }
                continue;
            }
            if let Some(open) = opening_fence(trimmed) {
                fence = Some(open);
                continue;
            }
            let (level, title) = if let Some(title) = trimmed.strip_prefix("### ") {
                (3_u8, title)
            } else if let Some(title) = trimmed.strip_prefix("## ") {
                (2_u8, title)
            } else {
                continue;
            };
            let title = collapse_whitespace(title);
            if title.is_empty() {
                self.dangling(
                    PLAN_FILE,
                    format!("line {}", index + 1),
                    "malformed_heading",
                    "heading text is empty",
                );
                continue;
            }
            let canonical = match numbered_heading(&title) {
                NumberedHeading::Numbered { key, major } => {
                    section = Some((key.clone(), major));
                    key
                }
                NumberedHeading::Malformed(candidate) => {
                    self.dangling(
                        PLAN_FILE,
                        candidate,
                        "malformed_heading",
                        "numbered headings require decimal components only",
                    );
                    continue;
                }
                NumberedHeading::Unnumbered => {
                    let Some((nearest, _major)) = &section else {
                        if level == 2 {
                            // The plan's table of contents precedes numbered
                            // sections and is outside the covered universe.
                            continue;
                        }
                        self.dangling(
                            PLAN_FILE,
                            format!("line {}", index + 1),
                            "malformed_heading",
                            "unnumbered heading has no preceding numbered section",
                        );
                        continue;
                    };
                    if level != 3 {
                        self.dangling(
                            PLAN_FILE,
                            format!("line {}", index + 1),
                            "malformed_heading",
                            "unnumbered level-two headings are not canonical plan sections",
                        );
                        continue;
                    }
                    let key = format!("{nearest}#{title}");
                    key
                }
            };
            let major = section.as_ref().map_or(0, |(_, major)| *major);
            if !known.insert(canonical.clone()) {
                self.dangling(
                    PLAN_FILE,
                    canonical,
                    "duplicate_key",
                    "duplicate canonical heading key",
                );
                continue;
            }
            if major >= 2 {
                required.insert(canonical);
            }
        }
        PlanTargets { known, required }
    }

    fn registry_ids(
        &mut self,
        document: &Json,
        file: &str,
        property: &str,
        valid: fn(&str) -> bool,
        kind: &str,
    ) -> BTreeSet<String> {
        let Some(rows) = array_property(document, property) else {
            self.dangling(
                file,
                property,
                "malformed_reference",
                "missing array property",
            );
            return BTreeSet::new();
        };
        self.ids_from_rows(rows, file, valid, kind)
    }

    fn hypothesis_ids(&mut self, document: &Json) -> BTreeSet<String> {
        let Some(rows) = array_property(document, "claims") else {
            self.dangling(
                CLAIMS_FILE,
                "claims",
                "malformed_reference",
                "missing array property",
            );
            return BTreeSet::new();
        };
        let mut ids = BTreeSet::new();
        for (index, row) in rows.iter().take(MAX_ROWS).enumerate() {
            let Some(object) = row.as_object() else {
                self.dangling(
                    CLAIMS_FILE,
                    format!("claims[{index}]"),
                    "malformed_reference",
                    "row is not an object",
                );
                continue;
            };
            let Some(id) = object.get("id").and_then(Json::as_str) else {
                self.dangling(
                    CLAIMS_FILE,
                    format!("claims[{index}]"),
                    "malformed_reference",
                    "missing string id",
                );
                continue;
            };
            if !is_hypothesis_id(id) {
                self.dangling(
                    CLAIMS_FILE,
                    id,
                    "malformed_reference",
                    "expected H followed by decimal digits",
                );
                continue;
            }
            if object.get("class").and_then(Json::as_str) != Some("hypothesis") {
                self.dangling(
                    CLAIMS_FILE,
                    id,
                    "malformed_reference",
                    "H identifiers must have class hypothesis",
                );
                continue;
            }
            if !ids.insert(id.to_owned()) {
                self.dangling(CLAIMS_FILE, id, "duplicate_key", "duplicate hypothesis id");
            }
        }
        if rows.len() > MAX_ROWS {
            self.dangling(
                CLAIMS_FILE,
                "claims",
                "input_limit",
                format!("more than {MAX_ROWS} rows"),
            );
        }
        ids
    }

    fn ids_from_rows(
        &mut self,
        rows: &[Json],
        file: &str,
        valid: fn(&str) -> bool,
        kind: &str,
    ) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for (index, row) in rows.iter().take(MAX_ROWS).enumerate() {
            let Some(id) = row.get("id").and_then(Json::as_str) else {
                self.dangling(
                    file,
                    format!("{kind}[{index}]"),
                    "malformed_reference",
                    "missing string id",
                );
                continue;
            };
            if !valid(id) {
                self.dangling(
                    file,
                    id,
                    "malformed_reference",
                    format!("invalid {kind} identifier"),
                );
                continue;
            }
            if !ids.insert(id.to_owned()) {
                self.dangling(file, id, "duplicate_key", format!("duplicate {kind} id"));
            }
        }
        if rows.len() > MAX_ROWS {
            self.dangling(
                file,
                kind,
                "input_limit",
                format!("more than {MAX_ROWS} rows"),
            );
        }
        ids
    }

    fn concordance(&mut self, document: &Json, targets: TargetUniverses<'_>) {
        let Some(ideas) = array_property(document, "founding_ideas") else {
            self.dangling(
                CONCORDANCE_FILE,
                "founding_ideas",
                "malformed_reference",
                "missing array property",
            );
            return;
        };
        let Some(syntheses) = array_property(document, "syntheses") else {
            self.dangling(
                CONCORDANCE_FILE,
                "syntheses",
                "malformed_reference",
                "missing array property",
            );
            return;
        };
        let Some(engineering) = array_property(document, "engineering_additions") else {
            self.dangling(
                CONCORDANCE_FILE,
                "engineering_additions",
                "malformed_reference",
                "missing array property",
            );
            return;
        };

        let mut idea_roots = BTreeSet::new();
        for (index, row) in ideas.iter().take(MAX_ROWS).enumerate() {
            if let Some(id) = row.get("id").and_then(Json::as_str) {
                if is_founding_id(id) {
                    if !idea_roots.insert(id.to_owned()) {
                        self.dangling(
                            CONCORDANCE_FILE,
                            id,
                            "duplicate_key",
                            "duplicate founding root",
                        );
                    }
                } else {
                    self.dangling(
                        CONCORDANCE_FILE,
                        id,
                        "invalid_root",
                        "expected FI-A## or FI-I##",
                    );
                }
            } else {
                self.dangling(
                    CONCORDANCE_FILE,
                    format!("founding_ideas[{index}]"),
                    "invalid_root",
                    "missing string root id",
                );
            }
        }
        let mut synthesis_roots = BTreeSet::new();
        for (index, row) in syntheses.iter().take(MAX_ROWS).enumerate() {
            if let Some(id) = row.get("id").and_then(Json::as_str) {
                if is_synthesis_id(id) {
                    if !synthesis_roots.insert(id.to_owned()) {
                        self.dangling(
                            CONCORDANCE_FILE,
                            id,
                            "duplicate_key",
                            "duplicate synthesis root",
                        );
                    }
                } else {
                    self.dangling(CONCORDANCE_FILE, id, "invalid_root", "expected FS-##");
                }
            } else {
                self.dangling(
                    CONCORDANCE_FILE,
                    format!("syntheses[{index}]"),
                    "invalid_root",
                    "missing string root id",
                );
            }
        }
        if ideas.len() > MAX_ROWS || syntheses.len() > MAX_ROWS || engineering.len() > MAX_ROWS {
            self.dangling(
                CONCORDANCE_FILE,
                "rows",
                "input_limit",
                format!("more than {MAX_ROWS} rows in a concordance array"),
            );
        }

        for row in ideas.iter().take(MAX_ROWS) {
            let Some(id) = row.get("id").and_then(Json::as_str) else {
                continue;
            };
            let valid = is_founding_id(id) && idea_roots.contains(id);
            let sections = self.references(row, id, "plan_sections");
            let owner_valid = valid && !sections.is_empty();
            if valid && sections.is_empty() {
                self.dangling(
                    CONCORDANCE_FILE,
                    id,
                    "malformed_reference",
                    "founding roots require at least one plan section",
                );
            }
            self.cover_refs(
                owner_valid,
                &sections,
                targets.headings,
                CoverageKind::Heading,
                "plan section",
            );
            let invariant_refs = self.references(row, id, "invariants");
            self.cover_refs(
                owner_valid,
                &invariant_refs,
                targets.invariants,
                CoverageKind::Invariant,
                "invariant",
            );
            let hypothesis_refs = self.references(row, id, "hypotheses");
            self.cover_refs(
                owner_valid,
                &hypothesis_refs,
                targets.hypotheses,
                CoverageKind::Hypothesis,
                "hypothesis",
            );
            let packet_refs = self.references(row, id, "packets");
            self.cover_refs(
                owner_valid,
                &packet_refs,
                targets.packets,
                CoverageKind::Packet,
                "packet",
            );
        }

        for row in syntheses.iter().take(MAX_ROWS) {
            let Some(id) = row.get("id").and_then(Json::as_str) else {
                continue;
            };
            let combines = self.references(row, id, "combines");
            let combines_valid = !combines.is_empty()
                && combines
                    .iter()
                    .all(|root| is_founding_id(root) && idea_roots.contains(root));
            for root in &combines {
                if !is_founding_id(root) {
                    self.dangling(
                        CONCORDANCE_FILE,
                        root,
                        "invalid_root",
                        format!("{id} combines a malformed founding root"),
                    );
                } else if !idea_roots.contains(root) {
                    self.dangling(
                        CONCORDANCE_FILE,
                        root,
                        "missing_root",
                        format!("{id} combines an unknown founding root"),
                    );
                }
            }
            let owner_valid = is_synthesis_id(id) && synthesis_roots.contains(id) && combines_valid;
            if is_synthesis_id(id) && synthesis_roots.contains(id) && combines.is_empty() {
                self.dangling(
                    CONCORDANCE_FILE,
                    id,
                    "malformed_reference",
                    "synthesis requires a nonempty combines array",
                );
            }
            self.cover_row(owner_valid, row, id, targets);
        }

        let roots: BTreeSet<String> = idea_roots.union(&synthesis_roots).cloned().collect();
        let mut mechanisms = BTreeSet::new();
        for (index, row) in engineering.iter().take(MAX_ROWS).enumerate() {
            let Some(mechanism) = row.get("mechanism").and_then(Json::as_str) else {
                self.dangling(
                    CONCORDANCE_FILE,
                    format!("engineering_additions[{index}]"),
                    "malformed_reference",
                    "missing string mechanism",
                );
                continue;
            };
            if !mechanisms.insert(mechanism.to_owned()) {
                self.dangling(
                    CONCORDANCE_FILE,
                    mechanism,
                    "duplicate_key",
                    "duplicate engineering mechanism",
                );
            }
            let serves = self.references(row, mechanism, "serves");
            let serves_valid = !serves.is_empty()
                && serves
                    .iter()
                    .all(|root| is_valid_root(root) && roots.contains(root));
            for root in &serves {
                if !is_valid_root(root) {
                    self.dangling(
                        CONCORDANCE_FILE,
                        root,
                        "invalid_root",
                        format!("{mechanism} serves a malformed root"),
                    );
                } else if !roots.contains(root) {
                    self.dangling(
                        CONCORDANCE_FILE,
                        root,
                        "missing_root",
                        format!("{mechanism} serves an unknown root"),
                    );
                }
            }
            if serves.is_empty() {
                self.dangling(
                    CONCORDANCE_FILE,
                    mechanism,
                    "malformed_reference",
                    "engineering addition requires a nonempty serves array",
                );
            }
            self.cover_row(serves_valid, row, mechanism, targets);
        }

        for target in targets.required_headings {
            if !self.covered_headings.contains(target) {
                self.missing(PLAN_FILE, target, "no direct typed concordance reference");
            }
        }
        for target in targets.invariants {
            if !self.covered_invariants.contains(target) {
                self.missing(
                    INVARIANTS_FILE,
                    target,
                    "no direct typed concordance reference",
                );
            }
        }
        for target in targets.hypotheses {
            if !self.covered_hypotheses.contains(target) {
                self.missing(CLAIMS_FILE, target, "no direct typed concordance reference");
            }
        }
        for target in targets.packets {
            if !self.covered_packets.contains(target) {
                self.missing(
                    ROADMAP_FILE,
                    target,
                    "no direct typed concordance reference",
                );
            }
        }
    }

    fn cover_row(
        &mut self,
        owner_valid: bool,
        row: &Json,
        owner: &str,
        targets: TargetUniverses<'_>,
    ) {
        let sections = self.references(row, owner, "plan_sections");
        self.cover_refs(
            owner_valid,
            &sections,
            targets.headings,
            CoverageKind::Heading,
            "plan section",
        );
        let invariant_refs = self.references(row, owner, "invariants");
        self.cover_refs(
            owner_valid,
            &invariant_refs,
            targets.invariants,
            CoverageKind::Invariant,
            "invariant",
        );
        let hypothesis_refs = self.references(row, owner, "hypotheses");
        self.cover_refs(
            owner_valid,
            &hypothesis_refs,
            targets.hypotheses,
            CoverageKind::Hypothesis,
            "hypothesis",
        );
        let packet_refs = self.references(row, owner, "packets");
        self.cover_refs(
            owner_valid,
            &packet_refs,
            targets.packets,
            CoverageKind::Packet,
            "packet",
        );
    }

    fn references(&mut self, row: &Json, owner: &str, property: &str) -> Vec<String> {
        let Some(value) = row.get(property) else {
            return Vec::new();
        };
        let Some(values) = value.as_array() else {
            self.dangling(
                CONCORDANCE_FILE,
                owner,
                "malformed_reference",
                format!("{property} is not an array"),
            );
            return Vec::new();
        };
        let mut values_seen = BTreeSet::new();
        let mut references = Vec::new();
        for (index, value) in values.iter().take(MAX_REFS_PER_ROW).enumerate() {
            if !self.reference_permitted() {
                break;
            }
            let Some(reference) = value.as_str() else {
                self.dangling(
                    CONCORDANCE_FILE,
                    format!("{owner}.{property}[{index}]"),
                    "malformed_reference",
                    "reference is not a string",
                );
                continue;
            };
            if !values_seen.insert(reference.to_owned()) {
                self.dangling(
                    CONCORDANCE_FILE,
                    reference,
                    "duplicate_key",
                    format!("duplicate {property} reference from {owner}"),
                );
                continue;
            }
            references.push(reference.to_owned());
        }
        if values.len() > MAX_REFS_PER_ROW {
            self.dangling(
                CONCORDANCE_FILE,
                owner,
                "input_limit",
                format!("{property} has more than {MAX_REFS_PER_ROW} entries"),
            );
        }
        references
    }

    fn cover_refs(
        &mut self,
        owner_valid: bool,
        references: &[String],
        targets: &BTreeSet<String>,
        coverage_kind: CoverageKind,
        kind: &str,
    ) {
        for reference in references {
            if !targets.contains(reference) {
                self.dangling(
                    CONCORDANCE_FILE,
                    reference,
                    "unknown_reference",
                    format!("unknown {kind}"),
                );
            } else if owner_valid {
                match coverage_kind {
                    CoverageKind::Heading => {
                        self.covered_headings.insert(reference.clone());
                    }
                    CoverageKind::Invariant => {
                        self.covered_invariants.insert(reference.clone());
                    }
                    CoverageKind::Hypothesis => {
                        self.covered_hypotheses.insert(reference.clone());
                    }
                    CoverageKind::Packet => {
                        self.covered_packets.insert(reference.clone());
                    }
                }
            }
        }
    }

    fn finish(&mut self) {
        self.report.missing.sort();
        self.report.missing.dedup();
        self.report.dangling.sort();
        self.report.dangling.dedup();
    }
}

fn array_property<'a>(document: &'a Json, property: &str) -> Option<&'a [Json]> {
    document.get(property)?.as_array()
}

fn bounded_prefix(value: &str, limit: usize) -> &str {
    if value.len() <= limit {
        return value;
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[derive(Clone, Copy)]
struct Fence {
    marker: u8,
    run: usize,
}

fn opening_fence(line: &str) -> Option<Fence> {
    let marker = *line.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let run = line.bytes().take_while(|byte| *byte == marker).count();
    (run >= 3).then_some(Fence { marker, run })
}

fn closes_fence(line: &str, opening: Fence) -> bool {
    let bytes = line.as_bytes();
    let run = bytes
        .iter()
        .take_while(|byte| **byte == opening.marker)
        .count();
    run >= opening.run && bytes[run..].iter().all(|byte| byte.is_ascii_whitespace())
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

enum NumberedHeading {
    Numbered { key: String, major: u32 },
    Malformed(String),
    Unnumbered,
}

fn numbered_heading(title: &str) -> NumberedHeading {
    let Some(first) = title.split_whitespace().next() else {
        return NumberedHeading::Unnumbered;
    };
    let candidate = first.strip_prefix('§').unwrap_or(first);
    if !first.starts_with('§') && !first.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return NumberedHeading::Unnumbered;
    }
    let candidate = candidate.strip_suffix('.').unwrap_or(candidate);
    if candidate.is_empty() || !candidate.split('.').all(is_decimal_component) {
        return NumberedHeading::Malformed(format!("§{candidate}"));
    }
    let Some(first_component) = candidate.split('.').next() else {
        return NumberedHeading::Malformed(format!("§{candidate}"));
    };
    let Ok(major) = first_component.parse::<u32>() else {
        return NumberedHeading::Malformed(format!("§{candidate}"));
    };
    NumberedHeading::Numbered {
        key: format!("§{candidate}"),
        major,
    }
}

fn is_decimal_component(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_invariant_id(value: &str) -> bool {
    exact_decimal_suffix(value, "FA-INV-", 3)
}

fn is_packet_id(value: &str) -> bool {
    exact_decimal_suffix(value, "FA-", 3)
}

fn is_hypothesis_id(value: &str) -> bool {
    value.strip_prefix('H').is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn is_founding_id(value: &str) -> bool {
    value
        .strip_prefix("FI-A")
        .or_else(|| value.strip_prefix("FI-I"))
        .is_some_and(|suffix| suffix.len() == 2 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_synthesis_id(value: &str) -> bool {
    exact_decimal_suffix(value, "FS-", 2)
}

fn is_valid_root(value: &str) -> bool {
    is_founding_id(value) || is_synthesis_id(value)
}

fn exact_decimal_suffix(value: &str, prefix: &str, width: usize) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        suffix.len() == width && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn render_findings(output: &mut String, findings: &[Finding]) {
    for (index, finding) in findings.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"file\":");
        json_string(output, &finding.file);
        output.push_str(",\"id\":");
        json_string(output, &finding.id);
        output.push_str(",\"code\":");
        json_string(output, &finding.code);
        output.push_str(",\"detail\":");
        json_string(output, &finding.detail);
        output.push('}');
    }
}

fn json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
}
