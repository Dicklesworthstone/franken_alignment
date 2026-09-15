use fa_reference::action::consequence::oversight::actor::{ActorBasis, ActorOutcome, BasisSource, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{WireError, WireResponse, MAX_RESPONSE_BYTES, decode_response, ResponseError};

#[test]
fn existing_encoder_observations_and_errors_decode_without_losing_identifier_bits() {
    for request in [1, 9_007_199_254_740_993, u64::MAX] {
        let mut cases = vec![Knowledge::Pending { request },
            Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable },
            Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown },
            Knowledge::Withheld { authority_required: "own_request" },
            Knowledge::Stale { generation: 0, current: u64::MAX },
            Knowledge::Absent { closed_domain: u64::MAX, frontier: 0 }];
        for (value, source) in [(ActorOutcome::NotAdmitted, BasisSource::Intake),
            (ActorOutcome::CancelledBeforeDispatch, BasisSource::Intake),
            (ActorOutcome::CancelledBeforeDispatch, BasisSource::ControlLedger),
            (ActorOutcome::Denied, BasisSource::ControlLedger),
            (ActorOutcome::Executed, BasisSource::ControlLedger),
            (ActorOutcome::ConfirmedNotExecuted, BasisSource::ControlLedger)] {
            cases.push(Knowledge::Known { value, basis: ActorBasis { request, generation: u64::MAX, source } });
        }
        for knowledge in cases {
            let response = WireResponse { request: Some(request), result: Ok(knowledge) };
            assert_eq!(decode_response(&response.encode()).unwrap(), response);
        }
        for error in [WireError::MalformedRequest, WireError::UnsupportedVersion, WireError::IdempotencyConflict,
            WireError::Capacity, WireError::Unavailable, WireError::Withheld] {
            let response = WireResponse { request: Some(request), result: Err(error) };
            assert_eq!(decode_response(&response.encode()).unwrap(), response);
        }
    }
    for error in [WireError::MalformedRequest, WireError::UnsupportedVersion, WireError::Capacity] {
        let response = WireResponse { request: None, result: Err(error) };
        assert_eq!(decode_response(&response.encode()).unwrap(), response);
    }
}

#[test]
fn independent_literal_vector_decodes_a_terminal_outcome_not_permission() {
    let bytes = br#"{"version":1,"request":"18446744073709551615","status":"ok","knowledge":{"state":"known","value":"executed","basis":{"request":"18446744073709551615","generation":"3","source":"control_ledger"}}}"#;
    assert_eq!(decode_response(bytes).unwrap(), WireResponse { request: Some(u64::MAX), result: Ok(Knowledge::Known {
        value: ActorOutcome::Executed, basis: ActorBasis { request: u64::MAX, generation: 3, source: BasisSource::ControlLedger },
    }) });
    for cut in 0..bytes.len() { assert!(decode_response(&bytes[..cut]).is_err()); }
}

#[test]
fn unknown_permission_fields_variants_duplicate_keys_and_noncanonical_ids_refuse() {
    let valid = r#"{"version":1,"request":"7","status":"ok","knowledge":{"state":"pending","request":"7"}}"#;
    assert!(decode_response(valid.as_bytes()).is_ok());
    let mut invalid = vec![
        valid.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
        valid.replace("\"pending\"", "\"approved\""),
        valid.replacen("\"status\":\"ok\"", "\"status\":\"ok\",\"permit\":\"go\"", 1),
        valid.replace("\"pending\"", "\"executed\""),
        valid.replacen("\"7\"", "7", 1),
        valid.replacen("\"7\"", "\"07\"", 1),
        valid.replacen("\"7\"", "\"+7\"", 1),
        valid.replacen("\"7\"", "\"18446744073709551616\"", 1),
        valid.replacen("\"7\"", "\"0\"", 1),
        format!("{valid}{{}}"),
    ];
    invalid.push(valid.replace("\"version\":1", "\"version\":1.0"));
    for bytes in invalid { assert!(decode_response(bytes.as_bytes()).is_err(), "{bytes}"); }
    assert_eq!(decode_response(valid.replace("\"version\":1", "\"version\":2").as_bytes()), Err(ResponseError::UnsupportedVersion));
    assert_eq!(decode_response(&vec![b' '; MAX_RESPONSE_BYTES + 1]), Err(ResponseError::Capacity));
    assert!(decode_response(&[255]).is_err());
}

#[test]
fn cross_request_bases_and_impossible_provenance_cannot_be_laundered() {
    for response in [
        WireResponse { request: Some(1), result: Ok(Knowledge::Pending { request: 2 }) },
        WireResponse { request: None, result: Ok(Knowledge::Pending { request: 1 }) },
        WireResponse { request: None, result: Err(WireError::IdempotencyConflict) },
        WireResponse { request: Some(1), result: Ok(Knowledge::Known { value: ActorOutcome::Executed,
            basis: ActorBasis { request: 2, generation: 1, source: BasisSource::ControlLedger } }) },
        WireResponse { request: Some(1), result: Ok(Knowledge::Known { value: ActorOutcome::Executed,
            basis: ActorBasis { request: 1, generation: 1, source: BasisSource::Intake } }) },
        WireResponse { request: Some(1), result: Ok(Knowledge::Known { value: ActorOutcome::NotAdmitted,
            basis: ActorBasis { request: 1, generation: 1, source: BasisSource::ControlLedger } }) },
    ] { assert_eq!(decode_response(&response.encode()), Err(ResponseError::Binding)); }
}
