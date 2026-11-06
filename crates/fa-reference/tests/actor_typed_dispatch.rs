//! Structured and byte commands share the original actor gateway and tickets.
//! Successful publication still requires the original independent review path.
#[path = "support/actor_gateway.rs"]
mod support;

use fa_reference::action::consequence::oversight::DispatchKeys;
use fa_reference::action::consequence::oversight::actor::{
    ActorOutcome, ActorProposal, BasisSource, IntakeLimits, Knowledge, MAX_ACTOR_REQUESTS,
    UnknownReason,
};
use fa_reference::action::consequence::oversight::actor_wire::{
    ActorWire, Command, WireError, WireResponse, encode_command,
};
use fa_reference::action::{ElapsedTick, MAX_PAYLOAD_BYTES};
use support::{fixture, proposal, review, snapshot};

fn submit(request: u64, proposal: ActorProposal) -> Command {
    Command::Submit { request, proposal }
}

#[test]
fn structured_validation_matches_byte_refusals_before_native_intake() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let before = supervisor.broker().inspect();
    let mut wire = ActorWire::new(port);
    let valid = submit(42, proposal());
    let bytes = String::from_utf8(encode_command(&valid).unwrap()).unwrap();
    let mut cases = Vec::new();

    for index in 0..7 {
        let mut candidate = proposal();
        let (field, original) = match index {
            0 => {
                candidate.target.adapter = 0;
                ("adapter", 1)
            }
            1 => {
                candidate.target.object = 0;
                ("object", 1)
            }
            2 => {
                candidate.target.contract_version = 0;
                ("contract_version", 1)
            }
            3 => {
                candidate.target.expected_version = 0;
                ("expected_version", 1)
            }
            4 => {
                candidate.target.generation = 0;
                ("generation", 1)
            }
            5 => {
                candidate.units = 0;
                ("units", 16)
            }
            _ => {
                candidate.deadline = ElapsedTick(0);
                ("deadline", 100)
            }
        };
        cases.push((
            submit(42, candidate),
            bytes.replacen(
                &format!("\"{field}\":\"{original}\""),
                &format!("\"{field}\":\"0\""),
                1,
            ),
            WireError::MalformedRequest,
        ));
    }
    cases.push((
        submit(0, proposal()),
        bytes.replacen("\"request\":\"42\"", "\"request\":\"0\"", 1),
        WireError::MalformedRequest,
    ));
    for command in [Command::Poll { request: 0 }, Command::Cancel { request: 0 }] {
        let operation = match command {
            Command::Poll { .. } => "poll",
            _ => "cancel",
        };
        cases.push((
            command,
            format!("{{\"version\":1,\"operation\":\"{operation}\",\"request\":\"0\"}}"),
            WireError::MalformedRequest,
        ));
    }
    let mut undercharged = proposal();
    undercharged.units = 6;
    cases.push((
        submit(42, undercharged),
        bytes.replacen("\"units\":\"16\"", "\"units\":\"6\"", 1),
        WireError::Capacity,
    ));
    let mut oversized = proposal();
    oversized.payload = vec![0xab; MAX_PAYLOAD_BYTES + 1];
    oversized.units = u64::MAX;
    let oversized_bytes = bytes
        .replacen("7075626c697368", &"ab".repeat(MAX_PAYLOAD_BYTES + 1), 1)
        .replacen("\"units\":\"16\"", "\"units\":\"18446744073709551615\"", 1);
    cases.push((submit(42, oversized), oversized_bytes, WireError::Capacity));

    for (command, document, reason) in cases {
        assert_eq!(encode_command(&command), Err(reason));
        let expected = WireResponse {
            request: None,
            result: Err(reason),
        };
        assert_eq!(wire.execute(command), expected);
        assert_eq!(wire.exchange(document.as_bytes()), expected);
        assert_eq!(supervisor.broker().inspect(), before);
        assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    }

    // Invalid attempts have acquired no ticket, request slot or ledger entry.
    assert!(matches!(
        wire.execute(Command::Poll { request: 42 }).result,
        Ok(Knowledge::Withheld { .. })
    ));
    assert_eq!(
        wire.execute(valid).result,
        Ok(Knowledge::Pending { request: 42 })
    );
    assert_eq!(
        supervisor
            .accept_next(&snapshot())
            .unwrap()
            .unwrap()
            .request,
        42
    );
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
}

