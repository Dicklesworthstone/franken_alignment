//! Independent causal conformance tests for README prose consistency.
//!
//! The corpus is intentionally small: each refusal changes one current-design
//! anchor or one founding-table section reference while preserving a paired,
//! permitted fixture. Historical prose and test-function counts are controls,
//! not a substitute for executed evidence.

use crate::json::{Json, Limits, parse};
use crate::prose_consistency::{Finding, Inputs, Report, check};

const CONCORDANCE: &[u8] = include_bytes!("../tests/fixtures/prose-consistency/concordance.json");
const INVARIANTS: &[u8] = include_bytes!("../tests/fixtures/prose-consistency/invariants.json");
const CLAIMS: &[u8] = include_bytes!("../tests/fixtures/prose-consistency/claims.json");
const ROADMAP: &[u8] = include_bytes!("../tests/fixtures/prose-consistency/roadmap.json");
const PLAN: &str = include_str!("../tests/fixtures/prose-consistency/plan.md");
const FENCED_FAKE_HEADING_PLAN: &str =
    include_str!("../tests/fixtures/prose-consistency/plan-fenced-fake-heading.md");
const REAL_99_9_HEADING_PLAN: &str =
    include_str!("../tests/fixtures/prose-consistency/plan-real-99-9-heading.md");
const MALFORMED_HEADING_PLAN: &str =
    include_str!("../tests/fixtures/prose-consistency/plan-malformed-heading.md");
const STATUS: &str = include_str!("../tests/fixtures/prose-consistency/status.md");
const STALE_STATUS_XTASK: &str =
    include_str!("../tests/fixtures/prose-consistency/status-stale-xtask.md");
const STATUS_MISSING_RUST_ROW: &str =
    include_str!("../tests/fixtures/prose-consistency/status-missing-rust-row.md");
const RECEIPT: &[u8] = include_bytes!("../tests/fixtures/prose-consistency/receipt.json");
const FAILED_RECEIPT: &[u8] =
    include_bytes!("../tests/fixtures/prose-consistency/receipt-failed.json");
const BAD_TOTAL_RECEIPT: &[u8] =
    include_bytes!("../tests/fixtures/prose-consistency/receipt-bad-total.json");
const MISSING_CLIPPY_RECEIPT: &[u8] =
    include_bytes!("../tests/fixtures/prose-consistency/receipt-missing-clippy.json");
const UNKNOWN_FORMAT_RECEIPT: &[u8] =
    include_bytes!("../tests/fixtures/prose-consistency/receipt-unknown-format.json");
const ZERO_TOTAL_RECEIPT: &[u8] =
    include_bytes!("../tests/fixtures/prose-consistency/receipt-zero-total.json");

const PERMITTED: &str = include_str!("../tests/fixtures/prose-consistency/permitted.md");
const STALE_INVARIANTS: &str =
    include_str!("../tests/fixtures/prose-consistency/stale-invariants.md");
const MISSING_INVARIANTS: &str =
    include_str!("../tests/fixtures/prose-consistency/missing-invariants.md");
const DUPLICATE_HYPOTHESES: &str =
    include_str!("../tests/fixtures/prose-consistency/duplicate-hypotheses.md");
const MALFORMED_INVARIANTS: &str =
    include_str!("../tests/fixtures/prose-consistency/malformed-invariants.md");
const UNKNOWN_SECTION: &str =
    include_str!("../tests/fixtures/prose-consistency/unknown-section.md");
const HISTORICAL_TEST_PROSE: &str =
    include_str!("../tests/fixtures/prose-consistency/historical-test-prose.md");
const EXECUTED_REFERENCE_ORACLE: &str =
    include_str!("../tests/fixtures/prose-consistency/executed-reference-oracle.md");
const STALE_REFERENCE_ORACLE: &str =
    include_str!("../tests/fixtures/prose-consistency/stale-reference-oracle.md");
