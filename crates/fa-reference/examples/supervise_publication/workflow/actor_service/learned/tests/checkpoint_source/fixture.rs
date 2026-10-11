//! Independently provisioned final-generation data for source-manifest tests.
//! The oracle loads manually changed effective tensors, never the source loader
//! or adapter merger being exercised by the operator under test.
use super::super::{Config, Root, fixture, models};
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderIdentity, DecoderModel,
        sampling::{SampleBudget, SamplingBudget, SamplingPolicy, SamplingStart}},
    model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy, replay::archive::MAX_FIT_ARCHIVE_BYTES},
};
use fa_reference::strict_json::{self, Limits};
use fa_reference::action::consequence::gate::containment::{ActorState, RestartProfile};
use std::{collections::BTreeMap, fs, path::{Path, PathBuf}};

pub(super) struct SourceFixture {
    pub path: PathBuf,
    pub manifest: PathBuf,
    pub base: Vec<u8>,
    pub effective_bytes: Vec<u8>,
    pub effective: DecoderModel,
    pub actor_shards: Vec<PathBuf>,
    pub helper_shards: Vec<PathBuf>,
    pub adapter_weights: PathBuf,
    pub adapter_config: PathBuf,
    pub single: PathBuf,
}
pub(super) fn final_identity() -> DecoderIdentity {
    DecoderIdentity { model_generation: 2, profile_generation: 2, ..models::identity(9) }
}
pub(super) fn configured(root: &Root) -> Config {
    let mut config = fixture::configured(root);
    // Independently provision the host for the declared final generation too.
    // Even AuditOnly actor metadata must match the original decoder gate's
    // model/tokenizer identities; no identity is copied from a loaded source.
    let original = &config.profile.delivery.actor;
    let profile = RestartProfile { model_generation: final_identity().model_generation,
        tokenizer_generation: final_identity().tokenizer_generation, ..original.profile() };
    config.profile.delivery.actor = ActorState::new(profile, original.tokens().to_vec(),
        original.cache().to_vec(), original.sampler().to_vec(), original.next_position()).unwrap();
    config
}
pub(super) fn read_budget() -> DecoderBudget { DecoderBudget { scalar_products: 1_000_000 } }
pub(super) fn write(root: &Root) -> SourceFixture {
    let path = fixture::write(root, b"allow");
    let configuration = fs::read(root.0.join("model.json")).unwrap();
    let base = fs::read(root.0.join("weights.safetensors")).unwrap();
    // rank=1, alpha=1, A=[1,0], B=[1,0] adds exactly 1 to V[0,0].
    let mut expected = base.clone();
    replace_value(&mut expected, "model.layers.0.self_attn.v_proj.weight", 0, 2.0);
    let effective = DecoderModel::from_llama_safetensors(final_identity(), 16,
        &configuration, &expected).unwrap().0;
    let unadapted_same_identity = DecoderModel::from_llama_safetensors(final_identity(), 16,
        &configuration, &base).unwrap().0;
    let plain = unadapted_same_identity.recompute(11, &[120, 121], read_budget()).unwrap();
    let adapted = effective.recompute(11, &[120, 121], read_budget()).unwrap();
    assert_eq!(plain.model().profile(), adapted.model().profile());
    assert_eq!(plain.logits().unwrap(), adapted.logits().unwrap());
    assert_ne!(plain.cache_image().unwrap().layer(1).unwrap().token(0).unwrap().value().encode_initial(23).unwrap(),
        adapted.cache_image().unwrap().layer(1).unwrap().token(0).unwrap().value().encode_initial(23).unwrap());
    fit(root, &effective);

    let adapter_config = root.0.join("actor-adapter.json");
    let adapter_weights = root.0.join("actor-adapter.safetensors");
    fs::write(&adapter_config, br#"{"peft_type":"LORA","task_type":"CAUSAL_LM","inference_mode":true,"r":1,"lora_alpha":1,"target_modules":["v_proj"],"bias":"none","use_dora":false,"use_rslora":false,"fan_in_fan_out":false,"lora_bias":false,"modules_to_save":null,"rank_pattern":{},"alpha_pattern":{},"init_lora_weights":true}"#).unwrap();
    fs::write(&adapter_weights, adapter()).unwrap();
    let (actor_base, actor_shards) = shards(&root.0, "actor", &base);
    let manifest = root.0.join("actor-source.json");
    let adapter = adapter_declaration(&adapter_config, &adapter_weights);
    fs::write(&manifest, source(&actor_base, &adapter)).unwrap();
    let helper_bytes = fs::read(root.0.join("helper-weights.safetensors")).unwrap();
    let (helper_base, helper_shards) = shards(&root.0, "helper", &helper_bytes);
    let helper_manifest = root.0.join("helper-source.json");
    fs::write(&helper_manifest, source(&helper_base, "null")).unwrap();

    let plan = fs::read_to_string(&path).unwrap()
        .replacen("fa.learned-publication/1", "fa.learned-publication/3", 1)
        .replacen("\"model_generation\":1", "\"model_generation\":2", 1)
        .replacen("\"profile_generation\":1", "\"profile_generation\":2", 1)
        .replacen("\"sidecar\":{", "\"sidecar\":{\"priority\":[],", 1)
        .replacen("\"native_review\":{", "\"native_review\":{\"round_ids\":[1],", 1)
        .replacen(&format!("\"weights\":{}", quote(root.0.join("weights.safetensors").to_str().unwrap())),
            &format!("\"weights\":{}", selection(&manifest)), 1);
    fs::write(&path, plan).unwrap();
    let roster_path = root.0.join("native-roster.json");
    let roster = fs::read_to_string(&roster_path).unwrap().replace("fa.learned-native-roster/1", "fa.learned-native-roster/2")
        .replace(&format!("\"weights\":{}", quote(root.0.join("helper-weights.safetensors").to_str().unwrap())),
            &format!("\"weights\":{}", selection(&helper_manifest)));
    fs::write(roster_path, roster).unwrap();
    // These absent original filenames make a hidden monolithic fallback fail.
    fs::remove_file(root.0.join("weights.safetensors")).unwrap();
    fs::remove_file(root.0.join("helper-weights.safetensors")).unwrap();
    SourceFixture { path, manifest, base, effective_bytes: expected, effective, actor_shards,
        helper_shards, adapter_weights, adapter_config, single: root.0.join("repacked-base.bin") }
}
impl SourceFixture {
    pub fn repack(&self) {
        fs::write(&self.single, &self.base).unwrap();
        let base = format!("{{\"kind\":\"single\",\"path\":{}}}", quote(self.single.to_str().unwrap()));
        fs::write(&self.manifest, source(&base, &adapter_declaration(&self.adapter_config, &self.adapter_weights))).unwrap();
        for path in &self.actor_shards { fs::remove_file(path).unwrap(); }
    }
    pub fn oracle(&self) -> (Vec<u32>, Vec<Vec<f32>>) {
        let start = SamplingStart { policy: SamplingPolicy::new(1, 1, 257, 1.0, 1, 1.0).unwrap(), stream: 21, seed: 9 };
        let mut original = self.effective.recompute_sampled(5, &[120, 121], read_budget(), start).unwrap();
        let mut tokens = Vec::new(); let mut logits = Vec::new();
        for position in 2..4 {
            let step = original.advance_sampled(position, SampleBudget { decoder: read_budget(),
                sampling: SamplingBudget { vocabulary: 257 } }).unwrap();
            tokens.push(step.choice.token); logits.push(step.computation.logits.to_vec());
        }
        assert_eq!(tokens, [65, 256]); (tokens, logits)
    }
}
pub(super) fn selection(path: &Path) -> String {
    format!("{{\"format\":\"checkpoint_source\",\"path\":{}}}", quote(path.to_str().unwrap()))
}
fn adapter_declaration(configuration: &Path, weights: &Path) -> String {
    format!("{{\"base_identity\":{{\"tenant\":1,\"model\":9,\"model_generation\":1,\"tokenizer_generation\":1,\"profile_generation\":1}},\"configuration\":{},\"weights\":{},\"merge_products\":4}}",
        quote(configuration.to_str().unwrap()), quote(weights.to_str().unwrap()))
}
pub(super) fn source(base: &str, adapter: &str) -> String {
    format!("{{\"schema\":\"fa.checkpoint-weights/1\",\"base\":{base},\"adapter\":{adapter},\"limits\":{{\"weight_bytes\":65536,\"weight_read_calls\":4096}}}}")
}
pub(super) fn replace_value(bytes: &mut [u8], name: &str, index: usize, value: f32) {
    let length = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let json = strict_json::parse(&bytes[8..8 + length], Limits::default()).unwrap();
    let descriptor = json.get(name).unwrap();
    assert_eq!(descriptor.get("dtype").unwrap().as_str(), Some("F32"));
    let offset = descriptor.get("data_offsets").unwrap().as_array().unwrap()[0].as_u64().unwrap() as usize;
    let at = 8 + length + offset + index * 4; bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn fit(root: &Root, model: &DecoderModel) {
    let source = model.recompute(11, &[120; 4], read_budget()).unwrap().cache_image().unwrap();
    let policy = LearnedKvPolicy::new(1, 1, 1, 8).unwrap();
    let budget = FitBudget { source_values: 64, parameter_values: 64, scratch_values: 16, work_units: 10_000 };
    let (_, checkpoint) = LearnedKvCodec::fit_with_checkpoint(policy, &BTreeMap::from([(101, source)]), budget).unwrap();
    let descriptor = checkpoint.binding().sources[&101].encode().unwrap().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    fs::write(root.0.join("fit-binding.json"), format!(r#"{{"schema":"fa.learned-fit-binding/1","policy":{{"id":1,"generation":1,"rank":1,"sweeps":8}},"budget":{{"source_values":64,"parameter_values":64,"scratch_values":16,"work_units":10000}},"sources":[{{"origin":101,"descriptor_hex":"{descriptor}"}}]}}"#)).unwrap();
    fs::write(root.0.join("fit.fakv"), checkpoint.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap()).unwrap();
}
fn adapter() -> Vec<u8> {
    let mut entries = Vec::new(); let mut data = Vec::new();
    for (name, shape) in [("A", "[1,2]"), ("B", "[2,1]")] {
        let start = data.len(); for value in [1.0_f32, 0.0] { data.extend_from_slice(&value.to_le_bytes()); }
        entries.push(format!("\"base_model.model.model.layers.0.self_attn.v_proj.lora_{name}.weight\":{{\"dtype\":\"F32\",\"shape\":{shape},\"data_offsets\":[{start},{}]}}", data.len()));
    }
    package(entries, data)
}
pub(super) fn shards(root: &Path, prefix: &str, bytes: &[u8]) -> (String, Vec<PathBuf>) {
    let header = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let json = strict_json::parse(&bytes[8..8 + header], Limits::default()).unwrap();
    let data = &bytes[8 + header..]; let mut map = Vec::new(); let mut declared = Vec::new(); let mut paths = Vec::new();
    for part in 0..2 {
        let label = format!("part-{part}.safetensors"); let path = root.join(format!("{prefix}-registered-{part}.bin"));
        let mut entries = Vec::new(); let mut packed = Vec::new();
        for (name, tensor) in json.as_object().unwrap() {
            if name.starts_with("model.layers.") != (part == 1) { continue; }
            let offsets = tensor.get("data_offsets").unwrap().as_array().unwrap();
            let begin = offsets[0].as_u64().unwrap() as usize; let end = offsets[1].as_u64().unwrap() as usize;
            let start = packed.len(); packed.extend_from_slice(&data[begin..end]);
            let shape = tensor.get("shape").unwrap().as_array().unwrap().iter().map(|size| size.as_u64().unwrap().to_string()).collect::<Vec<_>>().join(",");
            let dtype = tensor.get("dtype").unwrap().as_str().unwrap();
            entries.push(format!("{}:{{\"dtype\":{},\"shape\":[{shape}],\"data_offsets\":[{start},{}]}}", quote(name), quote(dtype), packed.len()));
            map.push(format!("{}:{}", quote(name), quote(&label)));
        }
        fs::write(&path, package(entries, packed)).unwrap();
        declared.push(format!("{}:{}", quote(&label), quote(path.to_str().unwrap()))); paths.push(path);
    }
    let index = root.join(format!("{prefix}-index.json"));
    fs::write(&index, format!("{{\"weight_map\":{{{}}}}}", map.join(","))).unwrap();
    (format!("{{\"kind\":\"sharded\",\"index\":{},\"shards\":{{{}}}}}", quote(index.to_str().unwrap()), declared.join(",")), paths)
}
fn package(entries: Vec<String>, data: Vec<u8>) -> Vec<u8> {
    let mut header = format!("{{{}}}", entries.join(",")).into_bytes();
    while !header.len().is_multiple_of(8) { header.push(b' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec(); bytes.extend(header); bytes.extend(data); bytes
}
pub(super) fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
