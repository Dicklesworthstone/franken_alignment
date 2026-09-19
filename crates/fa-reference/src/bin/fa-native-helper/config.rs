//! Strict, bounded operator configuration. This file is never model input.
//! No path, salt, profile or runtime option is accepted from a helper request.
use fa_reference::action::consequence::oversight::helper_client::native::{NativeEvaluator, NativeHelperPolicy};
use fa_reference::action::consequence::oversight::helper_client::native::process::{
    NativeProcessBudget, NativeProcessError, NativeProcessReport, inherited_worker_socket, run_native_worker,
};
use fa_reference::action::consequence::oversight::helper_client::native::peer::MIN_NATIVE_SALT_BYTES;
use fa_reference::action::consequence::oversight::helper_client::native::bootstrap::files::{
    NativeAssetReadBudget, NativeFileBootstrap, NativeFileBootstrapError, NativeHelperFiles, NativeHelperFileLimits,
};
use fa_reference::action::consequence::oversight::helper_workers::MAX_WORKER_SALT_BYTES;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::reader::WeightReadBudget;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{GenerationBudget, MAX_STOP_TOKENS};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::TokenizationBudget;
use fa_reference::full_input::{InputProfileBinding, MAX_PROFILE_BYTES};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::Error;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_MANIFEST_BYTES: usize = 65_536;
const MAX_FILE_READS: usize = 1024;
#[derive(Debug)]
pub enum LaunchError {
    Usage,
    Manifest,
    Field(&'static str),
    Contract(Error),
    File(io::ErrorKind),
    NotRegular,
    Limit,
    Deadline,
    Socket(NativeProcessError),
    Startup(NativeFileBootstrapError),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage => f.write_str("usage: fa-native-helper /absolute/worker.json (private socket on stdin)"),
            Self::Manifest => f.write_str("invalid bounded worker manifest"),
            Self::Field(field) => write!(f, "invalid manifest field: {field}"),
            Self::Contract(error) => write!(f, "refused configuration: {error:?}"),
            Self::File(kind) => write!(f, "operator file I/O: {kind:?}"),
            Self::NotRegular => f.write_str("operator input must be a regular non-symlink file"),
            Self::Limit => f.write_str("startup input or work limit exceeded"),
            Self::Deadline => f.write_str("worker lifetime elapsed during startup"),
            Self::Socket(error) => write!(f, "worker socket: {error}"),
            Self::Startup(error) => write!(f, "native startup: {error}"),
        }
    }
}

