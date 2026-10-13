//! Runnable actor-only custody of exact requests through the original durable client.
//! No supervisor config, evidence reader, human role or effect owner is loaded.
mod settings;
mod report;
mod cancel;
#[cfg(test)]
mod tests;
use settings::Settings;
use super::{Profile, Duration, Instant, debug, pause};
use crate::config::read_regular;
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::peer::{
    FilePeerClient, FilePeerConnection,
};
use fa_reference::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{Command, MAX_FRAME_BYTES, WireResponse, decode_command};
use fa_reference::action::consequence::oversight::actor_wire::client::ClientProgress;
use std::{io::Write, os::unix::net::UnixStream, path::Path};

const USAGE: &str = "actor-submit ACTOR_PROFILE --durable CLIENT_PROFILE create SUBMIT_JSON | submit MIN_REV SUBMIT_JSON | resume MIN_REV | cancel MIN_REV | inspect MIN_REV";
const EXPIRED: &str = "durable actor invocation deadline reached; no automatic reconnect, replacement request or cancellation was sent";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode { Create, Submit, Resume, Cancel, Inspect }
struct Invocation<'a> { actor: &'a Path, settings: &'a Path, mode: Mode, minimum: u64, document: Option<&'a Path> }
impl<'a> Invocation<'a> {
    // The entire option set is checked before any profile or document I/O.
    fn parse(args: &'a [String], credibility: Option<&Path>) -> Result<Self, String> {
        if credibility.is_some() || args.len() < 6 || args[0] != "actor-submit" || args[2] != "--durable" {
            return Err(USAGE.into());
        }
        let (mode, minimum, document) = match args[4].as_str() {
            "create" if args.len() == 6 => (Mode::Create, 0, Some(Path::new(&args[5]))),
            "submit" if args.len() == 7 => (Mode::Submit, revision(&args[5])?, Some(Path::new(&args[6]))),
            "resume" if args.len() == 6 => (Mode::Resume, revision(&args[5])?, None),
            "inspect" if args.len() == 6 => (Mode::Inspect, revision(&args[5])?, None),
            "cancel" if args.len() == 6 => (Mode::Cancel, revision(&args[5])?, None),
            _ => return Err(USAGE.into()),
        };
        Ok(Self { actor: Path::new(&args[1]), settings: Path::new(&args[3]), mode, minimum, document })
    }
}
fn revision(value: &str) -> Result<u64, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0')) { return Err("MIN_REV must be canonical unsigned decimal".into()); }
    value.parse().map_err(debug)
}

