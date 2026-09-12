use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{
    Sampler, SamplerSnapshot, SamplingBudget, SamplingPolicy, SAMPLER_SNAPSHOT_BYTES,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_VOCABULARY;
use fa_reference::Error;

fn policy(v: usize, t: f64, k: usize, p: f64) -> SamplingPolicy {
    SamplingPolicy::new(7, 3, v, t, k, p).unwrap()
}
fn budget(v: usize) -> SamplingBudget { SamplingBudget { vocabulary: v } }

#[test]
fn seed_zero_matches_literal_integer_vectors_and_uniform_bins() {
    let mut sampler = Sampler::seeded(policy(4, 1.0, 0, 1.0), 19, 0).unwrap();
    let initial = sampler.snapshot().encode();
    assert_eq!(&initial[..8], b"FASAMP\0\x01");
    assert_eq!(&initial[48..56], &19_u64.to_be_bytes());
    assert_eq!(&initial[56..64], &[0; 8]);
    let seeded = [0xe220a8397b1dcdaf_u64, 0x6e789e6aa1b965f4, 0x06c45d188009454f, 0xf88bb8a8724c81ec];
    for (index, word) in seeded.iter().enumerate() {
        assert_eq!(&initial[64 + 8 * index..72 + 8 * index], &word.to_be_bytes());
    }
    // Literal values from a separate modular-integer transcription of the
    // published PRNG, not expectations produced by the Rust sampler under test.
    let words = [0x99ec5f36cb75f2b4_u64, 0xbf6e1f784956452a, 0x1a5f849d4933e6e0,
        0x6aa594f1262d2d2c, 0xbba5ad4a1f842e59, 0xffef8375d9ebcaca,
        0x6c160deed2f54c98, 0x8920ad648fc30a3f];
    for (index, expected) in words.into_iter().enumerate() {
        let draw = sampler.sample(&[0.0; 4], budget(4)).unwrap();
        assert_eq!(draw.random_word, expected);
        assert_eq!(draw.token, (expected >> 62) as u32);
        assert_eq!(draw.probability, 0.25);
        assert_eq!(draw.draw, index as u64 + 1);
    }
}

#[test]
fn top_k_precedes_nucleus_and_ties_have_stable_original_ids() {
    let d = policy(4, 1.0, 2, 0.5).distribution(&[0.0; 4], budget(4)).unwrap();
    assert_eq!(d.probabilities().collect::<Vec<_>>(), vec![(0, 1.0)]);
    assert_eq!(d.work().logits_scanned, 4);
    assert_eq!(d.work().exponentials, 2);
    let d = policy(4, 1.0, 0, 0.5).distribution(&[0.0; 4], budget(4)).unwrap();
    assert_eq!(d.probabilities().collect::<Vec<_>>(), vec![(0, 0.5), (1, 0.5)]);
    let d = policy(4, 1.0, 1, 1.0).distribution(&[-0.0, 0.0, -1.0, -1.0], budget(4)).unwrap();
    assert_eq!(d.probabilities().collect::<Vec<_>>(), vec![(0, 1.0)]);
}

#[test]
fn temperature_has_the_declared_two_token_distribution() {
    for t in [0.25, 1.0, 2.0, 100.0] {
        let probabilities: Vec<_> = policy(2, t, 0, 1.0).distribution(&[0.0, 2.0], budget(2))
            .unwrap().probabilities().collect();
        let expected = 1.0 / (1.0 + (-2.0 / t).exp());
        assert_eq!(probabilities[0].0, 1);
        assert!((probabilities[0].1 - expected).abs() < 1e-15);
        assert!((probabilities.iter().map(|(_, p)| p).sum::<f64>() - 1.0).abs() < 1e-15);
    }
}

#[test]
fn extreme_scores_are_shifted_before_scaling_and_underflow_is_observed() {
    let p = policy(3, 0.000001, 0, 1.0);
    let d = p.distribution(&[-f32::MAX, f32::MAX, 0.0], budget(3)).unwrap();
    assert_eq!(d.work().zero_weights, 2);
    assert_eq!(d.probabilities().next(), Some((1, 1.0)));
    let mut sampler = Sampler::seeded(p, 1, u64::MAX).unwrap();
    for _ in 0..64 { assert_eq!(sampler.sample(&[-f32::MAX, f32::MAX, 0.0], budget(3)).unwrap().token, 1); }
    let tiny = f64::from_bits(1);
    let d = policy(3, 1.0, 0, tiny).distribution(&[1.0, 0.0, -1.0], budget(3)).unwrap();
    assert_eq!(d.probabilities().collect::<Vec<_>>(), vec![(0, 1.0)]);
}

#[test]
fn policy_admission_rejects_hidden_defaults_and_unsupported_domains() {
    for t in [0.0, -1.0, 0.0000001, 1_000_001.0, f64::INFINITY, f64::NAN] {
        assert!(SamplingPolicy::new(1, 1, 4, t, 0, 1.0).is_err());
    }
    for p in [0.0, -1.0, 1.01, f64::INFINITY, f64::NAN] {
        assert!(SamplingPolicy::new(1, 1, 4, 1.0, 0, p).is_err());
    }
    assert!(SamplingPolicy::new(0, 1, 4, 1.0, 0, 1.0).is_err());
    assert!(SamplingPolicy::new(1, 0, 4, 1.0, 0, 1.0).is_err());
    assert!(SamplingPolicy::new(1, 1, 0, 1.0, 0, 1.0).is_err());
    assert!(SamplingPolicy::new(1, 1, 4, 1.0, 5, 1.0).is_err());
    assert!(Sampler::seeded(policy(1, 1.0, 0, 1.0), 0, 0).is_err());
}

#[test]
fn malformed_discarded_scores_and_insufficient_budgets_do_not_draw() {
    let mut s = Sampler::seeded(policy(3, 1.0, 1, 1.0), 1, 123).unwrap();
    let before = s.snapshot();
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(s.sample(&[1.0, 0.0, invalid], budget(3)), Err(Error::InvalidInput));
        assert_eq!(s.snapshot(), before);
    }
    assert_eq!(s.sample(&[f32::NAN; 3], budget(2)), Err(Error::Limit));
    assert_eq!(s.sample(&[1.0; 2], budget(3)), Err(Error::Binding));
    assert_eq!(s.sample(&[1.0; 3], budget(MAX_DECODER_VOCABULARY + 1)), Err(Error::Limit));
    assert_eq!(s.snapshot(), before);
    assert_eq!(s.sample(&[1.0, 0.0, -1.0], budget(3)).unwrap().draw, 1);
}

