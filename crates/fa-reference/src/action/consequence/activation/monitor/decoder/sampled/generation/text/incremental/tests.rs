use super::*;
use super::super::tests::{numerical, profile, request, run, tokenizer};
use crate::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::monitor::{RefinementBudget, RefinementMonitor};
use crate::action::consequence::activation::probe::LinearProbe;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderLayerWeights, DecoderModel};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use std::collections::BTreeMap;

fn prefill(session: &mut TextGenerationSession) {
    for position in 0..2 {
        let chunk = session.advance(position).unwrap();
        assert!(chunk.bytes().is_empty());
        assert!(chunk.tokens().is_empty());
        assert_eq!(chunk.byte_range(), 0..0);
        assert!(chunk.finish().is_none());
    }
}

#[test]
fn every_pull_matches_one_shot_without_duplicate_bytes_or_tokens() {
    let mut reference = run(259, None);
    let expected = reference.generate(0, request(b"ab", 3)).unwrap();
    let mut session = run(259, None).into_generation(0, request(b"ab", 3)).unwrap();
    assert!(session.progress().finish().is_none());
    assert_eq!(session.decoder_work().tokens, 0);
    assert_eq!(session.prompt().tokens(), &[258]);
    prefill(&mut session);
    let mut bytes = Vec::new();
    let mut tokens = Vec::new();
    for position in 2..5 {
        let chunk = session.advance(position).unwrap();
        assert_eq!(chunk.byte_range(), bytes.len()..bytes.len() + 2);
        assert_eq!(chunk.tokens(), &[259]);
        assert_eq!(chunk.bytes(), "é".as_bytes());
        assert_eq!(chunk.position(), position + 1);
        assert_eq!(chunk.finish(), (position == 4).then_some(GenerationFinish::TokenLimit));
        bytes.extend_from_slice(chunk.bytes());
        tokens.extend_from_slice(chunk.tokens());
    }
    assert_eq!(bytes, expected.bytes().unwrap());
    assert_eq!(tokens, expected.generation().tokens());
    assert_eq!(session.decoder_work(), reference.decoder_work());
    assert_eq!(session.monitoring_work(), reference.monitoring_work());
    assert_eq!(session.sampled_draws(), reference.sampled_draws());
    assert_eq!(session.progress().work(), expected.generation().work());
    assert_eq!(session.utf8().unwrap(), "ééé");
    let (_, actual) = session.into_parts().unwrap();
    assert_eq!(actual.prompt().source(), expected.prompt().source());
    assert_eq!(actual.generation().tokens(), expected.generation().tokens());
    assert_eq!(actual.bytes(), expected.bytes());
}

#[test]
fn stale_pulls_and_repeated_terminal_polls_do_not_spend_or_reemit() {
    let mut session = run(65, None).into_generation(0, request(b"ab", 1)).unwrap();
    assert_eq!(session.advance(1).unwrap_err(), Error::Stale);
    assert!(!session.interrupted());
    assert_eq!(session.decoder_work().tokens, 0);
    prefill(&mut session);
    assert_eq!(session.advance(2).unwrap().bytes(), b"A");
    let work = session.decoder_work();
    let monitoring = session.monitoring_work();
    for _ in 0..3 {
        let chunk = session.advance(3).unwrap();
        assert_eq!(chunk.byte_range(), 1..1);
        assert!(chunk.tokens().is_empty());
        assert!(chunk.bytes().is_empty());
        assert_eq!(chunk.finish(), Some(GenerationFinish::TokenLimit));
    }
    assert_eq!(session.advance(2).unwrap_err(), Error::Stale);
    assert_eq!(session.decoder_work(), work);
    assert_eq!(session.monitoring_work(), monitoring);
    assert_eq!(session.sampled_draws(), 1);
    assert_eq!(session.bytes().unwrap(), b"A");
}

