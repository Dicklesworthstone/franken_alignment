//! Actor-only endpoint discovery: public profiles, one original native exchange.
//! This observes a disclosed contract, never an exact pass or effect permission.
use super::{Profile, Settings, Work, check_time, debug, drive_client_shared};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    client::PrecheckCapabilitiesClient,
    discovery::{MAX_CAPABILITIES_REQUEST_BYTES, PrecheckCapabilitiesRequest, PrecheckCapabilitiesResponse},
};
use fa_reference::action::consequence::oversight::actor::Knowledge;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

const USAGE: &str = "actor-precheck --capabilities ACTOR_PROFILE PRECHECK_PROFILE QUERY";

pub(super) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let query_id = arguments(args)?;
    let started = Instant::now();
    let actor = Profile::read(Path::new(&args[2]))?;
    check_time(started, Duration::from_millis(actor.runtime_ms))?;
    let settings = Settings::read(Path::new(&args[3]))?;
    query(&actor, &settings, query_id, started, out)
}

fn arguments(args: &[String]) -> Result<u64, String> {
    if args.len() != 5 || args[0] != "actor-precheck" || args[1] != "--capabilities"
        || args[2..].iter().any(|arg| arg.is_empty() || arg.starts_with("--"))
    { return Err(USAGE.into()); }
    let query = &args[4];
    if query.len() > 20 || query.starts_with('0')
        || !query.bytes().all(|byte| byte.is_ascii_digit())
    { return Err(USAGE.into()); }
    query.parse::<u64>().map_err(|_| USAGE.to_owned())
}

fn work() -> Work {
    Work { write_bytes: MAX_CAPABILITIES_REQUEST_BYTES + 1, ..Work::new() }
}

fn query(actor: &Profile, settings: &Settings, query: u64, started: Instant,
    out: &mut impl Write,
) -> Result<(), String> {
    let wall = Duration::from_millis(actor.runtime_ms.min(settings.runtime_ms));
    check_time(started, wall)?;
    if query == 0 { return Err(USAGE.into()); }
    settings.check_actor(actor)?;
    check_time(started, wall)?;
    // No proposal file, Config, source, journal, ordinary socket or retry path.
    let request = PrecheckCapabilitiesRequest { query, expected_policy_epoch: settings.expected_policy_epoch };
    let socket = UnixStream::connect(&settings.socket).map_err(debug)?;
    check_time(started, wall)?;
    let client = PrecheckCapabilitiesClient::connect_capabilities(socket, actor.supervisor,
        request, settings.expected_policy_epoch).map_err(debug)?;
    let mut work = work();
    let response = drive_client_shared(client, actor.poll_ms, wall, &mut work, || started.elapsed())?;
    check_time(started, wall)?;
    let result = write_response(&response, settings, out);
    check_time(started, wall)?;
    result
}

fn write_response(response: &PrecheckCapabilitiesResponse, settings: &Settings,
    out: &mut impl Write,
) -> Result<(), String> {
    if let Ok(Knowledge::Known { basis, .. }) = &response.result {
        if basis.disclosure != settings.disclosure_id {
            return Err("capabilities response belongs to another disclosure; no retry was sent".into());
        }
    }
    let bytes = response.encode().map_err(debug)?;
    out.write_all(&bytes).and_then(|()| out.write_all(b"\n")).and_then(|()| out.flush())
        .map_err(|error| format!("native capabilities response received, output failed: {error}; no retry was sent"))?;
    if !matches!(&response.result, Ok(Knowledge::Known { .. })) {
        return Err("capabilities unavailable; see original response; no retry was sent".into());
    }
    // Known with zero remaining work is still useful information, not a pass.
    Ok(())
}

#[cfg(test)]
mod tests;
