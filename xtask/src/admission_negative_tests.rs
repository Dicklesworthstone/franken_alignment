//! Independent causal regression tests for the closed-inventory admission gate.
//!
//! These tests deliberately use the captured metadata as their baseline, then
//! make one asserted mutation per bypass.  They are separate from the checker's
//! unit tests so that an implementation change cannot silently redefine both a
//! branch and the only test that exercises it.

use crate::admission::{Phase, Policy, Report, check, parse_policy};

const REAL_METADATA: &str = include_str!("../tests/fixtures/admission/metadata-current.json");
const POLICY: &str = include_str!("../tests/fixtures/admission/policy-closed-inventory.json");

fn policy() -> Policy {
    parse_policy(POLICY.as_bytes()).expect("closed-inventory policy must parse")
}

fn admitted_target(policy: &Policy) -> &str {
    policy
        .profiles
        .first()
        .expect("fixture policy must bind at least one target")
        .target
        .as_str()
}

fn check_for_admitted_target(metadata: &str) -> Report {
    let policy = policy();
    check(metadata.as_bytes(), &policy, admitted_target(&policy)).expect("metadata mutation parses")
}

fn assert_refusal(report: &Report, phase: Phase, code: &'static str) {
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.phase == phase && finding.code == code),
        "expected causal refusal [{}] {code}; got {:?}",
        phase.as_str(),
        report.findings
    );
    assert!(
        !report.is_admitted(),
        "a refusal finding cannot admit metadata"
    );
}

/// Replace exactly one known fragment so fixture drift cannot turn a negative
/// test into a baseline pass.
fn mutate(source: &str, from: &str, to: &str) -> String {
    assert_eq!(
        source.matches(from).count(),
        1,
        "fixture anchor must occur exactly once: {from:?}"
    );
    source.replacen(from, to, 1)
}

#[test]
fn external_source_is_unconditionally_refused() {
    let metadata = mutate(
        REAL_METADATA,
        "\"license_file\":\"../LICENSE\",\"description\":null,\"source\":null",
        "\"license_file\":\"../LICENSE\",\"description\":null,\"source\":\"registry+https://github.com/rust-lang/crates.io-index\"",
    );
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Source, "unsupported_external_admission");
}

#[test]
fn policy_external_row_is_rejected_before_it_can_launder_authority() {
    let policy_text = mutate(
        POLICY,
        "\"external_rows\": [],",
        "\"external_rows\":[{\"name\":\"serde\",\"version\":\"1.0.0\",\"source\":\"registry+x\",\"runtime_closure_inspected\":true}],",
    );
    let error = parse_policy(policy_text.as_bytes()).expect_err("external rows are unsupported");
    assert!(
        error.contains("external_rows"),
        "the hard policy error must identify the prohibited acceptance row: {error}"
    );
}

#[test]
fn missing_resolve_nodes_is_not_an_empty_graph() {
    let metadata = mutate(
        REAL_METADATA,
        "\"resolve\":{\"nodes\":[",
        "\"resolve\":{\"other\":[",
    );
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Graph, "missing_field");
}

#[test]
fn missing_node_deps_is_not_no_dependency_edges() {
    let fa_reference_node = "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/crates/fa-reference#0.2.0\",\"dependencies\":[],\"deps\":[],\"features\":[]}";
    let metadata = mutate(
        REAL_METADATA,
        fa_reference_node,
        "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/crates/fa-reference#0.2.0\",\"dependencies\":[],\"features\":[]}",
    );
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Graph, "missing_field");
}

#[test]
fn duplicate_resolve_node_cannot_overwrite_earlier_evidence() {
    let node = "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/xtask#0.2.0\",\"dependencies\":[],\"deps\":[],\"features\":[]}";
    let metadata = mutate(REAL_METADATA, node, &format!("{node},{node}"));
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Graph, "duplicate_resolve_node");
}

#[test]
fn package_id_without_its_resolve_node_is_refused() {
    let metadata = mutate(
        REAL_METADATA,
        "\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/xtask#0.2.0\",\"license\":null",
        "\"id\":\"path+file:///unrelated/xtask#0.2.0\",\"license\":null",
    );
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Graph, "package_missing_from_resolve");
}

#[test]
fn workspace_member_without_a_package_is_refused() {
    let metadata = mutate(
        REAL_METADATA,
        "\"workspace_members\":[",
        "\"workspace_members\":[\"path+file:///ghost#1.0.0\",",
    );
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Graph, "workspace_member_without_package");
}

#[test]
fn non_string_source_and_links_cannot_be_filtered_away() {
    let metadata = mutate(
        REAL_METADATA,
        "\"license_file\":\"../LICENSE\",\"description\":null,\"source\":null",
        "\"license_file\":\"../LICENSE\",\"description\":null,\"source\":[]",
    );
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Source, "expected_null_or_string");

    let link_count = REAL_METADATA.matches("\"links\":null").count();
    assert_eq!(link_count, 2, "captured two-package fixture drifted");
    let metadata = REAL_METADATA.replacen("\"links\":null", "\"links\":{}", 1);
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Build, "expected_null_or_string");
}

#[test]
fn empty_target_kind_cannot_exempt_build_checks() {
    let metadata = mutate(REAL_METADATA, "\"kind\":[\"bin\"]", "\"kind\":[]");
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Build, "empty_target_kind");
}

#[test]
fn sibling_path_prefix_is_not_workspace_containment() {
    let metadata = mutate(
        REAL_METADATA,
        "\"manifest_path\":\"/Users/jemanuel/projects/franken_alignment/xtask/Cargo.toml\"",
        "\"manifest_path\":\"/Users/jemanuel/projects/franken_alignmentxtask/Cargo.toml\"",
    );
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Source, "manifest_path_not_contained");
}

#[test]
fn policy_cannot_enable_or_omit_a_constitutional_prohibition() {
    let enabled = mutate(
        POLICY,
        "\"allow_build_scripts\": false",
        "\"allow_build_scripts\": true",
    );
    let error = parse_policy(enabled.as_bytes()).expect_err("true cannot relax prohibition");
    assert!(error.contains("allow_build_scripts"), "{error}");

    let omitted = mutate(POLICY, "\"allow_links\": false", "\"rules_missing\": false");
    let error = parse_policy(omitted.as_bytes()).expect_err("absence cannot default to false");
    assert!(error.contains("allow_links"), "{error}");
}

#[test]
fn unknown_target_profile_is_refused_before_metadata_is_claimed() {
    let policy = policy();
    let report = check(
        REAL_METADATA.as_bytes(),
        &policy,
        "not-a-frozen-cargo-target",
    )
    .expect("metadata parses");
    assert_refusal(&report, Phase::Target, "unknown_target_profile");
}

#[test]
fn feature_only_injection_is_compared_against_the_bound_profile() {
    let fa_reference_node = "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/crates/fa-reference#0.2.0\",\"dependencies\":[],\"deps\":[],\"features\":[]}";
    let metadata = mutate(
        REAL_METADATA,
        fa_reference_node,
        "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/crates/fa-reference#0.2.0\",\"dependencies\":[],\"deps\":[],\"features\":[\"native-zstd\"]}",
    );
    let report = check_for_admitted_target(&metadata);
    assert_refusal(&report, Phase::Features, "feature_set_mismatch");
}
