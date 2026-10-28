//! Independent retained topology plus explicit next-source selection. No graph
//! or lifetime profile is inferred from the authority journal being recovered.
use super::{Config, FileOversight, debug};
use super::super::super::checkpoint::{self, Output};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::{FileRecoveryFloor, anchored::FileHistoryAnchor, mediated::FileTopologyRequirement},
    mediation::FileMediationObserver,
    mediation::source::{TopologySourceRecovery, TopologySourceWork, TOPOLOGY_REFRESH_READ_BYTES},
    driver::topology::{FileDriverTopology, checkpoint::TopologyCheckpoint}};
use fa_reference::perimeter_inventory::{MAX_INVENTORY_BYTES,
    topology::{LoadedAuthorityTopology, MAX_TOPOLOGY_BYTES}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, fs, path::{Component, Path, PathBuf}};

const PROFILE_BYTES: usize = 16 * 1024;
#[derive(Clone)]
struct Pair { topology: PathBuf, inventory: PathBuf }
struct Documents { loaded: LoadedAuthorityTopology, topology: Vec<u8>, inventory: Vec<u8> }
#[derive(Clone)]
enum Basis {
    Manual { current: Pair, available: bool, work: TopologySourceWork, last_operation: u64 },
    Checkpoint(PathBuf),
}
#[derive(Clone)]
pub(crate) struct Selection {
    initial: Pair, basis: Basis, next: Pair, operation: u64, output: Option<PathBuf>,
}
#[derive(Clone)]
pub(crate) struct History {
    pub requirement: FileTopologyRequirement,
    current: LoadedAuthorityTopology,
    work: TopologySourceWork,
    last_operation: u64,
    saved: Option<TopologyCheckpoint>,
}
pub(crate) struct Next {
    topology: PathBuf, inventory: PathBuf, current: LoadedAuthorityTopology,
    next: LoadedAuthorityTopology, operation: u64, work: TopologySourceWork, last_operation: u64,
    output: Option<Output>,
}
impl History {
    /// Only v2 retained custody accounts for the exact complete original cut.
    /// Manual v1 minima cannot mint a source-work checkpoint after terminal work.
    pub(crate) fn checkpoint(&self) -> Option<&TopologyCheckpoint> { self.saved.as_ref() }

