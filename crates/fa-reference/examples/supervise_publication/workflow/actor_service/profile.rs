//! Public connection contract, separate from private supervisor/evidence config.
use super::super::{Config, CLOCK_DOMAIN, debug};
use crate::config::read_regular;
use fa_reference::action::{Purpose, Scope};
use fa_reference::action::consequence::oversight::actor_peer::{PeerPolicy, MAX_PEER_CONNECTIONS};
use fa_reference::action::consequence::oversight::actor_wire::{ChannelLimits, MAX_CHANNEL_EXCHANGES, MAX_FRAME_BYTES};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub(super) struct Profile {
    pub request: u64,
    pub scope: Scope,
    pub socket: PathBuf,
    pub supervisor: PeerPolicy,
    pub actor: PeerPolicy,
    pub candidates: u64,
    pub connections: u64,
    pub exchanges: u64,
    pub runtime_ms: u64,
    pub poll_ms: u64,
    pub reply_ms: u64,
}
impl Profile {
    pub fn read(path: &Path) -> Result<Self, String> { Self::decode(&read_regular(path, 16384)?) }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut f = Fields::new(strict_json::parse(bytes, Limits { max_bytes: 16384, max_depth: 4,
            max_items: 128, max_string_bytes: 4096 }).map_err(debug)?)?;
        if f.text("schema")? != "fa.actor-service/1" || f.text("clock")? != "unix_milliseconds" {
            return Err("unsupported actor service schema or clock".into());
        }
        let request = f.number("request")?;
        if request == 0 { return Err("request must be nonzero".into()); }
        let mut s = Fields::new(f.take("scope")?)?;
        let scope = Scope { tenant: s.number("tenant")?, principal: s.number("principal")?, run: s.number("run")?,
            branch: s.number("branch")?, authority: s.number("authority")?, purpose: Purpose::Effect };
        s.end()?;
        if [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority].contains(&0) {
            return Err("actor audience identifiers must be nonzero".into());
        }
        let socket = PathBuf::from(f.text("socket")?);
        if !socket.is_absolute() || socket.file_name().is_none()
            || socket.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || socket.as_os_str().as_encoded_bytes().contains(&0)
            || socket.as_os_str().as_encoded_bytes().len() > 100 {
            return Err("actor socket must be an absolute normalized path of at most 100 bytes".into());
        }
        let supervisor = identity(f.take("supervisor")?)?;
        let actor = identity(f.take("actor")?)?;
        let candidates = f.number("candidate_limit")?;
        let connections = f.number("connection_limit")?;
        let exchanges = f.number("exchange_limit")?;
        if candidates == 0 || candidates > MAX_PEER_CONNECTIONS || connections == 0
            || connections > candidates || exchanges == 0 || exchanges > MAX_CHANNEL_EXCHANGES {
            return Err("actor service connection/exchange limits are out of range".into());
        }
        let runtime_ms = f.number("runtime_ms")?;
        let poll_ms = f.number("poll_ms")?;
        let reply_ms = f.number("reply_ms")?;
        if runtime_ms == 0 || runtime_ms > 3_600_000 || poll_ms == 0 || poll_ms > 1000
            || poll_ms > runtime_ms || reply_ms == 0 || reply_ms > runtime_ms {
            return Err("actor service timing is out of range".into());
        }
        f.end()?;
        Ok(Self { request, scope, socket, supervisor, actor, candidates, connections, exchanges,
            runtime_ms, poll_ms, reply_ms })
    }
    pub fn channels(&self) -> ChannelLimits { ChannelLimits { frame_bytes: MAX_FRAME_BYTES, exchanges: self.exchanges } }
    pub fn check_host(&self, config: &Config) -> Result<(), String> {
        if self.scope != config.profile.delivery.scope || config.profile.delivery.clock_domain != CLOCK_DOMAIN {
            return Err("actor audience does not match supervisor scope/clock".into());
        }
        self.check_directory()
    }
    pub fn check_directory(&self) -> Result<(), String> {
        let metadata = fs::symlink_metadata(self.socket.parent().ok_or("missing socket directory")?).map_err(debug)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() || metadata.mode() & 0o022 != 0
            || metadata.uid() != self.supervisor.uid() || metadata.gid() != self.supervisor.gid() {
            return Err("actor socket directory must be supervisor-owned and not group/other writable".into());
        }
        Ok(())
    }
    pub fn secure_socket(&self) -> Result<(), String> {
        let metadata = fs::symlink_metadata(&self.socket).map_err(debug)?;
        if !metadata.file_type().is_socket() || metadata.uid() != self.supervisor.uid() || metadata.gid() != self.supervisor.gid() {
            return Err("actor socket has unexpected ownership or type".into());
        }
        fs::set_permissions(&self.socket, fs::Permissions::from_mode(0o660)).map_err(debug)
    }
}
fn identity(value: Json) -> Result<PeerPolicy, String> {
    let mut f = Fields::new(value)?;
    let uid = u32::try_from(f.number("uid")?).map_err(debug)?;
    let gid = u32::try_from(f.number("gid")?).map_err(debug)?;
    let pid = match f.take("pid")? {
        Json::Null => None,
        value => Some(u32::try_from(value.as_u64().ok_or("pid must be null or an unsigned integer")?).map_err(debug)?),
    };
    f.end()?; PeerPolicy::new(uid, gid, pid).map_err(debug)
}
struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> { match value { Json::Object(f) => Ok(Self(f)), _ => Err("expected object".into()) } }
    fn take(&mut self, name: &str) -> Result<Json, String> { self.0.remove(name).ok_or_else(|| format!("missing actor field {name}")) }
    fn text(&mut self, name: &str) -> Result<String, String> { self.take(name)?.as_str().map(str::to_owned).ok_or_else(|| format!("{name} must be text")) }
    fn number(&mut self, name: &str) -> Result<u64, String> { self.take(name)?.as_u64().ok_or_else(|| format!("{name} must be an unsigned integer")) }
    fn end(self) -> Result<(), String> { if self.0.is_empty() { Ok(()) } else { Err("unknown actor service field".into()) } }
}
