//! Service the native stop-only protocol at the SAME workflow's safe boundaries.
//! Kernel admission precedes protocol I/O; there is no unchecked control listener.
use super::{Config, Deadline, FileHumanReviewer, FileSupervisedDriver, PeerProfile};
use fa_reference::action::ElapsedTick;
#[cfg(target_os = "linux")]
#[path = "control/recovery.rs"]
mod recovery;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::{cell::RefCell, rc::Rc};

#[cfg(target_os = "linux")]
pub(crate) fn socket_path(profile: &PeerProfile, operation: u64) -> PathBuf {
    profile
        .socket(operation)
        .with_file_name(format!("stop-{operation}.sock"))
}

pub(super) struct Control {
    #[cfg(target_os = "linux")]
    service: Option<Rc<RefCell<Service>>>,
    // Shared across one synchronous workflow and its final reply grace. A
    // failed drain is retained too; pumping the peer must not retry it.
    native_outcome: Rc<RefCell<Option<Result<bool, String>>>>,
}
impl Control {
    pub(super) fn new(
        config: &Config,
        operation: u64,
        profile: Option<&PeerProfile>,
    ) -> Result<Self, String> {
        if let Some(profile) = profile {
            profile.check_host(config)?;
        }
        #[cfg(target_os = "linux")]
        {
            Ok(Self {
                service: profile
                    .map(|p| {
                        Service::new(config, operation, p)
                            .map(|service| Rc::new(RefCell::new(service)))
                    })
                    .transpose()?,
                native_outcome: Rc::new(RefCell::new(None)),
            })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (config, operation);
            if profile.is_some() {
                return Err("stop control requires Linux peer credentials".into());
            }
            Ok(Self { native_outcome: Rc::new(RefCell::new(None)) })
        }
    }

    /// Share only the SAME stop transport with another synchronous workflow.
    /// Socket ownership, credentials, candidate quota and terminal outcome remain
    /// one object. No reviewer role, effect permit or new admission budget exists.
    #[cfg(target_os = "linux")]
    pub(super) fn share(&self) -> Self {
        Self {
            service: self.service.as_ref().map(Rc::clone),
            native_outcome: Rc::clone(&self.native_outcome),
        }
    }

    /// Accept and retain only an authenticated stop peer. A native learned
    /// review uses this to cancel its numerical owner and take its ORIGINAL
    /// terminal handoff before the existing driver applies the stop protocol.
    /// Rejected candidates do not cancel work or touch the journal/clock.
    #[cfg(target_os = "linux")]
    pub(super) fn pending_peer(&mut self) -> Result<bool, String> {
        match &self.service {
            Some(service) => service
                .try_borrow_mut()
                .map_err(|_| "stop transport is already being driven")?
                .accept_peer(),
            None => Ok(false),
        }
    }

