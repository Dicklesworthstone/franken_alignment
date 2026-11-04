//! File-backed, nonzero-attention prompt effects through original public codecs.
use super::super::{fixture, models, Root};
use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::config::SamplingConfig,
    tensor::kv::{decoder::{DecoderBudget, DecoderModel,
        experiment::prompt::{PromptEdit, PromptExperimentConfig, PromptIntervention, campaign::PromptCampaignBudget},
        sampling::monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}},
        model::learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy,
            replay::archive::MAX_FIT_ARCHIVE_BYTES}},
};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::{BTreeMap, BTreeSet}, fs, path::{Path, PathBuf}};

pub(super) fn parse(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
pub(super) fn integer(value: u64) -> Json { parse(value.to_string().as_bytes()) }
pub(super) fn at<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(value, |value, key| value.get(key).unwrap())
}
pub(super) fn set(value: &mut Json, keys: &[&str], replacement: Json) {
    let (key, rest) = keys.split_first().unwrap();
    let Json::Object(fields) = value else { panic!("expected object"); };
    if rest.is_empty() { *fields.get_mut(*key).unwrap() = replacement; }
    else { set(fields.get_mut(*key).unwrap(), rest, replacement); }
}
pub(super) fn encode(value: &Json) -> String {
    match value {
        Json::Null => "null".into(), Json::Bool(value) => value.to_string(),
        Json::Number(value) => value.lexeme().to_owned(),
        Json::String(value) => {
            let mut text = String::from("\"");
            for character in value.chars() {
                match character {
                    '"' => text.push_str("\\\""), '\\' => text.push_str("\\\\"),
                    '\n' => text.push_str("\\n"), '\r' => text.push_str("\\r"), '\t' => text.push_str("\\t"),
                    value if value < ' ' => text.push_str(&format!("\\u{:04x}", value as u32)),
                    value => text.push(value),
                }
            }
            text.push('"'); text
        }
        Json::Array(values) => format!("[{}]", values.iter().map(encode).collect::<Vec<_>>().join(",")),
        Json::Object(fields) => format!("{{{}}}", fields.iter().map(|(key, value)|
            format!("{}:{}", encode(&Json::String(key.clone())), encode(value))).collect::<Vec<_>>().join(",")),
    }
}

