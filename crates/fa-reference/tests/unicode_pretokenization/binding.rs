//! The original complete trained binding must notice a grammar-only change even
//! when the selected input happens to have identical token IDs in every profile.
#[path = "../support/bound_kv_monitor.rs"]
#[allow(dead_code, unused_imports)]
mod trained;
use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::{ByteBpe, Merge, PreTokenization, TokenBytes, TokenizationBudget},
    probe::training::decoder::kv::{KvMonitorArtifact, MAX_KV_MONITOR_ARTIFACT_BYTES},
    tensor::kv::decoder::DecoderModel,
};
use fa_reference::Error;
use std::collections::BTreeMap;

fn tokenizer(model: &DecoderModel, mode: PreTokenization) -> ByteBpe {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    vocabulary.extend([TokenBytes::Control, TokenBytes::Content(b"ab".to_vec()),
        TokenBytes::Content(b"bc".to_vec()), TokenBytes::Control]);
    ByteBpe::new_with_pre_tokenization(model.profile().clone(), vocabulary, vec![
        Merge { left: 97, right: 98, result: 257 }, Merge { left: 98, right: 99, result: 258 },
    ], BTreeMap::from([(256, b"<|stop|>".to_vec()), (259, b"<|reserved|>".to_vec())]), mode).unwrap()
}

#[test]
fn unicode_complete_trained_monitor_binding_refuses_only_changed_input_semantics() {
    let campaign = trained::campaign();
    assert!(campaign.accepted());
    let model = campaign.model();
    let codec = trained::codec(model);
    let settings = trained::settings(model);
    let policy = campaign.policy(codec.clone(), &settings).unwrap();
    let raw = tokenizer(model, PreTokenization::RawBytes);
    assert_eq!(raw.to_bytes().unwrap(), trained::tokenizer(model).to_bytes().unwrap());
    let gpt2 = tokenizer(model, PreTokenization::Gpt2Unicode17);
    let llama3 = tokenizer(model, PreTokenization::Llama3Unicode17);
    let bytes = campaign.monitor_artifact(&gpt2, &codec, &settings, MAX_KV_MONITOR_ARTIFACT_BYTES).unwrap();
    let artifact = KvMonitorArtifact::decode(&bytes, bytes.len()).unwrap();
    artifact.verify(model, &gpt2, &policy).unwrap();
    let original = gpt2.encode(b"abc", TokenizationBudget::default()).unwrap();
    for changed in [raw, llama3] {
        assert_eq!(changed.profile(), gpt2.profile());
        assert_eq!(changed.encode(b"abc", TokenizationBudget::default()).unwrap().tokens(), original.tokens());
        assert_ne!(changed.to_bytes().unwrap(), gpt2.to_bytes().unwrap());
        assert_eq!(artifact.verify(model, &changed, &policy), Err(Error::Binding));
    }
    let restored = ByteBpe::from_bytes(model.profile(), &gpt2.to_bytes().unwrap()).unwrap();
    artifact.verify(model, &restored, &policy).unwrap();
}
