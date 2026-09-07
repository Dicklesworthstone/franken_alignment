//! Mechanical checks for current README claims and founding-table references.
//!
//! Only the uniquely anchored current Surface table in implementation status is
//! inspected. Dated qualified-batch history remains historical evidence.

use std::collections::BTreeSet;

use crate::json::Json;

const README_FILE: &str = "README.md";
const STATUS_FILE: &str = "IMPLEMENTATION_STATUS.md";
const PLAN_FILE: &str = "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md";
const CONCORDANCE_FILE: &str = "registry/founding_concordance.json";
const INVARIANTS_FILE: &str = "registry/invariants.json";
const CLAIMS_FILE: &str = "registry/claims.json";
const ROADMAP_FILE: &str = "registry/roadmap.json";
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
const MAX_REFERENCES: usize = 4_096;
const MAX_FINDINGS: usize = 4_096;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Finding {
    pub file: String,
    pub anchor: String,
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    pub founding_ideas: usize,
    pub syntheses: usize,
    pub engineering_additions: usize,
    pub invariants: usize,
    pub hypotheses: usize,
    pub packets: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub counts: Counts,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    pub fn render_json(&self) -> String {
        let mut output = String::from("{\"findings\":[");
        for (index, finding) in self.findings.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            output.push_str("{\"file\":");
            json_string(&mut output, &finding.file);
            output.push_str(",\"anchor\":");
            json_string(&mut output, &finding.anchor);
            output.push_str(",\"code\":");
            json_string(&mut output, &finding.code);
            output.push_str(",\"detail\":");
            json_string(&mut output, &finding.detail);
            output.push('}');
        }
        output.push_str("],\"counts\":{\"founding_ideas\":");
        output.push_str(&self.counts.founding_ideas.to_string());
        output.push_str(",\"syntheses\":");
        output.push_str(&self.counts.syntheses.to_string());
        output.push_str(",\"engineering_additions\":");
        output.push_str(&self.counts.engineering_additions.to_string());
        output.push_str(",\"invariants\":");
        output.push_str(&self.counts.invariants.to_string());
        output.push_str(",\"hypotheses\":");
        output.push_str(&self.counts.hypotheses.to_string());
        output.push_str(",\"packets\":");
        output.push_str(&self.counts.packets.to_string());
        output.push_str("}}");
        output
    }
}

/// Bounded inputs selected by the gate owner for one current-consistency check.
pub struct Inputs<'a> {
    pub readme: &'a str,
    pub status: &'a str,
    pub concordance: &'a Json,
    pub invariants: &'a Json,
    pub claims: &'a Json,
    pub roadmap: &'a Json,
    pub plan: &'a str,
    pub receipt: &'a Json,
}

/// Check only explicitly current README/status claims and founding tables.
pub fn check(inputs: Inputs<'_>) -> Report {
    let mut checker = Checker::default();
    checker.check_text_bound(README_FILE, "readme", inputs.readme);
    checker.check_text_bound(STATUS_FILE, "status", inputs.status);
    checker.check_text_bound(PLAN_FILE, "plan", inputs.plan);
    let counts = Counts {
        founding_ideas: checker.array_count(inputs.concordance, CONCORDANCE_FILE, "founding_ideas"),
        syntheses: checker.array_count(inputs.concordance, CONCORDANCE_FILE, "syntheses"),
        engineering_additions: checker.array_count(
            inputs.concordance,
            CONCORDANCE_FILE,
            "engineering_additions",
        ),
        invariants: checker.array_count(inputs.invariants, INVARIANTS_FILE, "invariants"),
        hypotheses: checker.array_count(inputs.claims, CLAIMS_FILE, "claims"),
        packets: checker.array_count(inputs.roadmap, ROADMAP_FILE, "packets"),
    };
    checker.report.counts = counts;
    let readme = bounded_prefix(inputs.readme, MAX_TEXT_BYTES);
    let status = bounded_prefix(inputs.status, MAX_TEXT_BYTES);
    let plan = bounded_prefix(inputs.plan, MAX_TEXT_BYTES);
    let readme_lines = crate::concordance::unfenced_markdown_lines(readme);
    let status_lines = crate::concordance::unfenced_markdown_lines(status);
    checker.check_current_counts(&readme_lines);
    checker.check_founding_table(&readme_lines, plan);
    if let Some(tests) = qualified_tests(inputs.receipt) {
        checker.check_execution_counts(&readme_lines, tests);
        checker.check_current_status(&status_lines, tests);
    } else {
        checker.finding(
            "execution receipt",
            "execution_receipt",
            "receipt_invalid",
            "receipt lacks a qualified passing result.tests object",
        );
    }
    checker.finish();
    checker.report
}

