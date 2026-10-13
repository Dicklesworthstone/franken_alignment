//! Explicit finite experiment registration, not a live policy or action patch.
use super::*;
use crate::config::{hex, read_regular};
use fa_reference::action::ResolvedTarget;
use fa_reference::action::consequence::experiment::{MAX_INTERVENTIONS, MAX_INTERVENTION_BYTES, MAX_SEARCH_CANDIDATES};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, path::Path};

// Encoding allowance only: native candidate, byte and search bounds are unchanged.
pub(super) const MENU_BYTES: usize = 256 * 1024;

pub(crate) struct Menu {
    pub scope: InterventionScope,
    pub id: u64,
    pub stop_after_cases: usize,
    pub candidates: Vec<Intervention>,
    pub budget: ProposalSearchBudget,
}
impl Menu {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, MENU_BYTES)?)
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut root = object(strict_json::parse(bytes, Limits { max_bytes: MENU_BYTES,
            max_depth: 6, max_items: 512, max_string_bytes: 2 * MAX_INTERVENTION_BYTES }).map_err(debug)?)?;
        if take(&mut root, "schema_version")?.as_str() != Some("fa.proposal-search/1") {
            return Err("unsupported proposal search menu".into());
        }
        let mut scope = object(take(&mut root, "scope")?)?;
        let id = number(&mut scope, "id")?;
        let payload = boolean(&mut scope, "payload")?;
        let target = boolean(&mut scope, "target")?;
        let units = boolean(&mut scope, "units")?;
        let keys = array(take(&mut scope, "keys")?)?;
        if keys.len() > MAX_INTERVENTIONS { return Err("too many registered keys".into()); }
        let keys = keys.iter().map(|key| key.as_u64().ok_or_else(|| "registered key must be unsigned".to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        end(&scope)?;
        let registered = InterventionScope::new(id, payload, target, units, &keys).map_err(debug)?;
        let mut budget = object(take(&mut root, "budget")?)?;
        let budget_value = ProposalSearchBudget { cases: size(&mut budget, "cases")?,
            retained_edit_bytes: size(&mut budget, "retained_edit_bytes")? };
        end(&budget)?;
        if budget_value.cases == 0 || budget_value.cases > MAX_PROPOSAL_SEARCH_CASES
            || budget_value.retained_edit_bytes > MAX_PROPOSAL_SEARCH_EDIT_BYTES {
            return Err("search budget is outside native bounds".into());
        }
        let stop_after_cases = root.remove("stop_after_cases").map(|value| {
            let count = value.as_u64().ok_or("stop_after_cases must be unsigned")?;
            usize::try_from(count).map_err(debug)
        }).transpose()?.unwrap_or(MAX_PROPOSAL_SEARCH_CASES);
        if stop_after_cases > MAX_PROPOSAL_SEARCH_CASES { return Err("stop_after_cases exceeds native case bound".into()); }
        let raw = array(take(&mut root, "candidates")?)?;
        end(&root)?;
        if raw.len() > MAX_SEARCH_CANDIDATES { return Err("too many search candidates".into()); }
        let mut retained = 0;
        let mut candidates = Vec::with_capacity(raw.len());
        for entry in raw {
            let mut fields = object(entry)?;
            let kind = take(&mut fields, "kind")?;
            let candidate = match kind.as_str() {
                Some("payload") if payload => Intervention::Payload(bytes_field(&mut fields, "bytes_hex", &mut retained)?),
                Some("units") if units => Intervention::Units(number(&mut fields, "value")?),
                Some("target") if target => {
                    let value = ResolvedTarget { adapter: number(&mut fields, "adapter")?, object: number(&mut fields, "object")?,
                        contract_version: number(&mut fields, "contract_version")?, expected_version: number(&mut fields, "expected_version")?,
                        generation: number(&mut fields, "generation")? };
                    if [value.adapter, value.object, value.contract_version, value.expected_version, value.generation].contains(&0) {
                        return Err("target identity must be nonzero".into());
                    }
                    Intervention::Target(value)
                }
                Some("key") => {
                    let key = number(&mut fields, "key")?;
                    if !keys.contains(&key) { return Err("candidate key is outside the registered scope".into()); }
                    // Null means exactly observed absence. Missing is never a wildcard.
                    Intervention::Key { key,
                        expected: optional_bytes(take(&mut fields, "expected_hex")?, &mut retained)?,
                        replacement: optional_bytes(take(&mut fields, "replacement_hex")?, &mut retained)? }
                }
                _ => return Err("candidate kind is unsupported or outside the registered scope".into()),
            };
            end(&fields)?;
            candidates.push(candidate);
        }
        Ok(Self { scope: registered, id, stop_after_cases, candidates, budget: budget_value })
    }
}
fn object(value: Json) -> Result<BTreeMap<String, Json>, String> {
    match value { Json::Object(value) => Ok(value), _ => Err("search menu requires an object".into()) }
}
fn array(value: Json) -> Result<Vec<Json>, String> {
    match value { Json::Array(value) => Ok(value), _ => Err("search menu requires an array".into()) }
}
fn take(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<Json, String> {
    fields.remove(name).ok_or_else(|| format!("search menu missing {name}"))
}
fn end(fields: &BTreeMap<String, Json>) -> Result<(), String> {
    if fields.is_empty() { Ok(()) } else { Err("unknown search menu fields".into()) }
}
fn number(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<u64, String> {
    take(fields, name)?.as_u64().ok_or_else(|| format!("search {name} must be unsigned"))
}
fn size(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<usize, String> {
    usize::try_from(number(fields, name)?).map_err(debug)
}
fn boolean(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<bool, String> {
    take(fields, name)?.as_bool().ok_or_else(|| format!("search {name} must be boolean"))
}
fn bytes_field(fields: &mut BTreeMap<String, Json>, name: &str, retained: &mut usize) -> Result<Vec<u8>, String> {
    decode_bytes(take(fields, name)?, retained)
}
fn optional_bytes(value: Json, retained: &mut usize) -> Result<Option<Vec<u8>>, String> {
    if value.is_null() { Ok(None) } else { decode_bytes(value, retained).map(Some) }
}
fn decode_bytes(value: Json, retained: &mut usize) -> Result<Vec<u8>, String> {
    let text = value.as_str().ok_or("candidate bytes must be lowercase hex or explicit null where allowed")?;
    let count = text.len().checked_div(2).and_then(|n| retained.checked_add(n)).ok_or("candidate byte overflow")?;
    if count > MAX_INTERVENTION_BYTES { return Err("candidate bytes exceed the native menu allowance".into()); }
    let result = hex(text)?;
    *retained = count;
    Ok(result)
}
