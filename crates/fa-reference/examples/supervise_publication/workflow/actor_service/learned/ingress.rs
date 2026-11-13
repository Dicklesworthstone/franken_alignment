//! Dedicated actor ingress for one ORIGINAL learned release. The peer never
//! receives a numerical owner, policy reader, helper input or approval role.
use super::{Config, Loaded, PeerProfile, Deadline, Duration, Instant, Control,
    FileOversight, FileSupervisedDriver, FileHumanReviewer, ElapsedTick,
    FileLearnedTextStreamActorPort, LearnedTextRelease, ActorWire, Command,
    JournalError, FileRequestDisposition, ActionState, Error, debug, encode_command};
use super::super::{Profile, BoundSocket, PeerSession, PeerRefusal, drive_budget};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorSupervisor, learned_intake::LearnedStreamAdmission,
};
use fa_reference::action::consequence::oversight::actor_transport::DriveReport;
use std::{io, path::Path};

const USAGE: &str = "live learned publication accepts --actor-profile ACTOR_PROFILE with a reviewer profile and at most one continuation selection; receipt, stop and audit modes cannot admit an actor";
pub(super) type Supervisor = FileActorSupervisor<FileOversight>;

/// Validate the complete live option grammar before stripping the actor pair.
/// An absent pair preserves the existing legacy grammar and behavior exactly.
pub(super) fn take_option(args: &[String]) -> Result<(Vec<String>, Option<&Path>), String> {
    if !args.iter().any(|arg| arg == "--actor-profile") { return Ok((args.to_vec(), None)); }
    if args.len() < 6 || args[0] != "create-learned-generated"
        || args[1..4].iter().any(|arg| arg.is_empty() || arg.starts_with("--")) {
        return Err(USAGE.into());
    }
    let mut output = args[..4].to_vec();
    let mut selected = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut continuation = false;
    let mut at = 4;
    while at < args.len() {
        let flag = args[at].as_str();
        if !seen.insert(flag) { return Err(USAGE.into()); }
        let is_continuation = matches!(flag, "--continue-generation" | "--after-message" | "--finish-after-message");
        if is_continuation && std::mem::replace(&mut continuation, true) { return Err(USAGE.into()); }
        if flag == "--continue-generation" {
            output.push(args[at].clone()); at += 1; continue;
        }
        if !matches!(flag, "--actor-profile" | "--after-message" | "--finish-after-message"
            | "--history-anchor" | "--retain-history-anchor" | "--topology-profile") {
            return Err(USAGE.into());
        }
        let value = args.get(at + 1).ok_or(USAGE)?;
        if value.is_empty() || value.starts_with("--") { return Err(USAGE.into()); }
        if flag == "--actor-profile" { selected = Some(Path::new(value)); }
        else { output.extend_from_slice(&args[at..at + 2]); }
        at += 2;
    }
    Ok((output, selected))
}

pub(super) fn check(config: &Config, loaded: &Loaded, reviewer: &PeerProfile, actor: &Profile)
    -> Result<(), String>
{
    actor.check_host(config)?;
    if actor.request != loaded.request {
        return Err("actor profile and learned recipe must select the same request".into());
    }
    if actor.socket == reviewer.socket(loaded.request)
        || actor.socket == crate::workflow::control::socket_path(reviewer, loaded.request) {
        return Err("actor, human-review and stop sockets must be distinct".into());
    }
    Ok(())
}

/// Claim transport before opening/creating a journal. Existing paths are never
/// unlinked; failed setup releases only this invocation's own socket.
pub(super) struct Bound { profile: Profile, socket: BoundSocket }
impl Bound {
    pub(super) fn bind(profile: &Profile) -> Result<Self, String> {
        let socket = BoundSocket::bind(&profile.socket, None)?;
        profile.secure_socket()?;
        Ok(Self { profile: profile.clone(), socket })
    }
    pub(super) fn attach(self, port: FileLearnedTextStreamActorPort,
        release: LearnedTextRelease, deadline: ElapsedTick) -> Result<Intake, String>
    {
        let selected = LearnedStreamAdmission::new(self.profile.request, release, deadline).map_err(debug)?;
        let session = PeerSession::new(self.profile.actor, ActorWire::new(port),
            self.profile.channels(), self.profile.connections).map_err(debug)?;
        Ok(Intake { profile: self.profile, socket: self.socket, session, selected, candidates: 0,
            release, precheck: None })
    }
}

pub(super) struct Intake {
    profile: Profile,
    socket: BoundSocket,
    session: PeerSession<FileLearnedTextStreamActorPort>,
    selected: LearnedStreamAdmission,
    candidates: u64,
    release: LearnedTextRelease,
    precheck: Option<super::precheck::Selection>,
}
impl Intake {
    pub(super) fn select_precheck(&mut self, settings: &super::precheck::Settings,
        reviewer: &PeerProfile)
    {
        self.precheck = Some(super::precheck::Selection {
            settings: settings.clone(), reviewer: reviewer.clone(),
        });
    }

