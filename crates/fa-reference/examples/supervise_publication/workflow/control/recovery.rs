//! The existing authenticated Stop protocol over one sealed recovery cursor.
//! No live owner, reviewer role, effect clock or alternate replay escapes here.
use super::{
    Control, Deadline, Duration, ElapsedTick, Instant, Rc, RefCell, ReviewStream, Service,
    StopControlPhase, StopControlProgress, StopRecoveryApplication, UnixStream, debug, nonce, pause,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
    FileRecoveryStop, StopRecoveryConnection,
};

pub(in crate::workflow) struct RecoveryOutcome {
    pub(in crate::workflow) application: StopRecoveryApplication,
    failure: Option<String>,
}
impl RecoveryOutcome {
    /// An acknowledged admission Stop terminates startup. It cannot be handed
    /// to the caller as a live owner, even when all old obligations are drained.
    pub(in crate::workflow) fn terminal_message(&self) -> String {
        let mut message = match &self.application.result {
            Ok(history) => format!(
                "native admission Stop acknowledged at revision {}; recovery remains {:?}; startup terminated before live custody; no effect was resent",
                history.snapshot().revision, self.application.receipt.status),
            Err(error) => format!(
                "native recovery Stop completion is unconfirmed: {error:?}; startup terminated before live custody; no effect was resent"),
        };
        if let Some(failure) = &self.failure {
            message.push_str("; ");
            message.push_str(failure);
        }
        message
    }
}

impl Control {
    /// Keep the admitted peer and its original finite session while replay
    /// verifies each event. Neither a silent peer nor a failed session can fall
    /// through into ordinary recovery or receive a replacement candidate quota.
    pub(in crate::workflow) fn recovery_pending_peer(&mut self) -> Result<bool, String> {
        let Some(service) = &self.service else { return Ok(false); };
        let mut service = service.try_borrow_mut()
            .map_err(|_| "stop transport is already being driven")?;
        if let Some(result) = &service.outcome {
            return Err(cached_terminal(result));
        }
        let pending = match service.accept_peer() {
            Ok(pending) => pending,
            Err(error) => {
                // An exhausted candidate quota or failed accept is terminal for
                // this SAME command, even if no later candidate is queued.
                service.outcome = Some(Err(error.clone()));
                return Err(error);
            }
        };
        if pending && service.pending_since.is_some_and(|started| started.elapsed() >= service.runtime) {
            let error = "authenticated stop session timed out".to_owned();
            service.outcome = Some(Err(error.clone()));
            return Err(error);
        }
        Ok(pending)
    }

    pub(in crate::workflow) fn recovery_operation(&self) -> Result<u64, String> {
        let service = self.service.as_ref().ok_or("no authenticated recovery Stop service")?
            .try_borrow().map_err(|_| "stop transport is already being driven")?;
        if let Some(result) = &service.outcome {
            return Err(cached_terminal(result));
        }
        if service.pending.is_none() {
            return Err("no authenticated recovery Stop peer".into());
        }
        Ok(service.operation)
    }

    /// Only a sealed native target and the retained verified socket enter this
    /// path. Transport errors retain any native application; they never restart
    /// the original cursor, run a live finalizer or replace the checked socket.
    pub(in crate::workflow) fn stop_recovery<F>(&mut self, target: FileRecoveryStop,
        deadline: &Deadline, time: &mut F) -> Result<RecoveryOutcome, String>
    where F: FnMut() -> ElapsedTick {
        let service = Rc::clone(self.service.as_ref().ok_or("no authenticated recovery Stop service")?);
        let (mut connection, session) = {
            let mut service = service.try_borrow_mut()
                .map_err(|_| "stop transport is already being driven")?;
            if let Some(result) = &service.outcome {
                return Err(cached_terminal(result));
            }
            let prepared = service.prepare_recovery_stop(target);
            // Latch before protocol execution. A nested caller or later attempt
            // cannot reinterpret this one-use session as a fresh live checkpoint.
            service.outcome = Some(Err(match &prepared {
                Ok(_) => "authenticated recovery Stop exchange is already being driven".into(),
                Err(error) => error.clone(),
            }));
            prepared?
        };
        // No RefCell borrow is held across the workflow's budget callback. The
        // acknowledged native value is inspectable before post-ack deadlines.
        let result = exchange(&service, &mut connection, session, deadline, time);
        let terminal = match &result {
            Ok(outcome) => outcome.terminal_message(),
            Err(error) => error.clone(),
        };
        if let Ok(mut service) = service.try_borrow_mut() {
            // Preserve the returned immutable evidence even if an unexpected
            // temporary borrow prevented caching it immediately after step().
            if service.recovery_application.is_none() {
                service.recovery_application = result.as_ref().ok()
                    .map(|outcome| outcome.application.clone());
            }
            service.outcome = Some(Err(terminal));
        }
        result
    }

