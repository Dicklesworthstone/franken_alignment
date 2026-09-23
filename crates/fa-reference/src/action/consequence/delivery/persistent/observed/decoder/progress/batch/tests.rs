//! Original numerical engine, original single-step path, and real journal files.
//! Synthetic weights are not trained-checkpoint or deployment qualification.
use super::*;
use super::super::{FileDecoderConfig, FileGenerationCommand, FileOversightProfile};
use super::super::super::text::FileTextGenerationCommand;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationFinish, GenerationRequest, MAX_SAMPLING_ENTRIES,
    text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::action::consequence::oversight::decoder_host::HostedStopPolicy;

// Reuse the unchanged byte-level fixture; do not implement another decoder or
// replace numerical outcomes with a callback. Some text helpers are unused here.
#[allow(dead_code)]
#[path = "../../text/tests/fixtures.rs"]
mod fixtures;
use fixtures::*;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];

fn input(prompt: &[u32], new: usize) -> GenerationRequest {
    GenerationRequest { prompt: prompt.to_vec(), max_new_tokens: new, stop_tokens: vec![256],
        budget: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS,
            sampling_entries: MAX_SAMPLING_ENTRIES } }
}
fn begin(host: &mut FileOversight, id: u64, input: GenerationRequest) -> FileGenerationCommand {
    let n = host.decoder_inspection().unwrap().numerical;
    let command = FileGenerationCommand::new(id, n.actor_revision, n.position, input).unwrap();
    host.begin_decoder_generation(host.revision(), command.clone()).unwrap();
    command
}
fn batch(host: &mut FileOversight, id: u64, steps: usize) -> FileGenerationProgress {
    let revision = host.decoder_generation_progress(id).unwrap().generation_revision();
    host.advance_decoder_generation_batch(host.revision(), id, revision, steps).unwrap()
}
fn single(host: &mut FileOversight, id: u64) -> FileGenerationProgress {
    let revision = host.decoder_generation_progress(id).unwrap().generation_revision();
    host.advance_decoder_generation(host.revision(), id, revision).unwrap()
}
fn resume(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
}
fn equal_history(actual: &FileOversight, expected: &FileOversight) {
    // Only the storage path differs. All original event/witness bytes must match,
    // including intermediate logits, held results and admitted work counters.
    let reference = journal::encode(&actual.profile, actual.store.identity(), &expected.events).unwrap();
    assert_eq!(bytes(actual), reference);
    assert_eq!(actual.machine.broker.hosted_replay_bytes().unwrap(),
        expected.machine.broker.hosted_replay_bytes().unwrap());
    assert_eq!(actual.decoder_inspection().unwrap(), expected.decoder_inspection().unwrap());
    assert_eq!(actual.inspect().control, expected.inspect().control);
    assert_eq!(actual.inspect().executions, 0);
}

#[test]
fn generation_batch_matches_single_step_records_at_every_bounded_cut() {
    for size in [1, 2, 3, MAX_FILE_GENERATION_BATCH_STEPS] {
        let root = Directory::new(); let reference_root = Directory::new();
        let c = config(3.0, 65);
        let mut host = owner(&root, &c); let mut reference = owner(&reference_root, &c);
        let command = begin(&mut host, 7, input(&[256, 257], 3));
        assert_eq!(begin(&mut reference, 7, input(&[256, 257], 3)), command);
        while !host.decoder_generation_progress(7).unwrap().is_complete() {
            let previous = host.decoder_generation_progress(7).unwrap().generation_revision();
            let revision = host.revision();
            let progress = batch(&mut host, 7, size);
            let advanced = progress.generation_revision() - previous;
            assert!(advanced > 0 && advanced <= size as u64);
            assert_eq!(host.revision(), revision + advanced);
            for _ in 0..advanced { single(&mut reference, 7); }
            equal_history(&host, &reference);
        }
        let report = host.decoder_generation(7).unwrap();
        assert_eq!(report.result().unwrap().tokens(), &[65, 65, 65]);
        assert_eq!(report.result().unwrap().reviewed_prompt_tokens(), 2);
        assert_eq!(report.result().unwrap().work().attempted_samples, 3);
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 3);
        assert_eq!(host.decoder_generation_progress(7).unwrap().generation_revision(), 5);
    }
}

