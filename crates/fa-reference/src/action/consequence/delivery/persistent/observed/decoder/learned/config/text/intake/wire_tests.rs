//! Original inference and durable authority behind the ORIGINAL actor protocol.
//! Synthetic model weights are controls, not evidence of detector effectiveness.
use super::*;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::persistent::{JournalIo, requests::actor::{
    FileActorSupervisor, FileLearnedTextActorPort, FileLearnedTextStreamActorPort, LearnedTextRelease}};
use crate::action::consequence::delivery::persistent::observed::{FileOversightProfile,
    FileHumanReviewer, decoder::learned::FileLearnedConfig};
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use crate::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use crate::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire,
    ChannelLimits, ChannelState, CloseReason, Command, WireError, WireResponse, encode_command};
use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;
use crate::Snapshot;
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;

fn document(request: u64, proposal: ActorProposal) -> Vec<u8> {
    encode_command(&Command::Submit { request, proposal }).unwrap()
}
fn raw_document(request: u64) -> Vec<u8> {
    let mut intent = proposal(); intent.units = 2;
    document(request, FileLearnedTextActorPort::encode_request(request, intent).unwrap())
}
fn stream_document(request: u64, release: LearnedTextRelease) -> Vec<u8> {
    document(request, FileLearnedTextStreamActorPort::encode_release(request, release, ElapsedTick(100)).unwrap())
}
fn stream_profile() -> StreamProfile { StreamProfile::new(61, 1, 8, 32, 128).unwrap() }
fn stream_config() -> FileLearnedConfig {
    let (model, tokenizer, source) = recipe();
    FileLearnedConfig::new_text_stream(model, tokenizer, source,
        LearnedDecoderBindingLimits::default(), stream_profile()).unwrap().with_required_sidecar().unwrap()
}
fn stream_delivery_profile() -> FileOversightProfile {
    let mut profile = profile(); profile.delivery.initial_payload.clear(); profile.delivery.total = 1000; profile
}
fn stream_owner(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_learned_text_stream(
        root.store(), stream_delivery_profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..3 { step(&mut host); }
    (host, reviewer)
}

#[test]
fn wire_admits_original_output_and_effect_units_without_inference_or_authorization() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    let bytes = raw_document(71);
    assert_eq!(wire.exchange(&bytes).result, Err(WireError::Unavailable));
    observe(&mut supervisor, snapshot());
    let revision = supervisor.host().unwrap().revision();
    let accepted = wire.exchange(&bytes);
    assert_eq!(accepted.result, Ok(Knowledge::Pending { request: 71 }));
    let host = supervisor.host().unwrap();
    let action = host.request_action(71).unwrap();
    assert_eq!(action.spec().payload, b"aa"); assert_eq!(action.spec().units, 2);
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    drop(host);
    observe(&mut supervisor, snapshot());
    assert_eq!(wire.exchange(&bytes), accepted);
    assert_eq!(wire.exchange(&raw_document(72)).result, Ok(Knowledge::Pending { request: 72 }));
    assert_eq!(wire.exchange(&raw_document(73)).result, Err(WireError::Unavailable));
}

