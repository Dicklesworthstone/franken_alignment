use super::*;
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::action::{Purpose, Scope};
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::observation_algebra::{Knowledge, UnknownReason};
use fa_reference::tower::publication::{PublicationLimits, PublicationResponse, PublicationResult};
use fa_reference::tower::{ReasonCode, ResponseStatus, TowerEnvelope, TowerResponse};

fn profile() -> Profile {
    Profile {
        request: 7,
        scope: Scope {
            tenant: 1,
            principal: 2,
            run: 3,
            branch: 4,
            authority: 5,
            purpose: Purpose::Effect,
        },
        socket: "/unopened/tower.sock".into(),
        supervisor: PeerPolicy::new(1, 1, None).unwrap(),
        actor: PeerPolicy::new(1, 1, None).unwrap(),
        candidates: 2,
        connections: 2,
        exchanges: 4,
        runtime_ms: 15_000,
        poll_ms: 1,
        reply_ms: 1,
    }
}

fn request() -> TowerRequest {
    let mut request = TowerRequest::new(
        "read-7",
        Address::new("1", Noun::Attempt, "request-3-2-4-5-7", Some(11)).unwrap(),
        Verb::Get,
    );
    request.idempotency_key = Some("visible-key".to_owned());
    request
}

fn response(request: &TowerRequest) -> String {
    original_response(request).to_json().unwrap()
}

fn original_response(request: &TowerRequest) -> PublicationResponse {
    // An explicitly unavailable fixture, encoded by the original public
    // encoder. This is protocol/correlation data, not evidence of an effect.
    TowerResponse {
        envelope: TowerEnvelope {
            schema_version: RESPONSE_SCHEMA,
            operation_id: request.operation_id.clone(),
            status: ResponseStatus::Error,
            reason_code: Some(ReasonCode::MissingRequiredObservation),
            address: request.address.clone(),
            generation: Knowledge::unknown(UnknownReason::Unavailable),
            control_seq: Knowledge::unknown(UnknownReason::Unavailable),
            source_frontiers: Knowledge::unknown(UnknownReason::Unavailable),
            missing: vec![request.address.clone()],
            limits: PublicationLimits::default(),
            budgets_remaining: Knowledge::unknown(UnknownReason::Unavailable),
            realized_cost: Knowledge::unknown(UnknownReason::Unavailable),
            affordances: Knowledge::unknown(UnknownReason::Unavailable),
            retry_safe: true,
            idempotency_key: request.idempotency_key.clone(),
            would_change_this: vec![request.address.clone()],
        },
        result: PublicationResult::Refused(Knowledge::unknown(UnknownReason::Unavailable)),
    }
}

#[test]
fn native_explanation_profile_is_carried_only_for_the_correlated_explain_request() {
    use fa_reference::action::consequence::oversight::actor::{
        Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
    };
    let mut query = request();
    query.verb = Verb::Explain;
    let mut original = original_response(&query);
    original.result = PublicationResult::Explanation(Ok(ActorKnowledge::Unknown {
        reason: ActorUnknown::ControllerUnavailable,
    }));
    let bytes = original.to_json().unwrap();
    validate_response(bytes.as_bytes(), &query).unwrap();
    for verb in [Verb::Get, Verb::Situation, Verb::WhyHeld, Verb::Next] {
        let mut different = query.clone();
        different.verb = verb;
        assert!(validate_response(bytes.as_bytes(), &different).is_err());
    }
    query.operation_id = "a-different-observation".into();
    assert!(validate_response(bytes.as_bytes(), &query).is_err());
}

#[test]
fn original_unknown_response_is_opaque_but_correlation_and_envelope_are_required() {
    let request = request();
    let reply = response(&request);
    validate_response(reply.as_bytes(), &request).unwrap();
    for changed in [
        reply.replace("read-7", "other-read"),
        reply.replace("visible-key", "other-key"),
        reply.replace("fa.tower_response/1", "fa.tower_response/2"),
        reply.replace("request-3-2-4-5-7", "request-3-2-4-5-8"),
        reply.replacen("\"envelope\":{", "\"envelope\":{\"operator\":true,", 1),
        reply.replacen(
            "\"retry_safe\":true",
            "\"retry_safe\":true,\"retry_safe\":false",
            1,
        ),
        reply.replace("\"kind\":\"refused\"", "\"kind\":\"capabilities\""),
        format!("{reply}null"),
    ] {
        assert!(validate_response(changed.as_bytes(), &request).is_err());
    }
}

#[test]
fn short_io_and_flush_barrier_spend_one_finite_budget_without_duplicating_bytes() {
    struct ShortWriter {
        bytes: Vec<u8>,
        flushes: usize,
    }
    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let count = bytes.len().min(2);
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            if self.flushes == 1 {
                Err(io::ErrorKind::WouldBlock.into())
            } else {
                Ok(())
            }
        }
    }
    let mut budget = Budget::new(&profile(), Instant::now());
    let mut writer = ShortWriter {
        bytes: Vec::new(),
        flushes: 0,
    };
    budget.write_frame(&mut writer, b"request\n").unwrap();
    assert_eq!(writer.bytes, b"request\n");
    assert_eq!(writer.flushes, 2);
    assert_eq!(budget.calls, 6);
    assert_eq!(budget.written, 8);
    let mut reader = io::Cursor::new(b"response\n");
    assert_eq!(budget.read_frame(&mut reader).unwrap(), b"response");
    assert_eq!(budget.read, 9);
    assert_eq!(budget.calls, 7);

    budget.calls = MAX_CLIENT_IO_CALLS - 1;
    assert!(budget.write_frame(&mut writer, b"x").is_err());
    assert_eq!(writer.bytes, b"request\nx");
    assert_eq!(
        writer.flushes, 2,
        "exhaustion cannot run another flush or resend"
    );
    assert_eq!(budget.calls, MAX_CLIENT_IO_CALLS);
}

#[test]
fn truncated_oversized_and_unsolicited_frames_never_become_complete_output() {
    for bytes in [
        b"unterminated".to_vec(),
        b"one\ntwo\n".to_vec(),
        vec![b'x'; MAX_REPLY_BYTES + 1],
    ] {
        let mut budget = Budget::new(&profile(), Instant::now());
        assert!(budget.read_frame(&mut io::Cursor::new(bytes)).is_err());
    }
    let mut profile = profile();
    profile.runtime_ms = 0;
    let mut budget = Budget::new(&profile, Instant::now());
    let mut untouched = io::Cursor::new(Vec::new());
    assert!(budget.write_frame(&mut untouched, b"private").is_err());
    assert!(untouched.into_inner().is_empty());
    assert_eq!(budget.calls, 0);
}

#[test]
fn actor_only_cli_owns_all_options_before_profile_or_supervisor_reads() {
    for values in [
        vec!["tower-exchange"],
        vec!["tower-exchange", "missing-profile"],
        vec![
            "tower-exchange",
            "missing-profile",
            "request",
            "--credibility-activation",
            "private",
        ],
        vec![
            "tower-exchange",
            "missing-profile",
            "request",
            "--guarded",
            "private",
        ],
        vec![
            "tower-exchange",
            "missing-profile",
            "request",
            "--precheck-profile",
            "private",
        ],
        vec![
            "tower-exchange",
            "missing-profile",
            "request",
            "--tower-generation",
            "1",
        ],
    ] {
        let args = values.into_iter().map(str::to_owned).collect();
        let mut output = Vec::new();
        let error = crate::command_with_output(args, &mut output).unwrap_err();
        assert!(error.contains("tower-exchange ACTOR_PROFILE"), "{error}");
        assert!(output.is_empty());
    }
}
