//! Independently authored v0.1 canonical JSON golden vectors.
//!
//! The fixture bytes below come from the retained schemas and canonical-profile
//! rules, not from `encode`. They cover draft interoperability shapes only, not
//! authentication, durable records, or authority.

use fa_reference::canonical_json::{
    CanonicalError, DraftDocument, SchemaVariant, decode_canonical, encode, validate_json,
};
use fa_reference::strict_json::{Json, Limits, parse};

const ACTION: &[u8] = include_bytes!("fixtures/canonical-json-v01/action.json");
const CAPABILITY: &[u8] = include_bytes!("fixtures/canonical-json-v01/capability.json");
const EVIDENCE: &[u8] = include_bytes!("fixtures/canonical-json-v01/evidence.json");

// This is another manually written valid canonical action document. It differs
// only in a schema-permitted semantic value, so it must validate rather than
// masquerade as a parser rejection or a golden update.
const DIFFERENT_ACTION: &[u8] = br#"{"action_id":"action-01","adapter_id":"adapter/main","branch":"sandbox","effect_kind":"publish","payload_bytes":17,"payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","policy_epoch":9,"required_observations":["obs-a","obs-b"],"run_id":"run-01","schema_version":"fa.action/0.1","target_binding":"target/v1","tenant_id":"tenant-01","units":4}"#;

fn parse_syntax(bytes: &[u8]) -> Json {
    parse(bytes, Limits::default()).expect("fixture must be strict JSON syntax")
}

fn assert_manual_golden(bytes: &[u8], expected_variant: SchemaVariant) -> DraftDocument {
    assert_eq!(
        bytes.last(),
        Some(&b'}'),
        "golden has no trailing whitespace"
    );
    let parsed = parse_syntax(bytes);
    let document = validate_json(&parsed).expect("fixture must match its v0.1 schema");
    assert_eq!(document.schema_variant(), expected_variant);
    assert_eq!(
        encode(&document),
        bytes,
        "encoder drifted from manual golden"
    );
    assert_eq!(
        decode_canonical(bytes).expect("manual golden must be canonical"),
        document,
        "canonical decoding changed the typed result"
    );
    document
}

#[test]
fn action_v01_manual_golden_is_exact_across_public_boundaries() {
    let document = assert_manual_golden(ACTION, SchemaVariant::ActionProposalV01);
    assert!(matches!(document, DraftDocument::ActionProposal(_)));
}

#[test]
fn capability_v01_manual_golden_preserves_literal_unicode_and_short_escapes() {
    let document = assert_manual_golden(CAPABILITY, SchemaVariant::CapabilityManifestV01);
    assert!(
        CAPABILITY
            .windows("café".len())
            .any(|bytes| bytes == "café".as_bytes())
    );
    assert!(
        CAPABILITY
            .windows(br#"\""#.len())
            .any(|bytes| bytes == br#"\""#)
    );
    assert!(CAPABILITY.windows(2).any(|bytes| bytes == br#"\\"#));
    assert!(matches!(document, DraftDocument::CapabilityManifest(_)));
}

#[test]
fn evidence_v01_manual_golden_preserves_literal_unicode_without_normalization() {
    let document = assert_manual_golden(EVIDENCE, SchemaVariant::EvidenceClaimV01);
    assert!(
        EVIDENCE
            .windows("café".len())
            .any(|bytes| bytes == "café".as_bytes())
    );
    assert!(
        EVIDENCE
            .windows(" ".len())
            .any(|bytes| bytes == " ".as_bytes())
    );
    assert!(matches!(document, DraftDocument::EvidenceClaim(_)));
}

#[test]
fn canonical_and_semantic_golden_mismatches_have_different_observables() {
    let expected = assert_manual_golden(ACTION, SchemaVariant::ActionProposalV01);
    let different = validate_json(&parse_syntax(DIFFERENT_ACTION))
        .expect("schema-permitted different document remains valid");
    assert_ne!(
        different, expected,
        "different canonical document replaced the fixture"
    );
    assert_eq!(encode(&different), DIFFERENT_ACTION);
    assert_eq!(decode_canonical(DIFFERENT_ACTION).unwrap(), different);

    let mut whitespace_equivalent = ACTION.to_vec();
    whitespace_equivalent.insert(1, b' ');
    assert_eq!(
        validate_json(&parse_syntax(&whitespace_equivalent)).unwrap(),
        expected,
        "whitespace does not change parsed schema semantics"
    );
    assert_eq!(
        decode_canonical(&whitespace_equivalent),
        Err(CanonicalError::NonCanonical),
        "canonical mismatch must not be silently normalized"
    );
}
