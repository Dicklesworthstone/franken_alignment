//! Original per-token journal cuts, exact byte cursors and real-file recovery.
use super::*;
use super::super::progress::FileTextGenerationProgress;

fn start(host: &mut FileOversight, id: u64, input: TextGenerationRequest)
    -> (FileTextGenerationCommand, FileTextGenerationProgress)
{
    let command = command(host, id, input);
    host.begin_decoder_text(host.revision(), command.clone()).unwrap();
    let progress = host.decoder_text_progress(id).unwrap();
    (command, progress)
}
fn advance(host: &mut FileOversight, progress: &FileTextGenerationProgress) -> FileTextGenerationProgress {
    host.advance_decoder_text(host.revision(), progress.command().id(), progress.generation_revision()).unwrap()
}
fn resume(host: &mut FileOversight) {
    let numerical = host.decoder_inspection().unwrap().numerical;
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).unwrap();
}

#[test]
fn incremental_text_matches_whole_run_and_old_revision_retries_do_not_duplicate_bytes() {
    let root = Directory::new(); let whole_root = Directory::new(); let c = config(3.0, 65);
    let mut host = owner(&root, &c); let mut whole = owner(&whole_root, &c);
    let input = request(b"ab\xff", 3);
    let (intent, mut progress) = start(&mut host, 7, input.clone());
    let whole_command = command(&whole, 7, input);
    let expected = whole.generate_decoder_text(whole.revision(), whole_command).unwrap();
    assert_eq!(host.pending_decoder_text().unwrap().unwrap(), intent);
    assert_eq!(progress.generation_revision(), 0);
    assert!(!progress.is_complete()); assert_eq!(progress.finish(), None);
    assert!(progress.bytes().unwrap().is_empty());
    let mut copied = Vec::new();
    for step in 1..=6 {
        let previous = progress.generation_revision();
        let journal_revision = host.revision();
        let before = host.decoder_inspection().unwrap().numerical;
        progress = advance(&mut host, &progress);
        assert_eq!(progress.generation_revision(), step);
        let after = host.decoder_inspection().unwrap().numerical;
        assert_eq!(after.position, before.position + 1);
        assert!(after.sampled_draws <= before.sampled_draws + 1);
        let delta = progress.delta_from(copied.len()).unwrap();
        assert_eq!(delta.request(), 7);
        assert_eq!(delta.byte_range().start, copied.len());
        copied.extend_from_slice(delta.bytes());
        let disk = bytes(&host); let inspection = host.decoder_inspection().unwrap();
        // Retry the LOST response under its old journal/generation predecessor.
        let repeated = host.advance_decoder_text(journal_revision, 7, previous).unwrap();
        assert_eq!(repeated.generation_revision(), progress.generation_revision());
        assert_eq!(repeated.bytes().unwrap(), copied);
        assert!(repeated.delta_from(copied.len()).unwrap().bytes().is_empty());
        assert_eq!(bytes(&host), disk); assert_eq!(host.decoder_inspection().unwrap(), inspection);
        assert!(matches!(repeated.delta_from(usize::MAX), Err(Error::Stale)));
    }
    assert_eq!(copied, b"AAA"); assert!(progress.is_complete());
    assert_eq!(progress.finish(), Some(Ok(GenerationFinish::TokenLimit)));
    assert!(host.pending_decoder_text().unwrap().is_none());
    let complete = host.decoder_text_generation(7).unwrap();
    assert_eq!(complete.result().unwrap().generation().work(), expected.result().unwrap().generation().work());
    assert_eq!(complete.result().unwrap().bytes().unwrap(), expected.result().unwrap().bytes().unwrap());
    assert_eq!(host.machine.broker.hosted_replay_bytes().unwrap(), whole.machine.broker.hosted_replay_bytes().unwrap());
    assert_eq!(host.inspect().control, whole.inspect().control);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn every_acknowledged_text_boundary_recovers_exact_cursor_bytes_and_remaining_budget() {
    for cut in 0..=4 {
        let root = Directory::new(); let c = config(3.0, 65); let tokenizer = tokenizer(false);
        let mut host = owner(&root, &c);
        let (command, mut progress) = start(&mut host, 7, request(b"ab", 2));
        for _ in 0..cut { progress = advance(&mut host, &progress); }
        let retained = progress.bytes().unwrap().to_vec();
        let disk_before = bytes(&host); let revision = host.revision();
        let image = FileOversight::read_decoder_text_progress(root.store(), &host_profile(), &c, &tokenizer, 7).unwrap();
        assert_eq!(image.publication.revision, revision);
        assert_eq!(image.text.bytes().unwrap(), retained);
        assert_eq!(image.text.generation_revision(), progress.generation_revision());
        assert_eq!(image.numerical.numerical, host.decoder_inspection().unwrap().numerical);
        assert_eq!(bytes(&host), disk_before); // read did not append a fence
        drop(host);
        let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &tokenizer).unwrap();
        let current = host.decoder_text_progress(7).unwrap();
        assert_eq!(current.bytes().unwrap(), retained);
        assert_eq!(current.generation_revision(), progress.generation_revision());
        assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
        if !current.is_complete() {
            let before = bytes(&host);
            assert!(matches!(host.advance_decoder_text(host.revision(), 7, current.generation_revision()),
                Err(JournalError::Contract(Error::Incomplete))));
            assert_eq!(bytes(&host), before);
            if cut > 0 {
                let retried = host.advance_decoder_text(0, 7, current.generation_revision() - 1).unwrap();
                assert_eq!(retried.bytes().unwrap(), retained); assert_eq!(bytes(&host), before);
            }
            resume(&mut host);
            if cut > 0 {
                let before = bytes(&host);
                assert!(matches!(host.generate_decoder_text(host.revision(), command.clone()),
                    Err(JournalError::Contract(Error::WrongState))));
                assert_eq!(bytes(&host), before); // no whole-run restart of a partial cursor
            }
        }
        progress = current;
        let mut delivered = retained;
        while !progress.is_complete() {
            progress = advance(&mut host, &progress);
            delivered.extend_from_slice(progress.delta_from(delivered.len()).unwrap().bytes());
        }
        assert_eq!(delivered, b"AA");
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(progress.numerical().receipt().unwrap().result().unwrap().work().attempted_samples, 2);
        assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"AA");
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn split_unicode_and_multibyte_token_cursors_remain_exact_byte_offsets() {
    let root = Directory::new(); let c = split_utf8_config(); let mut host = owner(&root, &c);
    let (_, mut progress) = start(&mut host, 7, request(b"ab", 2));
    for _ in 0..2 { progress = advance(&mut host, &progress); }
    progress = advance(&mut host, &progress);
    assert_eq!(progress.bytes().unwrap(), &[0xc3]);
    assert!(progress.utf8().is_err());
    assert!(!progress.is_complete());
    let prefix = progress.delta_from(0).unwrap();
    assert_eq!(prefix.byte_range(), 0..1);
    progress = advance(&mut host, &progress);
    assert_eq!(progress.utf8().unwrap(), "é");
    assert_eq!(progress.numerical().tokens(), &[0xc3, 0xa9]);
    let suffix = progress.delta_from(1).unwrap();
    assert_eq!(suffix.byte_range(), 1..2); assert_eq!(suffix.bytes(), &[0xa9]);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);

    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 258));
    let (_, mut progress) = start(&mut host, 7, request(b"ab", 1));
    while !progress.is_complete() { progress = advance(&mut host, &progress); }
    assert_eq!(progress.numerical().tokens(), &[258]);
    assert_eq!(progress.bytes().unwrap(), "é".as_bytes());
    // A downstream sink can acknowledge the first byte of ONE original token.
    assert_eq!(progress.delta_from(1).unwrap().bytes(), &[0xa9]);
    assert!(progress.delta_from(2).unwrap().bytes().is_empty());
    assert!(matches!(progress.delta_from(3), Err(Error::Stale)));
}

