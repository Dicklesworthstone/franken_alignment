//! Exclusive path ownership plus bounded acceptance; original pool owns all I/O.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorInbox;
use fa_reference::action::consequence::delivery::persistent::requests::actor::source_wire::inbox::pool::{
    FileActorPool, PoolAttachError, PoolBudget, PoolDriveReport,
};
use fa_reference::action::consequence::oversight::evidence_source::EvidenceFile;

struct Endpoint { socket: BoundSocket, profile: Profile, attempted: u64 }
pub(super) struct Listeners(Vec<Endpoint>);
impl Listeners {
    pub(super) fn bind(profiles: &[Profile]) -> Result<Self, String> {
        let mut endpoints = Vec::new();
        endpoints.try_reserve_exact(profiles.len()).map_err(debug)?;
        for profile in profiles {
            let socket = BoundSocket::bind(&profile.socket, None)?;
            profile.secure_socket()?;
            endpoints.push(Endpoint { socket, profile: profile.clone(), attempted: 0 });
        }
        Ok(Self(endpoints))
    }
    pub(super) fn connect(self, port: Port) -> Result<Intake, String> {
        let mut peers = Vec::new();
        peers.try_reserve_exact(self.0.len()).map_err(debug)?;
        for endpoint in &self.0 {
            let p = &endpoint.profile;
            peers.push((p.request, FileActorInbox::new(p.actor, ActorWire::new(port.clone()),
                p.channels(), p.connections).map_err(debug)?));
        }
        let pool = FileActorPool::for_requests(peers).map_err(debug)?;
        Ok(Intake { pool, endpoints: self.0, seen: BTreeSet::new() })
    }
}

pub(super) struct Intake {
    pub(super) pool: FileActorPool,
    endpoints: Vec<Endpoint>,
    seen: BTreeSet<u64>,
}
impl Intake {
    pub(super) fn all_seen(&self) -> bool { self.seen.len() == self.endpoints.len() }
    fn accept(&mut self) -> Result<(), String> {
        // Exactly one nonblocking accept attempt per disconnected endpoint.
        // Wrong credentials count against its lifetime candidate limit. Neither
        // retries nor other endpoints replace that budget or select its peer ID.
        for endpoint in &mut self.endpoints {
            let key = endpoint.profile.request;
            let (_, status, _) = self.pool.statuses().find(|(peer, _, _)| *peer == key)
                .ok_or("missing registered peer")?;
            if status.revoked || status.active.is_some() { continue; }
            let stream = match endpoint.socket.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => continue,
                Err(error) => return Err(debug(error)),
            };
            if endpoint.attempted == endpoint.profile.candidates { return Err("actor candidate quota exhausted".into()); }
            endpoint.attempted += 1;
            match self.pool.attach(key, stream) {
                Ok(_) => {}
                Err(PoolAttachError::Refused(PeerRefusal::CredentialsRejected))
                    if endpoint.attempted < endpoint.profile.candidates => {
                        eprintln!("Rejected actor credentials for request {key}; candidate={}", endpoint.attempted);
                    }
                Err(error) => return Err(debug(error)),
            }
        }
        Ok(())
    }
    fn record(&mut self, report: PoolDriveReport) -> Result<(), String> {
        // Preserve frame observations from completed peers before any later
        // failure. An outer successful pool Result is not success of all visits.
        for visit in report.visits {
            let report = visit.result.map_err(debug)?;
            if report.drive.progress.frames != 0 { self.seen.insert(visit.peer); }
        }
        Ok(())
    }
    pub(super) fn drive<S, F>(&mut self, driver: &mut FileSupervisedDriver, source: &mut S,
        time: F) -> Result<(), String>
    where S: EvidenceFile + ?Sized, F: FnMut() -> ElapsedTick {
        self.accept()?;
        let report = self.pool.drive(driver, source, time, budget()).map_err(debug)?;
        self.record(report)
    }
    pub(super) fn observe(&mut self, driver: &mut FileSupervisedDriver) -> Result<(), String> {
        self.accept()?;
        let report = self.pool.observe_registered(driver, budget()).map_err(debug)?;
        self.record(report)
    }
}
fn budget() -> PoolBudget {
    PoolBudget { total: DriveBudget { frames: 1, ..DriveBudget::default() }, ..PoolBudget::default() }
}
