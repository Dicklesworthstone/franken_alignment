//! Original file bootstrap and a complete binary request with independently
//! framed part/omission metadata. Synthetic weights are not detector evidence.
use super::assets;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::reader::WeightReadBudget;
use fa_reference::action::consequence::oversight::helper_client::native::{
    NativeEvaluationError, NativeEvaluationProgress, NativeEvaluationStatus, NativeEvaluator,
    NativeHelperPolicy,
};
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::{
    NativeAssetReadBudget, NativeFileBootstrap, NativeHelperFileLimits, NativeHelperFiles,
};
use fa_reference::action::consequence::oversight::helper_workers::wire::{WorkerInput, decode_request};

pub fn prompt(last: u8) -> Vec<u8> {
    let mut bytes = b"\0\xff".to_vec();
    for _ in 0..80 { bytes.extend_from_slice(b"allow"); }
    bytes.push(last);
    bytes
}

pub fn frame(prompt: &[u8]) -> Vec<u8> {
    assert!(prompt.len() > 3);
    let mut bytes = assets::frame(prompt);
    // Replace the fixture's one-part/zero-omission tail with three original
    // typed parts and a real declared closed-absence marker.
    bytes.truncate(bytes.len() - 13);
    bytes.extend_from_slice(&3_u16.to_be_bytes());
    for (start, end, kind) in [(0, 2, 7_u8), (2, prompt.len() - 1, 5),
        (prompt.len() - 1, prompt.len(), 1)] {
        bytes.extend_from_slice(&u32::try_from(start).unwrap().to_be_bytes());
        bytes.extend_from_slice(&u32::try_from(end).unwrap().to_be_bytes());
        bytes.push(kind);
        if kind == 5 {
            bytes.extend_from_slice(&17_u64.to_be_bytes());
            bytes.extend_from_slice(&22_u64.to_be_bytes());
        }
    }
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.extend_from_slice(&31_u64.to_be_bytes());
    bytes.extend_from_slice(&32_u64.to_be_bytes());
    let length = u32::try_from(bytes.len() - 9).unwrap();
    bytes[5..9].copy_from_slice(&length.to_be_bytes());
    bytes
}

pub fn input(prompt: &[u8]) -> WorkerInput { decode_request(&frame(prompt)).unwrap() }

pub fn load(fixture: &assets::Fixture, policy: &NativeHelperPolicy) -> NativeEvaluator {
    let mut asset_budget = NativeAssetReadBudget::new(1_048_576, 4096).unwrap();
    let mut weight_budget = WeightReadBudget::new(1_048_576, 4096).unwrap();
    let (worker, _) = NativeEvaluator::from_llama_files(NativeFileBootstrap {
        policy, stream: 12,
        files: NativeHelperFiles {
            configuration: &fixture.root.join("config.json"),
            tokenizer: &fixture.root.join("tokenizer.bin"),
            monitoring: &fixture.root.join("monitor.json"),
            sampling: &fixture.root.join("sampling.json"),
            weights: &fixture.root.join("weights.safetensors"),
        },
        limits: NativeHelperFileLimits::default(),
    }, &mut asset_budget, &mut weight_budget).unwrap();
    assert!(asset_budget.usage().bytes_read > 0);
    assert!(weight_budget.usage().bytes_read > 0);
    assert_eq!(worker.position(), 0);
    worker
}

pub fn tokenizer(fixture: &assets::Fixture) -> ByteBpe {
    ByteBpe::from_bytes(&fixture.policy.decoder_profile,
        &std::fs::read(fixture.root.join("tokenizer.bin")).unwrap()).unwrap()
}

pub fn admit(worker: &mut NativeEvaluator) -> Result<NativeEvaluationProgress, NativeEvaluationError> {
    for _ in 0..8192 {
        if worker.status() == NativeEvaluationStatus::Running { return Ok(worker.progress()); }
        let before = worker.progress();
        let progress = worker.advance(0)?;
        assert_eq!(progress.admission_steps, before.admission_steps + 1);
        assert_eq!(progress.work, before.work);
        assert_eq!(progress.work.decoder.tokens, 0);
        assert_eq!(progress.work.sampled_draws, 0);
        assert_eq!(progress.reviewed_prompt_tokens, 0);
        assert_eq!(progress.released_answer_tokens, 0);
        assert_eq!(progress.finish, None);
        assert!(worker.report().is_none());
        assert!(progress.tokenization.pair_lookups <= before.tokenization.pair_lookups + 2);
        assert!(progress.tokenization.heap_pops <= before.tokenization.heap_pops + 1);
        if progress.status == NativeEvaluationStatus::Admitting {
            assert_eq!(progress.requested_prompt_tokens, 0);
        } else {
            assert_eq!(progress.status, NativeEvaluationStatus::Running);
        }
    }
    panic!("bounded original tokenizer did not terminate");
}

pub fn finish(worker: &mut NativeEvaluator) -> Result<NativeEvaluationProgress, NativeEvaluationError> {
    for _ in 0..2048 {
        let before = worker.progress();
        let progress = worker.advance(worker.position())?;
        assert_eq!(progress.work.decoder.tokens, before.work.decoder.tokens + 1);
        assert_eq!(progress.admission_steps, before.admission_steps);
        assert_eq!(progress.tokenization, before.tokenization);
        if matches!(progress.status, NativeEvaluationStatus::Judged(_)) { return Ok(progress); }
        assert_eq!(progress.status, NativeEvaluationStatus::Running);
        assert!(worker.report().is_none());
    }
    panic!("bounded original model did not terminate");
}
