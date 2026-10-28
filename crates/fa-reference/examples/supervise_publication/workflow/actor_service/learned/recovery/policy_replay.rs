//! Complete retained current-policy corpus on the original guarded learned replay.
//! No live owner, recovery Fence, observer, helper or effect is finalized.
use super::super::{Config, Duration, Instant, Loaded, anchors, fields, recipe, topology};
use super::{cooperative::finish_replay, selected::Cursor};
use crate::policy_replay::{Request as Candidate, encode};
use fa_reference::action::consequence::{
    delivery::persistent::FileDeliverySnapshot,
    gate::containment::session::policy::Policy,
    policy_campaign::{PolicyReplayReport, ReplayLimits},
};
use std::{io::Write, path::Path};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --resume --policy-replay CANDIDATE_JSON --history-anchor INPUT [--topology-profile PATH]";
const DEADLINE: &str = "learned policy replay deadline reached; no report was emitted";

struct Request<'a> {
    config: &'a Path,
    recipe: &'a Path,
    candidate: &'a Path,
    topology: Option<&'a Path>,
}
impl<'a> Request<'a> {
    fn parse(args: &'a [String]) -> Result<Self, String> {
        if args.len() < 8
            || args[0] != "create-learned-generated"
            || args[3] != "--resume"
            || args[1..3]
                .iter()
                .any(|value| value.is_empty() || value.starts_with("--"))
        {
            return Err(USAGE.into());
        }
        let mut candidate = None;
        let mut anchor = None;
        let mut topology = None;
        let mut at = 4;
        // Own the COMPLETE argv before any live/receipt option can strip it.
        // Adjacent flags cannot repair missing values or introduce authority.
        while at < args.len() {
            let slot = match args[at].as_str() {
                "--policy-replay" => &mut candidate,
                "--history-anchor" => &mut anchor,
                "--topology-profile" => &mut topology,
                _ => return Err(USAGE.into()),
            };
            if slot.is_some() {
                return Err(USAGE.into());
            }
            let value = args.get(at + 1).ok_or(USAGE)?;
            fields::path(value).map_err(|_| USAGE.to_owned())?;
            *slot = Some(Path::new(value));
            at += 2;
        }
        if anchor.is_none() {
            return Err(USAGE.into());
        }
        Ok(Self {
            config: Path::new(&args[1]),
            recipe: Path::new(&args[2]),
            candidate: candidate.ok_or(USAGE)?,
            topology,
        })
    }
}

pub(in super::super) fn command(
    args: &[String],
    credibility: Option<&Path>,
    output: &mut impl Write,
) -> Result<(), String> {
    let started = Instant::now();
    command_with_elapsed(args, credibility, output, &mut || started.elapsed())
}

/// One finite allowance starts before admission and covers the original replay,
/// candidate evaluation, canonical confirmation and bounded pre-output encoding.
pub(in super::super) fn command_with_elapsed(
    args: &[String],
    credibility: Option<&Path>,
    output: &mut impl Write,
    elapsed: &mut impl FnMut() -> Duration,
) -> Result<(), String> {
    if credibility.is_some() {
        return Err(USAGE.into());
    }
    let request = Request::parse(args)?;
    let config = Config::read(request.config)?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let candidate = Candidate::read(request.candidate)?;
    checkpoint(wall, elapsed)?;
    let mut loaded = recipe::load(request.recipe, &config, false)?;
    checkpoint(wall, elapsed)?;
    // Reuse the original regular-file/outside-authority custody checks. The
    // grammar admits exactly one input anchor and no retained output.
    let (_, custody) = anchors::take_options(args)?;
    custody.admit(&config, &mut loaded, true)?;
    checkpoint(wall, elapsed)?;
    topology::admit(
        &config,
        &mut loaded,
        request.topology,
        topology::Mode::Historical,
    )?;
    checkpoint(wall, elapsed)?;
    let (cut, report) = read_with_elapsed(
        &config,
        &loaded,
        candidate.candidate,
        candidate.limits,
        elapsed,
    )?;
    if report.previous_policy().generation() != candidate.expected_policy_generation {
        return Err("retained effective policy differs from expected_policy_generation; no report was emitted".into());
    }
    let bytes = encode(&cut, &report, candidate.limits, candidate.output_bytes)?;
    checkpoint(wall, elapsed)?;
    output.write_all(&bytes).map_err(|error| {
        format!("learned policy replay output failed: {error}; no authority or effect was changed")
    })?;
    output.flush().map_err(|error| {
        format!("learned policy replay output flush failed: {error}; no authority or effect was changed")
    })
}

// Framing, one numerical replay event and final encoding are synchronous. These
// checkpoints bound admitted elapsed work, not individual operation latency.
fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall {
        Err(DEADLINE.into())
    } else {
        Ok(())
    }
}

/// Replay the complete retained current-policy proposal and review corpus,
/// not just the recipe's request. The same cursor validates recipe, reserve,
/// floors, anchor and topology before producing immutable values; no receipt
/// selection, live recovery or cleanup.
pub(in super::super) fn read_with_elapsed(
    config: &Config,
    loaded: &Loaded,
    candidate: Policy,
    limits: ReplayLimits,
    elapsed: &mut impl FnMut() -> Duration,
) -> Result<(FileDeliverySnapshot, PolicyReplayReport), String> {
    if loaded.recovery_anchor.is_none() || loaded.anchor_output.is_some() {
        return Err("learned policy replay requires an independent input history anchor without retention".into());
    }
    if topology::history(loaded)?.is_some_and(|history| history.checkpoint().is_none()) {
        return Err("learned policy replay requires an exact v2 topology-source checkpoint".into());
    }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let run = Cursor::begin(config, loaded)?;
    let result = finish_replay(
        run,
        &mut |_| checkpoint(wall, elapsed),
        |run| run.progress(),
        |run, done| run.advance(done),
        |run| run.finish_policy_replay(candidate, limits),
    )?;
    // Ready has its own checkpoint in finish_replay. A slow canonical reread
    // still cannot publish after the original allowance has elapsed.
    checkpoint(wall, elapsed)?;
    Ok(result)
}
