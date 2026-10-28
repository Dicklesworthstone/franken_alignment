//! Actual source-backed owner, separate actor channel, strict untrusted bytes.
use super::*;
use super::super::wire::{
    FilePrecheckWire, MAX_PRECHECK_EXCHANGES, MAX_PRECHECK_FRAME_BYTES,
    MAX_PRECHECK_RESPONSE_BYTES, PrecheckRequest, PrecheckResponseError,
    PrecheckWireError, PrecheckWireResponse, decode_precheck_request,
    decode_precheck_response, encode_precheck_request,
};
use crate::action::consequence::oversight::actor_wire::{ActorWire, Command, WireError, encode_command};

fn request(query: u64, payload: &[u8]) -> PrecheckRequest {
    PrecheckRequest { query, proposal: proposal(payload) }
}
fn exchange(wire: &mut FilePrecheckWire, request: &PrecheckRequest) -> PrecheckWireResponse {
    let response = wire.exchange(&encode_precheck_request(request).unwrap());
    let bytes = response.encode().unwrap();
    assert!(bytes.len() <= MAX_PRECHECK_RESPONSE_BYTES);
    let decoded = decode_precheck_response(&bytes, request.query, request.proposal.expected_policy_epoch).unwrap();
    assert_eq!(decoded, response);
    decoded
}

#[test]
fn wire_precheck_roundtrips_exact_outcomes_without_effects_and_real_submit_still_reviews() {
    for (payload, outcome, stage) in [
        (b"ok".as_slice(), FilePrecheckOutcome::PrecheckPassed, ActionState::Reviewing),
        (b"blocked".as_slice(), FilePrecheckOutcome::DeniedByPolicy, ActionState::Denied),
    ] {
        let root = Root::new(); let (host, source) = owner(&root);
        let (actor, mut supervisor) = host.into_actor_gateway();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot(1, b"ok"))).unwrap();
        let (port, grant) = disclose(&supervisor, 3);
        let mut wire = FilePrecheckWire::new(port, 4).unwrap();
        let files = root.files(); let before = supervisor.host().unwrap().inspect();
        let reads = source.status().read_attempts;
        let query = request(901, payload);
        let response = exchange(&mut wire, &query);
        known(response.result.unwrap(), outcome, 901);
        assert_eq!(grant.remaining_queries(), 2);
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
        assert_eq!(source.status().read_attempts, reads); assert_eq!(root.files(), files);
        let encoded = String::from_utf8(exchange(&mut wire, &query).encode().unwrap()).unwrap();
        assert!(encoded.contains("\"precheck\":true"));
        assert_eq!(encoded.contains("\"review_required\":true"), outcome == FilePrecheckOutcome::PrecheckPassed);
        for secret in ["helper", "not disclosed", "ExactValue", "snapshot", "sequence", "permit", "attempt"] {
            assert!(!encoded.contains(secret), "{secret} leaked in {encoded}");
        }
        // Same label remains an entirely new real request. No admission snapshot
        // was consumed and a pass still needs ordinary empirical/human review.
        let ticket = actor.submit(901, &query.proposal).unwrap();
        let host = supervisor.host().unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&1], stage);
        assert_eq!(host.inspect().control.ledger.available, 100);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.inspect().executions, 0);
        assert!(host.inspect().control.decisions.is_empty());
        if stage == ActionState::Reviewing {
            assert_eq!(actor.poll(&ticket), Knowledge::Pending { request: 901 });
        } else {
            assert!(matches!(actor.poll(&ticket), Knowledge::Known { value: ActorOutcome::Denied, .. }));
        }
        assert_eq!(exchange(&mut wire, &query).result.unwrap(),
            Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
    }
}

#[test]
fn untrusted_frames_are_bounded_charged_and_cannot_renew_shared_evaluation_budget() {
    let root = Root::new(); let (host, _) = owner(&root); let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = disclose(&supervisor, 1); let files = root.files();
    assert_eq!(FilePrecheckWire::new(port.clone(), 0).unwrap_err(), PrecheckWireError::Capacity);
    assert_eq!(FilePrecheckWire::new(port.clone(), MAX_PRECHECK_EXCHANGES + 1).unwrap_err(), PrecheckWireError::Capacity);
    let mut first = FilePrecheckWire::new(port.clone(), 3).unwrap();
    assert_eq!(first.exchange(b"{").query, None);
    assert_eq!(first.remaining_exchanges(), 2); assert_eq!(grant.remaining_queries(), 1);
    let huge = vec![b' '; MAX_PRECHECK_FRAME_BYTES + 1];
    let failure = first.exchange(&huge);
    assert_eq!(failure, PrecheckWireResponse { query: None, result: Err(PrecheckWireError::Capacity) });
    assert_eq!(decode_precheck_response(&failure.encode().unwrap(), 9, 0).unwrap(), failure);
    assert_eq!(first.remaining_exchanges(), 1); assert_eq!(grant.remaining_queries(), 1);
    known(exchange(&mut first, &request(9, b"ok")).result.unwrap(), FilePrecheckOutcome::PrecheckPassed, 9);
    for _ in 0..2 {
        assert_eq!(first.exchange(b"{}"), PrecheckWireResponse { query: None, result: Err(PrecheckWireError::Capacity) });
    }
    let mut reopened = FilePrecheckWire::new(port, 1).unwrap();
    assert_eq!(exchange(&mut reopened, &request(9, b"ok")).result, Err(PrecheckWireError::Capacity));
    assert_eq!(grant.remaining_queries(), 0); assert_eq!(root.files(), files);
}

