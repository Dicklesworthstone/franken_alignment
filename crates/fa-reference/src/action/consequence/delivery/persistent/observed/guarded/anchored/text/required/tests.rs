//! Real canonical files, original numerical state and original Store barriers.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::delivery::persistent::{JournalIo, Reconciliation, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{journal,
    decoder::{FileDecoderConfig, text::FileTextGenerationCommand},
    guarded::{FileGuardSet, FileRecoveryFloor}, stream::generated::FileTextMessageRequest};
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::Snapshot;
use std::collections::BTreeMap;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
    pub(super) fn stopped_config() -> FileDecoderConfig { configured(3.0, 65, Some(256)) }
}
use fixtures::{Directory, bytes, command, request, tokenizer};

fn profile() -> FileOversightProfile {
    let mut p = fixtures::host_profile(); p.delivery.initial_payload.clear();
    p.delivery.total = 4096;
    p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap(); p
}
fn stream() -> StreamProfile { StreamProfile::new(9, 1, 4, 64, 256).unwrap() }
fn requirements(c: &FileDecoderConfig) -> FileRecoveryRequirements {
    FileRecoveryRequirements { guards: FileGuardSet { stream: Some(stream()), decoder: Some(c.clone()),
        decoder_stop: None, source: None, identity: None, campaigns: None, credential: None },
        effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 } }
}
fn start(root: &Directory, c: &FileDecoderConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create_generated_text_stream(root.store(), profile(),
        stream(), c.clone(), tokenizer(false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}
fn begin(host: &mut FileOversight) {
    let cmd = command(host, 7, request(b"ab", 2)); host.begin_decoder_text(host.revision(), cmd).unwrap();
}
fn resume(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
}
fn source(host: &FileOversight) -> FileTextMessageRequest {
    FileTextMessageRequest { request: 91, generation: 7,
        generation_revision: host.decoder_generation_progress(7).unwrap().generation_revision(),
        target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100) }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }

#[test]
fn generated_message_required_recovery_keeps_each_cursor_boundary_and_original_role_bundle() {
    for steps in 0..=4 {
        let root = Directory::new(); let c = fixtures::stopped_config();
        let mut host = start(&root, &c); begin(&mut host);
        for revision in 0..steps { host.advance_decoder_text(host.revision(), 7, revision).unwrap(); }
        let progress = host.decoder_text_progress(7).unwrap();
        let numerical = host.decoder_inspection().unwrap().numerical;
        let revision = host.revision(); let anchor = host.history_anchor().unwrap(); drop(host);
        let (mut host, roles) = FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
            &requirements(&c), &tokenizer(false), &anchor).unwrap();
        assert!(roles.identity_observer.is_none()); assert!(roles.policy_governor.is_none());
        assert_eq!(host.revision(), revision + 1); assert!(host.generated_text_stream_required().unwrap());
        assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        let recovered = host.decoder_text_progress(7).unwrap();
        assert_eq!(recovered.bytes().unwrap(), progress.bytes().unwrap());
        assert_eq!(recovered.generation_revision(), progress.generation_revision());
        assert_eq!(recovered.finish(), progress.finish());
        resume(&mut host);
        let complete = host.advance_decoder_text_batch(host.revision(), 7, steps, 4).unwrap();
        assert_eq!(complete.bytes().unwrap(), b"A");
        assert_eq!(complete.finish(), Some(Ok(GenerationFinish::StopToken)));
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
        let raw = host.stream_message_spec("A", ElapsedTick(100)).unwrap();
        let before = bytes(&host);
        assert!(matches!(host.submit_request(host.revision(), 91, raw, snapshot()),
            Err(JournalError::Contract(Error::Binding))));
        assert_eq!(bytes(&host), before);
        let input = source(&host);
        let status = host.submit_decoder_text_message(host.revision(), input, snapshot()).unwrap();
        assert!(matches!(status.disposition, FileRequestDisposition::Admitted { .. }));
        assert_eq!(host.inspect().executions, 0);
        let disk = FileOversight::read_decoder_text_message(root.store(), &profile(), &c,
            &tokenizer(false), stream(), 91).unwrap();
        assert_eq!(disk.generation.bytes().unwrap(), b"A");
        assert_eq!(disk.status, status); assert_eq!(disk.stream.published.message_count(), 0);
    }
}