    /// Qualification may advance authority. Check its actual acknowledged epoch
    /// before paying for generation; bootstrap checks it again at the final cut.
    pub(super) fn check_precheck_epoch(&self, driver: &FileSupervisedDriver) -> Result<(), String> {
        if let Some(selected) = &self.precheck {
            let host = driver.supervisor().host().map_err(debug)?;
            super::precheck::check_owner_epoch(&selected.settings, &host)?;
        }
        Ok(())
    }

    fn accept(&mut self) -> Result<(), String> {
        if self.session.status().active.is_some() { return Ok(()); }
        match self.socket.listener.accept() {
            Ok((socket, _)) => {
                if self.candidates == self.profile.candidates { return Err("learned actor candidate quota exhausted".into()); }
                self.candidates += 1;
                match self.session.attach(socket) {
                    Ok(_) => {},
                    Err(PeerRefusal::CredentialsRejected) if self.candidates < self.profile.candidates => {
                        eprintln!("Rejected learned actor credentials; candidate={}", self.candidates);
                    }
                    Err(error) => return Err(debug(error)),
                }
            }
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => {},
            Err(error) => return Err(debug(error)),
        }
        Ok(())
    }
    fn finish_drive(&mut self, report: DriveReport) {
        if report.status.closed() { self.session.disconnect(); }
    }

    /// New admission exists only here, AFTER original generation or explicit
    /// finish preparation. Every source read remains on the native scoped gate.
    #[cfg(test)]
    pub(super) fn admit<F>(&mut self, driver: &mut FileSupervisedDriver, config: &mut Config,
        reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F)
        -> Result<bool, String>
    where F: FnMut() -> ElapsedTick {
        self.admit_serviced(driver, config, reviewer, control, deadline, time, &mut None)
    }

    pub(super) fn admit_serviced<F>(&mut self, driver: &mut FileSupervisedDriver, config: &mut Config,
        reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
        operator: &mut Option<super::operator::Service>) -> Result<bool, String>
    where F: FnMut() -> ElapsedTick {
        let mut precheck = None;
        if let Some(selected) = self.precheck.take() {
            // This one disclosure begins only after completed generation or
            // explicit Finish preparation. Stop wins before its source capture.
            deadline.check(time())?;
            if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
            super::operator::checkpoint_driver(operator, driver);
            if control.native_stop(driver, time)? { return Ok(false); }
            precheck = Some(super::precheck::Service::start_learned(&selected.settings,
                &self.profile, &selected.reviewer, config, driver, deadline, time, self.release)?);
        }
        let result = (|| loop {
            deadline.check(time())?;
            if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
            super::operator::checkpoint_driver(operator, driver);
            if control.native_stop(driver, time)? { return Ok(false); }
            if let Some(service) = &mut precheck { service.checkpoint(driver, time()); }
            self.accept()?;
            deadline.check(time())?;
            if self.session.status().active.is_some() {
                let report = driver.supervisor_mut().drive_learned_text_stream_request_from_policy_file(
                    &mut self.session, self.selected, &mut config.source, &mut *time, drive_budget()).map_err(debug)?;
                self.finish_drive(report.drive);
            }
            let status = driver.supervisor().host().map_err(debug)?.request_status(self.selected.request());
            match status {
                Ok(_) => return Ok(true),
                Err(JournalError::Contract(Error::Missing)) => {},
                Err(error) => return Err(debug(error)),
            }
            deadline.check(time())?;
            crate::workflow::pause(self.profile.poll_ms.min(50));
        })();
        // No query can cross into review or acquire a refreshed grant after
        // source-aware Submit, a refusal that changed the owner, Stop or expiry.
        if let Some(service) = &mut precheck { service.retire(); }
        result
    }

    /// Drive only the original authenticated observation transport. A
    /// quarantined owner can still return its original unavailable response;
    /// this path never replaces that response with a local status assertion.
    fn observe_transport(&mut self, supervisor: &mut Supervisor) -> Result<(), String> {
        self.accept()?;
        if self.session.status().active.is_some() {
            let report = supervisor.drive_learned_text_stream_request_observe(
                &mut self.session, self.selected, drive_budget()).map_err(debug)?;
            self.finish_drive(report);
        }
        Ok(())
    }

    /// Dedicated observation phase: no source reader or clock is accepted.
    /// Retrying cannot admit another request even with a preinstalled snapshot.
    /// Cancellation still uses the original ledger, including unknown charges.
    fn observe(&mut self, supervisor: &mut Supervisor) -> Result<bool, String> {
        self.observe_transport(supervisor)?;
        match supervisor.host().map_err(debug)?.request_status(self.selected.request()) {
            Ok(status) => Ok(matches!(status.disposition,
                FileRequestDisposition::Admitted { stage: ActionState::Cancelled, .. })),
            Err(JournalError::Contract(Error::Missing)) => Ok(false),
            Err(error) => Err(debug(error)),
        }
    }

