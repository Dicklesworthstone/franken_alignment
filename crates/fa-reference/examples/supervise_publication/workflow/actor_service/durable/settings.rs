//! Closed actor-only configuration; original constructors enforce all limits.
use super::{Profile, debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::{
    FileClientProfile, peer::FilePeerClientProfile,
};
use fa_reference::action::consequence::oversight::actor_wire::client::{ClientIoLimits, ClientSessionLimits};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, path::{Component, Path, PathBuf}};

pub(super) struct Settings {
    pub directory: PathBuf,
    pub profile: FilePeerClientProfile,
    pub drive: ClientDriveBudget,
}
impl Settings {
    pub fn read(path: &Path, actor: &Profile) -> Result<Self, String> {
        Self::decode(&read_regular(path, 16384)?, actor)
    }
    pub fn decode(bytes: &[u8], actor: &Profile) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: 16384, max_depth: 2,
            max_items: 64, max_string_bytes: 4096 }).map_err(debug)?;
        let mut f = Fields::new(json)?;
        if f.text("schema")? != "fa.durable-actor-client/1" { return Err("unsupported durable client schema".into()); }
        let directory = PathBuf::from(f.text("directory")?);
        if !directory.is_absolute() || directory.file_name().is_none()
            || directory.components().any(|p| !matches!(p, Component::RootDir | Component::Normal(_)))
            || directory.as_os_str().as_encoded_bytes().contains(&0)
            || directory.as_os_str().as_encoded_bytes().len() > 4096
            || actor.socket.starts_with(&directory) {
            return Err("client directory must be a normalized absolute private path distinct from the server socket tree".into());
        }
        let identity = f.text("server_identity")?;
        if identity.is_empty() || identity.len() > 1024 { return Err("server_identity must contain 1..=1024 bytes".into()); }
        let limits = ClientSessionLimits {
            requests: f.usize("requests")?, retained_payload_bytes: f.usize("payload_bytes")?,
            io: ClientIoLimits { exchanges: actor.exchanges, calls: f.number("io_calls")?,
                written_bytes: f.number("write_bytes")?, read_bytes: f.number("read_bytes")? },
        };
        let image_bytes = f.usize("image_bytes")?;
        let drive = ClientDriveBudget::new(f.number("drive_steps")?).map_err(debug)?;
        f.end()?;
        // A pathname alone does not identify a server. Independently supplied
        // domain identity, complete audience, clock domain and raw wire profile
        // are length-framed; the original peer wrapper then binds supervisor UID,
        // GID and optional PID. Request IDs and invocation timers are not domains.
        let mut binding = b"FAOPCLIENT\0raw-submit-v1\0".to_vec();
        let s = actor.scope;
        for n in [s.tenant, s.principal, s.run, s.branch, s.authority, crate::config::CLOCK_DOMAIN] {
            binding.extend_from_slice(&n.to_be_bytes());
        }
        for part in [identity.as_bytes(), actor.socket.as_os_str().as_encoded_bytes()] {
            binding.extend_from_slice(&(part.len() as u32).to_be_bytes()); binding.extend_from_slice(part);
        }
        let profile = FileClientProfile::new(binding, limits, image_bytes).map_err(debug)?
            .with_peer_policy(actor.supervisor).map_err(debug)?;
        Ok(Self { directory, profile, drive })
    }
}
struct Fields(BTreeMap<String, Json>);
impl Fields {
    fn new(value: Json) -> Result<Self, String> { match value { Json::Object(v) => Ok(Self(v)), _ => Err("expected client profile object".into()) } }
    fn take(&mut self, key: &str) -> Result<Json, String> { self.0.remove(key).ok_or_else(|| format!("missing client field {key}")) }
    fn text(&mut self, key: &str) -> Result<String, String> { self.take(key)?.as_str().map(str::to_owned).ok_or_else(|| format!("{key} must be text")) }
    fn number(&mut self, key: &str) -> Result<u64, String> { self.take(key)?.as_u64().ok_or_else(|| format!("{key} must be unsigned")) }
    fn usize(&mut self, key: &str) -> Result<usize, String> { usize::try_from(self.number(key)?).map_err(debug) }
    fn end(self) -> Result<(), String> { if self.0.is_empty() { Ok(()) } else { Err("unknown durable client field".into()) } }
}