    /// Observe only an already acknowledged Stop on this original owner. The
    /// native fence receipt remains historical and makes no drain claim; this
    /// independent host cleanup uses its exact request, closes the old driver
    /// job before sampling time, and never appends a second Stop.
    ///
    /// Retain both success and failure across shared control handles. Terminal
    /// reply pumping cannot repeat a failed sweep or replenish reserved space.
    pub(super) fn native_stop<F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        time: &mut F,
    ) -> Result<bool, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        let mut outcome = self.native_outcome.try_borrow_mut()
            .map_err(|_| "native stop cleanup is already being driven")?;
        if let Some(result) = outcome.as_ref() {
            return result.clone();
        }
        // The original Stop transport owns its own one-use outcome. Do not
        // retry its failed drain or relabel its acknowledgment through this path.
        #[cfg(target_os = "linux")]
        if let Some(service) = &self.service {
            let service = service.try_borrow()
                .map_err(|_| "stop transport is already being driven")?;
            if let Some(result) = &service.outcome {
                return result.clone();
            }
        }
        let request = {
            let host = driver.supervisor().host().map_err(super::debug)?;
            host.inspect().stop.map(|receipt| receipt.request())
        };
        let Some(request) = request else { return Ok(false); };
        // A caught clock/cleanup unwind must not turn the next poll into a retry.
        *outcome = Some(Err("native stop cleanup did not acknowledge completion".into()));
        let result = (|| {
            let stopped = driver.stop_with_recovery_reserve(request, &mut *time)
                .map_err(super::debug)?;
            let progress = match stopped.drain.map_err(super::debug)? {
                fa_reference::action::consequence::delivery::persistent::observed::driver::CapacityDrain::AlreadyDrained(progress) => progress,
                fa_reference::action::consequence::delivery::persistent::observed::driver::CapacityDrain::Advanced(sweep) => sweep.progress,
            };
            if !progress.drained() {
                return Err("native admission fence acknowledged; endpoint liabilities remain unresolved and charged".into());
            }
            Ok(true)
        })();
        *outcome = Some(result.clone());
        result
    }

    pub(super) fn native_cleanup_started(&self) -> bool {
        if self.native_outcome.try_borrow().map_or(true, |outcome| outcome.is_some()) {
            return true;
        }
        // The separately authenticated Stop transport owns an equally final
        // one-use outcome, including a failed drain. Automatic failure cleanup
        // must not replay it or discard a reply that this service still owns.
        #[cfg(target_os = "linux")]
        if let Some(service) = &self.service {
            return service.try_borrow().map_or(true, |service| {
                service.application_started || service.recovery_application.is_some()
            });
        }
        false
    }

    /// Without an acknowledged native Stop or a connection, this samples no
    /// time and performs no journal mutation. An admitted
    /// control exchange pauses forward work under the original workflow deadline
    /// and a bounded session timeout. It can only stop, never approve an effect.
    pub(super) fn checkpoint<F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        reviewer: &FileHumanReviewer,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<bool, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        if self.native_stop(driver, time)? { return Ok(true); }
        #[cfg(target_os = "linux")]
        if let Some(service) = &self.service {
            let mut service = service
                .try_borrow_mut()
                .map_err(|_| "stop transport is already being driven")?;
            return service.checkpoint(driver, reviewer, deadline, time);
        }
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
    StopControlConnection, StopControlPhase, StopControlProgress, StopRecoveryApplication,
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
    // A protocol refusal or silent peer is not a native cleanup attempt.
    application_started: bool,
    pending: Option<ReviewStream>,
    pending_since: Option<Instant>,
    recovery_application: Option<StopRecoveryApplication>,
}
#[cfg(target_os = "linux")]
impl Service {
    fn new(config: &Config, operation: u64, profile: &PeerProfile) -> Result<Self, String> {
        if operation == 0 {
            return Err("stop operation must be nonzero".into());
        }
        let admission = Admission::new(Some(profile))?;
        let path = socket_path(profile, operation);
        let socket = BoundSocket::bind(&path, Some(profile))?;
        eprintln!("Independent stop endpoint ready: {path:?}");
        Ok(Self { socket, admission, operation, runtime: Duration::from_millis(profile.runtime_ms),
            cleanup: Duration::from_millis(config.timing.cleanup_ms), poll_ms: profile.poll_ms,
            outcome: None, application_started: false, pending: None, pending_since: None,
            recovery_application: None })
    }
    fn accept_peer(&mut self) -> Result<bool, String> {
        if self.outcome.is_some() || self.pending.is_some() {
            return Ok(true);
        }
        let stream = match self.socket.listener.accept() {
            Ok((stream, _)) => stream,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(false);
            }
            Err(error) => return Err(debug(error)),
        };
        self.pending = match self.admission.admit(stream)? {
            Some(stream @ ReviewStream::Checked(_)) => Some(stream),
            None => return Ok(false),
            Some(ReviewStream::Legacy(_)) => return Err("unchecked stop connection refused".into()),
        };
        // Recovery retains this SAME authenticated session through replay. Its
        // finite allowance starts at kernel admission, never at Ready or retry.
        self.pending_since = Some(Instant::now());
        Ok(true)
    }
    fn checkpoint<F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        reviewer: &FileHumanReviewer,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<bool, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        if let Some(result) = &self.outcome {
            return result.clone();
        }
        if !self.accept_peer()? {
            return Ok(false);
        }
        let verified = match self.pending.take() {
            Some(ReviewStream::Checked(socket)) => socket,
            _ => return Err("unchecked stop connection refused".into()),
        };
        // A successful native admission is one-use. Never replace a failed or
        // silent checked connection with a fresh quota or an unchecked socket.
        let result = (|| {
            let mut connection = {
                let host = driver.supervisor().host().map_err(debug)?;
                verified
                    .into_stop_connection(&host, reviewer, self.operation, nonce()?)
                    .map_err(debug)?
            };
            let result = self.exchange(&mut connection, driver, deadline, time);
            // Even a failed sweep or receipt encode/write retains the one-use
            // native application. Earlier transport failures still require the
            // original automatic failure Stop, rather than leaving work live.
            self.application_started = connection.application().is_some();
            result
        })();
        self.outcome = Some(result.clone());
        result
    }
    fn exchange<F>(
        &self,
        connection: &mut StopControlConnection<UnixStream>,
        driver: &mut FileSupervisedDriver,
        deadline: &Deadline,
        time: &mut F,
    ) -> Result<bool, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        let started = Instant::now();
        loop {
            deadline.check(time())?;
            if started.elapsed() >= self.runtime {
                return Err("authenticated stop session timed out".into());
            }
            match connection.step(driver, &mut *time).map_err(debug)? {
                StopControlProgress::Applied => break,
                StopControlProgress::Complete => {
                    return Err("stop channel completed without a native application".into());
                }
                StopControlProgress::Progress | StopControlProgress::Blocked => pause(self.poll_ms),
            }
        }
        let application = connection
            .application()
            .ok_or("missing native stop result")?
            .clone();
        // Native stop already ran. Only the historical reply is drained here;
        // a disconnected client must not erase it or cause another application.
        let started = Instant::now();
        while connection.phase() != StopControlPhase::Complete && started.elapsed() < self.cleanup {
            match connection.step(driver, &mut *time) {
                Ok(StopControlProgress::Complete) => break,
                Ok(StopControlProgress::Applied) => {
                    return Err("duplicate native stop application".into());
                }
                Ok(_) => pause(self.poll_ms),
                Err(error) => {
                    eprintln!("Native stop reply was not delivered: {error:?}");
                    break;
                }
            }
        }
        if connection.phase() != StopControlPhase::Complete {
            eprintln!("Stop reply delivery is unconfirmed");
        }
        application.result.map_err(debug)?;
        if !application.receipt.drained() {
            return Err(format!(
                "native stop acknowledged; recovery remains {:?}",
                application.receipt.status
            ));
        }
        Ok(true)
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "control_tests.rs"]
mod tests;
