//! Text consumers use the same batch; no approximate re-tokenization or sink.
use super::*;
use crate::action::consequence::delivery::persistent::observed::decoder::text::progress::FileTextGenerationProgress;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryRequirements, FileRecoveryFloor,
};
use crate::action::consequence::delivery::persistent::{Event as BaseEvent, RecoveryReserve};
use crate::action::consequence::delivery::StopRequest;

fn start_text(host: &mut FileOversight, id: u64, input: TextGenerationRequest) -> FileTextGenerationCommand {
    let intent = command(host, id, input);
    host.begin_decoder_text(host.revision(), intent.clone()).unwrap(); intent
}
fn text_batch(host: &mut FileOversight, id: u64, steps: usize) -> FileTextGenerationProgress {
    let g = host.decoder_text_progress(id).unwrap().generation_revision();
    host.advance_decoder_text_batch(host.revision(), id, g, steps).unwrap()
}
fn requirements(host: &FileOversight, c: &FileDecoderConfig) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: Some(c.clone()), decoder_stop: None,
            source: None, identity: None, campaigns: None, credential: None },
        effective_policy: host_profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch },
    }
}

#[test]
fn generation_batch_text_deltas_match_original_ids_and_never_repeat_retry_bytes() {
    for size in [1, 3, 64] {
        let root = Directory::new(); let reference_root = Directory::new(); let c = config(3.0, 258);
        let mut host = owner(&root, &c); let mut reference = owner(&reference_root, &c);
        let input = request(b"ab\xff", 2);
        let intent = start_text(&mut host, 7, input.clone());
        assert_eq!(start_text(&mut reference, 7, input), intent);
        let mut delivered = Vec::new();
        while !host.decoder_text_progress(7).unwrap().is_complete() {
            let old = host.decoder_text_progress(7).unwrap().generation_revision();
            let progress = text_batch(&mut host, 7, size);
            for _ in old..progress.generation_revision() { single(&mut reference, 7); }
            equal_history(&host, &reference);
            delivered.extend_from_slice(progress.delta_from(delivered.len()).unwrap().bytes());
            let before = bytes(&host); let numerical = host.decoder_inspection().unwrap();
            let retried = host.advance_decoder_text_batch(0, 7, old, 64).unwrap();
            assert_eq!(retried.bytes().unwrap(), delivered);
            assert!(retried.delta_from(delivered.len()).unwrap().bytes().is_empty());
            assert!(matches!(retried.delta_from(delivered.len() + 1), Err(Error::Stale)));
            assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
        }
        assert_eq!(delivered, "éé".as_bytes());
        let report = host.decoder_text_generation(7).unwrap();
        assert_eq!(report.result().unwrap().prompt().tokens(), &[257, 255]);
        assert_eq!(report.result().unwrap().prefix_controls(), &[256]);
        assert_eq!(report.result().unwrap().generation().tokens(), &[258, 258]);
        assert_eq!(report.result().unwrap().generation().work().attempted_samples, 2);
    }
}

#[test]
fn generation_batch_split_utf8_survives_reopen_without_an_extra_completion_token() {
    let root = Directory::new(); let c = split_utf8_config(); let t = tokenizer(false);
    let mut host = owner(&root, &c);
    start_text(&mut host, 7, request(b"ab", 2));
    let prefix = text_batch(&mut host, 7, 3);
    assert_eq!(prefix.bytes().unwrap(), &[0xc3]); assert!(prefix.utf8().is_err());
    assert_eq!(prefix.numerical().tokens(), &[0xc3]);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &t).unwrap();
    let before = bytes(&host);
    let retry = host.advance_decoder_text_batch(0, 7, 0, 64).unwrap();
    assert_eq!(retry.bytes().unwrap(), &[0xc3]); assert_eq!(bytes(&host), before);
    assert!(matches!(host.advance_decoder_text_batch(host.revision(), 7, 3, 1),
        Err(JournalError::Contract(Error::Incomplete))));
    resume(&mut host);
    let done = text_batch(&mut host, 7, 1);
    assert_eq!(done.utf8().unwrap(), "é"); assert_eq!(done.delta_from(1).unwrap().bytes(), &[0xa9]);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
    assert_eq!(done.finish(), Some(Ok(GenerationFinish::TokenLimit)));
}

