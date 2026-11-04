//! Exact serialized BOS template admission, with no generic template evaluator.
//! Hugging Face tokenizers serializes Sequence/SpecialToken pieces with explicit
//! type IDs. Its Llama converter emits ByteLevel(trim_offsets=false) before this
//! template; the other ByteLevel flags are not consulted by postprocessing.

use super::{BYTE_LEVEL_FIELDS, Error, Json, exact_object, require_false, required, token_id};
use std::collections::BTreeMap;

pub(super) fn bos_token(value: &Json, names: &BTreeMap<u32, Vec<u8>>) -> Result<u32, Error> {
    // Null is a valid declaration of a different input profile, not this one.
    if value.is_null() { return Err(Error::Binding); }
    let root = value.as_object().ok_or(Error::InvalidInput)?;
    let template = match required(root, "type")?.as_str() {
        Some("TemplateProcessing") => value,
        Some("Sequence") => {
            let sequence = exact_object(value, &["type", "processors"])?;
            let steps = required(sequence, "processors")?.as_array().ok_or(Error::InvalidInput)?;
            if steps.len() != 2 { return Err(Error::Binding); }
            let byte_level = exact_object(&steps[0], BYTE_LEVEL_FIELDS)?;
            if required(byte_level, "type")?.as_str() != Some("ByteLevel") {
                return Err(Error::Binding);
            }
            require_false(required(byte_level, "trim_offsets")?)?;
            for flag in ["add_prefix_space", "use_regex"] {
                required(byte_level, flag)?.as_bool().ok_or(Error::InvalidInput)?;
            }
            &steps[1]
        }
        _ => return Err(Error::Binding),
    };
    let template = exact_object(template, &["type", "single", "pair", "special_tokens"])?;
    if required(template, "type")?.as_str() != Some("TemplateProcessing") {
        return Err(Error::Binding);
    }
    let registered = required(template, "special_tokens")?.as_object().ok_or(Error::InvalidInput)?;
    if registered.len() != 1 { return Err(Error::Binding); }
    let (name, record) = registered.iter().next().ok_or(Error::Incomplete)?;
    let record = exact_object(record, &["id", "ids", "tokens"])?;
    if required(record, "id")?.as_str() != Some(name.as_str()) { return Err(Error::Binding); }
    let ids = required(record, "ids")?.as_array().ok_or(Error::InvalidInput)?;
    let tokens = required(record, "tokens")?.as_array().ok_or(Error::InvalidInput)?;
    if ids.len() != 1 || tokens.len() != 1 { return Err(Error::Binding); }
    let bos = token_id(&ids[0])?;
    if tokens[0].as_str() != Some(name.as_str())
        || names.get(&bos).map(Vec::as_slice) != Some(name.as_bytes())
    { return Err(Error::Binding); }

    let single = required(template, "single")?.as_array().ok_or(Error::InvalidInput)?;
    if single.len() != 2 { return Err(Error::Binding); }
    piece(&single[0], "SpecialToken", name, 0)?;
    piece(&single[1], "Sequence", "A", 0)?;
    let pair = required(template, "pair")?.as_array().ok_or(Error::InvalidInput)?;
    if pair.len() != 4 { return Err(Error::Binding); }
    piece(&pair[0], "SpecialToken", name, 0)?;
    piece(&pair[1], "Sequence", "A", 0)?;
    piece(&pair[2], "SpecialToken", name, 1)?;
    piece(&pair[3], "Sequence", "B", 1)?;
    Ok(bos)
}

fn piece(value: &Json, kind: &str, id: &str, type_id: u64) -> Result<(), Error> {
    let outer = exact_object(value, &[kind])?;
    let inner = exact_object(required(outer, kind)?, &["id", "type_id"])?;
    if required(inner, "id")?.as_str() != Some(id)
        || required(inner, "type_id")?.as_u64().ok_or(Error::InvalidInput)? != type_id
    { return Err(Error::Binding); }
    Ok(())
}
