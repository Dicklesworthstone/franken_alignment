//! Actor-side exact precheck, separate from submission and private host config.
//! Explicit single/batch exact queries and discovery; no submission or retry.
mod batch;
mod discovery;
use super::Settings;
use super::super::{Profile, debug, pause};
use crate::config::read_regular;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FilePrecheckOutcome;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    MAX_PRECHECK_FRAME_BYTES, MAX_PRECHECK_RESPONSE_BYTES, PrecheckRequest,
    PrecheckWireResponse, decode_precheck_request,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::client::{
    PrecheckClientPhase, PrecheckPeerClient,
};
use fa_reference::action::consequence::oversight::actor::Knowledge;
use fa_reference::action::consequence::oversight::actor_transport::{DriveBudget, DriveProgress};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

const USAGE: &str = "actor-precheck ACTOR_PROFILE PRECHECK_PROFILE PRECHECK_JSON; alternatively actor-precheck --batch|--first-pass ACTOR_PROFILE PRECHECK_PROFILE PRECHECK_JSON [PRECHECK_JSON ...]; or actor-precheck --capabilities ACTOR_PROFILE PRECHECK_PROFILE QUERY";
const MAX_IO_CALLS: usize = 4_096;

/// Closed grammar, before opening any input. Neither public profile is a
/// supervisor Config; the actor never opens the journal or evidence source.
pub(in crate::workflow::actor_service) fn command(args: &[String], out: &mut impl Write)
    -> Result<(), String>
{
    if args.first().is_some_and(|arg| arg == "actor-precheck")
        && args.get(1).is_some_and(|arg| arg == "--capabilities")
    { return discovery::command(args, out); }
    if args.first().is_some_and(|arg| arg == "actor-precheck")
        && args.get(1).is_some_and(|arg| matches!(arg.as_str(), "--batch" | "--first-pass"))
    { return batch::command(args, out); }
    if args.len() != 4 || args[0] != "actor-precheck"
        || args[1..].iter().any(|arg| arg.starts_with("--"))
    { return Err(USAGE.into()); }
    let started = Instant::now();
    let actor = Profile::read(Path::new(&args[1]))?;
    check_time(started, Duration::from_millis(actor.runtime_ms))?;
    let settings = Settings::read(Path::new(&args[2]))?;
    query(&actor, &settings, Path::new(&args[3]), started, out)
}

/// Synchronous file, connect and output operations follow this example's
/// cooperative contract: check elapsed time around each operation once the
/// selected bounds are known. They are not preempted; this is not a hard startup
/// deadline. A write failure or late-returning stdout operation can leave partial
/// or complete response bytes visible even though the command returns an error.
pub(in crate::workflow::actor_service) fn query(
    actor: &Profile, settings: &Settings, document: &Path, started: Instant,
    out: &mut impl Write,
) -> Result<(), String> {
    let wall = Duration::from_millis(actor.runtime_ms.min(settings.runtime_ms));
    check_time(started, wall)?;
    settings.check_actor(actor)?;
    check_time(started, wall)?;
    let request = decode_precheck_request(&read_regular(document, MAX_PRECHECK_FRAME_BYTES)?)
        .map_err(debug)?;
    check_time(started, wall)?;
    if request.proposal.expected_policy_epoch != settings.expected_policy_epoch {
        return Err("precheck request differs from independently configured policy epoch; no query sent".into());
    }
    // There is deliberately no fallback to the ordinary submission socket.
    let socket = UnixStream::connect(&settings.socket).map_err(debug)?;
    check_time(started, wall)?;
    let response = exchange(socket, actor, settings, request, started, Work::new())?;
    check_time(started, wall)?;
    let result = write_response(&response, settings, out);
    check_time(started, wall)?;
    result
}

// Finite lifetime work, not a fresh per-drive allowance. The native client is
// already one-shot; the extra read byte permits its trailing-data boundary probe.
struct Work { read_bytes: usize, write_bytes: usize, io_calls: usize }
impl Work {
    fn new() -> Self {
        Self { read_bytes: MAX_PRECHECK_RESPONSE_BYTES + 2,
            write_bytes: MAX_PRECHECK_FRAME_BYTES + 1, io_calls: MAX_IO_CALLS }
    }
    fn budget(&self, phase: PrecheckClientPhase) -> Result<DriveBudget, String> {
        if self.io_calls == 0
            || (phase == PrecheckClientPhase::Writing && self.write_bytes == 0)
            || (matches!(phase, PrecheckClientPhase::Reading | PrecheckClientPhase::Confirming)
                && self.read_bytes == 0)
        { return Err("precheck lifetime I/O allowance exhausted; server work may be spent; no retry was sent".into()); }
        Ok(DriveBudget { read_bytes: self.read_bytes.min(16_384),
            write_bytes: self.write_bytes.min(16_384), frames: 1,
            io_calls: self.io_calls.min(16) })
    }
    fn debit(&mut self, progress: DriveProgress) -> Result<(), String> {
        let read_bytes = self.read_bytes.checked_sub(progress.read_bytes);
        let write_bytes = self.write_bytes.checked_sub(progress.written_bytes);
        let io_calls = self.io_calls.checked_sub(progress.io_calls);
        match (read_bytes, write_bytes, io_calls) {
            (Some(read_bytes), Some(write_bytes), Some(io_calls)) => {
                *self = Self { read_bytes, write_bytes, io_calls }; Ok(())
            }
            _ => Err("precheck transport exceeded its lifetime allowance; no retry was sent".into()),
        }
    }
}

