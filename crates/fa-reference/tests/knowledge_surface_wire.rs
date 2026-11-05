//! Public wire regressions for the sole epistemic boundary. These exercise the
//! actual serializer and strict parser, not a replacement evidence reducer.

use fa_reference::knowledge_surface::{
    AgentSurfaceSerializer, KNOWLEDGE_JSON_LIMITS, SafetyRelevant, SerializerRefusal,
    encode_knowledge_json,
};
use fa_reference::observation_algebra::{
    AuthorityKind, Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
    UnknownReason,
};
use fa_reference::product_frontier::{
    FrontierRequirement, FrontierStage, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::strict_json::{self, Json};
use fa_reference::witness::DomainProjection;

#[derive(Clone)]
struct TextPayload(String);

impl SafetyRelevant for TextPayload {
    fn safety_type_name() -> &'static str {
        "Text\"}\n\\Payload"
    }

    fn canonical_payload_string(&self) -> String {
        self.0.clone()
    }
}

struct StructuredPayload(String);

impl SafetyRelevant for StructuredPayload {
    fn safety_type_name() -> &'static str {
        "StructuredPayload"
    }

    fn canonical_payload_string(&self) -> String {
        "unused-text-fallback".to_owned()
    }

    fn canonical_payload_json(&self) -> String {
        self.0.clone()
    }
}

fn basis() -> Basis {
    Basis::new(
        BasisIdentity {
            subject_id: 11,
            question_id: 12,
            profile_id: 13,
            semantic_generation: 14,
            policy_epoch: 15,
            model_epoch: 16,
            tokenizer_epoch: 17,
            codec_epoch: 18,
            control_seq: u64::MAX,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: ScopedProof::Declared {
                scope_id: 21,
                generation: 22,
            },
            complete_for_contract: ScopedProof::Gapped {
                scope_id: 23,
                first_missing: 24,
            },
            semantically_valid: ScopedProof::Unsupported { scope_id: 25 },
            available_for_replay: ScopedProof::Expired {
                scope_id: 26,
                generation: 27,
            },
        },
    )
}

fn parse(encoded: &str) -> Json {
    strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS)
        .expect("serializer must produce bounded strict JSON")
}

fn field<'a>(object: &'a Json, name: &str) -> &'a Json {
    object
        .get(name)
        .unwrap_or_else(|| panic!("missing field {name}"))
}

fn text<'a>(object: &'a Json, name: &str) -> &'a str {
    field(object, name)
        .as_str()
        .unwrap_or_else(|| panic!("field {name} must be a string"))
}

fn number(object: &Json, name: &str) -> u64 {
    field(object, name)
        .as_u64()
        .unwrap_or_else(|| panic!("field {name} must be an exact u64"))
}

fn projection(source: u64) -> ProjectionKey {
    ProjectionKey {
        source,
        branch: 32,
        projection: 33,
        source_epoch: 34,
    }
}

#[test]
fn known_keeps_every_identity_epoch_and_noninterchangeable_proof_state() {
    let wrapped = Knowledge::known(TextPayload("evidence".to_owned()), basis());
    let encoded = encode_knowledge_json(&wrapped).expect("valid known encoding");
    let json = parse(&encoded);
    assert_eq!(text(&json, "state"), "Known");
    assert_eq!(text(&json, "claim_class"), "BoundedModel");
    assert_eq!(number(&json, "generation"), 14);
    assert_eq!(number(&json, "control_seq"), u64::MAX);

    let wire_basis = field(&json, "basis");
    assert_eq!(text(wire_basis, "claim_class"), "BoundedModel");
    let identity = field(wire_basis, "identity");
    for (key, expected) in [
        ("subject_id", 11),
        ("question_id", 12),
        ("profile_id", 13),
        ("semantic_generation", 14),
        ("policy_epoch", 15),
        ("model_epoch", 16),
        ("tokenizer_epoch", 17),
        ("codec_epoch", 18),
        ("control_seq", u64::MAX),
    ] {
        assert_eq!(number(identity, key), expected, "identity field {key}");
    }
    assert_eq!(identity.as_object().expect("identity object").len(), 9);

    let proofs = field(wire_basis, "proofs");
    assert_eq!(proofs.as_object().expect("proof object").len(), 4);
    let origin = field(proofs, "authentic_origin");
    assert_eq!(text(origin, "state"), "Declared");
    assert_eq!(number(origin, "scope_id"), 21);
    assert_eq!(number(origin, "generation"), 22);
    let completeness = field(proofs, "complete_for_contract");
    assert_eq!(text(completeness, "state"), "Gapped");
    assert_eq!(number(completeness, "scope_id"), 23);
    assert_eq!(number(completeness, "first_missing"), 24);
    let semantics = field(proofs, "semantically_valid");
    assert_eq!(text(semantics, "state"), "Unsupported");
    assert_eq!(number(semantics, "scope_id"), 25);
    let replay = field(proofs, "available_for_replay");
    assert_eq!(text(replay, "state"), "Expired");
    assert_eq!(number(replay, "scope_id"), 26);
    assert_eq!(number(replay, "generation"), 27);
    assert!(wrapped.as_known(ClaimClass::BoundedModel, 14).is_err());
}

