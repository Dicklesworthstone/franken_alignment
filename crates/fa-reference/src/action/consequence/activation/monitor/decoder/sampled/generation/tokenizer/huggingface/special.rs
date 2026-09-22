//! The admitted added-token subset is literal, non-normalized special controls.
//! Their spellings are raw UTF-8, NOT the model vocabulary's ByteLevel alphabet.

use super::{Error, Json, Object, exact_object, required, require_false, token_id, MAX_TOKEN_BYTES};
use super::super::{MAX_CONTROL_TOKENS, MAX_SPECIAL_TOKEN_BYTES};
use std::collections::{BTreeMap, BTreeSet};

const FIELDS: &[&str] = &["id", "content", "single_word", "lstrip", "rstrip", "normalized", "special"];

pub(super) fn names(added: &[Json], model: &Object, count: usize)
    -> Result<BTreeMap<u32, Vec<u8>>, Error>
{
    if added.len() > MAX_CONTROL_TOKENS { return Err(Error::Limit); }
    let mut names = BTreeMap::new();
    let mut spellings = BTreeSet::new();
    let mut retained = 0_usize;
    let mut previous = None;
    for value in added {
        let object = value.as_object().ok_or(Error::InvalidInput)?;
        // An added ID outside the independently supplied total vocabulary is a
        // binding error even when the rest of its declaration is incomplete.
        let id = token_id(required(object, "id")?)?;
        if id as usize >= count { return Err(Error::Binding); }
        let object = exact_object(value, FIELDS)?;
        if required(object, "special")?.as_bool() != Some(true) { return Err(Error::Binding); }
        for field in ["single_word", "lstrip", "rstrip", "normalized"] {
            require_false(required(object, field)?)?;
        }
        let name = required(object, "content")?.as_str().ok_or(Error::InvalidInput)?;
        if name.is_empty() { return Err(Error::InvalidInput); }
        if name.len() > MAX_TOKEN_BYTES { return Err(Error::Limit); }
        retained = retained.checked_add(name.len()).ok_or(Error::Limit)?;
        if retained > MAX_SPECIAL_TOKEN_BYTES { return Err(Error::Limit); }
        if names.contains_key(&id) || !spellings.insert(name) { return Err(Error::Duplicate); }
        // HF exports order added tokens by ID. Refuse a declaration whose
        // loading order could cause an upstream allocator to renumber tokens.
        if previous.is_some_and(|old| id < old) { return Err(Error::Binding); }
        previous = Some(id);
        match model.get(name) {
            Some(existing) if token_id(existing)? != id => return Err(Error::Binding),
            None if (id as usize) < model.len() => return Err(Error::Binding),
            _ => {}
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(name.len()).map_err(|_| Error::Limit)?;
        bytes.extend_from_slice(name.as_bytes());
        names.insert(id, bytes);
    }
    Ok(names)
}

#[cfg(test)]
mod tests;