#[derive(Default)]
struct Checker {
    report: Report,
    finding_limit_reported: bool,
}

impl Checker {
    fn finding(&mut self, file: &str, anchor: &str, code: &str, detail: impl Into<String>) {
        if self.report.findings.len() < MAX_FINDINGS {
            self.report.findings.push(Finding {
                file: file.to_owned(),
                anchor: anchor.to_owned(),
                code: code.to_owned(),
                detail: detail.into(),
            });
        } else if !self.finding_limit_reported {
            self.finding_limit_reported = true;
            self.report.findings.push(Finding {
                file: file.to_owned(),
                anchor: "findings".to_owned(),
                code: "input_limit".to_owned(),
                detail: format!("more than {MAX_FINDINGS} findings"),
            });
        }
    }

    fn check_text_bound(&mut self, file: &str, anchor: &str, text: &str) {
        if text.len() > MAX_TEXT_BYTES {
            self.finding(
                file,
                anchor,
                "input_limit",
                format!("text exceeds {MAX_TEXT_BYTES} bytes"),
            );
        }
    }

    fn array_count(&mut self, document: &Json, file: &str, property: &str) -> usize {
        match document.get(property).and_then(Json::as_array) {
            Some(values) => values.len(),
            None => {
                self.finding(file, property, "anchor_invalid", "missing array property");
                0
            }
        }
    }

    fn check_current_counts(&mut self, readme: &[&str]) {
        let Some(section) = unique_section(readme, "## Determinism, verification & governance")
        else {
            self.finding(
                README_FILE,
                "current_design",
                "anchor_invalid",
                "missing or duplicate current-design section",
            );
            return;
        };
        let invariant_count = self.report.counts.invariants;
        let hypothesis_count = self.report.counts.hypotheses;
        self.check_count_anchor(
            section,
            "registered_invariants",
            "- **Registered Invariants.**",
            "invariants",
            invariant_count,
        );
        self.check_count_anchor(
            section,
            "falsifiable_research_hypotheses",
            "- **Falsifiable Research Hypotheses.**",
            "explicit research cards",
            hypothesis_count,
        );
    }

    fn check_count_anchor(
        &mut self,
        section: &[&str],
        anchor: &str,
        prefix: &str,
        noun: &str,
        expected: usize,
    ) {
        let lines: Vec<&str> = section
            .iter()
            .copied()
            .filter(|line| line.starts_with(prefix))
            .collect();
        if lines.len() != 1 {
            self.finding(
                README_FILE,
                anchor,
                "anchor_invalid",
                "anchor is absent or ambiguous",
            );
            return;
        }
        let Some(actual) = counted_phrase(lines[0].strip_prefix(prefix).unwrap_or_default(), noun)
        else {
            self.finding(
                README_FILE,
                anchor,
                "anchor_invalid",
                format!("expected '<count> {noun}'"),
            );
            return;
        };
        if actual != expected {
            self.finding(
                README_FILE,
                anchor,
                "count_mismatch",
                format!("README says {actual}; registry has {expected}"),
            );
        }
    }

    fn check_founding_table(&mut self, readme: &[&str], plan: &str) {
        let Some(table) = unique_founding_tables(readme) else {
            self.finding(
                README_FILE,
                "founding_ideas_table",
                "anchor_invalid",
                "founding-ideas tables are absent or ambiguous",
            );
            return;
        };
        let sections = match crate::concordance::plan_heading_keys(plan) {
            Ok(keys) => keys,
            Err(_) => {
                self.finding(
                    PLAN_FILE,
                    "plan_heading_keys",
                    "plan_invalid",
                    "shared concordance heading scanner rejected the plan",
                );
                return;
            }
        };
        let mut references = BTreeSet::new();
        let mut references_seen = 0;
        for line in table {
            let mut offset = 0;
            while let Some(relative) = line[offset..].find('§') {
                let start = offset + relative;
                let value = &line[start..];
                let end = value
                    .char_indices()
                    .skip(1)
                    .find_map(|(index, character)| {
                        (character.is_whitespace()
                            || matches!(character, ',' | ';' | ')' | ']' | '}'))
                        .then_some(index)
                    })
                    .unwrap_or(value.len());
                let token = &value[..end];
                offset = start + end.max('§'.len_utf8());
                references_seen += 1;
                if references_seen > MAX_REFERENCES {
                    self.finding(
                        README_FILE,
                        "founding_ideas_table",
                        "input_limit",
                        format!("more than {MAX_REFERENCES} section references"),
                    );
                    return;
                }
                if !references.insert(token.to_owned()) {
                    continue;
                }
                if !sections.contains(token) {
                    self.finding(
                        README_FILE,
                        "founding_ideas_table",
                        "unknown_section",
                        format!("{token} is not a plan section"),
                    );
                }
            }
        }
    }

