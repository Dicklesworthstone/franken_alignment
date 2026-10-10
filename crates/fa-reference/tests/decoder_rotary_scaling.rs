//! Original inference, interchange and restart under immutable static RoPE.
//! Scalar frequency expectations are independent of the production dispatcher.
#[path = "support/rotary_fixture.rs"]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderModel, DecoderRestoreBudget, RotaryScaling,
    safetensors::{pretrained::LlamaConfig, reader::WeightReadBudget},
    sampling::{SampledSession, archive::{ArchiveLimits, SampledArchive}},
};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, TokenBytes, TokenizationBudget,
};
use fa_reference::Error;
use std::io::{self, Read};

const LINEAR: &str = r#", "rope_scaling":{"rope_type":"linear","factor":4}"#;
const LLAMA3: &str = r#", "rope_scaling":{"rope_type":"llama3","factor":8,"low_freq_factor":1,"high_freq_factor":4,"original_max_position_embeddings":128}"#;

fn scaling() -> [RotaryScaling; 2] {
    [RotaryScaling::linear(4.0).unwrap(), RotaryScaling::llama3(8.0, 1.0, 4.0, 128).unwrap()]
}
fn load(extra: &str) -> DecoderModel {
    DecoderModel::from_llama_safetensors(profile().identity(), 16, &config(extra), &weights()).unwrap().0
}

#[test]
fn imported_static_scaling_drives_actual_queries_and_changes_later_attention() {
    // Derived from the documented static definitions at transformers v4.50.0
    // modeling_rope_utils.py. The Llama3 table spans unchanged short wavelengths,
    // the interpolated band, and scaled long wavelengths; no engine helper is used.
    let inverse = [1.0_f64, 0.31622776601683794, 0.1, 0.03162277660168379,
        0.01, 0.0031622776601683794, 0.001, 0.00031622776601683794];
    let llama = [1.0_f64, 0.31622776601683794, 0.042751178754307596, 0.003952847075210474,
        0.00125, 0.0003952847075210474, 0.000125, 0.00003952847075210474];
    for (index, extra) in [LINEAR, LLAMA3].into_iter().enumerate() {
        let imported = load(extra);
        assert_eq!(imported.profile().rotary_scaling(), scaling()[index]);
        let mut actual = imported.session(9).unwrap();
        let mut original = model(RotaryScaling::None).session(9).unwrap();
        let mut different_logits = false;
        for (position, token) in [65, 66, 68, 67, 66, 65, 69, 66].into_iter().enumerate() {
            let step = actual.advance(position as u64, token, budget()).unwrap();
            let unscaled = original.advance(position as u64, token, budget()).unwrap();
            let query = values(step.layers[0].query.source());
            let unit = (1.0_f64 / (1.0_f64 + 1e-5).sqrt()) as f32;
            for pair in 0..8 {
                let frequency = if index == 0 { inverse[pair] / 4.0 } else { llama[pair] };
                let angle = position as f64 * frequency;
                let left = f64::from(sign(token, pair) * unit);
                let right = f64::from(sign(token, pair + 8) * unit);
                let expected = [(left * angle.cos() - right * angle.sin()) as f32,
                    (left * angle.sin() + right * angle.cos()) as f32];
                for (coordinate, value) in [(pair, expected[0]), (pair + 8, expected[1])] {
                    assert!((query[coordinate] - value).abs() <= 2e-7,
                        "mode={index} position={position} coordinate={coordinate}: {} != {value}", query[coordinate]);
                }
            }
            assert_eq!(step.work, unscaled.work);
            if position == 0 { assert_eq!(bits(&step.logits), bits(&unscaled.logits)); }
            else { different_logits |= bits(&step.logits) != bits(&unscaled.logits); }
        }
        assert!(different_logits, "scaling must affect real attention after position zero");
        assert_ne!(actual.cache_image().unwrap().encode().unwrap(), original.cache_image().unwrap().encode().unwrap());
        assert_eq!(actual.work(), original.work());
    }
}

