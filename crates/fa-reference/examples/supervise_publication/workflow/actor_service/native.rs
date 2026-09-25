//! Native monitored generation feeding the ORIGINAL source-only publication gate.
//! Stdout carries a submission reference, NEVER unapproved generated text.
mod recipe;
#[cfg(test)]
mod tests;

use super::*;
use super::super::FileHumanReviewer;
use super::super::control::Control;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::GenerationFinish;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;
use fa_reference::action::consequence::delivery::persistent::observed::stream::generated::FileTextMessageRequest;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{FileActorInbox, FileGeneratedTextActorPort};
use fa_reference::action::consequence::delivery::persistent::requests::actor::source_wire::inbox::pool::{
    FileActorPool, PoolAttachError, PoolBudget, PoolDriveReport,
};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use std::io::Write;
use recipe::Inputs;

const USAGE: &str = "serve-create CONFIG ACTOR_PROFILE REVIEWER_PROFILE --native-text RECIPE";

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    if args.len() != 6 || args[0] != "serve-create" || args[4] != "--native-text" || credibility.is_some() {
        return Err(USAGE.into());
    }
    let config = Config::read(Path::new(&args[1]))?;
    let actor = Profile::read(Path::new(&args[2]))?;
    let reviewer = PeerProfile::read(Path::new(&args[3]))?;
    check_profiles(&config, &actor, &reviewer)?;
    let inputs = recipe::load(Path::new(&args[5]), config.profile.delivery.scope.tenant,
        config.profile.delivery.limits.bytes)?;
    serve(config, &actor, &reviewer, inputs, super::super::clock, &mut std::io::stdout().lock())
}

fn check_profiles(config: &Config, actor: &Profile, reviewer: &PeerProfile) -> Result<(), String> {
    actor.check_host(config)?;
    reviewer.check_host(config)?;
    if !config.profile.delivery.initial_payload.is_empty() {
        return Err("a native text stream requires an explicitly empty initial payload".into());
    }
    if actor.socket == reviewer.socket(actor.request)
        || actor.socket == super::super::control::socket_path(reviewer, actor.request) {
        return Err("actor, reviewer and stop endpoints must be distinct".into());
    }
    Ok(())
}

fn serve<F>(mut config: Config, actor: &Profile, reviewer_profile: &PeerProfile,
    inputs: Inputs, mut time: F, output: &mut impl Write) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    check_profiles(&config, actor, reviewer_profile)?;
    if inputs.decoder.profile().identity().tenant != config.profile.delivery.scope.tenant {
        return Err("native model tenant mismatch".into());
    }
    let runtime = actor.runtime_ms.min(config.timing.runtime_ms);
    let start = time();
    let deadline = Deadline { logical: plus(start, runtime)?, started: Instant::now(), wall: Duration::from_millis(runtime) };
    // Exclusive listener ownership precedes the durable owner. No stale path is removed.
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let (mut host, reviewer) = FileOversight::create_generated_text_stream_with_reserve(
        &config.store, config.profile.clone(), inputs.stream, inputs.decoder.clone(), inputs.tokenizer.clone(),
        RecoveryReserve::terminal()).map_err(debug)?;
    host.enable_file_source(host.revision(), config.source_policy).map_err(debug)?;
    if !host.generated_text_stream_required().map_err(debug)? || !host.publication_guard_required() {
        return Err("native source and publication guards are mandatory".into());
    }
    let (port, supervisor) = host.into_generated_text_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let inbox = FileActorInbox::new(actor.actor, ActorWire::new(port), actor.channels(), actor.connections).map_err(debug)?;
    let pool = FileActorPool::for_requests(vec![(actor.request, inbox)]).map_err(debug)?;
    let mut ingress = Intake { socket, pool, profile: actor.clone(), attempted: 0 };
    let mut control = Some(Control::new(&config, actor.request, Some(reviewer_profile))?);
    let work = (|| {
        if !generate(&mut driver, &mut config, &inputs,
            (control.as_mut().expect("initial stop owner"), &reviewer, &deadline), &mut time)? {
            return Ok(());
        }
        // Only an acknowledged complete generation can acquire a source reference.
        let source = {
            let host = driver.supervisor().host().map_err(debug)?;
            let progress = host.decoder_generation_progress(inputs.generation).map_err(debug)?;
            let text = host.decoder_text_generation(inputs.generation).map_err(debug)?;
            let bytes = text.result().map_err(debug)?.bytes().map_err(debug)?;
            if bytes.is_empty() || std::str::from_utf8(bytes).is_err() {
                return Err("native result is not a nonempty complete UTF-8 message".into());
            }
            FileTextMessageRequest { request: actor.request, generation: inputs.generation,
                generation_revision: progress.generation_revision(), target: host.inspect().target,
                policy_epoch: host.inspect().control.ledger.epoch, deadline: deadline.logical }
        };
        let document = encode_command(&Command::Submit { request: actor.request,
            proposal: FileGeneratedTextActorPort::encode_message(&source).map_err(debug)? }).map_err(debug)?;
        output.write_all(&document).and_then(|()| output.write_all(b"\n"))
            .and_then(|()| output.flush()).map_err(|error| format!("source reference output failed: {error}; no actor request sent"))?;
        loop {
            deadline.check(time())?;
            if control.as_mut().expect("retained stop owner").checkpoint(&mut driver, &reviewer, &deadline, &mut time)? {
                return Ok(());
            }
            ingress.drive(&mut driver, &mut config, &mut time)?;
            match driver.supervisor().host().map_err(debug)?.request_status(actor.request) {
                Ok(_) => break,
                Err(JournalError::Contract(Error::Missing)) => {}
                Err(error) => return Err(debug(error)),
            }
            pause(actor.poll_ms);
        }
        let mut pump = |driver: &mut FileSupervisedDriver| {
            ingress.observe(driver)?;
            let status = driver.supervisor().host().map_err(debug)?.request_status(actor.request).map_err(debug)?;
            Ok(matches!(status.disposition, FileRequestDisposition::Admitted {
                stage: ActionState::Cancelled | ActionState::Denied | ActionState::Confirmed | ActionState::ConfirmedNotExecuted, .. }))
        };
        execute_serviced(&mut driver, &reviewer, &mut config, actor.request, &deadline,
            ExecuteServices { peers: Some(reviewer_profile), publication: None, ingress: &mut pump,
                stop: control.take() }, &mut time)
    })();
    let mut failure = work.err().map(|error| match stop(&mut driver, actor.request, &mut time) {
        Ok(()) => error, Err(stopping) => format!("{error}; stop/drain: {stopping}"),
    });
    let grace = Instant::now();
    while grace.elapsed() < Duration::from_millis(actor.reply_ms) {
        if let Err(error) = ingress.observe(&mut driver) {
            if failure.is_none() { failure = Some(error); }
            break;
        }
        pause(actor.poll_ms);
    }
    ingress.pool.revoke_all();
    let pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    if let Some(error) = failure { return Err(format!("{error}; helper cleanup pending={pending}")); }
    if pending != 0 { return Err(format!("{pending} direct helper children lack reaping confirmation")); }
    Ok(())
}

