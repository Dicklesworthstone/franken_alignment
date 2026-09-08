//! Independent causal tests for the system-map conformance checker.
//!
//! These fixtures keep cross-layer access separate from vocabulary ownership:
//! repeated get access across all nine layers is permitted even though the
//! vocabulary assigns get to L0. The Bead control verifies only the live
//! FA-132 checker-packet link, never broad roadmap coverage.

use crate::json::{Json, Limits, parse};
use crate::system_map::{Inputs, Report, check};

const SYSTEM_MAP: &[u8] = include_bytes!("../tests/fixtures/system-map/system-map.json");
const MISSING_PACKET_MAP: &[u8] =
    include_bytes!("../tests/fixtures/system-map/system-map-missing-packet.json");
const VOCABULARY: &[u8] = include_bytes!("../tests/fixtures/system-map/vocabulary.json");
const MISSING_AUTHORITY_VOCABULARY: &[u8] =
    include_bytes!("../tests/fixtures/system-map/vocabulary-missing-authority.json");
const BAD_CLASS_VOCABULARY: &[u8] =
    include_bytes!("../tests/fixtures/system-map/vocabulary-bad-class.json");
const MALFORMED_LAYER_VOCABULARY: &[u8] =
    include_bytes!("../tests/fixtures/system-map/vocabulary-malformed-layer.json");
const INVARIANTS: &[u8] = include_bytes!("../tests/fixtures/system-map/invariants.json");
const ROADMAP: &[u8] = include_bytes!("../tests/fixtures/system-map/roadmap.json");
const PLAN: &str = include_str!("../tests/fixtures/system-map/plan.md");
const UNKNOWN_REASON_PLAN: &str =
    include_str!("../tests/fixtures/system-map/plan-unknown-reason.md");
const EMPTY_REASON_LIST_PLAN: &str =
    include_str!("../tests/fixtures/system-map/plan-empty-reason-list.md");
const FENCED_FAKE_REASON_LIST_PLAN: &str =
    include_str!("../tests/fixtures/system-map/plan-fenced-fake-reason-list.md");
const README: &str = include_str!("../tests/fixtures/system-map/readme.md");
const UNKNOWN_NOUN_README: &str =
    include_str!("../tests/fixtures/system-map/readme-unknown-noun.md");
const GUIDE: &str = include_str!("../tests/fixtures/system-map/agent-guide.md");
const RETIRED_GUIDE: &str = include_str!("../tests/fixtures/system-map/agent-guide-retired.md");
const UNREGISTERED_GUIDE: &str =
    include_str!("../tests/fixtures/system-map/agent-guide-unregistered.md");
const TEMPLATE_GUIDE: &str = include_str!("../tests/fixtures/system-map/agent-guide-template.md");
const SOFA_GUIDE: &str = include_str!("../tests/fixtures/system-map/agent-guide-sofa.md");
const LITERAL_VERB_GUIDE: &str =
    include_str!("../tests/fixtures/system-map/agent-guide-literal-verb.md");
const MISMATCHED_FENCE_GUIDE: &str =
    include_str!("../tests/fixtures/system-map/agent-guide-mismatched-fence.md");
const SHORT_CLOSE_GUIDE: &str =
    include_str!("../tests/fixtures/system-map/agent-guide-short-close.md");
const SYSTEM_MAP_DOC: &str = include_str!("../tests/fixtures/system-map/system-map-doc.md");
const UNKNOWN_CONSTRUCTOR_DOC: &str =
    include_str!("../tests/fixtures/system-map/system-map-doc-unknown-constructor.md");
const UNKNOWN_VARIANT_LIST_DOC: &str =
    include_str!("../tests/fixtures/system-map/system-map-doc-unknown-variant-list.md");
const DOMAIN_CONSTRUCTOR_DOC: &str =
    include_str!("../tests/fixtures/system-map/system-map-doc-domain-constructor.md");
const KNOWN_WITH_DOMAIN_DATA_DOC: &str =
    include_str!("../tests/fixtures/system-map/system-map-doc-known-with-domain-data.md");
const ISSUES: &str = include_str!("../tests/fixtures/system-map/issues.jsonl");
const MISSING_LINK_ISSUES: &str =
    include_str!("../tests/fixtures/system-map/issues-missing-link.jsonl");
const MALFORMED_ISSUES: &str = include_str!("../tests/fixtures/system-map/issues-malformed.jsonl");
const DUPLICATE_LINK_ISSUES: &str =
    include_str!("../tests/fixtures/system-map/issues-duplicate-link.jsonl");
const TOMBSTONE_PLUS_LIVE_ISSUES: &str =
    include_str!("../tests/fixtures/system-map/issues-tombstone-plus-live.jsonl");

fn fixture_json(bytes: &[u8]) -> Json {
    parse(bytes, Limits::default()).expect("independent system-map fixture must be strict JSON")
}

