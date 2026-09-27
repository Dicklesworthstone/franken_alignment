//! Native text must come from real accepted learned generation, not supplied IDs.
#[path = "support/learned_text_model.rs"]
mod fixture;
use fixture::*;
use fa_reference::Error;
use fa_reference::action::MAX_PAYLOAD_BYTES;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationSpec, GenerationStatus, GenerationStop,
};
use fa_reference::action::consequence::oversight::learned_source::{
    LearnedEvidenceLimits, LearnedAvailability,
    text::LearnedTextCompletion,
};

#[test]
fn original_bpe_and_original_generator_produce_exact_text_with_stop_evidence() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let tokenizer = tokenizer(&model); let config = config(&model);
    let encoded = tokenizer.encode(config.prompt.as_bytes(), config.tokenization).unwrap();
    assert_eq!(encoded.tokens(), &[MERGED_PROMPT]);
    let spec = GenerationSpec::new(encoded.tokens().to_vec(), config.max_new_tokens,
        config.stop_tokens.clone(), config.sampling.clone()).unwrap();
    let mut control = model.monitored_generation_with_telemetry(config.stream, config.evaluation_origin,
        spec, config.policy.clone(), config.budget, config.telemetry).unwrap();
    let mut source = model.observed_learned_text_generation(tokenizer, config).unwrap();
    assert!(source.text_bound()); assert_eq!(source.text_prompt(), Some("PP"));
    assert_eq!(source.text_tokenization_work(), Some(encoded.work()));
    let observation = source.observation();
    while control.status().is_active() {
        assert_eq!(source.text_message(LearnedEvidenceLimits::default()).err(), Some(Error::Incomplete));
        let left = control.advance(control.position()).unwrap();
        let right = source.advance(source.position()).unwrap();
        assert_eq!(source.accepted_tokens(), control.accepted_tokens());
        assert_eq!(source.work(), control.work());
        assert_eq!(source.telemetry_work(), control.telemetry_work());
        assert_eq!(left.accepted().unwrap().logits, right.accepted().unwrap().logits);
        match (left.sample(), right.sample()) {
            (Some(a), Some(b)) => {
                assert_eq!(a.token, b.token); assert_eq!(a.random_word, b.random_word);
                assert_eq!(a.draw, b.draw); assert_eq!(a.probability.to_bits(), b.probability.to_bits());
            }
            (None, None) => {}, _ => panic!("sampling phases differ"),
        }
    }
    let message = source.text_message(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(message.bytes(), b"OK"); assert_eq!(message.output_tokens(), 2);
    assert_eq!(message.stop(), GenerationStop::StopToken(END));
    assert_eq!(message.evidence().tokens(), &[MERGED_PROMPT, b'O' as u32, b'K' as u32, END]);
    assert_eq!(message.work(), source.work());
    assert_eq!(message.telemetry_work(), source.telemetry_work());
    observation.validate(message.evidence()).unwrap();
    drop(source);
    assert_eq!(observation.availability(), LearnedAvailability::Closed);
    assert_eq!(observation.validate(message.evidence()), Err(Error::Incomplete));
    assert_eq!(message.bytes(), b"OK"); // Historical bytes are not currentness.
}

#[test]
fn token_limit_requires_explicit_acceptance_and_never_claims_an_end_marker() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    for permitted in [false, true] {
        let mut config = config(&model); config.max_new_tokens = 2;
        config.output.completion = if permitted { LearnedTextCompletion::StopOrTokenLimit }
            else { LearnedTextCompletion::StopRequired };
        let mut source = model.observed_learned_text_generation(tokenizer(&model), config).unwrap();
        assert_eq!(source.run_to_stop().unwrap(), GenerationStatus::Finished(GenerationStop::TokenLimit));
        let result = source.text_message(LearnedEvidenceLimits::default());
        if permitted {
            let message = result.unwrap(); assert_eq!(message.bytes(), b"OK");
            assert_eq!(message.stop(), GenerationStop::TokenLimit);
        } else { assert_eq!(result.err(), Some(Error::Incomplete)); }
    }
}