struct Manifest {
    policy: NativeHelperPolicy,
    files: [PathBuf; 5],
    stream: u64,
    salt_file: PathBuf,
    milliseconds: u64,
    steps: usize,
    asset_bytes: usize,
    asset_calls: usize,
    weight_bytes: usize,
    weight_calls: usize,
}
impl Manifest {
    fn parse(bytes: &[u8]) -> Result<Self, LaunchError> {
        let json = strict_json::parse(bytes, Limits { max_bytes: MAX_MANIFEST_BYTES,
            max_depth: 5, max_items: 4096, max_string_bytes: MAX_PROFILE_BYTES * 2 })
            .map_err(|_| LaunchError::Manifest)?;
        let r = object(&json, &["schema", "input", "decoder", "policy", "files", "stream",
            "salt_file", "lifetime", "startup"], "$")?;
        if r["schema"].as_str() != Some("fa.native-worker/1") { return Err(LaunchError::Field("schema")); }
        let input = object(&r["input"], &["id", "bytes_hex", "model_epoch", "tokenizer_epoch", "policy_epoch"], "input")?;
        let profile = InputProfileBinding { profile_id: u64_field(input, "id")?,
            profile_bytes: hex(&input["bytes_hex"])?, model_epoch: u64_field(input, "model_epoch")?,
            tokenizer_epoch: u64_field(input, "tokenizer_epoch")?, policy_epoch: u64_field(input, "policy_epoch")? };
        let decoder = object(&r["decoder"], &["identity", "shape", "epsilon", "theta"], "decoder")?;
        let id = object(&decoder["identity"], &["tenant", "model", "model_generation", "tokenizer_generation", "profile_generation"], "identity")?;
        let shape = object(&decoder["shape"], &["vocabulary", "hidden", "intermediate", "layers", "query_heads", "cache_heads", "context"], "shape")?;
        let decoder_profile = DecoderProfile::new(DecoderIdentity {
            tenant: u64_field(id, "tenant")?, model: u64_field(id, "model")?, model_generation: u64_field(id, "model_generation")?,
            tokenizer_generation: u64_field(id, "tokenizer_generation")?, profile_generation: u64_field(id, "profile_generation")?,
        }, DecoderShape { vocabulary: count(shape, "vocabulary")?, hidden: count(shape, "hidden")?,
            intermediate: count(shape, "intermediate")?, layers: count(shape, "layers")?,
            query_heads: count(shape, "query_heads")?, cache_heads: count(shape, "cache_heads")?, context: count(shape, "context")?,
        }, scalar(&decoder["epsilon"], "epsilon")?, scalar(&decoder["theta"], "theta")?).map_err(LaunchError::Contract)?;
        let p = object(&r["policy"], &["max_new_tokens", "stop_tokens", "max_output_bytes", "tokenization", "generation"], "policy")?;
        let tokens = p["stop_tokens"].as_array().ok_or(LaunchError::Field("stop_tokens"))?;
        if tokens.len() > MAX_STOP_TOKENS { return Err(LaunchError::Limit); }
        let stop_tokens = tokens.iter().map(|value| value.as_u64().and_then(|n| u32::try_from(n).ok())
            .ok_or(LaunchError::Field("stop_tokens"))).collect::<Result<Vec<_>, _>>()?;
        let t = object(&p["tokenization"], &["input_bytes", "pair_lookups", "heap_pops"], "tokenization")?;
        let g = object(&p["generation"], &["scalar_products", "sampling_entries"], "generation")?;
        let policy = NativeHelperPolicy { input_profile: profile, decoder_profile,
            max_new_tokens: count(p, "max_new_tokens")?, stop_tokens, max_output_bytes: count(p, "max_output_bytes")?,
            tokenization: TokenizationBudget { input_bytes: count(t, "input_bytes")?, pair_lookups: count(t, "pair_lookups")?, heap_pops: count(t, "heap_pops")? },
            generation: GenerationBudget { scalar_products: u64_field(g, "scalar_products")?, sampling_entries: u64_field(g, "sampling_entries")? } };
        let files = object(&r["files"], &["configuration", "tokenizer", "monitoring", "sampling", "weights"], "files")?;
        let l = object(&r["lifetime"], &["milliseconds", "steps"], "lifetime")?;
        let startup = object(&r["startup"], &["asset_bytes", "asset_calls", "weight_bytes", "weight_calls"], "startup")?;
        let result = Self { policy, files: [path(&files["configuration"], "configuration")?,
            path(&files["tokenizer"], "tokenizer")?, path(&files["monitoring"], "monitoring")?,
            path(&files["sampling"], "sampling")?, path(&files["weights"], "weights")?],
            stream: u64_field(r, "stream")?, salt_file: path(&r["salt_file"], "salt_file")?,
            milliseconds: u64_field(l, "milliseconds")?, steps: count(l, "steps")?,
            asset_bytes: count(startup, "asset_bytes")?, asset_calls: count(startup, "asset_calls")?,
            weight_bytes: count(startup, "weight_bytes")?, weight_calls: count(startup, "weight_calls")? };
        if result.stream == 0 { return Err(LaunchError::Field("stream")); }
        Ok(result)
    }
    fn bootstrap(&self) -> NativeFileBootstrap<'_> {
        NativeFileBootstrap { policy: &self.policy, stream: self.stream,
            files: NativeHelperFiles { configuration: &self.files[0], tokenizer: &self.files[1],
                monitoring: &self.files[2], sampling: &self.files[3], weights: &self.files[4] },
            limits: NativeHelperFileLimits::default() }
    }
}

pub fn run(args: Vec<OsString>) -> Result<NativeProcessReport, LaunchError> {
    if args.len() != 1 { return Err(LaunchError::Usage); }
    let manifest_path = Path::new(&args[0]);
    if !manifest_path.is_absolute() { return Err(LaunchError::Usage); }
    // Reject pipes/regular stdin before reading startup assets. No protocol I/O.
    let socket = inherited_worker_socket().map_err(LaunchError::Socket)?;
    let manifest = Manifest::parse(&read_regular(manifest_path, MAX_MANIFEST_BYTES)?)?;
    // This one budget covers salt/assets/weights and protocol; never reset it.
    let lifetime = NativeProcessBudget::new(manifest.milliseconds, manifest.steps).map_err(LaunchError::Contract)?;
    let mut assets = NativeAssetReadBudget::new(manifest.asset_bytes, manifest.asset_calls).map_err(LaunchError::Contract)?;
    let mut weights = WeightReadBudget::new(manifest.weight_bytes, manifest.weight_calls).map_err(|_| LaunchError::Limit)?;
    let salt = read_regular(&manifest.salt_file, MAX_WORKER_SALT_BYTES)?;
    if salt.len() < MIN_NATIVE_SALT_BYTES { return Err(LaunchError::Limit); }
    if lifetime.expired() { return Err(LaunchError::Deadline); }
    let (evaluator, _) = NativeEvaluator::from_llama_files(manifest.bootstrap(), &mut assets, &mut weights)
        .map_err(LaunchError::Startup)?;
    run_native_worker(socket, evaluator, salt, lifetime).map_err(LaunchError::Socket)
}