fn report_with(
    system_map: &[u8],
    vocabulary: &[u8],
    readme: &str,
    agent_guide: &str,
    system_map_doc: &str,
    beads_issues: &str,
    plan: &str,
) -> Report {
    let system_map = fixture_json(system_map);
    let vocabulary = fixture_json(vocabulary);
    let invariants = fixture_json(INVARIANTS);
    let roadmap = fixture_json(ROADMAP);
    check(Inputs {
        system_map: &system_map,
        vocabulary: &vocabulary,
        invariants: &invariants,
        roadmap: &roadmap,
        readme,
        agent_guide,
        system_map_doc,
        beads_issues,
        plan,
    })
}

fn baseline() -> Report {
    report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    )
}

fn assert_finding(report: &Report, file: &str, id: Option<&str>, code: &str) {
    assert!(
        report.findings.iter().any(|finding| {
            finding.file == file && finding.id.as_deref() == id && finding.code == code
        }),
        "missing intended finding {file}/{id:?}/{code}; actual: {:#?}",
        report.findings
    );
}

fn assert_only_finding(report: &Report, file: &str, id: Option<&str>, code: &str) {
    assert_eq!(
        report.findings.len(),
        1,
        "one-field causal fixture unexpectedly produced: {:#?}",
        report.findings
    );
    assert_finding(report, file, id, code);
}

#[test]
fn permitted_fixture_accepts_cross_layer_access_payload_types_and_live_epic_link() {
    let report = baseline();
    assert!(report.is_clean(), "paired permitted fixture: {report:#?}");
    assert_eq!(report.counts.layers, 9);
    assert_eq!(report.counts.verbs, 3);
    assert_eq!(report.counts.packets, 2);
    assert_eq!(
        report.counts.readme_commands, 4,
        "one inline command, two table alternatives, and one fenced command"
    );
    assert_eq!(report.counts.guide_commands, 1);
}

#[test]
fn command_template_is_not_a_registered_command_claim() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        TEMPLATE_GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert!(
        report.is_clean(),
        "literal command template is not a command: {report:#?}"
    );
    assert_eq!(report.counts.guide_commands, 0);
}

#[test]
fn retired_guide_command_is_refused_at_the_guide_surface() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        RETIRED_GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "docs/AGENT_GUIDE.md",
        Some("status"),
        "retired_verb",
    );
}

#[test]
fn unknown_guide_command_is_not_accepted_merely_because_it_looks_like_an_fa_command() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        UNREGISTERED_GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "docs/AGENT_GUIDE.md",
        Some("fabricate"),
        "unregistered_verb",
    );
}

#[test]
fn concrete_address_kind_must_be_a_registered_noun() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        UNKNOWN_NOUN_README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(&report, "README.md", Some("ghost"), "unknown_noun");
}

#[test]
fn registered_verb_without_authority_has_a_missing_contract_finding() {
    let report = report_with(
        SYSTEM_MAP,
        MISSING_AUTHORITY_VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_finding(
        &report,
        "registry/vocabulary.json",
        Some("get"),
        "missing_verb_contract",
    );
}

#[test]
fn verb_class_must_be_one_of_the_registered_command_classes() {
    let report = report_with(
        SYSTEM_MAP,
        BAD_CLASS_VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "registry/vocabulary.json",
        Some("get"),
        "invalid_verb_class",
    );
}

#[test]
fn verb_ownership_layer_must_use_the_registered_range_or_list_grammar() {
    let report = report_with(
        SYSTEM_MAP,
        MALFORMED_LAYER_VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "registry/vocabulary.json",
        Some("get"),
        "invalid_layer_descriptor",
    );
}

#[test]
fn missing_layer_packet_is_an_unknown_roadmap_reference() {
    let report = report_with(
        MISSING_PACKET_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "registry/system_map.json",
        Some("FA-999"),
        "unknown_reference",
    );
}

#[test]
fn knowledge_payload_types_are_not_variants_but_unknown_constructor_is() {
    assert!(
        baseline().is_clean(),
        "Knowledge payload wrappers are not variants"
    );
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        UNKNOWN_CONSTRUCTOR_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "docs/SYSTEM_MAP.md",
        Some("Fabricated"),
        "unknown_knowledge_variant",
    );
}

#[test]
fn labeled_knowledge_variant_list_rejects_an_unknown_member() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        UNKNOWN_VARIANT_LIST_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "docs/SYSTEM_MAP.md",
        Some("Fabricated"),
        "unknown_knowledge_variant",
    );
}

#[test]
fn ordinary_domain_constructors_do_not_create_knowledge_variants() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        DOMAIN_CONSTRUCTOR_DOC,
        ISSUES,
        PLAN,
    );
    assert!(
        report.is_clean(),
        "a domain constructor is not an epistemic constructor: {report:#?}"
    );
}

#[test]
fn known_constructor_with_payload_and_domain_constructors_is_not_name_allowlisted() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        KNOWN_WITH_DOMAIN_DATA_DOC,
        ISSUES,
        PLAN,
    );
    assert!(
        report.is_clean(),
        "only the Known constructor is epistemic: {report:#?}"
    );
}

#[test]
fn exact_plan_reason_codes_must_be_registered() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        UNKNOWN_REASON_PLAN,
    );
    assert_only_finding(
        &report,
        "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md",
        Some("InventedReason"),
        "missing_reason_code",
    );
}

