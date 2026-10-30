//! One declared-roster observation. No Store recovery, coordinator or mutation.
mod profile;
mod render;
use profile::{Member, Profile, MAX_PROFILE_BYTES};
use super::{Config, FileHistoryAnchor, FileOversight, FileSituationReplay,
    MAX_HISTORY_ANCHOR_BYTES, check_elapsed, debug, read_regular};
use crate::config::CONFIG_BYTES;
use fa_reference::action::consequence::delivery::persistent::observed::replay::situation::fleet::{
    FileSituationFleet, FileSituationFleetLimits, FileSituationFleetReplay, FileSituationFleetReplayStatus,
};
use std::{collections::BTreeSet, fs, io::Write, path::{Path, PathBuf}, time::{Duration, Instant}};

pub(super) const USAGE: &str = "situation --fleet FLEET_PROFILE";

pub(super) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with(args, &mut || started.elapsed(), out)
}

// The same original invocation clock covers profile/config/recipe intake,
// every member transition, formatting and output. It is never renewed per member.
pub(super) fn command_with(args: &[String], elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write) -> Result<(), String>
{
    if args.len() != 3 || args[0] != "situation" || args[1] != "--fleet"
        || args[2].is_empty() || args[2].starts_with("--") { return Err(USAGE.into()); }
    let bytes = read_regular(Path::new(&args[2]), MAX_PROFILE_BYTES)?;
    let spec = Profile::decode(&bytes)?;
    let remaining = spec.input_bytes.checked_sub(bytes.len()).ok_or("fleet profile exceeds input allowance")?;
    run(&spec, remaining, elapsed, out)
}

