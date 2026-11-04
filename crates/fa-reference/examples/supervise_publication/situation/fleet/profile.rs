//! Closed read-only roster, separate from mutating shutdown coordinator profiles.
use crate::config::debug;
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::{
    MAX_SITUATION_PAGE_ATTEMPTS, fleet::{MAX_FLEET_SITUATION_MEMBERS, MAX_FLEET_SITUATION_EVENTS},
};
use std::{collections::{BTreeMap, BTreeSet}, path::{Component, PathBuf}, time::Duration};

pub(super) const MAX_PROFILE_BYTES: usize = 65_536;
const MAX_INPUT_BYTES: usize = 512 * 1024 * 1024;
const MAX_JOURNAL_BYTES: usize = 256 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct Member {
    pub id: u64,
    pub config: PathBuf,
    pub recipe: Option<PathBuf>,
    pub anchor: Option<PathBuf>,
}
pub(super) struct Profile {
    pub wall: Duration,
    pub input_bytes: usize,
    pub journal_bytes: usize,
    pub output_bytes: usize,
    pub events: usize,
    pub page_size: usize,
    pub members: Vec<Member>,
}
impl Profile {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: MAX_PROFILE_BYTES,
            max_depth: 4, max_items: 1024, max_string_bytes: 4096 }).map_err(debug)?;
        let mut root = object(json)?;
        if take(&mut root, "schema")?.as_str() != Some("fa.publication-situation-fleet/1") {
            return Err("unsupported situation fleet schema".into());
        }
        let wall = Duration::from_millis(bounded(&mut root, "runtime_ms", 86_400_000)? as u64);
        let input_bytes = bounded(&mut root, "input_bytes", MAX_INPUT_BYTES)?;
        let journal_bytes = bounded(&mut root, "journal_bytes", MAX_JOURNAL_BYTES)?;
        let output_bytes = bounded(&mut root, "output_bytes", MAX_OUTPUT_BYTES)?;
        let events = take(&mut root, "replay_events")?.as_u64().and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n <= MAX_FLEET_SITUATION_EVENTS).ok_or("invalid fleet replay event allowance")?;
        let page_size = bounded(&mut root, "page_size", MAX_SITUATION_PAGE_ATTEMPTS)?;
        let Json::Array(items) = take(&mut root, "members")? else { return Err("fleet members must be an array".into()); };
        end(root)?;
        if items.is_empty() || items.len() > MAX_FLEET_SITUATION_MEMBERS {
            return Err("fleet requires 1..64 declared members".into());
        }
        let mut ids = BTreeSet::new(); let mut paths = BTreeSet::new();
        let mut members = Vec::with_capacity(items.len());
        for item in items {
            let mut fields = object(item)?;
            let id = take(&mut fields, "id")?.as_u64().filter(|id| *id != 0).ok_or("invalid fleet member id")?;
            let config = path(take(&mut fields, "config")?)?;
            let recipe = optional_path(take(&mut fields, "learned_recipe")?)?;
            let anchor = optional_path(take(&mut fields, "history_anchor")?)?;
            end(fields)?;
            if !ids.insert(id) || !paths.insert(config.clone()) {
                return Err("duplicate fleet member id or configuration path".into());
            }
            #[cfg(not(target_os = "linux"))]
            if recipe.is_some() { return Err("learned fleet situations require the Linux operator profile; no fallback".into()); }
            members.push(Member { id, config, recipe, anchor });
        }
        members.sort_unstable_by_key(|member| member.id);
        Ok(Self { wall, input_bytes, journal_bytes, output_bytes, events, page_size, members })
    }
}
fn object(value: Json) -> Result<BTreeMap<String, Json>, String> {
    match value { Json::Object(fields) => Ok(fields), _ => Err("expected fleet object".into()) }
}
fn take(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<Json, String> {
    fields.remove(name).ok_or_else(|| format!("missing fleet field {name}"))
}
fn end(fields: BTreeMap<String, Json>) -> Result<(), String> {
    if fields.is_empty() { Ok(()) } else { Err("unsupported fleet field".into()) }
}
fn bounded(fields: &mut BTreeMap<String, Json>, name: &str, maximum: usize) -> Result<usize, String> {
    take(fields, name)?.as_u64().and_then(|n| usize::try_from(n).ok())
        .filter(|n| (1..=maximum).contains(n)).ok_or_else(|| format!("invalid fleet {name}"))
}
fn optional_path(value: Json) -> Result<Option<PathBuf>, String> {
    match value { Json::Null => Ok(None), other => path(other).map(Some) }
}
fn path(value: Json) -> Result<PathBuf, String> {
    let text = value.as_str().ok_or("fleet path must be text or explicit null")?;
    let path = PathBuf::from(text);
    if !path.is_absolute() || path.file_name().is_none() || text.as_bytes().contains(&0)
        || path.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_))) {
        return Err("fleet paths must be absolute normalized file paths".into());
    }
    Ok(path)
}
