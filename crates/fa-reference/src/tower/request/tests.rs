use super::*;
use crate::action::consequence::experiment::MAX_INTERVENTIONS;
use crate::action::consequence::oversight::actor_wire::{Command, decode_command, encode_command};

const READ: &str = concat!(
    "{\"schema_version\":\"fa.tower_request/1\",\"operation_id\":\"read-1\",",
    "\"address\":\"fa://tenant/evidence/item@1\",\"verb\":\"get\",",
    "\"arguments\":null,\"idempotency_key\":null}"
);

fn request(verb: Verb, arguments: ReadArguments) -> TowerRequest {
    TowerRequest {
        operation_id: "request-1".to_owned(),
        address: Address::parse("fa://1/run/2@3").unwrap(),
        verb,
        arguments,
        idempotency_key: None,
    }
}

fn proposal() -> ActorProposal {
    ActorProposal {
        target: ResolvedTarget {
            adapter: 1,
            object: 2,
            contract_version: 3,
            expected_version: 4,
            generation: 5,
        },
        payload: vec![0, 255, b'"', b'\\'],
        units: 4,
        deadline: ElapsedTick(7),
        expected_policy_epoch: 0,
    }
}

fn proposed() -> TowerRequest {
    let mut result = request(
        Verb::Propose,
        ReadArguments::Propose {
            request: 1,
            proposal: proposal(),
        },
    );
    result.idempotency_key = Some("1".to_owned());
    result
}

fn document(verb: &str, arguments: &str) -> String {
    READ.replace("\"get\"", &quote(verb))
        .replace("\"arguments\":null", &format!("\"arguments\":{arguments}"))
}

