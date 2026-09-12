//! Stochastic continuation uses the actual existing decoder and cache writer.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::*;
use fa_reference::action::consequence::activation::tensor::kv::model::ModelKvImage;
use fa_reference::Error;

fn products(m: &DecoderModel, position: usize, n: usize) -> DecoderBudget {
    DecoderBudget { scalar_products: m.estimate(position, n).unwrap().scalar_products().unwrap() }
}
fn budget(m: &DecoderModel, position: usize, n: usize) -> SampleBudget {
    SampleBudget { decoder: products(m, position, n), sampling: SamplingBudget { vocabulary: m.profile().shape().vocabulary } }
}
fn start(m: &DecoderModel, seed: u64) -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(7, 1, m.profile().shape().vocabulary, 1.25, 0, 0.9).unwrap(), stream: 100, seed }
}
fn restore_budget(cp: &SampledCheckpoint) -> DecoderRestoreBudget {
    DecoderRestoreBudget { cache_values: cp.numerical().cache().normalized_values() }
}
fn bits(v: &[f32]) -> Vec<u32> { v.iter().map(|x| x.to_bits()).collect() }
fn same_cache(a: &ModelKvImage, b: &ModelKvImage) {
    assert_eq!(a.profile(), b.profile()); assert_eq!(a.len(), b.len());
    for id in a.profile().layers().keys() {
        // Stream and capture revisions are different provenance, not numeric data.
        let a = a.layer(*id).unwrap().encode().unwrap();
        let b = b.layer(*id).unwrap().encode().unwrap();
        assert_eq!(&a[156..], &b[156..]);
    }
}
fn same_state(a: &SampledSession, b: &SampledSession) {
    assert_eq!(a.tokens(), b.tokens());
    assert_eq!(bits(a.logits().unwrap()), bits(b.logits().unwrap()));
    assert_eq!(a.sampler_state(), b.sampler_state());
    assert_eq!(a.sampled_positions(), b.sampled_positions());
    same_cache(&a.cache_image().unwrap(), &b.cache_image().unwrap());
}

#[test]
fn uninterrupted_restored_and_recomputed_sampling_agree_for_32_steps() {
    let m = model(profile(48));
    let mut a = m.recompute_sampled(1, &[0, 3, 1], products(&m, 0, 3), start(&m, 71)).unwrap();
    for _ in 0..4 { let p = a.position(); a.advance_sampled(p, budget(&m, p as usize, 1)).unwrap(); }
    let cp = a.checkpoint().unwrap();
    let (mut b, receipt) = m.restore_sampled_checkpoint(&cp, 2, restore_budget(&cp)).unwrap();
    let mut c = m.recompute_sampled_checkpoint(&cp, 3, budget(&m, 0, cp.numerical().tokens().len())).unwrap();
    same_state(&a, &b); same_state(&a, &c);
    assert_eq!(receipt.bytes_written, receipt.bytes_recaptured);
    assert_eq!(b.work(), DecoderWork::default());
    assert_eq!(b.sampler_state().draws(), 4);
    for _ in 0..32 {
        let p = a.position(); let cost = budget(&m, p as usize, 1);
        let x = a.advance_sampled(p, cost).unwrap();
        let y = b.advance_sampled(p, cost).unwrap();
        let z = c.advance_sampled(p, cost).unwrap();
        assert_eq!(x.choice, y.choice); assert_eq!(x.choice, z.choice);
        assert_eq!(bits(&x.computation.logits), bits(&y.computation.logits));
        same_state(&a, &b); same_state(&a, &c);
    }
    assert_eq!(cp.sampler_state().draws(), 4);
    assert_eq!(b.work().tokens, 32);
    assert_eq!(a.sampler_state().draws(), 36);
}

#[test]
fn mixed_teacher_forcing_is_recomputed_without_inventing_extra_draws() {
    let m = model(profile(32));
    let mut a = m.recompute_sampled(1, &[2, 4], products(&m, 0, 2), start(&m, 19)).unwrap();
    for position in 2..10 {
        if position % 3 == 0 {
            let before = a.sampler_state();
            a.advance_forced(position, (position % 6) as u32, products(&m, position as usize, 1)).unwrap();
            assert_eq!(a.sampler_state(), before);
        } else { a.advance_sampled(position, budget(&m, position as usize, 1)).unwrap(); }
    }
    assert_eq!(a.sampled_positions(), &[2, 4, 5, 7, 8]);
    let cp = a.checkpoint().unwrap();
    let mut b = m.recompute_sampled_checkpoint(&cp, 2, budget(&m, 0, 10)).unwrap();
    same_state(&a, &b);
    for position in 10..18 {
        assert_eq!(a.advance_sampled(position, budget(&m, position as usize, 1)).unwrap().choice,
            b.advance_sampled(position, budget(&m, position as usize, 1)).unwrap().choice);
    }
}

