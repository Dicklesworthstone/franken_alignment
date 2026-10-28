//! Explicit learned policy-admission diagnosis on the original guarded replay.
//! The result is an immutable experiment; no live owner or helper is finalized.
use super::super::{
    Config, Duration, Instant, Loaded, anchors, debug, fields, finish, recipe, topology,
};
use super::{cooperative::finish_replay, selected::Cursor};
use crate::config::read_regular;
use crate::diagnosis::{render, search};
use fa_reference::action::MAX_PAYLOAD_BYTES;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalInvestigation;
use fa_reference::action::consequence::experiment::{
    Intervention, InterventionScope, MAX_INTERVENTION_BYTES,
};
use std::{io::Write, path::Path};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --resume --explain-proposal --history-anchor INPUT [--topology-profile PATH] [--finish-after-message PREVIOUS_REQUEST] [--payload REPLACEMENT_FILE | --search SEARCH_MENU]";
const DEADLINE: &str =
    "learned proposal explanation deadline reached; no diagnostic result was emitted";
const PAYLOAD_BYTES: usize = if MAX_PAYLOAD_BYTES < MAX_INTERVENTION_BYTES {
    MAX_PAYLOAD_BYTES
} else {
    MAX_INTERVENTION_BYTES
};

struct Request<'a> {
    config: &'a Path,
    recipe: &'a Path,
    topology: Option<&'a Path>,
    after: Option<u64>,
    payload: Option<&'a Path>,
    search: Option<&'a Path>,
}
impl<'a> Request<'a> {
    fn parse(args: &'a [String]) -> Result<Self, String> {
        if args.len() < 7
            || args[0] != "create-learned-generated"
            || args[3] != "--resume"
            || args[1..3]
                .iter()
                .any(|value| value.is_empty() || value.starts_with("--"))
        {
            return Err(USAGE.into());
        }
        let mut selected = false;
        let mut anchor = None;
        let mut topology = None;
        let mut after = None;
        let mut payload = None;
        let mut search = None;
        let mut at = 4;
        // Own the COMPLETE original argv before any existing option stripper.
        // An adjacent selector can never repair a missing path or predecessor.
        while at < args.len() {
            let flag = args[at].as_str();
            if flag == "--explain-proposal" {
                if selected {
                    return Err(USAGE.into());
                }
                selected = true;
                at += 1;
                continue;
            }
            if flag == "--finish-after-message" {
                if after.is_some() {
                    return Err(USAGE.into());
                }
                let value = args.get(at + 1).ok_or(USAGE)?;
                if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(USAGE.into());
                }
                let previous = value.parse::<u64>().map_err(|_| USAGE.to_owned())?;
                if previous == 0 {
                    return Err(USAGE.into());
                }
                after = Some(previous);
            } else {
                let slot = match flag {
                    "--history-anchor" => &mut anchor,
                    "--topology-profile" => &mut topology,
                    "--payload" => &mut payload,
                    "--search" => &mut search,
                    _ => return Err(USAGE.into()),
                };
                if slot.is_some() {
                    return Err(USAGE.into());
                }
                let value = args.get(at + 1).ok_or(USAGE)?;
                fields::path(value).map_err(|_| USAGE.to_owned())?;
                *slot = Some(Path::new(value));
            }
            at += 2;
        }
        if !selected || anchor.is_none() || (payload.is_some() && search.is_some()) {
            return Err(USAGE.into());
        }
        Ok(Self {
            config: Path::new(&args[1]),
            recipe: Path::new(&args[2]),
            topology,
            after,
            payload,
            search,
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

/// One allowance starts before input admission and covers replay, immutable
/// evaluation, formatting and the final pre-output boundary. No logical clock.
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
    let mut loaded = recipe::load(request.recipe, &config, false)?;
    checkpoint(wall, elapsed)?;
    // The strict parser already admitted exactly one input and no output. Reuse
    // the original regular-file/outside-authority custody checks unchanged.
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
    if let Some(path) = request.search {
        let menu = search::Menu::read(path)?;
        checkpoint(wall, elapsed)?;
        let investigation =
            read_with_elapsed(&config, &loaded, request.after, menu.scope.clone(), elapsed)?;
        return search::run(&investigation, &menu, wall, elapsed, output);
    }
    let payload = request
        .payload
        .map(|path| read_regular(path, PAYLOAD_BYTES))
        .transpose()?;
    checkpoint(wall, elapsed)?;
    let scope = InterventionScope::new(loaded.request, payload.is_some(), false, false, &[])
        .map_err(debug)?;
    let investigation = read_with_elapsed(&config, &loaded, request.after, scope, elapsed)?;
    let edits: Vec<_> = payload.into_iter().map(Intervention::Payload).collect();
    let diagnosis = investigation.run(&edits).map_err(unavailable)?;
    checkpoint(wall, elapsed)?;
    let bytes = render::encode(&diagnosis)?;
    checkpoint(wall, elapsed)?;
    output.write_all(&bytes).map_err(|error|
        format!("learned proposal explanation output failed: {error}; no authority or effect was changed"))?;
    output.flush().map_err(|error|
        format!("learned proposal explanation output flush failed: {error}; no authority or effect was changed"))
}

fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall {
        Err(DEADLINE.into())
    } else {
        Ok(())
    }
}
fn unavailable(error: impl std::fmt::Debug) -> String {
    format!(
        "historical learned proposal explanation unavailable: {error:?}; no authority or effect was changed"
    )
}

/// Capture the original request's FIRST policy admission during this SAME
/// fully guarded numerical replay. Complete suffix, independent ending guards,
/// reserve and exact release/predecessor selection qualify before any result.
pub(in super::super) fn read_with_elapsed(
    config: &Config,
    loaded: &Loaded,
    after: Option<u64>,
    scope: InterventionScope,
    elapsed: &mut impl FnMut() -> Duration,
) -> Result<FileProposalInvestigation, String> {
    if loaded.request == 0 || loaded.recovery_anchor.is_none() || loaded.anchor_output.is_some() {
        return Err("learned proposal diagnosis requires a nonzero request and independent input anchor without retention".into());
    }
    if let Some(after) = after {
        finish::check_ids(loaded.request, after)?;
    }
    if topology::history(loaded)?.is_some_and(|history| history.checkpoint().is_none()) {
        return Err(
            "learned proposal diagnosis requires an exact v2 topology-source checkpoint".into(),
        );
    }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let run = Cursor::begin(config, loaded)?
        .require_request_proposal_investigation(loaded.request, scope)
        .map_err(unavailable)?;
    let investigation = finish_replay(
        run,
        &mut |_| checkpoint(wall, elapsed),
        |run| run.progress(),
        |run, done| run.advance(done),
        |run| {
            run.receipt(loaded.request, after)?;
            run.finish_proposal_investigation()
        },
    )?;
    // Canonical confirmation is read-only but may take time. Its late return
    // cannot publish a diagnosis after the original elapsed allowance expired.
    checkpoint(wall, elapsed)?;
    Ok(investigation)
}
