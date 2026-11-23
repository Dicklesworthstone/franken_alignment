//! Peer-verified local evaluator intake for the native Andon operator.
//! Kernel credentials authenticate the connecting process at socket creation;
//! they do not authenticate a human, executable image, or remote identity.
#[path = "peer/receipt.rs"]
mod receipt;
#[path = "peer/transport.rs"]
mod transport;

use super::{Settings, mutations};
use crate::config::{Config, debug, read_regular};
#[cfg(test)]
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::strict_json::{self, Json, Limits};
use std::{
    fs,
    io::Write,
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const ASSESSMENT_BYTES: usize = 8 * 1024;

#[path = "peer/profile.rs"]
mod profile;
use profile::EvaluatorPeerProfile;

impl EvaluatorPeerProfile {
    fn check(&self, config: &Config, settings: &Settings) -> Result<(), String> {
        self.check_audience(config.profile.delivery.scope, settings.config.identity())
    }
}

struct BoundEvaluatorSocket {
    listener: UnixListener,
    path: PathBuf,
    identity: (u64, u64),
}
impl BoundEvaluatorSocket {
    fn bind(profile: &EvaluatorPeerProfile) -> Result<Self, String> {
        match fs::symlink_metadata(&profile.socket) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(debug(error)),
            Ok(_) => {
                return Err(
                    "evaluator socket path already exists; it is never unlinked automatically"
                        .into(),
                );
            }
        }
        let listener = UnixListener::bind(&profile.socket).map_err(debug)?;
        let meta = fs::symlink_metadata(&profile.socket).map_err(debug)?;
        if !meta.file_type().is_socket()
            || meta.uid() != profile.supervisor.uid
            || meta.gid() != profile.supervisor.gid
        {
            return Err("new evaluator socket has unexpected ownership or type".into());
        }
        let bound = Self {
            listener,
            path: profile.socket.clone(),
            identity: (meta.dev(), meta.ino()),
        };
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
            Ok(meta)
                if meta.file_type().is_socket() && (meta.dev(), meta.ino()) == self.identity =>
            {
                if let Err(error) = fs::remove_file(&self.path) {
                    eprintln!("evaluator socket cleanup: {error}");
                }
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                eprintln!("evaluator socket inspection: {error}")
            }
            _ => {}
        }
    }
}

pub(super) fn serve(
    config_path: &Path,
    settings_path: &Path,
    recovery_path: &Path,
    peer_path: &Path,
    out: &mut impl Write,
) -> Result<(), String> {
    let started = Instant::now();
    let config = Config::read(config_path)?;
    let settings = Settings::read(settings_path)?;
    let peer = EvaluatorPeerProfile::read(peer_path)?;
    peer.check(&config, &settings)?;
    let wall = Duration::from_millis(peer.runtime_ms.min(config.timing.runtime_ms));
    if started.elapsed() >= wall {
        return Err("evaluator intake deadline reached before bind".into());
    }
    let mut checkpoint = || {
        if started.elapsed() < wall {
            Ok(())
        } else {
            Err("evaluator invocation deadline reached".to_owned())
        }
    };
    let mut wait = || {
        std::thread::sleep(
            Duration::from_millis(peer.poll_ms).min(wall.saturating_sub(started.elapsed())),
        )
    };
    let socket = BoundEvaluatorSocket::bind(&peer)?;
    let policy = peer.evaluator.policy()?;
    let mut attempted = 0_u32;
    loop {
        if started.elapsed() >= wall {
            return Err("evaluator intake deadline reached without an accepted assessment".into());
        }
        match socket.listener.accept() {
            Ok((mut stream, _)) => {
                if attempted == peer.candidates {
                    return Err("evaluator candidate quota exhausted".into());
                }
                attempted += 1;
                match policy.verify(&stream) {
                    Ok(credentials) => {
                        eprintln!(
                            "Verified native evaluator peer: uid={}, gid={}, pid={}",
                            credentials.uid(),
                            credentials.gid(),
                            credentials.pid()
                        );
                        let bytes = transport::read(
                            &mut stream,
                            receipt::REQUEST_BYTES,
                            &mut checkpoint,
                            &mut wait,
                        )?;
                        let request = receipt::Request::accept(&peer, &bytes)?;
                        checkpoint()?;
                        // The native owner, journal and independent anchor finish
                        // before socket output can stall or fail. A report failure
                        // cannot undo an acknowledged mutation or justify resending.
                        let mut response = transport::Output::default();
                        let result = mutations::run(
                            &config,
                            &settings,
                            recovery_path,
                            request.mutation(),
                            &mut checkpoint,
                            &mut mutations::clock,
                            &mut response,
                        );
                        let delivered = receipt::capture(&request, &response.0, result.is_ok())
                            .and_then(|wire| {
                                transport::write(&mut stream, &wire, &mut checkpoint, &mut wait)
                            })
                            .and_then(|()| {
                                stream.shutdown(std::net::Shutdown::Write).map_err(debug)
                            });
                        let reported = writeln!(out,
                            "{{\"schema_version\":\"fa.native-evaluator-peer-result/2\",\"native_completed\":{},\"reply_written\":{},\"candidates\":{attempted}}}",
                            result.is_ok(), delivered.is_ok()).and_then(|()| out.flush()).map_err(debug);
                        return completion([result, delivered, reported]);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                        eprintln!(
                            "Rejected native evaluator peer credentials; attempted={attempted}"
                        );
                        if attempted == peer.candidates {
                            return Err("evaluator candidate quota exhausted".into());
                        }
                    }
                    Err(error) => return Err(debug(error)),
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                wait();
            }
            Err(error) => return Err(debug(error)),
        }
    }
}

pub(super) fn send(
    peer_path: &Path,
    assessment_path: &Path,
    out: &mut impl Write,
) -> Result<(), String> {
    let started = Instant::now();
    let peer = EvaluatorPeerProfile::read(peer_path)?;
    let wall = Duration::from_millis(peer.runtime_ms);
    let mut checkpoint = || {
        if started.elapsed() < wall {
            Ok(())
        } else {
            Err(
                "evaluator client deadline reached; completion is unconfirmed; no automatic retry"
                    .to_owned(),
            )
        }
    };
    let mut wait = || {
        std::thread::sleep(
            Duration::from_millis(peer.poll_ms).min(wall.saturating_sub(started.elapsed())),
        )
    };
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
    let reported = out
        .write_all(&response)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
        .map_err(debug);
    let result = if outcome.completed() {
        Ok(())
    } else {
        Err(outcome.error().into())
    };
    completion([result, reported, checkpoint()])
}

fn completion(results: [Result<(), String>; 3]) -> Result<(), String> {
    let errors: Vec<_> = results.into_iter().filter_map(Result::err).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{}; native work is not rolled back and no operation was retried",
            errors.join("; ")
        ))
    }
}

#[cfg(test)]
#[path = "peer/tests.rs"]
mod tests;
