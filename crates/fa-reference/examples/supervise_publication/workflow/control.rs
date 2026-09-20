//! Service the native stop-only protocol at the SAME workflow's safe boundaries.
//! Kernel admission precedes protocol I/O; there is no unchecked control listener.
use super::{Config, Deadline, FileHumanReviewer, FileSupervisedDriver, PeerProfile};
use fa_reference::action::ElapsedTick;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

#[cfg(target_os = "linux")]
pub(crate) fn socket_path(profile: &PeerProfile, operation: u64) -> PathBuf {
    profile.socket(operation).with_file_name(format!("stop-{operation}.sock"))
}

pub(super) struct Control {
    #[cfg(target_os = "linux")]
    service: Option<Service>,
}
impl Control {
    pub(super) fn new(config: &Config, operation: u64, profile: Option<&PeerProfile>) -> Result<Self, String> {
        if let Some(profile) = profile { profile.check_host(config)?; }
        #[cfg(target_os = "linux")]
        { Ok(Self { service: profile.map(|p| Service::new(config, operation, p)).transpose()? }) }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (config, operation);
            if profile.is_some() { return Err("stop control requires Linux peer credentials".into()); }
            Ok(Self {})
        }
    }

    /// No connection means no journal access or time observation. An admitted
    /// control exchange pauses forward work under the original workflow deadline
    /// and a bounded session timeout. It can only stop, never approve an effect.
    pub(super) fn checkpoint<F>(&mut self, driver: &mut FileSupervisedDriver,
        reviewer: &FileHumanReviewer, deadline: &Deadline, time: &mut F) -> Result<bool, String>
    where F: FnMut() -> ElapsedTick {
        #[cfg(target_os = "linux")]
        if let Some(service) = &mut self.service { return service.checkpoint(driver, reviewer, deadline, time); }
        let _ = (driver, reviewer, deadline, time);
        Ok(false)
    }
}

#[cfg(target_os = "linux")]
use super::{Admission, BoundSocket, Duration, Instant, debug, io, nonce, pause};
#[cfg(target_os = "linux")]
use crate::peers::ReviewStream;
#[cfg(target_os = "linux")]
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
    StopControlConnection, StopControlPhase, StopControlProgress,
};
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;

#[cfg(target_os = "linux")]
struct Service {
    socket: BoundSocket,
    admission: Admission,
    operation: u64,
    runtime: Duration,
    cleanup: Duration,
    poll_ms: u64,
    outcome: Option<Result<bool, String>>,
}
#[cfg(target_os = "linux")]
impl Service {
    fn new(config: &Config, operation: u64, profile: &PeerProfile) -> Result<Self, String> {
        if operation == 0 { return Err("stop operation must be nonzero".into()); }
        let admission = Admission::new(Some(profile))?;
        let path = socket_path(profile, operation);
        let socket = BoundSocket::bind(&path, Some(profile))?;
        eprintln!("Independent stop endpoint ready: {path:?}");
        Ok(Self { socket, admission, operation, runtime: Duration::from_millis(profile.runtime_ms),
            cleanup: Duration::from_millis(config.timing.cleanup_ms), poll_ms: profile.poll_ms,
            outcome: None })
    }
    fn checkpoint<F>(&mut self, driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer,
        deadline: &Deadline, time: &mut F) -> Result<bool, String>
    where F: FnMut() -> ElapsedTick {
        if let Some(result) = &self.outcome { return result.clone(); }
        let stream = match self.socket.listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => return Ok(false),
            Err(error) => return Err(debug(error)),
        };
        let verified = match self.admission.admit(stream)? {
            Some(ReviewStream::Checked(socket)) => socket,
            None => return Ok(false),
            Some(ReviewStream::Legacy(_)) => return Err("unchecked stop connection refused".into()),
        };
        // A successful native admission is one-use. Never replace a failed or
        // silent checked connection with a fresh quota or an unchecked socket.
        let result = (|| {
            let mut connection = {
                let host = driver.supervisor().host().map_err(debug)?;
                verified.into_stop_connection(&host, reviewer, self.operation, nonce()?).map_err(debug)?
            };
            self.exchange(&mut connection, driver, deadline, time)
        })();
        self.outcome = Some(result.clone());
        result
    }
    fn exchange<F>(&self, connection: &mut StopControlConnection<UnixStream>,
        driver: &mut FileSupervisedDriver, deadline: &Deadline, time: &mut F) -> Result<bool, String>
    where F: FnMut() -> ElapsedTick {
        let started = Instant::now();
        loop {
            deadline.check(time())?;
            if started.elapsed() >= self.runtime { return Err("authenticated stop session timed out".into()); }
            match connection.step(driver, &mut *time).map_err(debug)? {
                StopControlProgress::Applied => break,
                StopControlProgress::Complete => return Err("stop channel completed without a native application".into()),
                StopControlProgress::Progress | StopControlProgress::Blocked => pause(self.poll_ms),
            }
        }
        let application = connection.application().ok_or("missing native stop result")?.clone();
        // Native stop already ran. Only the historical reply is drained here;
        // a disconnected client must not erase it or cause another application.
        let started = Instant::now();
        while connection.phase() != StopControlPhase::Complete && started.elapsed() < self.cleanup {
            match connection.step(driver, &mut *time) {
                Ok(StopControlProgress::Complete) => break,
                Ok(StopControlProgress::Applied) => return Err("duplicate native stop application".into()),
                Ok(_) => pause(self.poll_ms),
                Err(error) => { eprintln!("Native stop reply was not delivered: {error:?}"); break; }
            }
        }
        if connection.phase() != StopControlPhase::Complete { eprintln!("Stop reply delivery is unconfirmed"); }
        application.result.map_err(debug)?;
        if !application.receipt.drained() {
            return Err(format!("native stop acknowledged; recovery remains {:?}", application.receipt.status));
        }
        Ok(true)
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "control_tests.rs"]
mod tests;