#[test]
fn wire_revocation_staleness_and_owner_drop_remain_distinct_without_private_revisions() {
    let root = Root::new(); let (host, _) = owner(&root); let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = disclose(&supervisor, 4); let mut wire = FilePrecheckWire::new(port, 4).unwrap();
    let mut stale = request(11, b"ok"); stale.proposal.expected_policy_epoch = 99;
    assert_eq!(exchange(&mut wire, &stale).result.unwrap(), Knowledge::Stale { generation: 99, current: 0 });
    let before = root.files();
    grant.revoke();
    assert_eq!(exchange(&mut wire, &request(11, b"ok")).result.unwrap(),
        Knowledge::Withheld { authority_required: "exact_policy_disclosure" });
    assert_eq!(root.files(), before);
    let (fresh, _grant) = disclose(&supervisor, 4); let mut fresh = FilePrecheckWire::new(fresh, 4).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap(); let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    assert_eq!(exchange(&mut fresh, &request(11, b"ok")).result.unwrap(),
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
    let (live, _live_grant) = disclose(&supervisor, 1); let mut live = FilePrecheckWire::new(live, 1).unwrap();
    let files = root.files(); drop(supervisor);
    assert_eq!(exchange(&mut live, &request(11, b"ok")).result.unwrap(),
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable });
    assert_eq!(root.files(), files);
    // The byte adapter, just like the native weak port, cannot retain the lock.
    let (_reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
}

#[test]
fn request_codec_preserves_all_integer_and_payload_bits_at_boundaries() {
    let mut query = request(u64::MAX, &(0..=255_u8).collect::<Vec<_>>());
    query.proposal.target = ResolvedTarget { adapter: u64::MAX, object: u64::MAX,
        contract_version: u64::MAX, expected_version: u64::MAX, generation: u64::MAX };
    query.proposal.units = u64::MAX; query.proposal.deadline = ElapsedTick(u64::MAX);
    query.proposal.expected_policy_epoch = u64::MAX;
    let bytes = encode_precheck_request(&query).unwrap();
    assert_eq!(decode_precheck_request(&bytes).unwrap(), query);
    query.proposal.payload = vec![0xff; MAX_PAYLOAD_BYTES];
    let maximum = encode_precheck_request(&query).unwrap();
    assert!(maximum.len() <= MAX_PRECHECK_FRAME_BYTES);
    assert_eq!(decode_precheck_request(&maximum).unwrap(), query);
    query.proposal.payload.push(0);
    assert_eq!(encode_precheck_request(&query), Err(PrecheckWireError::Capacity));
    let empty = request(1, b"");
    assert_eq!(decode_precheck_request(&encode_precheck_request(&empty).unwrap()).unwrap(), empty);
}

#[test]
fn request_codec_refuses_ambiguous_shapes_unknown_fields_and_submission_commands() {
    let good = String::from_utf8(encode_precheck_request(&request(9, b"ok")).unwrap()).unwrap();
    for bad in [
        good.replace("\"query\":\"9\"", "\"query\":\"09\""),
        good.replace("\"query\":\"9\"", "\"query\":9"),
        good.replace("\"query\":\"9\"", "\"query\":\"0\""),
        good.replace("\"query\":\"9\"", "\"query\":\"18446744073709551616\""),
        good.replace("\"query\":\"9\"", "\"query\":\"9\",\"query\":\"9\""),
        good.replace("\"units\":\"8\"", "\"units\":\"1\""),
        good.replace("\"deadline\":\"100\"", "\"deadline\":\"0\""),
        good.replace("\"object\":\"11\"", "\"object\":\"0\""),
        good.replace("\"expected_policy_epoch\":\"0\"", "\"expected_policy_epoch\":\"-1\""),
        good.replace("\"payload_hex\":\"6f6b\"", "\"payload_hex\":\"6F6B\""),
        good.replace("\"payload_hex\":\"6f6b\"", "\"payload_hex\":\"6f6\""),
        good.replace("\"operation\":\"precheck\"", "\"operation\":\"submit\""),
        good.replace("\"operation\":\"precheck\"", "\"operation\":\"precheck\",\"helper\":\"r\""),
        good.replace("\"target\":{", "\"scope\":{},\"target\":{"),
        format!("{good}{{}}"),
    ] { assert!(decode_precheck_request(bad.as_bytes()).is_err(), "accepted {bad}"); }
    assert_eq!(decode_precheck_request(good.replace("fa.actor-precheck/1", "fa.actor-precheck/2").as_bytes()),
        Err(PrecheckWireError::UnsupportedVersion));
    let malformed = [0xff]; assert!(decode_precheck_request(&malformed).is_err());
    let submit = encode_command(&Command::Submit { request: 9, proposal: proposal(b"ok") }).unwrap();
    assert!(decode_precheck_request(&submit).is_err());
    let root = Root::new(); let (host, _) = owner(&root); let (actor, supervisor) = host.into_actor_gateway();
    let mut ordinary = ActorWire::new(actor); let files = root.files();
    assert_eq!(ordinary.exchange(good.as_bytes()).result, Err(WireError::MalformedRequest));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0); assert_eq!(root.files(), files);
}

