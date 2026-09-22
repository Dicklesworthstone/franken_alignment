//! Real journal/decoder integration; source authoring is not execution evidence.
use super::*;
mod fixtures;
use fixtures::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{GenerationFinish, GenerationRequest};
use crate::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::oversight::decoder_host::HostedStopPolicy;

#[test]
fn durable_text_matches_original_ids_actual_cache_work_and_exact_bytes() {
    let root = Directory::new(); let config = config(3.0, 65); let mut host = owner(&root, &config);
    let c = command(&host, 7, request(b"ab\xff", 2));
    let before = host.inspect();
    let mut native = config.build().unwrap();
    let expected = native.generate(0, GenerationRequest { prompt: vec![256, 257, 255],
        max_new_tokens: 2, stop_tokens: vec![256], budget: c.request().generation }).unwrap();
    let receipt = host.generate_decoder_text(host.revision(), c.clone()).unwrap();
    let result = receipt.result().unwrap();
    assert_eq!(receipt.command(), &c);
    assert_eq!(result.prompt().source(), b"ab\xff");
    assert_eq!(result.prompt().tokens(), &[257, 255]);
    assert_eq!(result.prompt().spans(), &[0..2, 2..3]);
    assert_eq!(result.prefix_controls(), &[256]);
    assert_eq!(result.generation().tokens(), expected.tokens());
    assert_eq!(result.generation().work(), expected.work());
    assert_eq!(result.bytes().unwrap(), b"AA");
    assert_eq!(result.utf8().unwrap(), "AA");
    assert_eq!(host.revision(), before.revision + 2);
    assert!(matches!(&host.events[before.revision as usize], Event::Decoder(DecoderEvent::TextIntent(_))));
    assert_eq!(host.machine.broker.hosted_replay_bytes().unwrap(), native.replay_bytes().unwrap());
    assert_eq!(host.inspect().control, before.control);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().payload, b"initial");
}

#[test]
fn every_text_retry_field_is_bound_even_when_original_ids_would_match() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let c = command(&host, 7, request(b"ab", 2));
    host.generate_decoder_text(host.revision(), c.clone()).unwrap();
    let before = host.decoder_inspection().unwrap(); let disk = bytes(&host);
    assert_eq!(host.generate_decoder_text(0, c.clone()).unwrap().result().unwrap().bytes().unwrap(), b"AA");
    for case in 0..9 {
        let mut changed = c.clone();
        match case {
            0 => changed.request.prompt = b"ba".to_vec(),
            1 => changed.request.tokenization.pair_lookups -= 1,
            2 => changed.request.max_output_bytes += 1,
            3 => changed.request.generation.sampling_entries -= 1,
            4 => changed.request.stop_tokens.push(65),
            5 => changed.request.prefix_controls.clear(),
            6 => changed.actor_revision += 1,
            7 => changed.position += 1,
            8 => changed.request.max_new_tokens -= 1,
            _ => unreachable!(),
        }
        assert!(matches!(host.generate_decoder_text(0, changed), Err(JournalError::Contract(Error::Binding))));
    }
    assert_eq!(host.decoder_inspection().unwrap(), before); assert_eq!(bytes(&host), disk);
    // A completed bare-ID generation cannot be relabeled with text after the fact.
    let n = host.decoder_inspection().unwrap().numerical;
    let plain = FileGenerationCommand::new(8, n.actor_revision, n.position, GenerationRequest {
        prompt: Vec::new(), max_new_tokens: 1, stop_tokens: vec![256], budget: c.request.generation,
    }).unwrap();
    host.generate_decoder(host.revision(), plain).unwrap();
    let text = FileTextGenerationCommand::new(8, n.actor_revision, n.position,
        request(b"", 1)).unwrap();
    assert!(host.generate_decoder_text(host.revision(), text).is_err());
    assert!(matches!(host.decoder_text_generation(8), Err(JournalError::Contract(Error::Missing))));
}

