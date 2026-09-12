//! A complete indexed model, not a last-writer-wins merge of parameter files.
#[path = "support/weight_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::{WeightError, TensorIssue};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::shards::*;
use std::collections::BTreeMap;

fn partition(tensors: &[Tensor], count: usize) -> BTreeMap<String, Vec<Tensor>> {
    let mut groups: BTreeMap<String, Vec<Tensor>> = BTreeMap::new();
    for (i, tensor) in tensors.iter().enumerate() {
        groups.entry(format!("model-{:05}-of-{count:05}.safetensors", i % count + 1)).or_default().push(tensor.clone());
    }
    groups
}
fn index(groups: &BTreeMap<String, Vec<Tensor>>) -> Vec<u8> {
    let total: usize = groups.values().flatten().map(|tensor| tensor.bytes.len()).sum();
    let map = groups.iter().flat_map(|(file, tensors)| tensors.iter().map(move |tensor| {
        format!("\"{}\":\"{file}\"", tensor.name)
    })).collect::<Vec<_>>().join(",");
    format!("{{\"metadata\":{{\"total_size\":{total}}},\"weight_map\":{{{map}}}}}").into_bytes()
}
fn files(groups: &BTreeMap<String, Vec<Tensor>>) -> BTreeMap<String, Vec<u8>> {
    groups.iter().map(|(name, tensors)| (name.clone(), archive(tensors))).collect()
}
fn borrowed(sources: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, &[u8]> {
    sources.iter().map(|(name, bytes)| (name.clone(), bytes.as_slice())).collect()
}
fn budget(model: &DecoderModel, first: usize, count: usize) -> DecoderBudget {
    DecoderBudget { scalar_products: model.estimate(first, count).unwrap().scalar_products().unwrap() }
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|value| value.to_bits()).collect() }
fn compare(left: &DecoderModel, right: &DecoderModel) {
    let tokens = [0, 3, 1, 5];
    let mut a = left.recompute(9, &tokens, budget(left, 0, tokens.len())).unwrap();
    let mut b = right.recompute(9, &tokens, budget(right, 0, tokens.len())).unwrap();
    for _ in 0..8 {
        assert_eq!(bits(a.logits().unwrap()), bits(b.logits().unwrap()));
        let ac = a.cache_image().unwrap(); let bc = b.cache_image().unwrap();
        for id in left.cache_profile().layers().keys() {
            assert_eq!(ac.layer(*id).unwrap().encode().unwrap(), bc.layer(*id).unwrap().encode().unwrap());
        }
        let pos = a.position();
        assert_eq!(a.greedy_token().unwrap(), b.greedy_token().unwrap());
        a.advance_greedy(pos, budget(left, pos as usize, 1)).unwrap();
        b.advance_greedy(pos, budget(right, pos as usize, 1)).unwrap();
    }
}

#[test]
fn indexed_mixed_precision_shards_match_one_file_and_direct_model_inference() {
    let p = decoder::profile(16); let mut ts = tensors(&p);
    let norm = ts.iter_mut().find(|t| t.name == "model.norm.weight").unwrap();
    norm.bytes = norm.values().iter().flat_map(|v| ((v.to_bits() >> 16) as u16).to_le_bytes()).collect();
    norm.dtype = "BF16".into();
    let groups = partition(&ts, 3); let sources = files(&groups); let ix = index(&groups);
    let (loaded, receipt) = DecoderModel::from_safetensors_shards(p.clone(), &ix, &borrowed(&sources)).unwrap();
    let (single, _) = DecoderModel::from_safetensors(p.clone(), &archive(&ts)).unwrap();
    compare(&loaded, &single); compare(&loaded, &direct(p.clone(), &ts));
    assert_eq!(receipt.shards.len(), 3);
    assert_eq!(receipt.index_bytes, ix.len());
    assert_eq!(receipt.file_bytes, sources.values().map(Vec::len).sum::<usize>());
    assert_eq!(receipt.data_bytes, ts.iter().map(|t| t.bytes.len()).sum::<usize>());
    assert_eq!(receipt.normalized_bytes, p.parameter_count() * 4);
    assert_eq!(receipt.shards.values().map(|s| s.tensors.len()).sum::<usize>(), ts.len());
}

#[test]
fn each_tensor_can_live_in_its_own_shard_without_changing_execution() {
    let p = decoder::profile(16); let ts = tensors(&p); let groups = partition(&ts, ts.len());
    let sources = files(&groups);
    let (model, receipt) = DecoderModel::from_safetensors_shards(p.clone(), &index(&groups), &borrowed(&sources)).unwrap();
    assert_eq!(receipt.shards.len(), ts.len());
    compare(&model, &decoder::model(p));
}

#[test]
fn missing_or_unlisted_sources_never_reduce_the_required_inventory() {
    let p = decoder::profile(16); let groups = partition(&tensors(&p), 3); let ix = index(&groups);
    let sources = files(&groups);
    for name in sources.keys() {
        let mut missing = borrowed(&sources); missing.remove(name);
        assert_eq!(DecoderModel::from_safetensors_shards(p.clone(), &ix, &missing).unwrap_err(), WeightError::Inventory);
    }
    let mut extra = borrowed(&sources); extra.insert("extra.safetensors".into(), b"");
    assert_eq!(DecoderModel::from_safetensors_shards(p, &ix, &extra).unwrap_err(), WeightError::Inventory);
}

