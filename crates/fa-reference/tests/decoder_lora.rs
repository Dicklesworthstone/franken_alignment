//! Static adapter ingestion through original decoding, capture and checkpoint APIs.
#![forbid(unsafe_code)]
#[path = "decoder_lora/fixture.rs"]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderModel, DecoderRestoreBudget,
    safetensors::{lora::{LoraError, LoraMergeBudget, MAX_LORA_PRODUCTS},
        reader::WeightReadBudget},
};
use fa_reference::Error;
use std::io::{self, Cursor, Read};

fn adapt(base: &DecoderModel, rank: usize, zero: bool, targets: &[&str]) -> DecoderModel {
    base.with_lora_safetensors(identity(), &config(rank, targets), &encode(&adapter(rank, zero, targets)),
        &mut LoraMergeBudget::new(MAX_LORA_PRODUCTS).unwrap()).unwrap().0
}
fn same_trace(a: &DecoderModel, b: &DecoderModel) {
    let mut a = a.session(9).unwrap(); let mut b = b.session(9).unwrap();
    for (position, token) in [65, 66, 68, 67, 69, 66].into_iter().enumerate() {
        let a = a.advance(position as u64, token, budget()).unwrap();
        let b = b.advance(position as u64, token, budget()).unwrap();
        assert_eq!(bits(&a.logits), bits(&b.logits)); assert_eq!(a.work, b.work);
        for (a, b) in a.layers.iter().zip(&b.layers) {
            assert_eq!(bits(&values(a.query.source())), bits(&values(b.query.source())));
            assert_eq!(bits(&values(a.residual.source())), bits(&values(b.residual.source())));
        }
    }
}
#[test]
fn rank_one_and_two_all_layer_adapters_match_independent_outer_product_oracle() {
    for rank in [1, 2] {
        let base = base(); let rows = adapter(rank, false, &TARGETS);
        let expected_rows = oracle(&base_tensors(), &rows, rank); let expected = dense(&expected_rows);
        let mut products = LoraMergeBudget::new(240 * rank as u64).unwrap();
        let (actual, receipt) = base.with_lora_safetensors(identity(), &config(rank, &TARGETS),
            &encode(&mixed(&rows)), &mut products).unwrap();
        assert_eq!(receipt.work.updated_parameters, 240);
        assert_eq!(receipt.work.adapter_parameters, 116 * rank);
        assert_eq!(receipt.work.scalar_products, 240 * rank as u64);
        assert_eq!(products.remaining_products(), 0);
        assert_eq!(receipt.base_profile, profile());
        assert_eq!(actual.profile().identity(), identity());
        assert_eq!(actual.profile().shape(), base.profile().shape());
        assert_eq!(actual.profile().rotary_scaling(), base.profile().rotary_scaling());
        assert_eq!(actual.profile().epsilon().to_bits(), base.profile().epsilon().to_bits());
        assert_eq!(actual.profile().theta().to_bits(), base.profile().theta().to_bits());
        same_trace(&actual, &expected);
        same_trace(&base, &fixture::base());

        let token = 65;
        let first = actual.session(10).unwrap().advance(0, token, budget()).unwrap();
        let untouched = base.session(10).unwrap().advance(0, token, budget()).unwrap();
        assert_ne!(bits(&first.logits), bits(&untouched.logits), "nonzero adapter must change actual execution");
        // Independent scalar Q oracle at position zero, including original RMS
        // and rounded hidden input. Later positions above exercise preserved RoPE.
        let embeddings = words(expected_rows.iter().find(|r| r.name == "model.embed_tokens.weight").unwrap());
        let q = words(expected_rows.iter().find(|r| r.name == "model.layers.0.self_attn.q_proj.weight").unwrap());
        let hidden = &embeddings[token as usize * 4..token as usize * 4 + 4];
        let variance = hidden.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>() / 4.0;
        let normalized = hidden.iter().map(|x| (f64::from(*x) / (variance + 1e-5).sqrt()) as f32).collect::<Vec<_>>();
        let expected = (0..4).map(|row| (0..4).map(|column|
            f64::from(q[row * 4 + column]) * f64::from(normalized[column])).sum::<f64>() as f32).collect::<Vec<_>>();
        let query = values(first.layers[0].query.source());
        for (actual, expected) in query.iter().zip(&expected) { assert!((actual - expected).abs() <= 2e-7); }
    }
}

