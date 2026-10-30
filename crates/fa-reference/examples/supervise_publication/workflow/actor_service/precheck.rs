//! Optional exact-predicate ingress beside the ORIGINAL live actor service.
//! Bootstrap is an explicit supervisor operation. Query turns cannot read source,
//! observe owner time, append journal events, launch helpers or issue new grants.
#[path = "precheck/profile.rs"]
mod profile;
pub(super) use profile::Settings;
#[path = "precheck/client.rs"]
pub(super) mod client;

use super::{BoundSocket, Config, Deadline, Duration, ElapsedTick, FileSupervisedDriver,
    Instant, PeerProfile, Profile, debug, io, plus};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FilePrecheckDisclosure;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    FilePrecheckPeerSession, FilePrecheckWire, MAX_PRECHECK_FRAME_BYTES,
};
use fa_reference::action::consequence::oversight::actor_peer::PeerRefusal;
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ChannelState, CloseReason};
use fa_reference::action::consequence::oversight::policy_state::ObservationLease;

struct Active {
    socket: BoundSocket,
    session: FilePrecheckPeerSession,
    disclosure: FilePrecheckDisclosure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Retirement {
    SupervisorEnded,
    OwnerChanged,
    ClockMovedBack,
    Expired,
    CandidateLimit,
    ConnectionLimit,
    ExchangeLimit,
    TransportFailure,
}

pub(super) struct Service {
    active: Option<Active>,
    revision: u64,
    lease: ObservationLease,
    last_tick: ElapsedTick,
    logical_end: ElapsedTick,
    started: Instant,
    wall: Duration,
    candidates: u64,
    candidate_limit: u64,
    frames: usize,
    retirement: Option<Retirement>,
}

impl Service {
    /// One source/time bootstrap, one native grant, one finite peer session.
    /// Call only for a new selected ordinary request after independent stop has
    /// been installed. Source/clock work belongs to bootstrap, never to a query.
    pub(super) fn start<F>(settings: &Settings, actor: &Profile, reviewer: &PeerProfile,
        config: &mut Config, driver: &mut FileSupervisedDriver, deadline: &Deadline, time: &mut F)
        -> Result<Self, String>
    where F: FnMut() -> ElapsedTick {
        settings.check_host(config, actor, reviewer)?;
        {
            let host = driver.supervisor().host().map_err(debug)?;
            if host.inspect().control.ledger.epoch != settings.expected_policy_epoch {
                return Err("precheck expected policy epoch does not match the original owner".into());
            }
        }
        let started = Instant::now();
        let runtime_ms = settings.runtime_ms.min(actor.runtime_ms);
        let wall = Duration::from_millis(runtime_ms)
            .min(deadline.wall.saturating_sub(deadline.started.elapsed()));
        let start = time();
        deadline.check(start)?;
        if wall.is_zero() { return Err("precheck bootstrap deadline reached".into()); }
        let requested_end = plus(start, runtime_ms)?.min(deadline.logical);

        driver.prepare_file_intake(&mut config.source, &mut *time).result.map_err(debug)?;
        let now = time();
        deadline.check(now)?;
        if now < start || now >= requested_end || started.elapsed() >= wall {
            return Err("precheck bootstrap exceeded its original clock or elapsed bound".into());
        }
        let (revision, lease) = {
            let host = driver.supervisor().host().map_err(debug)?;
            let current = host.inspect();
            if current.control.ledger.epoch != settings.expected_policy_epoch {
                return Err("precheck policy epoch changed during bootstrap".into());
            }
            if now < current.control.ledger.elapsed.ok_or("precheck owner clock is unavailable")? {
                return Err("precheck clock moved behind the acknowledged source capture".into());
            }
            let lease = host.capture_file_policy_state().map_err(debug)?.lease()
                .ok_or("precheck requires the original timed source observation")?;
            lease.validate_at(now).map_err(debug)?;
            (host.revision(), lease)
        };
        // One owner-bound cache and one native grant for the selected lifetime.
        // No request or reconnect can change disclosure or replenish work.
        let cache = settings.cache.map(|limits|
            driver.supervisor().precheck_cache(limits).map_err(debug)).transpose()?;
        let (wire, disclosure) = if settings.full_policy {
            let (port, disclosure) = driver.supervisor().disclose_full_precheck(
                revision, settings.disclosure_id, settings.query_limit, cache.as_ref()).map_err(debug)?;
            (FilePrecheckWire::with_policy(port, settings.exchange_limit).map_err(debug)?, disclosure)
        } else {
            let (port, disclosure) = match cache.as_ref() {
                Some(cache) => driver.supervisor().disclose_exact_precheck_cached(
                    revision, settings.disclosure_id, settings.query_limit, cache),
                None => driver.supervisor().disclose_exact_precheck(
                    revision, settings.disclosure_id, settings.query_limit),
            }.map_err(debug)?;
            (FilePrecheckWire::new(port, settings.exchange_limit).map_err(debug)?, disclosure)
        };
        let session = FilePrecheckPeerSession::new(actor.actor, wire, MAX_PRECHECK_FRAME_BYTES,
            settings.connection_limit).map_err(debug)?;
        let logical_end = requested_end.min(lease.expires_at());
        // Preallocation and bootstrap may take time. Recheck BEFORE exposing the
        // named listener; nothing may extend the source read-start lease.
        let before_bind = time();
        deadline.check(before_bind)?;
        if before_bind < now || before_bind >= logical_end || started.elapsed() >= wall {
            return Err("precheck bootstrap expired before listener creation".into());
        }
        lease.validate_at(before_bind).map_err(debug)?;
        let socket = BoundSocket::bind(&settings.socket, None)?;
        settings.socket_profile(actor).secure_socket()?;
        let mut service = Self { active: Some(Active { socket, session, disclosure }),
            revision, lease, last_tick: before_bind, logical_end, started, wall,
            candidates: 0, candidate_limit: settings.candidate_limit, frames: 0,
            retirement: None };
        // Binding is synchronous as in the existing service. No accepted socket
        // can be read until the next cooperative deadline/revision checkpoint.
        let after_bind = time();
        if let Some(reason) = service.invalid(driver, after_bind) {
            service.retire_with(reason);
            return Err("precheck listener setup exceeded its original bound".into());
        }
        service.last_tick = after_bind;
        eprintln!("Exact precheck endpoint ready: {:?}; original disclosure={}",
            settings.socket, settings.disclosure_id);
        Ok(service)
    }

