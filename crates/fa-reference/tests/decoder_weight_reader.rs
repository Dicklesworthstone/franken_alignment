//! Real File and fault-injected Read paths feed the original numerical decoder.
#[path = "support/weight_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/pretrained_fixture.rs"]
#[allow(dead_code)]
mod official;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::{TensorIssue, WeightError};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::reader::*;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-read-weights-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("weight reader fixture cleanup: {error}"); }
    }
}
fn allowance() -> WeightReadBudget { WeightReadBudget::new(MAX_WEIGHT_READ_BYTES, MAX_WEIGHT_READ_CALLS).unwrap() }
fn compute(model: &DecoderModel, start: usize, count: usize) -> DecoderBudget {
    DecoderBudget { scalar_products: model.estimate(start, count).unwrap().scalar_products().unwrap() }
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }
fn compare(left: &DecoderModel, right: &DecoderModel) {
    let tokens = [0, 3, 1, 5];
    let mut a = left.recompute(5, &tokens, compute(left, 0, tokens.len())).unwrap();
    let mut b = right.recompute(5, &tokens, compute(right, 0, tokens.len())).unwrap();
    for _ in 0..8 {
        assert_eq!(bits(a.logits().unwrap()), bits(b.logits().unwrap()));
        let ac = a.cache_image().unwrap(); let bc = b.cache_image().unwrap();
        for id in left.cache_profile().layers().keys() {
            assert_eq!(ac.layer(*id).unwrap().encode().unwrap(), bc.layer(*id).unwrap().encode().unwrap());
        }
        let pos = a.position();
        assert_eq!(a.greedy_token().unwrap(), b.greedy_token().unwrap());
        a.advance_greedy(pos, compute(left, pos as usize, 1)).unwrap();
        b.advance_greedy(pos, compute(right, pos as usize, 1)).unwrap();
    }
}
fn header_end(bytes: &[u8]) -> usize { 8 + u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize }
fn partition(ts: &[Tensor]) -> BTreeMap<String, Vec<Tensor>> {
    let mut groups: BTreeMap<String, Vec<Tensor>> = BTreeMap::new();
    for (i, tensor) in ts.iter().enumerate() {
        groups.entry(format!("model-{}.safetensors", i % 3)).or_default().push(tensor.clone());
    }
    groups
}
fn index(groups: &BTreeMap<String, Vec<Tensor>>) -> Vec<u8> {
    let total: usize = groups.values().flatten().map(|t| t.bytes.len()).sum();
    let entries = groups.iter().flat_map(|(name, ts)| ts.iter().map(move |t| {
        format!("\"{}\":\"{name}\"", t.name)
    })).collect::<Vec<_>>().join(",");
    format!("{{\"weight_map\":{{{entries}}},\"metadata\":{{\"total_size\":{total}}}}}").into_bytes()
}
fn files(groups: &BTreeMap<String, Vec<Tensor>>) -> BTreeMap<String, Vec<u8>> {
    groups.iter().map(|(name, ts)| (name.clone(), archive(ts))).collect()
}
fn references(sources: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, &[u8]> {
    sources.iter().map(|(name, bytes)| (name.clone(), bytes.as_slice())).collect()
}

struct Source {
    bytes: Cursor<Vec<u8>>,
    chunk: usize,
    interrupt_every: Option<usize>,
    failure_at: Option<(usize, io::ErrorKind)>,
    calls: usize,
    body_start: usize,
    largest_body_request: usize,
}
impl Source {
    fn new(bytes: Vec<u8>) -> Self {
        let body_start = header_end(&bytes);
        Self { bytes: Cursor::new(bytes), chunk: usize::MAX, interrupt_every: None,
            failure_at: None, calls: 0, body_start, largest_body_request: 0 }
    }
    fn position(&self) -> usize { self.bytes.position() as usize }
}
impl Read for Source {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.interrupt_every.is_some_and(|period| self.calls.is_multiple_of(period)) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let position = self.position();
        if position >= self.body_start { self.largest_body_request = self.largest_body_request.max(output.len()); }
        let mut length = output.len().min(self.chunk);
        if let Some((after, kind)) = self.failure_at {
            if position >= after { return Err(kind.into()); }
            length = length.min(after - position);
        }
        self.bytes.read(&mut output[..length])
    }
}

