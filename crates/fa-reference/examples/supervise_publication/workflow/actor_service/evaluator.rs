//! Independent ground-truth intake on the SAME live native authority owner.
//! One finite cooperative service spans the complete publication schedule.
#[path = "../../../native_andon/assessment.rs"]
mod assessment;
#[path = "evaluator/client.rs"]
pub(crate) mod client;
#[path = "evaluator/profile.rs"]
mod profile;
#[path = "evaluator/protocol.rs"]
mod protocol;
#[path = "../../../native_andon/peer/profile.rs"]
mod shared_profile;
pub(crate) use shared_profile::EvaluatorPeerProfile as Settings;

use super::{BoundSocket, Deadline, Duration, ElapsedTick, FileSupervisedDriver, Instant, Profile};
use crate::config::debug;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, credibility::FileIndependentEvaluator,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorSupervisor;
use protocol::{Basis, Outcome, Request};
use std::{
    fs,
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::{
        fs::{FileTypeExt, MetadataExt, PermissionsExt},
        net::UnixStream,
    },
};

const MAX_IO_CALLS: usize = 65_536;
const TURN_CALLS: usize = 16;
const TURN_BYTES: usize = 16 * 1024;

struct Connection {
    socket: UnixStream,
    input: Vec<u8>,
    output: Option<(Vec<u8>, usize)>,
}

pub(super) struct Service {
    settings: Settings,
    socket: Option<BoundSocket>,
    connection: Option<Connection>,
    evaluator: Option<FileIndependentEvaluator>,
    started: Instant,
    wall: Duration,
    logical: ElapsedTick,
    candidates: u32,
    io_calls: usize,
    transferred: usize,
    max_bytes: usize,
    finishing: bool,
    retired: bool,
}

impl Service {
    pub(super) fn start(
        settings: &Settings,
        actor: &Profile,
        evaluator: FileIndependentEvaluator,
        driver: &FileSupervisedDriver,
        deadline: &Deadline,
    ) -> Result<Self, String> {
        settings.check_actor(actor)?;
        settings.check_unbound()?;
        {
            let host = driver.supervisor().host().map_err(debug)?;
            let config = host
                .andon_config()
                .map_err(debug)?
                .ok_or("live evaluator requires original native Andon accounting")?;
            settings.check_audience(actor.scope, config.identity())?;
            if host.inspect().stop.is_some() {
                return Err("a stopped owner cannot start a live evaluator".into());
            }
        }
        let started = Instant::now();
        let wall = Duration::from_millis(settings.runtime_ms.min(actor.runtime_ms))
            .min(deadline.wall.saturating_sub(deadline.started.elapsed()));
        if wall.is_zero() {
            return Err("evaluator setup exceeded the original invocation lifetime".into());
        }
        let socket = BoundSocket::bind(&settings.socket, None)?;
        let metadata = fs::symlink_metadata(&settings.socket).map_err(debug)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != settings.supervisor.uid
            || metadata.gid() != settings.supervisor.gid
        {
            return Err("new evaluator socket has unexpected ownership or type".into());
        }
        fs::set_permissions(&settings.socket, fs::Permissions::from_mode(0o660)).map_err(debug)?;
        if started.elapsed() >= wall {
            return Err("evaluator setup exceeded the original invocation lifetime".into());
        }
        let max_bytes = (protocol::REQUEST_BYTES + 1 + protocol::RESPONSE_BYTES)
            .checked_mul(settings.candidates as usize)
            .ok_or("evaluator transport allowance overflow")?;
        Ok(Self {
            settings: settings.clone(),
            socket: Some(socket),
            connection: None,
            evaluator: Some(evaluator),
            started,
            wall,
            logical: deadline.logical,
            candidates: 0,
            io_calls: 0,
            transferred: 0,
            max_bytes,
            finishing: false,
            retired: false,
        })
    }

    /// Call only AFTER the original independent Stop checkpoint. Kernel-peer
    /// verification precedes every read. One bounded turn never sleeps or waits.
    pub(super) fn checkpoint_driver(&mut self, driver: &mut FileSupervisedDriver) {
        self.checkpoint_supervisor(driver.supervisor_mut());
    }

    pub(super) fn checkpoint_supervisor(
        &mut self,
        supervisor: &mut FileActorSupervisor<FileOversight>,
    ) {
        self.checkpoint_with_clock(supervisor, &mut || Ok(super::super::clock()));
    }