    /// Bounded sidecar-only scheduling. A rejected peer, truncated frame or lost
    /// response cannot invoke the outer workflow's stop/drain mutation path.
    pub(super) fn checkpoint(&mut self, driver: &FileSupervisedDriver, now: ElapsedTick) {
        if self.retirement.is_some() { return; }
        if let Some(reason) = self.invalid(driver, now) {
            self.retire_with(reason);
            return;
        }
        self.last_tick = now;
        let active = self.active.as_mut().expect("active precheck service");
        if active.session.status().active.is_none() {
            if self.candidates == self.candidate_limit {
                self.retire_with(Retirement::CandidateLimit);
                return;
            }
            let status = active.session.status();
            if status.connections_admitted == status.connection_limit {
                self.retire_with(Retirement::ConnectionLimit);
                return;
            }
            let socket = match active.socket.listener.accept() {
                Ok((socket, _)) => socket,
                Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => return,
                Err(_) => { self.retire_with(Retirement::TransportFailure); return; }
            };
            self.candidates += 1;
            match active.session.attach(socket) {
                Ok(_) => {}
                Err(PeerRefusal::CredentialsRejected) => {
                    if self.candidates == self.candidate_limit {
                        self.retire_with(Retirement::CandidateLimit);
                    }
                    return;
                }
                Err(_) => { self.retire_with(Retirement::TransportFailure); return; }
            }
        }
        let report = match active.session.drive(DriveBudget { frames: 1, ..DriveBudget::default() }) {
            Ok(report) => report,
            Err(_) => { self.retire_with(Retirement::TransportFailure); return; }
        };
        self.frames += report.progress.frames;
        if report.status.failure.is_some() {
            self.retire_with(Retirement::TransportFailure);
        } else if let ChannelState::Closed(reason) = report.status.channel {
            match reason {
                CloseReason::PeerClosed => { active.session.disconnect(); }
                CloseReason::ExchangeLimit => self.retire_with(Retirement::ExchangeLimit),
                CloseReason::TruncatedFrame | CloseReason::FrameTooLarge | CloseReason::IoFailure => {
                    self.retire_with(Retirement::TransportFailure);
                }
            }
        }
    }

    /// Retire before effect review or whenever the containing service exits.
    /// This revokes this disclosure and closes only its listener/connections.
    pub(super) fn retire(&mut self) { self.retire_with(Retirement::SupervisorEnded); }

    fn invalid(&self, driver: &FileSupervisedDriver, now: ElapsedTick) -> Option<Retirement> {
        if now < self.last_tick { return Some(Retirement::ClockMovedBack); }
        if now >= self.logical_end || self.lease.validate_at(now).is_err()
            || self.started.elapsed() >= self.wall { return Some(Retirement::Expired); }
        match driver.supervisor().host() {
            Ok(host) if host.revision() == self.revision && host.storage_failure().is_none() => None,
            _ => Some(Retirement::OwnerChanged),
        }
    }

    fn retire_with(&mut self, reason: Retirement) {
        let Some(mut active) = self.active.take() else { return; };
        active.session.revoke();
        self.retirement = Some(reason);
        // Aggregate operator diagnostics only: no proposal, predicate or reply.
        eprintln!("Exact precheck ingress retired: {reason:?}; candidates={}; frames={}; remaining_queries={}",
            self.candidates, self.frames, active.disclosure.remaining_queries());
        // Drop the sole disclosure custodian and the inode-owned listener.
    }
}

#[cfg(test)]
#[path = "precheck/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "precheck/cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
#[path = "precheck/policy_tests.rs"]
mod policy_tests;
