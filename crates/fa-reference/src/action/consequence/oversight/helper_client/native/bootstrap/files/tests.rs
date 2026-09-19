use super::*;
use super::super::tests::{Fixture, budget};
use super::super::super::NativeEvaluationStatus;
#[cfg(unix)]
use super::super::super::NativeEvaluationError;
use super::super::super::tests::input;
use crate::round::Verdict;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory {
    root: PathBuf,
    paths: [PathBuf; 5],
    fixture: Fixture,
}
impl Directory {
    fn new(alarm: bool) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-native-cold-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        let paths = ["model.json", "tokens.fa", "monitor.json", "sample.json", "weights.safetensors"].map(|name| root.join(name));
        let fixture = Fixture::new(alarm);
        let weight_bytes = fixture.weights();
        for (path, bytes) in paths.iter().zip([
            fixture.configuration.as_slice(), &fixture.tokenizer, &fixture.monitoring,
            &fixture.sampling, weight_bytes.as_slice(),
        ]) { fs::write(path, bytes).unwrap(); }
        Self { root, paths, fixture }
    }
    fn request(&self) -> NativeFileBootstrap<'_> {
        NativeFileBootstrap { policy: &self.fixture.policy, stream: 12,
            files: NativeHelperFiles { configuration: &self.paths[0], tokenizer: &self.paths[1],
                monitoring: &self.paths[2], sampling: &self.paths[3], weights: &self.paths[4] },
            limits: NativeHelperFileLimits::default() }
    }
    fn asset_bytes(&self) -> usize {
        self.fixture.configuration.len() + self.fixture.tokenizer.len()
            + self.fixture.monitoring.len() + self.fixture.sampling.len()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("native bootstrap cleanup: {error}"); }
    }
}
fn assets() -> NativeAssetReadBudget { NativeAssetReadBudget::new(MAX_ASSET_READ_BYTES, MAX_ASSET_READ_CALLS).unwrap() }

#[test]
fn real_regular_files_bootstrap_a_fresh_input_sensitive_native_helper() {
    let root = Directory::new(false);
    for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
        let mut assets = assets(); let mut weights = budget();
        let (mut worker, receipt) = NativeEvaluator::from_llama_files(root.request(), &mut assets, &mut weights).unwrap();
        assert_eq!(assets.usage().bytes_read, root.asset_bytes());
        assert_eq!(weights.usage().bytes_read, root.fixture.weights().len());
        assert_eq!(receipt.weights.file_bytes, weights.usage().bytes_read);
        assert_eq!(worker.status(), NativeEvaluationStatus::AwaitingInput);
        assert_eq!(worker.position(), 0); assert_eq!(worker.sampled_draws(), 0);
        assert_eq!(worker.evaluate(&input(prompt)), Ok(verdict));
        assert_eq!(worker.report().unwrap().prompt().source(), prompt);
    }
}

#[test]
fn each_file_ceiling_is_exact_and_never_silently_truncates_a_valid_asset() {
    let root = Directory::new(false);
    let kinds = [NativeAsset::Configuration, NativeAsset::Tokenizer, NativeAsset::Monitoring, NativeAsset::Sampling, NativeAsset::Weights];
    for (index, asset) in kinds.into_iter().enumerate() {
        let size = fs::metadata(&root.paths[index]).unwrap().len() as usize;
        for shortage in [0, 1] {
            let mut request = root.request();
            let limit = match index {
                0 => &mut request.limits.configuration_bytes,
                1 => &mut request.limits.tokenizer_bytes,
                2 => &mut request.limits.monitoring_bytes,
                3 => &mut request.limits.sampling_bytes,
                _ => &mut request.limits.weight_bytes,
            };
            *limit = size - shortage;
            let result = NativeEvaluator::from_llama_files(request, &mut assets(), &mut budget());
            if shortage == 0 { assert!(result.is_ok()); }
            else { assert!(matches!(result, Err(NativeFileBootstrapError::Limit(found)) if found == asset)); }
        }
    }
}

#[test]
fn malformed_binding_precedes_weight_open_and_cannot_fall_back_to_another_file() {
    let root = Directory::new(false);
    fs::remove_file(&root.paths[4]).unwrap();
    let mut tokenizer = root.fixture.tokenizer.clone(); tokenizer[8] ^= 1;
    fs::write(&root.paths[1], tokenizer).unwrap();
    let mut weights = budget();
    assert!(matches!(NativeEvaluator::from_llama_files(root.request(), &mut assets(), &mut weights),
        Err(NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Tokenizer(Error::Binding)))));
    assert_eq!(weights.usage().read_calls, 0);
    // With the valid tokenizer, the missing explicit weight file is now exposed.
    fs::write(&root.paths[1], &root.fixture.tokenizer).unwrap();
    assert!(matches!(NativeEvaluator::from_llama_files(root.request(), &mut assets(), &mut weights),
        Err(NativeFileBootstrapError::Io { asset: NativeAsset::Weights, stage: NativeFileStage::Metadata, kind: io::ErrorKind::NotFound })));
}

