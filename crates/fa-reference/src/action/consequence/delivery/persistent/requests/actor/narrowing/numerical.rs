//! Actual learned generation and cold replay under one finite narrowing grant.
//! Synthetic original weights are deterministic controls, not qualified models.
use super::*;
use crate::Snapshot;
use crate::action::ResolvedTarget;
use crate::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversightProfile, decoder::learned::FileLearnedConfig,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileActorSupervisor, LearnedTextProposal, LearnedTextRelease,
};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::{
    decoder_monitoring::LearnedDecoderBindingLimits, learned_source::LearnedEvidenceLimits,
};

#[path = "../../../observed/decoder/learned/config/text/intake/tests/fixture.rs"]
mod learned;

fn stream_profile() -> FileOversightProfile {
    let mut selected = learned::profile();
    selected.delivery.initial_payload.clear();
    selected.delivery.total = 1000;
    selected
}

fn stream_config() -> FileLearnedConfig {
    let (model, tokenizer, source) = learned::recipe();
    FileLearnedConfig::new_text_stream(model, tokenizer, source,
        LearnedDecoderBindingLimits::default(),
        StreamProfile::new(61, 1, 7, 32, 128).unwrap()).unwrap()
        .with_required_sidecar().unwrap()
}

fn generated(root: &learned::Directory, config: &FileLearnedConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create_with_learned_text_stream(
        root.store(), stream_profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for _ in 0..3 { learned::step(&mut host); }
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    host.clear_learned_replay_cache();
    host
}

#[test]
fn learned_stream_restriction_pays_exact_cumulative_replay_without_resampling_or_advancing_output() {
    let root = learned::Directory::new();
    let config = stream_config();
    let host = generated(&root, &config);
    let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    learned::observe(&mut supervisor, learned::snapshot());
    actor.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    learned::observe(&mut supervisor, learned::snapshot());
    actor.submit(72, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let numerical = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let stream = supervisor.host().unwrap().stream_snapshot().unwrap();
    let events = before.revision as usize + 1;
    let image = std::fs::read(root.store().join("delivery.bin")).unwrap();
    learned::observe(&mut supervisor, learned::snapshot());

    let (default, default_grant) = supervisor.narrow_port(1, 1, events).unwrap();
    let unavailable = plan(default.next(1201, vec![before.target]));
    assert_eq!(unavailable.block_reason(), Some(FileNarrowBlockReason::NumericalReplayUnsupported));
    let first = FileNarrowRequest::new(1201, unavailable.basis(), vec![before.target]).unwrap();
    assert_eq!(default.narrow(first.clone()), Err(FileNarrowError::Unsupported));
    assert_eq!(default_grant.remaining_operations(), 1);
    assert_eq!(default_grant.remaining_numerical_replay_events(), None);
    let (short, short_grant) = supervisor.narrow_port_with_numerical_budget(
        1, 1, events, events - 1).unwrap();
    assert_eq!(plan(short.next(1201, vec![before.target])).block_reason(),
        Some(FileNarrowBlockReason::NumericalReplayLimit));
    assert_eq!(short.narrow(first.clone()), Err(FileNarrowError::Capacity));
    assert_eq!(short_grant.remaining_operations(), 1);
    assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().learned_generation_inspection().unwrap(), numerical);
    assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap(), stream);
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), image);
    assert!(supervisor.owner.borrow().snapshot.is_some());

    let total = events + events + 1;
    let (paid, grant) = supervisor.narrow_port_with_numerical_budget(
        3, 3, events + 2, total).unwrap();
    assert_eq!(plan(paid.next(1201, vec![before.target])).request(), Some(first.clone()));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total));
    let retained = receipt(paid.narrow(first.clone()));
    assert!(retained.cancelled().is_empty());
    assert_eq!(retained.refunded_units(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(events + 1));
    assert_eq!(grant.remaining_operations(), 2);
    assert!(supervisor.owner.borrow().snapshot.is_none());
    let second = plan(paid.next(1202, Vec::new())).request().unwrap();
    let excluded = receipt(paid.narrow(second));
    assert_eq!(excluded.cancelled(), &[1, 2]);
    assert_eq!(excluded.refunded_units(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(grant.remaining_operations(), 1);
    let after = supervisor.host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 2);
    assert_eq!(after.control.sequence, before.control.sequence + 2);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.available, before.control.ledger.available);
    assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
    assert_eq!(after.executions, 0);
    let current = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    assert_eq!(current.numerical, numerical.numerical);
    assert_eq!(current.pending, numerical.pending);
    assert_eq!(current.paused, numerical.paused);
    let current = supervisor.host().unwrap().stream_snapshot().unwrap();
    assert_eq!(current.confirmed, stream.confirmed);
    assert_eq!(current.published, stream.published);
    assert_eq!(current.pending, stream.pending);
    assert_eq!(current.confirmed_target, stream.confirmed_target);
    assert_eq!(supervisor.host().unwrap().learned_text_message(
        LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");

    learned::observe(&mut supervisor, learned::snapshot());
    let image = std::fs::read(root.store().join("delivery.bin")).unwrap();
    assert_eq!(receipt(paid.clone().narrow(first.clone())), retained);
    assert_eq!(paid.narrow(FileNarrowRequest::new(1201, first.expected(), Vec::new()).unwrap()),
        Err(FileNarrowError::IdempotencyConflict));
    let exhausted = plan(paid.next(1203, Vec::new()));
    assert_eq!(exhausted.block_reason(), Some(FileNarrowBlockReason::NumericalReplayLimit));
    assert_eq!(paid.narrow(FileNarrowRequest::new(1203, exhausted.basis(), Vec::new()).unwrap()),
        Err(FileNarrowError::Capacity));
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), image);
    assert_eq!(supervisor.host().unwrap().inspect(), after);

    // Canonical recovery reconstructs the restriction and original numerical
    // work, then applies its normal fresh fence; no old grant survives it.
    drop(supervisor);
    let (host, _) = FileOversight::open_with_learned_text_stream(
        root.store(), stream_profile(), &config).unwrap();
    assert!(host.inspect().control.ceiling.unwrap().targets().is_empty());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical.numerical);
    assert_eq!(host.stream_snapshot().unwrap().confirmed, stream.confirmed);
    assert_eq!(host.stream_snapshot().unwrap().published, stream.published);
    assert_eq!(paid.narrow(first), Ok(Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable }));
}