#[test]
fn generation_batch_retry_bound_changes_are_read_only_and_do_not_touch_new_jobs() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    begin(&mut host, 7, input(&[257], 3));
    let progress = batch(&mut host, 7, 2);
    assert_eq!(progress.tokens(), &[65]);
    let before = bytes(&host); let numerical = host.decoder_inspection().unwrap();
    host.source_interrupted = true;
    for size in [1, 3, MAX_FILE_GENERATION_BATCH_STEPS] {
        let retry = host.advance_decoder_generation_batch(0, 7, 0, size).unwrap();
        assert_eq!(retry.generation_revision(), 2); assert_eq!(retry.tokens(), &[65]);
        assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
        assert!(host.source_interrupted);
    }
    assert!(matches!(host.advance_decoder_generation_batch(host.revision(), 7, 2, 2),
        Err(JournalError::Contract(Error::Incomplete))));
    host.source_interrupted = false;
    assert!(batch(&mut host, 7, 64).is_complete());
    begin(&mut host, 8, input(&[257], 1));
    let before = bytes(&host); let numerical = host.decoder_inspection().unwrap();
    let old = host.advance_decoder_generation_batch(0, 7, 0, 64).unwrap();
    assert_eq!(old.finish(), Some(Ok(GenerationFinish::TokenLimit)));
    assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
    assert_eq!(host.pending_decoder_generation().unwrap().unwrap().id(), 8);
    assert!(batch(&mut host, 8, 2).is_complete());
}

#[test]
fn generation_batch_structural_source_and_predecessor_refusals_do_no_work() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    begin(&mut host, 7, input(&[257], 3));
    let before = bytes(&host); let numerical = host.decoder_inspection().unwrap();
    let revision = host.revision();
    for (j, id, g, steps, error) in [
        (revision, 7, 0, 0, Error::InvalidInput),
        (revision, 7, 0, 65, Error::Limit),
        (revision, 7, 0, usize::MAX, Error::Limit),
        (revision - 1, 7, 0, 2, Error::Stale),
        (revision, 7, 1, 2, Error::Stale),
        (revision, 99, 0, 2, Error::Missing),
    ] {
        assert!(matches!(host.advance_decoder_generation_batch(j, id, g, steps),
            Err(JournalError::Contract(actual)) if actual == error));
        assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
        assert!(host.storage_failure().is_none());
    }
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), host_profile(), &config(3.0, 65)).unwrap();
    let before = bytes(&host);
    assert!(matches!(host.advance_decoder_generation_batch(host.revision(), 7, 0, 2),
        Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_none());
    resume(&mut host);
    assert_eq!(batch(&mut host, 7, 2).generation_revision(), 2);
}

#[test]
fn generation_batch_stops_at_native_hold_stop_budget_or_admission_failure() {
    for mode in 0..5 {
        let root = Directory::new(); let reference_root = Directory::new();
        let c = config(if mode == 0 { 1.5 } else { 3.0 }, if mode == 1 { 256 } else { 65 });
        let mut host = owner(&root, &c); let mut reference = owner(&reference_root, &c);
        if mode == 0 {
            let stop = HostedStopPolicy::new(7, 1, 900).unwrap();
            host.enable_decoder_stop(host.revision(), stop).unwrap();
            reference.enable_decoder_stop(reference.revision(), stop).unwrap();
        }
        let mut request = input(&[257], 3);
        if mode == 2 { request.budget.sampling_entries = 259; }
        if mode == 3 { request.budget.scalar_products = 0; }
        if mode == 4 { request.prompt = vec![259]; }
        begin(&mut host, 7, request.clone()); begin(&mut reference, 7, request);
        let progress = batch(&mut host, 7, 64);
        while !reference.decoder_generation_progress(7).unwrap().is_complete() { single(&mut reference, 7); }
        equal_history(&host, &reference);
        let (finish, transitions, draws) = match mode {
            0 => (Ok(GenerationFinish::Held), 2, 1),
            1 => (Ok(GenerationFinish::StopToken), 2, 1),
            2 => (Ok(GenerationFinish::BudgetExhausted), 3, 1),
            3 => (Err(Error::Limit), 1, 0),
            _ => (Err(Error::InvalidInput), 1, 0),
        };
        assert_eq!(progress.finish(), Some(finish));
        assert_eq!(progress.generation_revision(), transitions);
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, draws);
        assert_eq!(progress.tokens(), if mode == 2 { &[65][..] } else { &[] });
        if mode == 0 { assert!(host.inspect().stop.is_some()); }
    }
}

#[test]
fn generation_batch_checks_full_initial_and_later_worst_case_event_capacity() {
    for limit in [8, 9] {
        let root = Directory::new(); let c = config(3.0, 65);
        let mut profile = host_profile(); profile.delivery.limits.events = limit;
        let mut host = owner_with_profile(&root, &c, profile);
        begin(&mut host, 7, input(&[256, 257], 3));
        assert_eq!(host.revision(), 4);
        let before = bytes(&host);
        if limit == 8 {
            assert!(matches!(host.advance_decoder_generation_batch(host.revision(), 7, 0, 2),
                Err(JournalError::Contract(Error::Limit))));
            assert_eq!(bytes(&host), before);
            assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
            assert!(host.storage_failure().is_none());
        } else {
            assert_eq!(batch(&mut host, 7, 2).generation_revision(), 2);
            // Consume one ordinary slot outside generation. No promise silently
            // reserves it against supervisor work. A smaller valid batch still fits.
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            let before = bytes(&host);
            assert!(matches!(host.advance_decoder_generation_batch(host.revision(), 7, 2, 3),
                Err(JournalError::Contract(Error::Limit))));
            assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_none());
            assert_eq!(batch(&mut host, 7, 1).generation_revision(), 3);
        }
    }
}

