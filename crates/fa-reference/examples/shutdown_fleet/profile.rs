//! Closed independent roster/custody configuration. No live-domain intake here.
use crate::config::{Config, CONFIG_BYTES, CLOCK_DOMAIN};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::action::consequence::delivery::persistent::{MAX_JOURNAL_BYTES, MAX_JOURNAL_EVENTS};
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    shutdown::{FileShutdownDomain, FileShutdownMember, FileShutdownPlan,
        MAX_SHUTDOWN_ATTEMPTS, MAX_SHUTDOWN_DOMAINS, MAX_SHUTDOWN_HEAD_BYTES}};
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::coordinator::FileShutdownLearnedMember;
use super::{LearnedLoader, LearnedTopologyPaths};
use std::{collections::BTreeMap, fs::{self, File}, io::{Cursor, Read},
    os::unix::fs::{MetadataExt, PermissionsExt}, path::{Component, Path, PathBuf}, time::Duration};

#[path = "profile/archive.rs"]
mod archive;
#[path = "profile/custody.rs"]
pub(super) mod custody;

const MANIFEST_BYTES: usize = 256 * 1024;
const INPUT_BYTES: usize = 128 * 1024 * 1024;
const ANCHOR_OVERHEAD: usize = MAX_HISTORY_ANCHOR_BYTES - MAX_JOURNAL_BYTES;
struct Member {
    id: u64, config: PathBuf, anchor: Option<PathBuf>, learned_recipe: Option<PathBuf>,
    topology: Option<LearnedTopologyPaths>,
}
pub(super) struct Spec {
    pub coordinator: PathBuf,
    pub minimum_revision: u64,
    pub coordinator_bytes: usize,
    pub wall: Duration,
    pub replay_events: usize,
    operation: u64,
    visits: usize,
    head_bytes: usize,
    input_bytes: usize,
    manifest_bytes: usize,
    members: Vec<Member>,
    plan_archive: Option<PathBuf>,
}
pub(super) struct Prepared {
    pub spec: Spec, pub plan: FileShutdownPlan,
    pub learned: Vec<FileShutdownLearnedMember>,
    remaining_input_bytes: usize,
}