    fn checkpoint_with_clock(
        &mut self,
        supervisor: &mut FileActorSupervisor<FileOversight>,
        clock: &mut impl FnMut() -> Result<ElapsedTick, String>,
    ) {
        if self.retired {
            return;
        }
        let unavailable = supervisor.host().map_or(true, |host| {
            host.inspect().stop.is_some()
                || (host.storage_failure().is_some()
                    && !self
                        .connection
                        .as_ref()
                        .is_some_and(|connection| connection.output.is_some()))
        });
        if unavailable || self.started.elapsed() >= self.wall {
            self.retire();
            return;
        }
        let mut calls = 0;
        if self.connection.is_none() {
            if self.finishing
                || self.candidates == self.settings.candidates
                || !self.call(&mut calls)
            {
                self.retire();
                return;
            }
            let accepted = self
                .socket
                .as_ref()
                .expect("live evaluator listener")
                .listener
                .accept();
            let socket = match accepted {
                Ok((socket, _)) => socket,
                Err(error) if transient(&error) => return,
                Err(_) => {
                    self.retire();
                    return;
                }
            };
            self.candidates += 1;
            if !self.call(&mut calls) {
                self.retire();
                return;
            }
            // A rejected process spends one candidate but discloses/reads no bytes.
            if self
                .settings
                .evaluator
                .policy()
                .and_then(|policy| policy.verify(&socket).map_err(debug))
                .is_err()
            {
                return;
            }
            if !self.call(&mut calls) || socket.set_nonblocking(true).is_err() {
                self.retire();
                return;
            }
            self.connection = Some(Connection {
                socket,
                input: Vec::new(),
                output: None,
            });
        }
        let mut connection = self
            .connection
            .take()
            .expect("accepted evaluator connection");
        let mut transferred = 0;
        while calls < TURN_CALLS && transferred < TURN_BYTES {
            if self.started.elapsed() >= self.wall || !self.call(&mut calls) {
                self.retire();
                return;
            }
            if let Some((output, position)) = &mut connection.output {
                let allowance =
                    (TURN_BYTES - transferred).min(self.max_bytes.saturating_sub(self.transferred));
                if allowance == 0 {
                    self.retire();
                    return;
                }
                let end = output.len().min(position.saturating_add(allowance));
                match connection.socket.write(&output[*position..end]) {
                    Ok(0) => return,
                    Ok(count) => {
                        *position += count;
                        self.transferred += count;
                        transferred += count;
                        if *position == output.len() {
                            if self.call(&mut calls) {
                                let _ = connection.socket.shutdown(Shutdown::Write);
                            }
                            return;
                        }
                    }
                    Err(error) if transient(&error) => {
                        self.connection = Some(connection);
                        return;
                    }
                    Err(_) => return,
                }
                continue;
            }
            if self.finishing {
                self.retire();
                return;
            }
            let allowance = 4096
                .min(TURN_BYTES - transferred)
                .min(protocol::REQUEST_BYTES + 1 - connection.input.len())
                .min(self.max_bytes.saturating_sub(self.transferred));
            if allowance == 0 {
                self.retire();
                return;
            }
            let mut buffer = [0_u8; 4096];
            match connection.socket.read(&mut buffer[..allowance]) {
                Ok(0) => {
                    let request = match Request::accept(&self.settings, &connection.input) {
                        Ok(request) => request,
                        Err(_) => return,
                    };
                    // EOF seals the entire request. Prefixes and a merely quiet
                    // socket cannot enter a native label or refresh its clock.
                    let response = match self.apply(supervisor, &request, clock) {
                        Ok(response) => response,
                        Err(_) => {
                            self.retire();
                            return;
                        }
                    };
                    connection.output = Some((response, 0));
                }
                Ok(count) => {
                    self.transferred += count;
                    transferred += count;
                    if connection.input.len() + count > protocol::REQUEST_BYTES {
                        return;
                    }
                    if connection.input.try_reserve(count).is_err() {
                        self.retire();
                        return;
                    }
                    connection.input.extend_from_slice(&buffer[..count]);
                }
                Err(error) if transient(&error) => {
                    self.connection = Some(connection);
                    return;
                }
                Err(_) => return,
            }
        }
        self.connection = Some(connection);
    }

    fn apply(
        &self,
        supervisor: &mut FileActorSupervisor<FileOversight>,
        request: &Request,
        clock: &mut impl FnMut() -> Result<ElapsedTick, String>,
    ) -> Result<Vec<u8>, String> {
        let mut host = supervisor.host_mut().map_err(debug)?;
        if host.inspect().stop.is_some() {
            return Err("original owner is stopped".into());
        }
        let before = Basis::observe(&host)?;
        let result = request.assessment.apply(
            &mut host,
            self.evaluator.as_ref().ok_or("evaluator role retired")?,
            &mut || {
                if self.started.elapsed() < self.wall {
                    Ok(())
                } else {
                    Err("original evaluator lifetime elapsed".into())
                }
            },
            &mut || {
                let tick = clock()?;
                if tick < self.logical {
                    Ok(tick)
                } else {
                    Err("original logical deadline reached".into())
                }
            },
        );
        // A failed replacement is never reported from the stale in-memory cut.
        // A healthy Time-only predecessor remains visible on a refused label.
        let after = Basis::observe(&host).ok();
        let outcome = match (&result, after) {
            (_, None) => Outcome::Unconfirmed,
            (Ok(true), Some(_)) => Outcome::Applied,
            (Ok(false), Some(_)) => Outcome::AlreadyApplied,
            (Err(_), Some(_)) => Outcome::Refused,
        };
        protocol::receipt(request, outcome, before, after)
    }

    fn call(&mut self, calls: &mut usize) -> bool {
        if *calls == TURN_CALLS || self.io_calls == MAX_IO_CALLS {
            return false;
        }
        *calls += 1;
        self.io_calls += 1;
        true
    }

    /// End native intake before terminal grace. Only an already acknowledged
    /// response may finish writing; a queued assessment gets no new opportunity.
    pub(super) fn finish(&mut self) {
        self.finishing = true;
        self.evaluator = None;
        self.socket = None;
        if !self
            .connection
            .as_ref()
            .is_some_and(|connection| connection.output.is_some())
        {
            self.retire();
        }
    }

    pub(super) fn retire(&mut self) {
        self.retired = true;
        self.evaluator = None;
        self.connection = None;
        self.socket = None;
    }
}

fn transient(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

#[cfg(test)]
#[path = "evaluator/tests.rs"]
mod tests;