#[test]
fn default_text_payload_and_untrusted_field_type_names_cannot_inject_json() {
    let controls: String = (0_u8..=31).map(char::from).collect();
    let payload = format!("\"}},\"state\":\"Known\",\"injected\":true,\\{controls}雪");
    let field_name = "\"field\":\"replacement\",\n\\\t";
    let wrapped = Knowledge::known(TextPayload(payload.clone()), basis());
    let serialized = AgentSurfaceSerializer::try_serialize_safety_field(field_name, &wrapped)
        .expect("untrusted strings are escaped, not refused");
    assert_eq!(serialized.variant, "Known");
    let json = parse(&serialized.canonical_json);
    assert_eq!(text(&json, "field"), field_name);
    assert_eq!(text(&json, "type"), TextPayload::safety_type_name());
    assert_eq!(text(&json, "value"), payload);
    assert_eq!(text(&json, "state"), "Known");
    assert_eq!(json.as_object().expect("wrapped object").len(), 8);
    assert!(json.get("injected").is_none());
    assert!(
        AgentSurfaceSerializer::validate_wire_field_is_wrapped(
            field_name,
            &serialized.canonical_json,
        )
        .is_ok()
    );
}

#[test]
fn structured_payload_preserves_objects_arrays_and_exact_integer_values() {
    let wrapped = Knowledge::known(
        StructuredPayload(
            "{ \"z\": [null, true, \"escaped\\ntext\"], \"a\": 18446744073709551615 }".to_owned(),
        ),
        basis(),
    );
    let encoded = encode_knowledge_json(&wrapped).expect("valid structured payload");
    let json = parse(&encoded);
    let value = field(&json, "value");
    assert_eq!(number(value, "a"), u64::MAX);
    let values = field(value, "z").as_array().expect("array payload");
    assert!(values[0].is_null());
    assert_eq!(values[1].as_bool(), Some(true));
    assert_eq!(values[2].as_str(), Some("escaped\ntext"));
    assert!(!encoded.contains("unused-text-fallback"));
    assert!(
        encoded.contains(
            "\"value\":{\"a\":18446744073709551615,\"z\":[null,true,\"escaped\\ntext\"]}"
        )
    );
}

#[test]
fn invalid_custom_payload_is_refused_and_legacy_surface_never_claims_known() {
    for payload in [
        "{",
        "{\"duplicate\":1,\"duplicate\":2}",
        "{\"ok\":true},\"state\":\"Known\"",
        "01",
        "NaN",
        "\"unterminated",
    ] {
        let wrapped = Knowledge::known(StructuredPayload(payload.to_owned()), basis());
        assert_eq!(
            encode_knowledge_json(&wrapped),
            Err(SerializerRefusal::InvalidPayloadJson {
                safety_type: "StructuredPayload",
            }),
            "invalid payload {payload:?}",
        );
        assert!(AgentSurfaceSerializer::try_serialize_safety_field("result", &wrapped).is_err());
        let fallback = AgentSurfaceSerializer::serialize_safety_field("result", &wrapped);
        assert_eq!(fallback.variant, "Unknown");
        let json = parse(&fallback.canonical_json);
        assert_eq!(text(&json, "state"), "Unknown");
        assert_eq!(text(&json, "reason"), "ConflictingEvidence");
        assert_eq!(text(&json, "serialization_error"), "IntegrityFailure");
        assert!(json.get("value").is_none());
        assert!(json.get("basis").is_none());
    }
}

#[test]
fn pending_names_each_source_projection_stage_and_closure_instead_of_only_count() {
    let first = FrontierRequirement {
        key: projection(31),
        stage: FrontierStage::Authenticated,
        through: 35,
        closure: Some(36),
    };
    let second = FrontierRequirement {
        key: ProjectionKey {
            source_epoch: 40,
            ..projection(39)
        },
        stage: FrontierStage::Presented,
        through: 41,
        closure: None,
    };
    let wrapped: Knowledge<TextPayload> =
        Knowledge::pending(vec![first, second], 42).expect("distinct valid pending frontiers");
    let json = parse(&encode_knowledge_json(&wrapped).expect("pending encoding"));
    assert_eq!(text(&json, "state"), "Pending");
    assert_eq!(number(&json, "expected_cost"), 42);
    assert_eq!(number(&json, "frontier_count"), 2);
    let frontiers = field(&json, "frontiers")
        .as_array()
        .expect("frontiers array");
    assert_eq!(frontiers.len(), 2);
    for (wire, expected) in frontiers.iter().zip([first, second]) {
        let key = field(wire, "key");
        assert_eq!(number(key, "source"), expected.key.source);
        assert_eq!(number(key, "branch"), expected.key.branch);
        assert_eq!(number(key, "projection"), expected.key.projection);
        assert_eq!(number(key, "source_epoch"), expected.key.source_epoch);
        assert_eq!(number(wire, "through"), expected.through);
        assert_eq!(field(wire, "closure").as_u64(), expected.closure);
    }
    assert_eq!(text(&frontiers[0], "stage"), "Authenticated");
    assert_eq!(text(&frontiers[1], "stage"), "Presented");
    assert!(field(&frontiers[1], "closure").is_null());
    assert!(json.get("value").is_none());
}