#[test]
fn text_intent_survives_restart_without_reconstructing_old_eligibility() {
    let root = Directory::new(); let config = config(3.0, 65); let mut host = owner(&root, &config);
    let c = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), c.clone()).unwrap();
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
    assert!(matches!(host.decoder_text_generation(7), Err(JournalError::Contract(Error::Incomplete))));
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 97,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }), Err(JournalError::Contract(Error::Incomplete))));
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &config, &tokenizer(false)).unwrap();
    let before = host.decoder_inspection().unwrap(); let disk = bytes(&host);
    assert!(!host.clock_ready()); assert!(before.paused);
    assert!(host.generate_decoder_text(host.revision(), c.clone()).is_err());
    assert_eq!(bytes(&host), disk);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_decoder(host.revision(), before.numerical.actor_revision, before.numerical.position).unwrap();
    let receipt = host.generate_decoder_text(host.revision(), c.clone()).unwrap();
    assert_eq!(receipt.result().unwrap().bytes().unwrap(), b"AA");
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &config, &tokenizer(false)).unwrap();
    let before = host.decoder_inspection().unwrap(); let disk = bytes(&host);
    assert_eq!(host.generate_decoder_text(0, c).unwrap().result().unwrap().bytes().unwrap(), b"AA");
    assert_eq!(host.decoder_inspection().unwrap(), before); assert_eq!(bytes(&host), disk);
}

#[test]
fn changed_tokenizer_refuses_before_replay_or_recovery_write_and_cannot_replace_live_binding() {
    let root = Directory::new(); let config = config(3.0, 65); let mut host = owner(&root, &config);
    let before = host.inspect(); let disk = bytes(&host);
    assert!(matches!(host.enable_decoder_tokenizer(host.revision(), tokenizer(true)), Err(JournalError::Contract(Error::Duplicate))));
    assert_eq!(host.inspect(), before); assert_eq!(bytes(&host), disk);
    let c = command(&host, 7, request(b"ab", 1));
    host.generate_decoder_text(host.revision(), c).unwrap();
    let disk = bytes(&host); drop(host);
    assert!(matches!(FileOversight::open_with_text_decoder(root.store(), host_profile(), &config, &tokenizer(true)),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), disk);
    let (host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &config, &tokenizer(false)).unwrap();
    assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"A");
}

#[test]
fn original_monitor_hold_consumes_draw_and_stops_without_leaking_text_or_rerolling() {
    let root = Directory::new(); let config = config(1.5, 65); let mut host = owner(&root, &config);
    host.enable_decoder_stop(host.revision(), HostedStopPolicy::new(7, 1, 900).unwrap()).unwrap();
    let c = command(&host, 7, request(b"ab", 2));
    let receipt = host.generate_decoder_text(host.revision(), c.clone()).unwrap();
    let report = receipt.result().unwrap();
    assert_eq!(report.generation().finish(), GenerationFinish::Held);
    assert!(report.bytes().unwrap().is_empty()); assert!(report.generation().tokens().is_empty());
    let n = host.decoder_inspection().unwrap();
    assert_eq!(n.numerical.sampled_draws, 1); assert_eq!(n.numerical.status, MonitoringStatus::Held);
    assert!(host.inspect().control.suspended); assert_eq!(host.inspect().executions, 0);
    let before = host.inspect();
    assert_eq!(host.generate_decoder_text(0, c.clone()).unwrap().result().unwrap().generation().finish(), GenerationFinish::Held);
    assert_eq!(host.inspect(), before);
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &config, &tokenizer(false)).unwrap();
    assert_eq!(host.generate_decoder_text(0, c).unwrap().result().unwrap().generation().finish(), GenerationFinish::Held);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
}