const FENCED_CURRENT_ONLY: &str =
    include_str!("../tests/fixtures/prose-consistency/fenced-current-only.md");
const SUFFIXED_CURRENT_HEADING: &str =
    include_str!("../tests/fixtures/prose-consistency/suffixed-current-heading.md");

fn fixture_json(bytes: &[u8]) -> Json {
    parse(bytes, Limits::default()).expect("independent prose fixture must be strict JSON")
}

fn report(readme: &str) -> Report {
    report_with_inputs(readme, STATUS, PLAN, RECEIPT)
}

fn report_with_evidence(readme: &str, status: &str, receipt: &[u8]) -> Report {
    report_with_inputs(readme, status, PLAN, receipt)
}

fn report_with_inputs(readme: &str, status: &str, plan: &str, receipt: &[u8]) -> Report {
    let concordance = fixture_json(CONCORDANCE);
    let invariants = fixture_json(INVARIANTS);
    let claims = fixture_json(CLAIMS);
    let roadmap = fixture_json(ROADMAP);
    let receipt = fixture_json(receipt);
    check(Inputs {
        readme,
        status,
        concordance: &concordance,
        invariants: &invariants,
        claims: &claims,
        roadmap: &roadmap,
        plan,
        receipt: &receipt,
    })
}

fn assert_finding(findings: &[Finding], anchor: &str, code: &str, detail: &str) {
    assert!(
        findings.iter().any(|finding| {
            finding.anchor == anchor && finding.code == code && finding.detail == detail
        }),
        "missing intended finding {anchor}/{code}/{detail:?}; actual findings: {findings:#?}"
    );
}

fn assert_only_finding(report: &Report, anchor: &str, code: &str, detail: &str) {
    assert_eq!(
        report.findings.len(),
        1,
        "one-field fixture mutation must have one causal refusal: {report:#?}"
    );
    assert_finding(&report.findings, anchor, code, detail);
}

#[test]
fn permitted_current_snapshot_and_founding_table_match_the_small_registries() {
    let report = report(PERMITTED);
    assert!(report.is_clean(), "permitted fixture: {report:#?}");
    assert_eq!(report.counts.founding_ideas, 2);
    assert_eq!(report.counts.invariants, 1);
    assert_eq!(report.counts.hypotheses, 1);
    assert_eq!(report.counts.packets, 1);
}

#[test]
fn stale_current_invariant_count_is_not_hidden_by_a_valid_table() {
    let report = report(STALE_INVARIANTS);
    assert_only_finding(
        &report,
        "registered_invariants",
        "count_mismatch",
        "README says 2; registry has 1",
    );
}

#[test]
fn missing_ambiguous_and_malformed_current_anchors_fail_closed() {
    let missing = report(MISSING_INVARIANTS);
    assert_only_finding(
        &missing,
        "registered_invariants",
        "anchor_invalid",
        "anchor is absent or ambiguous",
    );

    let duplicate = report(DUPLICATE_HYPOTHESES);
    assert_only_finding(
        &duplicate,
        "falsifiable_research_hypotheses",
        "anchor_invalid",
        "anchor is absent or ambiguous",
    );

    let malformed = report(MALFORMED_INVARIANTS);
    assert_only_finding(
        &malformed,
        "registered_invariants",
        "anchor_invalid",
        "expected '<count> invariants'",
    );
}

#[test]
fn fenced_current_section_copy_cannot_satisfy_readme_anchors() {
    let report = report(FENCED_CURRENT_ONLY);
    assert_eq!(
        report.findings.len(),
        2,
        "only a fenced current section must not satisfy either README current anchor: {report:#?}"
    );
    assert_finding(
        &report.findings,
        "current_design",
        "anchor_invalid",
        "missing or duplicate current-design section",
    );
    assert_finding(
        &report.findings,
        "reference_oracle_tests",
        "anchor_invalid",
        "missing or duplicate current-design section",
    );
}

