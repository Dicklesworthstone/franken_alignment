//! Exact text evidence across cancellation, lost replies and original Store faults.
use super::*;
use super::super::super::super::{journal, storage};
use crate::action::consequence::delivery::persistent::{JournalIo, codec::shared::{Reader, Writer}};
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements, FileRecoveryFloor,
};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
fn begin_text(host: &mut FileOversight, id: u64, prompt: &[u8], new: usize) -> FileTextGenerationCommand {
    let command = command(host, id, request(prompt, new));
    host.begin_decoder_text(host.revision(), command.clone()).unwrap(); command
}

#[test]
fn text_generation_cancel_preserves_prompt_partition_and_only_released_bytes() {
    for cut in 0..=5 {
        let root = Directory::new(); let c = config(3.0, 65); let mut host = owner(&root, &c);
        let command = begin_text(&mut host, 7, b"ab\xff", 3);
        for _ in 0..cut { step(&mut host, 7); }
        let previous = host.decoder_text_progress(7).unwrap(); let numeric = host.decoder_inspection().unwrap().numerical;
        assert!(!previous.is_complete());
        let cancelled = host.cancel_decoder_text(host.revision(), 7, cut).unwrap();
        assert_eq!(cancelled.command(), &command);
        assert_eq!(cancelled.finish(), Some(Ok(GenerationFinish::Cancelled)));
        assert_eq!(cancelled.bytes(), previous.bytes());
        assert_eq!(cancelled.numerical().tokens(), previous.numerical().tokens());
        let receipt = host.decoder_text_generation(7).unwrap(); let report = receipt.result().unwrap();
        assert_eq!(report.prompt().source(), b"ab\xff");
        assert_eq!(report.prompt().tokens(), &[257, 255]);
        assert_eq!(report.prompt().spans(), &[0..2, 2..3]);
        assert_eq!(report.prefix_controls(), &[256]);
        assert_eq!(report.generation().requested_prompt_tokens(), 3);
        assert_eq!(report.generation().reviewed_prompt_tokens(), (cut as usize).min(3));
        assert_eq!(report.bytes(), previous.bytes());
        assert_eq!(host.decoder_inspection().unwrap().numerical, numeric);
        assert!(host.pending_decoder_text().unwrap().is_none());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn text_generation_cancel_can_stop_recovered_work_without_resuming_or_observing_time() {
    for cut in 0..=3 {
        let root = Directory::new(); let c = config(3.0, 65); let t = tokenizer(false);
        let mut host = owner(&root, &c); begin_text(&mut host, 7, b"ab", 3);
        for _ in 0..cut { step(&mut host, 7); }
        let numerical = host.decoder_inspection().unwrap().numerical;
        let previous = host.decoder_text_progress(7).unwrap(); drop(host);
        let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &t).unwrap();
        assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
        assert!(host.advance_decoder_text(host.revision(), 7, cut).is_err());
        let cancelled = host.cancel_decoder_text(host.revision(), 7, cut).unwrap();
        assert_eq!(cancelled.bytes(), previous.bytes());
        assert_eq!(cancelled.finish(), Some(Ok(GenerationFinish::Cancelled)));
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical); drop(host);
        let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &t).unwrap();
        let saved = bytes(&host);
        assert_eq!(host.cancel_decoder_text(0, 7, cut).unwrap().generation_revision(), cut + 1);
        assert_eq!(bytes(&host), saved); assert!(!host.clock_ready());
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes(), previous.bytes());
        resume_after_cancel(&mut host, 2);
        let new = command(&host, 8, request(b"ab", 1));
        assert_eq!(host.generate_decoder_text(host.revision(), new).unwrap().result().unwrap().bytes().unwrap(), b"A");
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, numerical.sampled_draws + 1);
    }
}