#[test]
fn generation_batch_cancellation_between_calls_keeps_cache_and_cannot_pause_a_new_job() {
    let root = Directory::new(); let c = config(3.0, 65); let mut host = owner(&root, &c);
    start_text(&mut host, 7, request(b"ab", 3));
    let prefix = text_batch(&mut host, 7, 3); assert_eq!(prefix.bytes().unwrap(), b"A");
    let numerical = host.decoder_inspection().unwrap().numerical;
    let cancelled = host.cancel_decoder_text(host.revision(), 7, 3).unwrap();
    assert_eq!(cancelled.finish(), Some(Ok(GenerationFinish::Cancelled)));
    assert_eq!(cancelled.bytes().unwrap(), b"A");
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert!(!host.clock_ready()); resume(&mut host);
    let mut next = request(b"", 1); next.prefix_controls.clear();
    start_text(&mut host, 8, next);
    let before = bytes(&host);
    let old = host.advance_decoder_text_batch(0, 7, 0, 64).unwrap();
    assert_eq!(old.finish(), Some(Ok(GenerationFinish::Cancelled)));
    assert_eq!(bytes(&host), before); assert!(!host.decoder_inspection().unwrap().paused);
    let done = text_batch(&mut host, 8, 64);
    assert_eq!(done.bytes().unwrap(), b"A");
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn generation_batch_text_holds_stops_and_refusals_are_not_empty_successes() {
    for mode in 0..4 {
        let root = Directory::new();
        let c = config(if mode == 0 { 1.5 } else { 3.0 }, if mode == 1 { 256 } else { 65 });
        let mut host = owner(&root, &c); let mut r = request(b"ab", 2);
        if mode == 2 { r.generation.scalar_products = 0; }
        if mode == 3 { r.generation.sampling_entries = 259; }
        start_text(&mut host, 7, r);
        let p = text_batch(&mut host, 7, 64);
        let finish = match mode {
            0 => Ok(GenerationFinish::Held), 1 => Ok(GenerationFinish::StopToken),
            2 => Err(Error::Limit), _ => Ok(GenerationFinish::BudgetExhausted),
        };
        assert_eq!(p.finish(), Some(finish));
        if mode == 2 { assert_eq!(p.bytes(), Err(Error::Limit)); }
        else { assert_eq!(p.bytes().unwrap(), if mode == 3 { b"A".as_slice() } else { b"" }); }
        let before = bytes(&host);
        let retry = host.advance_decoder_text_batch(0, 7, 0, 64).unwrap();
        assert_eq!(retry.finish(), Some(finish)); assert_eq!(bytes(&host), before);
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, if mode == 2 { 0 } else { 1 });
    }
}