impl Spec {
    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = read_private(path, MANIFEST_BYTES)?;
        Self::decode(&bytes)
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: MANIFEST_BYTES, max_depth: 6,
            max_items: 1024, max_string_bytes: 4096 }).map_err(|e| format!("fleet profile JSON: {e:?}"))?;
        let mut f = Fields::new(json)?;
        let (version, plan_archive) = match f.text("schema_version")?.as_str() {
            "fa.shutdown-fleet/1" => (1, None),
            "fa.shutdown-fleet/2" => (2, Some(path(f.text("plan_archive")?)?)),
            "fa.shutdown-fleet/3" => (3, match f.take("plan_archive")? {
                Json::Null => None,
                Json::String(value) => Some(path(value)?),
                _ => return Err("v3 plan_archive must be null or an absolute archive path".into()),
            }),
            _ => return Err("unknown fleet profile".into()),
        };
        let operation = f.id("operation")?;
        let coordinator = path(f.text("coordinator")?)?;
        let minimum_revision = f.id("minimum_revision")?;
        let visits = f.count("max_visits", MAX_SHUTDOWN_ATTEMPTS)?;
        let head_bytes = f.count("max_head_bytes", MAX_SHUTDOWN_HEAD_BYTES)?;
        let coordinator_bytes = f.count("coordinator_bytes", MAX_JOURNAL_BYTES)?;
        let input_bytes = f.count("input_bytes", INPUT_BYTES)?;
        let runtime_ms = f.count("runtime_ms", 86_400_000)?;
        let replay_events = f.count("replay_events", MAX_JOURNAL_EVENTS)?;
        if operation == 0 || input_bytes < bytes.len() { return Err("invalid operation or input allowance".into()); }
        let Json::Array(raw) = f.take("members")? else { return Err("members must be an array".into()); };
        if raw.is_empty() || raw.len() > MAX_SHUTDOWN_DOMAINS { return Err("invalid fixed roster size".into()); }
        let mut members = Vec::<Member>::new();
        for item in raw {
            let mut m = Fields::new(item)?;
            let id = m.id("id")?; let config = path(m.text("config")?)?;
            let anchor = if plan_archive.is_none() { Some(path(m.text("history_anchor")?)?) } else { None };
            let learned_recipe = if version == 3 {
                match m.0.remove("learned_recipe") {
                    None => None,
                    Some(Json::String(value)) => Some(path(value)?),
                    Some(_) => return Err("learned_recipe must be an absolute recipe path".into()),
                }
            } else { None };
            let topology = if version == 3 {
                match (m.0.remove("topology_profile"), m.0.remove("topology_history_anchor")) {
                    (None, None) => None,
                    (Some(Json::String(profile)), Some(Json::String(anchor))) => {
                        if learned_recipe.is_none() {
                            return Err("fleet topology custody requires an independent learned_recipe".into());
                        }
                        Some(LearnedTopologyPaths { profile: path(profile)?, history_anchor: path(anchor)? })
                    }
                    _ => return Err("topology_profile and topology_history_anchor must be paired absolute paths".into()),
                }
            } else { None };
            m.end()?;
            if id == 0 || members.iter().any(|m| m.id == id || m.config == config
                || anchor.as_ref().is_some_and(|path| m.anchor.as_ref() == Some(path))) {
                return Err("duplicate or zero fleet member identity/custody".into());
            }
            members.push(Member { id, config, anchor, learned_recipe, topology });
        }
        f.end()?;
        if visits < members.len() { return Err("visit allowance cannot cover the fixed roster".into()); }
        Ok(Self { coordinator, minimum_revision, coordinator_bytes, wall: Duration::from_millis(runtime_ms as u64),
            replay_events, operation, visits, head_bytes, input_bytes, manifest_bytes: bytes.len(), members, plan_archive })
    }

    pub fn prepare(self, manifest: &Path, check: &mut impl FnMut() -> Result<(), String>) -> Result<Prepared, String> {
        self.prepare_with_learned(manifest, check, &mut super::unavailable_learned)
    }

    pub fn prepare_with_learned(self, manifest: &Path,
        check: &mut impl FnMut() -> Result<(), String>, loader: &mut LearnedLoader<'_>)
        -> Result<Prepared, String>
    {
        check()?;
        // Require the canonical existing parent spelling, so a symlinked parent
        // cannot place this writable coordinator inside an input authority store.
        let parent = self.coordinator.parent().ok_or("missing coordinator parent")?;
        if fs::canonicalize(parent).map_err(err)? != parent { return Err("coordinator parent must be canonical".into()); }
        let mut remaining = self.input_bytes - self.manifest_bytes;
        let mut configurations = Vec::new();
        let mut private_paths = vec![fs::canonicalize(manifest).map_err(err)?];
        for member in &self.members {
            check()?;
            let bytes = charged(&member.config, CONFIG_BYTES, &mut remaining)?;
            private_paths.push(fs::canonicalize(&member.config).map_err(err)?);
            if let Some(recipe) = &member.learned_recipe {
                private_paths.push(fs::canonicalize(recipe).map_err(err)?);
            }
            if let Some(topology) = &member.topology {
                private_paths.push(fs::canonicalize(&topology.profile).map_err(err)?);
                private_paths.push(fs::canonicalize(&topology.history_anchor).map_err(err)?);
            }
            let config = Config::decode(&bytes)?;
            check()?;
            if config.profile.delivery.clock_domain != CLOCK_DOMAIN { return Err("unsupported member clock".into()); }
            if config.store == self.coordinator || config.store.starts_with(&self.coordinator)
                || self.coordinator.starts_with(&config.store) { return Err("coordinator and domain directories overlap".into()); }
            configurations.push(config);
        }
        for (i, a) in configurations.iter().enumerate() {
            for b in &configurations[..i] {
                if a.store.starts_with(&b.store) || b.store.starts_with(&a.store) {
                    return Err("domain directories overlap".into());
                }
            }
        }
        // All independent inputs must lie outside EVERY authority/coordinator
        // directory. Domains themselves need not exist and are not opened here.
        if let Some(archive) = &self.plan_archive {
            private_paths.push(fs::canonicalize(archive).map_err(err)?);
        } else {
            for member in &self.members {
                private_paths.push(fs::canonicalize(member.anchor.as_ref().ok_or("missing history anchor")?).map_err(err)?);
            }
        }
        for private in &private_paths {
            if private.starts_with(&self.coordinator) || configurations.iter().any(|c| private.starts_with(&c.store)) {
                return Err("fleet custody must be outside all authority stores".into());
            }
        }
        // Numerical inputs share this SAME cumulative input allowance and are
        // admitted outside EVERY registered authority/coordinator directory.
        // The loader returns only immutable independently selected inputs; no
        // domain owner is opened and no coordinator intent exists at this point.
        let mut excluded = vec![self.coordinator.clone()];
        excluded.extend(configurations.iter().map(|config| config.store.clone()));
        let mut learned = Vec::new();
        for (member, config) in self.members.iter().zip(&configurations) {
            let inputs = if let Some(recipe) = member.learned_recipe.as_deref() {
                check()?;
                let inputs = loader(config, recipe, member.topology.as_ref(), &mut remaining, &excluded, check)?;
                check()?;
                Some(inputs)
            } else { None };
            learned.push(FileShutdownLearnedMember { domain: member.id, inputs });
        }
        learned.sort_by_key(|member| member.domain);
        if let Some(archive) = &self.plan_archive {
            let plan = archive::load(&self, archive, configurations, &mut remaining, check)?;
            return Ok(Prepared { spec: self, plan, learned, remaining_input_bytes: remaining });
        }
        let mut domains = Vec::new(); let mut retained = 0_usize;
        for (member, config) in self.members.iter().zip(configurations) {
            check()?;
            let limit = self.head_bytes.checked_sub(retained).ok_or("retained head overflow")?
                .min(MAX_JOURNAL_BYTES).checked_add(ANCHOR_OVERHEAD).ok_or("anchor limit overflow")?;
            let bytes = charged(member.anchor.as_ref().ok_or("missing history anchor")?, limit, &mut remaining)?;
            let anchor = FileHistoryAnchor::read_trusted(&mut Cursor::new(&bytes), limit.min(MAX_HISTORY_ANCHOR_BYTES)).map_err(err)?;
            retained = retained.checked_add(anchor.retained_bytes()).ok_or("retained head overflow")?;
            if retained > self.head_bytes { return Err("aggregate retained head allowance exceeded".into()); }
            domains.push(FileShutdownDomain::from_history_anchor(FileShutdownMember {
                id: member.id, directory: config.store, profile: config.profile,
            }, &anchor).map_err(err)?);
            check()?;
        }
        let plan = FileShutdownPlan::new(self.operation, domains, self.visits, self.head_bytes).map_err(err)?;
        check()?;
        Ok(Prepared { spec: self, plan, learned, remaining_input_bytes: remaining })
    }
}