#[test]
fn every_numerical_storage_barrier_spends_the_original_reservation_once() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync] {
        let root = learned::Directory::new();
        let config = stream_config();
        let host = generated(&root, &config);
        let before = host.inspect();
        let numerical = host.learned_generation_inspection().unwrap();
        let events = before.revision as usize + 1;
        let (_, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let (port, grant) = supervisor.narrow_port_with_numerical_budget(1, 1, events, events).unwrap();
        let input = plan(port.next(1401, Vec::new())).request().unwrap();
        learned::observe(&mut supervisor, learned::snapshot());
        supervisor.host().unwrap().fail_storage_once(barrier);
        assert_eq!(port.narrow(input.clone()), Err(FileNarrowError::Unavailable));
        assert_eq!(grant.remaining_operations(), 0);
        assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
        assert!(supervisor.owner.borrow().snapshot.is_none());
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        assert_eq!(port.clone().narrow(input), Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable }));
        drop(supervisor);
        let (reopened, _) = FileOversight::open_with_learned_text_stream(
            root.store(), stream_profile(), &config).unwrap();
        let after = reopened.inspect();
        assert_eq!(after.revision, before.revision + 1 + u64::from(barrier == JournalIo::DirectorySync));
        assert_eq!(after.control.ceiling.is_some(), barrier == JournalIo::DirectorySync);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.executions, 0);
        assert_eq!(reopened.learned_generation_inspection().unwrap().numerical, numerical.numerical);
    }
}

#[test]
fn restriction_during_a_pending_learned_step_preserves_its_original_numerical_intent() {
    let root = learned::Directory::new();
    let config = stream_config();
    let (mut host, _) = FileOversight::create_with_learned_text_stream(
        root.store(), stream_profile(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let original = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), original.actor_revision, original.position).unwrap();
    let pending = host.learned_generation_inspection().unwrap();
    assert!(pending.pending.is_some());
    let events = host.revision() as usize + 1;
    let (_, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let (port, grant) = supervisor.narrow_port_with_numerical_budget(1, 1, events, events).unwrap();
    receipt(port.narrow(plan(port.next(1451, Vec::new())).request().unwrap()));
    let mut host = supervisor.host_mut().unwrap();
    let current = host.learned_generation_inspection().unwrap();
    assert_eq!(current.numerical, pending.numerical);
    assert_eq!(current.pending, pending.pending);
    assert_eq!(current.paused, pending.paused);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    // The next numerical transition still consumes the exact original intent;
    // target restriction supplied no token, sampler draw or restart operation.
    let revision = host.revision();
    host.advance_learned_generation(revision, original.actor_revision, original.position).unwrap().unwrap();
    assert!(host.learned_generation_inspection().unwrap().pending.is_none());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, original.position + 1);
    assert!(host.inspect().control.ceiling.unwrap().targets().is_empty());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn grant_bounds_are_checked_before_provisioning_and_plain_history_spends_no_numerical_work() {
    let root = Root::new();
    let (_, supervisor) = owner(&root).into_actor_gateway();
    for (queries, operations, replay, numerical) in [
        (0, 1, 10, 10), (1, 0, 10, 10), (1, 1, 0, 1), (1, 1, 10, 0),
        (MAX_NARROW_QUERIES + 1, 1, 10, 10), (1, MAX_NARROW_OPERATIONS + 1, 10, 10),
        (1, 1, MAX_NARROW_REPLAY_EVENTS + 1, 1), (1, 2, 10, 21),
    ] {
        assert!(supervisor.narrow_port_with_numerical_budget(queries, operations, replay, numerical).is_err());
    }
    let before = supervisor.host().unwrap().inspect();
    let events = before.revision as usize + 1;
    let (port, grant) = supervisor.narrow_port_with_numerical_budget(1, 1, events, 1).unwrap();
    receipt(port.narrow(plan(port.next(1501, Vec::new())).request().unwrap()));
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(1));
}
