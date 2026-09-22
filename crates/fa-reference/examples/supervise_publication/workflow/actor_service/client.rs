//! Actor-only executable consumer of the original bounded exchange protocol.
//! Never reads the supervisor profile, journal, evidence, or reviewer identity.
mod sequence;
use super::{Profile, Duration, Instant, debug, pause};
use crate::config::read_regular;
use fa_reference::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{Command, MAX_FRAME_BYTES, WireResponse, decode_command};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorExchange, ClientIoBudget, ClientIoLimits, ClientProgress};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;

pub(super) const MAX_SEQUENCE_DOCUMENTS: usize = super::FileSupervisedDriver::MAX_REQUEST_SEQUENCE;

/// All documents are independently frozen actor proposals. The operator's
/// selected service schedule, not this client, determines which keys may run.
pub(super) fn submit_sequence(profile: &Profile, documents: &[&Path], out: &mut impl Write)
    -> Result<(), String>
{
    sequence::submit(profile, documents, out)
}

pub(super) fn submit(profile: &Profile, document: &Path, out: &mut impl Write) -> Result<(), String> {
    let command = decode_command(&read_regular(document, MAX_FRAME_BYTES)?).map_err(debug)?;
    if !matches!(&command, Command::Submit { request, .. } if *request == profile.request) {
        return Err("expected the selected request's original Submit document".into());
    }
    let socket = connect(profile)?;
    let response = exchange_until_terminal(socket, profile, command)?;
    write_response(&response, out)?;
    if !matches!(&response.result, Ok(Knowledge::Known { .. })) {
        return Err("original actor result is not known; see response and retain original request bytes".into());
    }
    Ok(())
}
fn connect(profile: &Profile) -> Result<UnixStream, String> {
    profile.check_directory()?;
    let socket = UnixStream::connect(&profile.socket).map_err(debug)?;
    // Authenticate THIS connected server before sending or reading any bytes.
    profile.supervisor.verify(&socket).map_err(debug)?;
    socket.set_nonblocking(true).map_err(debug)?;
    Ok(socket)
}

fn write_response(response: &WireResponse, out: &mut impl Write) -> Result<(), String> {
    out.write_all(&response.encode()).and_then(|()| out.write_all(b"\n")).and_then(|()| out.flush())
        .map_err(|error| format!("native actor response received, output failed: {error}; no retry was sent"))
}

fn exchange_until_terminal(socket: UnixStream, profile: &Profile, command: Command) -> Result<WireResponse, String> {
    let started = Instant::now();
    let mut budget = ClientIoBudget::new(ClientIoLimits { exchanges: profile.exchanges, ..ClientIoLimits::default() }).map_err(debug)?;
    exchange_on_connection(socket, profile, command, &mut budget, started).map(|(_, response)| response)
}

// One ORIGINAL exchange engine for both one-shot and sequential clients. The
// caller retains the budget and deadline; neither is renewed for a later key.
fn exchange_on_connection(mut socket: UnixStream, profile: &Profile, mut command: Command,
    budget: &mut ClientIoBudget, started: Instant) -> Result<(Option<UnixStream>, WireResponse), String>
{
    let request = command.request();
    loop {
        let mut exchange = ActorExchange::new(socket, command, budget).map_err(|error| {
            format!("actor exchange could not start: {error:?}; earlier request may have reached peer={}; no resend",
                budget.work().calls != 0)
        })?;
        let response = loop {
            if started.elapsed() >= Duration::from_millis(profile.runtime_ms) {
                return Err(format!("actor timeout; request bytes may have reached peer={}; retry only original request bytes",
                    budget.work().calls != 0));
            }
            match exchange.step(budget) {
                Ok(ClientProgress::Response(response)) => break response,
                Ok(ClientProgress::Complete) => return Err("actor exchange completed without a response".into()),
                Ok(ClientProgress::Progress | ClientProgress::Blocked) => pause(profile.poll_ms),
                Err(error) => return Err(format!("actor exchange failed: {error:?}; request bytes may have reached peer={}; no reconnect/resend",
                    budget.work().calls != 0)),
            }
        };
        // Dispatch temporarily projects OutcomeUnknown before publication settles.
        // Poll that state under the SAME finite deadline/budget, never resubmit.
        if !matches!(&response.result, Ok(Knowledge::Pending { .. }
            | Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })) { return Ok((exchange.into_stream(), response)); }
        socket = exchange.into_stream().ok_or("completed actor exchange lost its socket")?;
        command = Command::Poll { request };
        pause(profile.poll_ms);
    }
}
