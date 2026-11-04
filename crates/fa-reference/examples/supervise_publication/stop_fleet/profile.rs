//! Complete bounded operator intake before ANY connection. A member profile is
//! the original independent peer profile, never the supervisor CONFIG or a
//! claimed audience learned from a socket offer.
use super::super::{PeerProfile, MAX_PROFILE_BYTES};
use crate::config::{debug, read_regular};
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub(super) const MAX_MEMBERS: usize = 64;
const MAX_MANIFEST_BYTES: usize = 128 * 1024;
const MAX_STEPS_PER_PEER: usize = 65_536;

#[derive(Clone, Debug)]
pub(super) struct Member { pub id: u64, pub operation: u64, pub peer: PeerProfile }
#[derive(Clone, Debug)]
pub(super) struct Plan {
    pub runtime: Duration,
    pub poll: Duration,
    pub steps_per_peer: usize,
    pub members: Vec<Member>,
}
impl Plan {
    pub fn read<T: FnMut() -> Duration>(manifest_path: &Path, elapsed: &mut T) -> Result<Self, String> {
        let bytes = read_regular(manifest_path, MAX_MANIFEST_BYTES)?;
        let json = strict_json::parse(&bytes, Limits { max_bytes: MAX_MANIFEST_BYTES,
            max_depth: 4, max_items: 1024, max_string_bytes: 4096 }).map_err(debug)?;
        let root = object(&json, &["schema_version", "runtime_ms", "poll_ms", "steps_per_peer", "members"])?;
        if root["schema_version"].as_str() != Some("fa.live-fleet-stop/1") {
            return Err("unsupported live fleet stop schema".into());
        }
        let runtime_ms = number(root, "runtime_ms")?;
        let poll_ms = number(root, "poll_ms")?;
        let steps_per_peer = usize::try_from(number(root, "steps_per_peer")?).map_err(debug)?;
        if runtime_ms == 0 || runtime_ms > 3_600_000 || poll_ms == 0 || poll_ms > 1000
            || poll_ms > runtime_ms || steps_per_peer == 0 || steps_per_peer > MAX_STEPS_PER_PEER
        { return Err("invalid live fleet stop timing or step bound".into()); }
        let runtime = Duration::from_millis(runtime_ms);
        let Json::Array(entries) = &root["members"] else { return Err("fleet members must be an array".into()); };
        if entries.is_empty() || entries.len() > MAX_MEMBERS { return Err("fleet requires 1..=64 members".into()); }
        let mut members = Vec::new();
        members.try_reserve_exact(entries.len()).map_err(|_| "fleet roster capacity refused")?;
        let mut ids = BTreeSet::new();
        let mut audiences = BTreeSet::new();
        let mut endpoints = BTreeSet::new();
        for entry in entries {
            if elapsed() >= runtime { return Err("fleet deadline reached during intake; no connection attempted".into()); }
            let fields = object(entry, &["id", "operation", "peer_profile"])?;
            let id = identifier(&fields["id"])?;
            let operation = identifier(&fields["operation"])?;
            if !ids.insert(id) { return Err("duplicate fleet member identifier".into()); }
            let path = path(&fields["peer_profile"])?;
            // Every file is individually bounded; at most 64 original 16-KiB
            // profiles plus the 128-KiB manifest are read. No live source opens.
            let peer = PeerProfile::decode(&read_regular(&path, MAX_PROFILE_BYTES)?)?;
            let scope = peer.expected.scope;
            if !audiences.insert((scope.tenant, scope.principal, scope.run, scope.branch, scope.authority)) {
                return Err("duplicate fleet authority audience".into());
            }
            if !endpoints.insert(crate::workflow::control::socket_path(&peer, operation)) {
                return Err("duplicate fleet stop endpoint".into());
            }
            members.push(Member { id, operation, peer });
        }
        if elapsed() >= runtime { return Err("fleet deadline reached during intake; no connection attempted".into()); }
        members.sort_by_key(|member| member.id);
        Ok(Self { runtime, poll: Duration::from_millis(poll_ms), steps_per_peer, members })
    }
}

fn object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a BTreeMap<String, Json>, String> {
    let Json::Object(object) = value else { return Err("fleet profile expects an object".into()); };
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err("fleet profile has missing or unsupported fields".into());
    }
    Ok(object)
}
fn number(object: &BTreeMap<String, Json>, field: &str) -> Result<u64, String> {
    object[field].as_u64().ok_or_else(|| format!("{field} must be an unsigned integer"))
}
fn identifier(value: &Json) -> Result<u64, String> {
    let text = value.as_str().ok_or("fleet identifiers must be canonical decimal strings")?;
    let value = text.parse::<u64>().map_err(debug)?;
    if value == 0 || value.to_string() != text { return Err("fleet identifiers must be nonzero canonical decimal strings".into()); }
    Ok(value)
}
fn path(value: &Json) -> Result<PathBuf, String> {
    let text = value.as_str().ok_or("peer_profile must be a path")?;
    let path = PathBuf::from(text);
    if !path.is_absolute() || text.as_bytes().contains(&0)
        || path.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    { return Err("peer_profile must be an absolute normalized path".into()); }
    Ok(path)
}
