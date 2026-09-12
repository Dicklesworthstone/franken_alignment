use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderModel, DecoderRestoreBudget, MAX_DECODER_PRODUCTS,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::{
    TensorIssue, WeightError, MAX_WEIGHT_HEADER_BYTES,
};
use fa_reference::action::consequence::activation::tensor::ScalarEncoding;
use fa_reference::Error;

#[path = "support/pretrained_fixture.rs"]
mod fixture;

fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn compare(model: &DecoderModel) {
    let expected = fixture::model(16).recompute(19, &[1, 2, 0, 4], budget()).unwrap();
    let actual = model.recompute(19, &[1, 2, 0, 4], budget()).unwrap();
    assert_eq!(actual.logits().unwrap().iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        expected.logits().unwrap().iter().map(|x| x.to_bits()).collect::<Vec<_>>());
    assert_eq!(actual.cache_image().unwrap().encode().unwrap(), expected.cache_image().unwrap().encode().unwrap());
}

#[test]
fn complete_f32_weights_drive_the_original_decoder_without_transposes() {
    let bytes = fixture::encode(&fixture::tensors(false));
    let (model, receipt) = DecoderModel::from_safetensors(fixture::profile(16), &bytes).unwrap();
    compare(&model);
    assert_eq!(receipt.tensors.len(), 21);
    assert_eq!(receipt.normalized_bytes, 308 * 4);
    assert_eq!(receipt.data_bytes, receipt.normalized_bytes);
    assert_eq!(receipt.file_bytes, 8 + receipt.header_bytes + receipt.data_bytes);
    assert_eq!(receipt.tensors["model.layers.1.mlp.down_proj.weight"].shape, vec![4, 6]);
}

#[test]
fn official_library_fixture_and_independent_mixed_encoder_agree() {
    // Produced by safetensors.torch.save, not the parser under test. Its recorded
    // provenance explicitly identifies synthetic exact-eighth parameter values.
    let published = include_bytes!("fixtures/decoder_mixed.safetensors");
    let (model, receipt) = DecoderModel::from_safetensors(fixture::profile(16), published).unwrap();
    compare(&model);
    assert_eq!(receipt.header_bytes, 2032);
    assert_eq!(receipt.file_bytes, 2848);
    for encoding in [ScalarEncoding::Binary32, ScalarEncoding::Binary16, ScalarEncoding::BFloat16] {
        assert_eq!(receipt.tensors.values().filter(|tensor| tensor.encoding == encoding).count(), 7);
    }
    let bytes = fixture::encode(&fixture::tensors(true));
    let (other, _) = DecoderModel::from_safetensors(fixture::profile(16), &bytes).unwrap();
    compare(&other);
}

#[test]
fn header_order_data_order_and_string_metadata_do_not_change_meaning() {
    let mut tensors = fixture::tensors(true);
    tensors.reverse();
    let (header, data) = fixture::header_data(&tensors);
    let header = format!("{{\"__metadata__\":{{\"format\":\"pt\",\"note\":\"a \\u2603\"}},{}   ", &header[1..]);
    let (model, _) = DecoderModel::from_safetensors(fixture::profile(16), &fixture::frame(&header, &data)).unwrap();
    compare(&model);
}

#[test]
fn every_truncation_and_unindexed_trailing_byte_refuses() {
    let bytes = fixture::encode(&fixture::tensors(true));
    for length in 0..bytes.len() {
        assert!(DecoderModel::from_safetensors(fixture::profile(16), &bytes[..length]).is_err(), "prefix {length}");
    }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(DecoderModel::from_safetensors(fixture::profile(16), &trailing).is_err());
    DecoderModel::from_safetensors(fixture::profile(16), &bytes).unwrap();
}

#[test]
fn complete_inventory_rejects_missing_output_extra_bias_and_model_substitution() {
    let base = fixture::tensors(false);
    for removed in ["lm_head.weight", "model.norm.weight", "model.layers.1.self_attn.v_proj.weight"] {
        let mut tensors = base.clone(); tensors.retain(|row| row.name != removed);
        assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::encode(&tensors)), Err(WeightError::Inventory)));
    }
    let mut extra = base.clone(); let mut bias = extra[1].clone();
    bias.name = "model.layers.0.self_attn.q_proj.bias".to_owned(); extra.push(bias);
    assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::encode(&extra)), Err(WeightError::Inventory)));
    let mut renamed = base; renamed[0].name = "transformer.wte.weight".to_owned();
    assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::encode(&renamed)), Err(WeightError::Inventory)));
}