#[test]
fn legacy_and_modern_configuration_bind_the_same_static_profile() {
    for (legacy, modern, expected) in [
        (LINEAR, r#", "rope_parameters":{"rope_type":"linear","rope_theta":10000,"factor":4}"#, scaling()[0].clone()),
        (LLAMA3, r#", "rope_parameters":{"rope_type":"llama3","rope_theta":10000,"factor":8,"low_freq_factor":1,"high_freq_factor":4,"original_max_position_embeddings":128}"#, scaling()[1].clone()),
    ] {
        let a = LlamaConfig::decode(profile().identity(), 16, &config(legacy)).unwrap();
        let b = LlamaConfig::decode(profile().identity(), 16, &config(modern)).unwrap();
        assert_eq!(a.profile(), b.profile()); assert_eq!(a.profile().rotary_scaling(), expected);
        let both = format!("{legacy}{modern}");
        assert_eq!(LlamaConfig::decode(profile().identity(), 16, &config(&both)).unwrap().profile(), a.profile());
        let contradictory = both.replacen("\"factor\":4", "\"factor\":5", 1)
            .replacen("\"factor\":8", "\"factor\":9", 1);
        assert!(LlamaConfig::decode(profile().identity(), 16, &config(&contradictory)).is_err());
    }
    let legacy_type = LINEAR.replace("rope_type", "type");
    assert_eq!(load(&legacy_type).profile(), load(LINEAR).profile());
    assert_eq!(load("").profile(), &profile());
}

struct NeverRead;
impl Read for NeverRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("invalid config read weights") }
}
#[test]
fn unsupported_or_malformed_scaling_refuses_before_weight_io() {
    for extra in [
        r#", "rope_scaling":{"rope_type":"dynamic","factor":4}"#,
        r#", "rope_scaling":{"rope_type":"yarn","factor":4}"#,
        r#", "rope_scaling":{"rope_type":"linear"}"#,
        r#", "rope_scaling":{"rope_type":"llama3","factor":8}"#,
        r#", "rope_scaling":{"rope_type":"linear","factor":0.5}"#,
        r#", "rope_scaling":{"rope_type":"linear","factor":1e999}"#,
        r#", "rope_scaling":{"rope_type":"linear","factor":4,"attention_factor":1}"#,
        r#", "rope_scaling":{"rope_type":"linear","factor":4,"type":"llama3"}"#,
        r#", "rope_scaling":{"rope_type":"llama3","factor":8,"low_freq_factor":4,"high_freq_factor":1,"original_max_position_embeddings":128}"#,
        r#", "rope_scaling":{"rope_type":"llama3","factor":8,"low_freq_factor":1,"high_freq_factor":4,"original_max_position_embeddings":0}"#,
        r#", "rope_parameters":{"rope_type":"linear","rope_theta":10000,"factor":4},"rope_theta":20000"#,
    ] {
        let mut io = WeightReadBudget::new(1024, 32).unwrap();
        assert!(DecoderModel::read_llama_safetensors(profile().identity(), 16,
            &config(extra), &mut NeverRead, &mut io).is_err(), "{extra}");
        assert_eq!(io.usage().bytes_read, 0);
    }
    for factor in [0.0, -0.0, 0.5, -1.0, f64::INFINITY, f64::NAN] {
        assert!(RotaryScaling::linear(factor).is_err());
    }
    assert!(RotaryScaling::llama3(8.0, 1.0, 1.0, 128).is_err());
}