#[test]
fn failed_late_vocabulary_projection_does_not_consume_rng_or_publish_token() {
    let p = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: 2, hidden: 4, intermediate: 4, layers: 2,
            query_heads: 2, cache_heads: 1, context: 8 }, 0.00001, 10000.0).unwrap();
    let m = DecoderModel::new(p.clone(), vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0], zero_layers(&p),
        vec![1.0; 4], vec![0.0, 0.0, 0.0, 0.0, 0.0, f32::MAX, f32::MAX, f32::MAX]).unwrap();
    let config = |k| SamplingStart { policy: SamplingPolicy::new(1, 1, 2, 1.0, k, 1.0).unwrap(), stream: 3, seed: 0 };
    let mut a = m.recompute_sampled(1, &[0], products(&m, 0, 1), config(0)).unwrap();
    assert_eq!(a.logits().unwrap(), &[0.0, 0.0]);
    let rng = a.sampler_state(); let before = a.cache_image().unwrap().encode().unwrap(); let work = a.work();
    let mut oracle = Sampler::from_snapshot(&rng);
    assert_eq!(oracle.sample(a.logits().unwrap(), SamplingBudget { vocabulary: 2 }).unwrap().token, 1);
    for _ in 0..2 {
        assert_eq!(a.advance_sampled(1, budget(&m, 1, 1)).unwrap_err(), Error::Overflow);
        assert_eq!(a.sampler_state(), rng); assert_eq!(a.tokens(), &[0]);
        assert!(a.sampled_positions().is_empty()); assert_eq!(a.work(), work);
        assert_eq!(a.cache_image().unwrap().encode().unwrap(), before);
    }
    // Near-identical positive control: top-k=1 chooses the lower tied ID, whose
    // actual forward pass is finite. It still consumes exactly one random word.
    let mut allowed = m.recompute_sampled(2, &[0], products(&m, 0, 1), config(1)).unwrap();
    assert_eq!(allowed.advance_sampled(1, budget(&m, 1, 1)).unwrap().choice.token, 0);
    assert_eq!(allowed.sampler_state().draws(), 1);
}

#[test]
fn budget_stale_position_and_forced_token_errors_preserve_the_same_next_draw() {
    let m = model(profile(8));
    let mut a = m.recompute_sampled(1, &[1], products(&m, 0, 1), start(&m, 7)).unwrap();
    let cp = a.checkpoint().unwrap();
    let (mut control, _) = m.restore_sampled_checkpoint(&cp, 2, restore_budget(&cp)).unwrap();
    let mut short = budget(&m, 1, 1); short.decoder.scalar_products -= 1;
    assert_eq!(a.advance_sampled(1, short).unwrap_err(), Error::Limit);
    short = budget(&m, 1, 1); short.sampling.vocabulary -= 1;
    assert_eq!(a.advance_sampled(1, short).unwrap_err(), Error::Limit);
    assert_eq!(a.advance_sampled(0, budget(&m, 1, 1)).unwrap_err(), Error::Stale);
    assert_eq!(a.advance_forced(1, 6, products(&m, 1, 1)).unwrap_err(), Error::InvalidInput);
    same_state(&a, &control);
    assert_eq!(a.advance_sampled(1, budget(&m, 1, 1)).unwrap().choice,
        control.advance_sampled(1, budget(&m, 1, 1)).unwrap().choice);
}