pub(super) fn command(args: &[String], credibility: Option<&Path>, out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with(args, credibility, &mut || started.elapsed(), &mut pause, out)
}
fn command_with(args: &[String], credibility: Option<&Path>, elapsed: &mut impl FnMut() -> Duration,
    wait: &mut impl FnMut(u64), out: &mut impl Write) -> Result<(), String>
{
    let call = Invocation::parse(args, credibility)?;
    let actor = Profile::read(call.actor)?;
    let settings = Settings::read(call.settings, &actor)?;
    check_time(&actor, elapsed)?;
    let command = call.document.map(|path| {
        let bytes = read_regular(path, MAX_FRAME_BYTES)?;
        let command = decode_command(&bytes).map_err(debug)?;
        if !matches!(&command, Command::Submit { request, .. } if *request == actor.request) {
            return Err("durable intake requires the selected request's original Submit document".into());
        }
        Ok::<_, String>(command)
    }).transpose()?;
    check_time(&actor, elapsed)?;
    if call.mode == Mode::Inspect {
        // No listener directory, socket, owner lock, cleanup, clock or send here.
        let cut = FilePeerClient::read(&settings.directory, &settings.profile, call.minimum).map_err(debug)?;
        return report::write(&cut, actor.request, "inspection", out);
    }
    if call.mode == Mode::Cancel {
        // Refuse an unacknowledged request before socket checks, writer custody
        // or staging cleanup. Recheck the locked image below after opening.
        let cut = FilePeerClient::read(&settings.directory, &settings.profile, call.minimum).map_err(debug)?;
        cancel::check_record(cut.original_proposal(actor.request).is_some(),
            cut.last_response(actor.request), actor.request)?;
    }
    actor.check_directory()?;
    check_time(&actor, elapsed)?;
    let client = match call.mode {
        Mode::Create => FilePeerClient::create(&settings.directory, settings.profile.clone()),
        Mode::Submit | Mode::Resume | Mode::Cancel => FilePeerClient::open(&settings.directory, settings.profile.clone(), call.minimum),
        Mode::Inspect => unreachable!("inspection does not acquire a writer"),
    }.map_err(debug)?;
    // Resume never accepts replacement document bytes, creates a new key, or
    // treats absence as nonexecution. Missing keys refuse before connecting.
    if call.mode == Mode::Resume && client.state().original_proposal(actor.request).is_none() {
        return Err("selected request is not retained; resume does not create a replacement".into());
    }
    if call.mode == Mode::Cancel {
        cancel::check_record(client.state().original_proposal(actor.request).is_some(),
            client.state().last_response(actor.request), actor.request)?;
    }
    let floor = client.revision();
    let outcome = (|| {
        check_time(&actor, elapsed)?;
        let socket = UnixStream::connect(&actor.socket).map_err(debug)?;
        check_time(&actor, elapsed)?;
        let mut connected = client.connect(socket).map_err(|failure| format!("server peer refused: {:?}", failure.refusal))?;
        let result = if call.mode == Mode::Cancel {
            cancel::exchange(&mut connected, &actor, &settings, elapsed, wait)
        } else { exchange(&mut connected, &actor, &settings, command.as_ref(), elapsed, wait) };
        // Even on failure, this is only the last ACKNOWLEDGED client revision.
        // The selected canonical image may contain a newer lost acknowledgment.
        Ok::<_, String>((result, connected))
    })();
    let (result, floor, _custody) = match outcome {
        Ok((result, connected)) => (result, connected.revision(), Some(connected)),
        Err(error) => (Err(error), floor, None),
    };
    // Keep successful connection custody (including a faulted writer) through
    // inspection and reporting. A failed read returns no stale in-memory success.
    // This is history, not a new server observation or an automatic reconnect.
    let cut = FilePeerClient::read(&settings.directory, &settings.profile, floor).map_err(|error|
        format!("client result inspection failed: {error:?}; transport result={result:?}; do not replace the request"))?;
    let status = if result.is_ok() { "observed" } else { "interrupted" };
    report::write(&cut, actor.request, status, out)?;
    let response = result?;
    if call.mode == Mode::Cancel { return cancel::confirmed(&response); }
    if !matches!(response.result, Ok(Knowledge::Known { .. })) {
        return Err("original actor result is not known; inspect or resume only the retained request".into());
    }
    Ok(())
}
fn check_time(actor: &Profile, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= Duration::from_millis(actor.runtime_ms) { Err(EXPIRED.into()) } else { Ok(()) }
}

fn exchange(client: &mut FilePeerConnection, actor: &Profile, settings: &Settings,
    command: Option<&Command>, elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64))
    -> Result<WireResponse, String>
{
    check_time(actor, elapsed)?;
    match command {
        Some(Command::Submit { request, proposal }) if *request == actor.request =>
            client.submit(client.revision(), *request, proposal),
        None => client.retry_submission(client.revision(), actor.request),
        _ => return Err("expected exact selected Submit".into()),
    }.map_err(debug)?;
    loop {
        let response = receive(client, actor, settings, elapsed, wait)?;
        if !matches!(&response.result, Ok(Knowledge::Pending { .. }
            | Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })) {
            return Ok(response);
        }
        check_time(actor, elapsed)?;
        wait(actor.poll_ms);
        check_time(actor, elapsed)?;
        // Only explicit observation polling; never a replacement Submit.
        client.poll(client.revision(), actor.request).map_err(debug)?;
    }
}

// Receive one already-prepared original command, shared by submit, resume and
// cancellation. Only native drive touches the socket or records work/results.
fn receive(client: &mut FilePeerConnection, actor: &Profile, settings: &Settings,
    elapsed: &mut impl FnMut() -> Duration, wait: &mut impl FnMut(u64)) -> Result<WireResponse, String>
{
    loop {
        check_time(actor, elapsed)?;
        let report = client.drive(client.revision(), settings.drive).map_err(debug)?;
        match report.progress {
            ClientProgress::Response(response) => return Ok(response),
            ClientProgress::Blocked => { check_time(actor, elapsed)?; wait(actor.poll_ms); }
            ClientProgress::Progress => {}
            ClientProgress::Complete => return Err("native exchange ended without a response; retained request stays unresolved".into()),
        }
    }
}
