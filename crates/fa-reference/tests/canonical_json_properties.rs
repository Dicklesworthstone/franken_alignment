//! Fixed-seed, bounded FA-005 property coverage for all three retained schemas.
//!
//! This is deliberate generation, not a random search.  Every expected
//! acceptance/refusal is derived from the original schema clauses, never from
//! encoder output.

use std::collections::BTreeMap;

use fa_reference::canonical_json::{
    CanonicalError, SchemaVariant, decode_canonical, encode, validate_json,
};
use fa_reference::strict_json::{Json, Limits, parse};

const CASES_PER_SCHEMA: usize = 128;
const FIXED_SEEDS: [u64; 8] = [
    0x0000_0000_0000_0001,
    0x0123_4567_89ab_cdef,
    0xfedc_ba98_7654_3210,
    0x9e37_79b9_7f4a_7c15,
    0xd1b5_4a32_d192_ed03,
    0xa5a5_a5a5_5a5a_5a5a,
    0x6a09_e667_f3bc_c909,
    0xbb67_ae85_84ca_a73b,
];
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

fn limits() -> Limits {
    Limits {
        max_bytes: 512 * 1024,
        max_depth: 8,
        max_items: 1_024,
        max_string_bytes: 8_192,
    }
}

fn number(value: u64) -> Json {
    let text = value.to_string();
    parse(text.as_bytes(), limits()).expect("bounded decimal control must parse")
}

