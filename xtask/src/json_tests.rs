//! Independent conformance tests for the bounded xtask JSON input reader.

use crate::json::{ErrorKind, Json, Limits, parse};

// Captured by RedCastle on 2026-09-07 with:
// `cargo metadata --format-version 1 --locked --offline` (exit 0).
// Machine-specific workspace and target prefixes are normalized to `/workspace`;
// no package, target, dependency, or metadata shape was invented or removed.
const CURRENT_METADATA: &[u8] = include_bytes!("../tests/fixtures/metadata-current.json");
const VOCABULARY_REGISTRY: &[u8] = include_bytes!("../../registry/vocabulary.json");

fn limits() -> Limits {
    Limits {
        max_bytes: 16 * 1024,
        max_depth: 32,
        max_items: 4 * 1024,
        max_string_bytes: 4 * 1024,
    }
}

fn parse_positive(input: &[u8]) -> Json {
    parse(input, limits()).expect("independent positive corpus must parse")
}

fn object_field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value
        .get(name)
        .unwrap_or_else(|| panic!("missing object member {name:?}"))
}

fn as_array(value: &Json) -> &[Json] {
    value.as_array().expect("expected array")
}

fn as_string(value: &Json) -> &str {
    value.as_str().expect("expected string")
}

fn assert_error(input: &[u8], expected: ErrorKind) {
    let error = parse(input, limits()).expect_err("malformed JSON must be refused");
    assert_eq!(error.kind, expected, "input: {input:?}");
}

fn nested_empty_arrays(depth: usize) -> Vec<u8> {
    let mut json = Vec::with_capacity(depth.saturating_mul(2));
    json.extend(std::iter::repeat_n(b'[', depth));
    json.extend(std::iter::repeat_n(b']', depth));
    json
}

#[test]
fn parses_normalized_real_cargo_metadata_without_losing_structure() {
    let metadata = parse_positive(CURRENT_METADATA);
    let packages = as_array(object_field(&metadata, "packages"));
    assert_eq!(packages.len(), 2);
    assert_eq!(
        as_string(object_field(&packages[0], "name")),
        "fa-reference"
    );
    assert_eq!(as_string(object_field(&packages[1], "name")), "xtask");
    assert_eq!(
        as_string(object_field(&metadata, "workspace_root")),
        "/workspace/franken_alignment"
    );
    assert_eq!(
        as_array(object_field(object_field(&metadata, "resolve"), "nodes")).len(),
        2
    );
    assert_eq!(object_field(&metadata, "version").as_u64(), Some(1));
}

#[test]
fn parses_the_real_vocabulary_registry_and_its_registered_nouns() {
    let vocabulary = parse_positive(VOCABULARY_REGISTRY);
    assert_eq!(
        as_string(object_field(&vocabulary, "address_scheme")),
        "fa://<tenant>/<kind>/<id>[@<generation>]"
    );
    let nouns = as_array(object_field(&vocabulary, "nouns"));
    assert_eq!(nouns.len(), 26);
    assert_eq!(as_string(&nouns[0]), "run");
    assert_eq!(as_string(&nouns[25]), "handoff");
}

#[test]
fn preserves_unicode_and_exact_number_lexemes() {
    let value = parse_positive(br#"{"note":"\uD834\uDD1E","number":-12.50e+3}"#);
    assert_eq!(as_string(object_field(&value, "note")), "𝄞");
    assert_eq!(object_field(&value, "number").kind(), "number");
    match object_field(&value, "number") {
        Json::Number(number) => assert_eq!(number.lexeme(), "-12.50e+3"),
        _ => panic!("expected number"),
    }
}

#[test]
fn duplicate_key_is_rejected_before_a_value_can_be_overwritten() {
    assert_error(
        br#"{"scope":"tenant-a","scope":"tenant-b"}"#,
        ErrorKind::DuplicateKey("scope".into()),
    );
    assert_error(
        br#"{"a":1,"\u0061":2}"#,
        ErrorKind::DuplicateKey("a".into()),
    );
}

#[test]
fn malformed_input_is_rejected_at_its_named_boundary() {
    assert_error(b"true false", ErrorKind::TrailingValue);
    assert_error(&[b'"', 0xff, b'"'], ErrorKind::InvalidUtf8);
    assert_error(br#""\uD834x""#, ErrorKind::LoneSurrogate);
    assert_error(br#""\u12x4""#, ErrorKind::InvalidEscape);
    assert_error(b"\"line\x1fbreak\"", ErrorKind::ControlCharacter);

    for number in [
        b"01".as_slice(),
        b"1.".as_slice(),
        b".1".as_slice(),
        b"+1".as_slice(),
        b"NaN".as_slice(),
        b"Infinity".as_slice(),
    ] {
        assert_error(number, ErrorKind::InvalidNumber);
    }
}

#[test]
fn depth_and_input_limits_refuse_before_unbounded_work() {
    let depth_limited = Limits {
        max_bytes: 64,
        max_depth: 2,
        max_items: 64,
        max_string_bytes: 64,
    };
    let error = parse(b"[[[]]]", depth_limited).expect_err("nesting above limit must refuse");
    assert_eq!(error.kind, ErrorKind::DepthLimit);

    let size_limited = Limits {
        max_bytes: 3,
        max_depth: 8,
        max_items: 64,
        max_string_bytes: 64,
    };
    let error = parse(b"true", size_limited).expect_err("input above byte cap must refuse");
    assert_eq!(error.kind, ErrorKind::SizeLimit);
}

#[test]
fn absolute_recursion_ceiling_survives_an_oversized_caller_limit() {
    let caller_requests_unbounded_depth = Limits {
        max_bytes: 1024,
        max_depth: usize::MAX,
        max_items: 256,
        max_string_bytes: 64,
    };
    let at_hard_limit = nested_empty_arrays(128);
    parse(&at_hard_limit, caller_requests_unbounded_depth)
        .expect("the documented hard depth boundary itself must remain usable");

    let beyond_hard_limit = nested_empty_arrays(129);
    let error = parse(&beyond_hard_limit, caller_requests_unbounded_depth)
        .expect_err("an oversized caller limit must not disable the parser recursion ceiling");
    assert_eq!(error.kind, ErrorKind::DepthLimit);
}
