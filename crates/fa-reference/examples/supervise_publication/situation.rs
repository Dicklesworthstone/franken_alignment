//! Privileged historical situation read, never recovery or live admission.
mod render;
use crate::config::{Config, debug, read_regular};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    guarded::anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    replay::situation::{FileSituation, FileSituationReplay, FileSituationReplayStatus,
        MAX_SITUATION_PAGE_ATTEMPTS}};
use fa_reference::action::consequence::oversight::actor::Knowledge;
use std::{io::Write, path::Path, time::{Duration, Instant}};

const USAGE: &str = "situation CONFIG [--history-anchor INPUT] [--page-size 1..64] [--learned-recipe RECIPE]";
const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

struct Arguments<'a> { config: &'a Path, anchor: Option<&'a Path>, recipe: Option<&'a Path>, page_size: usize }
impl<'a> Arguments<'a> {
    fn parse(args: &'a [String]) -> Result<Self, String> {
        if !matches!(args.len(), 2 | 4 | 6 | 8) || args[0] != "situation"
            || args[1].is_empty() || args[1].starts_with("--") { return Err(USAGE.into()); }
        let mut anchor = None; let mut page_size = None; let mut recipe = None;
        for pair in args[2..].chunks_exact(2) {
            if pair[1].is_empty() || pair[1].starts_with("--") { return Err(USAGE.into()); }
            match pair[0].as_str() {
                "--history-anchor" if anchor.is_none() => anchor = Some(Path::new(&pair[1])),
                "--learned-recipe" if recipe.is_none() => recipe = Some(Path::new(&pair[1])),
                "--page-size" if page_size.is_none() => {
                    let value = &pair[1];
                    if value.starts_with('0') || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                        return Err(USAGE.into());
                    }
                    let count = value.parse::<usize>().map_err(|_| USAGE.to_owned())?;
                    if !(1..=MAX_SITUATION_PAGE_ATTEMPTS).contains(&count) { return Err(USAGE.into()); }
                    page_size = Some(count);
                }
                _ => return Err(USAGE.into()),
            }
        }
        Ok(Self { config: Path::new(&args[1]), anchor, recipe, page_size: page_size.unwrap_or(32) })
    }
}

pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    // Own the complete argv before shared live/credential flags can be removed.
    let selected = Arguments::parse(args)?;
    #[cfg(not(target_os = "linux"))]
    if selected.recipe.is_some() {
        return Err("learned situation recipes require the Linux operator profile; no fallback".into());
    }
    let started = Instant::now();
    let config = Config::read(selected.config)?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || check_elapsed(started.elapsed(), wall);
    checkpoint()?;
    let anchor = selected.anchor.map(|path| {
        let bytes = read_regular(path, MAX_HISTORY_ANCHOR_BYTES)?;
        checkpoint()?;
        FileHistoryAnchor::read_trusted(&mut bytes.as_slice(), MAX_HISTORY_ANCHOR_BYTES).map_err(debug)
    }).transpose()?;
    checkpoint()?;
    let replay = match selected.recipe {
        None => FileOversight::begin_read_situation(&config.store, &config.profile, anchor.as_ref()).map_err(debug)?,
        Some(path) => {
            #[cfg(target_os = "linux")]
            { config.begin_learned_situation(path, anchor.as_ref(), &mut checkpoint)? }
            #[cfg(not(target_os = "linux"))]
            { let _ = path; return Err("learned situation recipes require the Linux operator profile; no fallback".into()); }
        }
    };
    let situation = finish_replay(replay, &mut checkpoint)?;
    emit(&situation, selected.page_size, out, MAX_OUTPUT_BYTES, &mut checkpoint)
}

fn check_elapsed(elapsed: Duration, wall: Duration) -> Result<(), String> {
    if elapsed >= wall { Err("situation elapsed allowance reached; no recovery or effect was performed".into()) }
    else { Ok(()) }
}

// One original event per cooperative turn; a partial history is never printed.
// The private checkpoint seam permits deterministic boundary tests, not a new
// executor or a promise to preempt synchronous file/decode/reducer operations.
fn finish_replay(mut replay: FileSituationReplay, checkpoint: &mut impl FnMut() -> Result<(), String>)
    -> Result<FileSituation, String>
{
    checkpoint()?;
    while replay.progress().status == FileSituationReplayStatus::Reading {
        replay.advance(1).map_err(debug)?;
        checkpoint()?;
    }
    let situation = replay.finish().map_err(debug)?;
    checkpoint()?;
    Ok(situation)
}

fn emit(situation: &FileSituation, page_size: usize, out: &mut impl Write,
    mut remaining_bytes: usize, checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<(), String>
{
    let mut continuation = None;
    loop {
        checkpoint()?;
        let Knowledge::Known { value, basis } = situation.page(continuation.as_ref(), page_size).map_err(debug)?
            else { return Err("situation is not a completely verified historical cut".into()); };
        let bytes = render::page(&value, basis)?;
        remaining_bytes = remaining_bytes.checked_sub(bytes.len())
            .ok_or("situation output allowance exhausted; any earlier pages are an incomplete enumeration")?;
        checkpoint()?;
        out.write_all(&bytes).and_then(|()| out.flush())
            .map_err(|error| format!("situation output failed: {error}; no recovery or retry was performed"))?;
        // A late write can leave a complete line visible despite a nonzero exit.
        checkpoint()?;
        continuation = value.next;
        if continuation.is_none() { return Ok(()); }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod learned_arguments;
