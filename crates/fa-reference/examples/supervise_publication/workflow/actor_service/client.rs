//! Actor-only executable consumer of the original bounded exchange protocol.
//! Never reads the supervisor profile, journal, evidence, or reviewer identity.
use super::{Profile, Duration, Instant, debug, pause};
use crate::config::read_regular;
use fa_reference::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{Command, MAX_FRAME_BYTES, WireResponse, decode_command};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorExchange, ClientIoBudget, ClientIoLimits, ClientProgress};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;

pub(super) fn submit(profile: &Profile, document: &Path, out: &mut impl Write) -> Result<(), String> {
    let command = decode_command(&read_regular(document, MAX_FRAME_BYTES)?).map_err(debug)?;
    if !matches!(&command, Command::Submit { request, .. } if *request == profile.request) {
        return Err("expected the selected request's original Submit document".into());
    }
    profile.check_directory()?;
    let socket = UnixStream::connect(&profile.socket).map_err(debug)?;
    // Authenticate THIS connected server before sending or reading any bytes.
    profile.supervisor.verify(&socket).map_err(debug)?;
    socket.set_nonblocking(true).map_err(debug)?;
    let response = exchange_until_terminal(socket, profile, command)?;
    out.write_all(&response.encode()).and_then(|()| out.write_all(b"\n")).and_then(|()| out.flush())
        .map_err(|error| format!("native actor response received, output failed: {error}; no retry was sent"))?;
    if !matches!(&response.result, Ok(Knowledge::Known { .. })) {
        return Err("original actor result is not known; see response and retain original request bytes".into());
    }
    Ok(())
}
fn exchange_until_terminal(mut socket: UnixStream, profile: &Profile, mut command: Command) -> Result<WireResponse, String> {
    let started = Instant::now();
    let mut budget = ClientIoBudget::new(ClientIoLimits { exchanges: profile.exchanges, ..ClientIoLimits::default() }).map_err(debug)?;
    loop {
        let mut exchange = ActorExchange::new(socket, command, &mut budget).map_err(|error| {
            format!("actor exchange could not start: {error:?}; earlier request may have reached peer={}; no resend",
                budget.work().calls != 0)
        })?;
        let response = loop {
            if started.elapsed() >= Duration::from_millis(profile.runtime_ms) {
                return Err(format!("actor timeout; request bytes may have reached peer={}; retry only original request bytes",
                    budget.work().calls != 0));
            }
            match exchange.step(&mut budget) {
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
            | Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown })) { return Ok(response); }
        socket = exchange.into_stream().ok_or("completed actor exchange lost its socket")?;
        command = Command::Poll { request: profile.request };
        pause(profile.poll_ms);
    }
}