fn round_trip(request: &TowerRequest) {
    let bytes = encode_request(request).unwrap();
    assert_eq!(decode_request(bytes.as_bytes()).unwrap(), *request);
    assert_eq!(
        encode_request(&decode_request(bytes.as_bytes()).unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn fixed_request_vector_is_stable_and_all_registered_verbs_are_recognized() {
    let decoded = decode_request(READ.as_bytes()).unwrap();
    assert_eq!(decoded.operation_id, "read-1");
    assert_eq!(
        decoded.address,
        Address::parse("fa://tenant/evidence/item@1").unwrap()
    );
    assert_eq!(decoded.verb, Verb::Get);
    assert_eq!(decoded.arguments, ReadArguments::None);
    assert_eq!(encode_request(&decoded).unwrap(), READ);

    for verb in Verb::ALL {
        let mut value = request(verb, ReadArguments::None);
        match verb {
            Verb::Propose => value = proposed(),
            Verb::Precheck => {
                value.arguments = ReadArguments::Precheck {
                    query: 1,
                    proposal: proposal(),
                }
            }
            Verb::Capabilities => {
                value.arguments = ReadArguments::Capabilities {
                    query: 1,
                    expected_policy_epoch: 0,
                }
            }
            Verb::Replay => value.idempotency_key = Some("replay-1".to_owned()),
            Verb::PolicyReplay => {
                use crate::action::consequence::gate::containment::session::policy::{
                    Policy, Predicate,
                };
                value.arguments = ReadArguments::PolicyReplay {
                    campaign: 1,
                    expected_policy_generation: 1,
                    candidate: Policy::new(2, vec![Predicate::UnitsAtMost(4)]).unwrap(),
                };
                value.idempotency_key = Some("1".to_owned());
            }
            _ => {}
        }
        round_trip(&value);
    }
    for retired_or_unknown in ["status", "verify-bundle", "Get", "", "permit"] {
        assert_eq!(
            decode_request(document(retired_or_unknown, "null").as_bytes()),
            Err(TowerDecodingError::UnknownVerb)
        );
    }
}

#[test]
fn exact_u64_and_arbitrary_payload_bits_survive_both_tower_and_native_codecs() {
    let maximum = u64::MAX;
    let native = ActorProposal {
        target: ResolvedTarget {
            adapter: maximum,
            object: maximum,
            contract_version: maximum,
            expected_version: maximum,
            generation: maximum,
        },
        payload: vec![0, 128, 255, b'"', b'\\'],
        units: maximum,
        deadline: ElapsedTick(maximum),
        expected_policy_epoch: maximum,
    };
    let mut input = request(
        Verb::Propose,
        ReadArguments::Propose {
            request: maximum,
            proposal: native.clone(),
        },
    );
    input.address.generation = Some(maximum);
    input.idempotency_key = Some(maximum.to_string());
    round_trip(&input);
    let encoded = encode_request(&input).unwrap();
    assert!(encoded.contains("\"request\":\"18446744073709551615\""));
    assert!(encoded.contains("\"payload_hex\":\"0080ff225c\""));
    let decoded = decode_request(encoded.as_bytes()).unwrap();
    let ReadArguments::Propose {
        request: key,
        proposal,
    } = decoded.arguments
    else {
        panic!("native request")
    };
    let command = Command::Submit {
        request: key,
        proposal,
    };
    assert_eq!(
        decode_command(&encode_command(&command).unwrap()).unwrap(),
        command
    );
    assert_eq!(
        command,
        Command::Submit {
            request: maximum,
            proposal: native
        }
    );

    round_trip(&request(
        Verb::Tail,
        ReadArguments::Tail {
            since_seq: maximum,
            max_events: 128,
        },
    ));
    round_trip(&request(
        Verb::Next,
        ReadArguments::Next { max_cost: maximum },
    ));
    round_trip(&request(
        Verb::Capabilities,
        ReadArguments::Capabilities {
            query: maximum,
            expected_policy_epoch: maximum,
        },
    ));
    round_trip(&request(
        Verb::Situation,
        ReadArguments::SituationPage { cursor: maximum },
    ));
}

#[test]
fn native_situation_cursor_is_nonzero_canonical_data_and_never_an_offset() {
    let query = request(Verb::Situation, ReadArguments::SituationPage { cursor: 17 });
    round_trip(&query);
    assert!(
        encode_request(&query)
            .unwrap()
            .contains("\"cursor\":\"17\"")
    );
    for arguments in [
        "{\"cursor\":\"0\"}",
        "{\"cursor\":\"01\"}",
        "{\"cursor\":\"-1\"}",
        "{\"cursor\":\"1.0\"}",
        "{\"cursor\":\"18446744073709551616\"}",
        "{\"cursor\":1}",
        "{\"cursor\":null}",
        "{\"offset\":\"16\"}",
        "{\"cursor\":\"1\",\"offset\":\"16\"}",
        "{\"cursor\":\"1\",\"cursor\":\"2\"}",
    ] {
        assert_eq!(
            decode_request(document("situation", arguments).as_bytes()),
            Err(TowerDecodingError::MalformedRequest),
            "{arguments}"
        );
    }
    assert_eq!(
        encode_request(&request(
            Verb::Situation,
            ReadArguments::SituationPage { cursor: 0 }
        )),
        Err(TowerDecodingError::MalformedRequest)
    );
    for verb in [Verb::Get, Verb::Tail, Verb::WhyHeld, Verb::Explain] {
        assert_eq!(
            encode_request(&request(verb, ReadArguments::SituationPage { cursor: 17 })),
            Err(TowerDecodingError::MalformedRequest)
        );
        assert_eq!(
            decode_request(document(verb.as_str(), "{\"cursor\":\"17\"}").as_bytes()),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
    round_trip(&request(Verb::Situation, ReadArguments::None));
}

#[test]
fn every_object_level_is_closed_and_duplicate_names_are_not_aliases() {
    let valid = encode_request(&proposed()).unwrap();
    for invalid in [
        valid.replacen('{', "{\"operator\":true,", 1),
        valid.replacen(
            "\"arguments\":{",
            "\"arguments\":{\"authority\":\"Permit\",",
            1,
        ),
        valid.replacen("\"proposal\":{", "\"proposal\":{\"scope\":{},", 1),
        valid.replacen("\"target\":{", "\"target\":{\"resolved\":true,", 1),
        valid.replacen(
            "\"request\":\"1\"",
            "\"request\":\"1\",\"request\":\"2\"",
            1,
        ),
        valid.replacen(
            "\"verb\":\"propose\"",
            "\"verb\":\"propose\",\"\\u0076erb\":\"get\"",
            1,
        ),
        valid.replace(",\"expected_policy_epoch\":\"0\"", ""),
    ] {
        assert_eq!(
            decode_request(invalid.as_bytes()),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
    assert_eq!(
        decode_request(READ.replace("/1\"", "/2\"").as_bytes()),
        Err(TowerDecodingError::UnsupportedVersion)
    );
}

#[test]
fn linked_situations_require_explicit_true_and_keep_original_cursor_identity() {
    for cursor in [None, Some(1), Some(u64::MAX)] {
        let query = request(Verb::Situation, ReadArguments::SituationLinks { cursor });
        round_trip(&query);
        let encoded = encode_request(&query).unwrap();
        assert!(encoded.contains("\"request_links\":true"));
        assert_eq!(encoded.contains("\"cursor\":"), cursor.is_some());
    }
    for arguments in [
        "{\"request_links\":false}",
        "{\"request_links\":null}",
        "{\"request_links\":\"true\"}",
        "{\"request_links\":1}",
        "{\"request_links\":true,\"cursor\":null}",
        "{\"request_links\":true,\"cursor\":\"0\"}",
        "{\"request_links\":true,\"cursor\":\"01\"}",
        "{\"request_links\":true,\"cursor\":\"-1\"}",
        "{\"request_links\":true,\"cursor\":\"1.0\"}",
        "{\"request_links\":true,\"cursor\":\"18446744073709551616\"}",
        "{\"request_links\":true,\"cursor\":1}",
        "{\"request_links\":true,\"offset\":\"16\"}",
        "{\"request_links\":true,\"schema\":\"fa.publication-situation/2\"}",
        "{\"request_links\":true,\"request_links\":true}",
        "{\"request_links\":true,\"cursor\":\"1\",\"cursor\":\"2\"}",
    ] {
        assert_eq!(
            decode_request(document("situation", arguments).as_bytes()),
            Err(TowerDecodingError::MalformedRequest),
            "{arguments}"
        );
    }
    assert_eq!(
        encode_request(&request(
            Verb::Situation,
            ReadArguments::SituationLinks { cursor: Some(0) }
        )),
        Err(TowerDecodingError::MalformedRequest)
    );
}

#[test]
fn situation_request_links_cannot_be_selected_for_any_other_verb() {
    for verb in Verb::ALL
        .into_iter()
        .filter(|verb| *verb != Verb::Situation)
    {
        for cursor in [None, Some(17)] {
            assert_eq!(
                encode_request(&request(verb, ReadArguments::SituationLinks { cursor })),
                Err(TowerDecodingError::MalformedRequest),
                "{verb:?}"
            );
        }
        assert_eq!(
            decode_request(document(verb.as_str(), "{\"request_links\":true}").as_bytes()),
            Err(TowerDecodingError::MalformedRequest),
            "{verb:?}"
        );
    }
}

#[test]
fn root_and_argument_types_cannot_select_another_operation() {
    for invalid in [
        READ.replace("\"operation_id\":\"read-1\"", "\"operation_id\":1"),
        READ.replace(
            "\"address\":\"fa://tenant/evidence/item@1\"",
            "\"address\":{}",
        ),
        READ.replace("\"verb\":\"get\"", "\"verb\":true"),
        READ.replace("\"arguments\":null", "\"arguments\":{}"),
        READ.replace("\"idempotency_key\":null", "\"idempotency_key\":1"),
        document("get", "{\"since_seq\":\"0\",\"max_events\":\"1\"}"),
        document("tail", "{\"max_cost\":\"1\"}"),
        document("doctor", "{\"passed\":true}"),
        document("precheck", "null"),
        document("propose", "null"),
        document("capabilities", "null"),
        "[]".to_owned(),
    ] {
        assert_eq!(
            decode_request(invalid.as_bytes()),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
}

#[test]
fn malformed_text_and_trailing_documents_are_rejected_without_input_errors() {
    let mut invalid_utf8 = READ.as_bytes().to_vec();
    invalid_utf8[5] = 255;
    assert_eq!(
        decode_request(&invalid_utf8),
        Err(TowerDecodingError::MalformedRequest)
    );
    for value in ["\\ud800", "\\udfff", "\\ud800\\u0041", "\\q", "\n"] {
        let invalid = READ.replace("read-1", value);
        let error = decode_request(invalid.as_bytes()).unwrap_err();
        assert_eq!(error, TowerDecodingError::MalformedRequest);
        assert_eq!(error.to_string(), "malformed Tower request");
    }
    for suffix in ["{}", "null", "0", "/*comment*/"] {
        assert_eq!(
            decode_request(format!("{READ}{suffix}").as_bytes()),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
    let mut valid = decode_request(READ.as_bytes()).unwrap();
    valid.operation_id = "quote\" slash\\ null\0 newline\n 😀".to_owned();
    valid.idempotency_key = Some("λ\tkey".to_owned());
    round_trip(&valid);
    let decoded = decode_request(READ.replace("read-1", "\\ud83d\\ude00").as_bytes()).unwrap();
    assert_eq!(decoded.operation_id, "😀");
}

#[test]
fn numeric_spellings_and_address_generations_are_canonical() {
    for number in [
        "",
        "01",
        "+1",
        "-1",
        "1.0",
        "1e0",
        " 1",
        "1 ",
        "١",
        "18446744073709551616",
    ] {
        let args = format!("{{\"since_seq\":{},\"max_events\":\"1\"}}", quote(number));
        assert_eq!(
            decode_request(document("tail", &args).as_bytes()),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
    for number in ["1", "1.0", "1e0", "-0", "null", "true"] {
        let args = format!("{{\"since_seq\":{number},\"max_events\":\"1\"}}");
        assert_eq!(
            decode_request(document("tail", &args).as_bytes()),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
    for address in [
        "fa://tenant/evidence/item@01",
        "fa://tenant/evidence/item@+1",
        "fa://tenant/evidence/item@1@2",
        "fa://tenant/evidence/item/other",
        "fa://tenant/evidence/item?secret=x",
        "display#alias",
        "fa://tenant/evidence/%69tem",
    ] {
        assert_eq!(
            decode_request(
                READ.replace("fa://tenant/evidence/item@1", address)
                    .as_bytes()
            ),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
    let zero = READ.replace("item@1", "item@0");
    assert_eq!(
        decode_request(zero.as_bytes()).unwrap().address.generation,
        Some(0)
    );
}

#[test]
fn native_proposal_validation_and_exact_request_binding_precede_dispatch() {
    let valid = proposed();
    for key in [
        None,
        Some("2".to_owned()),
        Some("01".to_owned()),
        Some("".to_owned()),
    ] {
        let mut changed = valid.clone();
        changed.idempotency_key = key;
        assert_eq!(
            encode_request(&changed),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
    for generation in [None, Some(0)] {
        let mut changed = valid.clone();
        changed.address.generation = generation;
        assert_eq!(
            encode_request(&changed),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
    let bytes = encode_request(&valid).unwrap();
    for (from, to, error) in [
        (
            "\"request\":\"1\"",
            "\"request\":\"0\"",
            TowerDecodingError::MalformedRequest,
        ),
        (
            "\"adapter\":\"1\"",
            "\"adapter\":\"0\"",
            TowerDecodingError::MalformedRequest,
        ),
        (
            "\"deadline\":\"7\"",
            "\"deadline\":\"0\"",
            TowerDecodingError::MalformedRequest,
        ),
        (
            "\"units\":\"4\"",
            "\"units\":\"3\"",
            TowerDecodingError::Capacity,
        ),
        (
            "\"units\":\"4\"",
            "\"units\":\"0\"",
            TowerDecodingError::MalformedRequest,
        ),
        (
            "\"payload_hex\":\"00ff225c\"",
            "\"payload_hex\":\"00FF225C\"",
            TowerDecodingError::MalformedRequest,
        ),
        (
            "\"payload_hex\":\"00ff225c\"",
            "\"payload_hex\":\"0\"",
            TowerDecodingError::MalformedRequest,
        ),
    ] {
        assert_eq!(
            decode_request(bytes.replace(from, to).as_bytes()),
            Err(error)
        );
    }
    let mut empty = proposal();
    empty.payload.clear();
    empty.units = 1;
    round_trip(&request(
        Verb::Precheck,
        ReadArguments::Precheck {
            query: 1,
            proposal: empty,
        },
    ));
}

#[test]
fn relation_variants_and_optional_read_arguments_round_trip() {
    for relation in [
        None,
        Some(RelationKind::ClosureStrongRef),
        Some(RelationKind::ClosureWeakRef),
        Some(RelationKind::Provenance),
        Some(RelationKind::ThoughtGraphEdge),
        Some(RelationKind::SupportSetMember),
        Some(RelationKind::AnnotationTarget),
    ] {
        round_trip(&request(Verb::Related, ReadArguments::Related { relation }));
    }
    for verb in [Verb::Related, Verb::Tail, Verb::Next] {
        round_trip(&request(verb, ReadArguments::None));
    }
    round_trip(&request(Verb::Next, ReadArguments::Next { max_cost: 0 }));
    assert_eq!(
        decode_request(document("related", "{\"relation\":\"other\"}").as_bytes()),
        Err(TowerDecodingError::MalformedRequest)
    );
    for max_events in [0, 129, usize::MAX] {
        assert_eq!(
            encode_request(&request(
                Verb::Tail,
                ReadArguments::Tail {
                    since_seq: 0,
                    max_events
                }
            )),
            Err(TowerDecodingError::Capacity)
        );
    }
}

#[test]
fn replay_preserves_absence_empty_values_order_and_native_semantic_refusals() {
    let edits = vec![
        Intervention::Payload(vec![0, 255]),
        Intervention::Units(0),
        Intervention::Target(ResolvedTarget {
            adapter: 0,
            object: u64::MAX,
            contract_version: 0,
            expected_version: 0,
            generation: 0,
        }),
        Intervention::Key {
            key: 0,
            expected: None,
            replacement: Some(Vec::new()),
        },
        Intervention::Key {
            key: 0,
            expected: Some(Vec::new()),
            replacement: None,
        },
    ];
    let mut input = request(
        Verb::Replay,
        ReadArguments::Replay {
            interventions: edits,
        },
    );
    input.idempotency_key = Some("paired-1".to_owned());
    // Duplicate edits and an invalid semantic target remain native execution
    // errors: decoding may not turn those trials into free checked experiments.
    round_trip(&input);
    input.arguments = ReadArguments::Replay {
        interventions: Vec::new(),
    };
    round_trip(&input);
    input.arguments = ReadArguments::None;
    round_trip(&input);
    input.idempotency_key = None;
    assert_eq!(
        encode_request(&input),
        Err(TowerDecodingError::MalformedRequest)
    );
    for args in [
        "{\"interventions\":[{\"kind\":\"unknown\"}]}",
        "{\"interventions\":[{\"kind\":\"key\",\"key\":\"0\",\"expected_hex\":null,\"replacement_hex\":\"AA\"}]}",
    ] {
        assert_eq!(
            decode_request(document("replay", args).as_bytes()),
            Err(TowerDecodingError::MalformedRequest)
        );
    }
}

#[test]
fn replay_byte_and_count_caps_admit_the_complete_original_boundary() {
    let mut input = request(
        Verb::Replay,
        ReadArguments::Replay {
            interventions: vec![Intervention::Payload(vec![7; MAX_INTERVENTION_BYTES])],
        },
    );
    input.idempotency_key = Some("maximum".to_owned());
    round_trip(&input);
    input.arguments = ReadArguments::Replay {
        interventions: vec![Intervention::Key {
            key: 0,
            expected: Some(vec![7; MAX_INTERVENTION_BYTES / 2]),
            replacement: Some(vec![8; MAX_INTERVENTION_BYTES / 2]),
        }],
    };
    round_trip(&input);
    let encoded = encode_request(&input).unwrap();
    let over = encoded.replacen("\"replacement_hex\":\"", "\"replacement_hex\":\"00", 1);
    assert_eq!(
        decode_request(over.as_bytes()),
        Err(TowerDecodingError::Capacity)
    );
    input.arguments = ReadArguments::Replay {
        interventions: vec![Intervention::Target(proposal().target); MAX_INTERVENTIONS],
    };
    round_trip(&input);
    input.arguments = ReadArguments::Replay {
        interventions: vec![Intervention::Units(1); MAX_INTERVENTIONS + 1],
    };
    assert_eq!(encode_request(&input), Err(TowerDecodingError::Capacity));
    let many = format!(
        "{{\"interventions\":[{}]}}",
        vec!["{\"kind\":\"units\",\"units\":\"1\"}"; MAX_INTERVENTIONS + 1].join(",")
    );
    assert_eq!(
        decode_request(document("replay", &many).as_bytes()),
        Err(TowerDecodingError::Capacity)
    );
}

#[test]
fn parser_work_limits_and_identity_bounds_fail_before_unbounded_output() {
    let mut exact = READ.as_bytes().to_vec();
    exact.resize(MAX_REQUEST_BYTES, b' ');
    assert!(decode_request(&exact).is_ok());
    exact.push(b' ');
    assert_eq!(decode_request(&exact), Err(TowerDecodingError::Capacity));
    let nested = format!(
        "{}null{}",
        "[".repeat(MAX_REQUEST_DEPTH + 1),
        "]".repeat(MAX_REQUEST_DEPTH + 1)
    );
    assert_eq!(
        decode_request(nested.as_bytes()),
        Err(TowerDecodingError::Capacity)
    );
    let many_items = format!("[{}]", vec!["null"; MAX_REQUEST_ITEMS + 1].join(","));
    assert_eq!(
        decode_request(many_items.as_bytes()),
        Err(TowerDecodingError::Capacity)
    );
    let large_string = format!("\"{}\"", "x".repeat(MAX_REQUEST_STRING_BYTES + 1));
    assert_eq!(
        decode_request(large_string.as_bytes()),
        Err(TowerDecodingError::Capacity)
    );

    let mut label = decode_request(READ.as_bytes()).unwrap();
    label.operation_id = "😀".repeat(32);
    label.idempotency_key = Some("k".repeat(128));
    round_trip(&label);
    label.operation_id.push('x');
    assert_eq!(encode_request(&label), Err(TowerDecodingError::Capacity));
    assert_eq!(
        decode_request(READ.replace("read-1", &"x".repeat(129)).as_bytes()),
        Err(TowerDecodingError::Capacity)
    );
    assert_eq!(
        decode_request(READ.replace("read-1", "").as_bytes()),
        Err(TowerDecodingError::MalformedRequest)
    );
}

#[test]
fn typed_encoding_cannot_bypass_shape_address_or_payload_admission() {
    let mut input = decode_request(READ.as_bytes()).unwrap();
    input.arguments = ReadArguments::Tail {
        since_seq: 0,
        max_events: 1,
    };
    assert_eq!(
        encode_request(&input),
        Err(TowerDecodingError::MalformedRequest)
    );
    input.arguments = ReadArguments::None;
    input.address.tenant = "other/tenant".to_owned();
    assert_eq!(
        encode_request(&input),
        Err(TowerDecodingError::MalformedRequest)
    );
    let mut value = proposal();
    value.payload = vec![0; MAX_PAYLOAD_BYTES];
    value.units = MAX_PAYLOAD_BYTES as u64;
    round_trip(&request(
        Verb::Precheck,
        ReadArguments::Precheck {
            query: 1,
            proposal: value.clone(),
        },
    ));
    value.payload.push(0);
    value.units += 1;
    assert_eq!(
        encode_request(&request(
            Verb::Precheck,
            ReadArguments::Precheck {
                query: 1,
                proposal: value
            }
        )),
        Err(TowerDecodingError::Capacity)
    );
}