#[test]
fn checkpoint_pins_rng_parameters_and_cache_after_original_owners_drop() {
    let cp = {
        let m = model(profile(16));
        let mut s = m.recompute_sampled(1, &[1, 2], products(&m, 0, 2), start(&m, 8)).unwrap();
        for p in 2..5 { s.advance_sampled(p, budget(&m, p as usize, 1)).unwrap(); }
        s.checkpoint().unwrap()
    };
    let m = cp.numerical().model().clone();
    let mut oracle = Sampler::from_snapshot(cp.sampler_state());
    let expected = oracle.sample(cp.numerical().logits().unwrap(), SamplingBudget { vocabulary: 6 }).unwrap();
    let (mut restored, _) = m.restore_sampled_checkpoint(&cp, 2, restore_budget(&cp)).unwrap();
    drop(cp);
    assert_eq!(restored.advance_sampled(5, budget(&m, 5, 1)).unwrap().choice, expected);
}

#[test]
fn independent_same_named_models_and_old_capture_streams_are_rejected() {
    let m = model(profile(8)); let other = model(profile(8));
    let s = m.recompute_sampled(1, &[1, 4], products(&m, 0, 2), start(&m, 0)).unwrap();
    let cp = s.checkpoint().unwrap();
    assert_eq!(other.restore_sampled_checkpoint(&cp, 2, restore_budget(&cp)).unwrap_err(), Error::Binding);
    assert_eq!(other.recompute_sampled_checkpoint(&cp, 2, budget(&other, 0, 2)).unwrap_err(), Error::Binding);
    for stream in [0, 1] {
        assert_eq!(m.restore_sampled_checkpoint(&cp, stream, restore_budget(&cp)).unwrap_err(), Error::InvalidInput);
    }
    assert!(m.clone().restore_sampled_checkpoint(&cp, 2, restore_budget(&cp)).is_ok());
    let invalid = SamplingStart { policy: SamplingPolicy::new(1, 1, 5, 1.0, 0, 1.0).unwrap(), stream: 2, seed: 0 };
    assert_eq!(m.sampled_session(3, invalid).unwrap_err(), Error::Binding);
}

#[test]
fn restored_siblings_do_not_share_mutable_draw_state_or_cache() {
    let m = model(profile(12));
    let parent = m.recompute_sampled(1, &[5, 0], products(&m, 0, 2), start(&m, 4)).unwrap();
    let cp = parent.checkpoint().unwrap(); let saved = cp.numerical().cache().encode().unwrap();
    let (mut a, _) = m.restore_sampled_checkpoint(&cp, 2, restore_budget(&cp)).unwrap();
    let (mut b, _) = m.restore_sampled_checkpoint(&cp, 3, restore_budget(&cp)).unwrap();
    let original_b_rng = b.sampler_state();
    let first = a.advance_sampled(2, budget(&m, 2, 1)).unwrap();
    let second = a.advance_sampled(3, budget(&m, 3, 1)).unwrap();
    assert_eq!(b.sampler_state(), original_b_rng); assert_eq!(b.position(), 2);
    assert_eq!(b.advance_sampled(2, budget(&m, 2, 1)).unwrap().choice, first.choice);
    assert_eq!(b.advance_sampled(3, budget(&m, 3, 1)).unwrap().choice, second.choice);
    same_state(&a, &b);
    a.advance_forced(4, 0, products(&m, 4, 1)).unwrap();
    b.advance_forced(4, 5, products(&m, 4, 1)).unwrap();
    assert_ne!(a.tokens(), b.tokens()); assert_eq!(a.sampler_state(), b.sampler_state());
    assert_eq!(cp.numerical().cache().encode().unwrap(), saved);
}

#[test]
fn empty_and_full_checkpoints_cannot_invent_logits_or_context_capacity() {
    let m = model(profile(2));
    let mut empty = m.sampled_session(1, start(&m, 1)).unwrap();
    let state = empty.sampler_state();
    assert_eq!(empty.advance_sampled(0, budget(&m, 0, 1)).unwrap_err(), Error::Incomplete);
    assert_eq!(empty.sampler_state(), state);
    let cp = empty.checkpoint().unwrap();
    let (mut a, _) = m.restore_sampled_checkpoint(&cp, 2, restore_budget(&cp)).unwrap();
    a.advance_forced(0, 1, products(&m, 0, 1)).unwrap();
    a.advance_sampled(1, budget(&m, 1, 1)).unwrap();
    let full = a.checkpoint().unwrap();
    let (mut b, _) = m.restore_sampled_checkpoint(&full, 3, restore_budget(&full)).unwrap();
    let rng = b.sampler_state();
    assert_eq!(b.advance_sampled(2, SampleBudget { decoder: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
        sampling: SamplingBudget { vocabulary: 6 } }).unwrap_err(), Error::Limit);
    assert_eq!(b.sampler_state(), rng); assert_eq!(b.tokens(), a.tokens());
}

