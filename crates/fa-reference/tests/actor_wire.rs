//! Public wire admission and real reference control, not authenticated transport.
#[path = "support/actor_gateway.rs"]
mod support;

use fa_reference::action::consequence::oversight::actor::{ActorBasis, ActorOutcome, BasisSource, IntakeLimits, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, MAX_FRAME_BYTES, MAX_RESPONSE_BYTES, WireError, WireResponse, decode_command, encode_command};
use fa_reference::action::{ElapsedTick, MAX_PAYLOAD_BYTES};
use fa_reference::strict_json::{self, Limits};
use support::{fixture, proposal, review, snapshot};

fn submit(id: u64) -> Vec<u8> { encode_command(&Command::Submit { request: id, proposal: proposal() }).unwrap() }
fn poll(id: u64) -> Vec<u8> { encode_command(&Command::Poll { request: id }).unwrap() }

#[test]
fn manual_golden_and_arbitrary_bytes_preserve_all_u64_bits() {
    let golden = br#"{"version":1,"operation":"submit","request":"42","target":{"adapter":"1","object":"1","contract_version":"1","expected_version":"1","generation":"1"},"payload_hex":"7075626c697368","units":"16","deadline":"100","expected_policy_epoch":"0"}"#;
    let expected = Command::Submit { request: 42, proposal: proposal() };
    assert_eq!(decode_command(golden), Ok(expected.clone()));
    assert_eq!(encode_command(&expected).unwrap(), golden);
    for size in [0, 1, 255, 256, MAX_PAYLOAD_BYTES] {
        let mut p = proposal();
        p.payload = (0..size).map(|i| (i % 256) as u8).collect();
        p.units = u64::MAX; p.deadline = ElapsedTick(u64::MAX); p.expected_policy_epoch = u64::MAX;
        p.target.adapter = u64::MAX; p.target.object = u64::MAX; p.target.expected_version = u64::MAX;
        p.target.contract_version = u64::MAX; p.target.generation = u64::MAX;
        let command = Command::Submit { request: u64::MAX, proposal: p };
        let bytes = encode_command(&command).unwrap();
        assert!(bytes.len() <= MAX_FRAME_BYTES);
        assert_eq!(decode_command(&bytes), Ok(command));
    }
}

#[test]
fn privilege_fields_duplicate_keys_and_lossy_numeric_spellings_refuse() {
    let good = String::from_utf8(submit(42)).unwrap();
    for mutated in [
        good.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
        good.replacen("\"request\":\"42\"", "\"request\":42", 1),
        good.replacen("\"request\":\"42\"", "\"request\":\"042\"", 1),
        good.replacen("\"request\":\"42\"", "\"request\":\"18446744073709551616\"", 1),
        good.replacen("\"request\":\"42\"", "\"request\":\"4.2e1\"", 1),
        good.replacen("\"request\":\"42\"", "\"request\":\"+42\"", 1),
        good.replacen("\"adapter\":\"1\"", "\"adapter\":\"0\"", 1),
        good.replacen("\"units\":\"16\"", "\"units\":\"0\"", 1),
        good.replacen("\"payload_hex\":\"7075626c697368\"", "\"payload_hex\":\"0A\"", 1),
        good.replacen("\"payload_hex\":\"7075626c697368\"", "\"payload_hex\":\"f\"", 1),
        good.replacen("\"operation\":\"submit\"", "\"operation\":\"authorize\"", 1),
        good.replacen("\"version\":1", "\"version\":1.0", 1),
        good.replacen("\"version\":1", "\"version\":1,\"scope\":{}", 1),
        good.replacen("\"version\":1", "\"version\":1,\"permit\":\"approved\"", 1),
        good.replacen("\"version\":1", "\"version\":1,\"verdict\":\"allow\"", 1),
        good.replacen("\"version\":1", "\"version\":1,\"evidence_root\":\"trusted\"", 1),
        good.replacen("\"generation\":\"1\"", "\"generation\":\"1\",\"path\":\"/secret\"", 1),
    ] {
        assert_eq!(decode_command(mutated.as_bytes()), Err(WireError::MalformedRequest), "{mutated}");
    }
    assert_eq!(decode_command(good.replace("\"version\":1", "\"version\":2").as_bytes()), Err(WireError::UnsupportedVersion));
    assert_eq!(decode_command(&vec![b' '; MAX_FRAME_BYTES + 1]), Err(WireError::Capacity));
    assert_eq!(decode_command(b"\xff"), Err(WireError::MalformedRequest));
    assert_eq!(decode_command(&submit(42)).unwrap().request(), 42);
}

