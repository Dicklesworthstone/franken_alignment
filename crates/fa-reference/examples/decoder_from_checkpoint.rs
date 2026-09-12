//! Offline numerical inference from explicit config, SafeTensors and original IDs.
//! No tokenizer, remote code, downloads, effect adapters or production permission.

use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderModel, MAX_DECODER_PRODUCTS,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::pretrained::CheckpointFileLimits;
use fa_reference::action::consequence::activation::tensor::kv::MAX_KV_POSITIONS;
use fa_reference::strict_json::{self, Limits};
use std::fs::{self, File};
use std::io::{self, Read};
use std::process::ExitCode;

const MAX_REQUEST_BYTES: usize = 65_536;
const USAGE: &str = "decoder_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS TOKEN_REQUEST_JSON CONTEXT NEW_TOKENS PRODUCT_BUDGET";
fn invalid(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, message.into()) }
fn numerical(error: impl std::fmt::Debug) -> io::Error { invalid(format!("numerical refusal: {error:?}")) }

fn request(bytes: &[u8]) -> io::Result<(DecoderIdentity, Vec<u32>)> {
    let json = strict_json::parse(bytes, Limits { max_bytes: MAX_REQUEST_BYTES, max_depth: 3,
        max_items: MAX_KV_POSITIONS + 32, max_string_bytes: 128 }).map_err(|error| invalid(error.to_string()))?;
    let root = json.as_object().ok_or_else(|| invalid("request must be an object"))?;
    if root.len() != 2 || !root.contains_key("identity") || !root.contains_key("tokens") { return Err(invalid("request fields: identity, tokens")); }
    let ids = root["identity"].as_object().ok_or_else(|| invalid("identity must be an object"))?;
    let names = ["tenant", "model", "model_generation", "tokenizer_generation", "profile_generation"];
    if ids.len() != names.len() || names.iter().any(|name| !ids.contains_key(*name)) { return Err(invalid("identity inventory mismatch")); }
    let get = |name: &str| ids[name].as_u64().filter(|value| *value > 0).ok_or_else(|| invalid(format!("invalid identity: {name}")));
    let identity = DecoderIdentity { tenant: get("tenant")?, model: get("model")?, model_generation: get("model_generation")?,
        tokenizer_generation: get("tokenizer_generation")?, profile_generation: get("profile_generation")? };
    let values = root["tokens"].as_array().ok_or_else(|| invalid("tokens must be an array of original IDs"))?;
    if values.is_empty() || values.len() > MAX_KV_POSITIONS { return Err(invalid("token count outside reference context bound")); }
    let tokens = values.iter().map(|value| value.as_u64().and_then(|id| u32::try_from(id).ok())
        .ok_or_else(|| invalid("token IDs must be unsigned 32-bit integers"))).collect::<io::Result<Vec<_>>>()?;
    Ok((identity, tokens))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).take(7).collect();
    if args.len() != 6 { return Err(invalid(USAGE).into()); }
    let number = |index: usize| -> io::Result<u64> {
        let text = args[index].to_str().ok_or_else(|| invalid(USAGE))?;
        if text.is_empty() || text.len() > 20 || !text.bytes().all(|byte| byte.is_ascii_digit()) { return Err(invalid(USAGE)); }
        text.parse().map_err(|_| invalid(USAGE))
    };
    let context = usize::try_from(number(3)?).map_err(numerical)?;
    let generated = usize::try_from(number(4)?).map_err(numerical)?;
    let products = number(5)?;
    if context == 0 || context > MAX_KV_POSITIONS || products == 0 || products > MAX_DECODER_PRODUCTS { return Err(invalid("execution budget outside reference limits").into()); }
    let meta = fs::symlink_metadata(&args[2])?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_REQUEST_BYTES as u64 { return Err(invalid("token request must be a bounded regular file").into()); }
    let file = File::open(&args[2])?;
    if !file.metadata()?.is_file() { return Err(invalid("token request is not a regular file").into()); }
    let mut bytes = Vec::new(); file.take(MAX_REQUEST_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    let (identity, tokens) = request(&bytes)?;
    let count = tokens.len().checked_add(generated).ok_or_else(|| invalid("token count overflow"))?;
    if count > context { return Err(invalid("prefix plus requested continuation exceeds context").into()); }
    let (model, receipt) = DecoderModel::from_llama_files(identity, context, &args[0], &args[1], CheckpointFileLimits::default())?;
    // Admit the COMPLETE prefix and continuation before computing either. The
    // per-token calls below spend parts of this estimate, not renewed allowances.
    let estimate = model.estimate(0, count).map_err(numerical)?.scalar_products().map_err(numerical)?;
    if estimate > products { return Err(invalid("complete run exceeds product budget").into()); }
    let mut session = model.recompute(1, &tokens, DecoderBudget { scalar_products: products }).map_err(numerical)?;
    for _ in 0..generated {
        let position = session.position();
        let terms = model.estimate(position as usize, 1).map_err(numerical)?.scalar_products().map_err(numerical)?;
        session.advance_greedy(position, DecoderBudget { scalar_products: terms }).map_err(numerical)?;
    }
    // No partial output is printed on a later numerical failure. EOS/BOS fields
    // are metadata only; the requested original IDs and number of steps govern.
    println!("{{\"parameters\":{},\"loaded_data_bytes\":{},\"prefix_tokens\":{},\"generated_ids\":{:?},\"next_token_id\":{},\"scalar_products\":{},\"permission\":\"not_issued\"}}",
        model.profile().parameter_count(), receipt.weights.data_bytes, tokens.len(), &session.tokens()[tokens.len()..],
        session.greedy_token().map_err(numerical)?, session.work().scalar_products().map_err(numerical)?);
    Ok(())
}
fn main() -> ExitCode {
    match run() { Ok(()) => ExitCode::SUCCESS, Err(error) => { eprintln!("{error}"); ExitCode::FAILURE } }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PREFIX: &str = r#"{"identity":{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5},"tokens":"#;
    #[test]
    fn request_preserves_original_ids_and_explicit_identity() {
        let (id, tokens) = request(format!("{PREFIX}[1,4294967295,0]}}").as_bytes()).unwrap();
        assert_eq!(id.model, 2); assert_eq!(tokens, vec![1, u32::MAX, 0]);
    }
    #[test]
    fn ambiguous_ids_duplicate_fields_and_extra_capabilities_refuse() {
        for suffix in [r#"[1.0]}"#, r#"["1"]}"#, r#"[-1]}"#, r#"[4294967296]}"#, r#"[1],"tokens":[2]}"#, r#"[1],"permit":true}"#] {
            assert!(request(format!("{PREFIX}{suffix}").as_bytes()).is_err());
        }
    }
    #[test]
    fn request_limits_pair_maximum_context_with_one_over() {
        let values = vec!["0"; MAX_KV_POSITIONS];
        assert_eq!(request(format!("{PREFIX}[{}]}}", values.join(",")).as_bytes()).unwrap().1.len(), MAX_KV_POSITIONS);
        assert!(request(format!("{PREFIX}[{},0]}}", values.join(",")).as_bytes()).is_err());
        assert!(request(&vec![b' '; MAX_REQUEST_BYTES + 1]).is_err());
    }
}