#[test]
fn generation_batch_rejects_corrupted_middle_step_witness_on_original_replay() {
    let root = Directory::new(); let c = config(3.0, 65); let mut host = owner(&root, &c);
    begin(&mut host, 7, input(&[257], 3)); batch(&mut host, 7, 3);
    let mut events = host.events.clone();
    let Event::Decoder(DecoderEvent::AdvanceGeneration { witness, .. }) = &mut events[5]
        else { panic!("second original progress event"); };
    let mut corrupt = witness.to_vec(); let last = corrupt.len() - 1; corrupt[last] ^= 1;
    *witness = corrupt.into();
    let corrupt = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    let path = host.store.identity().join(super::super::storage::CANONICAL);
    drop(host); std::fs::write(&path, &corrupt).unwrap();
    assert!(FileOversight::open_with_decoder(root.store(), host_profile(), &c).is_err());
    assert_eq!(std::fs::read(path).unwrap(), corrupt);
}

#[test]
fn generation_batch_storage_faults_expose_old_or_whole_batch_never_first_step() {
    for barrier in BARRIERS {
        let root = Directory::new(); let c = config(3.0, 65); let mut host = owner(&root, &c);
        begin(&mut host, 7, input(&[257], 3));
        let before = bytes(&host); let revision = host.revision();
        host.store.fail_once(barrier);
        assert!(matches!(host.advance_decoder_generation_batch(revision, 7, 0, 3), Err(JournalError::Io(_))));
        assert_eq!(host.storage_failure().unwrap().operation, barrier);
        assert!(matches!(host.decoder_generation_progress(7), Err(JournalError::Unavailable)));
        assert!(matches!(host.advance_decoder_generation_batch(0, 7, 0, 1), Err(JournalError::Unavailable)));
        let image = FileOversight::read_decoder_generation_progress(root.store(), &host_profile(), &c, 7).unwrap();
        let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(image.publication.revision, revision + if visible { 3 } else { 0 });
        assert_eq!(image.generation.generation_revision(), if visible { 3 } else { 0 });
        assert_eq!(image.generation.tokens(), if visible { &[65, 65][..] } else { &[] });
        assert_eq!(image.numerical.numerical.sampled_draws, if visible { 2 } else { 0 });
        if !visible { assert_eq!(bytes(&host), before); }
        drop(host);
        let (mut host, _) = FileOversight::open_with_decoder(root.store(), host_profile(), &c).unwrap();
        assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
        resume(&mut host);
        let progress = batch(&mut host, 7, 64);
        assert_eq!(progress.tokens(), &[65, 65, 65]);
        assert_eq!(progress.finish(), Some(Ok(GenerationFinish::TokenLimit)));
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 3);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn generation_batch_late_byte_refusal_cannot_publish_a_smaller_prefix() {
    let reference_root = Directory::new(); let c = config(3.0, 65);
    let mut reference = owner(&reference_root, &c);
    begin(&mut reference, 7, input(&[257], 3));
    while !reference.decoder_generation_progress(7).unwrap().is_complete() { single(&mut reference, 7); }
    let root = Directory::new();
    let identity = std::fs::canonicalize(root.store().parent().unwrap()).unwrap().join("publication");
    let mut profile = host_profile();
    let final_size = journal::encode(&profile, &identity, &reference.events).unwrap().len();
    // Changing a fixed-width bound cannot change the encoded file length.
    profile.delivery.limits.bytes = final_size - 1;
    assert!(journal::encode(&profile, &identity, &reference.events).is_err());
    let mut host = owner_with_profile(&root, &c, profile);
    begin(&mut host, 7, input(&[257], 3));
    let before = bytes(&host);
    assert!(matches!(host.advance_decoder_generation_batch(host.revision(), 7, 0, 64),
        Err(JournalError::Contract(Error::Limit))));
    assert_eq!(bytes(&host), before);
    assert!(!host.storage_failure().unwrap().replacement_may_be_visible);
    assert!(matches!(host.decoder_generation_progress(7), Err(JournalError::Unavailable)));
    assert_eq!(host.inspect().executions, 0);
}

mod text;