#[test]
fn misplaced_and_duplicated_parameters_cannot_override_the_index_owner() {
    let p = decoder::profile(16); let groups = partition(&tensors(&p), 3); let ix = index(&groups);
    let names: Vec<_> = groups.keys().cloned().collect();
    let mut wrong = groups.clone(); let moved = wrong.get_mut(&names[0]).unwrap().remove(0);
    wrong.get_mut(&names[1]).unwrap().push(moved);
    assert_eq!(DecoderModel::from_safetensors_shards(p.clone(), &ix, &borrowed(&files(&wrong))).unwrap_err(), WeightError::Inventory);
    let mut duplicate = groups.clone();
    duplicate.get_mut(&names[1]).unwrap().push(groups[&names[0]][0].clone());
    assert_eq!(DecoderModel::from_safetensors_shards(p, &ix, &borrowed(&files(&duplicate))).unwrap_err(), WeightError::Inventory);
}

#[test]
fn index_paths_duplicate_keys_and_false_total_size_refuse() {
    let p = decoder::profile(16); let groups = partition(&tensors(&p), 3); let sources = files(&groups);
    let text = String::from_utf8(index(&groups)).unwrap();
    let name = groups.keys().next().unwrap();
    for bad in ["../outside.safetensors", "/root/model.safetensors", "https://remote/model.safetensors", "C:model.safetensors", ".hidden.safetensors"] {
        let altered = text.replace(name, bad);
        assert_eq!(DecoderModel::from_safetensors_shards(p.clone(), altered.as_bytes(), &borrowed(&sources)).unwrap_err(), WeightError::Inventory);
    }
    let duplicate = text.replacen("\"metadata\":", "\"weight_map\":{},\"metadata\":", 1);
    assert_eq!(DecoderModel::from_safetensors_shards(p.clone(), duplicate.as_bytes(), &borrowed(&sources)).unwrap_err(), WeightError::Header);
    let total: usize = groups.values().flatten().map(|t| t.bytes.len()).sum();
    let wrong = text.replace(&format!("\"total_size\":{total}"), "\"total_size\":0");
    assert_eq!(DecoderModel::from_safetensors_shards(p, wrong.as_bytes(), &borrowed(&sources)).unwrap_err(), WeightError::Inventory);
}

#[test]
fn a_bad_last_shard_cannot_replace_or_damage_an_existing_parameter_owner() {
    let p = decoder::profile(16); let ts = tensors(&p); let original = decoder::model(p.clone());
    let run = original.recompute(1, &[0, 3], budget(&original, 0, 2)).unwrap();
    let before = bits(run.logits().unwrap());
    let mut groups = partition(&ts, 3); let ix = index(&groups);
    let last = groups.values_mut().next_back().unwrap(); let tensor = last.last_mut().unwrap();
    tensor.bytes[..4].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(matches!(DecoderModel::from_safetensors_shards(p.clone(), &ix, &borrowed(&files(&groups))),
        Err(WeightError::Tensor { issue: TensorIssue::NonFinite, .. })));
    assert_eq!(before, bits(run.logits().unwrap()));
    compare(&original, &decoder::model(p));
}

#[test]
fn index_limits_and_declared_total_do_not_grant_more_parameter_capacity() {
    let p = decoder::profile(16); let groups = partition(&tensors(&p), 3); let sources = files(&groups);
    let text = String::from_utf8(index(&groups)).unwrap();
    let total = p.parameter_count() * 4;
    let huge = text.replace(&format!("\"total_size\":{total}"), &format!("\"total_size\":{}", total + 1));
    assert_eq!(DecoderModel::from_safetensors_shards(p.clone(), huge.as_bytes(), &borrowed(&sources)).unwrap_err(), WeightError::Limit);
    let mut padded = index(&groups); padded.resize(MAX_WEIGHT_INDEX_BYTES, b' ');
    assert!(DecoderModel::from_safetensors_shards(p.clone(), &padded, &borrowed(&sources)).is_ok());
    padded.push(b' ');
    assert_eq!(DecoderModel::from_safetensors_shards(p, &padded, &borrowed(&sources)).unwrap_err(), WeightError::Limit);
}

#[test]
fn a_sharded_checkpoint_survives_source_loss_but_not_parameter_substitution() {
    let p = decoder::profile(16); let groups = partition(&tensors(&p), 3); let sources = files(&groups);
    let (loaded, _) = DecoderModel::from_safetensors_shards(p.clone(), &index(&groups), &borrowed(&sources)).unwrap();
    let run = loaded.recompute(1, &[0, 3], budget(&loaded, 0, 2)).unwrap();
    let checkpoint = run.checkpoint().unwrap();
    let restore = DecoderRestoreBudget { cache_values: checkpoint.cache().normalized_values() };
    let (separately_loaded, _) = DecoderModel::from_safetensors_shards(p, &index(&groups), &borrowed(&sources)).unwrap();
    assert!(separately_loaded.restore_checkpoint(&checkpoint, 2, restore).is_err());
    drop(sources); drop(groups); drop(run); drop(loaded);
    let (resumed, _) = checkpoint.model().restore_checkpoint(&checkpoint, 2, restore).unwrap();
    let original_tokens = checkpoint.model().recompute_checkpoint(&checkpoint, 3, budget(checkpoint.model(), 0, 2)).unwrap();
    assert_eq!(bits(resumed.logits().unwrap()), bits(original_tokens.logits().unwrap()));
}