#[test]
fn directories_and_symlinks_never_become_model_or_configuration_streams() {
    let root = Directory::new(false);
    for index in 0..5 {
        let original = fs::read(&root.paths[index]).unwrap();
        fs::remove_file(&root.paths[index]).unwrap(); fs::create_dir(&root.paths[index]).unwrap();
        assert!(matches!(NativeEvaluator::from_llama_files(root.request(), &mut assets(), &mut budget()),
            Err(NativeFileBootstrapError::NotRegular(_))));
        fs::remove_dir(&root.paths[index]).unwrap();
        #[cfg(unix)] {
            let other = root.root.join(format!("target-{index}")); fs::write(&other, &original).unwrap();
            std::os::unix::fs::symlink(&other, &root.paths[index]).unwrap();
            assert!(matches!(NativeEvaluator::from_llama_files(root.request(), &mut assets(), &mut budget()),
                Err(NativeFileBootstrapError::NotRegular(_))));
            fs::remove_file(&root.paths[index]).unwrap();
        }
        fs::write(&root.paths[index], original).unwrap();
    }
    assert!(NativeEvaluator::from_llama_files(root.request(), &mut assets(), &mut budget()).is_ok());
}

#[test]
fn malformed_monitor_and_trailing_weights_do_not_return_partly_loaded_evaluators() {
    let root = Directory::new(false);
    fs::write(&root.paths[2], b"{}").unwrap(); let mut weight_usage = budget();
    assert!(matches!(NativeEvaluator::from_llama_files(root.request(), &mut assets(), &mut weight_usage),
        Err(NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Sampling(_)))));
    assert_eq!(weight_usage.usage().bytes_read, root.fixture.weights().len());
    fs::write(&root.paths[2], &root.fixture.monitoring).unwrap();
    let mut bytes = root.fixture.weights(); bytes.push(0); fs::write(&root.paths[4], &bytes).unwrap();
    assert!(matches!(NativeEvaluator::from_llama_files(root.request(), &mut assets(), &mut budget()),
        Err(NativeFileBootstrapError::Bootstrap(NativeBootstrapError::Checkpoint(_)))));
}

#[test]
fn exhausted_asset_budget_survives_a_second_startup_attempt_without_weight_work() {
    let root = Directory::new(false);
    let mut assets = NativeAssetReadBudget::new(MAX_ASSET_READ_BYTES, 1).unwrap(); let mut weights = budget();
    assert!(NativeEvaluator::from_llama_files(root.request(), &mut assets, &mut weights).is_err());
    let before = assets.usage();
    assert_eq!(before.read_calls, 1); assert!(before.bytes_read > 0);
    assert_eq!(weights.usage().read_calls, 0);
    assert!(NativeEvaluator::from_llama_files(root.request(), &mut assets, &mut weights).is_err());
    assert_eq!(assets.usage(), before); assert_eq!(weights.usage().read_calls, 0);
}

#[test]
fn aggregate_asset_bytes_leave_room_for_the_original_eof_observation() {
    let root = Directory::new(false);
    for spare in [0, 1] {
        let mut usage = NativeAssetReadBudget::new(root.asset_bytes() + spare, MAX_ASSET_READ_CALLS).unwrap();
        let result = NativeEvaluator::from_llama_files(root.request(), &mut usage, &mut budget());
        assert_eq!(result.is_ok(), spare == 1);
        assert_eq!(usage.usage().bytes_read, root.asset_bytes());
    }
}

#[test]
fn repeated_interruptions_and_faulty_read_counts_are_bounded_without_false_success() {
    struct Interrupted;
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { Err(io::ErrorKind::Interrupted.into()) }
    }
    let mut usage = NativeAssetReadBudget::new(100, 3).unwrap();
    assert_eq!(read_bytes(&mut Interrupted, &mut Vec::new(), 10, NativeAsset::Sampling, &mut usage),
        Err(NativeFileBootstrapError::Limit(NativeAsset::Sampling)));
    assert_eq!(usage.usage().read_calls, 3); assert_eq!(usage.usage().bytes_read, 0);
    struct Invalid;
    impl Read for Invalid { fn read(&mut self, b: &mut [u8]) -> io::Result<usize> { Ok(b.len() + 1) } }
    let mut usage = assets();
    assert!(matches!(read_bytes(&mut Invalid, &mut Vec::new(), 10, NativeAsset::Sampling, &mut usage),
        Err(NativeFileBootstrapError::Io { kind: io::ErrorKind::InvalidData, .. })));
    assert_eq!(usage.usage().bytes_read, 0); assert_eq!(usage.usage().read_calls, 1);
}