#[test]
fn generation_batch_text_refuses_bare_ids_bad_bounds_and_source_interruption_before_work() {
    let root = Directory::new(); let c = config(3.0, 65); let mut host = owner(&root, &c);
    begin(&mut host, 7, input(&[257], 1));
    let before = bytes(&host);
    assert!(matches!(host.advance_decoder_text_batch(host.revision(), 7, 0, 2),
        Err(JournalError::Contract(Error::Missing))));
    assert_eq!(bytes(&host), before);
    host.cancel_decoder_generation(host.revision(), 7, 0).unwrap(); resume(&mut host);
    start_text(&mut host, 8, request(b"ab", 2));
    let before = bytes(&host); let revision = host.revision();
    for (j, g, count, error) in [(revision, 0, 0, Error::InvalidInput),
        (revision, 0, 65, Error::Limit), (revision - 1, 0, 2, Error::Stale),
        (revision, 1, 2, Error::Stale)] {
        assert!(matches!(host.advance_decoder_text_batch(j, 8, g, count),
            Err(JournalError::Contract(actual)) if actual == error));
        assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_none());
    }
    host.source_interrupted = true;
    assert!(matches!(host.advance_decoder_text_batch(host.revision(), 8, 0, 2),
        Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(bytes(&host), before); host.source_interrupted = false;
    assert_eq!(text_batch(&mut host, 8, 1).generation_revision(), 1);
    host.source_interrupted = true; let before = bytes(&host);
    assert_eq!(host.advance_decoder_text_batch(0, 8, 0, 64).unwrap().generation_revision(), 1);
    assert!(host.source_interrupted); assert_eq!(bytes(&host), before);
}

#[test]
fn generation_batch_text_storage_barriers_withhold_the_entire_new_byte_suffix() {
    for barrier in BARRIERS {
        let root = Directory::new(); let c = config(3.0, 65); let t = tokenizer(false);
        let mut host = owner(&root, &c); start_text(&mut host, 7, request(b"ab", 3));
        let revision = host.revision(); host.store.fail_once(barrier);
        assert!(matches!(host.advance_decoder_text_batch(revision, 7, 0, 4), Err(JournalError::Io(_))));
        assert!(matches!(host.decoder_text_progress(7), Err(JournalError::Unavailable)));
        assert!(matches!(host.advance_decoder_text_batch(0, 7, 0, 64), Err(JournalError::Unavailable)));
        let image = FileOversight::read_decoder_text_progress(root.store(), &host_profile(), &c, &t, 7).unwrap();
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(image.publication.revision, revision + if visible { 4 } else { 0 });
        assert_eq!(image.text.generation_revision(), if visible { 4 } else { 0 });
        assert_eq!(image.text.bytes().unwrap(), if visible { b"AA".as_slice() } else { b"" });
        assert_eq!(image.numerical.numerical.sampled_draws, if visible { 2 } else { 0 });
        drop(host);
        let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &t).unwrap();
        resume(&mut host); let final_progress = text_batch(&mut host, 7, 64);
        assert_eq!(final_progress.bytes().unwrap(), b"AAA");
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 3);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn generation_batch_anchored_text_recovery_preserves_every_cut_and_remaining_work() {
    for cut in 1..=5 {
        let root = Directory::new(); let c = config(3.0, 65); let t = tokenizer(false);
        let mut host = owner(&root, &c); start_text(&mut host, 7, request(b"ab", 3));
        let previous = text_batch(&mut host, 7, cut);
        let numerical = host.decoder_inspection().unwrap().numerical;
        let expected = requirements(&host, &c); let anchor = host.history_anchor().unwrap();
        let retained = previous.bytes().unwrap().to_vec(); drop(host);
        let (mut host, roles) = FileOversight::open_guarded_text_anchored(
            root.store(), host_profile(), &expected, &t, &anchor).unwrap();
        assert!(roles.identity_observer.is_none()); assert!(roles.policy_governor.is_none());
        assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        let before = bytes(&host);
        let retry = host.advance_decoder_text_batch(0, 7, 0, 64).unwrap();
        assert_eq!(retry.generation_revision(), cut as u64);
        assert_eq!(retry.bytes().unwrap(), retained); assert_eq!(bytes(&host), before);
        if !retry.is_complete() { resume(&mut host); }
        let done = text_batch(&mut host, 7, 64);
        assert_eq!(done.bytes().unwrap(), b"AAA");
        assert_eq!(done.numerical().receipt().unwrap().result().unwrap().work().attempted_samples, 3);
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 3);
        assert!(host.pending_decoder_text().unwrap().is_none());
    }
}

#[test]
fn generation_batch_preserves_terminal_recovery_reserve_instead_of_publishing_a_shorter_batch() {
    let root = Directory::new(); let c = config(3.0, 65); let t = tokenizer(false);
    let mut profile = host_profile(); profile.delivery.limits.events = 12;
    let (mut host, _) = FileOversight::create(root.store(), profile.clone()).unwrap();
    host.transact(host.revision(), Event::Core(BaseEvent::ReserveRecovery(RecoveryReserve::terminal()))).unwrap();
    host.enable_decoder(host.revision(), c.clone()).unwrap();
    host.enable_decoder_tokenizer(host.revision(), t.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    start_text(&mut host, 7, request(b"ab", 3));
    assert_eq!(text_batch(&mut host, 7, 2).generation_revision(), 2);
    assert_eq!(host.revision(), 7); let before = bytes(&host);
    // Ten records fit the hard ceiling, but not its nine-record ordinary lane.
    assert!(matches!(host.advance_decoder_text_batch(host.revision(), 7, 2, 3),
        Err(JournalError::Contract(Error::Limit))));
    assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_some());
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), profile, &c, &t).unwrap();
    assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 2);
    assert_eq!(host.revision(), 8);
    let control = host.inspect().control;
    let stop = StopRequest { operation: 99, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch };
    host.transact(host.revision(), Event::Core(BaseEvent::Stop(stop))).unwrap();
    host.transact(host.revision(), Event::Core(BaseEvent::StopProgress(ElapsedTick(2)))).unwrap();
    assert_eq!(host.revision(), 10); assert!(host.inspect().stop.is_some());
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 0);
    assert_eq!(host.inspect().executions, 0);
}