// Local operator files, not race-free confinement or an atomic multi-file cut.
// Both final-component checks reject symlinks/devices, with bounds before buffers.
fn read_regular(path: &Path, maximum: usize) -> Result<Vec<u8>, LaunchError> {
    let metadata = fs::symlink_metadata(path).map_err(|e| LaunchError::File(e.kind()))?;
    if !metadata.file_type().is_file() { return Err(LaunchError::NotRegular); }
    if metadata.len() > maximum as u64 { return Err(LaunchError::Limit); }
    let mut source = File::open(path).map_err(|e| LaunchError::File(e.kind()))?;
    let metadata = source.metadata().map_err(|e| LaunchError::File(e.kind()))?;
    if !metadata.is_file() { return Err(LaunchError::NotRegular); }
    if metadata.len() > maximum as u64 { return Err(LaunchError::Limit); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(maximum + 1).map_err(|_| LaunchError::Limit)?;
    let mut buffer = [0; 4096];
    for _ in 0..MAX_FILE_READS {
        let offered = buffer.len().min(maximum - bytes.len() + 1);
        let count = match source.read(&mut buffer[..offered]) {
            Ok(0) => return Ok(bytes),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(LaunchError::File(e.kind())),
        };
        if count > maximum - bytes.len() { return Err(LaunchError::Limit); }
        bytes.extend_from_slice(&buffer[..count]);
    }
    Err(LaunchError::Limit)
}
fn object<'a>(value: &'a Json, fields: &[&str], label: &'static str) -> Result<&'a BTreeMap<String, Json>, LaunchError> {
    let object = value.as_object().ok_or(LaunchError::Field(label))?;
    if object.len() != fields.len() || fields.iter().any(|key| !object.contains_key(*key)) {
        return Err(LaunchError::Field(label));
    }
    Ok(object)
}
fn u64_field(object: &BTreeMap<String, Json>, field: &'static str) -> Result<u64, LaunchError> {
    object[field].as_u64().ok_or(LaunchError::Field(field))
}
fn count(object: &BTreeMap<String, Json>, field: &'static str) -> Result<usize, LaunchError> {
    usize::try_from(u64_field(object, field)?).map_err(|_| LaunchError::Limit)
}
fn scalar(value: &Json, field: &'static str) -> Result<f64, LaunchError> {
    let number = match value { Json::Number(n) => n.lexeme().parse::<f64>().ok(), _ => None };
    number.filter(|n| n.is_finite()).ok_or(LaunchError::Field(field))
}
fn path(value: &Json, field: &'static str) -> Result<PathBuf, LaunchError> {
    let name = value.as_str().ok_or(LaunchError::Field(field))?;
    if name.len() > 4096 { return Err(LaunchError::Limit); }
    if name.chars().any(char::is_control) || !Path::new(name).is_absolute() {
        return Err(LaunchError::Field(field));
    }
    Ok(PathBuf::from(name))
}
fn hex(value: &Json) -> Result<Vec<u8>, LaunchError> {
    let text = value.as_str().ok_or(LaunchError::Field("bytes_hex"))?;
    if text.len() > 2 * MAX_PROFILE_BYTES { return Err(LaunchError::Limit); }
    if !text.len().is_multiple_of(2) { return Err(LaunchError::Field("bytes_hex")); }
    let digit = |b: u8| match b {
        b'0'..=b'9' => Ok(b - b'0'), b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(LaunchError::Field("bytes_hex")),
    };
    text.as_bytes().chunks_exact(2).map(|pair| Ok(digit(pair[0])? * 16 + digit(pair[1])?)).collect()
}

#[cfg(test)]
mod tests;
