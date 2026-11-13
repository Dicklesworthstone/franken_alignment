//! Actual deterministic SafeTensors inference; fixtures are not detector evidence.
use super::*;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, MAX_SAMPLING_ENTRIES,
    text::TextGenerationRequest,
    tokenizer::{ByteBpe, Merge, TokenBytes, TokenizationBudget},
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::{
    FileDecoderConfig, text::FileTextGenerationCommand,
};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use std::collections::BTreeMap;

pub(super) fn profile() -> FileOversightProfile {
    let mut profile = support::profile();
    profile.delivery.initial_payload.clear();
    profile.delivery.total = 4096;
    profile
}

pub(super) fn model_profile() -> DecoderProfile {
    DecoderProfile::new(
        DecoderIdentity {
            tenant: 1,
            model: 9,
            model_generation: 1,
            tokenizer_generation: 1,
            profile_generation: 1,
        },
        DecoderShape {
            vocabulary: 259,
            hidden: 2,
            intermediate: 2,
            layers: 1,
            query_heads: 1,
            cache_heads: 1,
            context: 16,
        },
        1e-5,
        10000.0,
    )
    .unwrap()
}

pub(super) fn decoder() -> FileDecoderConfig {
    // Prompt -> A -> Control comes from the original sampled decoder. The first
    // output changes the actual residual used by the second numerical step.
    let mut embedding: Vec<f32> = (0..259).flat_map(|_| [1.0, 0.0]).collect();
    embedding[130] = 0.0;
    embedding[131] = 1.0;
    let mut head = vec![0.0_f32; 518];
    head[130] = 1.0;
    head[513] = 1.0;
    let mut tensors = BTreeMap::from([
        (
            "model.embed_tokens.weight".to_owned(),
            (vec![259, 2], embedding),
        ),
        ("model.norm.weight".to_owned(), (vec![2], vec![1.0_f32; 2])),
        ("lm_head.weight".to_owned(), (vec![259, 2], head)),
    ]);
    for name in ["input_layernorm.weight", "post_attention_layernorm.weight"] {
        tensors.insert(format!("model.layers.0.{name}"), (vec![2], vec![1.0; 2]));
    }
    for name in [
        "self_attn.q_proj.weight",
        "self_attn.k_proj.weight",
        "self_attn.v_proj.weight",
        "self_attn.o_proj.weight",
        "mlp.gate_proj.weight",
        "mlp.up_proj.weight",
        "mlp.down_proj.weight",
    ] {
        tensors.insert(format!("model.layers.0.{name}"), (vec![2, 2], vec![0.0; 4]));
    }
    let mut payload = Vec::new();
    let mut fields = Vec::new();
    for (name, (shape, values)) in tensors {
        let start = payload.len();
        for value in values {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        fields.push(format!(
            r#""{name}":{{"dtype":"F32","shape":{shape:?},"data_offsets":[{start},{}]}}"#,
            payload.len(),
        ));
    }
    let mut header = format!("{{{}}}", fields.join(","));
    while header.len() % 8 != 0 {
        header.push(' ');
    }
    let mut weights = (header.len() as u64).to_le_bytes().to_vec();
    weights.extend_from_slice(header.as_bytes());
    weights.extend(payload);
    let monitor = br#"{"schema":"fa.decoder-monitor/1","generation":1,
      "identity":{"tenant":1,"model":9,"model_generation":1,"tokenizer_generation":1,"profile_generation":1},
      "budget":{"encoded_bytes":10000,"probe_coordinates":10000},
      "layers":[{"layer":1,"levels":[23],"budget":{"encoded_bytes":10000,"probe_coordinates":10000},
        "probes":[{"id":1,"generation":1,"weights":[1.0,0.0],"bias":0.0,"threshold":3.0}]}]}"#;
    let sampling = br#"{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":259,
      "temperature":1.0,"top_k":1,"top_p":1.0,"stream":21,"seed":9}"#;
    FileDecoderConfig::new(
        model_profile(),
        weights,
        monitor.to_vec(),
        sampling.to_vec(),
        5,
        DecoderBindingLimits::default(),
    )
    .unwrap()
}

pub(super) fn tokenizer() -> ByteBpe {
    let mut vocabulary: Vec<_> = (0..=255)
        .map(|byte| TokenBytes::Content(vec![byte]))
        .collect();
    vocabulary.extend([
        TokenBytes::Control,
        TokenBytes::Content(b"ab".to_vec()),
        TokenBytes::Content("é".as_bytes().to_vec()),
    ]);
    ByteBpe::new(
        model_profile(),
        vocabulary,
        vec![
            Merge {
                left: 97,
                right: 98,
                result: 257,
            },
            Merge {
                left: 195,
                right: 169,
                result: 258,
            },
        ],
    )
    .unwrap()
}

pub(super) fn create(
    root: &Directory,
) -> (FileGeneratedTextActorPort, Supervisor, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_generated_text_stream_with_reserve(
        root.store(),
        profile(),
        StreamProfile::new(9, 1, 4, 64, 256).unwrap(),
        decoder(),
        tokenizer(),
        RecoveryReserve::terminal(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, supervisor) = host.into_generated_text_actor_gateway().unwrap();
    (actor, supervisor, reviewer)
}

pub(super) fn begin(supervisor: &mut Supervisor) {
    let mut host = supervisor.host_mut().unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        7,
        numerical.actor_revision,
        numerical.position,
        TextGenerationRequest {
            prompt: b"ab".to_vec(),
            prefix_controls: vec![256],
            max_new_tokens: 2,
            stop_tokens: vec![256],
            tokenization: TokenizationBudget::default(),
            generation: GenerationBudget {
                scalar_products: MAX_DECODER_PRODUCTS,
                sampling_entries: MAX_SAMPLING_ENTRIES,
            },
            max_output_bytes: 4,
        },
    )
    .unwrap();
    let revision = host.revision();
    host.begin_decoder_text(revision, command).unwrap();
}

pub(super) fn generate(supervisor: &mut Supervisor) -> FileTextMessageRequest {
    begin(supervisor);
    let mut host = supervisor.host_mut().unwrap();
    for revision in 0..4 {
        let journal_revision = host.revision();
        host.advance_decoder_text(journal_revision, 7, revision)
            .unwrap();
    }
    assert_eq!(
        host.decoder_text_generation(7)
            .unwrap()
            .result()
            .unwrap()
            .bytes()
            .unwrap(),
        b"A"
    );
    FileTextMessageRequest {
        request: REQUEST,
        generation: 7,
        generation_revision: host.decoder_text_progress(7).unwrap().generation_revision(),
        target: host.inspect().target,
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
    }
}