fn charged(path: &Path, max: usize, remaining: &mut usize) -> Result<Vec<u8>, String> {
    let bytes = read_private(path, max.min(*remaining))?;
    *remaining = remaining.checked_sub(bytes.len()).ok_or("input allowance exceeded")?;
    Ok(bytes)
}
fn read_private(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    if max == 0 { return Err("input allowance exhausted".into()); }
    let before = fs::symlink_metadata(path).map_err(err)?;
    if !before.is_file() || before.file_type().is_symlink() || before.permissions().mode() & 0o077 != 0
        || before.len() > max as u64 { return Err("input must be a private bounded regular file".into()); }
    let file = File::open(path).map_err(err)?;
    let opened = file.metadata().map_err(err)?;
    if !opened.is_file() || opened.permissions().mode() & 0o077 != 0 || opened.len() > max as u64
        || (before.dev(), before.ino()) != (opened.dev(), opened.ino()) { return Err("input identity changed during open".into()); }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes).map_err(err)?;
    if bytes.len() > max { return Err("input grew beyond its allowance".into()); }
    Ok(bytes)
}
fn path(text: String) -> Result<PathBuf, String> {
    let p = PathBuf::from(&text);
    if !p.is_absolute() || text.len() > 4096 || p.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_))) {
        return Err("fleet paths must be absolute without parent components".into());
    }
    Ok(p)
}
fn err(e: impl std::fmt::Debug) -> String { format!("{e:?}") }
struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> {
        match value { Json::Object(map) => Ok(Self(map)), _ => Err("expected fleet profile object".into()) }
    }
    fn take(&mut self, key: &str) -> Result<Json, String> { self.0.remove(key).ok_or_else(|| format!("missing {key}")) }
    fn text(&mut self, key: &str) -> Result<String, String> {
        match self.take(key)? { Json::String(value) => Ok(value), _ => Err(format!("{key} must be a string")) }
    }
    fn id(&mut self, key: &str) -> Result<u64, String> {
        let value = self.text(key)?;
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) || (value.len() > 1 && value.starts_with('0')) {
            return Err(format!("{key} must be a canonical decimal string"));
        }
        value.parse().map_err(|_| format!("{key} exceeds u64"))
    }
    fn count(&mut self, key: &str, max: usize) -> Result<usize, String> {
        let value = self.take(key)?.as_u64().ok_or_else(|| format!("{key} must be an unsigned integer"))?;
        let count = usize::try_from(value).map_err(err)?;
        if count == 0 || count > max { return Err(format!("{key} outside original limits")); }
        Ok(count)
    }
    fn end(self) -> Result<(), String> {
        if self.0.is_empty() { Ok(()) } else { Err("unknown fleet profile fields".into()) }
    }
}