fn passing_response() -> PrecheckWireResponse {
    PrecheckWireResponse { query: Some(9), result: Ok(Knowledge::Known {
        value: FilePrecheckOutcome::PrecheckPassed,
        basis: FilePrecheckBasis { disclosure: 81, query: 9, policy_epoch: 0, observed_at: ElapsedTick(1) },
    }) }
}

#[test]
fn response_codec_requires_review_and_binds_original_query_and_epoch() {
    let response = passing_response(); let bytes = response.encode().unwrap();
    assert_eq!(decode_precheck_response(&bytes, 9, 0).unwrap(), response);
    assert_eq!(decode_precheck_response(&bytes, 10, 0), Err(PrecheckResponseError::Binding));
    assert_eq!(decode_precheck_response(&bytes, 9, 1), Err(PrecheckResponseError::Binding));
    let good = String::from_utf8(bytes).unwrap();
    for bad in [
        good.replace("\"review_required\":true", "\"review_required\":false"),
        good.replace("\"review_required\":true,", ""),
        good.replace("\"precheck\":true", "\"precheck\":false"),
        good.replace("\"precheck\":true", "\"precheck\":true,\"permit\":\"yes\""),
        good.replace("\"value\":\"precheck_passed\"", "\"value\":\"executed\""),
        good.replace("\"disclosure\":\"81\"", "\"disclosure\":\"0\""),
        good.replace("\"observed_at\":\"1\"", "\"observed_at\":1"),
        good.replace("\"basis\":{", "\"helper\":\"r\",\"basis\":{"),
        good.replace("\"query\":\"9\"", "\"query\":\"9\",\"query\":\"9\""),
        format!("{good}{{}}"),
    ] { assert!(decode_precheck_response(bad.as_bytes(), 9, 0).is_err(), "accepted {bad}"); }
    // Change only the nested basis or only the outer envelope.
    for bad in [
        good.replacen("\"query\":\"9\"", "\"query\":\"10\"", 1),
        good.replace("\"policy_epoch\":\"0\"", "\"policy_epoch\":\"1\""),
        good.replace("\"query\":\"9\"", "\"query\":null"),
    ] { assert!(decode_precheck_response(bad.as_bytes(), 9, 0).is_err()); }
    assert_eq!(decode_precheck_response(&vec![b' '; MAX_PRECHECK_RESPONSE_BYTES + 1], 9, 0),
        Err(PrecheckResponseError::Capacity));
}

#[test]
fn response_encoder_and_decoder_never_invent_outcomes_or_unsupported_knowledge() {
    for observation in [
        Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable },
        Knowledge::Withheld { authority_required: "exact_policy_disclosure" },
        Knowledge::Stale { generation: 0, current: 1 },
    ] {
        let response = PrecheckWireResponse { query: Some(9), result: Ok(observation) };
        assert_eq!(decode_precheck_response(&response.encode().unwrap(), 9, 0).unwrap(), response);
    }
    for observation in [
        Knowledge::Pending { request: 9 },
        Knowledge::Absent { closed_domain: 1, frontier: 1 },
        Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown },
        Knowledge::Withheld { authority_required: "own_request" },
        Knowledge::Stale { generation: 0, current: 0 },
    ] {
        assert!(PrecheckWireResponse { query: Some(9), result: Ok(observation) }.encode().is_err());
    }
    let mut unbound = passing_response(); unbound.query = None; assert!(unbound.encode().is_err());
    unbound.query = Some(10); assert!(unbound.encode().is_err());
    assert!(PrecheckWireResponse { query: None, result: Err(PrecheckWireError::Unavailable) }.encode().is_err());
    for error in [PrecheckWireError::MalformedRequest, PrecheckWireError::UnsupportedVersion,
        PrecheckWireError::Capacity, PrecheckWireError::Unavailable, PrecheckWireError::Withheld] {
        let response = PrecheckWireResponse { query: Some(9), result: Err(error) };
        assert_eq!(decode_precheck_response(&response.encode().unwrap(), 9, 0).unwrap(), response);
    }
    let stale = PrecheckWireResponse { query: Some(9),
        result: Ok(Knowledge::Stale { generation: 4, current: 5 }) }.encode().unwrap();
    assert_eq!(decode_precheck_response(&stale, 9, 0), Err(PrecheckResponseError::Binding));
}