#[test]
fn reason_code_section_cannot_be_empty() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        EMPTY_REASON_LIST_PLAN,
    );
    assert_only_finding(
        &report,
        "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md",
        Some("§17.3"),
        "missing_reason_code",
    );
}

#[test]
fn fenced_reason_code_heading_cannot_substitute_for_the_normative_section() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        FENCED_FAKE_REASON_LIST_PLAN,
    );
    assert_only_finding(
        &report,
        "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md",
        Some("§17.3"),
        "unknown_reference",
    );
}

#[test]
fn missing_live_checker_epic_link_is_not_broadened_into_general_bead_coverage() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        MISSING_LINK_ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        ".beads/issues.jsonl",
        Some("FA-132"),
        "missing_bead_packet_reference",
    );
}

#[test]
fn malformed_bead_lines_are_refused_and_cannot_supply_the_required_live_link() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        MALFORMED_ISSUES,
        PLAN,
    );
    assert_finding(&report, ".beads/issues.jsonl", None, "invalid_json");
    assert_finding(
        &report,
        ".beads/issues.jsonl",
        Some("FA-132"),
        "missing_bead_packet_reference",
    );
}

#[test]
fn duplicate_live_checker_epic_links_are_refused() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        DUPLICATE_LINK_ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        ".beads/issues.jsonl",
        Some("FA-132"),
        "duplicate_bead_packet_reference",
    );
}

#[test]
fn tombstoned_packet_links_do_not_make_a_second_live_reference() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        TOMBSTONE_PLUS_LIVE_ISSUES,
        PLAN,
    );
    assert!(
        report.is_clean(),
        "tombstone is outside the live-link universe: {report:#?}"
    );
}

#[test]
fn sofa_is_not_an_fa_command_prefix() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        SOFA_GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert!(
        report.is_clean(),
        "ordinary sofa prose is not a command: {report:#?}"
    );
    assert_eq!(report.counts.guide_commands, 0);
}

#[test]
fn literal_fa_verb_is_a_real_unregistered_command_not_a_placeholder() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        LITERAL_VERB_GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "docs/AGENT_GUIDE.md",
        Some("verb"),
        "unregistered_verb",
    );
}

#[test]
fn mismatched_fence_marker_does_not_hide_a_retired_command() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        MISMATCHED_FENCE_GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "docs/AGENT_GUIDE.md",
        Some("status"),
        "retired_verb",
    );
}

#[test]
fn shorter_fence_run_does_not_hide_a_retired_command() {
    let report = report_with(
        SYSTEM_MAP,
        VOCABULARY,
        README,
        SHORT_CLOSE_GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "docs/AGENT_GUIDE.md",
        Some("status"),
        "retired_verb",
    );
}

#[test]
fn invalid_layer_identifier_cannot_substitute_for_a_required_layer() {
    let invalid_layer_map = String::from_utf8(SYSTEM_MAP.to_vec())
        .expect("fixture is UTF-8")
        .replacen(r#""id":"L8""#, r#""id":"L9""#, 1);
    let report = report_with(
        invalid_layer_map.as_bytes(),
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_finding(
        &report,
        "registry/system_map.json",
        Some("L9"),
        "invalid_id",
    );
    assert_finding(
        &report,
        "registry/system_map.json",
        Some("L8"),
        "missing_field",
    );
}

#[test]
fn unknown_normative_plan_section_is_not_accepted_by_numeric_prefix() {
    let unknown_section_map = String::from_utf8(SYSTEM_MAP.to_vec())
        .expect("fixture is UTF-8")
        .replacen(
            r#""normative_sections": ["§2"]"#,
            r#""normative_sections": ["§99.9"]"#,
            1,
        );
    let report = report_with(
        unknown_section_map.as_bytes(),
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "registry/system_map.json",
        Some("§99.9"),
        "unknown_reference",
    );
}

#[test]
fn object_may_belong_to_only_one_layer() {
    let duplicate_object_map = String::from_utf8(SYSTEM_MAP.to_vec())
        .expect("fixture is UTF-8")
        .replacen(r#""objects":["object8"]"#, r#""objects":["object0"]"#, 1);
    let report = report_with(
        duplicate_object_map.as_bytes(),
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(
        &report,
        "registry/system_map.json",
        Some("object0"),
        "duplicate_object",
    );
}

#[test]
fn layer_row_bound_refuses_oversized_map_before_semantic_traversal() {
    let layer = r#"{"id":"L0"}"#;
    let layers = std::iter::repeat_n(layer, 4_097)
        .collect::<Vec<_>>()
        .join(",");
    let oversized = format!(
        r#"{{"normative_sections":["§2"],"layers":[{layers}],"epistemic_type":{{"variants":["Known","Pending","Unknown","Withheld","Stale","Absent"]}},"checker_packet":"FA-132"}}"#
    );
    let report = report_with(
        oversized.as_bytes(),
        VOCABULARY,
        README,
        GUIDE,
        SYSTEM_MAP_DOC,
        ISSUES,
        PLAN,
    );
    assert_only_finding(&report, "registry/system_map.json", None, "input_too_large");
}