#[test]
fn a_growing_auxiliary_stream_is_rejected_instead_of_accepting_truncated_json() {
    let mut usage = assets(); let mut retained = Vec::new();
    let mut source = io::Cursor::new(b"{}extra".to_vec());
    assert_eq!(read_bytes(&mut source, &mut retained, 2, NativeAsset::Sampling, &mut usage),
        Err(NativeFileBootstrapError::Limit(NativeAsset::Sampling)));
    assert_eq!(usage.usage().bytes_read, 3);
    let mut retained = Vec::new();
    read_bytes(&mut io::Cursor::new(b"{}"), &mut retained, 2, NativeAsset::Sampling, &mut assets()).unwrap();
    assert_eq!(retained, b"{}");
}

#[test]
fn invalid_file_limits_and_zero_stream_refuse_before_any_asset_reads() {
    let root = Directory::new(false);
    for zero_stream in [false, true] {
        let mut request = root.request();
        if zero_stream { request.stream = 0; } else { request.limits.weight_bytes = 0; }
        let mut usage = assets(); let mut weights = budget();
        assert!(matches!(NativeEvaluator::from_llama_files(request, &mut usage, &mut weights),
            Err(NativeFileBootstrapError::Contract(_))));
        assert_eq!(usage.usage().read_calls, 0); assert_eq!(weights.usage().read_calls, 0);
    }
}

#[cfg(unix)]
#[test]
fn checkpoint_file_peer_uses_the_original_cooperative_request_commit_and_reveal_frames() {
    use super::super::super::peer::{NativeClientError, NativeHelperClient};
    use super::super::super::super::{ClientPhase, ClientInterest};
    use super::super::super::tests::{frame, expected};
    use std::os::unix::net::UnixStream;
    use std::io::Write;
    for alarm in [false, true] {
        let root = Directory::new(alarm); let (mut supervisor, socket) = UnixStream::pair().unwrap();
        supervisor.set_nonblocking(true).unwrap(); let salt = vec![27; 16];
        let (mut worker, _) = NativeHelperClient::from_llama_files(root.request(), socket, salt.clone(), &mut assets(), &mut budget()).unwrap();
        assert_eq!(worker.phase(), ClientPhase::ReadingRequest); assert_eq!(worker.evaluations(), 0);
        let mut scratch = [0]; assert_eq!(supervisor.read(&mut scratch).unwrap_err().kind(), io::ErrorKind::WouldBlock);
        supervisor.write_all(&frame(b"?", &expected())).unwrap();
        for _ in 0..256 {
            if worker.step().is_err() || worker.phase() == ClientPhase::AwaitingReveal { break; }
        }
        assert_eq!(worker.evaluations(), 1);
        if alarm {
            assert!(matches!(worker.failure(), Some(NativeClientError::Inference(NativeEvaluationError::Incomplete(_)))));
            assert_eq!(worker.interest(), ClientInterest::Finished);
            assert_eq!(supervisor.read(&mut scratch).unwrap_err().kind(), io::ErrorKind::WouldBlock);
        } else {
            assert_eq!(worker.phase(), ClientPhase::AwaitingReveal);
            let mut commit = [0; 9]; supervisor.read_exact(&mut commit).unwrap();
            assert_eq!(commit, worker.input().unwrap().commitment_frame(Verdict::Allow, &salt).unwrap());
            supervisor.write_all(b"R").unwrap();
            for _ in 0..128 { worker.step().unwrap(); if worker.phase() == ClientPhase::ReplySent { break; } }
            assert_eq!(worker.phase(), ClientPhase::ReplySent);
            let expected = worker.input().unwrap().reveal_frame(Verdict::Allow, &salt).unwrap();
            let mut reveal = vec![0; expected.len()]; supervisor.read_exact(&mut reveal).unwrap();
            assert_eq!(reveal, expected); assert_eq!(worker.sampled_draws(), 2);
        }
    }
}

#[cfg(unix)]
#[test]
fn invalid_salt_prevents_all_disk_reads_and_failed_bootstrap_closes_without_a_vote() {
    use super::super::super::peer::NativeHelperClient;
    use std::os::unix::net::UnixStream;
    for bad_salt in [false, true] {
        let root = Directory::new(false); let (mut supervisor, socket) = UnixStream::pair().unwrap();
        let mut usage = assets(); let mut weights = budget();
        if !bad_salt { fs::write(&root.paths[1], b"not a tokenizer").unwrap(); }
        assert!(NativeHelperClient::from_llama_files(root.request(), socket, vec![27; if bad_salt { 15 } else { 16 }],
            &mut usage, &mut weights).is_err());
        assert_eq!(weights.usage().read_calls, 0);
        if bad_salt { assert_eq!(usage.usage().read_calls, 0); }
        let mut output = [0]; assert_eq!(supervisor.read(&mut output).unwrap(), 0);
    }
}
