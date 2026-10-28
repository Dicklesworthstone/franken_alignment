//! Read-only comparison of a candidate against the complete retained policy corpus.
//! Native replay owns observations and dispositions. No helper, source, writer,
//! campaign approval or effect capability is constructed by this command.
#[path = "policy_replay/input.rs"]
mod input;
#[path = "policy_replay/render.rs"]
mod render;
#[cfg(test)]
#[path = "policy_replay/tests.rs"]
mod tests;

use crate::config::Config;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};

const USAGE: &str = "usage: supervise_publication policy-replay CONFIG CANDIDATE_JSON";
const DEADLINE: &str = "policy replay deadline reached; no report was emitted";

pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with_elapsed(args, &mut || started.elapsed(), out)
}

fn command_with_elapsed(
    args: &[String],
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    // Closed before shared flag stripping, including options for live authority.
    if args.len() != 3 || args[0] != "policy-replay" {
        return Err(USAGE.into());
    }
    let config = Config::read(Path::new(&args[1]))?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let request = input::Request::read(Path::new(&args[2]))?;
    checkpoint(wall, elapsed)?;
    // This API reads one canonical image and replays its complete original
    // history. It does not open the writer, recover, fence or clean up pending
    // files. Caller-selected archives or a new evidence read cannot substitute.
    let (cut, report) = FileOversight::read_policy_replay(
        &config.store,
        &config.profile,
        request.candidate,
        request.limits,
    )
    .map_err(|error| format!("historical policy replay unavailable: {error:?}"))?;
    checkpoint(wall, elapsed)?;
    if report.previous_policy().generation() != request.expected_policy_generation {
        return Err("retained effective policy differs from expected_policy_generation; no report was emitted".into());
    }
    let bytes = render::encode(&cut, &report, request.limits, request.output_bytes)?;
    checkpoint(wall, elapsed)?;
    // Parsing, complete native replay and bounded encoding finish before the
    // first output byte. A failed sink can still receive a partial transport.
    out.write_all(&bytes).map_err(|error| {
        format!("policy replay output failed: {error}; no authority or effect was changed")
    })?;
    out.flush().map_err(|error| {
        format!("policy replay output flush failed: {error}; no authority or effect was changed")
    })
}

// A bounded file read/native replay/output write is synchronous. These checks
// bound admitted elapsed work, not the latency of an individual operation.
fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall {
        Err(DEADLINE.into())
    } else {
        Ok(())
    }
}
