//! Synthetic checkpoint assets; original loaders, inference and protocol run.
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{GenerationBudget, MAX_SAMPLING_ENTRIES};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{ByteBpe, Merge, TokenBytes, TokenizationBudget};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::oversight::helper_client::native::NativeHelperPolicy;
use fa_reference::action::consequence::oversight::helper_workers::wire::{WorkerInput, decode_request};
use fa_reference::full_input::InputProfileBinding;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub const SALT: &[u8] = b"independent-process-fixture-salt-123";
#[derive(Debug)]
pub struct Fixture {
    pub root: PathBuf,
    pub path: PathBuf,
    pub manifest: String,
    pub policy: NativeHelperPolicy,
}
impl Fixture {
    pub fn new(alarm: bool) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-native-process-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let mut vocab: Vec<_> = (0_u8..=255).map(|b| TokenBytes::Content(vec![b])).collect();
        let mut merges = Vec::new();
        let mut word = |bytes: &[u8]| {
            let mut left = u32::from(bytes[0]);
            for end in 2..=bytes.len() {
                let result = u32::try_from(vocab.len()).unwrap();
                vocab.push(TokenBytes::Content(bytes[..end].to_vec()));
                merges.push(Merge { left, right: u32::from(bytes[end - 1]), result }); left = result;
            }
            left
        };
        let allow = word(b"allow"); let deny = word(b"deny");
        let stop = u32::try_from(vocab.len()).unwrap(); vocab.push(TokenBytes::Control);
        let count = vocab.len();
        let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
            model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
            DecoderShape { vocabulary: count, hidden: 2, intermediate: 2, layers: 1,
                query_heads: 1, cache_heads: 1, context: 1024 }, 0.00001, 10000.0).unwrap();
        let tokenizer = ByteBpe::new(profile.clone(), vocab, merges).unwrap().to_bytes().unwrap();
        let configuration = format!(r#"{{"model_type":"llama","vocab_size":{count},"hidden_size":2,"intermediate_size":2,"num_hidden_layers":1,"num_attention_heads":1,"max_position_embeddings":1024,"rms_norm_eps":0.00001,"rope_theta":10000.0}}"#);
        let threshold = if alarm { 0.5 } else { 100.0 };
        let monitoring = format!(r#"{{"schema":"fa.decoder-monitor/1","generation":13,"identity":{{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5}},"budget":{{"encoded_bytes":100000,"probe_coordinates":100000}},"layers":[{{"layer":1,"levels":[23],"budget":{{"encoded_bytes":100000,"probe_coordinates":100000}},"probes":[{{"id":1,"generation":1,"weights":[-1.0,-1.0],"bias":0.0,"threshold":{threshold}}}]}}]}}"#);
        let sampling = format!(r#"{{"schema":"fa.decoder-sampling/1","id":1,"generation":1,"vocabulary":{count},"temperature":1.0,"top_k":1,"top_p":1.0,"stream":10,"seed":11}}"#);
        let input_profile = expected();
        let policy = NativeHelperPolicy { input_profile: input_profile.clone(), decoder_profile: profile,
            max_new_tokens: 2, stop_tokens: vec![stop], tokenization: TokenizationBudget::default(),
            generation: GenerationBudget { scalar_products: MAX_DECODER_PRODUCTS, sampling_entries: MAX_SAMPLING_ENTRIES }, max_output_bytes: 128 };
        let mut embeddings = vec![0.0_f32; count * 2];
        for id in 0..count { embeddings[id * 2] = 1.0; }
        embeddings[usize::from(b'!') * 2] = 0.0; embeddings[usize::from(b'!') * 2 + 1] = 1.0;
        for id in [allow, deny] { embeddings[id as usize * 2] = -1.0; embeddings[id as usize * 2 + 1] = -1.0; }
        let mut output = vec![0.0_f32; count * 2];
        output[allow as usize * 2] = 10.0; output[deny as usize * 2 + 1] = 10.0;
        output[stop as usize * 2] = -10.0; output[stop as usize * 2 + 1] = -10.0;
        let mut tensors = vec![
            ("model.embed_tokens.weight".to_owned(), vec![count, 2], embeddings),
            ("model.norm.weight".to_owned(), vec![2], vec![1.0; 2]),
            ("lm_head.weight".to_owned(), vec![count, 2], output),
        ];
        for (name, values) in [("input_layernorm.weight", vec![1.0; 2]),
            ("post_attention_layernorm.weight", vec![1.0; 2]), ("self_attn.q_proj.weight", vec![0.0; 4]),
            ("self_attn.k_proj.weight", vec![0.0; 4]), ("self_attn.v_proj.weight", vec![0.0; 4]),
            ("self_attn.o_proj.weight", vec![0.0; 4]), ("mlp.gate_proj.weight", vec![0.0; 4]),
            ("mlp.up_proj.weight", vec![0.0; 4]), ("mlp.down_proj.weight", vec![0.0; 4])] {
            let shape = if values.len() == 2 { vec![2] } else { vec![2, 2] };
            tensors.push((format!("model.layers.0.{name}"), shape, values));
        }
        let weights = weights_file(&tensors);
        for (name, bytes) in [("config.json", configuration.as_bytes()), ("tokenizer.bin", tokenizer.as_slice()),
            ("monitor.json", monitoring.as_bytes()), ("sampling.json", sampling.as_bytes()),
            ("weights.safetensors", weights.as_slice()), ("salt.bin", SALT)] {
            std::fs::write(root.join(name), bytes).unwrap();
        }
        let hex = input_profile.profile_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let configuration = quote(root.join("config.json").to_str().unwrap());
        let tokenizer = quote(root.join("tokenizer.bin").to_str().unwrap());
        let monitoring = quote(root.join("monitor.json").to_str().unwrap());
        let sampling = quote(root.join("sampling.json").to_str().unwrap());
        let weights = quote(root.join("weights.safetensors").to_str().unwrap());
        let salt_file = quote(root.join("salt.bin").to_str().unwrap());
        let manifest = format!(r#"{{"schema":"fa.native-worker/1","input":{{"id":7,"bytes_hex":"{hex}","model_epoch":0,"tokenizer_epoch":0,"policy_epoch":0}},"decoder":{{"identity":{{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5}},"shape":{{"vocabulary":{count},"hidden":2,"intermediate":2,"layers":1,"query_heads":1,"cache_heads":1,"context":1024}},"epsilon":0.00001,"theta":10000.0}},"policy":{{"max_new_tokens":2,"stop_tokens":[{stop}],"max_output_bytes":128,"tokenization":{{"input_bytes":65536,"pair_lookups":196608,"heap_pops":196608}},"generation":{{"scalar_products":1099511627776,"sampling_entries":16777216}}}},"files":{{"configuration":{configuration},"tokenizer":{tokenizer},"monitoring":{monitoring},"sampling":{sampling},"weights":{weights}}},"stream":12,"salt_file":{salt_file},"lifetime":{{"milliseconds":10000,"steps":10000}},"startup":{{"asset_bytes":1048576,"asset_calls":4096,"weight_bytes":1048576,"weight_calls":4096}}}}"#);
        let path = root.join("worker.json");
        let fixture = Self { root, path, manifest, policy }; fixture.save(); fixture
    }
    pub fn save(&self) { std::fs::write(&self.path, &self.manifest).unwrap(); }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.root) { eprintln!("native-worker fixture cleanup: {e}"); }
    }
}
fn quote(text: &str) -> String {
    let mut output = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => output.push_str("\\\\"), '"' => output.push_str("\\\""),
            c if c.is_control() => output.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => output.push(c),
        }
    }
    output.push('"'); output
}
fn weights_file(tensors: &[(String, Vec<usize>, Vec<f32>)]) -> Vec<u8> {
    let mut data = Vec::new(); let mut entries = Vec::new();
    for (name, shape, values) in tensors {
        let start = data.len();
        for value in values { data.extend_from_slice(&value.to_le_bytes()); }
        let shape = shape.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
        entries.push(format!("\"{name}\":{{\"dtype\":\"F32\",\"shape\":[{shape}],\"data_offsets\":[{start},{}]}}", data.len()));
    }
    let mut header = format!("{{{}}}", entries.join(",")).into_bytes();
    while !header.len().is_multiple_of(8) { header.push(b' '); }
    let mut file = u64::try_from(header.len()).unwrap().to_le_bytes().to_vec();
    file.extend_from_slice(&header); file.extend_from_slice(&data); file
}
pub fn expected() -> InputProfileBinding {
    InputProfileBinding { profile_id: 7, profile_bytes: b"native categorical fixture".to_vec(),
        model_epoch: 0, tokenizer_epoch: 0, policy_epoch: 0 }
}
pub fn frame(prompt: &[u8]) -> Vec<u8> {
    let profile = expected();
    let mut out = b"FAHW1".to_vec(); out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&9_u64.to_be_bytes()); out.extend_from_slice(&[7; 32]);
    out.extend_from_slice(&6_u16.to_be_bytes()); out.extend_from_slice(b"native");
    out.extend_from_slice(&64_u16.to_be_bytes());
    for number in [profile.profile_id, profile.model_epoch, profile.tokenizer_epoch, profile.policy_epoch] { out.extend_from_slice(&number.to_be_bytes()); }
    for bytes in [profile.profile_bytes.as_slice(), prompt] {
        out.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_be_bytes()); out.extend_from_slice(bytes);
    }
    out.extend_from_slice(&1_u16.to_be_bytes()); out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&u32::try_from(prompt.len()).unwrap().to_be_bytes()); out.push(1);
    out.extend_from_slice(&0_u16.to_be_bytes());
    let len = u32::try_from(out.len() - 9).unwrap(); out[5..9].copy_from_slice(&len.to_be_bytes()); out
}
pub fn input(prompt: &[u8]) -> WorkerInput { decode_request(&frame(prompt)).unwrap() }