#[test]
fn whole_recomputation_and_restore_budgets_are_not_reset_per_token() {
    let m = model(profile(10));
    let mut a = m.recompute_sampled(1, &[2, 4], products(&m, 0, 2), start(&m, 45)).unwrap();
    for p in 2..6 { a.advance_sampled(p, budget(&m, p as usize, 1)).unwrap(); }
    let cp = a.checkpoint().unwrap(); let rng = cp.sampler_state().clone();
    let mut short = budget(&m, 0, 6); short.decoder.scalar_products -= 1;
    assert_eq!(m.recompute_sampled_checkpoint(&cp, 2, short).unwrap_err(), Error::Limit);
    let mut restore = restore_budget(&cp); restore.cache_values -= 1;
    assert_eq!(m.restore_sampled_checkpoint(&cp, 2, restore).unwrap_err(), Error::Limit);
    let replayed = m.recompute_sampled_checkpoint(&cp, 2, budget(&m, 0, 6)).unwrap();
    assert_eq!(replayed.work().tokens, 6); assert_eq!(replayed.sampler_state().draws(), 4);
    assert_eq!(cp.sampler_state(), &rng);
}

#[test]
fn imported_mixed_precision_weights_use_the_same_sampled_restart_path() {
    let (m, _) = DecoderModel::from_safetensors(profile(20), include_bytes!("fixtures/decoder_mixed.safetensors")).unwrap();
    let mut a = m.recompute_sampled(1, &[1, 3, 2], products(&m, 0, 3), start(&m, 99)).unwrap();
    for p in 3..6 { a.advance_sampled(p, budget(&m, p as usize, 1)).unwrap(); }
    let cp = a.checkpoint().unwrap();
    let (mut b, _) = m.restore_sampled_checkpoint(&cp, 2, restore_budget(&cp)).unwrap();
    for p in 6..14 {
        assert_eq!(a.advance_sampled(p, budget(&m, p as usize, 1)).unwrap().choice,
            b.advance_sampled(p, budget(&m, p as usize, 1)).unwrap().choice);
        same_state(&a, &b);
    }
}

#[test]
fn repeated_restart_retains_initial_draw_lineage_and_sampled_position_history() {
    let m = model(profile(20));
    let mut a = m.recompute_sampled(1, &[2, 1], products(&m, 0, 2), start(&m, 19)).unwrap();
    let mut b = m.recompute_sampled(2, &[2, 1], products(&m, 0, 2), start(&m, 19)).unwrap();
    for cycle in 0..4 {
        for _ in 0..2 {
            let p = a.position(); let cost = budget(&m, p as usize, 1);
            assert_eq!(a.advance_sampled(p, cost).unwrap().choice, b.advance_sampled(p, cost).unwrap().choice);
        }
        let cp = b.checkpoint().unwrap();
        b = m.restore_sampled_checkpoint(&cp, 10 + cycle, restore_budget(&cp)).unwrap().0;
        let replay = m.recompute_sampled_checkpoint(&cp, 30 + cycle, budget(&m, 0, b.position() as usize)).unwrap();
        same_state(&a, &b); same_state(&b, &replay);
    }
    assert_eq!(b.sampler_state().draws(), 8);
    assert_eq!(b.sampled_positions(), &(2..10).collect::<Vec<_>>());
}

#[test]
fn stream_labels_do_not_falsely_claim_independent_random_sequences() {
    let m = model(profile(8));
    let mut first = start(&m, 0); first.stream = 10;
    let mut second = first.clone(); second.stream = 11;
    let mut a = m.recompute_sampled(1, &[0], products(&m, 0, 1), first).unwrap();
    let mut b = m.recompute_sampled(2, &[0], products(&m, 0, 1), second).unwrap();
    let x = a.advance_sampled(1, budget(&m, 1, 1)).unwrap().choice;
    let y = b.advance_sampled(1, budget(&m, 1, 1)).unwrap().choice;
    assert_eq!(x.random_word, y.random_word); assert_eq!(x.token, y.token);
    assert_ne!(x.stream, y.stream);
}