#[test]
fn cancellation_destroys_the_live_source_without_computing_or_refunding_a_draw() {
    let owner = run(65, None);
    let source = owner.observation();
    let mut session = owner.into_generation(0, request(b"ab", 4)).unwrap();
    prefill(&mut session);
    session.advance(2).unwrap();
    let evidence = source.capture().unwrap();
    let work = session.decoder_work();
    let monitoring = session.monitoring_work();
    let cancelled = session.cancel();
    assert_eq!(source.availability(), DecoderAvailability::Closed);
    assert_eq!(source.validate(&evidence), Err(Error::Incomplete));
    assert_eq!(cancelled.prompt().source(), b"ab");
    assert_eq!(cancelled.prefix_controls(), &[256]);
    assert_eq!(cancelled.bytes(), b"A");
    assert!(cancelled.progress().finish().is_none());
    assert!(cancelled.progress().report().is_none());
    assert_eq!(cancelled.decoder_work(), work);
    assert_eq!(cancelled.monitoring_work(), monitoring);
    assert_eq!(cancelled.sampled_draws(), 1);
    assert!(!cancelled.interrupted());
    assert!(cancelled.output_failure().is_none());
}

#[test]
fn cancelling_before_work_and_during_prefill_never_claims_a_complete_prompt() {
    for pulls in 0..2 {
        let owner = run(65, None);
        let source = owner.observation();
        let mut session = owner.into_generation(0, request(b"abc", 3)).unwrap();
        for position in 0..pulls { session.advance(position).unwrap(); }
        let cancelled = session.cancel();
        assert_eq!(source.availability(), DecoderAvailability::Closed);
        assert_eq!(cancelled.prompt().source(), b"abc");
        assert_eq!(cancelled.prompt().tokens(), &[258, 99]);
        assert_eq!(cancelled.progress().requested_prompt_tokens(), 3);
        assert_eq!(cancelled.progress().reviewed_prompt_tokens(), pulls as usize);
        assert!(cancelled.progress().finish().is_none());
        assert!(cancelled.bytes().is_empty());
        assert_eq!(cancelled.decoder_work().tokens, pulls);
        assert_eq!(cancelled.sampled_draws(), 0);
    }
}

#[test]
fn pending_session_cannot_export_an_unfinished_owner() {
    let owner = run(65, None);
    let source = owner.observation();
    let mut session = owner.into_generation(0, request(b"ab", 2)).unwrap();
    prefill(&mut session);
    assert_eq!(session.into_parts().unwrap_err(), Error::Incomplete);
    assert_eq!(source.availability(), DecoderAvailability::Closed);
    let mut complete = run(65, None).into_generation(0, request(b"ab", 1)).unwrap();
    prefill(&mut complete);
    complete.advance(2).unwrap();
    assert!(complete.into_parts().is_ok());
}

#[test]
fn held_token_produces_no_chunk_and_the_exported_owner_stays_held() {
    let mut session = run(33, Some(33)).into_generation(0, request(b"ab", 3)).unwrap();
    prefill(&mut session);
    let chunk = session.advance(2).unwrap();
    assert!(chunk.tokens().is_empty());
    assert!(chunk.bytes().is_empty());
    assert_eq!(chunk.finish(), Some(GenerationFinish::Held));
    assert_eq!(session.sampled_draws(), 1);
    assert_eq!(session.decoder_work().tokens, 3);
    let (mut owner, report) = session.into_parts().unwrap();
    assert_eq!(owner.status(), MonitoringStatus::Held);
    assert_eq!(report.generation().finish(), GenerationFinish::Held);
    assert_eq!(owner.generate(3, request(b"ab", 1)).unwrap_err().error, Error::WrongState);
    assert_eq!(owner.sampled_draws(), 1);
}

#[test]
fn a_later_hold_preserves_the_quiet_prefix_and_cancellation_keeps_that_finish() {
    let owner = TextDecoder::new(numerical(66, Some(33), true), tokenizer()).unwrap();
    let mut session = owner.into_generation(0, request(b"ab", 3)).unwrap();
    prefill(&mut session);
    assert_eq!(session.advance(2).unwrap().bytes(), b"B");
    let held = session.advance(3).unwrap();
    assert_eq!(held.byte_range(), 1..1);
    assert!(held.tokens().is_empty());
    assert_eq!(held.finish(), Some(GenerationFinish::Held));
    let cancelled = session.cancel();
    assert_eq!(cancelled.bytes(), b"B");
    assert_eq!(cancelled.progress().finish(), Some(GenerationFinish::Held));
    assert_eq!(cancelled.sampled_draws(), 2);
    assert_eq!(cancelled.decoder_work().tokens, 4);
}