#[test]
fn unknown_reasons_remain_unknown_without_synthetic_values() {
    for reason in [
        UnknownReason::Cancelled,
        UnknownReason::Incomplete,
        UnknownReason::IncompatibleGeneration,
        UnknownReason::MissingExactDomain,
        UnknownReason::LowerLayerUnverified,
        UnknownReason::ConflictingEvidence,
        UnknownReason::Unavailable,
    ] {
        let wrapped: Knowledge<TextPayload> = Knowledge::unknown(reason);
        let json = parse(&encode_knowledge_json(&wrapped).expect("unknown encoding"));
        assert_eq!(text(&json, "state"), "Unknown");
        assert_eq!(text(&json, "reason"), format!("{reason:?}"));
        assert!(json.get("value").is_none());
        assert!(json.get("basis").is_none());
    }
}

#[test]
fn withheld_never_calls_a_payload_encoder_or_discloses_existence() {
    struct UnavailablePayload;
    impl SafetyRelevant for UnavailablePayload {
        fn safety_type_name() -> &'static str {
            "UnavailablePayload"
        }

        fn canonical_payload_string(&self) -> String {
            panic!("withheld must never inspect a hidden payload")
        }

        fn canonical_payload_json(&self) -> String {
            panic!("withheld must never encode a hidden payload")
        }
    }

    let existing: Knowledge<UnavailablePayload> = Knowledge::withheld(AuthorityKind::ExactRead);
    let nonexistent: Knowledge<UnavailablePayload> = Knowledge::withheld(AuthorityKind::ExactRead);
    let existing_json = encode_knowledge_json(&existing).expect("withheld encoding");
    let nonexistent_json = encode_knowledge_json(&nonexistent).expect("withheld encoding");
    assert_eq!(existing_json, nonexistent_json);
    let json = parse(&existing_json);
    assert_eq!(text(&json, "state"), "Withheld");
    assert_eq!(text(&json, "authority_required"), "ExactRead");
    assert_eq!(json.as_object().expect("withheld object").len(), 4);
    assert!(json.get("value").is_none());
    assert!(json.get("basis").is_none());
    assert!(json.get("exists").is_none());
}

#[test]
fn stale_retains_old_basis_and_current_generation_without_restoring_value() {
    let known = Knowledge::known(TextPayload("old value".to_owned()), basis());
    let stale: Knowledge<TextPayload> = Knowledge::stale(basis(), 100);
    let known_json = parse(&encode_knowledge_json(&known).expect("known encoding"));
    let stale_json = parse(&encode_knowledge_json(&stale).expect("stale encoding"));
    assert_eq!(text(&stale_json, "state"), "Stale");
    assert_eq!(number(&stale_json, "basis_generation"), 14);
    assert_eq!(number(&stale_json, "current_generation"), 100);
    assert_eq!(field(&stale_json, "basis"), field(&known_json, "basis"));
    assert!(stale_json.get("value").is_none());
}

