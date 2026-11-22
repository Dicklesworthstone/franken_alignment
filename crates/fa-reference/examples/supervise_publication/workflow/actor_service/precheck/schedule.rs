//! Explicit per-request disclosure custody for one finite ordinary Tower schedule.
//! All rows and their aggregate allowances are admitted before the first owner.
//! A null row selects no bootstrap, source capture, cache or disclosure.
use super::super::{governor, operator, Config, FileSupervisedDriver, PeerProfile, Profile};
use super::Settings;
use crate::config::{debug, read_regular};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[path = "schedule/recovery.rs"]
mod recovery;

const SCHEDULE_BYTES: usize = 512 * 1024;

pub(crate) struct Schedule {
    entries: Vec<(u64, Option<Settings>)>,
    inputs: Vec<PathBuf>,
}

impl Schedule {
    /// Load every selected profile once. A later handoff cannot reopen an edited
    /// profile, reuse a previous grant or infer selection from an actor request.
    pub(crate) fn read(path: &Path, requests: &[u64]) -> Result<Self, String> {
        let entries = decode(&read_regular(path, SCHEDULE_BYTES)?, requests)?;
        // Retain names as well as decoded settings: a file removed after this
        // read cannot become an independently selected governor listener.
        let mut inputs = vec![path.to_path_buf()];
        inputs.extend(entries.iter().filter_map(|(_, path)| path.clone()));
        let entries = entries
            .into_iter()
            .map(|(request, path)| {
                Ok((request, path.as_deref().map(Settings::read).transpose()?))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let selected = Self { entries, inputs };
        selected.check_limits()?;
        Ok(selected)
    }

    pub(crate) fn selected(&self, request: u64) -> Option<&Settings> {
        self.entries
            .iter()
            .find(|(key, _)| *key == request)
            .and_then(|(_, settings)| settings.as_ref())
    }

    fn check_limits(&self) -> Result<(), String> {
        Settings::check_schedule(self.entries.iter().filter_map(|(_, row)| row.as_ref()))
    }

    /// Governance keeps its own role and listener. Admit every future row and
    /// all retained profile names before creating the original guarded owner.
    pub(crate) fn check_governed_host(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        requests: &[u64],
        operator: &operator::Settings,
        governor: &governor::Settings,
        topology: Option<&super::super::super::continuation::creation::topology::Selection>,
    ) -> Result<(), String> {
        self.check_host(config, actor, reviewer, requests, Some(operator), topology)?;
        let paths = self.inputs.iter().map(PathBuf::as_path).chain(
            self.entries.iter().filter_map(|(_, row)|
                row.as_ref().map(Settings::socket_path)),
        ).collect::<Vec<_>>();
        governor.check_custody_paths(&paths)
    }

    /// Reserve every future listener and original input/output identity before
    /// source/time work. Resolve existing ancestors so parent-symlink aliases
    /// cannot hide a collision, including with a currently absent destination.
    pub(crate) fn check_host(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        requests: &[u64],
        operator: Option<&operator::Settings>,
        topology: Option<&super::super::super::continuation::creation::topology::Selection>,
    ) -> Result<(), String> {
        if !self.entries.iter().map(|(key, _)| *key).eq(requests.iter().copied()) {
            return Err("precheck schedule keys must exactly match the request schedule".into());
        }
        self.check_limits()?;
        if self.entries.iter().all(|(_, row)| row.is_none()) {
            return Ok(());
        }
        let store = resolve_path(&config.store)?;
        let mut sockets = BTreeSet::new();
        let mut reserve_socket = |path: &Path| -> Result<(), String> {
            let path = resolve_path(path)?;
            if path.starts_with(&store) || !sockets.insert(path) {
                return Err("scheduled service sockets must be distinct and outside the authority store".into());
            }
            Ok(())
        };
        reserve_socket(&actor.socket)?;
        for &request in requests {
            reserve_socket(&reviewer.socket(request))?;
            reserve_socket(&super::super::super::control::socket_path(reviewer, request))?;
        }
        if let Some(settings) = operator {
            reserve_socket(&settings.socket)?;
        }
        let mut inputs = vec![resolve_path(config.source.path())?];
        if let Some(selection) = topology {
            for path in selection.input_paths() {
                inputs.push(resolve_path(path)?);
            }
            if let Some(path) = selection.acknowledged_output() {
                inputs.push(resolve_path(path)?);
            }
        }
        if inputs.iter().any(|path| sockets.contains(path)) {
            return Err("scheduled service sockets cannot alias source or topology paths".into());
        }
        let mut reserved = sockets;
        reserved.extend(inputs);
        for (request, settings) in &self.entries {
            let Some(settings) = settings else { continue };
            let mut selected_actor = actor.clone();
            selected_actor.request = *request;
            settings.check_host(config, &selected_actor, reviewer)?;
            let socket = resolve_path(settings.socket_path())?;
            if socket.starts_with(&store) || !reserved.insert(socket) {
                return Err("precheck sockets must be distinct from every scheduled endpoint, source, topology path and authority store".into());
            }
        }
        Ok(())
    }
}

fn decode(bytes: &[u8], requests: &[u64]) -> Result<Vec<(u64, Option<PathBuf>)>, String> {
    let mut fields = Fields::new(strict_json::parse(bytes, Limits {
        max_bytes: SCHEDULE_BYTES,
        max_depth: 4,
        max_items: 1024,
        max_string_bytes: 4096,
    }).map_err(debug)?)?;
    if fields.take("schema")?.as_str() != Some("fa.actor-precheck-schedule/1") {
        return Err("unsupported precheck schedule schema".into());
    }
    let Json::Array(rows) = fields.take("entries")? else {
        return Err("precheck schedule entries must be an array".into());
    };
    fields.end()?;
    if rows.is_empty() || rows.len() > FileSupervisedDriver::MAX_REQUEST_SEQUENCE
        || rows.len() != requests.len()
    {
        return Err("precheck schedule must explicitly select all 1..=64 request keys".into());
    }
    let mut entries = Vec::new();
    entries.try_reserve_exact(rows.len()).map_err(debug)?;
    let mut seen = BTreeSet::new();
    for (row, &expected) in rows.into_iter().zip(requests) {
        let mut fields = Fields::new(row)?;
        let request = fields.take("request")?.as_u64()
            .ok_or("precheck schedule request must be an unsigned integer")?;
        if request == 0 || request != expected || !seen.insert(request) {
            return Err("precheck schedule keys must exactly match the request schedule".into());
        }
        let path = match fields.take("precheck_profile")? {
            Json::Null => None,
            Json::String(value) => {
                let path = PathBuf::from(value);
                if !path.is_absolute() || path.file_name().is_none()
                    || path.as_os_str().as_encoded_bytes().contains(&0)
                    || path.components().any(|part|
                        !matches!(part, Component::RootDir | Component::Normal(_)))
                {
                    return Err("precheck profile paths must be absolute normalized file paths".into());
                }
                Some(path)
            }
            _ => return Err("precheck_profile must be an absolute path or explicit null".into()),
        };
        fields.end()?;
        entries.push((request, path));
    }
    Ok(entries)
}

struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value {
            Json::Object(fields) => Ok(Self(fields)),
            _ => Err("expected precheck schedule object".into()),
        }
    }
    fn take(&mut self, name: &str) -> Result<Json, String> {
        self.0.remove(name).ok_or_else(|| format!("missing precheck schedule field {name}"))
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) }
        else { Err("unknown precheck schedule field".into()) }
    }
}

fn resolve_path(path: &Path) -> Result<PathBuf, String> {
    let mut ancestor = path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(_) => {
                let mut canonical = fs::canonicalize(&ancestor).map_err(debug)?;
                for component in suffix.into_iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                suffix.push(ancestor.file_name()
                    .ok_or("scheduled path needs an existing ancestor")?.to_os_string());
                if !ancestor.pop() {
                    return Err("scheduled path needs an existing ancestor".into());
                }
            }
            Err(error) => return Err(debug(error)),
        }
    }
}