#[test]
fn stop_control_is_reviewed_once_but_never_emitted() {
    let mut session = run(257, None).into_generation(0, request(b"ab", 3)).unwrap();
    prefill(&mut session);
    let stopped = session.advance(2).unwrap();
    assert_eq!(stopped.finish(), Some(GenerationFinish::StopToken));
    assert!(stopped.tokens().is_empty());
    assert!(stopped.bytes().is_empty());
    assert_eq!(session.sampled_draws(), 1);
    assert_eq!(session.progress().work().attempted_samples, 1);
    assert!(session.advance(3).unwrap().bytes().is_empty());
    assert_eq!(session.sampled_draws(), 1);
}

// Two genuine consecutive samples split one UTF-8 character across token IDs.
// The same original numerical model/monitor, not fabricated released steps.
fn split_utf8_owner() -> TextDecoder {
    let p = profile();
    let mut embeddings: Vec<f32> = (0..260).flat_map(|_| [1.0, 0.0]).collect();
    embeddings[195 * 2] = 0.0;
    embeddings[195 * 2 + 1] = 1.0;
    let mut output = vec![0.0; 520];
    output[195 * 2] = 1.0;
    output[169 * 2 + 1] = 1.0;
    let model = DecoderModel::new(p, embeddings, vec![DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2],
        gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
    }], vec![1.0; 2], output).unwrap();
    let allowance = RefinementBudget { encoded_bytes: 100_000, probe_coordinates: 100_000 };
    let probe = LinearProbe::new(1, 1, model.residual_contract(1).unwrap().profile(), &[1.0, 0.0], 0.0, 2.0).unwrap();
    let monitors = BTreeMap::from([(1, RefinementMonitor::new(vec![probe], vec![23], allowance).unwrap())]);
    let numerical = MonitoredSampledDecoder::new(model, 7, 11, monitors, allowance, SamplingStart {
        policy: SamplingPolicy::new(1, 1, 260, 1.0, 1, 1.0).unwrap(), stream: 9, seed: 42,
    }).unwrap();
    TextDecoder::new(numerical, tokenizer()).unwrap()
}

#[test]
fn utf8_boundaries_can_cross_chunks_without_lossy_replacement() {
    let mut session = split_utf8_owner().into_generation(0, request(b"ab", 2)).unwrap();
    prefill(&mut session);
    assert_eq!(session.advance(2).unwrap().bytes(), &[195]);
    assert!(matches!(session.utf8(), Err(TextOutputError::InvalidUtf8(_))));
    assert_eq!(session.bytes().unwrap(), &[195]);
    assert_eq!(session.advance(3).unwrap().bytes(), &[169]);
    assert_eq!(session.utf8().unwrap(), "é");
    let (_, report) = session.into_parts().unwrap();
    assert_eq!(report.generation().tokens(), &[195, 169]);
    assert_eq!(report.bytes().unwrap(), "é".as_bytes());
    let mut invalid = run(255, None).into_generation(0, request(b"ab", 1)).unwrap();
    prefill(&mut invalid);
    assert_eq!(invalid.advance(2).unwrap().bytes(), &[255]);
    assert!(matches!(invalid.utf8(), Err(TextOutputError::InvalidUtf8(_))));
    assert_eq!(invalid.into_parts().unwrap().1.bytes().unwrap(), &[255]);
}

#[test]
fn numerical_budget_survives_yielding_and_exhaustion_is_not_text_success() {
    let mut input = request(b"ab", 3);
    input.generation.sampling_entries = 260;
    let mut session = run(65, None).into_generation(0, input).unwrap();
    prefill(&mut session);
    session.advance(2).unwrap();
    let before = session.decoder_work();
    let exhausted = session.advance(3).unwrap();
    assert_eq!(exhausted.finish(), Some(GenerationFinish::BudgetExhausted));
    assert!(exhausted.bytes().is_empty());
    assert_eq!(session.decoder_work(), before);
    assert_eq!(session.sampled_draws(), 1);
    let (_, report) = session.into_parts().unwrap();
    assert_eq!(report.bytes().unwrap(), b"A");
    assert_eq!(report.generation().finish(), GenerationFinish::BudgetExhausted);
}