#[test]
fn official_fixture_file_matches_memory_and_checkpoint_survives_source_removal() {
    let directory = Directory::new(); let path = directory.0.join("model.safetensors");
    // Retained by the concurrent single-file implementation; not generated here.
    let bytes = include_bytes!("fixtures/decoder_mixed.safetensors");
    fs::write(&path, bytes).unwrap();
    let mut source = File::open(&path).unwrap(); let mut budget = allowance();
    let (loaded, receipt) = DecoderModel::read_safetensors(official::profile(16), &mut source, &mut budget).unwrap();
    let (memory, expected) = DecoderModel::from_safetensors(official::profile(16), bytes).unwrap();
    assert_eq!(receipt, expected);
    assert_eq!(budget.usage().bytes_read, bytes.len());
    compare(&loaded, &memory);
    let run = loaded.recompute(1, &[0, 3], compute(&loaded, 0, 2)).unwrap();
    let checkpoint = run.checkpoint().unwrap(); let expected_logits = bits(run.logits().unwrap());
    drop(source); fs::remove_file(path).unwrap(); drop(run); drop(loaded); drop(memory);
    let (resumed, _) = checkpoint.model().restore_checkpoint(&checkpoint, 2,
        DecoderRestoreBudget { cache_values: checkpoint.cache().normalized_values() }).unwrap();
    assert_eq!(bits(resumed.logits().unwrap()), expected_logits);
}

#[test]
fn fragmented_and_interrupted_input_matches_the_original_archive_interpretation() {
    let p = decoder::profile(16); let bytes = archive(&tensors(&p));
    let (memory, expected) = DecoderModel::from_safetensors(p.clone(), &bytes).unwrap();
    let mut source = Source::new(bytes.clone()); source.chunk = 1; source.interrupt_every = Some(3);
    let mut budget = allowance();
    let (loaded, receipt) = DecoderModel::read_safetensors(p, &mut source, &mut budget).unwrap();
    assert_eq!(receipt, expected); compare(&loaded, &memory);
    assert_eq!(budget.usage().bytes_read, bytes.len());
    assert_eq!(budget.usage().read_calls, source.calls);
    assert!(source.calls > bytes.len());
    assert!(source.largest_body_request <= WEIGHT_READ_CHUNK_BYTES);
}

#[test]
fn tensors_larger_than_scratch_are_decoded_without_a_tensor_sized_raw_read() {
    let p = DecoderProfile::new(decoder::profile(16).identity(), DecoderShape {
        vocabulary: 6, hidden: 64, intermediate: 96, layers: 2,
        query_heads: 4, cache_heads: 2, context: 16,
    }, 0.00001, 10000.0).unwrap();
    let bytes = archive(&tensors(&p)); let mut source = Source::new(bytes);
    let (loaded, _) = DecoderModel::read_safetensors(p.clone(), &mut source, &mut allowance()).unwrap();
    assert_eq!(source.largest_body_request, WEIGHT_READ_CHUNK_BYTES);
    let original = decoder::model(p);
    let a = loaded.recompute(1, &[0, 3], compute(&loaded, 0, 2)).unwrap();
    let b = original.recompute(1, &[0, 3], compute(&original, 0, 2)).unwrap();
    assert_eq!(bits(a.logits().unwrap()), bits(b.logits().unwrap()));
    assert_eq!(a.cache_image().unwrap().encode().unwrap(), b.cache_image().unwrap().encode().unwrap());
}

#[test]
fn every_shard_header_is_checked_before_even_the_first_scalar_body_is_read() {
    let p = decoder::profile(16); let mut groups = partition(&tensors(&p)); let ix = index(&groups);
    groups.values_mut().next().unwrap()[0].bytes[..4].copy_from_slice(&f32::NAN.to_le_bytes());
    groups.values_mut().next_back().unwrap().last_mut().unwrap().shape[0] += 1;
    let bytes = files(&groups);
    let mut sources: BTreeMap<_, _> = bytes.iter().map(|(name, bytes)| (name.clone(), Source::new(bytes.clone()))).collect();
    let mut budget = allowance();
    assert!(matches!(DecoderModel::read_safetensors_shards(p, &ix, &mut sources, &mut budget),
        Err(WeightReadError::Refused(WeightError::Tensor { issue: TensorIssue::Shape, .. }))));
    for (name, source) in &sources {
        assert_eq!(source.position(), header_end(&bytes[name]));
        assert_eq!(source.largest_body_request, 0);
    }
    assert_eq!(budget.usage().bytes_read, bytes.values().map(|bytes| header_end(bytes)).sum::<usize>());
}

