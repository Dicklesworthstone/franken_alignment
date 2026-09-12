//! Portable data is never installed until the original stochastic path checks it.
#[path = "support/decoder_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::archive::*;
use fa_reference::Error;

fn budget() -> SampleBudget {
    SampleBudget { decoder: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
        sampling: SamplingBudget { vocabulary: 6 } }
}
fn start() -> SamplingStart {
    SamplingStart { policy: SamplingPolicy::new(4, 2, 6, 0.8, 5, 0.95).unwrap(), stream: 77, seed: 123 }
}
fn populated(context: usize) -> (DecoderModel, SampledSession) {
    let m = model(profile(context));
    let mut s = m.recompute_sampled(9, &[0, 3], budget().decoder, start()).unwrap();
    s.advance_sampled(2, budget()).unwrap();
    s.advance_forced(3, 1, budget().decoder).unwrap();
    s.advance_sampled(4, budget()).unwrap();
    (m, s)
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }
fn same(a: &SampledSession, b: &SampledSession) {
    assert_eq!(a.tokens(), b.tokens());
    assert_eq!(a.sampled_positions(), b.sampled_positions());
    assert_eq!(a.sampler_state(), b.sampler_state());
    assert_eq!(a.logits().map(bits), b.logits().map(bits));
    let a = a.cache_image().unwrap(); let b = b.cache_image().unwrap();
    for id in a.profile().layers().keys() {
        let a = a.layer(*id).unwrap().encode().unwrap();
        let b = b.layer(*id).unwrap().encode().unwrap();
        assert_eq!(&a[156..], &b[156..]);
    }
}
fn decode(bytes: &[u8], m: &DecoderModel) -> Result<SampledArchive, Error> {
    SampledArchive::decode(bytes, m, &start().policy, ArchiveLimits::default())
}
fn cache_at(bytes: &[u8]) -> usize {
    let count = |offset| u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    320 + 4 * count(100) + 8 * count(104) + 4 * count(108)
}

#[test]
fn reloaded_model_verifies_portable_state_then_matches_thirty_two_random_steps() {
    let (m, mut original) = populated(48);
    let checkpoint = original.checkpoint().unwrap();
    let bytes = checkpoint.encode_archive(ArchiveLimits::default()).unwrap();
    let loaded = model(profile(48));
    // Direct restore still forbids substituting separately allocated parameters.
    assert_eq!(loaded.restore_sampled_checkpoint(&checkpoint, 10,
        DecoderRestoreBudget { cache_values: checkpoint.numerical().cache().normalized_values() }).unwrap_err(), Error::Binding);
    let archive = decode(&bytes, &loaded).unwrap();
    let (mut replay, receipt) = archive.recompute(&loaded, 10, budget()).unwrap();
    assert_eq!(receipt.source, checkpoint.numerical().cache().descriptor());
    assert_eq!(receipt.replay_stream, 10); assert_eq!(receipt.tokens_compared, 5);
    assert_eq!(receipt.sampled_tokens_compared, 2); assert_eq!(receipt.logits_compared, 6);
    assert_eq!(receipt.encoded_bytes, bytes.len());
    assert_eq!(receipt.recomputation.tokens, 5);
    same(&original, &replay);
    for _ in 0..32 {
        let position = original.position();
        let a = original.advance_sampled(position, budget()).unwrap();
        let b = replay.advance_sampled(position, budget()).unwrap();
        assert_eq!(a.choice, b.choice); same(&original, &replay);
    }
    assert_eq!(checkpoint.numerical().tokens().len(), 5);
    assert_eq!(checkpoint.encode_archive(ArchiveLimits::default()).unwrap(), bytes);
    drop(m);
}