fn same_sampled(a: &SampledSession, b: &SampledSession) {
    assert_eq!(a.tokens(), b.tokens()); assert_eq!(a.sampled_positions(), b.sampled_positions());
    assert_eq!(a.sampler_state(), b.sampler_state());
    assert_eq!(a.logits().map(bits), b.logits().map(bits));
}
#[test]
fn sampled_archives_bind_scaling_before_replay_and_preserve_rng_continuation() {
    for scale in scaling() {
        let m = model(scale.clone());
        let mut source = m.recompute_sampled(9, &[65, 66], budget(), start()).unwrap();
        source.advance_sampled(2, sample_budget()).unwrap();
        source.advance_forced(3, 68, budget()).unwrap();
        let checkpoint = source.checkpoint().unwrap();
        let bytes = checkpoint.encode_archive(ArchiveLimits::default()).unwrap();
        let fresh = model(scale);
        let archive = SampledArchive::decode(&bytes, &fresh, &start().policy, ArchiveLimits::default()).unwrap();
        let (mut restored, receipt) = archive.recompute(&fresh, 10, sample_budget()).unwrap();
        assert_eq!(receipt.tokens_compared, 4); assert_eq!(receipt.sampled_tokens_compared, 1);
        same_sampled(&source, &restored);
        for _ in 0..8 {
            let position = source.position();
            let a = source.advance_sampled(position, sample_budget()).unwrap();
            let b = restored.advance_sampled(position, sample_budget()).unwrap();
            assert_eq!(a.choice, b.choice); same_sampled(&source, &restored);
        }
        for foreign in [model(RotaryScaling::None), model(RotaryScaling::linear(5.0).unwrap()),
            model(RotaryScaling::llama3(8.0, 1.0, 4.0, 256).unwrap())] {
            assert!(SampledArchive::decode(&bytes, &foreign, &start().policy, ArchiveLimits::default()).is_err());
        }
        let exact = ArchiveLimits { bytes: bytes.len(), ..ArchiveLimits::default() };
        assert_eq!(checkpoint.encode_archive(exact).unwrap(), bytes);
        assert_eq!(checkpoint.encode_archive(ArchiveLimits { bytes: bytes.len() - 1, ..exact }), Err(Error::Limit));
        // Live checkpoints remain tied to their original immutable model owner.
        assert_eq!(fresh.restore_sampled_checkpoint(&checkpoint, 11,
            DecoderRestoreBudget { cache_values: checkpoint.numerical().cache().normalized_values() }).unwrap_err(), Error::Binding);
    }
}

#[test]
fn tokenization_archives_keep_original_bytes_and_reject_scaled_profile_substitution() {
    for scale in [RotaryScaling::None, scaling()[0].clone(), scaling()[1].clone()] {
        let p = profile().with_rotary_scaling(scale).unwrap();
        let original = ByteBpe::new(p.clone(), (0..=255).map(|b| TokenBytes::Content(vec![b])).collect(), Vec::new()).unwrap();
        let bytes = original.to_bytes().unwrap();
        let loaded = ByteBpe::from_bytes(&p, &bytes).unwrap();
        assert_eq!(loaded.to_bytes().unwrap(), bytes);
        assert_eq!(loaded.encode("Hello, λ!".as_bytes(), TokenizationBudget::default()).unwrap().tokens(),
            "Hello, λ!".as_bytes().iter().map(|b| u32::from(*b)).collect::<Vec<_>>());
        let wrong = profile().with_rotary_scaling(RotaryScaling::linear(5.0).unwrap()).unwrap();
        assert!(ByteBpe::from_bytes(&wrong, &bytes).is_err());
    }
}

#[test]
fn named_control_archives_retain_scaling_and_special_token_identity() {
    use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderProfile;
    use std::collections::BTreeMap;
    let base = profile(); let mut shape = base.shape(); shape.vocabulary = 257;
    for scale in [RotaryScaling::None, scaling()[0], scaling()[1]] {
        let p = DecoderProfile::new(base.identity(), shape, base.epsilon(), base.theta()).unwrap()
            .with_rotary_scaling(scale).unwrap();
        let mut vocabulary: Vec<_> = (0..=255).map(|b| TokenBytes::Content(vec![b])).collect();
        vocabulary.push(TokenBytes::Control);
        let original = ByteBpe::new_with_special_tokens(p.clone(), vocabulary, Vec::new(),
            BTreeMap::from([(256, b"<|end|>".to_vec())])).unwrap();
        let bytes = original.to_bytes().unwrap();
        let loaded = ByteBpe::from_bytes(&p, &bytes).unwrap();
        assert_eq!(loaded.to_bytes().unwrap(), bytes);
        assert_eq!(loaded.special_tokens(), original.special_tokens());
        let foreign = p.with_rotary_scaling(RotaryScaling::linear(5.0).unwrap()).unwrap();
        assert!(ByteBpe::from_bytes(&foreign, &bytes).is_err());
    }
}

#[path = "decoder_rotary_scaling/bindings.rs"]
mod bindings;
