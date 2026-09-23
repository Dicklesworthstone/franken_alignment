//! Rebase regressions: preserve the intervening explicit tied-head contract.
use super::*;

fn tied(root: &mut Directory, stored_head: bool) {
    let text = String::from_utf8(root.fixture.configuration.clone()).unwrap();
    root.fixture.configuration = format!("{},\"tie_word_embeddings\":true}}", text.strip_suffix('}').unwrap()).into_bytes();
    fs::write(&root.assets[0], &root.fixture.configuration).unwrap();
    let embeddings = &root.fixture.tensors[0].2;
    let mut data = Vec::new(); let mut entries = Vec::new(); let mut mapping = Vec::new();
    for (index, (name, shape, values)) in root.fixture.tensors.iter().enumerate() {
        if name == "lm_head.weight" && !stored_head { continue; }
        let label = if index % 2 == 0 { "first.safetensors" } else { "second.safetensors" };
        mapping.push(format!("\"{name}\":\"{label}\""));
        if index % 2 != 0 { continue; }
        let values = if name == "lm_head.weight" { embeddings } else { values };
        let start = data.len();
        for value in values { data.extend_from_slice(&value.to_le_bytes()); }
        let dimensions = shape.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
        entries.push(format!("\"{name}\":{{\"dtype\":\"F32\",\"shape\":[{dimensions}],\"data_offsets\":[{start},{}]}}", data.len()));
    }
    let mut header = format!("{{{}}}", entries.join(",")).into_bytes();
    while !header.len().is_multiple_of(8) { header.push(b' '); }
    let mut body = (header.len() as u64).to_le_bytes().to_vec();
    body.extend_from_slice(&header); body.extend_from_slice(&data);
    fs::write(&root.paths["first.safetensors"], &body).unwrap();
    root.bodies.insert("first.safetensors".to_owned(), body);
    root.index = format!("{{\"weight_map\":{{{}}}}}", mapping.join(",")).into_bytes();
    fs::write(&root.assets[4], &root.index).unwrap();
}

#[test]
fn shard_files_tied_config_preserves_omitted_and_redundant_heads_without_guessing() {
    for stored in [false, true] {
        let mut root = Directory::new(false); tied(&mut root, stored);
        let (mut loaded, receipt) = NativeEvaluator::from_llama_shard_files(
            root.request(), &mut assets(), &mut budget()).unwrap();
        let (mut direct, expected) = NativeEvaluator::read_llama_checkpoint_shards(
            root.fixture.bootstrap(), &root.index, &mut root.readers(), &mut budget()).unwrap();
        assert_eq!(receipt, expected);
        assert_eq!(receipt.configuration.output_head(), OutputHead::TiedEmbeddings);
        assert_eq!(receipt.weights.shards["first.safetensors"].tensors.contains_key("lm_head.weight"), stored);
        assert_eq!(loaded.position(), 0); assert_eq!(loaded.sampled_draws(), 0);
        let original = input(b"?");
        loaded.begin(&original).unwrap(); direct.begin(&original).unwrap();
        assert_eq!(loaded.advance(0), direct.advance(0));
        assert_eq!(loaded.work(), direct.work());
        assert_eq!(loaded.position(), 1); assert_eq!(loaded.sampled_draws(), 0);
        if !stored {
            let independent = String::from_utf8(root.fixture.configuration.clone()).unwrap()
                .replace("\"tie_word_embeddings\":true", "\"tie_word_embeddings\":false");
            fs::write(&root.assets[0], independent).unwrap();
            let mut usage = budget();
            assert!(NativeEvaluator::from_llama_shard_files(root.request(), &mut assets(), &mut usage).is_err());
            assert_eq!(usage.usage().read_calls, 0);
        }
    }
}

#[test]
fn shard_files_tied_heads_keep_actual_directory_limit_and_conflict_checks() {
    let mut root = Directory::new(false); tied(&mut root, false);
    for shortage in [0, 1] {
        let mut readers = root.readers(); let mut usage = budget();
        let result = DecoderModel::read_safetensors_shards_bounded(
            root.fixture.policy.decoder_profile.clone(), &root.index, &mut readers, &mut usage,
            root.weight_bytes() - shortage, OutputHead::TiedEmbeddings);
        if shortage == 0 { assert!(result.is_ok()); }
        else {
            assert!(matches!(result, Err(WeightReadError::Refused(WeightError::Limit))));
            for (label, reader) in &readers {
                assert_eq!(reader.position(), header_bytes(&root.bodies[label]) as u64);
            }
        }
    }
    let mut conflict = Directory::new(false);
    let original = conflict.bodies["first.safetensors"].clone();
    tied(&mut conflict, true);
    assert!(NativeEvaluator::from_llama_shard_files(conflict.request(), &mut assets(), &mut budget()).is_ok());
    // Restore the original DIFFERENT dense head while retaining explicit tying.
    fs::write(&conflict.paths["first.safetensors"], &original).unwrap();
    let mut usage = budget();
    assert!(NativeEvaluator::from_llama_shard_files(conflict.request(), &mut assets(), &mut usage).is_err());
    assert_eq!(usage.usage().bytes_read, original.len() + conflict.bodies["second.safetensors"].len());
}