fn run(spec: &Profile, mut remaining: usize, elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write) -> Result<(), String>
{
    let mut wall = spec.wall;
    let mut roots = BTreeSet::new();
    let mut configured = Vec::with_capacity(spec.members.len());
    let mut journal_bytes = spec.journal_bytes;
    // Resolve the entire readable roster before any model or journal intake.
    // Aliases cannot masquerade as two independent observed members. Missing
    // configurations/stores retain their declared slots as Unknown.
    for member in &spec.members {
        check_elapsed(elapsed(), wall)?;
        let selected = budgeted_read(&member.config, CONFIG_BYTES, &mut remaining)
            .and_then(|bytes| Config::decode(&bytes));
        check_elapsed(elapsed(), wall)?;
        let config = match selected {
            Ok(mut config) => {
                wall = wall.min(Duration::from_millis(config.timing.runtime_ms));
                check_elapsed(elapsed(), wall)?;
                match fs::canonicalize(&config.store) {
                    Ok(identity) => {
                        if !roots.insert(identity.clone()) { return Err("fleet members alias the same authority store".into()); }
                        // Reserve the original reader's complete byte bound and
                        // its overflow sentinel before it can open the image.
                        let charge = config.profile.delivery.limits.bytes.checked_add(1)
                            .ok_or("fleet journal allowance overflow")?;
                        journal_bytes = journal_bytes.checked_sub(charge)
                            .ok_or("fleet aggregate journal allowance exceeded")?;
                        config.store = identity;
                        Some(config)
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        };
        configured.push(config);
    }
    let forbidden: Vec<_> = roots.into_iter().collect();
    for (member, config) in spec.members.iter().zip(&configured) {
        if config.is_some() { external(&member.config, &forbidden)?; }
    }
    let mut checkpoint = || check_elapsed(elapsed(), wall);
    let mut inputs = Vec::with_capacity(configured.len());
    for (member, config) in spec.members.iter().zip(configured) {
        checkpoint()?;
        let cursor = match config {
            Some(config) => read_member(&config, member, &mut remaining, &forbidden, &mut checkpoint)?,
            None => None,
        };
        inputs.push((member.id, cursor));
        checkpoint()?;
    }
    let mut replay = FileSituationFleetReplay::new(inputs, FileSituationFleetLimits {
        members: spec.members.len(), events: spec.events,
    }).map_err(debug)?;
    checkpoint()?;
    while replay.progress().status == FileSituationFleetReplayStatus::Reading {
        replay.advance(1).map_err(debug)?;
        checkpoint()?;
    }
    let report = replay.finish().map_err(debug)?;
    checkpoint()?;
    emit(&report, spec.page_size, spec.output_bytes, out, &mut checkpoint)
}

fn read_member(config: &Config, member: &Member, remaining: &mut usize,
    forbidden: &[PathBuf], checkpoint: &mut impl FnMut() -> Result<(), String>)
    -> Result<Option<FileSituationReplay>, String>
{
    checkpoint()?;
    let anchor = member.anchor.as_ref().map(|path| {
        external(path, forbidden)?;
        let bytes = budgeted_read(path, MAX_HISTORY_ANCHOR_BYTES, remaining)?;
        FileHistoryAnchor::read_trusted(&mut bytes.as_slice(), MAX_HISTORY_ANCHOR_BYTES).map_err(debug)
    }).transpose();
    checkpoint()?;
    let Ok(anchor) = anchor else { return Ok(None); };
    #[cfg(target_os = "linux")]
    let mut cancelled = None;
    let result = match &member.recipe {
        None => FileOversight::begin_read_situation(&config.store, &config.profile, anchor.as_ref()).map_err(debug),
        Some(path) => {
            #[cfg(target_os = "linux")]
            { config.begin_learned_fleet_situation(path, anchor.as_ref(), remaining, forbidden, &mut || {
                let result = checkpoint();
                if let Err(error) = &result { cancelled = Some(error.clone()); }
                result
            }) }
            #[cfg(not(target_os = "linux"))]
            { let _ = path; Err("learned fleet situations require Linux; no fallback".to_owned()) }
        }
    };
    #[cfg(target_os = "linux")]
    if let Some(error) = cancelled { return Err(error); }
    checkpoint()?;
    // A coarse unavailable observation is intentional. No path, model input or
    // arbitrary parser error reaches stdout, and no alternate reader is tried.
    Ok(result.ok())
}

fn external(path: &Path, forbidden: &[PathBuf]) -> Result<(), String> {
    let canonical = fs::canonicalize(path).map_err(debug)?;
    if forbidden.iter().any(|root| path.starts_with(root) || canonical.starts_with(root)) {
        return Err("fleet independent inputs must be outside every authority store".into());
    }
    Ok(())
}

// Reserve the maximum attempted read (including growth sentinel) before I/O.
// Only successful reads refund their unused reservation; failures cannot refill
// a shared allowance for later members. This is byte accounting, not peak RSS.
fn budgeted_read(path: &Path, maximum: usize, remaining: &mut usize) -> Result<Vec<u8>, String> {
    if *remaining == 0 { return Err("fleet input allowance exhausted".into()); }
    let bound = maximum.min(*remaining - 1);
    let reservation = bound + 1;
    *remaining -= reservation;
    let bytes = read_regular(path, bound)?;
    *remaining += reservation - bytes.len();
    Ok(bytes)
}

fn emit(report: &FileSituationFleet, page_size: usize, mut remaining: usize,
    out: &mut impl Write, checkpoint: &mut impl FnMut() -> Result<(), String>) -> Result<(), String>
{
    let mut continuation = None;
    loop {
        checkpoint()?;
        let page = report.page(continuation.as_ref(), page_size).map_err(debug)?;
        let bytes = render::page(&page)?;
        remaining = remaining.checked_sub(bytes.len())
            .ok_or("fleet situation output allowance exhausted; earlier lines are an incomplete enumeration")?;
        checkpoint()?;
        out.write_all(&bytes).and_then(|()| out.flush())
            .map_err(|error| format!("fleet situation output failed: {error}; no recovery or retry was performed"))?;
        checkpoint()?;
        continuation = page.next;
        if continuation.is_none() { break; }
    }
    if !report.coverage().all_members_known() {
        return Err("fleet enumeration completed with unknown members; no recovery or effect was performed".into());
    }
    Ok(())
}