#[test]
fn saved_bytes_survive_all_original_owners_and_keep_forced_positions() {
    let bytes = {
        let (_, s) = populated(12);
        s.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap()
    };
    let m = model(profile(12));
    let archive = decode(&bytes, &m).unwrap();
    assert_eq!(archive.source_stream(), 9); assert_eq!(archive.sampled_positions(), &[2, 4]);
    assert_eq!(archive.initial_sampler().draws(), 0); assert_eq!(archive.final_sampler().draws(), 2);
    let (mut s, _) = archive.recompute(&m, 11, budget()).unwrap();
    drop(archive);
    s.advance_forced(5, 5, budget().decoder).unwrap();
    assert_eq!(s.sampler_state().draws(), 2);
    s.advance_sampled(6, budget()).unwrap(); assert_eq!(s.sampler_state().draws(), 3);
    let bytes2 = s.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let archive2 = decode(&bytes2, &m).unwrap();
    assert_eq!(archive2.source_stream(), 11);
    assert_eq!(archive2.sampled_positions(), &[2, 4, 6]);
    let (again, _) = archive2.recompute(&m, 12, budget()).unwrap(); same(&s, &again);
}

#[test]
fn every_truncation_suffix_component_flag_and_profile_change_refuses() {
    let (m, s) = populated(8);
    let bytes = s.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    assert!(decode(&bytes, &m).is_ok());
    for end in 0..bytes.len() { assert!(decode(&bytes[..end], &m).is_err(), "end={end}"); }
    let mut changed = bytes.clone(); changed.push(0); assert!(decode(&changed, &m).is_err());
    for at in 0..92 {
        let mut changed = bytes.clone(); changed[at] ^= 1;
        assert!(decode(&changed, &m).is_err(), "profile/header byte={at}");
    }
    for bit in 0..32 {
        let mut changed = bytes.clone(); changed[124..128].copy_from_slice(&(63_u32 ^ (1 << bit)).to_be_bytes());
        assert!(decode(&changed, &m).is_err(), "components bit={bit}");
    }
    for at in [100, 104, 108, 112, 116] {
        let mut changed = bytes.clone(); changed[at..at + 4].fill(255);
        assert!(decode(&changed, &m).is_err());
    }
}

#[test]
fn finite_cache_and_logit_edits_parse_but_cannot_become_a_session() {
    let (m, s) = populated(8);
    let bytes = s.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let at = cache_at(&bytes);
    let descriptor = u32::from_be_bytes(bytes[112..116].try_into().unwrap()) as usize;
    for offset in [at - 4, at + descriptor, bytes.len() - 4] {
        let mut changed = bytes.clone();
        let replacement = if offset < at { 123.0_f32.to_bits().to_be_bytes() } else { 123.0_f32.to_le_bytes() };
        changed[offset..offset + 4].copy_from_slice(&replacement);
        let archive = decode(&changed, &m).unwrap();
        assert_eq!(archive.recompute(&m, 10, budget()).unwrap_err(), Error::Binding);
    }
    assert!(decode(&bytes, &m).unwrap().recompute(&m, 10, budget()).is_ok());
}

#[test]
fn full_rng_words_and_sampled_attribution_are_checked_not_just_counters() {
    let (m, s) = populated(8);
    let bytes = s.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    for at in [128 + 64, 224 + 64] {
        let mut changed = bytes.clone(); changed[at] ^= 1;
        let archive = decode(&changed, &m).unwrap();
        assert_eq!(archive.final_sampler().draws(), 2);
        assert_eq!(archive.recompute(&m, 10, budget()).unwrap_err(), Error::Binding);
    }
    let positions_at = 320 + 4 * 5;
    let mut changed = bytes.clone();
    changed[positions_at..positions_at + 8].copy_from_slice(&0_u64.to_be_bytes());
    assert!(decode(&changed, &m).is_err());
    changed[positions_at..positions_at + 8].copy_from_slice(&4_u64.to_be_bytes());
    assert!(decode(&changed, &m).is_err());
    let mut changed = bytes.clone(); changed[224 + 56..224 + 64].copy_from_slice(&3_u64.to_be_bytes());
    assert!(decode(&changed, &m).is_err());
}