#[test]
fn exact_retries_do_not_queue_again_and_changed_payload_cannot_replace_reviewed_bytes() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut wire = ActorWire::new(port.clone());
    assert_eq!(wire.exchange(&submit(42)).result, Ok(Knowledge::Pending { request: 42 }));
    assert_eq!(wire.exchange(&submit(42)).result, Ok(Knowledge::Pending { request: 42 }));
    assert_eq!(supervisor.accept_next(&snapshot()).unwrap().unwrap().request, 42);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    let mut changed = proposal(); changed.payload[0] ^= 1;
    let command = encode_command(&Command::Submit { request: 42, proposal: changed }).unwrap();
    assert_eq!(wire.exchange(&command).result, Err(WireError::IdempotencyConflict));
    let mut reconnected = ActorWire::new(port);
    assert!(matches!(reconnected.exchange(&poll(42)).result, Ok(Knowledge::Withheld { .. })));
    assert_eq!(reconnected.exchange(&submit(42)).result, Ok(Knowledge::Pending { request: 42 }));
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
}

#[test]
fn successful_wire_submission_waits_for_congress_then_publishes_once() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let mut wire = ActorWire::new(port);
    wire.exchange(&submit(42)).result.unwrap();
    assert!(supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap().is_some());
    assert!(supervisor.authorize_request(42, None, &snapshot()).is_err());
    let inputs = review(&mut supervisor, 42, 1);
    let permit = supervisor.authorize_request(42, Some(&inputs), &snapshot()).unwrap();
    let pending = wire.exchange(&poll(42));
    assert_eq!(pending.result, Ok(Knowledge::Pending { request: 42 }));
    let message = supervisor.dispatch_request(42, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).unwrap();
    assert_eq!(wire.exchange(&poll(42)).result, Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
    let receipt = endpoint.deliver(&message).unwrap();
    supervisor.accept_receipt(receipt).unwrap();
    let response = wire.exchange(&poll(42));
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(endpoint.payload(), b"publish"); assert_eq!(endpoint.execution_count(), 1);
    assert!(wire.exchange(&submit(42)).result.is_ok());
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    assert!(supervisor.dispatch_request(42, DispatchKeys::single(&permit), Some(&inputs), &snapshot()).is_err());
    for visible in [String::from_utf8(response.encode()).unwrap(), format!("{wire:?}")] {
        for secret in ["secret-helper", "secret-model-profile", "secret-detector-question", "secret-cohort", "secret-salt"] {
            assert!(!visible.contains(secret));
        }
    }
}

#[test]
fn cancellation_is_a_request_not_a_refund_and_foreign_ids_are_withheld() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let (foreign, _foreign_supervisor, _) = fixture(IntakeLimits::default());
    let mut wire = ActorWire::new(port); let mut other = ActorWire::new(foreign);
    wire.exchange(&submit(42)).result.unwrap();
    assert!(matches!(other.exchange(&poll(42)).result, Ok(Knowledge::Withheld { .. })));
    let cancel = encode_command(&Command::Cancel { request: 42 }).unwrap();
    assert_eq!(other.exchange(&cancel).result, Err(WireError::Withheld));
    assert_eq!(wire.exchange(&cancel).result, Ok(Knowledge::Pending { request: 42 }));
    assert!(supervisor.accept_next(&snapshot()).unwrap().unwrap().result.unwrap().is_none());
    assert!(matches!(wire.exchange(&poll(42)).result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert!(supervisor.broker().inspect().ledger.stages.is_empty());
}

#[test]
fn all_epistemic_variants_have_bounded_valid_distinct_json_without_untrusted_strings() {
    let states = [
        Knowledge::Known { value: ActorOutcome::Executed, basis: ActorBasis { request: u64::MAX, generation: u64::MAX, source: BasisSource::ControlLedger } },
        Knowledge::Pending { request: u64::MAX },
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable },
        Knowledge::Withheld { authority_required: "\"evil\":true" },
        Knowledge::Stale { generation: u64::MAX, current: u64::MAX },
        Knowledge::Absent { closed_domain: u64::MAX, frontier: u64::MAX },
    ];
    let mut names = std::collections::BTreeSet::new();
    for state in states {
        let encoded = WireResponse { request: Some(u64::MAX), result: Ok(state) }.encode();
        assert!(encoded.len() <= MAX_RESPONSE_BYTES);
        assert!(!String::from_utf8_lossy(&encoded).contains("evil"));
        let json = strict_json::parse(&encoded, Limits::default()).unwrap();
        assert_eq!(json.get("request").unwrap().as_str(), Some("18446744073709551615"));
        names.insert(json.get("knowledge").unwrap().get("state").unwrap().as_str().unwrap().to_owned());
    }
    assert_eq!(names.len(), 6);
    let golden = br#"{"version":1,"request":"42","status":"ok","knowledge":{"state":"pending","request":"42"}}"#;
    assert_eq!(WireResponse { request: Some(42), result: Ok(Knowledge::Pending { request: 42 }) }.encode(), golden);
}