fn object(entries: impl IntoIterator<Item = (&'static str, Json)>) -> Json {
    Json::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn strings(values: impl IntoIterator<Item = String>) -> Json {
    Json::Array(values.into_iter().map(Json::String).collect())
}

fn object_mut(value: &mut Json) -> &mut BTreeMap<String, Json> {
    match value {
        Json::Object(fields) => fields,
        other => panic!("generated root must be an object, got {:?}", other),
    }
}

fn replace_field(value: &mut Json, key: &str, replacement: Json) {
    assert!(
        object_mut(value)
            .insert(key.to_owned(), replacement)
            .is_some(),
        "generated document must contain {}",
        key
    );
}

fn remove_field(value: &mut Json, key: &str) {
    assert!(
        object_mut(value).remove(key).is_some(),
        "generated document must contain {}",
        key
    );
}

fn seed(case: usize) -> u64 {
    FIXED_SEEDS[case % FIXED_SEEDS.len()].rotate_left((case % 64) as u32)
        ^ (case as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn id(prefix: &str, case: usize, lane: usize) -> String {
    format!(
        "{}-{:03}-{:02x}-{:016x}",
        prefix,
        case,
        lane,
        seed(case).rotate_left(lane as u32)
    )
}

fn digest(case: usize, lane: usize) -> String {
    format!("{:064x}", seed(case).wrapping_add(lane as u64))
}

fn text(case: usize, lane: usize) -> String {
    match (case + lane) % 8 {
        0 => format!("plain-{}-{}", case, lane),
        1 => format!("combining-e\u{301}-{}-{}", case, lane),
        2 => format!("multibyte-😀-{}-{}", case, lane),
        3 => format!("control-\u{0001}-{}-{}", case, lane),
        4 => format!("line\n-{}-{}", case, lane),
        5 => format!("separator-\u{2028}-{}-{}", case, lane),
        6 => format!("cjk-漢字-{}-{}", case, lane),
        _ => format!("accent-é-{}-{}", case, lane),
    }
}

fn ids(prefix: &str, case: usize, count: usize) -> Vec<String> {
    (0..count).map(|lane| id(prefix, case, lane)).collect()
}

fn action(case: usize) -> Json {
    let payload_bytes = match case % 5 {
        0 => 0,
        1 => 1,
        2 => 65_535,
        3 => 65_536,
        _ => seed(case) % 65_537,
    };
    let units = match case % 4 {
        0 => 1,
        1 => 2,
        2 => MAX_SAFE_INTEGER - 1,
        _ => MAX_SAFE_INTEGER,
    };
    let observations = match case % 4 {
        0 => Vec::new(),
        1 => ids("obs", case, 1),
        2 => ids("obs", case, 2),
        _ => ids("obs", case, 16),
    };
    object([
        ("schema_version", Json::String("fa.action/0.1".to_owned())),
        ("action_id", Json::String(id("action", case, 0))),
        ("tenant_id", Json::String(id("tenant", case, 1))),
        ("run_id", Json::String(id("run", case, 2))),
        (
            "branch",
            Json::String(
                if case.is_multiple_of(2) {
                    "production"
                } else {
                    "sandbox"
                }
                .to_owned(),
            ),
        ),
        ("effect_kind", Json::String(id("effect", case, 3))),
        ("adapter_id", Json::String(id("adapter", case, 4))),
        ("target_binding", Json::String(id("target", case, 5))),
        ("payload_sha256", Json::String(digest(case, 0))),
        ("payload_bytes", number(payload_bytes)),
        ("units", number(units)),
        (
            "policy_epoch",
            number(if case.is_multiple_of(2) {
                1
            } else {
                MAX_SAFE_INTEGER
            }),
        ),
        ("required_observations", strings(observations)),
    ])
}

fn manifest(case: usize) -> Json {
    const GRADES: [&str; 5] = [
        "unavailable",
        "decision_replay",
        "trace_replay",
        "functional_restart",
        "exact_restart_profile",
    ];
    const MODES: [&str; 4] = [
        "observe_only",
        "cooperative_gate",
        "brokered_effects",
        "attested_profile",
    ];
    let count = 1 + case % GRADES.len();
    let grades = (0..count)
        .map(|index| GRADES[(case + index) % GRADES.len()].to_owned())
        .collect::<Vec<_>>();
    let families = match case % 4 {
        0 => Vec::new(),
        1 => ids("family", case, 1),
        2 => ids("family", case, 2),
        _ => ids("family", case, 128),
    };
    object([
        (
            "schema_version",
            Json::String("fa.capabilities/0.1".to_owned()),
        ),
        ("adapter_id", Json::String(id("adapter", case, 0))),
        (
            "status",
            Json::String(
                if case.is_multiple_of(2) {
                    "example_only"
                } else {
                    "tested_profile"
                }
                .to_owned(),
            ),
        ),
        (
            "mediation_mode",
            Json::String(MODES[case % MODES.len()].to_owned()),
        ),
        ("activation_capture", Json::Bool(case.is_multiple_of(2))),
        ("restart_grades", strings(grades)),
        ("effect_families", strings(families)),
        (
            "limitations",
            strings((0..(1 + case % 4)).map(|lane| text(case, lane))),
        ),
    ])
}

fn claim(case: usize) -> Json {
    const CLASSES: [&str; 7] = [
        "invariant",
        "proof",
        "bounded_model",
        "statistical",
        "benchmark",
        "slo",
        "hypothesis",
    ];
    let count = 1 + case % 4;
    let sources = match case % 4 {
        0 => Vec::new(),
        1 => ids("source", case, 1),
        2 => ids("source", case, 2),
        _ => ids("source", case, 64),
    };
    let evidence = match case % 3 {
        0 => Vec::new(),
        1 => vec![digest(case, 0)],
        _ => vec![digest(case, 0), digest(case, 1)],
    };
    object([
        ("schema_version", Json::String("fa.claim/0.1".to_owned())),
        ("claim_id", Json::String(id("claim", case, 0))),
        (
            "claim_class",
            Json::String(CLASSES[case % CLASSES.len()].to_owned()),
        ),
        (
            "status",
            Json::String(
                if case.is_multiple_of(2) {
                    "proposed"
                } else {
                    "observed_in_declared_scope"
                }
                .to_owned(),
            ),
        ),
        ("assertion", Json::String(text(case, 0))),
        ("scope", Json::String(text(case, 1))),
        (
            "assumptions",
            strings((0..count).map(|lane| text(case, lane + 2))),
        ),
        (
            "limitations",
            strings((0..count).map(|lane| text(case, lane + 6))),
        ),
        ("source_refs", strings(sources)),
        ("evidence_refs", strings(evidence)),
    ])
}

/// Independently inspect the flat schema-object byte surface.  Values may be
/// arrays or strings, but schema keys are ASCII and must be literal, ordered
/// UTF-8 bytes.  This deliberately does not reuse the format parser/encoder.
fn assert_canonical_object_surface(bytes: &[u8]) {
    assert!(bytes.starts_with(b"{") && bytes.ends_with(b"}"));
    let mut keys = Vec::<Vec<u8>>::new();
    let mut depth = 0usize;
    let mut expecting_key = false;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            panic!("canonical bytes contain whitespace outside a string at {index}");
        }
        match byte {
            b'{' | b'[' => {
                depth += 1;
                expecting_key = depth == 1;
                index += 1;
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1).expect("balanced canonical containers");
                index += 1;
            }
            b',' if depth == 1 => {
                expecting_key = true;
                index += 1;
            }
            b':' if depth == 1 => {
                expecting_key = false;
                index += 1;
            }
            b'"' => {
                let start = index + 1;
                index += 1;
                let mut escaped = false;
                while index < bytes.len() {
                    match bytes[index] {
                        b'\\' if !escaped => escaped = true,
                        b'"' if !escaped => break,
                        _ => escaped = false,
                    }
                    index += 1;
                }
                assert!(index < bytes.len(), "canonical string must close");
                if depth == 1 && expecting_key {
                    let key = &bytes[start..index];
                    assert!(!key.contains(&b'\\'), "schema key must be literal UTF-8");
                    keys.push(key.to_vec());
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    assert_eq!(depth, 0, "canonical containers must balance");
    assert!(
        keys.windows(2).all(|pair| pair[0] < pair[1]),
        "top-level schema keys must be ascending UTF-8"
    );
}

fn round_trip(case: usize, expected: SchemaVariant, syntax: Json) -> Vec<u8> {
    let document = validate_json(&syntax)
        .unwrap_or_else(|error| panic!("case {} valid syntax refused: {:?}", case, error));
    assert_eq!(
        document.schema_variant(),
        expected,
        "case {} selected the wrong schema",
        case
    );
    let canonical = encode(&document);
    assert_canonical_object_surface(&canonical);
    assert_eq!(
        parse(&canonical, limits()).expect("encoded bytes must parse independently"),
        syntax,
        "case {} changed the original generated syntax meaning",
        case
    );
    let decoded = decode_canonical(&canonical)
        .unwrap_or_else(|error| panic!("case {} canonical bytes refused: {:?}", case, error));
    assert_eq!(decoded, document, "case {} changed typed meaning", case);
    assert_eq!(
        encode(&decoded),
        canonical,
        "case {} changed bytes on exact reencode",
        case
    );
    canonical
}

fn replace_once(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let start = bytes
        .windows(from.len())
        .position(|window| window == from)
        .expect("real canonical bytes must contain mutation anchor");
    let mut output = Vec::with_capacity(bytes.len() + to.len() - from.len());
    output.extend_from_slice(&bytes[..start]);
    output.extend_from_slice(to);
    output.extend_from_slice(&bytes[start + from.len()..]);
    output
}

fn noncanonical_twins(case: usize, canonical: &[u8]) {
    let whitespace = [b" ".as_slice(), canonical].concat();
    assert_eq!(
        decode_canonical(&whitespace),
        Err(CanonicalError::NonCanonical),
        "case {} leading whitespace became canonical",
        case
    );
    let escaped = replace_once(canonical, b"\"fa.", b"\"\\u0066a.");
    assert_eq!(
        decode_canonical(&escaped),
        Err(CanonicalError::NonCanonical),
        "case {} optional escape became canonical",
        case
    );
}

fn schema_version_byte_twin(canonical: &[u8], expected: &[u8]) {
    let mutated = replace_once(canonical, expected, b"fa.invalid/0.1");
    assert_eq!(
        decode_canonical(&mutated),
        Err(CanonicalError::WrongSchemaVersion(
            "fa.invalid/0.1".to_owned()
        ))
    );
}

fn action_negatives(case: usize, valid: &Json, canonical: &[u8]) {
    let mut invalid_id = valid.clone();
    replace_field(
        &mut invalid_id,
        "action_id",
        Json::String("not an id".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_id),
        Err(CanonicalError::InvalidValue("action_id"))
    );

    let mut invalid_enum = valid.clone();
    replace_field(
        &mut invalid_enum,
        "branch",
        Json::String("not_a_branch".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_enum),
        Err(CanonicalError::InvalidValue("branch"))
    );

    let mut wrong_type = valid.clone();
    replace_field(
        &mut wrong_type,
        "payload_bytes",
        Json::String("0".to_owned()),
    );
    assert_eq!(
        validate_json(&wrong_type),
        Err(CanonicalError::WrongType("payload_bytes"))
    );

    let mut invalid_digest = valid.clone();
    replace_field(
        &mut invalid_digest,
        "payload_sha256",
        Json::String("not-a-digest".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_digest),
        Err(CanonicalError::InvalidValue("payload_sha256"))
    );

    let mut range = valid.clone();
    replace_field(&mut range, "payload_bytes", number(65_537));
    assert_eq!(
        validate_json(&range),
        Err(CanonicalError::IntegerOutOfRange("payload_bytes"))
    );

    let mut missing = valid.clone();
    remove_field(&mut missing, "tenant_id");
    assert_eq!(
        validate_json(&missing),
        Err(CanonicalError::MissingField("tenant_id"))
    );

    let mut unknown = valid.clone();
    object_mut(&mut unknown).insert("unknown".to_owned(), Json::Bool(true));
    assert_eq!(
        validate_json(&unknown),
        Err(CanonicalError::UnknownCriticalField("unknown".to_owned()))
    );

    let mut duplicate = valid.clone();
    replace_field(
        &mut duplicate,
        "required_observations",
        strings(["duplicate".to_owned(), "duplicate".to_owned()]),
    );
    assert_eq!(
        validate_json(&duplicate),
        Err(CanonicalError::DuplicateArrayValue("required_observations"))
    );
    assert_eq!(
        validate_json(&Json::Array(Vec::new())),
        Err(CanonicalError::WrongType("root"))
    );
    schema_version_byte_twin(canonical, b"fa.action/0.1");
    noncanonical_twins(case, canonical);
}

fn manifest_negatives(case: usize, valid: &Json, canonical: &[u8]) {
    let mut invalid_id = valid.clone();
    replace_field(
        &mut invalid_id,
        "adapter_id",
        Json::String("not an id".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_id),
        Err(CanonicalError::InvalidValue("adapter_id"))
    );

    let mut invalid_enum = valid.clone();
    replace_field(
        &mut invalid_enum,
        "mediation_mode",
        Json::String("not_a_mode".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_enum),
        Err(CanonicalError::InvalidValue("mediation_mode"))
    );

    let mut invalid_status = valid.clone();
    replace_field(
        &mut invalid_status,
        "status",
        Json::String("not_a_manifest_status".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_status),
        Err(CanonicalError::InvalidValue("status"))
    );

    let mut wrong_type = valid.clone();
    replace_field(
        &mut wrong_type,
        "activation_capture",
        Json::String("true".to_owned()),
    );
    assert_eq!(
        validate_json(&wrong_type),
        Err(CanonicalError::WrongType("activation_capture"))
    );

    let mut max_chars = valid.clone();
    replace_field(&mut max_chars, "limitations", strings(["😀".repeat(1_025)]));
    assert_eq!(
        validate_json(&max_chars),
        Err(CanonicalError::InvalidValue("limitations"))
    );

    let mut missing = valid.clone();
    remove_field(&mut missing, "status");
    assert_eq!(
        validate_json(&missing),
        Err(CanonicalError::MissingField("status"))
    );

    let mut unknown = valid.clone();
    object_mut(&mut unknown).insert("unknown".to_owned(), Json::Bool(true));
    assert_eq!(
        validate_json(&unknown),
        Err(CanonicalError::UnknownCriticalField("unknown".to_owned()))
    );

    let mut duplicate = valid.clone();
    replace_field(
        &mut duplicate,
        "restart_grades",
        strings(["unavailable".to_owned(), "unavailable".to_owned()]),
    );
    assert_eq!(
        validate_json(&duplicate),
        Err(CanonicalError::DuplicateArrayValue("restart_grades"))
    );
    let mut invalid_grade = valid.clone();
    replace_field(
        &mut invalid_grade,
        "restart_grades",
        strings(["not_a_restart_grade".to_owned()]),
    );
    assert_eq!(
        validate_json(&invalid_grade),
        Err(CanonicalError::InvalidValue("restart_grades"))
    );
    let mut empty_required = valid.clone();
    replace_field(
        &mut empty_required,
        "restart_grades",
        Json::Array(Vec::new()),
    );
    assert_eq!(
        validate_json(&empty_required),
        Err(CanonicalError::IntegerOutOfRange("restart_grades"))
    );
    let mut empty_limitations = valid.clone();
    replace_field(
        &mut empty_limitations,
        "limitations",
        Json::Array(Vec::new()),
    );
    assert_eq!(
        validate_json(&empty_limitations),
        Err(CanonicalError::IntegerOutOfRange("limitations"))
    );
    assert_eq!(
        validate_json(&Json::Array(Vec::new())),
        Err(CanonicalError::WrongType("root"))
    );
    schema_version_byte_twin(canonical, b"fa.capabilities/0.1");
    noncanonical_twins(case, canonical);
}

fn claim_negatives(case: usize, valid: &Json, canonical: &[u8]) {
    let mut invalid_id = valid.clone();
    replace_field(
        &mut invalid_id,
        "claim_id",
        Json::String("not an id".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_id),
        Err(CanonicalError::InvalidValue("claim_id"))
    );

    let mut invalid_enum = valid.clone();
    replace_field(
        &mut invalid_enum,
        "claim_class",
        Json::String("not_a_claim_class".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_enum),
        Err(CanonicalError::InvalidValue("claim_class"))
    );

    let mut invalid_status = valid.clone();
    replace_field(
        &mut invalid_status,
        "status",
        Json::String("not_a_claim_status".to_owned()),
    );
    assert_eq!(
        validate_json(&invalid_status),
        Err(CanonicalError::InvalidValue("status"))
    );

    let mut wrong_type = valid.clone();
    replace_field(&mut wrong_type, "assertion", Json::Bool(true));
    assert_eq!(
        validate_json(&wrong_type),
        Err(CanonicalError::WrongType("assertion"))
    );

    let mut invalid_digest = valid.clone();
    replace_field(
        &mut invalid_digest,
        "evidence_refs",
        strings(["not-a-digest".to_owned()]),
    );
    assert_eq!(
        validate_json(&invalid_digest),
        Err(CanonicalError::InvalidValue("evidence_refs"))
    );

    let mut max_chars = valid.clone();
    replace_field(
        &mut max_chars,
        "assertion",
        Json::String(format!("{}x", "😀".repeat(2_048))),
    );
    assert_eq!(
        validate_json(&max_chars),
        Err(CanonicalError::InvalidValue("assertion"))
    );

    let mut missing = valid.clone();
    remove_field(&mut missing, "claim_class");
    assert_eq!(
        validate_json(&missing),
        Err(CanonicalError::MissingField("claim_class"))
    );

    let mut unknown = valid.clone();
    object_mut(&mut unknown).insert("unknown".to_owned(), Json::Bool(true));
    assert_eq!(
        validate_json(&unknown),
        Err(CanonicalError::UnknownCriticalField("unknown".to_owned()))
    );

    let mut duplicate = valid.clone();
    replace_field(
        &mut duplicate,
        "source_refs",
        strings(["duplicate".to_owned(), "duplicate".to_owned()]),
    );
    assert_eq!(
        validate_json(&duplicate),
        Err(CanonicalError::DuplicateArrayValue("source_refs"))
    );
    let mut empty_required = valid.clone();
    replace_field(&mut empty_required, "assumptions", Json::Array(Vec::new()));
    assert_eq!(
        validate_json(&empty_required),
        Err(CanonicalError::IntegerOutOfRange("assumptions"))
    );
    let mut empty_limitations = valid.clone();
    replace_field(
        &mut empty_limitations,
        "limitations",
        Json::Array(Vec::new()),
    );
    assert_eq!(
        validate_json(&empty_limitations),
        Err(CanonicalError::IntegerOutOfRange("limitations"))
    );
    assert_eq!(
        validate_json(&Json::Array(Vec::new())),
        Err(CanonicalError::WrongType("root"))
    );
    schema_version_byte_twin(canonical, b"fa.claim/0.1");
    noncanonical_twins(case, canonical);
}

#[test]
fn action_schema_deterministic_properties() {
    let mut passed = 0;
    for case in 0..CASES_PER_SCHEMA {
        let syntax = action(case);
        let canonical = round_trip(case, SchemaVariant::ActionProposalV01, syntax.clone());
        action_negatives(case, &syntax, &canonical);
        passed += 1;
    }
    eprintln!(
        "canonical_json_properties schema=action generated={} passed={} seeds={:?}",
        CASES_PER_SCHEMA, passed, FIXED_SEEDS
    );
}

#[test]
fn capability_manifest_schema_deterministic_properties() {
    let mut passed = 0;
    for case in 0..CASES_PER_SCHEMA {
        let syntax = manifest(case);
        let canonical = round_trip(case, SchemaVariant::CapabilityManifestV01, syntax.clone());
        manifest_negatives(case, &syntax, &canonical);
        passed += 1;
    }
    eprintln!(
        "canonical_json_properties schema=manifest generated={} passed={} seeds={:?}",
        CASES_PER_SCHEMA, passed, FIXED_SEEDS
    );
}

#[test]
fn evidence_claim_schema_deterministic_properties() {
    let mut passed = 0;
    for case in 0..CASES_PER_SCHEMA {
        let syntax = claim(case);
        let canonical = round_trip(case, SchemaVariant::EvidenceClaimV01, syntax.clone());
        claim_negatives(case, &syntax, &canonical);
        passed += 1;
    }
    eprintln!(
        "canonical_json_properties schema=claim generated={} passed={} seeds={:?}",
        CASES_PER_SCHEMA, passed, FIXED_SEEDS
    );
}
