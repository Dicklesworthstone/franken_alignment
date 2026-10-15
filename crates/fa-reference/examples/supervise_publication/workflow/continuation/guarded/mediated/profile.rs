//! Independent retained topology plus explicit next-source selection. No graph
//! or lifetime profile is inferred from the authority journal being recovered.
use super::{Config, FileOversight, debug};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::mediated::FileTopologyRequirement, mediation::FileMediationObserver,
    mediation::source::{TopologySourceRecovery, TopologySourceWork, TOPOLOGY_REFRESH_READ_BYTES},
    driver::topology::FileDriverTopology};
use fa_reference::perimeter_inventory::{MAX_INVENTORY_BYTES,
    topology::{LoadedAuthorityTopology, MAX_TOPOLOGY_BYTES}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, fs, path::{Component, Path, PathBuf}};

const PROFILE_BYTES: usize = 16 * 1024;
struct Pair { topology: PathBuf, inventory: PathBuf }
pub(super) struct Selection {
    initial: Pair, current: Pair, available: bool, next: Pair, operation: u64,
    work: TopologySourceWork, last_operation: u64,
}
pub(super) struct History { pub requirement: FileTopologyRequirement, current: LoadedAuthorityTopology }
pub(super) struct Next {
    topology: PathBuf, inventory: PathBuf, current: LoadedAuthorityTopology,
    next: LoadedAuthorityTopology, operation: u64, work: TopologySourceWork, last_operation: u64,
}
impl Selection {
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&crate::config::read_regular(path, PROFILE_BYTES)?)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let value = strict_json::parse(bytes, Limits { max_bytes: PROFILE_BYTES,
            max_depth: 4, max_items: 64, max_string_bytes: 4096 }).map_err(debug)?;
        let mut root = object(value)?;
        if take(&mut root, "schema_version")?.as_str() != Some("fa.mediated-submit/1") {
            return Err("unsupported mediated submission topology profile".into());
        }
        let initial = Pair::decode(take(&mut root, "initial")?)?;
        let current = Pair::decode(take(&mut root, "current")?)?;
        let available = take(&mut root, "available")?.as_bool().ok_or("available must be a boolean")?;
        let next = Pair::decode(take(&mut root, "next")?)?;
        let mut source = object(take(&mut root, "source")?)?;
        let work = TopologySourceWork { checks: number(&mut source, "checks")?,
            reserved_read_bytes: number(&mut source, "reserved_read_bytes")? };
        let last_operation = number(&mut source, "last_operation")?;
        end(&source)?;
        if work.checks.checked_mul(TOPOLOGY_REFRESH_READ_BYTES) != Some(work.reserved_read_bytes)
            || last_operation < work.checks {
            return Err("source continuity requires exact retained work and an operation floor".into());
        }
        let operation = take(&mut root, "operation")?.as_u64().ok_or("operation must be an unsigned integer")?;
        if operation <= last_operation || operation > u64::MAX - 2 {
            return Err("topology operation must leave IDs for initial and final source checks".into());
        }
        end(&root)?;
        Ok(Self { initial, current, available, next, operation, work, last_operation })
    }
    pub(super) fn history(&self, config: &Config, checkpoint: &mut impl FnMut() -> Result<(), String>)
        -> Result<History, String>
    {
        let initial = self.initial.load(config, checkpoint)?;
        let current = self.current.load(config, checkpoint)?;
        Ok(History { requirement: FileTopologyRequirement { initial: initial.graph().clone(),
            current: current.graph().clone(), available: self.available }, current })
    }
    pub(super) fn next(&self, config: &Config, history: History,
        checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<Next, String>
    {
        let next = self.next.load(config, checkpoint)?;
        if next.source_limits() != history.current.source_limits()
            || next.check_edge_budget() != history.current.check_edge_budget() {
            return Err("next topology must retain the independently selected source and checker limits".into());
        }
        let before = history.current.graph().spec(); let after = next.graph().spec();
        if after.generation <= before.generation || after.inventory_generation <= before.inventory_generation {
            return Err("next graph and inventory generations must both advance".into());
        }
        if self.work.checks.checked_add(2).is_none_or(|checks| checks > next.source_limits().checks)
            || self.work.reserved_read_bytes.checked_add(2 * TOPOLOGY_REFRESH_READ_BYTES)
                .is_none_or(|bytes| bytes > next.source_limits().read_bytes) {
            return Err("topology budget must cover initial admission and final publication".into());
        }
        Ok(Next { topology: self.next.topology.clone(), inventory: self.next.inventory.clone(),
            current: history.current, next, operation: self.operation, work: self.work, last_operation: self.last_operation })
    }
}
impl Next {
    pub(super) fn bind(self, host: &mut FileOversight, observer: FileMediationObserver)
        -> Result<FileDriverTopology, String>
    {
        let revision = host.revision();
        let source = observer.resume_topology_source(host, revision, self.operation, TopologySourceRecovery {
            topology_path: self.topology, inventory_path: self.inventory,
            previous: self.current, next: self.next, work: self.work, last_operation: self.last_operation,
        }).map_err(debug)?;
        Ok(FileDriverTopology::new(source))
    }
}
impl Pair {
    fn decode(value: Json) -> Result<Self, String> {
        let mut fields = object(value)?;
        let topology = path(take(&mut fields, "topology_path")?)?;
        let inventory = path(take(&mut fields, "inventory_path")?)?;
        end(&fields)?;
        if topology == inventory { return Err("topology and inventory paths must differ".into()); }
        Ok(Self { topology, inventory })
    }
    fn load(&self, config: &Config, checkpoint: &mut impl FnMut() -> Result<(), String>)
        -> Result<LoadedAuthorityTopology, String>
    {
        checkpoint()?;
        let store = fs::canonicalize(&config.store).map_err(debug)?;
        let topology_path = fs::canonicalize(&self.topology).map_err(debug)?;
        let inventory_path = fs::canonicalize(&self.inventory).map_err(debug)?;
        if topology_path == inventory_path || topology_path.starts_with(&store) || inventory_path.starts_with(&store) {
            return Err("distinct topology inputs require custody outside the authority store".into());
        }
        let topology = crate::config::read_regular(&self.topology, MAX_TOPOLOGY_BYTES)?;
        checkpoint()?;
        let inventory = crate::config::read_regular(&self.inventory, MAX_INVENTORY_BYTES)?;
        checkpoint()?;
        let loaded = LoadedAuthorityTopology::from_json_bytes(&topology, &inventory).map_err(debug)?;
        if !loaded.graph().binds(config.profile.delivery.scope, config.profile.delivery.target) {
            return Err("topology does not bind the original publication resource and scope".into());
        }
        checkpoint()?;
        Ok(loaded)
    }
}
fn object(value: Json) -> Result<BTreeMap<String, Json>, String> {
    match value { Json::Object(fields) => Ok(fields), _ => Err("topology profile requires an object".into()) }
}
fn take(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<Json, String> {
    fields.remove(key).ok_or_else(|| format!("topology profile missing {key}"))
}
fn number(fields: &mut BTreeMap<String, Json>, key: &str) -> Result<u64, String> {
    take(fields, key)?.as_u64().ok_or_else(|| format!("{key} must be an unsigned integer"))
}
fn end(fields: &BTreeMap<String, Json>) -> Result<(), String> {
    if fields.is_empty() { Ok(()) } else { Err("unknown topology profile fields".into()) }
}
fn path(value: Json) -> Result<PathBuf, String> {
    let path = PathBuf::from(value.as_str().ok_or("topology path must be a string")?);
    if !path.is_absolute() || path.file_name().is_none()
        || path.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err("topology paths must be absolute without parent traversal".into());
    }
    Ok(path)
}
