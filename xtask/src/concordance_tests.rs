//! Independent causal conformance tests for the founding concordance checker.
//!
//! This corpus intentionally does not derive expectations from checker
//! implementation helpers. Each negative differs from a paired permitted
//! fixture in one semantic respect, and its report is frozen as deterministic
//! JSON for review.

use crate::concordance::{Finding, Report, check};
use crate::json::{Json, Limits, parse};

const BASE_CONCORDANCE: &[u8] =
    include_bytes!("../tests/fixtures/concordance/baseline/concordance.json");
const BASE_INVARIANTS: &[u8] =
    include_bytes!("../tests/fixtures/concordance/baseline/invariants.json");
const BASE_CLAIMS: &[u8] = include_bytes!("../tests/fixtures/concordance/baseline/claims.json");
const BASE_ROADMAP: &[u8] = include_bytes!("../tests/fixtures/concordance/baseline/roadmap.json");
const BASE_PLAN: &str = include_str!("../tests/fixtures/concordance/baseline/plan.md");

const REMOVE_FI_A10: &[u8] =
    include_bytes!("../tests/fixtures/concordance/remove-fi-a10/concordance.json");
const ADD_FA_999_ROADMAP: &[u8] =
    include_bytes!("../tests/fixtures/concordance/add-fa-999/roadmap.json");
const DANGLING_FA_INV_099: &[u8] =
    include_bytes!("../tests/fixtures/concordance/dangling-fa-inv-099/concordance.json");
const RENAMED_9_8: &str = include_str!("../tests/fixtures/concordance/rename-9-8-to-9-8a/plan.md");
const DUPLICATE_CANONICAL_KEY: &str =
    include_str!("../tests/fixtures/concordance/duplicate-canonical-key/plan.md");
const INVALID_ROOT: &[u8] =
    include_bytes!("../tests/fixtures/concordance/invalid-root/concordance.json");
const MISSING_ROOT: &[u8] =
    include_bytes!("../tests/fixtures/concordance/missing-root/concordance.json");
const FENCE_BOUNDARIES: &[u8] =
    include_bytes!("../tests/fixtures/concordance/fence-boundaries/concordance.json");
const FENCE_BOUNDARIES_PLAN: &str =
    include_str!("../tests/fixtures/concordance/fence-boundaries/plan.md");

const BASELINE_REPORT: &str =
    include_str!("../tests/fixtures/concordance/baseline/expected-report.json");
const REMOVE_FI_A10_REPORT: &str =
    include_str!("../tests/fixtures/concordance/remove-fi-a10/expected-report.json");
const ADD_FA_999_REPORT: &str =
    include_str!("../tests/fixtures/concordance/add-fa-999/expected-report.json");
const DANGLING_FA_INV_099_REPORT: &str =
    include_str!("../tests/fixtures/concordance/dangling-fa-inv-099/expected-report.json");
const RENAMED_9_8_REPORT: &str =
    include_str!("../tests/fixtures/concordance/rename-9-8-to-9-8a/expected-report.json");
const DUPLICATE_CANONICAL_KEY_REPORT: &str =
    include_str!("../tests/fixtures/concordance/duplicate-canonical-key/expected-report.json");
const INVALID_ROOT_REPORT: &str =
    include_str!("../tests/fixtures/concordance/invalid-root/expected-report.json");
const MISSING_ROOT_REPORT: &str =
    include_str!("../tests/fixtures/concordance/missing-root/expected-report.json");
const FENCE_BOUNDARIES_REPORT: &str =
    include_str!("../tests/fixtures/concordance/fence-boundaries/expected-report.json");

fn fixture_json(bytes: &[u8]) -> Json {
    parse(bytes, Limits::default())
        .expect("the independent concordance fixture must be strict JSON")
}

fn report(concordance: &[u8], plan: &str) -> Report {
    report_with_roadmap(concordance, BASE_ROADMAP, plan)
}

fn report_with_roadmap(concordance: &[u8], roadmap: &[u8], plan: &str) -> Report {
    let concordance = fixture_json(concordance);
    let invariants = fixture_json(BASE_INVARIANTS);
    let claims = fixture_json(BASE_CLAIMS);
    let roadmap = fixture_json(roadmap);
    check(&concordance, &invariants, &claims, &roadmap, plan)
}

fn assert_frozen_json(case: &str, report: &Report, expected: &str) {
    assert_eq!(
        report.render_json(),
        expected.trim_end(),
        "{case}: deterministic report changed; review semantic output rather than regenerating its fixture"
    );
}

