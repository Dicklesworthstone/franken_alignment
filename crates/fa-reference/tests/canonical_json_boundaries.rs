//! Public boundary tests for the FA-005 canonical draft-document format.
//!
//! These are byte-level tests of the bounded reference format only. A passing
//! decode neither authenticates a record nor creates authority, persistence,
//! or a live effect.

use std::collections::BTreeMap;

use fa_reference::canonical_json::{
    CanonicalError, MAX_DOCUMENT_BYTES, SchemaVariant, decode_canonical, encode, validate_json,
};
use fa_reference::strict_json::{Json, Limits, parse};

const ACTION: &[u8] = br#"{"action_id":"act-1","adapter_id":"adapter-1","branch":"production","effect_kind":"publish","payload_bytes":0,"payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","policy_epoch":1,"required_observations":["obs-1"],"run_id":"run-1","schema_version":"fa.action/0.1","target_binding":"target-1","tenant_id":"tenant-1","units":1}"#;
const MANIFEST: &[u8] = br#"{"activation_capture":false,"adapter_id":"adapter-1","effect_families":["publish"],"limitations":["bounded profile"],"mediation_mode":"observe_only","restart_grades":["unavailable"],"schema_version":"fa.capabilities/0.1","status":"example_only"}"#;
const CLAIM: &[u8] = br#"{"assertion":"line\n","assumptions":["bounded"],"claim_class":"bounded_model","claim_id":"claim-1","evidence_refs":["0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"],"limitations":["no authority"],"schema_version":"fa.claim/0.1","scope":"tenant-1","source_refs":["source-1"],"status":"proposed"}"#;

fn limits() -> Limits {
    Limits {
        max_bytes: MAX_DOCUMENT_BYTES,
        max_depth: 8,
        max_items: 1_024,
        max_string_bytes: 8_192,
    }
}

fn parse_valid(bytes: &[u8]) -> Json {
    parse(bytes, limits()).expect("fixed valid control must strictly parse")
}

fn replace_once(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let source = String::from_utf8(bytes.to_vec()).expect("fixed controls are UTF-8");
    assert!(source.contains(from), "mutation anchor must exist: {from}");
    source.replacen(from, to, 1).into_bytes()
}

fn object_mut(value: &mut Json) -> &mut BTreeMap<String, Json> {
    match value {
        Json::Object(object) => object,
        other => panic!("fixed document root must be an object, got {other:?}"),
    }
}

fn string_array(values: impl IntoIterator<Item = String>) -> Json {
    Json::Array(values.into_iter().map(Json::String).collect())
}

fn replace_field(value: &mut Json, field: &str, replacement: Json) {
    assert!(
        object_mut(value)
            .insert(field.to_owned(), replacement)
            .is_some(),
        "fixed control must contain {field}"
    );
}

fn maximum_text(characters: usize) -> String {
    assert!(characters > 0, "schema text maxima are positive");
    let mut text = "😀".repeat(characters - 1);
    text.push('\u{0001}');
    assert_eq!(text.chars().count(), characters);
    text
}

#[test]
fn three_fixed_canonical_schema_controls_decode_validate_and_reencode_exactly() {
    for (bytes, variant) in [
        (ACTION, SchemaVariant::ActionProposalV01),
        (MANIFEST, SchemaVariant::CapabilityManifestV01),
        (CLAIM, SchemaVariant::EvidenceClaimV01),
    ] {
        let document = decode_canonical(bytes).expect("fixed canonical control must decode");
        assert_eq!(document.schema_variant(), variant);
        assert_eq!(validate_json(&parse_valid(bytes)), Ok(document.clone()));
        assert_eq!(encode(&document), bytes);
        assert!(!bytes.ends_with(b"\n"));
        assert!(!bytes.windows(2).any(|window| window == b", "));
    }
}