#[test]
fn suffixed_current_heading_cannot_satisfy_exact_readme_anchors() {
    let report = report(SUFFIXED_CURRENT_HEADING);
    assert_eq!(
        report.findings.len(),
        2,
        "a suffixed heading must not satisfy either exact README current anchor: {report:#?}"
    );
    for anchor in ["current_design", "reference_oracle_tests"] {
        assert_finding(
            &report.findings,
            anchor,
            "anchor_invalid",
            "missing or duplicate current-design section",
        );
    }
}

#[test]
fn founding_table_requires_exact_current_plan_section_keys() {
    let fenced = report_with_inputs(UNKNOWN_SECTION, STATUS, FENCED_FAKE_HEADING_PLAN, RECEIPT);
    assert_only_finding(
        &fenced,
        "founding_ideas_table",
        "unknown_section",
        "§99.9 is not a plan section",
    );

    let real = report_with_inputs(UNKNOWN_SECTION, STATUS, REAL_99_9_HEADING_PLAN, RECEIPT);
    assert!(
        real.is_clean(),
        "the paired real heading, not a fenced lookalike, makes §99.9 valid: {real:#?}"
    );
}

#[test]
fn shared_concordance_scanner_refusal_is_not_reparsed_locally() {
    let report = report_with_inputs(PERMITTED, STATUS, MALFORMED_HEADING_PLAN, RECEIPT);
    assert_only_finding(
        &report,
        "plan_heading_keys",
        "plan_invalid",
        "shared concordance heading scanner rejected the plan",
    );
}

#[test]
fn current_reference_oracle_claim_uses_the_executed_receipt_categories() {
    let permitted = report(EXECUTED_REFERENCE_ORACLE);
    assert!(
        permitted.is_clean(),
        "executed receipt twin: {permitted:#?}"
    );

    let stale = report(STALE_REFERENCE_ORACLE);
    assert_only_finding(
        &stale,
        "reference_oracle_tests",
        "count_mismatch",
        "README says 2 qualified unit tests; receipt has 1",
    );
}

#[test]
fn receipt_must_be_complete_passing_and_arithmetically_consistent() {
    for receipt in [
        FAILED_RECEIPT,
        BAD_TOTAL_RECEIPT,
        MISSING_CLIPPY_RECEIPT,
        UNKNOWN_FORMAT_RECEIPT,
        ZERO_TOTAL_RECEIPT,
    ] {
        let report = report_with_evidence(EXECUTED_REFERENCE_ORACLE, STATUS, receipt);
        assert_only_finding(
            &report,
            "execution_receipt",
            "receipt_invalid",
            "receipt lacks a qualified passing result.tests object",
        );
    }
}

#[test]
fn current_status_uses_receipt_categories_and_ignores_dated_tables() {
    let permitted = report(EXECUTED_REFERENCE_ORACLE);
    assert!(permitted.is_clean(), "dated status control: {permitted:#?}");

    let stale = report_with_evidence(EXECUTED_REFERENCE_ORACLE, STALE_STATUS_XTASK, RECEIPT);
    assert_only_finding(
        &stale,
        "status_rust_test_functions",
        "count_mismatch",
        "current Surface row does not bind receipt xtask tests=1",
    );
}

#[test]
fn missing_current_status_row_refuses_without_indexing_an_empty_row_set() {
    let report = report_with_evidence(EXECUTED_REFERENCE_ORACLE, STATUS_MISSING_RUST_ROW, RECEIPT);
    assert_only_finding(
        &report,
        "status_rust_test_functions",
        "anchor_invalid",
        "row is absent or ambiguous",
    );
}

#[test]
fn historical_and_test_count_prose_do_not_claim_executed_evidence() {
    let report = report(HISTORICAL_TEST_PROSE);
    assert!(
        report.is_clean(),
        "dated history and source test counts are outside this checker’s evidence scope: {report:#?}"
    );
}