#[test]
fn real_shard_files_match_memory_receipts_and_share_one_consumption_budget() {
    let directory = Directory::new(); let p = decoder::profile(16);
    let groups = partition(&tensors(&p)); let bytes = files(&groups); let ix = index(&groups);
    let total = bytes.values().map(Vec::len).sum::<usize>();
    let (memory, expected) = DecoderModel::from_safetensors_shards(p.clone(), &ix, &references(&bytes)).unwrap();
    let mut sources = BTreeMap::new();
    for (name, contents) in &bytes {
        let path = directory.0.join(name); fs::write(&path, contents).unwrap();
        sources.insert(name.clone(), File::open(path).unwrap());
    }
    let mut budget = WeightReadBudget::new(total + 1, MAX_WEIGHT_READ_CALLS).unwrap();
    let (loaded, receipt) = DecoderModel::read_safetensors_shards(p.clone(), &ix, &mut sources, &mut budget).unwrap();
    assert_eq!(receipt, expected); compare(&loaded, &memory);
    assert_eq!(budget.usage().bytes_read, total); assert_eq!(budget.remaining_bytes(), 1);
    // One whole-set budget cannot be replaced by each file's separate allowance.
    let mut short: BTreeMap<_, _> = bytes.iter().map(|(name, bytes)| (name.clone(), Source::new(bytes.clone()))).collect();
    let mut budget = WeightReadBudget::new(total, MAX_WEIGHT_READ_CALLS).unwrap();
    assert_eq!(DecoderModel::read_safetensors_shards(p, &ix, &mut short, &mut budget).unwrap_err(),
        WeightReadError::Refused(WeightError::Limit));
    assert!(short.values().all(|source| source.position() == source.body_start));
}

#[test]
fn missing_sources_and_hostile_index_labels_refuse_before_any_io() {
    let p = decoder::profile(16); let groups = partition(&tensors(&p)); let bytes = files(&groups);
    let ix = index(&groups); let name = groups.keys().next().unwrap();
    let mut sources: BTreeMap<_, _> = bytes.iter().map(|(name, bytes)| (name.clone(), Source::new(bytes.clone()))).collect();
    let missing = sources.remove(name).unwrap(); let mut budget = allowance();
    assert_eq!(DecoderModel::read_safetensors_shards(p.clone(), &ix, &mut sources, &mut budget).unwrap_err(),
        WeightReadError::Refused(WeightError::Inventory));
    assert_eq!(budget.usage(), WeightReadUsage::default());
    sources.insert(name.clone(), missing);
    let hostile = String::from_utf8(ix).unwrap().replace(name.as_str(), "../outside.safetensors");
    assert_eq!(DecoderModel::read_safetensors_shards(p, hostile.as_bytes(), &mut sources, &mut budget).unwrap_err(),
        WeightReadError::Refused(WeightError::Inventory));
    assert_eq!(budget.usage(), WeightReadUsage::default());
    assert!(sources.values().all(|source| source.calls == 0 && source.position() == 0));
}

#[test]
fn infinite_interruptions_exhaust_calls_and_retries_cannot_reset_the_same_budget() {
    let p = decoder::profile(16); let bytes = archive(&tensors(&p));
    let mut source = Source::new(bytes); source.interrupt_every = Some(1);
    let mut budget = WeightReadBudget::new(MAX_WEIGHT_READ_BYTES, 3).unwrap();
    for _ in 0..2 {
        assert_eq!(DecoderModel::read_safetensors(p.clone(), &mut source, &mut budget).unwrap_err(),
            WeightReadError::Refused(WeightError::Limit));
        assert_eq!(budget.usage(), WeightReadUsage { bytes_read: 0, read_calls: 3 });
        assert_eq!(source.calls, 3);
    }
}

#[test]
fn truncation_and_late_io_errors_keep_exact_consumption_and_do_not_change_an_existing_model() {
    let p = decoder::profile(16); let bytes = archive(&tensors(&p));
    let existing = decoder::model(p.clone());
    let existing_run = existing.recompute(1, &[0, 3], compute(&existing, 0, 2)).unwrap();
    let before = bits(existing_run.logits().unwrap());
    for cut in [0, 7, 8, header_end(&bytes) - 1, header_end(&bytes), bytes.len() - 1] {
        let mut truncated = Cursor::new(bytes[..cut].to_vec()); let mut budget = allowance();
        assert!(matches!(DecoderModel::read_safetensors(p.clone(), &mut truncated, &mut budget),
            Err(WeightReadError::Io { kind: io::ErrorKind::UnexpectedEof, .. })));
        assert_eq!(budget.usage().bytes_read, cut);
    }
    let mut source = Source::new(bytes.clone()); source.failure_at = Some((bytes.len() - 2, io::ErrorKind::PermissionDenied));
    let mut budget = allowance();
    assert_eq!(DecoderModel::read_safetensors(p.clone(), &mut source, &mut budget).unwrap_err(),
        WeightReadError::Io { stage: WeightReadStage::TensorData, kind: io::ErrorKind::PermissionDenied });
    assert_eq!(budget.usage().bytes_read, bytes.len() - 2);
    assert_eq!(bits(existing_run.logits().unwrap()), before);
    compare(&existing, &decoder::model(p));
}

