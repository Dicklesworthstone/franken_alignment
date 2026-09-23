//! Original synthetic-weight decoder and real journals; no invented token source.
use super::*;
use super::super::{FileDecoderConfig, FileOversightProfile};
use super::super::generation::{FileGenerationCommand, MAX_FILE_GENERATION_STEPS};
use super::super::text::FileTextGenerationCommand;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, GenerationFinish, GenerationRequest, GenerationWork,
    MAX_GENERATION_TOKENS, MAX_SAMPLING_ENTRIES,
    text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;

// Reuse the unchanged original durable-text fixture, not another numerical model.
#[allow(dead_code)]
#[path = "../text/tests/fixtures.rs"]
mod fixtures;
use fixtures::*;

fn raw(host: &FileOversight, id: u64, prompt: &[u32], new: usize) -> FileGenerationCommand {
    let n = host.decoder_inspection().unwrap().numerical;
    FileGenerationCommand::new(id, n.actor_revision, n.position, GenerationRequest {
        prompt: prompt.to_vec(), max_new_tokens: new, stop_tokens: vec![256],
        budget: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES },
    }).unwrap()
}
fn step(host: &mut FileOversight, id: u64) -> FileGenerationProgress {
    let revision = host.decoder_generation_progress(id).unwrap().generation_revision();
    host.advance_decoder_generation(host.revision(), id, revision).unwrap()
}
fn resume_after_cancel(host: &mut FileOversight, tick: u64) {
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.resume_decoder(host.revision(), n.actor_revision, n.position),
        Err(JournalError::Contract(Error::Incomplete))));
    host.observe_time(host.revision(), ElapsedTick(tick)).unwrap();
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
}

#[test]
fn generation_cancel_preserves_every_unstarted_prefill_and_sample_boundary() {
    for cut in 0..=4 {
        let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
        let command = raw(&host, 7, &[98, 99], 3);
        host.begin_decoder_generation(host.revision(), command.clone()).unwrap();
        for _ in 0..cut { step(&mut host, 7); }
        let previous = host.decoder_generation_progress(7).unwrap();
        assert!(!previous.is_complete());
        let numerical = host.decoder_inspection().unwrap().numerical;
        let state = host.machine.broker.hosted_replay_bytes().unwrap();
        let authority = host.inspect().control;
        let journal = host.revision();
        let cancelled = host.cancel_decoder_generation(journal, 7, cut).unwrap();
        assert_eq!(host.revision(), journal + 1);
        assert_eq!(cancelled.generation_revision(), cut + 1);
        assert_eq!(cancelled.finish(), Some(Ok(GenerationFinish::Cancelled)));
        let receipt = cancelled.receipt().unwrap(); let report = receipt.result().unwrap();
        assert_eq!(receipt.command(), &command);
        assert_eq!(report.start_position(), 0);
        assert_eq!(report.end_position(), numerical.position);
        assert_eq!(report.requested_prompt_tokens(), 2);
        assert_eq!(report.reviewed_prompt_tokens(), (cut as usize).min(2));
        assert_eq!(report.tokens(), previous.tokens());
        assert_eq!(report.work(), previous.partial().map_or(GenerationWork::default(), |p| p.work()));
        assert_eq!(report.last_review(), previous.partial().and_then(|p| p.last_review()));
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(host.machine.broker.hosted_replay_bytes().unwrap(), state);
        assert_eq!(host.inspect().control, authority);
        assert!(host.pending_decoder_generation().unwrap().is_none());
        assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn generation_cancel_requires_exact_live_predecessors_and_refuses_missing_ids() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let command = raw(&host, 7, &[98, 99], 2);
    host.begin_decoder_generation(host.revision(), command).unwrap(); step(&mut host, 7);
    let before = bytes(&host); let numerical = host.decoder_inspection().unwrap(); let revision = host.revision();
    for (journal, id, generation) in [(revision - 1, 7, 1), (revision, 7, 0), (revision, 7, 2), (revision, 99, 0)] {
        assert!(host.cancel_decoder_generation(journal, id, generation).is_err());
        assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
        assert!(host.storage_failure().is_none());
    }
    assert!(!step(&mut host, 7).is_complete()); // same owner still genuinely advances
    assert_eq!(host.cancel_decoder_generation(host.revision(), 7, 2).unwrap().finish(),
        Some(Ok(GenerationFinish::Cancelled)));
}

#[test]
fn generation_cancel_resumes_same_cache_without_rerolling_the_cancelled_id() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let old = raw(&host, 7, &[98], 3);
    host.begin_decoder_generation(host.revision(), old.clone()).unwrap(); step(&mut host, 7); step(&mut host, 7);
    host.cancel_decoder_generation(host.revision(), 7, 2).unwrap();
    let saved = bytes(&host); let numerical = host.decoder_inspection().unwrap().numerical;
    assert_eq!(host.generate_decoder(0, old.clone()).unwrap().result().unwrap().finish(), GenerationFinish::Cancelled);
    assert_eq!(bytes(&host), saved);
    let mut changed = old.request().clone(); changed.budget.sampling_entries -= 1;
    let changed = FileGenerationCommand::new(7, old.actor_revision(), old.position(), changed).unwrap();
    assert!(matches!(host.generate_decoder(0, changed), Err(JournalError::Contract(Error::Binding))));
    let next = raw(&host, 8, &[98], 2);
    assert!(host.begin_decoder_generation(host.revision(), next.clone()).is_err());
    resume_after_cancel(&mut host, 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    host.begin_decoder_generation(host.revision(), next).unwrap();
    let active = host.decoder_inspection().unwrap(); let saved = bytes(&host);
    // The lost reply for an older cancellation cannot pause a new healthy job.
    assert_eq!(host.cancel_decoder_generation(0, 7, 2).unwrap().finish(), Some(Ok(GenerationFinish::Cancelled)));
    assert_eq!(host.decoder_inspection().unwrap(), active); assert_eq!(bytes(&host), saved);
    while !host.decoder_generation_progress(8).unwrap().is_complete() { step(&mut host, 8); }
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, numerical.sampled_draws + 2);
    assert_eq!(host.decoder_generation(7).unwrap().result().unwrap().tokens(), &[65]);
}