#[test]
fn text_generation_cancel_faults_never_expose_candidate_results_or_repeat_tokens() {
    for cut in [0, 1, 3] { for barrier in BARRIERS {
        let root = Directory::new(); let c = config(3.0, 65); let t = tokenizer(false);
        let mut host = owner(&root, &c); begin_text(&mut host, 7, b"ab", 3);
        for _ in 0..cut { step(&mut host, 7); }
        let previous = host.decoder_text_progress(7).unwrap(); let numeric = host.decoder_inspection().unwrap().numerical;
        let revision = host.revision(); let before = bytes(&host);
        host.store.fail_once(barrier);
        let error = host.cancel_decoder_text(revision, 7, cut).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected original Store fault"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        assert!(matches!(host.cancel_decoder_text(0, 7, 0), Err(JournalError::Unavailable)));
        assert!(matches!(host.cancel_decoder_generation(0, 7, 0), Err(JournalError::Unavailable)));
        assert!(matches!(host.decoder_text_progress(7), Err(JournalError::Unavailable)));
        let image = FileOversight::read_decoder_text_progress(root.store(), &host_profile(), &c, &t, 7).unwrap();
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(image.publication.revision, revision + u64::from(visible));
        assert_eq!(image.text.generation_revision(), cut + u64::from(visible));
        assert_eq!(image.text.is_complete(), visible);
        assert_eq!(image.text.finish(), visible.then_some(Ok(GenerationFinish::Cancelled)));
        assert_eq!(image.text.bytes(), previous.bytes()); assert_eq!(image.numerical.numerical, numeric);
        if !visible { assert_eq!(bytes(&host), before); }
        drop(host);
        let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &t).unwrap();
        let cancelled = host.cancel_decoder_text(host.revision(), 7, cut).unwrap();
        assert_eq!(cancelled.finish(), Some(Ok(GenerationFinish::Cancelled)));
        assert_eq!(cancelled.bytes(), previous.bytes());
        assert_eq!(cancelled.generation_revision(), cut + 1);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numeric);
        assert_eq!(host.inspect().executions, 0);
    }}
}

#[test]
fn text_generation_cancel_keeps_split_utf8_and_does_not_sample_a_repair() {
    let root = Directory::new(); let c = split_utf8_config(); let mut host = owner(&root, &c);
    begin_text(&mut host, 7, b"ab", 2);
    for _ in 0..3 { step(&mut host, 7); }
    let previous = host.decoder_text_progress(7).unwrap(); assert_eq!(previous.bytes().unwrap(), &[0xc3]);
    let cancelled = host.cancel_decoder_text(host.revision(), 7, 3).unwrap();
    assert!(cancelled.utf8().is_err()); assert_eq!(cancelled.bytes().unwrap(), &[0xc3]);
    assert_eq!(cancelled.delta_from(0).unwrap().byte_range(), 0..1);
    assert!(cancelled.delta_from(1).unwrap().bytes().is_empty());
    assert!(matches!(cancelled.delta_from(2), Err(Error::Stale)));
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    resume_after_cancel(&mut host, 2);
    // Explicit NEW continuation starts from the retained actual C3-token state.
    let next = raw(&host, 8, &[], 1);
    assert_eq!(host.generate_decoder(host.revision(), next).unwrap().result().unwrap().tokens(), &[0xa9]);
    assert_eq!(host.decoder_text_progress(7).unwrap().bytes().unwrap(), &[0xc3]);
    assert!(host.decoder_text_generation(7).unwrap().result().unwrap().utf8().is_err());
}

#[test]
fn text_generation_cancel_preserves_the_existing_source_interruption_latch() {
    for interrupted in [false, true] {
        let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
        begin_text(&mut host, 7, b"ab", 2); step(&mut host, 7);
        // The SAME private transient fault seam used by existing progress tests.
        // This is a modeled interrupted reader, not a claim of live capture.
        host.source_interrupted = interrupted;
        let saved = bytes(&host); let numeric = host.decoder_inspection().unwrap().numerical;
        if interrupted {
            assert!(matches!(host.advance_decoder_text(host.revision(), 7, 1), Err(JournalError::Contract(Error::Incomplete))));
            assert_eq!(bytes(&host), saved);
        }
        host.cancel_decoder_text(host.revision(), 7, 1).unwrap();
        assert_eq!(host.source_interrupted, interrupted);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numeric);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let resumed = host.resume_decoder(host.revision(), numeric.actor_revision, numeric.position);
        assert_eq!(resumed.is_ok(), !interrupted);
        assert_eq!(host.source_interrupted, interrupted);
        assert_eq!(host.decoder_inspection().unwrap().paused, interrupted);
    }
}