#[test]
fn zero_delta_changes_coordinate_identity_and_cannot_reuse_old_probes_or_checkpoints() {
    let base = base(); let adapted = adapt(&base, 2, true, &TARGETS);
    same_trace(&base, &adapted);
    assert_ne!(base.cache_profile(), adapted.cache_profile());
    let mut old = base.session(9).unwrap();
    let source = old.advance(0, 65, budget()).unwrap();
    let changed = adapted.session(10).unwrap().advance(0, 65, budget()).unwrap();
    let old_probe = LinearProbe::new(1, 1, base.residual_contract(1).unwrap().profile(),
        &[0.0; 4], 0.0, 1.0).unwrap();
    assert!(old_probe.evaluate(&frame(source.layers[0].residual.source())).is_ok());
    assert_eq!(old_probe.evaluate(&frame(changed.layers[0].residual.source())), Err(Error::Binding));
    let checkpoint = old.checkpoint().unwrap();
    let restore = DecoderRestoreBudget { cache_values: checkpoint.cache().normalized_values() };
    assert!(base.restore_checkpoint(&checkpoint, 11, restore).is_ok());
    assert_eq!(adapted.restore_checkpoint(&checkpoint, 11, restore).unwrap_err(), Error::Binding);
}

struct NeverRead;
impl Read for NeverRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("early refusal touched adapter bytes") }
}
fn no_read(base: &DecoderModel, id: DecoderIdentity, config: &[u8], allowance: u64) -> LoraError {
    let mut reads = WeightReadBudget::new(100_000, 100).unwrap();
    let mut products = LoraMergeBudget::new(allowance).unwrap();
    let error = base.read_lora_safetensors(id, config, &mut NeverRead, &mut reads, &mut products).unwrap_err();
    assert_eq!(reads.usage().read_calls, 0); assert_eq!(reads.usage().bytes_read, 0);
    assert_eq!(products.reserved_products(), 0); error
}
#[test]
fn identity_configuration_and_aggregate_work_refuse_before_any_adapter_read() {
    let base = base(); let valid = config(2, &TARGETS);
    let old = profile().identity(); let new = identity();
    for wrong in [
        old,
        DecoderIdentity { tenant: 9, ..new },
        DecoderIdentity { model: 9, ..new },
        DecoderIdentity { tokenizer_generation: 9, ..new },
        DecoderIdentity { model_generation: old.model_generation, ..new },
        DecoderIdentity { profile_generation: old.profile_generation, ..new },
    ] { assert_eq!(no_read(&base, wrong, &valid, 480), LoraError::Identity); }
    assert_eq!(no_read(&base, new, &valid, 479), LoraError::Limit);
    let minimal = String::from_utf8(config(1, &["q_proj"])).unwrap();
    for extra in [
        r#","fan_in_fan_out":true"#, r#","use_dora":true"#, r#","use_rslora":true"#,
        r#","lora_bias":true"#, r#","bias":"all""#, r#","modules_to_save":["lm_head"]"#,
        r#","rank_pattern":{"q_proj":2}"#, r#","alpha_pattern":{"q_proj":8}"#,
        r#","layers_to_transform":[0]"#, r#","layer_replication":[[0,1]]"#,
        r#","exclude_modules":["k_proj"]"#, r#","auto_mapping":{"base_model_class":"untrusted"}"#,
        r#","init_lora_weights":"pissa""#, r#","init_lora_weights":"olora""#,
        r#","init_lora_weights":"loftq""#, r#","init_lora_weights":"eva""#,
        r#","unknown_behavior":true"#,
    ] {
        let malformed = format!("{}{extra}}}", minimal.strip_suffix('}').unwrap());
        assert!(matches!(no_read(&base, new, malformed.as_bytes(), 480), LoraError::Configuration { .. }), "{extra}");
    }
    for malformed in [
        minimal.replace("\"r\":1", "\"r\":0"),
        minimal.replace("\"r\":1", "\"r\":65"),
        minimal.replace("\"lora_alpha\":4", "\"lora_alpha\":0"),
        minimal.replace("\"lora_alpha\":4", "\"lora_alpha\":1e999"),
        minimal.replace("\"inference_mode\":true", "\"inference_mode\":false"),
        minimal.replace("[\"q_proj\"]", "[\"q_proj\",\"q_proj\"]"),
        minimal.replace("[\"q_proj\"]", "\"all-linear\""),
        minimal.replace("[\"q_proj\"]", "[\"lm_head\"]"),
    ] { let _ = no_read(&base, new, malformed.as_bytes(), 480); }
}