    fn check_execution_counts(&mut self, readme: &[&str], tests: ExecutionTests) {
        let Some(section) = unique_section(readme, "## Determinism, verification & governance")
        else {
            self.finding(
                README_FILE,
                "reference_oracle_tests",
                "anchor_invalid",
                "missing or duplicate current-design section",
            );
            return;
        };
        let lines: Vec<&str> = section
            .iter()
            .copied()
            .filter(|line| line.starts_with("- **Reference Oracle.**"))
            .collect();
        if lines.len() != 1 {
            self.finding(
                README_FILE,
                "reference_oracle_tests",
                "anchor_invalid",
                "anchor is absent or ambiguous",
            );
            return;
        }
        let Some((unit, integration)) = reference_oracle_counts(lines[0]) else {
            self.finding(
                README_FILE,
                "reference_oracle_tests",
                "anchor_invalid",
                "expected qualified unit and public-API integration counts",
            );
            return;
        };
        if unit != tests.reference_unit {
            self.finding(
                README_FILE,
                "reference_oracle_tests",
                "count_mismatch",
                format!(
                    "README says {unit} qualified unit tests; receipt has {}",
                    tests.reference_unit
                ),
            );
        }
        if integration != tests.reference_integration {
            self.finding(
                README_FILE,
                "reference_oracle_tests",
                "count_mismatch",
                format!(
                    "README says {integration} public-API integration tests; receipt has {}",
                    tests.reference_integration
                ),
            );
        }
    }

    fn check_current_status(&mut self, status: &[&str], tests: ExecutionTests) {
        let Some(table) = unique_status_surface_table(status) else {
            self.finding(
                STATUS_FILE,
                "current_surface_table",
                "anchor_invalid",
                "current Surface table is absent or ambiguous",
            );
            return;
        };
        self.check_status_tests(table, tests);
        let counts = self.report.counts.clone();
        self.check_status_registry_counts(table, &counts);
        self.check_status_concordance_counts(table, &counts);
    }

    fn check_status_tests(&mut self, table: &[&str], tests: ExecutionTests) {
        let Some(row) = unique_table_row(table, "| Rust test functions |") else {
            self.finding(
                STATUS_FILE,
                "status_rust_test_functions",
                "anchor_invalid",
                "row is absent or ambiguous",
            );
            return;
        };
        for (phrase, count) in [
            ("unit", tests.reference_unit),
            ("integration reference tests", tests.reference_integration),
            ("xtask tests", tests.xtask),
            ("doctests", tests.doctests),
        ] {
            if number_before(row, phrase) != Some(count) {
                self.finding(
                    STATUS_FILE,
                    "status_rust_test_functions",
                    "count_mismatch",
                    format!("current Surface row does not bind receipt {phrase}={count}"),
                );
            }
        }
    }

    fn check_status_registry_counts(&mut self, table: &[&str], counts: &Counts) {
        let Some(row) = unique_table_row(table, "| Machine-readable registries |") else {
            self.finding(
                STATUS_FILE,
                "status_machine_readable_registries",
                "anchor_invalid",
                "row is absent or ambiguous",
            );
            return;
        };
        for (phrase, count) in [
            ("invariants", counts.invariants),
            ("hypotheses", counts.hypotheses),
            ("packets", counts.packets),
            ("ideas", counts.founding_ideas),
            ("syntheses", counts.syntheses),
            ("engineering additions", counts.engineering_additions),
        ] {
            if number_before(row, phrase) != Some(count) {
                self.finding(
                    STATUS_FILE,
                    "status_machine_readable_registries",
                    "count_mismatch",
                    format!("current Surface row does not bind registry {phrase}={count}"),
                );
            }
        }
    }

    fn check_status_concordance_counts(&mut self, table: &[&str], counts: &Counts) {
        let Some(row) = unique_table_row(table, "| Founding-ideas concordance |") else {
            self.finding(
                STATUS_FILE,
                "status_founding_ideas_concordance",
                "anchor_invalid",
                "row is absent or ambiguous",
            );
            return;
        };
        for (phrase, count) in [
            ("founding ideas", counts.founding_ideas),
            ("syntheses", counts.syntheses),
            ("engineering additions", counts.engineering_additions),
        ] {
            if number_before(row, phrase) != Some(count) {
                self.finding(
                    STATUS_FILE,
                    "status_founding_ideas_concordance",
                    "count_mismatch",
                    format!("current Surface row does not bind concordance {phrase}={count}"),
                );
            }
        }
    }

