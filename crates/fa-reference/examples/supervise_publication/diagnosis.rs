//! Offline, request-key explanation through the ORIGINAL anchored proposal read.
//! No live authority, helper, current evidence or effect path is constructed.
mod render;
mod search;
#[cfg(test)]
mod tests;
use crate::config::{Config, debug, read_regular};
use crate::recover_stop::guarded::Profile;
use fa_reference::action::MAX_PAYLOAD_BYTES;
use fa_reference::action::consequence::experiment::{Intervention, InterventionScope, MAX_INTERVENTION_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, guarded::{FileGuardSet, FileRecoveryRequirements,
        investigation::{FileProposalInvestigation, FileProposalReadStatus}}};
use std::{io::Write, path::Path, time::{Duration, Instant}};

const USAGE: &str = "usage: supervise_publication explain-proposal CONFIG REQUEST_ID RECOVERY_PROFILE [--payload REPLACEMENT_FILE | --search SEARCH_MENU]";
const DEADLINE: &str = "proposal explanation replay deadline reached; no diagnostic result was emitted";
const PAYLOAD_BYTES: usize = if MAX_PAYLOAD_BYTES < MAX_INTERVENTION_BYTES {
    MAX_PAYLOAD_BYTES
} else { MAX_INTERVENTION_BYTES };

struct Request<'a> {
    config: &'a Path,
    profile: &'a Path,
    request: u64,
    payload: Option<&'a Path>,
    search: Option<&'a Path>,
}
impl<'a> Request<'a> {
    fn parse(args: &'a [String]) -> Result<Self, String> {
        if !matches!(args.len(), 4 | 6) || args[0] != "explain-proposal"
            || (args.len() == 6 && !matches!(args[4].as_str(), "--payload" | "--search")) {
            return Err(USAGE.into());
        }
        let text = &args[2];
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(USAGE.into());
        }
        let request = text.parse::<u64>().map_err(|_| USAGE.to_owned())?;
        if request == 0 { return Err(USAGE.into()); }
        Ok(Self { config: Path::new(&args[1]), profile: Path::new(&args[3]), request,
            payload: (args.len() == 6 && args[4] == "--payload").then(|| Path::new(&args[5])),
            search: (args.len() == 6 && args[4] == "--search").then(|| Path::new(&args[5])) })
    }
}

pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    // One elapsed budget includes configuration, independent custody and payload
    // ingestion. Its bound is checked as soon as CONFIG has supplied that bound.
    let started = Instant::now();
    command_with_elapsed(args, &mut || started.elapsed(), out)
}

fn command_with_elapsed(args: &[String], elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write) -> Result<(), String>
{
    let request = Request::parse(args)?;
    let config = Config::read(request.config)?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let profile = Profile::read(request.profile)?;
    checkpoint(wall, elapsed)?;
    if let Some(path) = request.search {
        let menu = search::Menu::read(path)?;
        checkpoint(wall, elapsed)?;
        let investigation = read_with_elapsed(&config, &profile, request.request, menu.scope.clone(), elapsed)?;
        return search::run(&investigation, &menu, wall, elapsed, out);
    }
    let payload = request.payload.map(|path| read_regular(path, PAYLOAD_BYTES)).transpose()?;
    checkpoint(wall, elapsed)?;
    run_with_elapsed(&config, &profile, request.request, payload.as_deref(), elapsed, out)
}

fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall { Err(DEADLINE.into()) } else { Ok(()) }
}

// The executable and deterministic boundary tests use this exact implementation.
// A single native event, bounded file read and output write remain synchronous;
// checkpoints are not a hard wall-clock latency guarantee for those operations.
fn run_with_elapsed(config: &Config, profile: &Profile, request: u64, payload: Option<&[u8]>,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String>
{
    if request == 0 { return Err("proposal explanation requires a nonzero request".into()); }
    if payload.is_some_and(|bytes| bytes.len() > PAYLOAD_BYTES) {
        return Err("replacement payload exceeds the original experiment limits".into());
    }
    let scope = InterventionScope::new(request, payload.is_some(), false, false, &[]).map_err(debug)?;
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let investigation = read_with_elapsed(config, profile, request, scope, elapsed)?;
    let edits: Vec<_> = payload.into_iter().map(|bytes| Intervention::Payload(bytes.to_vec())).collect();
    let diagnosis = investigation.run(&edits).map_err(unavailable)?;
    checkpoint(wall, elapsed)?;
    // Complete the bounded predicate-tree encoding before any output. A timeout
    // or reconstruction refusal never publishes a promising partial diagnosis.
    let bytes = render::encode(&diagnosis)?;
    checkpoint(wall, elapsed)?;
    out.write_all(&bytes).map_err(|error| format!("proposal explanation output failed: {error}; no authority or effect was changed"))?;
    out.flush().map_err(|error| format!("proposal explanation output flush failed: {error}; no authority or effect was changed"))
}

fn unavailable(error: impl std::fmt::Debug) -> String {
    format!("historical proposal explanation unavailable: {error:?}; no authority or effect was changed")
}

// Both operator modes share the SAME independent custody, complete guard set,
// one-event replay, full suffix and Ready checkpoints. No preliminary ID read.
fn read_with_elapsed(config: &Config, profile: &Profile, request: u64, scope: InterventionScope,
    elapsed: &mut impl FnMut() -> Duration) -> Result<FileProposalInvestigation, String>
{
    let wall = Duration::from_millis(config.timing.runtime_ms);
    checkpoint(wall, elapsed)?;
    let (anchor, minimum) = profile.admit_investigation(config)?;
    checkpoint(wall, elapsed)?;
    // EXACT ordinary source-publication profile: None means required absence,
    // not acceptance of an unknown optional guard or a weaker fallback profile.
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None, minimum,
    };
    let mut read = FileOversight::begin_read_request_proposal_investigation_anchored(
        &config.store, &config.profile, &expected, &anchor, request, scope).map_err(unavailable)?;
    loop {
        checkpoint(wall, elapsed)?;
        let current = read.progress();
        if current.status == FileProposalReadStatus::Ready { break; }
        read.advance(current.replayed_events, 1).map_err(unavailable)?;
    }
    let investigation = read.finish().map_err(unavailable)?;
    checkpoint(wall, elapsed)?;
    Ok(investigation)
}
