//! Peer-verified local evaluator intake for the native Andon operator.
//! Kernel credentials authenticate the connecting process at socket creation;
//! they do not authenticate a human, executable image, or remote identity.
use super::{Settings, mutations};
use crate::config::{Config, debug, read_regular};
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, fs, io::{Read, Write}, os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    os::unix::net::{UnixListener, UnixStream}, path::{Component, Path, PathBuf}, time::{Duration, Instant}};

const PROFILE_BYTES: usize = 16 * 1024;
const ASSESSMENT_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug)]
struct Identity { uid: u32, gid: u32, pid: Option<u32> }
impl Identity {
    fn policy(self) -> Result<PeerPolicy, String> { PeerPolicy::new(self.uid, self.gid, self.pid).map_err(debug) }
}

#[derive(Clone, Debug)]
pub(super) struct EvaluatorPeerProfile {
    socket: PathBuf,
    scope: fa_reference::action::Scope,
    deployment: u64,
    evaluator_id: u64,
    supervisor: Identity,
    evaluator: Identity,
    candidates: u32,
    runtime_ms: u64,
    poll_ms: u64,
}
impl EvaluatorPeerProfile {
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, PROFILE_BYTES)?)
    }
    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: PROFILE_BYTES, max_depth: 5,
            max_items: 64, max_string_bytes: 4096 }).map_err(debug)?;
        let root = object(&json, &["schema_version","socket","scope","deployment","evaluator",
            "supervisor","peer","candidate_limit","runtime_ms","poll_ms"])?;
        if root["schema_version"].as_str() != Some("fa.native-evaluator-peer/1") {
            return Err("unsupported native evaluator peer profile".into());
        }
        let scope_fields = object(&root["scope"], &["tenant","principal","run","branch","authority"])?;
        let scope = fa_reference::action::Scope {
            tenant: number(scope_fields,"tenant")?, principal: number(scope_fields,"principal")?,
            run: number(scope_fields,"run")?, branch: number(scope_fields,"branch")?,
            authority: number(scope_fields,"authority")?, purpose: fa_reference::action::Purpose::Effect,
        };
        if [scope.tenant,scope.principal,scope.run,scope.branch,scope.authority].contains(&0) {
            return Err("evaluator peer scope identities must be nonzero".into());
        }
        let socket = PathBuf::from(root["socket"].as_str().ok_or("evaluator socket must be text")?);
        if !socket.is_absolute() || socket.components().any(|part| !matches!(part, Component::RootDir|Component::Normal(_)))
            || socket.as_os_str().as_encoded_bytes().contains(&0) {
            return Err("evaluator socket must be an absolute normalized path without NUL".into());
        }
        let deployment = number(root,"deployment")?;
        let evaluator_id = number(root,"evaluator")?;
        let supervisor = identity(&root["supervisor"])?;
        let evaluator = identity(&root["peer"])?;
        supervisor.policy()?; evaluator.policy()?;
        let candidates = u32::try_from(number(root,"candidate_limit")?).map_err(debug)?;
        let runtime_ms = number(root,"runtime_ms")?;
        let poll_ms = number(root,"poll_ms")?;
        if deployment == 0 || evaluator_id == 0 || candidates == 0 || candidates > 1024
            || runtime_ms == 0 || runtime_ms > 3_600_000 || poll_ms == 0 || poll_ms > 1000 || poll_ms > runtime_ms {
            return Err("invalid finite evaluator peer limits".into());
        }
        Ok(Self { socket, scope, deployment, evaluator_id, supervisor, evaluator, candidates, runtime_ms, poll_ms })
    }
    fn check(&self, config: &Config, settings: &Settings) -> Result<(), String> {
        let id = settings.config.identity();
        if self.scope != config.profile.delivery.scope || self.deployment != id.deployment
            || self.evaluator_id != id.evaluator {
            return Err("evaluator peer profile does not match the native accounting audience".into());
        }
        let parent = self.socket.parent().ok_or("evaluator socket requires a parent directory")?;
        let meta = fs::symlink_metadata(parent).map_err(debug)?;
        if !meta.is_dir() || meta.file_type().is_symlink() || meta.mode() & 0o022 != 0
            || meta.uid() != self.supervisor.uid || meta.gid() != self.supervisor.gid {
            return Err("evaluator socket directory must be supervisor-owned and not group/other writable".into());
        }
        Ok(())
    }
}

struct BoundEvaluatorSocket { listener: UnixListener, path: PathBuf }
impl BoundEvaluatorSocket {
    fn bind(profile: &EvaluatorPeerProfile) -> Result<Self, String> {
        match fs::symlink_metadata(&profile.socket) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(debug(error)),
            Ok(_) => return Err("evaluator socket path already exists; it is never unlinked automatically".into()),
        }
        let listener = UnixListener::bind(&profile.socket).map_err(debug)?;
        let bound = Self { listener, path: profile.socket.clone() };
        let meta = fs::symlink_metadata(&profile.socket).map_err(debug)?;
        if !meta.file_type().is_socket() || meta.uid() != profile.supervisor.uid || meta.gid() != profile.supervisor.gid {
            return Err("new evaluator socket has unexpected ownership or type".into());
        }
        fs::set_permissions(&profile.socket, fs::Permissions::from_mode(0o660)).map_err(debug)?;
        bound.listener.set_nonblocking(true).map_err(debug)?;
        Ok(bound)
    }
}
impl Drop for BoundEvaluatorSocket {
    fn drop(&mut self) { let _ = fs::remove_file(&self.path); }
}