#[test]
fn completed_export_continues_original_cache_and_never_retokens_previous_text() {
    let mut session = run(65, None).into_generation(0, request(b"ab", 1)).unwrap();
    prefill(&mut session);
    session.advance(2).unwrap();
    let (owner, _) = session.into_parts().unwrap();
    let mut continuation = request(&[], 2);
    continuation.prefix_controls.clear();
    let mut resumed = owner.into_generation(3, continuation).unwrap();
    assert!(resumed.prompt().tokens().is_empty());
    assert_eq!(resumed.sampled_draws(), 1);
    assert_eq!(resumed.advance(3).unwrap().bytes(), b"A");
    assert_eq!(resumed.advance(4).unwrap().bytes(), b"A");
    assert_eq!(resumed.sampled_draws(), 3);
    assert_eq!(resumed.decoder_work().tokens, 5);
    assert_eq!(resumed.into_parts().unwrap().1.generation().requested_prompt_tokens(), 0);
}

#[test]
fn whole_prompt_output_and_numerical_preflight_precede_first_pull() {
    for case in 0..3 {
        let owner = run(65, None);
        let observation = owner.observation();
        let mut input = request(b"ab", 3);
        match case {
            0 => input.max_output_bytes = 5,
            1 => input.prompt = b"ab".repeat(32),
            2 => input.generation.scalar_products = 0,
            _ => unreachable!(),
        }
        let failure = owner.into_generation(0, input).unwrap_err();
        assert_eq!(failure.error, Error::Limit);
        assert_eq!(observation.availability(), DecoderAvailability::Closed);
    }
    let accepted = run(65, None).into_generation(0, request(b"ab", 3)).unwrap();
    assert_eq!(accepted.decoder_work().tokens, 0);
    assert_eq!(accepted.sampled_draws(), 0);
    assert!(accepted.progress().report().is_none());
}

#[test]
fn output_failure_keeps_native_work_and_forbids_reroll_or_partial_owner_export() {
    let mut session = run(65, None).into_generation(0, request(b"ab", 3)).unwrap();
    prefill(&mut session);
    // Plant an inconsistent downstream capacity after valid admission. The real
    // native token must still be charged, but cannot become a partial text success.
    session.capacity = 0;
    assert_eq!(session.advance(2).unwrap_err(), Error::Limit);
    assert_eq!(session.output_failure(), Some(Error::Limit));
    assert_eq!(session.bytes(), Err(Error::Limit));
    assert_eq!(session.sampled_draws(), 1);
    assert_eq!(session.decoder_work().tokens, 3);
    assert_eq!(session.advance(3).unwrap_err(), Error::WrongState);
    let cancelled = session.cancel();
    assert!(cancelled.bytes().is_empty());
    assert_eq!(cancelled.output_failure(), Some(Error::Limit));
    assert_eq!(cancelled.sampled_draws(), 1);
    assert_eq!(cancelled.progress().tokens(), &[65]);
}

#[test]
fn caught_unwind_after_native_step_cannot_pull_again_or_claim_current_output() {
    let owner = run(65, None);
    let observation = owner.observation();
    let mut session = owner.into_generation(0, request(b"ab", 3)).unwrap();
    prefill(&mut session);
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = session.advance_with(2, || panic!("selected post-native interruption"));
    }));
    assert!(unwound.is_err());
    assert!(session.interrupted());
    assert_eq!(session.bytes(), Err(Error::Incomplete));
    assert_eq!(session.sampled_draws(), 1);
    assert_eq!(session.decoder_work().tokens, 3);
    assert_eq!(session.advance(3).unwrap_err(), Error::WrongState);
    let cancelled = session.cancel();
    assert!(cancelled.interrupted());
    assert!(cancelled.bytes().is_empty());
    assert_eq!(cancelled.progress().tokens(), &[65]);
    assert_eq!(cancelled.sampled_draws(), 1);
    assert_eq!(observation.availability(), DecoderAvailability::Closed);
}