#[test]
fn text_generation_cancel_rejects_bare_ids_and_stale_text_without_touching_work() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let raw = raw(&host, 7, &[98], 3); host.begin_decoder_generation(host.revision(), raw).unwrap();
    let before = bytes(&host);
    assert!(matches!(host.cancel_decoder_text(host.revision(), 7, 0), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(bytes(&host), before); assert!(!host.decoder_inspection().unwrap().paused);
    host.cancel_decoder_generation(host.revision(), 7, 0).unwrap(); resume_after_cancel(&mut host, 2);
    begin_text(&mut host, 8, b"ab", 3); step(&mut host, 8);
    let before = bytes(&host); let numeric = host.decoder_inspection().unwrap();
    for (journal, generation) in [(host.revision(), 0), (host.revision() - 1, 1), (host.revision(), 2)] {
        assert!(matches!(host.cancel_decoder_text(journal, 8, generation), Err(JournalError::Contract(Error::Stale))));
        assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), numeric);
    }
    let cancelled = host.cancel_decoder_text(host.revision(), 8, 1).unwrap();
    let before = bytes(&host);
    assert_eq!(host.advance_decoder_text(0, 8, 0).unwrap().finish(), cancelled.finish());
    assert_eq!(host.cancel_decoder_text(0, 8, 1).unwrap().bytes(), cancelled.bytes());
    assert_eq!(bytes(&host), before);
}

#[test]
fn text_generation_cancel_survives_exact_history_guarded_recovery() {
    let root = Directory::new(); let c = config(3.0, 65); let t = tokenizer(false);
    let mut host = owner(&root, &c); begin_text(&mut host, 7, b"ab", 3);
    for _ in 0..3 { step(&mut host, 7); }
    let anchor = host.history_anchor().unwrap(); let control = host.inspect().control;
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: Some(c), decoder_stop: None,
            source: None, identity: None, campaigns: None, credential: None },
        effective_policy: host_profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch },
    };
    let cancelled = host.cancel_decoder_text(host.revision(), 7, 3).unwrap();
    let numeric = host.decoder_inspection().unwrap().numerical; drop(host);
    let (mut host, roles) = FileOversight::open_guarded_text_anchored(root.store(), host_profile(), &expected, &t, &anchor).unwrap();
    assert!(roles.identity_observer.is_none()); assert!(roles.policy_governor.is_none());
    assert_eq!(host.decoder_inspection().unwrap().numerical, numeric);
    let before = bytes(&host);
    let retry = host.cancel_decoder_text(0, 7, 3).unwrap();
    assert_eq!(retry.bytes(), cancelled.bytes()); assert_eq!(retry.finish(), cancelled.finish());
    assert_eq!(bytes(&host), before); assert!(!host.clock_ready());
    assert!(host.pending_decoder_text().unwrap().is_none());
}

#[test]
fn generation_cancel_codec_has_exact_tag_and_rejects_truncated_or_invalid_keys() {
    use super::super::super::{read, write};
    let event = DecoderEvent::CancelGeneration { id: 7, revision: 3 };
    let golden: [u8; 17] = [11, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 3];
    let mut writer = Writer::new(17); write(&mut writer, &event).unwrap(); assert_eq!(writer.finish(), golden);
    let mut reader = Reader::new(&golden);
    assert!(matches!(read(&mut reader).unwrap(), DecoderEvent::CancelGeneration { id: 7, revision: 3 }));
    reader.end().unwrap();
    for end in 0..17 { assert!(read(&mut Reader::new(&golden[..end])).is_err()); }
    for (id, revision, error) in [(0, 3, Error::InvalidInput), (7, MAX_GENERATION_TOKENS as u64, Error::Limit)] {
        let mut writer = Writer::new(17);
        assert_eq!(write(&mut writer, &DecoderEvent::CancelGeneration { id, revision }), Err(error));
        let mut bytes = golden; bytes[1..9].copy_from_slice(&id.to_be_bytes()); bytes[9..].copy_from_slice(&revision.to_be_bytes());
        assert!(matches!(read(&mut Reader::new(&bytes)), Err(found) if found == error));
    }
    // The old resume record remains byte-identical, not renumbered by the new tag.
    let mut writer = Writer::new(17); write(&mut writer, &DecoderEvent::Resume { revision: 7, position: 3 }).unwrap();
    let mut original = golden; original[0] = 3; assert_eq!(writer.finish(), original);
    let root = Directory::new(); let c = config(3.0, 65); let mut host = owner(&root, &c);
    begin_text(&mut host, 7, b"ab", 2); host.cancel_decoder_text(host.revision(), 7, 0).unwrap();
    let mut invalid = host.events.clone(); invalid.push(Event::Decoder(event));
    let bad = journal::encode(&host.profile, host.store.identity(), &invalid).unwrap();
    let file = root.store().join(storage::CANONICAL); drop(host); std::fs::write(&file, &bad).unwrap();
    assert!(FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &tokenizer(false)).is_err());
    assert_eq!(std::fs::read(file).unwrap(), bad);
}