    /// Recover the operator's local observation ticket using the exact accepted
    /// original deadline, not the maximum offered to the peer. No policy read,
    /// fresh request or deadline rewriting is allowed during this exact retry.
    pub(super) fn observe_original(&self, driver: &FileSupervisedDriver,
        wire: &mut ActorWire<FileLearnedTextStreamActorPort>, release: LearnedTextRelease,
        after: Option<u64>) -> Result<ElapsedTick, String>
    {
        let host = driver.supervisor().host().map_err(debug)?;
        let original = match after {
            None => host.learned_text_stream_message_request(self.selected.request()),
            Some(after) => host.learned_text_stream_finish_request(after, self.selected.request()),
        }.map_err(debug)?;
        let revision = host.revision(); drop(host);
        let proposal = FileLearnedTextStreamActorPort::encode_release(self.selected.request(), release,
            original.deadline).map_err(debug)?;
        let response = wire.exchange(&encode_command(&Command::Submit {
            request: self.selected.request(), proposal }).map_err(debug)?);
        if response.result.is_err() || driver.supervisor().host().map_err(debug)?.revision() != revision {
            return Err("original learned actor ticket could not be recovered without mutation".into());
        }
        Ok(original.deadline)
    }

    /// Bounded final observation service inside the ORIGINAL lifetime, not a
    /// fresh admission or a peer-delivery acknowledgement. No failure resends an
    /// effect or erases the operator's original final response.
    #[cfg(test)]
    pub(super) fn finish<F>(&mut self, supervisor: &mut Supervisor, deadline: &Deadline,
        time: &mut F) -> Result<(), String>
    where F: FnMut() -> ElapsedTick {
        let started = Instant::now();
        let window = Duration::from_millis(self.profile.reply_ms);
        while started.elapsed() < window && deadline.check(time()).is_ok() {
            // Allow the existing wire to flush ControllerUnavailable even when
            // the quarantined ledger cannot supply a live cancellation status.
            self.observe_transport(supervisor)?;
            let remaining = window.saturating_sub(started.elapsed());
            let pause = self.profile.poll_ms.min(50)
                .min(u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX));
            if pause == 0 { break; }
            crate::workflow::pause(pause);
        }
        self.session.revoke();
        Ok(())
    }

    /// Terminal grace keeps both original observation endpoints available.
    /// Stop is checked first, including after an already acknowledged Stop; no
    /// result can restart numerical work, review, admission or an effect.
    pub(super) fn finish_serviced<F>(&mut self, driver: &mut FileSupervisedDriver,
        reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
        operator: &mut Option<super::operator::Service>) -> Result<(), String>
    where F: FnMut() -> ElapsedTick {
        let started = Instant::now();
        let window = Duration::from_millis(self.profile.reply_ms);
        let mut failure = None;
        let mut actor_transport_failed = false;
        while started.elapsed() < window && deadline.check(time()).is_ok() {
            if let Err(error) = control.checkpoint(driver, reviewer, deadline, time) {
                // The original one-use Stop service retains its first failed
                // outcome. Keep reporting it without suppressing the actor's
                // original terminal result, including ControllerUnavailable
                // from a quarantined owner. This phase admits no new work.
                failure.get_or_insert(error);
            }
            super::operator::checkpoint_driver(operator, driver);
            if let Err(error) = control.native_stop(driver, time) {
                // Keep flushing the original historical reply after cleanup
                // failure; the shared control will not repeat its drain.
                failure.get_or_insert(error);
            }
            if !actor_transport_failed {
                if let Err(error) = self.observe_transport(driver.supervisor_mut()) {
                    failure = Some(match failure {
                        Some(original) => format!("{original}; actor response transport: {error}"),
                        None => error,
                    });
                    actor_transport_failed = true;
                    // Stop the failed actor transport, retaining the independent
                    // operator's original bounded historical reply opportunity.
                    self.session.revoke();
                }
            }
            let remaining = window.saturating_sub(started.elapsed());
            let pause = self.profile.poll_ms.min(50)
                .min(u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX));
            if pause == 0 { break; }
            crate::workflow::pause(pause);
        }
        self.session.revoke();
        failure.map_or(Ok(()), Err)
    }
}
impl Drop for Intake { fn drop(&mut self) { self.session.revoke(); } }

pub(super) fn observe(intake: &mut Option<Intake>, supervisor: &mut Supervisor) -> Result<bool, String> {
    intake.as_mut().map_or(Ok(false), |intake| intake.observe(supervisor))
}
