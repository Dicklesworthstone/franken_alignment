//! Peer-verified local evaluator intake for the native Andon operator.
//! Kernel credentials authenticate the connecting process at socket creation;
//! they do not authenticate a human, executable image, or remote identity.
mod transport;
mod receipt;

use super::{Settings, mutations};
use crate::config::{Config, debug, read_regular};
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::strict_json::{self, Json, Limits};
use std::{collections::BTreeMap, fs, io::Write, os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
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

struct BoundEvaluatorSocket { listener: UnixListener, path: PathBuf, identity: (u64, u64) }
impl BoundEvaluatorSocket {
    fn bind(profile: &EvaluatorPeerProfile) -> Result<Self, String> {
        match fs::symlink_metadata(&profile.socket) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(debug(error)),
            Ok(_) => return Err("evaluator socket path already exists; it is never unlinked automatically".into()),
        }
        let listener = UnixListener::bind(&profile.socket).map_err(debug)?;
        let meta = fs::symlink_metadata(&profile.socket).map_err(debug)?;
        if !meta.file_type().is_socket() || meta.uid() != profile.supervisor.uid || meta.gid() != profile.supervisor.gid {
            return Err("new evaluator socket has unexpected ownership or type".into());
        }
        let bound = Self { listener, path: profile.socket.clone(), identity: (meta.dev(), meta.ino()) };
        fs::set_permissions(&profile.socket, fs::Permissions::from_mode(0o660)).map_err(debug)?;
        bound.listener.set_nonblocking(true).map_err(debug)?;
        Ok(bound)
    }
}
impl Drop for BoundEvaluatorSocket {
    fn drop(&mut self) {
        // The namespace is operator controlled. Do not remove a replacement
        // file/socket after our original pathname was renamed or unlinked.
        match fs::symlink_metadata(&self.path) {
            Ok(meta) if meta.file_type().is_socket() && (meta.dev(), meta.ino()) == self.identity => {
                if let Err(error) = fs::remove_file(&self.path) { eprintln!("evaluator socket cleanup: {error}"); }
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => eprintln!("evaluator socket inspection: {error}"),
            _ => {}
        }
    }
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
    let mut checkpoint = || if started.elapsed() < wall { Ok(()) }
        else { Err("evaluator invocation deadline reached".to_owned()) };
    let mut wait = || std::thread::sleep(Duration::from_millis(peer.poll_ms)
        .min(wall.saturating_sub(started.elapsed())));
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
                        let bytes = transport::read(&mut stream, receipt::REQUEST_BYTES, &mut checkpoint, &mut wait)?;
                        let request = receipt::Request::accept(&peer, &bytes)?;
                        checkpoint()?;
                        // The native owner, journal and independent anchor finish
                        // before socket output can stall or fail. A report failure
                        // cannot undo an acknowledged mutation or justify resending.
                        let mut response = transport::Output::default();
                        let result = mutations::run(&config, &settings, recovery_path, request.mutation(),
                            &mut checkpoint, &mut mutations::clock, &mut response);
                        let delivered = receipt::capture(&request, &response.0, result.is_ok())
                            .and_then(|wire| transport::write(&mut stream, &wire, &mut checkpoint, &mut wait))
                            .and_then(|()| stream.shutdown(std::net::Shutdown::Write).map_err(debug));
                        let reported = writeln!(out,
                            "{{\"schema_version\":\"fa.native-evaluator-peer-result/2\",\"native_completed\":{},\"reply_written\":{},\"candidates\":{attempted}}}",
                            result.is_ok(), delivered.is_ok()).and_then(|()| out.flush()).map_err(debug);
                        return completion([result, delivered, reported]);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                        eprintln!("Rejected native evaluator peer credentials; attempted={attempted}");
                        if attempted == peer.candidates { return Err("evaluator candidate quota exhausted".into()); }
                    }
                    Err(error) => return Err(debug(error)),
                }
            }
            Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock|std::io::ErrorKind::Interrupted) => {
                wait();
            }
            Err(error) => return Err(debug(error)),
        }
    }
}

pub(super) fn send(peer_path: &Path, assessment_path: &Path, out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    let peer = EvaluatorPeerProfile::read(peer_path)?;
    let wall = Duration::from_millis(peer.runtime_ms);
    let mut checkpoint = || if started.elapsed() < wall { Ok(()) }
        else { Err("evaluator client deadline reached; completion is unconfirmed; no automatic retry".to_owned()) };
    let mut wait = || std::thread::sleep(Duration::from_millis(peer.poll_ms)
        .min(wall.saturating_sub(started.elapsed())));
    checkpoint()?;
    let bytes = read_regular(assessment_path, ASSESSMENT_BYTES)?;
    // Closed schema validation occurs before any socket connection or disclosure.
    let request = receipt::Request::prepare(&peer, &bytes)?;
    checkpoint()?;
    // Standard-library connect and file/native operations remain synchronous;
    // these surrounding checks are not a hard preemption guarantee.
    let mut stream = UnixStream::connect(&peer.socket).map_err(debug)?;
    peer.supervisor.policy()?.verify(&stream).map_err(|error| {
        format!("native evaluator refused unverified supervisor before label disclosure: {error:?}")
    })?;
    checkpoint()?;
    let response = (|| {
        transport::write(&mut stream, request.wire(), &mut checkpoint, &mut wait)?;
        stream.shutdown(std::net::Shutdown::Write).map_err(debug)?;
        transport::read(&mut stream, transport::MAX_MESSAGE_BYTES, &mut checkpoint, &mut wait)
    })().map_err(|error| format!("evaluator exchange incomplete: {error}; the original assessment may be recorded; no automatic retry"))?;
    checkpoint()?;
    // Authentication alone is not a success response. Validate exact request,
    // audience, schema and native acknowledgment BEFORE rendering any bytes.
    let outcome = receipt::validate(&request, &response).map_err(|error| format!(
        "evaluator result unconfirmed: {error}; the original assessment may be recorded; no automatic retry"))?;
    let reported = out.write_all(&response).and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush()).map_err(debug);
    let result = if outcome.completed() { Ok(()) } else { Err(outcome.error().into()) };
    completion([result, reported, checkpoint()])
}

fn completion(results: [Result<(), String>; 3]) -> Result<(), String> {
    let errors: Vec<_> = results.into_iter().filter_map(Result::err).collect();
    if errors.is_empty() { Ok(()) } else { Err(format!(
        "{}; native work is not rolled back and no operation was retried", errors.join("; "))) }
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
