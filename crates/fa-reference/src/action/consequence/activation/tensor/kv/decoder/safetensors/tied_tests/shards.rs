//! Whole-checkpoint sharing does not weaken any physical shard assignment.
use super::*;
use super::super::shards::ShardedWeightLoadReceipt;
use crate::action::consequence::activation::tensor::kv::decoder::DecoderRestoreBudget;
use std::collections::BTreeMap;

type Files = BTreeMap<String, Vec<u8>>;
fn bundle(values: &[Raw]) -> (Vec<u8>, Files) {
    let mut groups: BTreeMap<&str, Vec<Raw>> = BTreeMap::new();
    for value in values {
        let shard = match value.name.as_str() {
            EMBEDDINGS => "a.safetensors", OUTPUT_HEAD => "c.safetensors", _ => "b.safetensors",
        };
        groups.entry(shard).or_default().push(value.clone());
    }
    let entries = groups.iter().flat_map(|(file, tensors)| tensors.iter()
        .map(move |tensor| format!("\"{}\":\"{file}\"", tensor.name))).collect::<Vec<_>>().join(",");
    let total: usize = values.iter().map(|tensor| tensor.bytes.len()).sum();
    let index = format!("{{\"metadata\":{{\"total_size\":{total}}},\"weight_map\":{{{entries}}}}}").into_bytes();
    (index, groups.into_iter().map(|(file, tensors)| (file.to_owned(), archive(&tensors))).collect())
}
fn views(files: &Files) -> BTreeMap<String, &[u8]> {
    files.iter().map(|(file, bytes)| (file.clone(), bytes.as_slice())).collect()
}
fn readers(files: &Files) -> BTreeMap<String, Cursor<Vec<u8>>> {
    files.iter().map(|(file, bytes)| (file.clone(), Cursor::new(bytes.clone()))).collect()
}
fn size(files: &Files) -> usize { files.values().map(Vec::len).sum() }
fn header_end(bytes: &[u8]) -> u64 { 8 + u64::from_le_bytes(bytes[..8].try_into().unwrap()) }
fn memory(index: &[u8], files: &Files) -> Result<(DecoderModel, pretrained::PretrainedShardReceipt), CheckpointError> {
    DecoderModel::from_llama_shards(profile().identity(), 8, &config(Some("true")), index, &views(files))
}
fn stream(index: &[u8], files: &Files, budget: &mut WeightReadBudget)
    -> Result<(DecoderModel, ShardedWeightLoadReceipt), WeightReadError>
{
    DecoderModel::read_safetensors_shards_with_output_head(profile(), index, &mut readers(files), budget, OutputHead::TiedEmbeddings)
}

#[test]
fn tied_sharded_memory_and_streaming_keep_actual_receipts_and_native_execution() {
    for stored in [false, true] {
        let (index, files) = bundle(&tensors(stored));
        let (model, expected) = memory(&index, &files).unwrap(); compare(&model);
        let mut budget = allowance(size(&files) + 1);
        let (model, receipt) = DecoderModel::read_llama_shards(profile().identity(), 8,
            &config(Some("true")), &index, &mut readers(&files), &mut budget).unwrap();
        assert_eq!(receipt, expected); compare(&model);
        assert_eq!(receipt.configuration.output_head(), OutputHead::TiedEmbeddings);
        assert_eq!(receipt.weights.shards.len(), if stored { 3 } else { 2 });
        assert_eq!(receipt.weights.data_bytes, if stored { 184 } else { 160 });
        assert_eq!(receipt.weights.normalized_bytes, 184);
        assert_eq!(receipt.weights.file_bytes, size(&files));
        assert_eq!(budget.usage().bytes_read, size(&files));
        assert_eq!(receipt.weights.shards.values().any(|shard| shard.tensors.contains_key(OUTPUT_HEAD)), stored);
    }
}