#[test]
fn text_output_admission_precedes_intent_and_native_budget_refusal_stays_historical() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let before = host.decoder_inspection().unwrap(); let disk = bytes(&host);
    let mut too_small = request(b"ab", 2); too_small.max_output_bytes = 3;
    let c = command(&host, 7, too_small);
    assert!(matches!(host.generate_decoder_text(host.revision(), c), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(host.decoder_inspection().unwrap(), before); assert_eq!(bytes(&host), disk);
    let mut low_budget = request(b"ab", 2); low_budget.generation.scalar_products = 0;
    let c = command(&host, 7, low_budget);
    let receipt = host.generate_decoder_text(host.revision(), c.clone()).unwrap();
    assert_eq!(receipt.result().unwrap_err(), Error::Limit);
    assert_eq!(receipt.numerical().result().unwrap_err(), Error::Limit);
    assert_eq!(host.decoder_inspection().unwrap().numerical, before.numerical);
    let revision = host.revision();
    assert_eq!(host.generate_decoder_text(0, c.clone()).unwrap().result().unwrap_err(), Error::Limit);
    assert_eq!(host.revision(), revision);
    let mut repaired = c; repaired.request.generation.scalar_products = MAX_DECODER_PRODUCTS;
    assert!(matches!(host.generate_decoder_text(host.revision(), repaired), Err(JournalError::Contract(Error::Binding))));
    let valid = command(&host, 8, request(b"ab", 2));
    assert_eq!(host.generate_decoder_text(host.revision(), valid).unwrap().result().unwrap().bytes().unwrap(), b"AA");
}

#[test]
fn stop_controls_and_invalid_utf8_output_are_not_silently_stripped_or_replaced() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 256));
    let c = command(&host, 7, request(b"ab", 3));
    let receipt = host.generate_decoder_text(host.revision(), c).unwrap();
    assert_eq!(receipt.result().unwrap().generation().finish(), GenerationFinish::StopToken);
    assert!(receipt.result().unwrap().bytes().unwrap().is_empty());
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 255));
    let c = command(&host, 7, request(b"ab", 1));
    let receipt = host.generate_decoder_text(host.revision(), c).unwrap();
    assert_eq!(receipt.result().unwrap().bytes().unwrap(), &[255]);
    assert!(receipt.result().unwrap().utf8().is_err());
}

#[test]
fn altered_text_intent_cannot_match_an_existing_original_numerical_witness() {
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let c = command(&host, 7, request(b"ab", 2));
    let intent = host.events.len();
    host.generate_decoder_text(host.revision(), c.clone()).unwrap();
    assert!(Machine::replay(&host_profile(), &host.events).is_ok());
    let mut changed = c; changed.request.prompt = b"ba".to_vec();
    let mut events = host.events.clone();
    events[intent] = Event::Decoder(DecoderEvent::TextIntent(Rc::new(changed)));
    let encoded = journal::encode(&host_profile(), host.store.identity(), &events).unwrap();
    let decoded = journal::decode(&host_profile(), host.store.identity(), &encoded).unwrap();
    assert!(matches!(Machine::replay(&host_profile(), &decoded), Err(Error::Binding)));
    let mut events = host.events.clone();
    events.insert(intent, Event::Decoder(DecoderEvent::Tokenizer(tokenizer(false).to_bytes().unwrap().into())));
    assert!(Machine::replay(&host_profile(), &events).is_err());
}

#[test]
fn two_event_capacity_is_checked_before_text_intent_and_failed_sources_remain_closed() {
    for spare in [1, 2] {
        let root = Directory::new(); let config = config(3.0, 65);
        let mut p = host_profile(); p.delivery.limits.events = 3 + spare;
        let mut host = owner_with_profile(&root, &config, p);
        let before = bytes(&host); let c = command(&host, 7, request(b"ab", 1));
        if spare == 1 {
            assert!(matches!(host.generate_decoder_text(host.revision(), c), Err(JournalError::Contract(Error::Limit))));
            assert_eq!(bytes(&host), before); assert_eq!(host.revision(), 3);
        } else { assert!(host.generate_decoder_text(host.revision(), c).is_ok()); }
    }
    let root = Directory::new(); let mut host = owner(&root, &config(3.0, 65));
    let c = command(&host, 7, request(b"ab", 1)); let disk = bytes(&host);
    host.source_interrupted = true;
    assert!(matches!(host.generate_decoder_text(host.revision(), c), Err(JournalError::Contract(Error::Incomplete))));
    assert!(host.source_interrupted); assert_eq!(bytes(&host), disk);
}

