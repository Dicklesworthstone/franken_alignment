//! Actor-only definition read. No private Config, evidence, proposal or Submit.
use super::{Profile, Settings, Work, MAX_IO_CALLS, check_time, debug, drive_client_shared};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    client::PrecheckPolicyClient,
    policy::{PolicyRequest, PolicyResponse, MAX_POLICY_REQUEST_BYTES, MAX_POLICY_RESPONSE_BYTES},
};
use fa_reference::action::consequence::oversight::actor::Knowledge;
use std::{io::Write, os::unix::net::UnixStream, path::Path, time::{Duration, Instant}};

const USAGE: &str = "actor-precheck --policy ACTOR_PROFILE PRECHECK_PROFILE QUERY";
pub(super) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let query = arguments(args)?;
    let started = Instant::now();
    let actor = Profile::read(Path::new(&args[2]))?;
    check_time(started, Duration::from_millis(actor.runtime_ms))?;
    let settings = Settings::read(Path::new(&args[3]))?;
    query_policy(&actor, &settings, query, started, out)
}
fn arguments(args: &[String]) -> Result<u64, String> {
    if args.len() != 5 || args[0] != "actor-precheck" || args[1] != "--policy"
        || args[2..].iter().any(|arg| arg.is_empty() || arg.starts_with("--")) {
        return Err(USAGE.into());
    }
    let value = &args[4];
    if value.len() > 20 || value.starts_with('0') || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(USAGE.into());
    }
    value.parse::<u64>().map_err(|_| USAGE.into())
}
fn query_policy(actor: &Profile, settings: &Settings, query: u64, started: Instant,
    out: &mut impl Write,
) -> Result<(), String> {
    let wall = Duration::from_millis(actor.runtime_ms.min(settings.runtime_ms));
    check_time(started, wall)?;
    if query == 0 || !settings.full_policy {
        return Err("policy read requires a nonzero query and an explicit full-disclosure profile; no query sent".into());
    }
    settings.check_actor(actor)?;
    check_time(started, wall)?;
    let socket = UnixStream::connect(&settings.socket).map_err(debug)?;
    check_time(started, wall)?;
    let client = PrecheckPolicyClient::connect_policy(socket, actor.supervisor,
        PolicyRequest { query, expected_policy_epoch: settings.expected_policy_epoch },
        settings.expected_policy_epoch).map_err(debug)?;
    let mut work = work();
    let response = drive_client_shared(client, actor.poll_ms, wall, &mut work, || started.elapsed())?;
    check_time(started, wall)?;
    let result = emit(&response, settings.disclosure_id, out);
    check_time(started, wall)?;
    result
}
fn work() -> Work {
    Work { read_bytes: MAX_POLICY_RESPONSE_BYTES + 2,
        write_bytes: MAX_POLICY_REQUEST_BYTES + 1, io_calls: MAX_IO_CALLS }
}
fn emit(response: &PolicyResponse, disclosure: u64, out: &mut impl Write) -> Result<(), String> {
    if let Ok(Knowledge::Known { basis, .. }) = &response.result {
        if basis.disclosure != disclosure {
            return Err("policy response belongs to another disclosure; no retry was sent".into());
        }
    }
    let bytes = response.encode().map_err(debug)?;
    out.write_all(&bytes).and_then(|()| out.write_all(b"\n")).and_then(|()| out.flush())
        .map_err(|error| format!("policy response output failed: {error}; no retry was sent"))?;
    if !matches!(&response.result, Ok(Knowledge::Known { .. })) {
        return Err("policy definition unavailable; see original response; no retry was sent".into());
    }
    // Known definition is information, never a policy pass or effect authority.
    Ok(())
}

#[cfg(test)]
mod tests;