    #[cfg(test)]
    pub(in crate::workflow) fn recovery_application(&self) -> Option<StopRecoveryApplication> {
        self.service.as_ref()?.try_borrow().ok()?.recovery_application.clone()
    }
}

fn cached_terminal(result: &Result<bool, String>) -> String {
    match result {
        Err(error) => error.clone(),
        Ok(_) => "native Stop session is already complete".into(),
    }
}

struct Session {
    started: Instant,
    runtime: Duration,
    cleanup: Duration,
    poll_ms: u64,
}
impl Session {
    fn check(&self) -> Result<(), String> {
        if self.started.elapsed() >= self.runtime {
            Err("authenticated stop session timed out".into())
        } else {
            Ok(())
        }
    }
}

impl Service {
    fn prepare_recovery_stop(&mut self, target: FileRecoveryStop)
        -> Result<(StopRecoveryConnection<UnixStream>, Session), String>
    {
        if target.request().operation != self.operation {
            return Err("recovery Stop operation differs from the admitted endpoint".into());
        }
        let session = Session {
            started: self.pending_since.ok_or("missing original recovery Stop session")?,
            runtime: self.runtime, cleanup: self.cleanup, poll_ms: self.poll_ms,
        };
        session.check()?;
        let verified = match self.pending.take() {
            Some(ReviewStream::Checked(socket)) => socket,
            _ => return Err("unchecked recovery Stop connection refused".into()),
        };
        let connection = verified.into_recovery_stop_connection(target, nonce()?).map_err(debug)?;
        Ok((connection, session))
    }
}

fn exchange<F>(service: &Rc<RefCell<Service>>, connection: &mut StopRecoveryConnection<UnixStream>,
    session: Session, deadline: &Deadline, time: &mut F) -> Result<RecoveryOutcome, String>
where F: FnMut() -> ElapsedTick {
    let mut failures = Vec::new();
    let (application, cleanup_started) = loop {
        deadline.check(time())?;
        session.check()?;
        let progress = connection.step();
        // A native result may precede a fallible receipt encoding/write. Inspect
        // it even when step returned an error; the known Stop cannot be erased.
        if let Some(application) = connection.application().cloned() {
            let cleanup_started = Instant::now();
            match service.try_borrow_mut() {
                Ok(mut service) => service.recovery_application = Some(application.clone()),
                Err(_) => failures.push("native recovery Stop result could not be cached".into()),
            }
            if let Err(error) = progress {
                failures.push(format!("native Stop reply failed: {error:?}"));
            }
            break (application, cleanup_started);
        }
        match progress.map_err(debug)? {
            StopControlProgress::Applied | StopControlProgress::Complete => {
                return Err("recovery Stop channel completed without a native application".into());
            }
            StopControlProgress::Progress | StopControlProgress::Blocked => pause(session.poll_ms),
        }
    };
    // This is only the active workflow's budget observation. The sealed native
    // finalizer never acquires or journals a clock, and this late check cannot
    // revoke an acknowledgment or skip independently selected custody exports.
    if let Err(error) = deadline.check(time()) {
        failures.push(format!("{error} after native recovery Stop application"));
    }
    if let Err(error) = session.check() {
        failures.push(format!("{error} after native recovery Stop application"));
    }
    // Only the original bounded cleanup allowance drains the immutable reply.
    // No startup/session budget is renewed and no effect-clock callback runs.
    while !matches!(connection.phase(), StopControlPhase::Complete | StopControlPhase::Failed)
        && cleanup_started.elapsed() < session.cleanup
    {
        match connection.step() {
            Ok(StopControlProgress::Complete) => break,
            Ok(StopControlProgress::Applied) => {
                failures.push("duplicate native recovery Stop application".into());
                break;
            }
            Ok(StopControlProgress::Progress | StopControlProgress::Blocked) => pause(session.poll_ms),
            Err(error) => {
                failures.push(format!("native Stop reply failed: {error:?}"));
                break;
            }
        }
    }
    if connection.phase() != StopControlPhase::Complete {
        failures.push("Stop reply delivery is unconfirmed".into());
    }
    Ok(RecoveryOutcome { application,
        failure: if failures.is_empty() { None } else { Some(failures.join("; ")) } })
}