#[test]
fn held_control_and_native_refusal_stay_distinct_from_empty_pending_output() {
    for (threshold, output, expected, draws) in [
        (1.5, 65, Ok(GenerationFinish::Held), 1),
        (3.0, 256, Ok(GenerationFinish::StopToken), 1),
        (3.0, 65, Err(Error::Limit), 0),
    ] {
        let root = Directory::new(); let c = config(threshold, output); let mut host = owner(&root, &c);
        if threshold < 2.0 {
            host.enable_decoder_stop(host.revision(), HostedStopPolicy::new(7, 1, 900).unwrap()).unwrap();
        }
        let mut input = request(b"ab", 2);
        if expected.is_err() { input.generation.scalar_products = 0; }
        let (_, mut progress) = start(&mut host, 7, input);
        assert_eq!(progress.finish(), None); assert!(progress.bytes().unwrap().is_empty());
        while !progress.is_complete() { progress = advance(&mut host, &progress); }
        assert_eq!(progress.finish(), Some(expected));
        if let Err(error) = expected {
            assert_eq!(progress.bytes(), Err(error));
            assert!(matches!(progress.delta_from(0), Err(Error::Limit)));
            assert_eq!(progress.numerical().receipt().unwrap().result().unwrap_err(), error);
        } else { assert!(progress.bytes().unwrap().is_empty()); }
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, draws);
        let before = bytes(&host);
        let retry = host.advance_decoder_text(0, 7, progress.generation_revision()).unwrap();
        assert_eq!(retry.finish(), Some(expected)); assert_eq!(bytes(&host), before);
        if threshold < 2.0 {
            assert!(host.inspect().control.suspended);
            assert_eq!(host.decoder_inspection().unwrap().numerical.status, MonitoringStatus::Held);
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn progress_refusals_preserve_source_latch_and_never_admit_a_bare_id_as_text() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let (_, mut progress) = start(&mut host, 7, request(b"ab", 2));
    let disk = bytes(&host);
    assert!(matches!(host.advance_decoder_text(0, 7, 0), Err(JournalError::Contract(Error::Stale))));
    assert!(matches!(host.advance_decoder_text(host.revision(), 7, 1), Err(JournalError::Contract(Error::Stale))));
    assert!(matches!(host.advance_decoder_text(host.revision(), 99, 0), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(bytes(&host), disk);
    progress = advance(&mut host, &progress);
    let disk = bytes(&host);
    host.source_interrupted = true;
    assert!(matches!(host.advance_decoder_text(host.revision(), 7, 1), Err(JournalError::Contract(Error::Incomplete))));
    assert!(host.source_interrupted); assert_eq!(bytes(&host), disk);
    assert_eq!(host.advance_decoder_text(0, 7, 0).unwrap().generation_revision(), 1);
    assert!(host.source_interrupted); assert_eq!(bytes(&host), disk);
    assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), progress.generation_revision());

    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let n = host.decoder_inspection().unwrap().numerical;
    let c = FileGenerationCommand::new(8, n.actor_revision, n.position, GenerationRequest {
        prompt: vec![257], max_new_tokens: 1, stop_tokens: vec![256], budget: request(b"ab", 1).generation,
    }).unwrap();
    host.begin_decoder_generation(host.revision(), c).unwrap();
    assert!(host.pending_decoder_text().unwrap().is_none());
    assert!(host.pending_decoder_generation().unwrap().is_some());
    let disk = bytes(&host);
    assert!(matches!(host.advance_decoder_text(host.revision(), 8, 0), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(bytes(&host), disk);
}

#[test]
fn complete_incremental_capacity_is_admitted_before_the_first_text_token() {
    for limit in [7, 8] {
        let root = Directory::new(); let c = config(3.0, 65);
        let mut p = host_profile(); p.delivery.limits.events = limit;
        let mut host = owner_with_profile(&root, &c, p);
        let (_, mut progress) = start(&mut host, 7, request(b"ab", 2));
        assert_eq!(host.revision(), 4); let before = bytes(&host);
        if limit == 7 {
            assert!(matches!(host.advance_decoder_text(host.revision(), 7, 0), Err(JournalError::Contract(Error::Limit))));
            assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
            assert!(host.storage_failure().is_none());
        } else {
            while !progress.is_complete() { progress = advance(&mut host, &progress); }
            assert_eq!(host.revision(), 8); assert_eq!(progress.bytes().unwrap(), b"AA");
            let before = bytes(&host);
            assert_eq!(host.advance_decoder_text(0, 7, 0).unwrap().bytes().unwrap(), b"AA");
            assert_eq!(bytes(&host), before);
        }
    }
}

#[test]
fn output_cut_failures_expose_no_candidate_bytes_and_recover_only_disk_acknowledgment_state() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let c = config(3.0, 65); let t = tokenizer(false);
        let mut host = owner(&root, &c);
        let (_, mut progress) = start(&mut host, 7, request(b"ab", 2));
        for _ in 0..2 { progress = advance(&mut host, &progress); }
        assert!(progress.bytes().unwrap().is_empty()); let revision = host.revision();
        let numerical = host.decoder_inspection().unwrap().numerical;
        host.store.fail_once(barrier);
        assert!(matches!(host.advance_decoder_text(revision, 7, 2), Err(JournalError::Io(_))));
        assert!(matches!(host.decoder_text_progress(7), Err(JournalError::Unavailable)));
        assert!(matches!(host.pending_decoder_text(), Err(JournalError::Unavailable)));
        assert!(matches!(host.advance_decoder_text(0, 7, 0), Err(JournalError::Unavailable)));
        let image = FileOversight::read_decoder_text_progress(root.store(), &host_profile(), &c, &t, 7).unwrap();
        let replaced = barrier == JournalIo::DirectorySync;
        assert_eq!(image.publication.revision, revision + u64::from(replaced));
        assert_eq!(image.text.bytes().unwrap(), if replaced { &b"A"[..] } else { &b""[..] });
        assert_eq!(image.numerical.numerical.sampled_draws, numerical.sampled_draws + u64::from(replaced));
        assert_eq!(image.text.generation_revision(), 2 + u64::from(replaced));
        assert_eq!(image.publication.executions, 0);
        drop(host);
        let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &t).unwrap();
        resume(&mut host);
        progress = host.decoder_text_progress(7).unwrap();
        while !progress.is_complete() { progress = advance(&mut host, &progress); }
        assert_eq!(progress.bytes().unwrap(), b"AA");
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn canonical_progress_requires_exact_tokenizer_and_rejects_corrupt_native_witnesses() {
    let root = Directory::new(); let c = config(3.0, 65); let mut host = owner(&root, &c);
    let (_, mut progress) = start(&mut host, 7, request(b"ab", 2));
    for _ in 0..3 { progress = advance(&mut host, &progress); }
    let before = bytes(&host);
    assert!(matches!(FileOversight::read_decoder_text_progress(root.store(), &host_profile(), &c, &tokenizer(true), 7),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(bytes(&host), before);
    let image = FileOversight::read_decoder_text_progress(root.store(), &host_profile(), &c, &tokenizer(false), 7).unwrap();
    assert_eq!(image.text.bytes().unwrap(), b"A"); assert_eq!(bytes(&host), before);
    let mut events = host.events.clone();
    let Event::Decoder(DecoderEvent::AdvanceGeneration { witness, .. }) = events.last_mut().unwrap() else { panic!("actual native progress witness"); };
    let mut changed = witness.to_vec(); let last = changed.len() - 1; changed[last] ^= 1;
    *witness = changed.into();
    let corrupted = journal::encode(&host_profile(), host.store.identity(), &events).unwrap();
    drop(host);
    let path = root.store().join(storage::CANONICAL);
    std::fs::write(&path, &corrupted).unwrap();
    assert!(matches!(FileOversight::read_decoder_text_progress(root.store(), &host_profile(), &c, &tokenizer(false), 7),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(path).unwrap(), corrupted);
}

#[test]
fn resumed_text_cannot_replenish_the_original_sampling_allowance() {
    let root = Directory::new(); let c = config(3.0, 65); let t = tokenizer(false);
    let mut host = owner(&root, &c);
    let mut input = request(b"ab", 3);
    input.generation.sampling_entries = 259; // exactly ONE original vocabulary scan
    let (_, mut progress) = start(&mut host, 7, input);
    for _ in 0..3 { progress = advance(&mut host, &progress); }
    assert_eq!(progress.bytes().unwrap(), b"A"); assert!(!progress.is_complete());
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &t).unwrap();
    resume(&mut host);
    progress = host.decoder_text_progress(7).unwrap();
    let before = host.decoder_inspection().unwrap().numerical;
    progress = advance(&mut host, &progress);
    assert_eq!(progress.finish(), Some(Ok(GenerationFinish::BudgetExhausted)));
    assert_eq!(progress.bytes().unwrap(), b"A");
    assert!(progress.delta_from(1).unwrap().bytes().is_empty());
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, before.position);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    let report = progress.numerical().receipt().unwrap().result().unwrap();
    assert_eq!(report.work().admitted_sampling_entries, 259);
    assert_eq!(report.work().attempted_samples, 1);
    assert_eq!(host.inspect().executions, 0);
}
