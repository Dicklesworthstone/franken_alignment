use super::{PolicyReplayClientError, Result};
use crate::strict_json::Json;
use std::collections::BTreeMap;

pub(super) fn ensure(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(PolicyReplayClientError::InvalidResponse)
    }
}

pub(super) fn object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a BTreeMap<String, Json>> {
    let value = value
        .as_object()
        .ok_or(PolicyReplayClientError::InvalidResponse)?;
    ensure(value.len() == fields.len() && fields.iter().all(|field| value.contains_key(*field)))?;
    Ok(value)
}

pub(super) fn text(value: &Json) -> Result<&str> {
    value
        .as_str()
        .ok_or(PolicyReplayClientError::InvalidResponse)
}
pub(super) fn boolean(value: &Json) -> Result<bool> {
    value
        .as_bool()
        .ok_or(PolicyReplayClientError::InvalidResponse)
}
pub(super) fn array(value: &Json) -> Result<&[Json]> {
    value
        .as_array()
        .ok_or(PolicyReplayClientError::InvalidResponse)
}

pub(super) fn number(value: &Json) -> Result<u64> {
    let raw = text(value)?;
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| PolicyReplayClientError::InvalidResponse)?;
    ensure(parsed.to_string() == raw)?;
    Ok(parsed)
}

pub(super) fn natural(value: &Json) -> Result<usize> {
    usize::try_from(number(value)?).map_err(|_| PolicyReplayClientError::InvalidResponse)
}

pub(super) fn optional_number(value: &Json) -> Result<Option<u64>> {
    if value.is_null() {
        Ok(None)
    } else {
        number(value).map(Some)
    }
}
