//! Independent file framing for the existing two-layer K/V command fixture.
//! This helper writes real checkpoint/adapter tensors; it never loads or merges
//! them and never supplies numerical success, a probe, or a source observation.
use fa_reference::strict_json::{self, Limits};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

type Tensors = BTreeMap<String, (Vec<usize>, Vec<f32>)>;

pub(crate) struct SourceFiles {
    pub manifest: PathBuf,
    pub index: Option<PathBuf>,
    pub base_files: Vec<PathBuf>,
    pub adapter_configuration: Option<PathBuf>,
    pub adapter_weights: Option<PathBuf>,
}
impl SourceFiles {
    pub fn selected(&self, operands: &[OsString]) -> Vec<OsString> {
        let mut selected = operands.to_vec();
        selected[1] = self.manifest.clone().into_os_string();
        selected
    }
    pub fn invocation(&self, operands: &[OsString]) -> Vec<OsString> {
        let mut selected = vec![OsString::from("--checkpoint-source")];
        selected.extend(self.selected(operands));
        selected
    }
    pub fn weight_bytes(&self) -> usize {
        self.base_files.iter().chain(self.adapter_weights.iter())
            .map(|path| fs::metadata(path).unwrap().len() as usize).sum()
    }
    pub fn auxiliary_bytes(&self) -> usize {
        std::iter::once(&self.manifest).chain(self.index.iter()).chain(self.adapter_configuration.iter())
            .map(|path| fs::metadata(path).unwrap().len() as usize).sum()
    }
    pub fn remove_tensors(&self) {
        for path in self.base_files.iter().chain(self.adapter_weights.iter()) {
            fs::remove_file(path).unwrap();
        }
    }
    pub fn set_number(&self, name: &str, value: u64) {
        let text = fs::read_to_string(&self.manifest).unwrap();
        let prefix = format!("\"{name}\":");
        let start = text.find(&prefix).unwrap() + prefix.len();
        let end = start + text[start..].find(|ch: char| !ch.is_ascii_digit()).unwrap();
        let mut changed = text[..start].to_owned();
        changed.push_str(&value.to_string()); changed.push_str(&text[end..]);
        fs::write(&self.manifest, changed).unwrap();
    }
    pub fn replace(&self, before: &str, after: &str) {
        let text = fs::read_to_string(&self.manifest).unwrap();
        assert!(text.contains(before), "missing source fixture: {before}");
        fs::write(&self.manifest, text.replacen(before, after, 1)).unwrap();
    }
    pub fn remove_final_value_signal(&self) {
        let path = self.adapter_weights.as_ref().unwrap();
        let mut tensors = decode(&fs::read(path).unwrap());
        let (_, values) = tensors.get_mut("base_model.model.model.layers.1.self_attn.v_proj.lora_B.weight").unwrap();
        assert_eq!(values.as_slice(), &[0.5, 0.0, 0.0, 0.5]);
        *values = vec![-0.5, 0.0, 0.0, -0.5];
        fs::write(path, encode(&tensors)).unwrap();
    }
}

