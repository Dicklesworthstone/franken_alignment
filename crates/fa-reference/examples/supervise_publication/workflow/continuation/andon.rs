//! Actual publication feeds the same durable native evaluation/accounting owner.
//! The original continuation owns source capture, helpers, human approval,
//! actor retries, effect publication, reconciliation, stopping and child cleanup.
#[path = "../../../native_andon/settings.rs"]
mod settings;
use settings::Settings;
use super::{Continuation, Deadline, RunResult, combine_failure, from_recovered, plus};
use crate::config::{Config, CLOCK_DOMAIN, debug, read_regular};
use crate::peers::PeerProfile;
use crate::publication::PublicationProfile;
use crate::recover_stop::guarded::{CreationOutput, Custody, SubmitProfile};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve, Reconciliation,
    observed::{FileOversight, guarded::recovery::FileGuardedRecoveryStatus}};
use fa_reference::action::consequence::oversight::actor_wire::{Command, decode_command, MAX_FRAME_BYTES};
use std::{io::Write, path::Path, time::{Duration, Instant}};

const USAGE: &str = "usage: supervise_publication create|create-checked CONFIG SUBMIT_JSON [WITNESS_PROFILE] --andon ANDON_PROFILE --retain-history-anchor NEW_FILE [--reviewer-profile REVIEWER_JSON]; or submit|submit-checked CONFIG SUBMIT_JSON [WITNESS_PROFILE] --andon ANDON_PROFILE --guarded SUBMIT_RECOVERY_PROFILE [--reviewer-profile REVIEWER_JSON]";
struct Arguments<'a> { create: bool, publication: Option<&'a Path>,
    andon: &'a Path, custody: &'a Path, peers: Option<&'a Path> }
fn arguments(args: &[String]) -> Result<Arguments<'_>, String> {
    let (create, checked) = match args.first().map(String::as_str) {
        Some("create") => (true, false), Some("submit") => (false, false),
        Some("create-checked") => (true, true), Some("submit-checked") => (false, true),
        _ => return Err(USAGE.into()),
    };
    let base = if checked { 4 } else { 3 };
    let expected = if checked { [8, 10] } else { [7, 9] };
    if !expected.contains(&args.len()) || args[1..base].iter().any(|v| v.is_empty() || v.starts_with("--")) {
        return Err(USAGE.into());
    }
    let publication = checked.then(|| Path::new(args[3].as_str()));
    let mut andon = None; let mut custody = None; let mut peers = None;
    for pair in args[base..].chunks_exact(2) {
        let slot = match pair[0].as_str() {
            "--andon" => &mut andon,
            "--retain-history-anchor" if create => &mut custody,
            "--guarded" if !create => &mut custody,
            "--reviewer-profile" => &mut peers,
            _ => return Err(USAGE.into()),
        };
        if slot.is_some() || pair[1].is_empty() || pair[1].starts_with("--") { return Err(USAGE.into()); }
        *slot = Some(Path::new(&pair[1]));
    }
    Ok(Arguments { create, publication, andon: andon.ok_or(USAGE)?,
        custody: custody.ok_or(USAGE)?, peers })
}

pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    // Routed before ANY shared flag removal. Unsupported combinations cannot
    // silently discard native accounting or use a legacy opener.
    let selected = arguments(args)?;
    let started = Instant::now();
    let config = Config::read(Path::new(&args[1]))?;
    let settings = Settings::read(selected.andon)?;
    let publication = selected.publication.map(PublicationProfile::read).transpose()?;
    if let Some(profile) = &publication { profile.simple_limits()?; }
    let document = read_regular(Path::new(&args[2]), MAX_FRAME_BYTES)?;
    let peers = selected.peers.map(PeerProfile::read).transpose()?;
    let profile = if selected.create { None } else { Some(SubmitProfile::read(selected.custody)?) };
    let mode = match &profile { Some(profile) => Mode::Existing(profile), None => Mode::Create(selected.custody) };
    let result = run(config, Submission { document: &document, settings: &settings, mode,
        peers: peers.as_ref(), publication: publication.as_ref() },
        started, crate::workflow::clock, &mut || started.elapsed())?;
    // The original wire response and independent output failure semantics.
    super::guarded::emit(result, out)
}

enum Mode<'a> { Create(&'a Path), Existing(&'a SubmitProfile) }
struct Submission<'a> { document: &'a [u8], settings: &'a Settings, mode: Mode<'a>,
    peers: Option<&'a PeerProfile>, publication: Option<&'a PublicationProfile> }