#[test]
fn tied_index_declared_head_stays_mandatory_and_all_headers_precede_scalars() {
    let (index, files) = bundle(&tensors(true));
    let mut missing = files.clone(); missing.insert("c.safetensors".into(), archive(&[]));
    let mut sources = readers(&missing); let mut budget = allowance(size(&files) + 1);
    assert_eq!(DecoderModel::read_safetensors_shards_with_output_head(profile(), &index,
        &mut sources, &mut budget, OutputHead::TiedEmbeddings).unwrap_err(), WeightReadError::Refused(WeightError::Inventory));
    // Earlier valid shards still have not supplied any tensor-body bytes.
    for (file, source) in &sources { assert_eq!(source.position(), header_end(&missing[file])); }
    assert_eq!(memory(&index, &missing).unwrap_err(), CheckpointError::Weights(WeightError::Inventory));
    // Moving a declared tensor to another otherwise valid file cannot repair it.
    let all = tensors(true);
    let mut misplaced = missing;
    misplaced.insert("b.safetensors".into(), archive(&all.into_iter().filter(|row| row.name != EMBEDDINGS).collect::<Vec<_>>()));
    assert_eq!(memory(&index, &misplaced).unwrap_err(), CheckpointError::Weights(WeightError::Inventory));
    let mut sources = readers(&misplaced); let mut budget = allowance(size(&misplaced) + 1);
    assert!(DecoderModel::read_safetensors_shards_with_output_head(profile(), &index,
        &mut sources, &mut budget, OutputHead::TiedEmbeddings).is_err());
    for (file, source) in &sources { assert!(source.position() <= header_end(&misplaced[file])); }
    compare(&memory(&index, &files).unwrap().0);
}

#[test]
fn tied_shard_index_and_source_set_refuse_before_any_reader_call() {
    struct Never;
    impl Read for Never {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("unadmitted shard set reached I/O") }
    }
    let (index, files) = bundle(&tensors(false));
    let missing_embedding = String::from_utf8(index.clone()).unwrap()
        .replace("\"model.embed_tokens.weight\":\"a.safetensors\",", "");
    assert_ne!(missing_embedding.as_bytes(), index);
    let traversal = String::from_utf8(index.clone()).unwrap().replace("a.safetensors", "../a.safetensors");
    for document in [missing_embedding.as_bytes(), traversal.as_bytes()] {
        let mut sources: BTreeMap<_, _> = files.keys().map(|file| (file.clone(), Never)).collect();
        let mut budget = allowance(size(&files) + 1);
        assert!(DecoderModel::read_safetensors_shards_with_output_head(profile(), document,
            &mut sources, &mut budget, OutputHead::TiedEmbeddings).is_err());
        assert_eq!(budget.usage().read_calls, 0);
    }
    for independent in [false, true] {
        let mut sources: BTreeMap<_, _> = files.keys().map(|file| (file.clone(), Never)).collect();
        if !independent { sources.insert("extra.safetensors".into(), Never); }
        let mut budget = allowance(size(&files) + 1);
        let mode = if independent { OutputHead::Independent } else { OutputHead::TiedEmbeddings };
        assert_eq!(DecoderModel::read_safetensors_shards_with_output_head(profile(), &index,
            &mut sources, &mut budget, mode).unwrap_err(), WeightReadError::Refused(WeightError::Inventory));
        assert_eq!(budget.usage().read_calls, 0);
    }
    assert_eq!(DecoderModel::from_safetensors_shards(profile(), &index, &views(&files)).unwrap_err(), WeightError::Inventory);
    compare(&memory(&index, &files).unwrap().0);
}

#[test]
fn tied_shard_redundant_head_is_compared_across_physical_files() {
    for word in [2.0_f32, -0.0_f32] {
        let mut values = tensors(true);
        values.iter_mut().find(|value| value.name == OUTPUT_HEAD).unwrap().bytes[4..8]
            .copy_from_slice(&word.to_le_bytes());
        let (index, files) = bundle(&values);
        assert!(DecoderModel::from_safetensors_shards(profile(), &index, &views(&files)).is_ok());
        assert_eq!(memory(&index, &files).unwrap_err(), CheckpointError::Weights(issue(OUTPUT_HEAD, TensorIssue::TiedValues)));
        let mut budget = allowance(size(&files) + 1);
        assert_eq!(stream(&index, &files, &mut budget).unwrap_err(), WeightReadError::Refused(issue(OUTPUT_HEAD, TensorIssue::TiedValues)));
        assert_eq!(budget.usage().bytes_read, size(&files));
    }
    let mut values = tensors(true);
    let head = values.iter_mut().find(|row| row.name == OUTPUT_HEAD).unwrap();
    head.dtype = "BF16";
    head.bytes = [0x3f80_u16, 0, 0, 0x3f80, 0xbf80, 0x3f00].iter().flat_map(|word| word.to_le_bytes()).collect();
    let (index, files) = bundle(&values);
    let (model, receipt) = memory(&index, &files).unwrap(); compare(&model);
    let (streamed, other) = stream(&index, &files, &mut allowance(size(&files) + 1)).unwrap(); compare(&streamed);
    assert_eq!(other, receipt.weights); assert_eq!(other.data_bytes, 172);
}