#[test]
fn generation_cancel_never_relabels_a_hold_stop_or_native_refusal() {
    for mode in 0..3 {
        let root = Directory::new(); let mut host = owner(&root, &config(if mode == 0 { 1.5 } else { 3.0 }, 65));
        let initial = raw(&host, 7, &[98], 3); let mut r = initial.request().clone();
        if mode == 1 { r.stop_tokens.push(65); }
        if mode == 2 { r.budget.scalar_products = 0; }
        let command = FileGenerationCommand::new(7, initial.actor_revision(), initial.position(), r).unwrap();
        host.generate_decoder(host.revision(), command).unwrap();
        let expected = match mode { 0 => Ok(GenerationFinish::Held), 1 => Ok(GenerationFinish::StopToken), _ => Err(Error::Limit) };
        let saved = bytes(&host); let numerical = host.decoder_inspection().unwrap();
        assert_eq!(host.cancel_decoder_generation(0, 7, 0).unwrap().finish(), Some(expected));
        assert_eq!(bytes(&host), saved); assert_eq!(host.decoder_inspection().unwrap(), numerical);
        assert!(matches!(host.cancel_decoder_generation(0, 7, 1), Err(JournalError::Contract(Error::Stale))));
    }
}

#[test]
fn generation_cancel_unstarted_unadmitted_input_is_not_a_completed_prompt() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let initial = raw(&host, 7, &[u32::MAX], 1); let mut r = initial.request().clone();
    r.budget.scalar_products = 0;
    let command = FileGenerationCommand::new(7, initial.actor_revision(), 0, r).unwrap();
    host.begin_decoder_generation(host.revision(), command).unwrap();
    let result = host.cancel_decoder_generation(host.revision(), 7, 0).unwrap();
    let report = result.receipt().unwrap().result().unwrap();
    assert_eq!(report.finish(), GenerationFinish::Cancelled);
    assert_eq!(report.requested_prompt_tokens(), 1); assert_eq!(report.reviewed_prompt_tokens(), 0);
    assert_eq!(report.work(), GenerationWork::default());
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
    resume_after_cancel(&mut host, 2);
    let valid = raw(&host, 8, &[98], 1);
    assert_eq!(host.generate_decoder(host.revision(), valid).unwrap().result().unwrap().tokens(), &[65]);
}