enum Retention { Creation(CreationOutput), Recovery(Custody) }
impl Retention {
    fn retain(&self, host: &FileOversight) -> Result<(), String> {
        match self {
            Self::Creation(output) => output.retain(host),
            Self::Recovery(custody) => host.history_anchor_after(custody.anchor()).map_err(debug)
                .and_then(|anchor| custody.retain(&anchor)).map_err(|error| format!(
                    "native publication history retention failed: {error}; preserve the previous archive and original request; no effect was resent")),
        }
    }
}

fn run<F>(config: Config, submission: Submission<'_>, started: Instant,
    mut time: F, elapsed: &mut impl FnMut() -> Duration) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let Submission { document, settings, mode, peers, publication } = submission;
    let Command::Submit { request, proposal } = decode_command(document).map_err(debug)? else {
        return Err("native publication requires an original Submit document".into());
    };
    if config.profile.delivery.clock_domain != CLOCK_DOMAIN { return Err("native publication clock domain mismatch".into()); }
    if let Some(peers) = peers { peers.check_host(&config)?; }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || if elapsed() < wall { Ok(()) } else { Err("native publication invocation deadline reached".to_owned()) };
    checkpoint()?;
    let start = time(); let runtime_deadline = plus(start, config.timing.runtime_ms)?;
    let (host, roles, retention) = match mode {
        Mode::Create(path) => {
            settings.creation()?;
            let output = CreationOutput::admit(&config, path)?;
            checkpoint()?;
            // Creation is not receipt recovery; refuse an expired original
            // proposal before creating a new domain. No actor field is rewritten.
            Deadline { logical: proposal.deadline.min(runtime_deadline), started, wall }.check(time())?;
            let (host, roles) = match publication {
                Some(profile) => settings.create_checked(&config, profile.simple_limits()?, RecoveryReserve::terminal())?,
                None => settings.create(&config, Some(RecoveryReserve::terminal()))?,
            };
            (host, roles, Retention::Creation(output))
        }
        Mode::Existing(profile) => {
            let custody = profile.admit(&config)?;
            if !custody.retention_requested() { return Err("native publication requires a new independent successor anchor".into()); }
            let expected = settings.requirements(&config, profile.minimum())?;
            let expected = match publication {
                Some(witness) => expected.require_publication(witness.simple_limits()?).map_err(debug)?,
                None => expected,
            };
            checkpoint()?;
            let mut recovery = FileOversight::begin_open_andon_guarded(&config.store, config.profile.clone(),
                &expected, &settings.protocol, custody.anchor())
                .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal())).map_err(debug)?;
            loop {
                checkpoint()?; // Includes Ready, before the original final transaction.
                let progress = recovery.progress();
                if progress.status == FileGuardedRecoveryStatus::Ready { break; }
                recovery.advance(progress.replayed_events, 1).map_err(debug)?;
            }
            let now = time(); checkpoint()?;
            Deadline { logical: runtime_deadline, started, wall }.check(now)?;
            let (host, roles, outcomes) = recovery.finish_reconciled(|| now).map_err(debug)?;
            let retention = Retention::Recovery(custody);
            let outstanding = host.inspect().control.ledger.stages.values().any(|stage|
                matches!(stage, ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown));
            if outstanding || outcomes.values().any(|result| !matches!(result, Ok(Reconciliation::Resolved(_)))) {
                return Err(combine_failure(Some("native recovery acknowledged; unresolved prior deliveries prevent new work; original charges remain".into()),
                    retention.retain(&host).err()).expect("unresolved liabilities"));
            }
            (host, roles, retention)
        }
    };
    let acknowledged = host.revision();
    // Keep this checkpoint after an acknowledged creation/recovery too: expiry
    // must retain that real cut, not strand the first anchor or trigger recreation.
    if let Err(error) = checkpoint() {
        return Err(combine_failure(Some(format!("native owner acknowledged at revision {acknowledged}; {error}")),
            retention.retain(&host).err()).expect("deadline"));
    }
    // The separate evaluator role is intentionally not exposed to actor intake
    // or helpers. Later native_andon assess recovers fresh custodian roles.
    drop(roles.evaluator);
    from_recovered(config, host, roles.oversight.human, Continuation {
        document, request, proposal, peers, publication, credibility: None,
        runtime_deadline, started,
    }, time, |host| retention.retain(host))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod checked_tests;