#[test]
fn tied_shard_total_size_and_whole_budget_refuse_before_tensor_bodies() {
    let (index, files) = bundle(&tensors(false));
    let mut sources = readers(&files); let mut short = allowance(size(&files));
    assert_eq!(DecoderModel::read_safetensors_shards_with_output_head(profile(), &index,
        &mut sources, &mut short, OutputHead::TiedEmbeddings).unwrap_err(), WeightReadError::Refused(WeightError::Limit));
    for (file, source) in &sources { assert_eq!(source.position(), header_end(&files[file])); }
    let wrong = String::from_utf8(index.clone()).unwrap().replace("\"total_size\":160", "\"total_size\":161");
    assert_ne!(wrong.as_bytes(), index);
    let mut sources = readers(&files); let mut budget = allowance(size(&files) + 1);
    assert_eq!(DecoderModel::read_safetensors_shards_with_output_head(profile(), wrong.as_bytes(),
        &mut sources, &mut budget, OutputHead::TiedEmbeddings).unwrap_err(), WeightReadError::Refused(WeightError::Inventory));
    for (file, source) in &sources { assert_eq!(source.position(), header_end(&files[file])); }
    let used = budget.usage(); assert!(used.bytes_read > 0);
    // Repositioning sources does not refund the failed import's aggregate budget.
    assert!(stream(&index, &files, &mut budget).is_err());
    assert!(budget.usage().bytes_read >= used.bytes_read);
    let (model, _) = stream(&index, &files, &mut allowance(size(&files) + 1)).unwrap(); compare(&model);
}

#[test]
fn tied_shards_require_each_complete_body_and_real_eof() {
    let (index, files) = bundle(&tensors(true));
    for selected in files.keys() {
        let mut truncated = files.clone(); truncated.get_mut(selected).unwrap().pop();
        assert!(memory(&index, &truncated).is_err());
        assert!(stream(&index, &truncated, &mut allowance(size(&files) + 1)).is_err());
        let mut trailing = files.clone(); trailing.get_mut(selected).unwrap().push(0);
        assert!(memory(&index, &trailing).is_err());
        assert_eq!(stream(&index, &trailing, &mut allowance(size(&trailing) + 1)).unwrap_err(), WeightReadError::Refused(WeightError::Header));
    }
    struct LateFailure(Cursor<Vec<u8>>);
    impl Read for LateFailure {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if self.0.position() == self.0.get_ref().len() as u64 { return Err(io::ErrorKind::TimedOut.into()); }
            self.0.read(bytes)
        }
    }
    let mut sources: BTreeMap<_, _> = readers(&files).into_iter().map(|(file, source)| (file, LateFailure(source))).collect();
    let mut budget = allowance(size(&files) + 1);
    assert_eq!(DecoderModel::read_safetensors_shards_with_output_head(profile(), &index,
        &mut sources, &mut budget, OutputHead::TiedEmbeddings).unwrap_err(),
        WeightReadError::Io { stage: WeightReadStage::EndOfFile, kind: io::ErrorKind::TimedOut });
    compare(&memory(&index, &files).unwrap().0);
}

#[test]
fn tied_real_shard_files_support_original_checkpoint_continuation_after_removal() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("tied shard cleanup: {error}"); } }
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let directory = Directory(std::env::temp_dir().join(format!("fa-tied-shards-{}-{stamp}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
    std::fs::create_dir(&directory.0).unwrap();
    let (index, files) = bundle(&tensors(false));
    let mut sources = BTreeMap::new();
    for (file, bytes) in &files {
        let path = directory.0.join(file); std::fs::write(&path, bytes).unwrap();
        sources.insert(file.clone(), std::fs::File::open(path).unwrap());
    }
    let (model, receipt) = DecoderModel::read_llama_shards(profile().identity(), 8,
        &config(Some("true")), &index, &mut sources, &mut allowance(size(&files) + 1)).unwrap();
    assert_eq!(receipt.weights.data_bytes, 160); drop(sources); drop(directory); compare(&model);
    let budget = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let mut live = model.recompute(7, &[0, 1, 2], budget).unwrap();
    let checkpoint = live.checkpoint().unwrap();
    let (mut restored, _) = model.restore_checkpoint(&checkpoint, 8,
        DecoderRestoreBudget { cache_values: checkpoint.cache().normalized_values() }).unwrap();
    for _ in 0..5 {
        let a = live.advance_greedy(live.position(), budget).unwrap();
        let b = restored.advance_greedy(restored.position(), budget).unwrap();
        assert_eq!(a.token, b.token); assert_eq!(bits(&a.logits), bits(&b.logits));
        assert_eq!(a.work, b.work);
    }
}