#[test]
fn absent_retains_exact_closed_domain_epoch_cut_and_terminal_marker() {
    let key = projection(51);
    let requirement = FrontierRequirement {
        key,
        stage: FrontierStage::Authenticated,
        through: 53,
        closure: Some(54),
    };
    let marker = TrustedClosingMarker {
        key,
        final_sequence: 53,
        marker_generation: 54,
    };
    let domain = DomainProjection::new(55, 56, key);
    let wrapped: Knowledge<TextPayload> =
        Knowledge::absent_from_closed_frontier(domain, 57, 58, 59, requirement, marker)
            .expect("complete closed domain");
    let json = parse(&encode_knowledge_json(&wrapped).expect("absent encoding"));
    assert_eq!(text(&json, "state"), "Absent");
    assert_eq!(number(&json, "closed_domain_id"), 55);
    assert_eq!(number(&json, "revision"), 57);
    let closed = field(&json, "domain");
    for (name, expected) in [
        ("domain_id", 55),
        ("domain_epoch", 56),
        ("revision", 57),
        ("control_cut", 58),
        ("semantic_epoch", 59),
    ] {
        assert_eq!(number(closed, name), expected);
    }
    let wire_requirement = field(closed, "requirement");
    assert_eq!(text(wire_requirement, "stage"), "Authenticated");
    assert_eq!(number(wire_requirement, "through"), 53);
    assert_eq!(number(wire_requirement, "closure"), 54);
    let wire_marker = field(closed, "marker");
    assert_eq!(number(wire_marker, "final_sequence"), 53);
    assert_eq!(number(wire_marker, "marker_generation"), 54);
    assert_eq!(field(wire_requirement, "key"), field(wire_marker, "key"));
    assert_eq!(field(closed, "projection"), field(wire_marker, "key"));
    assert_eq!(number(field(wire_marker, "key"), "source"), 51);
    assert!(json.get("value").is_none());

    let open = FrontierRequirement {
        closure: None,
        ..requirement
    };
    assert!(
        Knowledge::<TextPayload>::absent_from_closed_frontier(domain, 57, 58, 59, open, marker,)
            .is_err()
    );
}

#[test]
fn custom_payload_depth_size_and_metadata_bounds_have_permitted_twins() {
    let nested = |depth: usize| format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
    let permitted = Knowledge::known(StructuredPayload(nested(8)), basis());
    assert!(encode_knowledge_json(&permitted).is_ok());
    for payload in [
        nested(80),
        format!(
            "\"{}\"",
            "x".repeat(KNOWLEDGE_JSON_LIMITS.max_string_bytes + 1)
        ),
        " ".repeat(KNOWLEDGE_JSON_LIMITS.max_bytes + 1),
    ] {
        let excessive = Knowledge::known(StructuredPayload(payload), basis());
        assert!(matches!(
            encode_knowledge_json(&excessive),
            Err(SerializerRefusal::InvalidPayloadJson { .. }),
        ));
    }
    assert!(AgentSurfaceSerializer::try_serialize_safety_field("result", &permitted).is_ok());
    assert!(matches!(
        AgentSurfaceSerializer::try_serialize_safety_field(&"x".repeat(2048), &permitted),
        Err(SerializerRefusal::EncodingLimitExceeded {
            component: "field_name",
            ..
        }),
    ));
}

#[test]
fn public_pending_construction_cannot_bypass_wire_frontier_limit() {
    let frontier = FrontierRequirement {
        key: projection(61),
        stage: FrontierStage::Captured,
        through: 62,
        closure: None,
    };
    let permitted: Knowledge<TextPayload> =
        Knowledge::pending(vec![frontier], 1).expect("valid pending");
    assert!(encode_knowledge_json(&permitted).is_ok());
    let excessive: Knowledge<TextPayload> = Knowledge::Pending {
        frontiers: vec![frontier; fa_reference::observation_algebra::MAX_OPERATOR_FRONTIERS + 1],
        expected_cost: 1,
    };
    assert!(matches!(
        encode_knowledge_json(&excessive),
        Err(SerializerRefusal::EncodingLimitExceeded {
            component: "frontiers",
            ..
        }),
    ));
}

#[test]
fn wrapper_guard_rejects_nested_forged_duplicate_and_unknown_state_tags() {
    let permitted: Knowledge<TextPayload> = Knowledge::unknown(UnknownReason::Unavailable);
    let valid = AgentSurfaceSerializer::serialize_safety_field("result", &permitted);
    assert!(
        AgentSurfaceSerializer::validate_wire_field_is_wrapped("result", &valid.canonical_json,)
            .is_ok()
    );
    for forged in [
        "{\"type\":\"TextPayload\",\"nested\":{\"state\":\"Known\"}}",
        "{\"type\":\"TextPayload\",\"state\":\"Known\",\"state\":\"Unknown\"}",
        "{\"type\":\"TextPayload\",\"state\":\"Invented\"}",
        "{\"type\":\"TextPayload\",\"state\":true}",
        "{\"state\":\"Known\"}",
        "{\"type\":\"TextPayload\",\"state\":\"Known\"",
        "{\"type\":\"TextPayload\",\"state\":\"Known\"} trailing",
        "{\"text\":\"\\\"state\\\":\\\"Known\\\"\"}",
    ] {
        assert!(
            AgentSurfaceSerializer::validate_wire_field_is_wrapped("result", forged).is_err(),
            "forged wrapper {forged:?}",
        );
    }
    let refusal = AgentSurfaceSerializer::validate_wire_field_is_wrapped("field\ninjected", "true")
        .expect_err("bare scalar is refused");
    match refusal {
        SerializerRefusal::BareScalarRejected { log_line, .. } => {
            assert!(!log_line.contains('\n'));
            assert!(log_line.contains("field\\ninjected"));
        }
        other => panic!("unexpected refusal: {other:?}"),
    }
}