#[test]
fn generated_message_required_opener_rejects_a_valid_legacy_stream_even_with_its_matching_anchor() {
    let root = Directory::new(); let c = fixtures::stopped_config();
    let (mut legacy, _) = FileOversight::create_stream(root.store(), profile(), stream()).unwrap();
    legacy.enable_decoder(legacy.revision(), c.clone()).unwrap();
    legacy.enable_decoder_tokenizer(legacy.revision(), tokenizer(false)).unwrap();
    legacy.observe_time(legacy.revision(), ElapsedTick(1)).unwrap();
    let cmd = command(&legacy, 7, request(b"ab", 2));
    legacy.generate_decoder_text(legacy.revision(), cmd).unwrap();
    let anchor = legacy.history_anchor().unwrap(); let before = bytes(&legacy); drop(legacy);
    assert!(matches!(FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
        &requirements(&c), &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
    let (legacy, _) = FileOversight::open_guarded_text_anchored(root.store(), profile(),
        &requirements(&c), &tokenizer(false), &anchor).unwrap();
    assert!(!legacy.generated_text_stream_required().unwrap());
    assert_eq!(legacy.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"A");
}

#[test]
fn generated_message_required_recovery_barriers_return_no_candidate_owner_or_new_token() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let c = fixtures::stopped_config();
        let mut host = start(&root, &c); begin(&mut host);
        host.advance_decoder_text_batch(host.revision(), 7, 0, 3).unwrap();
        let n = host.decoder_inspection().unwrap().numerical;
        let revision = host.revision(); let anchor = host.history_anchor().unwrap(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let error = FileOversight::open_generated_text_stream_store(store, profile(), &requirements(&c),
            &tokenizer(false).to_bytes().unwrap(), &anchor).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("original replacement barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        let visible = barrier == JournalIo::DirectorySync;
        let disk = FileOversight::read_decoder_text_progress(root.store(), &profile(), &c, &tokenizer(false), 7).unwrap();
        assert_eq!(disk.publication.revision, revision + u64::from(visible));
        assert_eq!(disk.numerical.numerical, n); assert_eq!(disk.numerical.paused, visible);
        assert_eq!(disk.text.bytes().unwrap(), b"A"); assert!(!disk.text.is_complete());
        let image = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();
        let identity = storage::identity(&root.store()).unwrap();
        let events = journal::decode(&profile(), &identity, &image).unwrap();
        assert!(Machine::replay(&profile(), &events).unwrap().generated_text_only);
        let (mut host, _) = FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
            &requirements(&c), &tokenizer(false), &anchor).unwrap();
        assert!(host.generated_text_stream_required().unwrap());
        resume(&mut host);
        let complete = host.advance_decoder_text(host.revision(), 7, 3).unwrap();
        assert_eq!(complete.finish(), Some(Ok(GenerationFinish::StopToken)));
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn generated_message_required_mode_substitution_and_bad_guard_requirements_do_not_write() {
    let root = Directory::new(); let c = fixtures::stopped_config();
    let mut host = start(&root, &c); begin(&mut host);
    host.advance_decoder_text_batch(host.revision(), 7, 0, 4).unwrap();
    let before = bytes(&host); let anchor = host.history_anchor().unwrap();
    let mut downgraded = host.events.clone(); downgraded[0] = Event::StreamBootstrap(stream());
    let altered = journal::encode(&profile(), host.store.identity(), &downgraded).unwrap();
    assert!(!Machine::replay(&profile(), &downgraded).unwrap().generated_text_only);
    drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &altered).unwrap();
    assert!(matches!(FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
        &requirements(&c), &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), altered);
    std::fs::write(root.store().join(storage::CANONICAL), &before).unwrap();
    let mut missing = requirements(&c); missing.guards.stream = None;
    assert!(matches!(FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
        &missing, &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Incomplete))));
    let mut wrong = requirements(&c); wrong.guards.stream = Some(StreamProfile::new(10, 1, 4, 64, 256).unwrap());
    assert!(matches!(FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
        &wrong, &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
    let (host, _) = FileOversight::open_generated_text_stream_anchored(root.store(), profile(),
        &requirements(&c), &tokenizer(false), &anchor).unwrap();
    assert!(host.generated_text_stream_required().unwrap());
}

#[test]
fn generated_message_required_generic_reopen_preserves_mode_and_existing_obligations() {
    let root = Directory::new(); let c = fixtures::stopped_config();
    let mut host = start(&root, &c);
    let cmd = command(&host, 7, request(b"ab", 2)); host.generate_decoder_text(host.revision(), cmd).unwrap();
    let input = source(&host); host.submit_decoder_text_message(host.revision(), input.clone(), snapshot()).unwrap();
    let before = host.request_status(91).unwrap();
    assert!(matches!(before.disposition, FileRequestDisposition::Admitted { .. }));
    drop(host);
    let (mut host, _) = FileOversight::open_stream(root.store(), profile(), stream()).unwrap();
    assert!(host.generated_text_stream_required().unwrap());
    let cancelled = host.request_status(91).unwrap();
    assert!(matches!(cancelled.disposition, FileRequestDisposition::Admitted {
        stage: crate::action::ActionState::Cancelled, .. }));
    let disk = bytes(&host);
    assert_eq!(host.submit_decoder_text_message(0, input, Snapshot::default()).unwrap(), cancelled);
    assert_eq!(bytes(&host), disk); assert!(!host.clock_ready());
    resume(&mut host);
    let raw = host.stream_message_spec("replacement", ElapsedTick(100)).unwrap();
    assert!(matches!(host.submit_request(host.revision(), 92, raw, snapshot()),
        Err(JournalError::Contract(Error::Binding))));
    // No effect was dispatched; the original empty reconciliation sweep remains
    // valid instead of inventing an execution/nonexecution receipt for it.
    let outcomes = host.reconcile_pending(host.revision()).unwrap();
    assert_eq!(outcomes, BTreeMap::<u64, Result<Reconciliation, Error>>::new());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn generated_message_required_reserved_tail_recovers_and_stops_after_ordinary_work_is_full() {
    use crate::action::consequence::delivery::StopRequest;
    let root = Directory::new(); let c = fixtures::stopped_config();
    let mut p = profile(); p.delivery.limits.events = 10;
    let reserve = RecoveryReserve::terminal();
    let (mut host, _) = FileOversight::create_generated_text_stream_with_reserve(root.store(), p.clone(),
        stream(), c.clone(), tokenizer(false), reserve).unwrap();
    assert_eq!(host.revision(), 4); assert_eq!(host.journal_capacity().unwrap().reserve(), Some(reserve));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let cmd = command(&host, 7, request(b"ab", 2)); host.generate_decoder_text(host.revision(), cmd).unwrap();
    assert_eq!(host.revision(), 7); assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    assert!(host.journal_capacity().unwrap().terminal_space_remaining());
    let input = source(&host); let before = bytes(&host);
    assert!(matches!(host.submit_decoder_text_message(host.revision(), input, snapshot()),
        Err(JournalError::Contract(Error::Limit))));
    assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_none());
    let anchor = host.history_anchor().unwrap(); drop(host);
    let (mut host, _) = FileOversight::open_generated_text_stream_anchored(root.store(), p,
        &requirements(&c), &tokenizer(false), &anchor).unwrap();
    assert_eq!(host.revision(), 8); assert!(host.generated_text_stream_required().unwrap());
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(reserve));
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let control = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 99,
        expected_control_sequence: control.sequence, expected_authority_epoch: control.ledger.epoch }).unwrap();
    host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.revision(), 10); assert_eq!(host.journal_capacity().unwrap().remaining().events, 0);
    assert!(host.inspect().stop.is_some()); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
    assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"A");
}

#[test]
fn generated_message_required_reserve_remains_original_pre_work_admission_not_a_late_refund() {
    let c = fixtures::stopped_config();
    let root = Directory::new(); let mut host = start(&root, &c);
    let before = bytes(&host);
    assert!(matches!(host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()),
        Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_none());
    drop(host);
    for reserve in [RecoveryReserve { events: 2, bytes: 50 }, RecoveryReserve { events: 3, bytes: 49 }] {
        let root = Directory::new();
        assert!(matches!(FileOversight::create_generated_text_stream_with_reserve(root.store(), profile(),
            stream(), c.clone(), tokenizer(false), reserve), Err(JournalError::Contract(Error::InvalidInput))));
        assert!(!root.store().join(storage::CANONICAL).exists());
    }
    let root = Directory::new();
    let (host, _) = FileOversight::create_generated_text_stream_with_reserve(root.store(), profile(),
        stream(), c, tokenizer(false), RecoveryReserve::terminal()).unwrap();
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    assert!(host.generated_text_stream_required().unwrap());
}