#[test]
fn typed_and_byte_routes_share_retries_and_follow_actual_publication() {
    let (port, mut supervisor, mut endpoint) = fixture(IntakeLimits::default());
    let mut wire = ActorWire::new(port.clone());
    let command = submit(42, proposal());
    let bytes = encode_command(&command).unwrap();
    let pending = WireResponse {
        request: Some(42),
        result: Ok(Knowledge::Pending { request: 42 }),
    };
    assert_eq!(wire.execute(command.clone()), pending);
    assert_eq!(wire.exchange(&bytes), pending);
    assert_eq!(wire.execute(Command::Poll { request: 42 }), pending);
    let accepted = supervisor.accept_next(&snapshot()).unwrap().unwrap();
    assert_eq!(accepted.request, 42);
    assert!(accepted.result.unwrap().is_some());
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());

    let mut reconnected = ActorWire::new(port);
    assert!(matches!(
        reconnected.execute(Command::Poll { request: 42 }).result,
        Ok(Knowledge::Withheld { .. })
    ));
    assert_eq!(reconnected.execute(command.clone()), pending);
    let mut changed = proposal();
    changed.payload[0] ^= 1;
    assert_eq!(
        wire.execute(submit(42, changed)),
        WireResponse {
            request: Some(42),
            result: Err(WireError::IdempotencyConflict),
        }
    );
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());

    assert!(supervisor.authorize_request(42, None, &snapshot()).is_err());
    let inputs = review(&mut supervisor, 42, 1);
    let permit = supervisor
        .authorize_request(42, Some(&inputs), &snapshot())
        .unwrap();
    assert_eq!(wire.execute(Command::Poll { request: 42 }), pending);
    let message = supervisor
        .dispatch_request(
            42,
            DispatchKeys::single(&permit),
            Some(&inputs),
            &snapshot(),
        )
        .unwrap();
    let poll = encode_command(&Command::Poll { request: 42 }).unwrap();
    assert_eq!(
        wire.exchange(&poll).result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown,
        })
    );
    let receipt = endpoint.deliver(&message).unwrap();
    supervisor.accept_receipt(receipt).unwrap();
    let terminal = wire.execute(Command::Poll { request: 42 });
    assert!(matches!(
        terminal.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            basis,
        }) if basis.request == 42 && basis.source == BasisSource::ControlLedger
    ));
    assert_eq!(wire.exchange(&poll), terminal);
    assert_eq!(reconnected.execute(command), terminal);
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
    assert_eq!(endpoint.payload(), b"publish");
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn typed_cancel_keeps_request_local_custody_and_native_disposition() {
    let (port, mut supervisor, _) = fixture(IntakeLimits::default());
    let mut wire = ActorWire::new(port.clone());
    let mut separate_session = ActorWire::new(port);
    wire.execute(submit(42, proposal())).result.unwrap();
    assert_eq!(
        separate_session
            .execute(Command::Cancel { request: 42 })
            .result,
        Err(WireError::Withheld)
    );
    assert_eq!(
        wire.execute(Command::Cancel { request: 42 }).result,
        Ok(Knowledge::Pending { request: 42 })
    );
    assert!(
        supervisor
            .accept_next(&snapshot())
            .unwrap()
            .unwrap()
            .result
            .unwrap()
            .is_none()
    );
    let terminal = wire.execute(Command::Poll { request: 42 });
    assert!(matches!(
        terminal.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            basis,
        }) if basis.source == BasisSource::Intake
    ));
    let retry = encode_command(&submit(42, proposal())).unwrap();
    assert_eq!(wire.exchange(&retry), terminal);
    assert!(supervisor.broker().inspect().ledger.stages.is_empty());
    assert!(supervisor.accept_next(&snapshot()).unwrap().is_none());
}

#[test]
fn full_session_preserves_validation_order_and_existing_tickets() {
    let (port, _supervisor, _) = fixture(IntakeLimits::default());
    let mut wire = ActorWire::new(port);
    for request in 1..=MAX_ACTOR_REQUESTS as u64 {
        let command = submit(request, proposal());
        let response = if request.is_multiple_of(2) {
            wire.execute(command)
        } else {
            wire.exchange(&encode_command(&command).unwrap())
        };
        assert_eq!(response.result, Ok(Knowledge::Pending { request }));
    }
    let overflow = MAX_ACTOR_REQUESTS as u64 + 1;
    let command = submit(overflow, proposal());
    let refusal = WireResponse {
        request: Some(overflow),
        result: Err(WireError::Capacity),
    };
    assert_eq!(wire.execute(command.clone()), refusal);
    assert_eq!(wire.exchange(&encode_command(&command).unwrap()), refusal);
    assert_eq!(
        wire.execute(Command::Poll { request: 0 }),
        WireResponse {
            request: None,
            result: Err(WireError::MalformedRequest),
        }
    );
    let mut invalid = proposal();
    invalid.target.object = 0;
    assert_eq!(
        wire.execute(submit(overflow, invalid)),
        WireResponse {
            request: None,
            result: Err(WireError::MalformedRequest),
        }
    );
    assert_eq!(
        wire.execute(submit(1, proposal())).result,
        Ok(Knowledge::Pending { request: 1 })
    );
    assert_eq!(
        wire.execute(Command::Poll {
            request: MAX_ACTOR_REQUESTS as u64
        })
        .result,
        Ok(Knowledge::Pending {
            request: MAX_ACTOR_REQUESTS as u64
        })
    );
    assert!(matches!(
        wire.execute(Command::Poll { request: overflow }).result,
        Ok(Knowledge::Withheld { .. })
    ));
}