#[test]
fn every_intent_and_result_barrier_withholds_output_and_recovers_only_canonical_progress() {
    for result_cut in [false, true] {
        for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let config = config(3.0, 65); let mut host = owner(&root, &config);
            let c = command(&host, 7, request(b"ab", 2));
            if result_cut { host.begin_decoder_text(host.revision(), c.clone()).unwrap(); }
            let before = host.inspect(); host.store.fail_once(barrier);
            assert!(matches!(host.generate_decoder_text(host.revision(), c.clone()), Err(JournalError::Io(_))));
            assert!(matches!(host.decoder_text_generation(7), Err(JournalError::Unavailable)));
            assert!(matches!(host.generate_decoder_text(0, c.clone()), Err(JournalError::Unavailable)));
            assert_eq!(host.inspect(), before);
            let disk = FileOversight::read_publication(root.store(), &host_profile()).unwrap();
            assert_eq!(disk.revision, before.revision + u64::from(barrier == JournalIo::DirectorySync));
            assert_eq!(disk.executions, 0);
            drop(host);
            let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &config, &tokenizer(false)).unwrap();
            let n = host.decoder_inspection().unwrap();
            let has_result = result_cut && barrier == JournalIo::DirectorySync;
            assert_eq!(n.numerical.sampled_draws, if has_result { 2 } else { 0 });
            if !has_result {
                host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
                host.resume_decoder(host.revision(), n.numerical.actor_revision, n.numerical.position).unwrap();
            }
            let receipt = host.generate_decoder_text(host.revision(), c).unwrap();
            assert_eq!(receipt.result().unwrap().bytes().unwrap(), b"AA");
            assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
            assert_eq!(host.inspect().executions, 0);
        }
    }
}

#[test]
fn new_input_tags_roundtrip_exactly_and_reject_truncation_and_oversize_declarations() {
    use super::super::super::super::codec::shared::{Reader, Writer};
    let c = FileTextGenerationCommand::new(7, 1, 0, request(b"ab\xff", 2)).unwrap();
    for (tag, event) in [(9, DecoderEvent::Tokenizer(tokenizer(false).to_bytes().unwrap().into())),
        (10, DecoderEvent::TextIntent(Rc::new(c)))] {
        let mut w = Writer::new(10000); super::super::write(&mut w, &event).unwrap(); let bytes = w.finish();
        assert_eq!(bytes[0], tag);
        let mut r = Reader::new(&bytes); let decoded = super::super::read(&mut r).unwrap(); r.end().unwrap();
        let mut w = Writer::new(10000); super::super::write(&mut w, &decoded).unwrap(); assert_eq!(w.finish(), bytes);
        for end in 0..bytes.len() { assert!(super::super::read(&mut Reader::new(&bytes[..end])).is_err()); }
    }
    let mut r = request(b"ab", 1); r.prompt = vec![0; MAX_INPUT_BYTES + 1];
    assert_eq!(FileTextGenerationCommand::new(1, 1, 0, r).unwrap_err(), Error::Limit);
    let empty = DecoderEvent::Tokenizer(Rc::from(&b""[..]));
    assert_eq!(super::super::write(&mut Writer::new(100), &empty), Err(Error::Incomplete));
    let mut r = request(b"ab", 1); r.tokenization.heap_pops = MAX_HEAP_POPS + 1;
    assert_eq!(FileTextGenerationCommand::new(1, 1, 0, r).unwrap_err(), Error::Limit);
}