#[test]
fn duplicate_unknown_and_missing_fields_each_refuse_without_reinterpretation() {
    assert!(decode_canonical(ACTION).is_ok());
    assert_eq!(
        decode_canonical(b"{}"),
        Err(CanonicalError::MissingField("schema_version"))
    );
    let wrong_version_type = replace_once(
        ACTION,
        "\"schema_version\":\"fa.action/0.1\"",
        "\"schema_version\":123",
    );
    assert_eq!(
        decode_canonical(&wrong_version_type),
        Err(CanonicalError::WrongType("schema_version"))
    );
    let duplicate = br#"{"schema_version":"fa.action/0.1","schema_version":"fa.action/0.1"}"#;
    assert_eq!(
        decode_canonical(duplicate),
        Err(CanonicalError::DuplicateKey("schema_version".to_owned()))
    );

    let unknown = replace_once(ACTION, "}", ",\"unknown\":true}");
    assert_eq!(
        decode_canonical(&unknown),
        Err(CanonicalError::UnknownCriticalField("unknown".to_owned()))
    );

    assert_eq!(
        decode_canonical(br#"{"schema_version":"fa.action/0.1"}"#),
        Err(CanonicalError::MissingField("action_id"))
    );
}

#[test]
fn canonical_bytes_refuse_whitespace_reordered_keys_and_nonshortest_escapes() {
    let leading_space = [b" ".as_slice(), ACTION].concat();
    assert_eq!(
        decode_canonical(&leading_space),
        Err(CanonicalError::NonCanonical)
    );

    let trailing_newline = [ACTION, b"\n".as_slice()].concat();
    assert_eq!(
        decode_canonical(&trailing_newline),
        Err(CanonicalError::NonCanonical)
    );

    let reordered = replace_once(
        ACTION,
        "{\"action_id\":\"act-1\",\"adapter_id\":\"adapter-1\",",
        "{\"adapter_id\":\"adapter-1\",\"action_id\":\"act-1\",",
    );
    assert_eq!(
        decode_canonical(&reordered),
        Err(CanonicalError::NonCanonical)
    );

    let optional_escape = replace_once(CLAIM, "\"bounded\"", r#""\u0062ounded""#);
    assert_eq!(
        decode_canonical(&optional_escape),
        Err(CanonicalError::NonCanonical)
    );

    let long_control_escape = replace_once(CLAIM, r#""line\n""#, r#""line\u000a""#);
    assert_eq!(
        decode_canonical(&long_control_escape),
        Err(CanonicalError::NonCanonical)
    );
}

#[test]
fn literals_remain_unicode_without_normalization_and_text_limits_use_characters() {
    let mut composed_json = parse_valid(CLAIM);
    replace_field(
        &mut composed_json,
        "assertion",
        Json::String("é".to_owned()),
    );
    let composed = validate_json(&composed_json).expect("composed literal is a valid claim");
    let composed_bytes = encode(&composed);

    let mut decomposed_json = parse_valid(CLAIM);
    replace_field(
        &mut decomposed_json,
        "assertion",
        Json::String("e\u{301}".to_owned()),
    );
    let decomposed = validate_json(&decomposed_json).expect("decomposed literal is a valid claim");
    let decomposed_bytes = encode(&decomposed);

    assert_ne!(composed_bytes, decomposed_bytes);
    assert_eq!(decode_canonical(&composed_bytes), Ok(composed));
    assert_eq!(decode_canonical(&decomposed_bytes), Ok(decomposed));
    assert!(
        composed_bytes
            .windows("é".len())
            .any(|window| window == "é".as_bytes())
    );

    let mut at_limit_json = parse_valid(CLAIM);
    replace_field(
        &mut at_limit_json,
        "assertion",
        Json::String(maximum_text(2_048)),
    );
    let at_limit = validate_json(&at_limit_json).expect("2,048 Unicode scalars are valid");
    let at_limit_bytes = encode(&at_limit);
    assert!(
        at_limit_bytes
            .windows("😀".len())
            .any(|window| window == "😀".as_bytes())
    );
    assert!(at_limit_bytes.windows(6).any(|window| window == b"\\u0001"));
    assert_eq!(decode_canonical(&at_limit_bytes), Ok(at_limit));

    let mut above_limit = parse_valid(CLAIM);
    replace_field(
        &mut above_limit,
        "assertion",
        Json::String(format!("{}x", maximum_text(2_048))),
    );
    assert_eq!(
        validate_json(&above_limit),
        Err(CanonicalError::InvalidValue("assertion"))
    );
}

#[test]
fn unsigned_decimal_integer_forms_overflow_and_schema_ranges_refuse_causally() {
    for lexeme in ["1e0", "1.0", "-1"] {
        let non_integer = replace_once(ACTION, "\"units\":1", &format!("\"units\":{lexeme}"));
        assert_eq!(
            decode_canonical(&non_integer),
            Err(CanonicalError::InvalidValue("units")),
            "{lexeme} must not become an admitted unsigned integer"
        );
    }

    let overflow = replace_once(ACTION, "\"units\":1", "\"units\":18446744073709551616");
    assert_eq!(
        decode_canonical(&overflow),
        Err(CanonicalError::IntegerOverflow("units"))
    );

    let out_of_range = replace_once(ACTION, "\"payload_bytes\":0", "\"payload_bytes\":65537");
    assert_eq!(
        decode_canonical(&out_of_range),
        Err(CanonicalError::IntegerOutOfRange("payload_bytes"))
    );
}

#[test]
fn truncated_and_schema_flagged_duplicate_arrays_refuse_while_order_and_unflagged_duplicates_survive()
 {
    assert_eq!(
        decode_canonical(b"{\"schema_version\":"),
        Err(CanonicalError::Truncated)
    );

    let duplicate_required = replace_once(ACTION, "[\"obs-1\"]", "[\"obs-1\",\"obs-1\"]");
    assert_eq!(
        decode_canonical(&duplicate_required),
        Err(CanonicalError::DuplicateArrayValue("required_observations"))
    );

    let ordered = replace_once(ACTION, "[\"obs-1\"]", "[\"obs-b\",\"obs-a\"]");
    let ordered_document =
        decode_canonical(&ordered).expect("unique arrays preserve declared order");
    assert_eq!(encode(&ordered_document), ordered);

    let duplicate_limitation = replace_once(CLAIM, "[\"no authority\"]", "[\"same\",\"same\"]");
    let duplicate_limitation_document =
        decode_canonical(&duplicate_limitation).expect("limitations are not schema-unique");
    assert_eq!(encode(&duplicate_limitation_document), duplicate_limitation);

    let duplicate_grade = replace_once(
        MANIFEST,
        "[\"unavailable\"]",
        "[\"unavailable\",\"unavailable\"]",
    );
    assert_eq!(
        decode_canonical(&duplicate_grade),
        Err(CanonicalError::DuplicateArrayValue("restart_grades"))
    );
}

#[test]
fn schema_arrays_without_a_minimum_allow_empty_but_required_arrays_still_do_not() {
    let mut action = parse_valid(ACTION);
    replace_field(
        &mut action,
        "required_observations",
        Json::Array(Vec::new()),
    );
    let action = validate_json(&action).expect("action observations have no schema minimum");
    assert_eq!(decode_canonical(&encode(&action)), Ok(action));

    let mut manifest = parse_valid(MANIFEST);
    replace_field(&mut manifest, "effect_families", Json::Array(Vec::new()));
    let manifest = validate_json(&manifest).expect("effect families have no schema minimum");
    assert_eq!(decode_canonical(&encode(&manifest)), Ok(manifest));

    let mut claim = parse_valid(CLAIM);
    replace_field(&mut claim, "source_refs", Json::Array(Vec::new()));
    let claim = validate_json(&claim).expect("source references have no schema minimum");
    assert_eq!(decode_canonical(&encode(&claim)), Ok(claim));

    let mut empty_required = parse_valid(MANIFEST);
    replace_field(
        &mut empty_required,
        "restart_grades",
        Json::Array(Vec::new()),
    );
    assert_eq!(
        validate_json(&empty_required),
        Err(CanonicalError::IntegerOutOfRange("restart_grades"))
    );
}

#[test]
fn parser_document_depth_item_and_string_bounds_refuse_before_schema_admission() {
    let exact_string = replace_once(CLAIM, r#""line\n""#, &format!("\"{}\"", "😀".repeat(2_048)));
    assert!(decode_canonical(&exact_string).is_ok());
    let over_string = replace_once(CLAIM, r#""line\n""#, &format!("\"{}\"", "😀".repeat(2_049)));
    assert_eq!(
        decode_canonical(&over_string),
        Err(CanonicalError::InputLimit)
    );
    let too_large = vec![b' '; MAX_DOCUMENT_BYTES + 1];
    assert_eq!(
        decode_canonical(&too_large),
        Err(CanonicalError::InputLimit)
    );

    let mut too_deep = vec![b'['; 9];
    too_deep.push(b'0');
    too_deep.extend(std::iter::repeat_n(b']', 9));
    assert_eq!(decode_canonical(&too_deep), Err(CanonicalError::InputLimit));

    let mut too_many_items = String::from("[");
    for item in 0..1_025 {
        if item != 0 {
            too_many_items.push(',');
        }
        too_many_items.push('0');
    }
    too_many_items.push(']');
    assert_eq!(
        decode_canonical(too_many_items.as_bytes()),
        Err(CanonicalError::InputLimit)
    );

    let too_long_string = format!("\"{}\"", "x".repeat(8_193));
    assert_eq!(
        decode_canonical(too_long_string.as_bytes()),
        Err(CanonicalError::InputLimit)
    );
}

#[test]
fn maximum_schema_forms_fit_inside_parser_bounds_and_exceeding_schema_counts_refuses() {
    let mut action_json = parse_valid(ACTION);
    replace_field(
        &mut action_json,
        "required_observations",
        string_array((0..16).map(|index| format!("obs-{index:02}"))),
    );
    let action_at_limit = validate_json(&action_json).expect("16 observations are valid");
    let action_bytes = encode(&action_at_limit);
    assert_eq!(decode_canonical(&action_bytes), Ok(action_at_limit));
    replace_field(
        &mut action_json,
        "required_observations",
        string_array((0..17).map(|index| format!("obs-{index:02}"))),
    );
    assert_eq!(
        validate_json(&action_json),
        Err(CanonicalError::IntegerOutOfRange("required_observations"))
    );

    let mut manifest_json = parse_valid(MANIFEST);
    replace_field(
        &mut manifest_json,
        "effect_families",
        string_array((0..128).map(|index| format!("family-{index:03}"))),
    );
    let manifest_at_limit = validate_json(&manifest_json).expect("128 families are valid");
    let manifest_bytes = encode(&manifest_at_limit);
    assert_eq!(decode_canonical(&manifest_bytes), Ok(manifest_at_limit));
    replace_field(
        &mut manifest_json,
        "effect_families",
        string_array((0..129).map(|index| format!("family-{index:03}"))),
    );
    assert_eq!(
        validate_json(&manifest_json),
        Err(CanonicalError::IntegerOutOfRange("effect_families"))
    );

    let mut claim_json = parse_valid(CLAIM);
    replace_field(
        &mut claim_json,
        "assertion",
        Json::String(maximum_text(2_048)),
    );
    replace_field(&mut claim_json, "scope", Json::String(maximum_text(2_048)));
    replace_field(
        &mut claim_json,
        "assumptions",
        string_array((0..32).map(|index| format!("{index:04}{}", maximum_text(1_020)))),
    );
    replace_field(
        &mut claim_json,
        "limitations",
        string_array((0..32).map(|index| format!("{index:04}{}", maximum_text(1_020)))),
    );
    replace_field(
        &mut claim_json,
        "source_refs",
        string_array((0..64).map(|index| format!("source-{index:03}-{}", "a".repeat(117)))),
    );
    replace_field(
        &mut claim_json,
        "evidence_refs",
        string_array((0..64).map(|index| format!("{index:02x}{}", "a".repeat(62)))),
    );
    let maximal_claim = validate_json(&claim_json).expect("combined schema maxima are valid");
    let maximal_claim_bytes = encode(&maximal_claim);
    assert!(maximal_claim_bytes.len() <= MAX_DOCUMENT_BYTES);
    assert_eq!(decode_canonical(&maximal_claim_bytes), Ok(maximal_claim));
}