/// The original intent and one-token reducer own all cursor, budget and RNG state.
fn generate<F>(driver: &mut FileSupervisedDriver, config: &mut Config, inputs: &Inputs,
    controls: (&mut Control, &FileHumanReviewer, &Deadline), time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let (control, reviewer, deadline) = controls;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    refresh(driver, config, time)?;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        stamp(&mut host, deadline, time)?;
        let numerical = host.decoder_inspection().map_err(debug)?.numerical;
        let command = FileTextGenerationCommand::new(inputs.generation, numerical.actor_revision,
            numerical.position, inputs.request.clone()).map_err(debug)?;
        let revision = host.revision();
        host.begin_decoder_text(revision, command).map_err(debug)?;
    }
    loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
        let progress = driver.supervisor().host().map_err(debug)?
            .decoder_generation_progress(inputs.generation).map_err(debug)?;
        if progress.is_complete() {
            if progress.finish() != Some(Ok(GenerationFinish::StopToken)) {
                return Err(format!("native generation did not complete on a monitored control token: {:?}", progress.finish()));
            }
            return Ok(true);
        }
        refresh(driver, config, time)?;
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        stamp(&mut host, deadline, time)?;
        let revision = host.revision();
        host.advance_decoder_generation(revision, inputs.generation, progress.generation_revision()).map_err(debug)?;
        // No new-request token, source or actor I/O before the next stop checkpoint.
    }
}

// Re-sample after potentially slow source/control work. A source lease is checked
// by the ORIGINAL numerical admission against this exact just-observed tick.
fn stamp<F>(host: &mut FileOversight, deadline: &Deadline, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let now = time();
    deadline.check(now)?;
    if !host.clock_ready() || host.inspect().control.ledger.elapsed != Some(now) {
        let revision = host.revision();
        host.observe_time(revision, now).map_err(debug)?;
    }
    Ok(())
}

fn refresh<F>(driver: &mut FileSupervisedDriver, config: &mut Config, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    let revision = host.revision();
    host.refresh_file_source(revision, &mut config.source, time()).map_err(debug)?;
    let now = time();
    if !host.clock_ready() || host.inspect().control.ledger.elapsed != Some(now) {
        let revision = host.revision();
        host.observe_time(revision, now).map_err(debug)?;
    }
    Ok(())
}

struct Intake {
    socket: BoundSocket,
    pool: FileActorPool<FileGeneratedTextActorPort>,
    profile: Profile,
    attempted: u64,
}
impl Intake {
    fn accept(&mut self) -> Result<(), String> {
        let (_, status, _) = self.pool.statuses().next().ok_or("missing native actor session")?;
        if status.active.is_some() || status.revoked { return Ok(()); }
        let socket = match self.socket.listener.accept() {
            Ok((socket, _)) => socket,
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => return Ok(()),
            Err(error) => return Err(debug(error)),
        };
        if self.attempted == self.profile.candidates { return Err("native actor candidate limit reached".into()); }
        self.attempted += 1;
        match self.pool.attach(self.profile.request, socket) {
            Ok(_) => Ok(()),
            Err(PoolAttachError::Refused(PeerRefusal::CredentialsRejected)) if self.attempted < self.profile.candidates => Ok(()),
            Err(error) => Err(debug(error)),
        }
    }
    fn drive<F>(&mut self, driver: &mut FileSupervisedDriver, config: &mut Config, time: &mut F) -> Result<(), String>
    where F: FnMut() -> ElapsedTick {
        self.accept()?;
        check_report(self.pool.drive(driver, &mut config.source, time, pool_budget()).map_err(debug)?)
    }
    fn observe(&mut self, driver: &mut FileSupervisedDriver) -> Result<(), String> {
        self.accept()?;
        check_report(self.pool.observe(driver, pool_budget()).map_err(debug)?)
    }
}
fn pool_budget() -> PoolBudget {
    PoolBudget { total: DriveBudget { frames: 1, ..DriveBudget::default() }, ..PoolBudget::default() }
}
fn check_report(report: PoolDriveReport) -> Result<(), String> {
    for visit in report.visits { visit.result.map_err(debug)?; }
    Ok(())
}