#[test]
fn generation_cancel_retains_full_requested_history_reservations() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let count = MAX_FILE_GENERATION_STEPS / MAX_GENERATION_TOKENS;
    for id in 1..=count as u64 {
        let request = raw(&host, id, &[98], MAX_GENERATION_TOKENS - 1);
        host.begin_decoder_generation(host.revision(), request).unwrap();
        host.cancel_decoder_generation(host.revision(), id, 0).unwrap();
        resume_after_cancel(&mut host, id + 1);
    }
    // Intent capacity is conservative, even if numerical context admission never
    // ran. Cancellation must not recycle these retained requested-step promises.
    let extra = raw(&host, count as u64 + 1, &[98], 0); let saved = bytes(&host);
    assert!(matches!(host.begin_decoder_generation(host.revision(), extra), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(bytes(&host), saved); assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
}

#[test]
fn generation_cancel_uses_one_ordinary_event_and_no_hidden_time_prefix() {
    for spare in [0, 1] {
        let root = Directory::new(); let mut p = host_profile(); p.delivery.limits.events = 5 + spare;
        let mut host = owner_with_profile(&root, &config(3.0, 65), p);
        assert_eq!(host.revision(), 3);
        let command = raw(&host, 7, &[98], 3);
        host.begin_decoder_generation(host.revision(), command).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let saved = bytes(&host); let numerical = host.decoder_inspection().unwrap();
        let result = host.cancel_decoder_generation(host.revision(), 7, 0);
        if spare == 0 {
            assert!(matches!(result, Err(JournalError::Contract(Error::Limit))));
            assert_eq!(bytes(&host), saved); assert_eq!(host.decoder_inspection().unwrap(), numerical);
            assert!(host.clock_ready()); assert!(host.storage_failure().is_none());
        } else {
            assert_eq!(result.unwrap().finish(), Some(Ok(GenerationFinish::Cancelled)));
            assert_eq!(host.revision(), 6); assert!(!host.clock_ready());
        }
    }
}

#[test]
fn generation_cancel_replays_as_a_transition_not_a_caller_asserted_report() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let command = raw(&host, 7, &[98], 3);
    host.begin_decoder_generation(host.revision(), command).unwrap(); step(&mut host, 7);
    let before = host.events.clone();
    host.cancel_decoder_generation(host.revision(), 7, 1).unwrap();
    assert!(matches!(host.events.last(), Some(Event::Decoder(DecoderEvent::CancelGeneration { id: 7, revision: 1 }))));
    let mut duplicate = host.events.clone(); duplicate.push(host.events.last().unwrap().clone());
    assert!(super::super::super::Machine::replay(&host.profile, &duplicate).is_err());
    for (id, revision) in [(8, 1), (7, 0), (7, 2)] {
        let mut bad = before.clone(); bad.push(Event::Decoder(DecoderEvent::CancelGeneration { id, revision }));
        assert!(super::super::super::Machine::replay(&host.profile, &bad).is_err());
    }
    let restored = super::super::super::Machine::replay(&host.profile, &host.events).unwrap();
    assert_eq!(restored.decoder_generation_progress(7).unwrap().finish(), Some(Ok(GenerationFinish::Cancelled)));
    assert_eq!(restored.broker.hosted_replay_bytes().unwrap(), host.machine.broker.hosted_replay_bytes().unwrap());
}

mod text;
