//! Independent envelope and custody controls; durable positives are integration tests.
use super::*;
use crate::action::consequence::oversight::{
    actor::{ActorBasis, BasisSource, UnknownReason},
    actor_wire::{ActorWire, Command, WireError, encode_command},
};

fn proposal() -> ActorProposal {
    FileLearnedTextStreamActorPort::encode_release(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap()
}
fn executed() -> Knowledge<ActorOutcome> {
    Knowledge::Known { value: ActorOutcome::Executed,
        basis: ActorBasis { request: 71, generation: 3, source: BasisSource::ControlLedger } }
}
fn port(outcome: Knowledge<ActorOutcome>) -> FileLearnedReceiptActorPort {
    FileLearnedReceiptActorPort::verified(71, proposal(), outcome)
}

#[test]
fn every_outer_field_and_inner_intent_byte_is_bound_without_normalization() {
    let port = port(executed()); let original = proposal();
    assert_eq!(port.poll(&port.submit(71, &original).unwrap()), executed());
    let mut mutations = Vec::new();
    for field in 0..8 {
        let mut changed = original.clone();
        match field {
            0 => changed.target.adapter += 1,
            1 => changed.target.object += 1,
            2 => changed.target.contract_version += 1,
            3 => changed.target.expected_version += 1,
            4 => changed.target.generation += 1,
            5 => changed.expected_policy_epoch += 1,
            6 => changed.units += 1,
            _ => changed.deadline.0 += 1,
        }
        mutations.push(changed);
    }
    for byte in 0..original.payload.len() {
        let mut changed = original.clone(); changed.payload[byte] ^= 1; mutations.push(changed);
    }
    let mut changed = original.clone(); changed.payload.push(0); mutations.push(changed);
    let mut changed = original.clone(); changed.payload.pop(); mutations.push(changed);
    mutations.push(FileLearnedTextStreamActorPort::encode_release(
        71, LearnedTextRelease::Finish, ElapsedTick(100)).unwrap());
    for changed in mutations {
        assert_eq!(port.submit(71, &changed).err(), Some(ActorError::IdempotencyConflict));
        assert_eq!(port.poll(&port.submit(71, &original).unwrap()), executed());
    }
}

#[test]
fn request_scope_and_ticket_issuer_cannot_be_reconstructed_from_matching_labels() {
    let first = port(executed()); let second = port(executed());
    let original = proposal(); let ticket = first.submit(71, &original).unwrap();
    assert_eq!(ticket.request(), 71); assert_eq!(first.request(), 71);
    assert_eq!(first.submit(0, &original).err(), Some(ActorError::MalformedProposal));
    assert_eq!(first.submit(72, &original).err(), Some(ActorError::Withheld));
    assert!(matches!(second.poll(&ticket), Knowledge::Withheld { .. }));
    assert_eq!(second.cancel(&ticket), Err(ActorError::Withheld));
    assert_eq!(first.poll(&ticket.clone()), executed());
    drop(first);
    assert!(ticket.receipt.upgrade().is_none());
    assert!(matches!(second.poll(&ticket), Knowledge::Withheld { .. }));
}

#[test]
fn polling_and_cancel_cannot_promote_or_refund_any_saved_outcome() {
    for outcome in [Knowledge::Pending { request: 71 },
        Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }, executed(),
        Knowledge::Known { value: ActorOutcome::NotAdmitted,
            basis: ActorBasis { request: 71, generation: 1, source: BasisSource::Intake } }] {
        let port = port(outcome.clone()); let ticket = port.submit(71, &proposal()).unwrap();
        for _ in 0..8 {
            assert_eq!(port.poll(&ticket), outcome);
            assert_eq!(port.cancel(&ticket), Err(ActorError::Withheld));
            assert_eq!(port.poll(&ticket), outcome);
        }
    }
}

#[test]
fn original_actor_wire_observes_exact_receipts_but_never_accepts_cancellation() {
    let mut wire = ActorWire::new(port(executed()));
    let submit = encode_command(&Command::Submit { request: 71, proposal: proposal() }).unwrap();
    assert_eq!(wire.exchange(&submit).result, Ok(executed()));
    let cancel = encode_command(&Command::Cancel { request: 71 }).unwrap();
    assert_eq!(wire.exchange(&cancel).result, Err(WireError::Withheld));
    let poll = encode_command(&Command::Poll { request: 71 }).unwrap();
    assert_eq!(wire.exchange(&poll).result, Ok(executed()));
    assert_eq!(wire.exchange(&submit).result, Ok(executed()));
    let other = encode_command(&Command::Submit { request: 72,
        proposal: FileLearnedTextStreamActorPort::encode_release(72,
            LearnedTextRelease::Message, ElapsedTick(100)).unwrap() }).unwrap();
    assert_eq!(wire.exchange(&other).result, Err(WireError::Withheld));
    assert_eq!(wire.exchange(&poll).result, Ok(executed()));
}