pub(super) fn serve(config_path: &Path, settings_path: &Path, recovery_path: &Path,
    peer_path: &Path, out: &mut impl Write) -> Result<(), String>
{
    let started = Instant::now();
    let config = Config::read(config_path)?;
    let settings = Settings::read(settings_path)?;
    let peer = EvaluatorPeerProfile::read(peer_path)?;
    peer.check(&config, &settings)?;
    let wall = Duration::from_millis(peer.runtime_ms.min(config.timing.runtime_ms));
    if started.elapsed() >= wall { return Err("evaluator intake deadline reached before bind".into()); }
    let socket = BoundEvaluatorSocket::bind(&peer)?;
    let policy = peer.evaluator.policy()?;
    let mut attempted = 0_u32;
    loop {
        if started.elapsed() >= wall { return Err("evaluator intake deadline reached without an accepted assessment".into()); }
        match socket.listener.accept() {
            Ok((mut stream, _)) => {
                if attempted == peer.candidates { return Err("evaluator candidate quota exhausted".into()); }
                attempted += 1;
                match policy.verify(&stream) {
                    Ok(credentials) => {
                        eprintln!("Verified native evaluator peer: uid={}, gid={}, pid={}", credentials.uid(), credentials.gid(), credentials.pid());
                        let remaining = wall.saturating_sub(started.elapsed());
                        stream.set_read_timeout(Some(remaining)).map_err(debug)?;
                        let bytes = read_document(&mut stream)?;
                        let mutation = mutations::Mutation::decode("assess", &bytes)?;
                        if started.elapsed() >= wall { return Err("evaluator intake deadline reached before durable recovery".into()); }
                        let mut checkpoint = || if started.elapsed() < wall { Ok(()) }
                            else { Err("evaluator intake deadline reached".to_owned()) };
                        let result = mutations::run(&config, &settings, recovery_path, &mutation,
                            &mut checkpoint, &mut mutations::clock, &mut stream);
                        let status = if result.is_ok() { "accepted" } else { "refused" };
                        writeln!(out, "{{\"schema_version\":\"fa.native-evaluator-peer-result/1\",\"status\":\"{status}\",\"candidates\":{attempted}}}").map_err(debug)?;
                        out.flush().map_err(debug)?;
                        return result;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                        eprintln!("Rejected native evaluator peer credentials; attempted={attempted}");
                        if attempted == peer.candidates { return Err("evaluator candidate quota exhausted".into()); }
                    }
                    Err(error) => return Err(debug(error)),
                }
            }
            Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock|std::io::ErrorKind::Interrupted) => {
                std::thread::sleep(Duration::from_millis(peer.poll_ms));
            }
            Err(error) => return Err(debug(error)),
        }
    }
}

pub(super) fn send(peer_path: &Path, assessment_path: &Path, out: &mut impl Write) -> Result<(), String> {
    let peer = EvaluatorPeerProfile::read(peer_path)?;
    let bytes = read_regular(assessment_path, ASSESSMENT_BYTES)?;
    // Closed schema validation occurs before any socket connection or disclosure.
    let _ = mutations::Mutation::decode("assess", &bytes)?;
    let started = Instant::now();
    let wall = Duration::from_millis(peer.runtime_ms);
    let mut stream = UnixStream::connect(&peer.socket).map_err(debug)?;
    peer.supervisor.policy()?.verify(&stream).map_err(|error| {
        format!("native evaluator refused unverified supervisor before label disclosure: {error:?}")
    })?;
    if started.elapsed() >= wall { return Err("evaluator client deadline reached before label disclosure".into()); }
    let remaining = wall.saturating_sub(started.elapsed());
    stream.set_write_timeout(Some(remaining)).map_err(debug)?;
    stream.set_read_timeout(Some(remaining)).map_err(debug)?;
    stream.write_all(&bytes).map_err(debug)?;
    stream.shutdown(std::net::Shutdown::Write).map_err(debug)?;
    let mut response = Vec::new();
    stream.take(64 * 1024 + 1).read_to_end(&mut response).map_err(debug)?;
    if response.is_empty() { return Err("verified evaluator server returned no mutation result".into()); }
    if response.len() > 64 * 1024 { return Err("verified evaluator result exceeded byte limit".into()); }
    out.write_all(&response).map_err(debug)?;
    out.flush().map_err(debug)
}

fn read_document(stream: &mut UnixStream) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    stream.take(ASSESSMENT_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(debug)?;
    if bytes.is_empty() { return Err("empty evaluator assessment document".into()); }
    if bytes.len() > ASSESSMENT_BYTES { return Err("evaluator assessment exceeds byte limit".into()); }
    Ok(bytes)
}

fn object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a BTreeMap<String,Json>,String> {
    let Json::Object(object)=value else { return Err("evaluator peer profile expects an object".into()); };
    if object.len()!=fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err("evaluator peer profile has missing or unsupported fields".into());
    }
    Ok(object)
}
fn number(object:&BTreeMap<String,Json>, field:&str)->Result<u64,String> {
    object[field].as_u64().ok_or_else(|| format!("{field} must be an unsigned integer"))
}
fn identity(value:&Json)->Result<Identity,String> {
    let fields=object(value,&["uid","gid","pid"])?;
    let uid=u32::try_from(number(fields,"uid")?).map_err(debug)?;
    let gid=u32::try_from(number(fields,"gid")?).map_err(debug)?;
    let pid=match &fields["pid"] {
        Json::Null=>None,
        value=>Some(u32::try_from(value.as_u64().ok_or("pid must be null or unsigned")?).map_err(debug)?),
    };
    Ok(Identity{uid,gid,pid})
}

#[cfg(test)]
mod tests;