#[test]
fn malformed_wire_cannot_inject_output_or_spend_the_next_valid_requests_snapshot() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    observe(&mut supervisor, snapshot());
    let revision = supervisor.host().unwrap().revision();
    let encoded = FileLearnedTextActorPort::encode_request(71, proposal()).unwrap();
    for mutation in 0..5 {
        let mut bad = encoded.clone();
        match mutation {
            0 => bad.payload = b"actor replacement".to_vec(),
            1 => bad.payload[0] ^= 1, 2 => bad.payload[8] ^= 1,
            3 => { bad.payload.push(0); bad.units += 1; }, _ => bad.units += 1,
        }
        assert_eq!(wire.exchange(&document(71, bad)).result, Err(WireError::MalformedRequest));
    }
    assert_eq!(wire.exchange(br#"{"version":1,"version":1}"#).result, Err(WireError::MalformedRequest));
    assert_eq!(wire.exchange(&stream_document(71, LearnedTextRelease::Message)).result,
        Err(WireError::MalformedRequest));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
    assert_eq!(wire.exchange(&raw_document(71)).result, Ok(Knowledge::Pending { request: 71 }));
    let mut changed = proposal(); changed.units = 3;
    let conflict = document(71, FileLearnedTextActorPort::encode_request(71, changed).unwrap());
    observe(&mut supervisor, snapshot());
    assert_eq!(wire.exchange(&conflict).result, Err(WireError::IdempotencyConflict));
    assert_eq!(wire.exchange(&raw_document(72)).result, Ok(Knowledge::Pending { request: 72 }));
}

#[test]
fn reconnect_and_fenced_recovery_restore_only_exact_historical_ticket_knowledge() {
    let root = Directory::new(); let config = config(); let (host, _) = owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port.clone());
    observe(&mut supervisor, snapshot());
    let accepted = wire.exchange(&raw_document(71));
    assert_eq!(accepted.result, Ok(Knowledge::Pending { request: 71 }));
    let poll = encode_command(&Command::Poll { request: 71 }).unwrap();
    let mut other = ActorWire::new(port);
    assert!(matches!(other.exchange(&poll).result, Ok(Knowledge::Withheld { .. })));
    assert_eq!(other.exchange(&raw_document(71)), accepted);
    let cancelled = wire.exchange(&encode_command(&Command::Cancel { request: 71 }).unwrap());
    assert!(matches!(cancelled.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    drop(supervisor);
    assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Unknown { .. })));
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let revision = host.revision();
    let (port, supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut recovered = ActorWire::new(port);
    assert!(matches!(recovered.exchange(&poll).result, Ok(Knowledge::Withheld { .. })));
    assert_eq!(recovered.exchange(&raw_document(71)), cancelled);
    assert_eq!(recovered.exchange(&raw_document(72)).result, Err(WireError::Unavailable));
    let host = supervisor.host().unwrap();
    assert_eq!(host.revision(), revision); assert!(!host.clock_ready());
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn original_channel_preserves_fragments_consumed_prefix_and_write_then_flush_backpressure() {
    let root = Directory::new(); let (host, _) = owner(&root, &config());
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let mut channel = ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap();
    let first = raw_document(71); let mut second = raw_document(72); second.push(b'\n');
    observe(&mut supervisor, snapshot());
    let revision = supervisor.host().unwrap().revision();
    assert_eq!(channel.feed(&first[..23]).state, ChannelState::Reading);
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    let mut tail = first[23..].to_vec(); tail.push(b'\n'); tail.extend_from_slice(&second);
    let consumed = channel.feed(&tail);
    assert_eq!(consumed.consumed, first.len() - 23 + 1);
    assert_eq!(consumed.state, ChannelState::ReplyReady);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    observe(&mut supervisor, snapshot());
    assert_eq!(channel.feed(&second).consumed, 0);
    let mut expected = WireResponse { request: Some(71), result: Ok(Knowledge::Pending { request: 71 }) }.encode();
    expected.push(b'\n'); assert_eq!(channel.pending_output(), expected);
    channel.acknowledge_written(1).unwrap();
    assert_eq!(channel.feed(&second).consumed, 0);
    channel.acknowledge_written(channel.pending_output().len()).unwrap();
    assert_eq!(channel.feed(&second).consumed, 0, "written is not flushed");
    channel.acknowledge_flushed().unwrap();
    assert_eq!(channel.feed(&second).consumed, second.len());
    assert_eq!(supervisor.host().unwrap().retained_requests(), 2);
    channel.acknowledge_written(channel.pending_output().len()).unwrap();
    channel.acknowledge_flushed().unwrap();
    channel.feed(&raw_document(73));
    assert_eq!(channel.finish_input(), ChannelState::Closed(CloseReason::TruncatedFrame));
    assert_eq!(supervisor.host().unwrap().retained_requests(), 2);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}

#[test]
fn learned_stream_wire_derives_full_frame_and_refuses_early_finish_or_target_override() {
    let root = Directory::new(); let (host, _) = stream_owner(&root, &stream_config());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    observe(&mut supervisor, snapshot());
    assert_eq!(wire.exchange(&stream_document(71, LearnedTextRelease::Finish)).result, Err(WireError::Unavailable));
    assert_eq!(wire.exchange(&raw_document(71)).result, Err(WireError::MalformedRequest));
    let mut bad = FileLearnedTextStreamActorPort::encode_release(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    bad.target = profile().delivery.target;
    assert_eq!(wire.exchange(&document(71, bad)).result, Err(WireError::MalformedRequest));
    let accepted = wire.exchange(&stream_document(71, LearnedTextRelease::Message));
    assert_eq!(accepted.result, Ok(Knowledge::Pending { request: 71 }));
    let host = supervisor.host().unwrap();
    let action = host.request_action(71).unwrap();
    let frame = ReleaseFrame::decode(&action.spec().payload).unwrap();
    assert_eq!(frame.message(), Some("aa")); assert!(frame.prior_messages().is_empty());
    assert_eq!(frame.profile(), stream_profile());
    assert_eq!(action.spec().target, Some(profile().delivery.target));
    assert_eq!(action.spec().units, action.spec().payload.len() as u64);
    assert!(action.spec().units > FileLearnedTextStreamActorPort::INTENT_BYTES as u64);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.stream_snapshot().unwrap().published.visible().is_empty());
    drop(host);
    assert_eq!(wire.exchange(&stream_document(71, LearnedTextRelease::Finish)).result,
        Err(WireError::IdempotencyConflict));
    assert_eq!(wire.exchange(&stream_document(71, LearnedTextRelease::Message)), accepted);
}

#[test]
fn stream_wire_recovery_does_not_reinterpret_an_old_message_as_a_new_finish() {
    let root = Directory::new(); let config = stream_config(); let (host, _) = stream_owner(&root, &config);
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    observe(&mut supervisor, snapshot());
    assert_eq!(wire.exchange(&stream_document(71, LearnedTextRelease::Message)).result,
        Ok(Knowledge::Pending { request: 71 }));
    drop(supervisor);
    let (host, _) = FileOversight::open_with_learned_text_stream(root.store(), stream_delivery_profile(), &config).unwrap();
    let revision = host.revision();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port);
    assert_eq!(wire.exchange(&stream_document(71, LearnedTextRelease::Finish)).result,
        Err(WireError::IdempotencyConflict));
    assert!(matches!(wire.exchange(&stream_document(71, LearnedTextRelease::Message)).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(wire.exchange(&stream_document(72, LearnedTextRelease::Finish)).result, Err(WireError::Unavailable));
    let host = supervisor.host().unwrap();
    assert_eq!(host.revision(), revision); assert!(!host.clock_ready());
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn storage_fault_never_returns_a_wire_ticket_or_replays_an_unacknowledged_submission() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(); let (host, _) = owner(&root, &config);
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        let mut wire = ActorWire::new(port);
        observe(&mut supervisor, snapshot()); supervisor.host().unwrap().store.fail_once(barrier);
        assert_eq!(wire.exchange(&raw_document(71)).result, Err(WireError::Unavailable));
        let poll = encode_command(&Command::Poll { request: 71 }).unwrap();
        assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Withheld { .. })));
        assert_eq!(wire.exchange(&raw_document(71)).result, Err(WireError::Unavailable));
        drop(supervisor);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let recorded = host.request_status(71).is_ok(); let revision = host.revision();
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
        let (port, supervisor) = host.into_learned_text_actor_gateway().unwrap();
        let mut wire = ActorWire::new(port);
        let retried = wire.exchange(&raw_document(71));
        if recorded {
            assert!(matches!(retried.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        } else { assert_eq!(retried.result, Err(WireError::Unavailable)); }
        assert_eq!(supervisor.host().unwrap().revision(), revision);
    }
}

#[cfg(unix)]
mod policy_tests;