#[test]
fn trailing_data_and_wouldblock_are_errors_not_implicit_restarts() {
    let p = decoder::profile(16); let bytes = archive(&tensors(&p));
    let mut extra = bytes.clone(); extra.push(1); let mut source = Source::new(extra); let mut budget = allowance();
    assert_eq!(DecoderModel::read_safetensors(p.clone(), &mut source, &mut budget).unwrap_err(),
        WeightReadError::Refused(WeightError::Header));
    assert_eq!(budget.usage().bytes_read, bytes.len() + 1);
    let mut blocked = Source::new(bytes.clone()); blocked.failure_at = Some((header_end(&bytes), io::ErrorKind::WouldBlock));
    let mut budget = allowance();
    assert_eq!(DecoderModel::read_safetensors(p.clone(), &mut blocked, &mut budget).unwrap_err(),
        WeightReadError::Io { stage: WeightReadStage::TensorData, kind: io::ErrorKind::WouldBlock });
    assert_eq!(blocked.calls, 3);
    let previous = budget.usage(); let mut fresh = Cursor::new(bytes.clone());
    let (loaded, _) = DecoderModel::read_safetensors(p.clone(), &mut fresh, &mut budget).unwrap();
    assert_eq!(budget.usage().bytes_read, previous.bytes_read + bytes.len());
    compare(&loaded, &decoder::model(p));
}

#[test]
fn oversized_headers_and_insufficient_byte_or_call_allowances_refuse_before_parameter_reads() {
    let p = decoder::profile(16); let bytes = archive(&tensors(&p));
    let mut forged = Cursor::new(u64::MAX.to_le_bytes()); let mut budget = allowance();
    assert_eq!(DecoderModel::read_safetensors(p.clone(), &mut forged, &mut budget).unwrap_err(),
        WeightReadError::Refused(WeightError::Limit));
    assert_eq!(budget.usage(), WeightReadUsage { bytes_read: 8, read_calls: 1 });
    for (byte_limit, calls, expected_position) in [(7, 10, 7), (bytes.len(), MAX_WEIGHT_READ_CALLS, header_end(&bytes)), (bytes.len() + 1, 1, 8)] {
        let mut source = Source::new(bytes.clone()); let mut budget = WeightReadBudget::new(byte_limit, calls).unwrap();
        assert_eq!(DecoderModel::read_safetensors(p.clone(), &mut source, &mut budget).unwrap_err(),
            WeightReadError::Refused(WeightError::Limit));
        assert_eq!(source.position(), expected_position);
        assert_eq!(source.largest_body_request, 0);
    }
    let mut source = Cursor::new(bytes.clone());
    let mut exact = WeightReadBudget::new(bytes.len() + 1, MAX_WEIGHT_READ_CALLS).unwrap();
    assert!(DecoderModel::read_safetensors(p, &mut source, &mut exact).is_ok());
    assert_eq!(exact.remaining_bytes(), 1);
}

#[test]
fn late_bad_shard_does_not_publish_a_model_or_damage_a_prior_parameter_owner() {
    let p = decoder::profile(16); let mut groups = partition(&tensors(&p)); let ix = index(&groups);
    let existing = decoder::model(p.clone());
    let first = existing.recompute(1, &[0, 3], compute(&existing, 0, 2)).unwrap();
    let before = bits(first.logits().unwrap());
    groups.values_mut().next_back().unwrap().last_mut().unwrap().bytes[..4].copy_from_slice(&f32::INFINITY.to_le_bytes());
    let bytes = files(&groups); let mut sources: BTreeMap<_, _> = bytes.iter().map(|(name, bytes)| (name.clone(), Source::new(bytes.clone()))).collect();
    let mut budget = allowance();
    assert!(matches!(DecoderModel::read_safetensors_shards(p.clone(), &ix, &mut sources, &mut budget),
        Err(WeightReadError::Refused(WeightError::Tensor { issue: TensorIssue::NonFinite, .. }))));
    assert_eq!(budget.usage().bytes_read, sources.values().map(Source::position).sum::<usize>());
    let first_source = sources.values().next().unwrap();
    assert_eq!(first_source.position(), first_source.bytes.get_ref().len());
    assert_eq!(bits(first.logits().unwrap()), before);
    compare(&existing, &decoder::model(p));
}