pub(crate) fn create(root: &Path, original_weights: &Path, sharded: bool, adapted: bool) -> SourceFiles {
    let mut tensors = decode(&fs::read(original_weights).unwrap());
    if adapted {
        for layer in 0..2 {
            for side in ["k_proj", "v_proj"] {
                let (shape, values) = tensors.get_mut(&format!("model.layers.{layer}.self_attn.{side}.weight")).unwrap();
                assert_eq!(shape.as_slice(), &[2, 4]);
                assert_eq!(values.as_slice(), &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
                values[0] = 0.5; values[5] = 0.5;
            }
        }
    }
    let mut files = SourceFiles { manifest: root.join("checkpoint-source.json"), index: None,
        base_files: Vec::new(), adapter_configuration: None, adapter_weights: None };
    let base = if sharded {
        let mut first = BTreeMap::new(); let mut second = BTreeMap::new();
        let mut assignments = Vec::new(); let mut scalar_bytes = 0;
        for (index, (name, value)) in tensors.into_iter().enumerate() {
            scalar_bytes += value.1.len() * 4;
            let label = if index % 2 == 0 { "part-a.safetensors" } else { "part-b.safetensors" };
            assignments.push(format!("{}:{}", quote(&name), quote(label)));
            if index % 2 == 0 { first.insert(name, value); } else { second.insert(name, value); }
        }
        let a = root.join("source-a.bin"); let b = root.join("source-b.bin");
        fs::write(&a, encode(&first)).unwrap(); fs::write(&b, encode(&second)).unwrap();
        let index = root.join("source-index.json");
        fs::write(&index, format!("{{\"metadata\":{{\"total_size\":{scalar_bytes}}},\"weight_map\":{{{}}}}}", assignments.join(","))).unwrap();
        files.base_files.extend([a.clone(), b.clone()]); files.index = Some(index.clone());
        format!("{{\"kind\":\"sharded\",\"index\":{},\"shards\":{{\"part-a.safetensors\":{},\"part-b.safetensors\":{}}}}}",
            quoted_path(&index), quoted_path(&a), quoted_path(&b))
    } else {
        let path = root.join("source-base.safetensors"); fs::write(&path, encode(&tensors)).unwrap();
        files.base_files.push(path.clone());
        format!("{{\"kind\":\"single\",\"path\":{}}}", quoted_path(&path))
    };
    let adapter = if adapted {
        let configuration = root.join("adapter-config.json");
        fs::write(&configuration, br#"{"peft_type":"LORA","task_type":"CAUSAL_LM","inference_mode":true,"r":2,"lora_alpha":2.0,"target_modules":["k_proj","v_proj"]}"#).unwrap();
        let mut tensors = BTreeMap::new();
        for layer in 0..2 {
            for side in ["k_proj", "v_proj"] {
                let name = format!("base_model.model.model.layers.{layer}.self_attn.{side}");
                tensors.insert(format!("{name}.lora_A.weight"), (vec![2, 4], vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
                tensors.insert(format!("{name}.lora_B.weight"), (vec![2, 2], vec![0.5, 0.0, 0.0, 0.5]));
            }
        }
        let weights = root.join("adapter.safetensors"); fs::write(&weights, encode(&tensors)).unwrap();
        files.adapter_configuration = Some(configuration.clone()); files.adapter_weights = Some(weights.clone());
        format!("{{\"base_identity\":{{\"tenant\":1,\"model\":2,\"model_generation\":2,\"tokenizer_generation\":4,\"profile_generation\":4}},\"configuration\":{},\"weights\":{},\"merge_products\":64}}",
            quoted_path(&configuration), quoted_path(&weights))
    } else { "null".to_owned() };
    fs::write(&files.manifest, format!("{{\"schema\":\"fa.checkpoint-weights/1\",\"base\":{base},\"adapter\":{adapter},\"limits\":{{\"weight_bytes\":1048576,\"weight_read_calls\":10000}}}}")).unwrap();
    files
}

fn decode(bytes: &[u8]) -> Tensors {
    let header = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let parsed = strict_json::parse(&bytes[8..8 + header], Limits::default()).unwrap();
    let body = &bytes[8 + header..];
    parsed.as_object().unwrap().iter().map(|(name, value)| {
        assert_eq!(value.get("dtype").unwrap().as_str(), Some("F32"));
        let shape = value.get("shape").unwrap().as_array().unwrap().iter()
            .map(|value| value.as_u64().unwrap() as usize).collect();
        let range = value.get("data_offsets").unwrap().as_array().unwrap();
        let start = range[0].as_u64().unwrap() as usize; let end = range[1].as_u64().unwrap() as usize;
        let values = body[start..end].chunks_exact(4).map(|word| f32::from_le_bytes(word.try_into().unwrap())).collect();
        (name.clone(), (shape, values))
    }).collect()
}
fn encode(tensors: &Tensors) -> Vec<u8> {
    let mut header = String::from("{"); let mut body = Vec::new();
    for (index, (name, (shape, values))) in tensors.iter().enumerate() {
        if index != 0 { header.push(','); }
        let start = body.len(); for value in values { body.extend_from_slice(&value.to_le_bytes()); }
        write!(header, "{}:{{\"dtype\":\"F32\",\"shape\":{shape:?},\"data_offsets\":[{start},{}]}}", quote(name), body.len()).unwrap();
    }
    header.push('}'); while !header.len().is_multiple_of(8) { header.push(' '); }
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec(); bytes.extend_from_slice(header.as_bytes()); bytes.extend_from_slice(&body); bytes
}
fn quoted_path(path: &Path) -> String { quote(path.to_str().unwrap()) }
fn quote(text: &str) -> String { format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")) }