#[test]
fn exactly_sized_budgets_work_one_under_fails_without_changing_source() {
    let (m, s) = populated(8); let cp = s.checkpoint().unwrap();
    let bytes = cp.encode_archive(ArchiveLimits::default()).unwrap();
    let limits = ArchiveLimits { bytes: bytes.len(), tokens: 5, cache_values: cp.numerical().cache().normalized_values() };
    assert_eq!(cp.encode_archive(limits).unwrap(), bytes);
    assert!(SampledArchive::decode(&bytes, &m, &start().policy, limits).is_ok());
    for small in [ArchiveLimits { bytes: limits.bytes - 1, ..limits }, ArchiveLimits { tokens: 4, ..limits },
        ArchiveLimits { cache_values: limits.cache_values - 1, ..limits }] {
        assert_eq!(cp.encode_archive(small), Err(Error::Limit));
        assert_eq!(SampledArchive::decode(&bytes, &m, &start().policy, small).unwrap_err(), Error::Limit);
    }
    let archive = decode(&bytes, &m).unwrap();
    let products = m.estimate(0, 5).unwrap().scalar_products().unwrap();
    let mut insufficient = budget(); insufficient.decoder.scalar_products = products - 1;
    assert_eq!(archive.recompute(&m, 10, insufficient).unwrap_err(), Error::Limit);
    insufficient.decoder.scalar_products = products;
    let (again, _) = archive.recompute(&m, 10, insufficient).unwrap(); same(&s, &again);
}

#[test]
fn empty_and_full_archives_have_honest_logits_and_cannot_gain_context_capacity() {
    let m = model(profile(2));
    let mut empty = m.sampled_session(9, start()).unwrap();
    let bytes = empty.checkpoint().unwrap().encode_archive(ArchiveLimits { tokens: 0, cache_values: 0,
        ..ArchiveLimits::default() }).unwrap();
    let archive = decode(&bytes, &m).unwrap();
    let (copy, receipt) = archive.recompute(&m, 10, budget()).unwrap();
    assert_eq!(receipt.logits_compared, 0); assert_eq!(receipt.cache_values_compared, 0);
    assert_eq!(copy.logits(), Err(Error::Incomplete));
    empty.advance_forced(0, 1, budget().decoder).unwrap();
    empty.advance_sampled(1, budget()).unwrap();
    let bytes = empty.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let (mut full, _) = decode(&bytes, &m).unwrap().recompute(&m, 10, budget()).unwrap();
    same(&empty, &full);
    let before = full.sampler_state();
    assert_eq!(full.advance_sampled(2, budget()).unwrap_err(), Error::Limit);
    assert_eq!(full.sampler_state(), before);
    for stream in [0, 9] { assert_eq!(decode(&bytes, &m).unwrap().recompute(&m, stream, budget()).unwrap_err(), Error::InvalidInput); }
}

#[test]
fn intended_profile_policy_and_numerically_different_weights_are_not_substitutable() {
    let (m, s) = populated(8); let bytes = s.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let different_context = model(profile(9));
    assert_eq!(decode(&bytes, &different_context).unwrap_err(), Error::Binding);
    let policy = SamplingPolicy::new(4, 3, 6, 0.8, 5, 0.95).unwrap();
    assert_eq!(SampledArchive::decode(&bytes, &m, &policy, ArchiveLimits::default()).unwrap_err(), Error::Binding);
    let p = profile(8); let shape = p.shape();
    let wrong = DecoderModel::new(p.clone(), values(shape.vocabulary * shape.hidden, 11), layers(&p),
        vec![1.0; shape.hidden], vec![0.0; shape.vocabulary * shape.hidden]).unwrap();
    let archive = decode(&bytes, &wrong).unwrap();
    assert_eq!(archive.recompute(&wrong, 10, budget()).unwrap_err(), Error::Binding);
}

#[test]
fn wrong_cache_namespace_and_nonfinite_scalars_refuse_without_promoting_partial_state() {
    let (m, s) = populated(8); let bytes = s.checkpoint().unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let at = cache_at(&bytes);
    let mut changed = bytes.clone(); changed[at + 8] ^= 1;
    assert!(decode(&changed, &m).is_err());
    let mut changed = bytes.clone(); changed[at - 4..at].copy_from_slice(&f32::NAN.to_bits().to_be_bytes());
    assert!(decode(&changed, &m).is_err());
    let mut changed = bytes.clone(); let end = changed.len();
    changed[end - 4..].copy_from_slice(&f32::INFINITY.to_le_bytes());
    assert!(decode(&changed, &m).is_err());
    let mut changed = bytes.clone(); changed[320..324].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode(&changed, &m).is_err());
    assert!(decode(&bytes, &m).unwrap().recompute(&m, 10, budget()).is_ok());
}