#[test]
fn equal_scalar_counts_cannot_hide_transposed_or_flattened_shapes() {
    let mut tensors = fixture::tensors(false);
    let row = tensors.iter_mut().find(|row| row.name == "model.layers.1.mlp.down_proj.weight").unwrap();
    row.shape = vec![6, 4];
    assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::encode(&tensors)),
        Err(WeightError::Tensor { issue: TensorIssue::Shape, .. })));
    row_shape_reset(&mut tensors);
    let row = tensors.iter_mut().find(|row| row.name == "model.layers.0.self_attn.q_proj.weight").unwrap();
    row.shape = vec![16];
    assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::encode(&tensors)),
        Err(WeightError::Tensor { issue: TensorIssue::Shape, .. })));
}
fn row_shape_reset(tensors: &mut [fixture::Tensor]) {
    tensors.iter_mut().find(|row| row.name == "model.layers.1.mlp.down_proj.weight").unwrap().shape = vec![4, 6];
}

#[test]
fn structural_preflight_precedes_even_the_first_nonfinite_scalar() {
    let mut tensors = fixture::tensors(false);
    tensors[0].bytes[..4].copy_from_slice(&f32::NAN.to_le_bytes());
    tensors.last_mut().unwrap().shape = vec![24];
    assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::encode(&tensors)),
        Err(WeightError::Tensor { issue: TensorIssue::Shape, .. })));
}

#[test]
fn offset_aliases_gaps_and_overflow_cannot_select_other_tensor_bytes() {
    let tensors = fixture::tensors(true);
    let (header, data) = fixture::header_data(&tensors);
    for invalid in ["[0,8]", "[100,108]", "[18446744073709551615,8]", "[96,103]", "[-1,7]"] {
        let changed = header.replace("[96,104]", invalid);
        assert_ne!(changed, header);
        assert!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::frame(&changed, &data)).is_err());
    }
    compare(&DecoderModel::from_safetensors(fixture::profile(16), &fixture::frame(&header, &data)).unwrap().0);
}

#[test]
fn duplicate_fields_nonstring_metadata_and_unknown_descriptors_refuse() {
    let (header, data) = fixture::header_data(&fixture::tensors(false));
    let invalid = [
        format!("{{\"__metadata__\":{{\"unsafe\":true}},{}", &header[1..]),
        header.replacen("\"dtype\":\"F32\"", "\"dtype\":\"F32\",\"dtype\":\"F16\"", 1),
        header.replacen("\"dtype\":\"F32\"", "\"dtype\":\"F32\",\"extra\":0", 1),
        header.replacen("\"dtype\":\"F32\"", "\"dtype\":\"I32\"", 1),
        format!(" {header}"),
    ];
    for changed in invalid {
        assert!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::frame(&changed, &data)).is_err());
    }
}

#[test]
fn nonfinite_last_parameter_returns_no_partial_model() {
    let original = fixture::model(16);
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut tensors = fixture::tensors(false);
        let last = &mut tensors.last_mut().unwrap().bytes;
        let offset = last.len() - 4; last[offset..].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::encode(&tensors)),
            Err(WeightError::Tensor { issue: TensorIssue::NonFinite, .. })));
        compare(&original);
    }
}

#[test]
fn header_length_and_shape_limits_refuse_before_payload_allocation() {
    for declared in [u64::MAX, MAX_WEIGHT_HEADER_BYTES as u64 + 1] {
        assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &declared.to_le_bytes()), Err(WeightError::Limit)));
    }
    let mut tensors = fixture::tensors(false); tensors[0].shape[0] = usize::MAX;
    assert!(matches!(DecoderModel::from_safetensors(fixture::profile(16), &fixture::encode(&tensors)),
        Err(WeightError::Tensor { issue: TensorIssue::Shape, .. })));
}

#[test]
fn imported_parameters_survive_source_drop_and_use_original_restart_identity() {
    let bytes = fixture::encode(&fixture::tensors(true));
    let (model, _) = DecoderModel::from_safetensors(fixture::profile(16), &bytes).unwrap();
    drop(bytes);
    let mut original = model.recompute(1, &[1, 2, 0], budget()).unwrap();
    let checkpoint = original.checkpoint().unwrap();
    let (mut restored, _) = model.clone().restore_checkpoint(&checkpoint, 2,
        DecoderRestoreBudget { cache_values: checkpoint.cache().normalized_values() }).unwrap();
    for _ in 0..8 {
        let a = original.advance_greedy(original.position(), budget()).unwrap();
        let b = restored.advance_greedy(restored.position(), budget()).unwrap();
        assert_eq!(a.token, b.token);
        assert!(a.logits.iter().zip(b.logits.iter()).all(|(a,b)| a.to_bits() == b.to_bits()));
    }
    assert!(matches!(fixture::model(16).restore_checkpoint(&checkpoint, 3,
        DecoderRestoreBudget { cache_values: checkpoint.cache().normalized_values() }), Err(Error::Binding)));
}
