//! New publication work through the original independently anchored recovery.
//! Fresh source, helper review and human approval remain the shared continuation.
use super::{Continuation, Deadline, RunResult, combine_failure, from_recovered, plus};
use crate::config::{CLOCK_DOMAIN, Config, debug};
use crate::peers::PeerProfile;
use crate::recover_stop::guarded::{Custody, SubmitProfile};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation,
    RecoveryReserve, observed::{FileOversight, guarded::{FileGuardSet, FileRecoveryRequirements,
        recovery::{FileGuardedRecoveryStatus, ReconciledGuardedOwner}}}};
use fa_reference::action::consequence::oversight::actor_wire::{Command, decode_command};
use std::{path::Path, time::{Duration, Instant}};

#[path = "guarded/mediated.rs"] pub(crate) mod mediated;

pub(crate) struct Submission<'a> {
    pub document: &'a [u8],
    pub profile: &'a SubmitProfile,
    pub peers: Option<&'a PeerProfile>,
    pub credibility: Option<&'a Path>,
}

/// Recover the exact independent history, reconcile old obligations, then drive
/// this original actor document. No new authority, source observation, target,
/// deadline or expected epoch is manufactured from the recovery profile.
pub(crate) fn submit<F>(config: Config, submission: Submission<'_>, time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let started = Instant::now();
    submit_with_elapsed(config, submission, time, started, &mut || started.elapsed())
}

// One original start Instant also enters the live Deadline. The elapsed seam
// permits deterministic tests at every replay boundary; it does not renew time.
pub(super) fn submit_with_elapsed<F>(config: Config, submission: Submission<'_>, mut time: F,
    started: Instant, elapsed: &mut impl FnMut() -> Duration) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    let Submission { document, profile, peers, credibility } = submission;
    let Command::Submit { request, proposal } = decode_command(document).map_err(debug)? else {
        return Err("guarded submit requires an original submit document".into());
    };
    if config.profile.delivery.clock_domain != CLOCK_DOMAIN {
        return Err("guarded submit requires the configured Unix-millisecond clock domain".into());
    }
    if let Some(peers) = peers { peers.check_host(&config)?; }
    let wall = Duration::from_millis(config.timing.runtime_ms);
    let mut checkpoint = || {
        if elapsed() >= wall { Err("guarded submission recovery replay deadline reached".to_owned()) }
        else { Ok(()) }
    };
    checkpoint()?;
    let start = time();
    let runtime_deadline = plus(start, config.timing.runtime_ms)?;
    let custody = profile.admit(&config)?;
    checkpoint()?;
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None,
            source: Some(config.source_policy), identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: profile.minimum(),
    };
    let mut recovery = FileOversight::begin_open_guarded_anchored(
        &config.store, config.profile.clone(), &expected, custody.anchor())
        .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal())).map_err(unconfirmed)?;
    loop {
        checkpoint()?; // Includes a distinct Ready checkpoint before any fence.
        let current = recovery.progress();
        if current.status == FileGuardedRecoveryStatus::Ready { break; }
        recovery.advance(current.replayed_events, 1).map_err(unconfirmed)?;
    }
    let now = time();
    Deadline { logical: runtime_deadline, started, wall }.check(now)?;
    let ReconciledGuardedOwner { owner, roles, outcomes } =
        recovery.finish_reconciled(|| now).map_err(unconfirmed)?;
    let revision = owner.revision();
    // This command admits new work only after all old deliveries resolve.
    // An old irrecoverable entry can be omitted from a fresh sweep: check BOTH.
    let outstanding = owner.inspect().control.ledger.stages.values().any(|stage|
        matches!(stage, ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown));
    if outstanding || outcomes.values().any(|result| !matches!(result, Ok(Reconciliation::Resolved(_)))) {
        let error = combine_failure(Some("unresolved prior deliveries; no new work admitted; charges remain in the original ledger".into()),
            retain(&custody, &owner).err()).expect("unresolved obligations");
        return Err(acknowledged(revision, &error));
    }
    // The same source/refusal, review, two-key publication, retry and cleanup
    // implementation serves legacy and guarded submission. No decoded actor
    // field is rewritten to match the new recovery epoch or endpoint version.
    let result = from_recovered(config, owner, roles.human, Continuation {
        document, request, proposal, peers, publication: None, credibility, runtime_deadline, started,
    }, time, |host| retain(&custody, host));
    match result {
        Ok(mut result) => {
            result.failure = result.failure.map(|error| acknowledged(revision, &error));
            Ok(result)
        }
        Err(error) => Err(acknowledged(revision, &error)),
    }
}