    fn finish(&mut self) {
        self.report.findings.sort();
        self.report.findings.dedup();
    }
}

#[derive(Clone, Copy)]
struct ExecutionTests {
    reference_unit: usize,
    reference_integration: usize,
    xtask: usize,
    doctests: usize,
}

fn qualified_tests(receipt: &Json) -> Option<ExecutionTests> {
    if receipt.get("schema")?.as_str()? != "fa.execution_receipt/1" {
        return None;
    }
    let result = receipt.get("result")?.as_object()?;
    if result.get("exit_code")?.as_u64()? != 0
        || result.get("format")?.as_str()? != "passed"
        || result.get("check")?.as_str()? != "passed"
        || !matches!(
            result.get("clippy")?.as_str()?,
            "passed" | "passed; warnings denied"
        )
    {
        return None;
    }
    let tests = result.get("tests")?.as_object()?;
    let reference_unit = usize::try_from(tests.get("reference_unit")?.as_u64()?).ok()?;
    let reference_integration =
        usize::try_from(tests.get("reference_integration")?.as_u64()?).ok()?;
    let xtask = usize::try_from(tests.get("xtask")?.as_u64()?).ok()?;
    let doctests = usize::try_from(tests.get("doctests")?.as_u64()?).ok()?;
    let total_passed = usize::try_from(tests.get("total_passed")?.as_u64()?).ok()?;
    if total_passed == 0
        || tests.get("failed")?.as_u64()? != 0
        || reference_unit
            .checked_add(reference_integration)?
            .checked_add(xtask)?
            .checked_add(doctests)?
            != total_passed
    {
        return None;
    }
    Some(ExecutionTests {
        reference_unit,
        reference_integration,
        xtask,
        doctests,
    })
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

fn unique_status_surface_table<'a>(status: &'a [&'a str]) -> Option<&'a [&'a str]> {
    let historical = status
        .iter()
        .position(|line| line.starts_with("## Execution facts recorded "))
        .unwrap_or(status.len());
    let status = &status[..historical];
    let header = "| Surface | Actual status | Claim boundary |";
    let starts: Vec<usize> = status
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (*line == header).then_some(index))
        .collect();
    let [start] = starts.as_slice() else {
        return None;
    };
    let end = status[*start + 1..]
        .iter()
        .position(|line| line.is_empty())
        .map_or(status.len(), |offset| *start + 1 + offset);
    Some(&status[*start + 1..end])
}

fn unique_table_row<'a>(table: &'a [&'a str], prefix: &str) -> Option<&'a str> {
    let rows: Vec<&str> = table
        .iter()
        .copied()
        .filter(|line| line.starts_with(prefix))
        .collect();
    match rows.as_slice() {
        [row] => Some(*row),
        _ => None,
    }
}

fn number_before(text: &str, phrase: &str) -> Option<usize> {
    let position = text.find(phrase)?;
    let prefix = text[..position].trim_end();
    let start = prefix
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            (!character.is_ascii_digit()).then_some(index + character.len_utf8())
        })
        .unwrap_or(0);
    prefix[start..].parse().ok()
}

fn reference_oracle_counts(line: &str) -> Option<(usize, usize)> {
    Some((
        number_before(line, "qualified unit tests")?,
        number_before(line, "public-API integration tests")?,
    ))
}

fn unique_section<'a>(text: &'a [&'a str], heading: &str) -> Option<&'a [&'a str]> {
    let starts: Vec<usize> = text
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (*line == heading).then_some(index))
        .collect();
    let [start] = starts.as_slice() else {
        return None;
    };
    let end = text[*start + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(text.len(), |offset| *start + 1 + offset);
    Some(&text[*start + 1..end])
}

fn unique_founding_tables<'a>(readme: &'a [&'a str]) -> Option<&'a [&'a str]> {
    let heading = "## The founding ideas";
    let starts: Vec<usize> = readme
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (*line == heading).then_some(index))
        .collect();
    let [start] = starts.as_slice() else {
        return None;
    };
    let end = readme[*start + 1..]
        .iter()
        .position(|line| *line == "---")
        .map_or(readme.len(), |offset| *start + 1 + offset);
    let tables = &readme[*start + 1..end];
    (tables.iter().filter(|line| line.starts_with('|')).count() >= 4).then_some(tables)
}

fn counted_phrase(line: &str, noun: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let digits_len = trimmed
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits_len == 0 {
        return None;
    }
    let value = trimmed[..digits_len].parse().ok()?;
    let remainder = trimmed[digits_len..].trim_start();
    remainder.starts_with(noun).then_some(value)
}

fn json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
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