#[test]
fn sampler_serialization_restores_exact_next_draws() {
    let p = policy(5, 0.75, 4, 0.8);
    let mut a = Sampler::seeded(p.clone(), 10, 124).unwrap();
    let logits = [2.0, 0.25, -1.0, 1.0, 0.0];
    for _ in 0..7 { a.sample(&logits, budget(5)).unwrap(); }
    let saved = a.snapshot();
    let decoded = SamplerSnapshot::decode(&saved.encode(), &p).unwrap();
    assert_eq!(decoded, saved);
    let mut b = Sampler::from_snapshot(&decoded);
    for _ in 0..64 {
        assert_eq!(a.sample(&logits, budget(5)), b.sample(&logits, budget(5)));
        assert_eq!(a.snapshot(), b.snapshot());
    }
    assert_eq!(saved.draws(), 7);
}

#[test]
fn snapshot_framing_configuration_and_forbidden_zero_state_refuse() {
    let p = policy(4, 1.0, 0, 1.0);
    let original = Sampler::seeded(p.clone(), 10, 4).unwrap().snapshot().encode();
    assert_eq!(original.len(), SAMPLER_SNAPSHOT_BYTES);
    for n in 0..original.len() { assert!(SamplerSnapshot::decode(&original[..n], &p).is_err()); }
    let mut extra = original.to_vec(); extra.push(0);
    assert!(SamplerSnapshot::decode(&extra, &p).is_err());
    for index in 0..48 {
        let mut bad = original; bad[index] ^= 1;
        assert!(SamplerSnapshot::decode(&bad, &p).is_err(), "accepted changed header byte {index}");
    }
    let mut bad = original; bad[48..56].fill(0);
    assert!(SamplerSnapshot::decode(&bad, &p).is_err());
    let mut bad = original; bad[64..].fill(0);
    assert!(SamplerSnapshot::decode(&bad, &p).is_err());
    assert!(SamplerSnapshot::decode(&original, &policy(4, 1.0, 1, 1.0)).is_err());
}

#[test]
fn portable_state_is_not_misrepresented_as_authentication() {
    let p = policy(4, 1.0, 0, 1.0);
    let original = Sampler::seeded(p.clone(), 10, 4).unwrap().snapshot();
    let mut altered = original.encode(); altered[64] ^= 1;
    let accepted = SamplerSnapshot::decode(&altered, &p).unwrap();
    assert_ne!(accepted, original);
    // The owning sampled-decoder checkpoint will not admit an arbitrary sampler
    // replacement. This standalone format is only bounded numerical replay data.
}

#[test]
fn draw_counter_overflow_refuses_without_wrap_or_state_change() {
    let p = policy(4, 1.0, 0, 1.0);
    let mut bytes = Sampler::seeded(p.clone(), 1, 0).unwrap().snapshot().encode();
    bytes[56..64].copy_from_slice(&u64::MAX.to_be_bytes());
    let saved = SamplerSnapshot::decode(&bytes, &p).unwrap();
    let mut sampler = Sampler::from_snapshot(&saved);
    assert_eq!(sampler.sample(&[0.0; 4], budget(4)), Err(Error::Overflow));
    assert_eq!(sampler.snapshot(), saved);
}

#[test]
fn uniform_nucleus_prefix_matches_independent_cardinality_formula() {
    for v in 1..=32 {
        for k in 0..=v {
            for p in [0.25, 0.5, 0.75, 1.0] {
                let retained = if k == 0 { v } else { k };
                let expected = (retained as f64 * p).ceil() as usize;
                let d = policy(v, 1.0, k, p).distribution(&vec![0.0; v], budget(v)).unwrap();
                let probabilities: Vec<_> = d.probabilities().collect();
                assert_eq!(probabilities.len(), expected);
                for (index, (id, probability)) in probabilities.iter().enumerate() {
                    assert_eq!(*id, index as u32);
                    assert_eq!(*probability, 1.0 / expected as f64);
                }
            }
        }
    }
}

#[test]
fn full_vocabulary_and_singleton_still_have_explicit_sampling_costs() {
    let v = MAX_DECODER_VOCABULARY;
    let d = policy(v, 1.0, 0, 0.5).distribution(&vec![0.0; v], budget(v)).unwrap();
    assert_eq!(d.work().exponentials, v);
    assert_eq!(d.work().retained_candidates, v / 2);
    let mut s = Sampler::seeded(policy(2, 1.0, 1, 1.0), 1, 0).unwrap();
    let first = s.sample(&[1.0, 0.0], budget(2)).unwrap();
    let second = s.sample(&[1.0, 0.0], budget(2)).unwrap();
    assert_eq!((first.token, second.token), (0, 0));
    assert_ne!(first.random_word, second.random_word);
    assert_eq!(s.snapshot().draws(), 2);
}