fn check_time(started: Instant, wall: Duration) -> Result<(), String> {
    check_elapsed(started.elapsed(), wall)
}

fn check_elapsed(elapsed: Duration, wall: Duration) -> Result<(), String> {
    if elapsed >= wall {
        Err("precheck elapsed allowance reached; server work may be spent; no retry was sent".into())
    } else { Ok(()) }
}

fn exchange(socket: UnixStream, actor: &Profile, settings: &Settings,
    request: PrecheckRequest, started: Instant, work: Work,
) -> Result<PrecheckWireResponse, String> {
    let wall = Duration::from_millis(actor.runtime_ms.min(settings.runtime_ms));
    check_time(started, wall)?;
    // The native constructor verifies THIS peer before any proposal bytes.
    let client = PrecheckPeerClient::connect(socket, actor.supervisor, request,
        settings.expected_policy_epoch).map_err(debug)?;
    drive_client(client, actor.poll_ms, wall, work, || started.elapsed())
}

// The private clock seam permits deadline-boundary regression tests without
// timing races. Production always uses the original command's elapsed Instant.
fn drive_client(client: PrecheckPeerClient, poll_ms: u64, wall: Duration,
    mut work: Work, elapsed: impl FnMut() -> Duration,
) -> Result<PrecheckWireResponse, String> {
    drive_client_shared(client, poll_ms, wall, &mut work, elapsed)
}

// Batch members borrow the SAME remaining work; neither a new connection nor a
// completed response replenishes it. Discovery uses this loop with its pinned
// native response type, not a second transport or another deadline mechanism.
fn drive_client_shared<R>(mut client: PrecheckPeerClient<R>, poll_ms: u64, wall: Duration,
    work: &mut Work, mut elapsed: impl FnMut() -> Duration,
) -> Result<R, String> {
    loop {
        check_elapsed(elapsed(), wall)?;
        let budget = work.budget(client.status().phase)?;
        let report = client.drive(budget).map_err(|error|
            format!("precheck exchange failed: {error:?}; server work may be spent; no retry was sent"))?;
        work.debit(report.progress)?;
        // A response arriving after the selected allowance is not success.
        check_elapsed(elapsed(), wall)?;
        if let Some(error) = report.status.failure {
            return Err(format!("precheck exchange failed: {error:?}; server work may be spent; no retry was sent"));
        }
        if let Some(response) = client.take_response() { return Ok(response); }
        if report.status.closed() {
            return Err("precheck exchange closed without a response; no retry was sent".into());
        }
        pause(poll_ms);
    }
}

fn write_response(response: &PrecheckWireResponse, settings: &Settings,
    out: &mut impl Write,
) -> Result<(), String> {
    if !emit_response(response, settings, out)? {
        return Err("precheck did not pass; see original response; no retry was sent".into());
    }
    // A pass remains review_required; no ordinary submission is performed here.
    Ok(())
}

// Emit the original native response without converting a denial into a transport
// error. The single-query command and bounded candidate batch choose exit status.
fn emit_response(response: &PrecheckWireResponse, settings: &Settings,
    out: &mut impl Write,
) -> Result<bool, String> {
    // The generic native codec binds query/epoch. This selected CLI profile also
    // names a disclosure, so a different grant at the same epoch is not accepted.
    if let Ok(Knowledge::Known { basis, .. }) = &response.result {
        if basis.disclosure != settings.disclosure_id {
            return Err("precheck response belongs to another disclosure; no retry was sent".into());
        }
    }
    let bytes = response.encode().map_err(debug)?;
    out.write_all(&bytes).and_then(|()| out.write_all(b"\n")).and_then(|()| out.flush())
        .map_err(|error| format!("native precheck response received, output failed: {error}; no retry was sent"))?;
    Ok(matches!(&response.result,
        Ok(Knowledge::Known { value: FilePrecheckOutcome::PrecheckPassed, .. })))
}

#[cfg(test)]
mod tests;
