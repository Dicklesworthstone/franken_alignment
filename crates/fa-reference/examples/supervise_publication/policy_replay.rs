//! Read-only comparison of a candidate against the complete retained policy corpus.
//! Native replay owns observations and dispositions. No helper, source, writer,
//! campaign approval or effect capability is constructed by this command.
#[path = "policy_replay/input.rs"]
mod input;
#[path = "policy_replay/render.rs"]
mod render;
pub(crate) use input::Request;
pub(crate) use render::encode;
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

const USAGE: &str = "usage: supervise_publication policy-replay CONFIG CANDIDATE_JSON [--native-text RECIPE --history-anchor INPUT [--publication-profile PROFILE]]";
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
    let native = match args {
        [command, _, _] if command == "policy-replay" => false,
        [command, _, _, recipe, _, anchor, _, rest @ ..]
            if cfg!(target_os = "linux")
                && command == "policy-replay"
                && recipe == "--native-text"
                && anchor == "--history-anchor"
                && (rest.is_empty() || (rest.len() == 2 && rest[0] == "--publication-profile")) =>
        {
            if args.iter().enumerate().any(|(index, value)| {
                matches!(index, 1 | 2 | 4 | 6 | 8)
                    && (value.is_empty() || value.starts_with("--"))
            }) {
                return Err(USAGE.into());
            }
            true
        }
        _ => return Err(USAGE.into()),
    };
    let config = Config::read(Path::new(&args[1]))?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    #[cfg(target_os = "linux")]
    let native_selection = if native {
        Some(crate::workflow::actor_service::NativePolicyReplay::admit(args, &config)?)
    } else {
        None
    };
    let request = input::Request::read(Path::new(&args[2]))?;
    checkpoint(wall, elapsed)?;
    let (cut, report) = if native {
        #[cfg(target_os = "linux")]
        {
            native_selection.ok_or(USAGE)?.read(
                &config, request.candidate, request.limits, || checkpoint(wall, elapsed),
            )?
        }
        #[cfg(not(target_os = "linux"))]
        return Err(USAGE.into());
    } else {
        FileOversight::read_policy_replay(
            &config.store,
            &config.profile,
            request.candidate,
            request.limits,
        )
        .map_err(|error| format!("historical policy replay unavailable: {error:?}"))?
    };
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