#[test]
fn byte_boundaries_control_ids_and_empty_output_are_not_silently_rewritten() {
    for (chain, expected) in [
        (vec![0xc3, 0xa9, END], Ok(vec![0xc3, 0xa9])),
        (vec![0xc3, END], Err(Error::InvalidInput)),
        (vec![OTHER_CONTROL, END], Err(Error::Binding)),
        (vec![END], Err(Error::Incomplete)),
    ] {
        let model = model(&chain); let mut config = config(&model); config.max_new_tokens = chain.len();
        let mut source = model.observed_learned_text_generation(tokenizer(&model), config).unwrap();
        source.run_to_stop().unwrap();
        assert_eq!(source.text_message(LearnedEvidenceLimits::default()).map(|m| m.bytes().to_vec()), expected);
    }
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    for cap in [1, 2] {
        let mut config = config(&model); config.output.max_bytes = cap;
        let mut source = model.observed_learned_text_generation(tokenizer(&model), config).unwrap();
        source.run_to_stop().unwrap();
        let result = source.text_message(LearnedEvidenceLimits::default());
        if cap == 2 { assert_eq!(result.unwrap().bytes(), b"OK"); }
        else { assert_eq!(result.err(), Some(Error::Limit)); }
        assert_eq!(source.accepted_tokens().len(), 4);
    }
}

#[test]
fn held_or_failed_numerical_suffix_cannot_publish_its_quiet_prefix_as_text() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    for alarm in [true, false] {
        let mut config = config(&model);
        if alarm { config.policy = policy(&model, true); }
        else { config.telemetry.source_check_values = model.cache_profile().values_per_token() as u64; }
        let mut source = model.observed_learned_text_generation(tokenizer(&model), config).unwrap();
        source.advance(0).unwrap();
        let result = source.advance(1);
        if alarm { assert!(result.unwrap().accepted().is_none()); }
        else { assert_eq!(result.err(), Some(Error::Limit)); }
        assert_eq!(source.text_message(LearnedEvidenceLimits::default()).err(), Some(Error::Incomplete));
        assert_eq!(source.accepted_tokens(), &[MERGED_PROMPT]);
        assert_eq!(source.work().sampling_attempts, 1);
        assert_eq!(source.advance(1).err(), Some(Error::WrongState));
    }
}

#[test]
fn invalid_text_recipes_refuse_without_a_partial_generator_and_controls_succeed() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    for case in 0..6 {
        let mut config = config(&model);
        match case {
            0 => config.output.max_bytes = 0,
            1 => config.output.max_bytes = MAX_PAYLOAD_BYTES + 1,
            2 => { config.stop_tokens.clear(); },
            3 => { config.stop_tokens = [b'O' as u32].into(); },
            4 => config.tokenization.input_bytes = 1,
            5 => config.tokenization.pair_lookups = 0,
            _ => unreachable!(),
        }
        assert!(model.observed_learned_text_generation(tokenizer(&model), config).is_err());
        assert!(model.observed_learned_text_generation(tokenizer(&model), fixture::config(&model)).is_ok());
    }
    use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
    let original = tokenizer(&model);
    // The whole native profile is pinned, not just equal vocabulary size.
    let mut identity = model.profile().identity(); identity.model_generation += 1;
    let foreign = fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderProfile::new(
        identity, model.profile().shape(), 0.00001, 10000.0).unwrap();
    use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{TokenBytes, Merge};
    let mut vocabulary: Vec<_> = (0..=255).map(|b| TokenBytes::Content(vec![b])).collect();
    vocabulary.extend([TokenBytes::Control, TokenBytes::Control, TokenBytes::Content(b"PP".to_vec())]);
    let foreign = ByteBpe::new(foreign, vocabulary,
        vec![Merge { left: PROMPT, right: PROMPT, result: MERGED_PROMPT }]).unwrap();
    assert_eq!(model.observed_learned_text_generation(foreign, config(&model)).err(), Some(Error::Binding));
    assert!(model.observed_learned_text_generation(original, config(&model)).is_ok());
}

#[test]
fn text_capture_keeps_original_retention_bounds_and_owner_validation() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let mut first = model.observed_learned_text_generation(tokenizer(&model), config(&model)).unwrap();
    let mut second = model.observed_learned_text_generation(tokenizer(&model), config(&model)).unwrap();
    first.run_to_stop().unwrap(); second.run_to_stop().unwrap();
    let message = first.text_message(LearnedEvidenceLimits::default()).unwrap();
    let cost = message.evidence().cost();
    let exact = LearnedEvidenceLimits { token_ids: cost.token_ids, score_words: cost.score_words,
        encoded_bytes: cost.encoded_bytes };
    assert_eq!(first.text_message(exact).unwrap().bytes(), b"OK");
    assert_eq!(first.text_message(LearnedEvidenceLimits { token_ids: cost.token_ids - 1, ..exact }).err(), Some(Error::Limit));
    assert_eq!(second.observation().validate(message.evidence()), Err(Error::Binding));
    assert_eq!(second.text_message(exact).unwrap().bytes(), message.bytes());
    let before = first.work();
    assert_eq!(first.advance(0).err(), Some(Error::WrongState));
    assert_eq!(first.work(), before);
}