fn assert_finding(findings: &[Finding], id: &str, code: &str) {
    assert!(
        findings
            .iter()
            .any(|finding| finding.id == id && finding.code == code),
        "missing intended finding {id}/{code}; actual findings: {findings:#?}"
    );
}

#[test]
fn permitted_baseline_covers_numbered_unicode_and_fenced_heading_controls() {
    let report = report(BASE_CONCORDANCE, BASE_PLAN);
    assert!(report.is_clean(), "paired permitted fixture: {report:#?}");
    assert_eq!(report.counts.headings, 5);
    assert_eq!(report.counts.invariants, 1);
    assert_eq!(report.counts.hypotheses, 1);
    assert_eq!(report.counts.packets, 1);
    assert_frozen_json("baseline", &report, BASELINE_REPORT);
}

#[test]
fn fenced_heading_controls_require_matching_marker_run_and_blank_closure_tail() {
    let report = report(FENCE_BOUNDARIES, FENCE_BOUNDARIES_PLAN);
    assert!(report.is_clean(), "fence-boundary control: {report:#?}");
    assert_eq!(report.counts.headings, 6);
    assert_frozen_json("fence-boundaries", &report, FENCE_BOUNDARIES_REPORT);
}

#[test]
fn removal_of_fi_a10_exposes_its_unique_coverage_and_dangling_reference() {
    let report = report(REMOVE_FI_A10, BASE_PLAN);
    assert!(!report.is_clean());
    assert_finding(&report.dangling, "FI-A10", "missing_root");
    for id in [
        "§2",
        "§2.1",
        "§9",
        "§9.8",
        "§9#Signals — α/β!",
        "FA-INV-001",
        "H1",
        "FA-001",
    ] {
        assert_finding(&report.missing, id, "uncovered");
    }
    assert_frozen_json("remove-fi-a10", &report, REMOVE_FI_A10_REPORT);
}

#[test]
fn new_roadmap_packet_is_uncovered_when_the_concordance_is_unchanged() {
    let report = report_with_roadmap(BASE_CONCORDANCE, ADD_FA_999_ROADMAP, BASE_PLAN);
    assert!(!report.is_clean());
    assert_finding(&report.missing, "FA-999", "uncovered");
    assert_frozen_json("add-fa-999", &report, ADD_FA_999_REPORT);
}

#[test]
fn unknown_invariant_reference_is_a_distinct_causal_refusal() {
    let report = report(DANGLING_FA_INV_099, BASE_PLAN);
    assert!(!report.is_clean());
    assert_finding(&report.dangling, "FA-INV-099", "unknown_reference");
    assert_frozen_json("dangling-fa-inv-099", &report, DANGLING_FA_INV_099_REPORT);
}

#[test]
fn malformed_9_8a_does_not_prefix_recover_the_absent_9_8_reference() {
    let report = report(BASE_CONCORDANCE, RENAMED_9_8);
    assert!(!report.is_clean());
    assert_finding(&report.dangling, "§9.8a", "malformed_heading");
    assert_finding(&report.dangling, "§9.8", "unknown_reference");
    assert!(
        !report.missing.iter().any(|finding| finding.id == "§9.8"),
        "a malformed 9.8a token must not invent an uncovered §9.8 target"
    );
    assert_frozen_json("rename-9-8-to-9-8a", &report, RENAMED_9_8_REPORT);
}

#[test]
fn duplicate_canonical_heading_key_is_refused_even_when_one_copy_is_covered() {
    let report = report(BASE_CONCORDANCE, DUPLICATE_CANONICAL_KEY);
    assert!(!report.is_clean());
    assert_finding(&report.dangling, "§9.8", "duplicate_key");
    assert_frozen_json(
        "duplicate-canonical-key",
        &report,
        DUPLICATE_CANONICAL_KEY_REPORT,
    );
}

#[test]
fn malformed_and_absent_mapping_roots_are_distinct_refusals() {
    let invalid = report(INVALID_ROOT, BASE_PLAN);
    assert!(!invalid.is_clean());
    assert_finding(&invalid.dangling, "FA-ROOT", "invalid_root");
    assert_frozen_json("invalid-root", &invalid, INVALID_ROOT_REPORT);

    let missing = report(MISSING_ROOT, BASE_PLAN);
    assert!(!missing.is_clean());
    assert_finding(&missing.dangling, "FI-A11", "missing_root");
    assert_frozen_json("missing-root", &missing, MISSING_ROOT_REPORT);
}