pub(super) struct Case { pub path: PathBuf, pub plan: Json }
impl Case {
    pub(super) fn new(root: &Root) -> Self {
        let recipe = fixture::write(root, b"allow");
        let mut weights = fs::read(root.0.join("weights.safetensors")).unwrap();
        tensor(&mut weights, "model.layers.0.self_attn.q_proj.weight", &[1.0, 0.0, 0.0, 1.0]);
        tensor(&mut weights, "model.layers.0.self_attn.o_proj.weight", &[0.25, 0.0, 0.0, 0.25]);
        fs::write(root.0.join("weights.safetensors"), weights).unwrap();
        refit(root);
        // The experiment does not provision native reviewer execution assets.
        for entry in fs::read_dir(&root.0).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name(); let name = name.to_str().unwrap();
            if name.starts_with("helper-") || name == "native-roster.json" {
                fs::remove_file(entry.path()).unwrap();
            }
        }
        let Json::Object(mut old) = parse(&fs::read(recipe).unwrap()) else { unreachable!() };
        let Json::Object(mut model) = old.remove("model").unwrap() else { unreachable!() };
        model.remove("monitor_generation");
        let Json::Object(mut files) = old.remove("files").unwrap() else { unreachable!() };
        files.remove("native_roster");
        let Json::Object(mut generation) = old.remove("text").unwrap() else { unreachable!() };
        generation.remove("completion"); generation.remove("max_output_bytes");
        let budget = budget(PromptCampaignBudget::default());
        let plan = Json::Object(BTreeMap::from([
            ("schema".into(), Json::String("fa.learned-prompt-counterfactual/1".into())),
            ("campaign".into(), integer(71)), ("intervention".into(), integer(72)),
            ("runtime_ms".into(), integer(60_000)), ("asset_bytes".into(), integer(4_194_304)),
            ("model".into(), Json::Object(model)), ("files".into(), Json::Object(files)),
            ("generation".into(), Json::Object(generation)),
            ("edit".into(), parse(br#"{"start":1,"expected":[121],"replacement":[65]}"#)),
            ("schedule".into(), parse(br#"{"seeds":[0,7,18446744073709551615],"outcome_tokens":[65]}"#)),
            ("budget".into(), budget),
        ]));
        let case = Self { path: root.0.join("prompt-experiment.json"), plan };
        case.write(); case
    }
    pub(super) fn write(&self) { fs::write(&self.path, encode(&self.plan)).unwrap(); }
    pub(super) fn args(&self) -> Vec<String> {
        vec!["compare-learned-prompt".into(), self.path.display().to_string()]
    }
}

pub(super) fn model(root: &Root) -> DecoderModel {
    DecoderModel::from_llama_safetensors(models::identity(9), 16,
        &fs::read(root.0.join("model.json")).unwrap(),
        &fs::read(root.0.join("weights.safetensors")).unwrap()).unwrap().0
}
pub(super) fn original_plan(root: &Root, sham: bool) -> PromptIntervention {
    use super::super::super::{fit, monitor::MonitorInput};
    let model = model(root);
    let binding = fit::binding(&fs::read(root.0.join("fit-binding.json")).unwrap()).unwrap();
    let codec = fit::replay(&fs::read(root.0.join("fit.fakv")).unwrap(), &binding, MAX_FIT_ARCHIVE_BYTES).unwrap();
    let policy = MonitorInput::decode(&fs::read(root.0.join("learned-monitor.json")).unwrap()).unwrap()
        .bind(&model, codec).unwrap().policy;
    let sampling = SamplingConfig::decode(&fs::read(root.0.join("sampling.json")).unwrap(), 257).unwrap().start();
    model.prompt_intervention(72, PromptExperimentConfig { stream: 5, evaluation_origin: 201,
        original: GenerationSpec::new(vec![120, 121], 2, BTreeSet::from([256]), sampling).unwrap(),
        policy, generation: GenerationBudget { decoder_products: 1_000_000, vocabulary_scores: 1024 },
        telemetry: GenerationTelemetryBudget::default() },
        PromptEdit { start: 1, expected: vec![121], replacement: vec![if sham { 121 } else { 65 }] }).unwrap()
}

pub(super) fn budget(value: PromptCampaignBudget) -> Json {
    parse(format!(r#"{{"trials":{},"positions":{},"decoder_products":{},"vocabulary_scores":{},"outcome_comparisons":{}}}"#,
        value.trials, value.positions, value.decoder_products, value.vocabulary_scores, value.outcome_comparisons).as_bytes())
}
pub(super) fn snapshot(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() { files.extend(snapshot(&entry.path())); }
        else { files.insert(entry.path(), fs::read(entry.path()).unwrap()); }
    }
    files
}
fn tensor(bytes: &mut [u8], name: &str, values: &[f32]) {
    let length = usize::try_from(u64::from_le_bytes(bytes[..8].try_into().unwrap())).unwrap();
    let header = parse(&bytes[8..8 + length]);
    let offsets = at(&header, &[name, "data_offsets"]).as_array().unwrap();
    let start = offsets[0].as_u64().unwrap() as usize;
    let end = offsets[1].as_u64().unwrap() as usize;
    assert_eq!(end - start, values.len() * 4);
    for (index, value) in values.iter().enumerate() {
        bytes[8 + length + start + index * 4..8 + length + start + index * 4 + 4]
            .copy_from_slice(&value.to_le_bytes());
    }
}
fn refit(root: &Root) {
    let source = model(root).recompute(11, &[120, 120, 120, 120],
        DecoderBudget { scalar_products: 100_000 }).unwrap().cache_image().unwrap();
    let (_, checkpoint) = LearnedKvCodec::fit_with_checkpoint(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(101, source)]), FitBudget { source_values: 64, parameter_values: 64,
            scratch_values: 16, work_units: 10_000 }).unwrap();
    let descriptor = checkpoint.binding().sources[&101].encode().unwrap().iter()
        .map(|byte| format!("{byte:02x}")).collect::<String>();
    fs::write(root.0.join("fit-binding.json"), format!(r#"{{"schema":"fa.learned-fit-binding/1","policy":{{"id":1,"generation":1,"rank":1,"sweeps":8}},"budget":{{"source_values":64,"parameter_values":64,"scratch_values":16,"work_units":10000}},"sources":[{{"origin":101,"descriptor_hex":"{descriptor}"}}]}}"#)).unwrap();
    fs::write(root.0.join("fit.fakv"), checkpoint.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap()).unwrap();
}