    /// Uncheckpointed journal successors may contain additional source work.
    /// v2 cannot reuse older counters as their accounting; v1 remains a minimum.
    pub(crate) fn check_extent(&self, events: usize) -> Result<(), String> {
        if self.saved.as_ref().is_some_and(|saved| saved.history_anchor().revision() != events as u64) {
            return Err("journal advanced beyond the source checkpoint; retained work is incomplete".into());
        }
        Ok(())
    }
    /// A checkpoint cannot be mixed with another independently selected cut.
    /// Its floors can only strengthen the original external minimum.
    pub(crate) fn minimum(&self, anchor: &FileHistoryAnchor, mut minimum: FileRecoveryFloor)
        -> Result<FileRecoveryFloor, String>
    {
        if let Some(saved) = &self.saved {
            if saved.history_anchor() != anchor { return Err("source checkpoint and history anchor describe different cuts".into()); }
            let floor = saved.minimum();
            minimum.journal_revision = minimum.journal_revision.max(floor.journal_revision);
            minimum.control_sequence = minimum.control_sequence.max(floor.control_sequence);
            minimum.authority_epoch = minimum.authority_epoch.max(floor.authority_epoch);
        }
        Ok(minimum)
    }
}
impl Selection {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&crate::config::read_regular(path, PROFILE_BYTES)?)
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let value = strict_json::parse(bytes, Limits { max_bytes: PROFILE_BYTES,
            max_depth: 4, max_items: 64, max_string_bytes: 4096 }).map_err(debug)?;
        let mut root = object(value)?;
        let version = take(&mut root, "schema_version")?;
        let initial = Pair::decode(take(&mut root, "initial")?)?;
        let next = Pair::decode(take(&mut root, "next")?)?;
        let operation = number(&mut root, "operation")?;
        if operation == 0 || operation > u64::MAX - 2 {
            return Err("topology operation must leave IDs for initial and final source checks".into());
        }
        let (basis, output) = match version.as_str() {
            Some("fa.mediated-submit/1") => {
                let current = Pair::decode(take(&mut root, "current")?)?;
                let available = take(&mut root, "available")?.as_bool().ok_or("available must be a boolean")?;
                let mut source = object(take(&mut root, "source")?)?;
                let work = TopologySourceWork { checks: number(&mut source, "checks")?,
                    reserved_read_bytes: number(&mut source, "reserved_read_bytes")? };
                let last_operation = number(&mut source, "last_operation")?;
                end(&source)?;
                if work.checks.checked_mul(TOPOLOGY_REFRESH_READ_BYTES) != Some(work.reserved_read_bytes)
                    || last_operation < work.checks || operation <= last_operation {
                    return Err("source continuity requires exact retained work and an operation floor".into());
                }
                (Basis::Manual { current, available, work, last_operation }, None)
            }
            Some("fa.mediated-submit/2") => {
                let saved = path(take(&mut root, "source_checkpoint")?)?;
                let output = root.remove("retain_checkpoint").map(path).transpose()?;
                (Basis::Checkpoint(saved), output)
            }
            _ => return Err("unsupported mediated submission topology profile".into()),
        };
        end(&root)?;
        Ok(Self { initial, basis, next, operation, output })
    }
    pub(crate) fn retention_requested(&self) -> bool { self.output.is_some() }
    /// The independently selected destination can retain a terminal cut without
    /// loading or binding the next live topology/inventory documents.
    #[cfg(target_os = "linux")]
    pub(crate) fn acknowledged_output(&self) -> Option<&Path> { self.output.as_deref() }
    pub(crate) fn history(&self, config: &Config, checkpoint: &mut impl FnMut() -> Result<(), String>)
        -> Result<History, String>
    {
        let initial = self.initial.load(config, checkpoint)?;
        let (current, available, work, last_operation, saved) = match &self.basis {
            Basis::Manual { current, available, work, last_operation } =>
                (current.load(config, checkpoint)?, *available, *work, *last_operation, None),
            Basis::Checkpoint(path) => {
                let saved = checkpoint::read(config, path, checkpoint)?;
                (saved.expected().clone(), saved.available(), saved.work(), saved.last_operation(), Some(saved))
            }
        };
        if self.operation <= last_operation { return Err("topology operation is behind retained source work".into()); }
        Ok(History { requirement: FileTopologyRequirement { initial: initial.graph().clone(),
            current: current.graph().clone(), available }, current, work, last_operation, saved })
    }
    pub(crate) fn next(&self, config: &Config, history: History,
        checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<Next, String>
    {
        let Documents { loaded: next, topology, inventory } = self.next.documents(config, checkpoint)?;
        if next.source_limits() != history.current.source_limits()
            || next.check_edge_budget() != history.current.check_edge_budget() {
            return Err("next topology must retain the independently selected source and checker limits".into());
        }
        let before = history.current.graph().spec(); let after = next.graph().spec();
        if after.generation <= before.generation || after.inventory_generation <= before.inventory_generation {
            return Err("next graph and inventory generations must both advance".into());
        }
        if history.work.checks.checked_add(2).is_none_or(|checks| checks > next.source_limits().checks)
            || history.work.reserved_read_bytes.checked_add(2 * TOPOLOGY_REFRESH_READ_BYTES)
                .is_none_or(|bytes| bytes > next.source_limits().read_bytes) {
            return Err("topology budget must cover initial admission and final publication".into());
        }
        let output = self.output.as_ref().map(|path| Output::admit(config, path, topology, inventory)).transpose()?;
        if output.is_some() { checkpoint()?; }
        Ok(Next { topology: self.next.topology.clone(), inventory: self.next.inventory.clone(),
            current: history.current, next, operation: self.operation, work: history.work,
            last_operation: history.last_operation, output })
    }
}
impl Next {
    pub(crate) fn bind(self, host: &mut FileOversight, observer: FileMediationObserver)
        -> Result<(FileDriverTopology, Option<Output>), String>
    {
        let revision = host.revision();
        let source = observer.resume_topology_source(host, revision, self.operation, TopologySourceRecovery {
            topology_path: self.topology, inventory_path: self.inventory,
            previous: self.current, next: self.next, work: self.work, last_operation: self.last_operation,
        }).map_err(debug)?;
        Ok((FileDriverTopology::new(source), self.output))
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
        Ok(self.documents(config, checkpoint)?.loaded)
    }
    fn documents(&self, config: &Config, checkpoint: &mut impl FnMut() -> Result<(), String>)
        -> Result<Documents, String>
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
        Ok(Documents { loaded, topology, inventory })
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