#[test]
fn malformed_truncated_trailing_and_nonfinite_adapters_retain_original_read_usage() {
    let base = base(); let rows = adapter(1, false, &TARGETS); let good = encode(&rows);
    let mut malformed = vec![good[..good.len() - 1].to_vec()];
    let mut trailing = good.clone(); trailing.push(0); malformed.push(trailing);
    let mut nonfinite = rows.clone(); nonfinite[0].bytes[..4].copy_from_slice(&f32::INFINITY.to_le_bytes());
    malformed.push(encode(&nonfinite));
    let mut missing = rows.clone(); missing.pop(); malformed.push(encode(&missing));
    let mut runtime = rows.clone(); runtime[0].name = runtime[0].name.replace(".lora_A.weight", ".lora_A.default.weight");
    malformed.push(encode(&runtime));
    let mut wrong = rows.clone(); wrong[0].shape.swap(0, 1); malformed.push(encode(&wrong));
    for bytes in malformed {
        let mut reads = WeightReadBudget::new(good.len() + 4096, 4096).unwrap();
        let mut products = LoraMergeBudget::new(240).unwrap();
        assert!(matches!(base.read_lora_safetensors(identity(), &config(1, &TARGETS),
            &mut Cursor::new(bytes), &mut reads, &mut products), Err(LoraError::Weights(_))));
        assert!(reads.usage().bytes_read > 0); assert!(reads.usage().read_calls > 0);
        assert_eq!(products.reserved_products(), 0);
    }
    let mut reads = WeightReadBudget::new(good.len(), 4096).unwrap();
    let mut products = LoraMergeBudget::new(240).unwrap();
    assert!(base.read_lora_safetensors(identity(), &config(1, &TARGETS),
        &mut Cursor::new(&good), &mut reads, &mut products).is_err(), "EOF needs remaining read capacity");
    assert!(reads.usage().bytes_read > 0);
    let mut reads = WeightReadBudget::new(good.len() + 1, 4096).unwrap();
    assert!(base.read_lora_safetensors(identity(), &config(1, &TARGETS),
        &mut Cursor::new(&good), &mut reads, &mut products).is_ok());
    assert_eq!(reads.usage().bytes_read, good.len());
}

#[test]
fn late_merge_overflow_spends_the_full_product_reservation_and_cannot_retry_free() {
    let base = base(); let mut rows = adapter(1, false, &["q_proj"]);
    for row in &mut rows {
        for value in row.bytes.chunks_exact_mut(4) { value.copy_from_slice(&f32::MAX.to_le_bytes()); }
    }
    let bytes = encode(&rows);
    let mut reads = WeightReadBudget::new(bytes.len() + 1000, 4096).unwrap();
    let mut products = LoraMergeBudget::new(32).unwrap();
    let error = base.read_lora_safetensors(identity(), &config(1, &["q_proj"]),
        &mut Cursor::new(&bytes), &mut reads, &mut products).unwrap_err();
    assert_eq!(error, LoraError::Model(Error::Overflow));
    assert_eq!(reads.usage().bytes_read, bytes.len()); assert_eq!(products.reserved_products(), 32);
    let spent = reads.usage();
    assert_eq!(base.read_lora_safetensors(identity(), &config(1, &["q_proj"]),
        &mut NeverRead, &mut reads, &mut products).unwrap_err(), LoraError::Limit);
    assert_eq!(reads.usage(), spent); same_trace(&base, &fixture::base());
}

#[path = "decoder_lora/replay.rs"]
mod replay;