fn retain(custody: &Custody, host: &FileOversight) -> Result<(), String> {
    if !custody.retention_requested() { return Ok(()); }
    host.history_anchor_after(custody.anchor()).map_err(debug)
        .and_then(|anchor| custody.retain(&anchor))
        .map_err(|error| format!("history anchor retention failed: {error}; keep the previous archive and select a new output path; no effect retry was sent"))
}
fn unconfirmed(error: JournalError) -> String {
    format!("guarded submission recovery is unconfirmed: {}; no new actor submission was forwarded", debug(error))
}
fn acknowledged(revision: u64, error: &str) -> String {
    format!("guarded submission recovery acknowledged at revision {revision}; {error}")
}

const USAGE: &str = "usage: supervise_publication submit CONFIG SUBMIT_JSON --guarded RECOVERY_PROFILE [--reviewer-profile REVIEWER_JSON] [--credibility-activation EVIDENCE_FILE]";

/// The outer command has already removed the existing credibility option. Only
/// these explicit remaining options select this mode; no rejected guarded call
/// can return to the legacy opener or guess a profile from the journal.
pub(crate) fn command(args: &[String], credibility: Option<&Path>, out: &mut impl std::io::Write)
    -> Result<(), String>
{
    if args.iter().skip(3).any(|argument| argument == "--topology-profile") {
        return mediated::command(args, credibility, out);
    }
    let (recovery_path, peer_path) = arguments(args)?;
    let config = Config::read(Path::new(&args[1]))?;
    let document = crate::config::read_regular(Path::new(&args[2]),
        fa_reference::action::consequence::oversight::actor_wire::MAX_FRAME_BYTES)?;
    let profile = SubmitProfile::read(recovery_path)?;
    let peers = peer_path.map(PeerProfile::read).transpose()?;
    let result = submit(config, Submission { document: &document, profile: &profile,
        peers: peers.as_ref(), credibility }, crate::workflow::clock)?;
    emit(result, out)
}

fn arguments(args: &[String]) -> Result<(&Path, Option<&Path>), String> {
    if !matches!(args.len(), 5 | 7) || args[0] != "submit" { return Err(USAGE.into()); }
    let mut guarded = None;
    let mut peers = None;
    for pair in args[3..].chunks_exact(2) {
        let selected = match pair[0].as_str() {
            "--guarded" => &mut guarded,
            "--reviewer-profile" => &mut peers,
            _ => return Err(USAGE.into()),
        };
        if selected.is_some() || pair[1].is_empty() || pair[1].starts_with("--") {
            return Err(USAGE.into());
        }
        *selected = Some(Path::new(&pair[1]));
    }
    Ok((guarded.ok_or(USAGE)?, peers))
}

// Retention and original child cleanup already ran before response rendering.
// Preserve both a workflow/export failure and output failure, never issue a retry.
pub(super) fn emit(result: RunResult, out: &mut impl std::io::Write) -> Result<(), String> {
    use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
    let executed = matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }));
    let mut failure = result.failure;
    if result.cleanup_pending != 0 {
        failure = combine_failure(failure, Some(format!("{} direct helper children lack reaping confirmation", result.cleanup_pending)));
    }
    if !executed {
        failure = combine_failure(failure, Some("the original actor result does not confirm execution; see the JSON response".into()));
    }
    if let Err(error) = out.write_all(&result.response.encode())
        .and_then(|_| out.write_all(b"\n")).and_then(|_| out.flush()) {
        failure = combine_failure(failure, Some(format!(
            "publication response output failed: {error:?}; acknowledged work is not undone; recover the original request, do not resend an effect")));
    }
    failure.map_or(Ok(()), Err)
}
