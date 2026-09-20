//! One explicit request through existing actor, helper, reviewer and delivery APIs.
//! This is a synchronous executable consumer, not an alternative executor/ledger.
pub mod continuation;
pub(crate) mod control;

use super::config::{Config, CLOCK_DOMAIN, debug};
use super::peers::{Admission, PeerProfile};
use super::publication::PublicationProfile;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{StopRequest};
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase, FileSupervisedDriver};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::FileReviewLaunch;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{ReviewerPhase, ReviewerProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::delivery::persistent::requests::{FileRequestDisposition};
use fa_reference::action::consequence::oversight::{ReviewWindow};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, WireResponse, decode_command, encode_command};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Never synthesize a current time from saved ticks or a restarted Instant.
/// Failure to obtain this clock aborts the command rather than issuing a grant
/// using a default tick. Native time observations reject backward wall-clock jumps.
pub fn clock() -> ElapsedTick {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH)
        .expect("Unix-millisecond clock is unavailable").as_millis();
    ElapsedTick(u64::try_from(ms).expect("Unix-millisecond clock overflow"))
}
fn plus(now: ElapsedTick, duration: u64) -> Result<ElapsedTick, String> {
    now.0.checked_add(duration).map(ElapsedTick).ok_or_else(|| "clock overflow".into())
}
struct Deadline { logical: ElapsedTick, started: Instant, wall: Duration }
impl Deadline {
    fn check(&self, now: ElapsedTick) -> Result<(), String> {
        if now >= self.logical || self.started.elapsed() >= self.wall { Err("one-shot workflow deadline reached".into()) }
        else { Ok(()) }
    }
}
fn pause(ms: u64) { std::thread::sleep(Duration::from_millis(ms)); }

pub struct RunResult {
    pub response: WireResponse,
    pub failure: Option<String>,
    pub cleanup_pending: usize,
}

/// Explicit compatibility path. It assumes the namespace/embedding operator
/// already isolates the reviewer endpoint; it does not inspect peer credentials.
pub fn run<F>(config: Config, document: &[u8], resume: bool, time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    run_with_peers(config, document, resume, None, time)
}

/// resume permits only exact retrieval/reconciliation of a PREEXISTING request.
/// It never reads the evidence file, launches helpers, obtains a new human key,
/// recreates an old job, or retries the original effect. A peer profile is for
/// creation only; query-only recovery never pretends it authenticated a new peer.
pub fn run_with_peers<F>(config: Config, document: &[u8], resume: bool,
    peers: Option<&PeerProfile>, time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    run_with_publication(config, document, resume, peers, None, time)
}

/// Explicit stronger mode over this SAME workflow. The native publication profile
/// is persisted before any actor proposal and never retried through the legacy
/// path. Recovery is still receipt-only and reads no original/current evidence.
pub fn run_with_publication<F>(mut config: Config, document: &[u8], resume: bool,
    peers: Option<&PeerProfile>, publication: Option<&PublicationProfile>, mut time: F)
    -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    if resume && peers.is_some() { return Err("query-only resume does not accept a reviewer peer profile".into()); }
    if let Some(profile) = peers { profile.check_host(&config)?; }
    let command = decode_command(document).map_err(debug)?;
    let Command::Submit { request, proposal } = command else { return Err("expected an original submit document".into()); };
    let start = time();
    let logical = if resume { plus(start, config.timing.runtime_ms)? }
        else { proposal.deadline.min(plus(start, config.timing.runtime_ms)?) };
    let deadline = Deadline { logical, started: Instant::now(), wall: Duration::from_millis(config.timing.runtime_ms) };
    deadline.check(start)?;
    let (mut host, reviewer) = if resume {
        let result = match publication {
            Some(profile) => profile.open(&config.store, config.profile.clone()),
            None => FileOversight::open(&config.store, config.profile.clone()),
        }.map_err(debug)?;
        // Opening already performs the original fence. No absent-key fallback
        // is allowed to mint a new request after that recovery.
        result.0.request_status(request).map_err(debug)?;
        result
    } else {
        let (mut host, reviewer) = match publication {
            Some(profile) => profile.create(&config.store, config.profile.clone()),
            None => FileOversight::create(&config.store, config.profile.clone()),
        }.map_err(debug)?;
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).map_err(debug)?;
        host.enable_file_source(host.revision(), config.source_policy).map_err(debug)?;
        (host, reviewer)
    };
    if host.publication_validation_profile().map_err(debug)? != publication.map(|profile| profile.limits) {
        return Err("stored witness profile requires its matching explicit checked mode".into());
    }
    if host.file_source_status().map(|s| s.policy) != Some(config.source_policy)
        || host.journal_capacity().map_err(debug)?.reserve() != Some(RecoveryReserve::terminal())
        || !host.publication_guard_required() || config.profile.delivery.clock_domain != CLOCK_DOMAIN
    { return Err("stored deployment does not match this mandatory source/recovery profile".into()); }
    let admission = if resume { None } else {
        let observed = host.refresh_file_source(host.revision(), &mut config.source, time()).map_err(debug)?;
        host.observe_time(host.revision(), time()).map_err(debug)?;
        Some(observed.snapshot().clone())
    };
    let (port, mut driver) = host.into_supervised_driver();
    let mut wire = ActorWire::new(port);
    if let Some(snapshot) = admission {
        let revision = driver.supervisor().host().map_err(debug)?.revision();
        driver.supervisor_mut().set_snapshot(revision, Some(snapshot)).map_err(debug)?;
    }
    let submitted = wire.exchange(document);
    if submitted.result.is_err() {
        return Ok(RunResult { response: submitted, failure: Some("original actor gateway refused submission".into()), cleanup_pending: 0 });
    }
    let work = if resume { resume_existing(&mut driver, request, &mut time) }
        else { execute(&mut driver, &reviewer, &mut config, request, &deadline, (peers, publication), &mut time) };
    let failure = match work {
        Ok(()) => None,
        Err(error) => {
            // The original stop comes BEFORE its fallible drain. Preserve both
            // facts if durable stop succeeds but settlement does not. Never erase
            // a storage fault or refund an unknown external outcome locally.
            let stopped = stop(&mut driver, request, &mut time);
            Some(match stopped { Ok(()) => error, Err(stop) => format!("{error}; stop/drain: {stop}") })
        }
    };
    let response = wire.exchange(&encode_command(&Command::Poll { request }).map_err(debug)?);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    Ok(RunResult { response, failure, cleanup_pending })
}

fn resume_existing<F>(driver: &mut FileSupervisedDriver, request: u64, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let status = driver.supervisor().host().map_err(debug)?.request_status(request).map_err(debug)?;
    if matches!(status.disposition, FileRequestDisposition::Admitted {
        stage: ActionState::Dispatching | ActionState::Unknown, ..
    }) {
        driver.resume_reconciliation(request).map_err(debug)?;
        // The provider is unreachable on the original query-only path. Do not
        // substitute new context for historical outcome evidence.
        driver.step_with_evidence(time, |_, _| Err(fa_reference::Error::Incomplete), None).map_err(debug)?;
    }
    Ok(())
}

fn execute<F>(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer, config: &mut Config,
    request: u64, deadline: &Deadline, profiles: (Option<&PeerProfile>, Option<&PublicationProfile>),
    time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let (peers, publication) = profiles;
    let status = driver.supervisor().host().map_err(debug)?.request_status(request).map_err(debug)?;
    let FileRequestDisposition::Admitted { attempt, stage: ActionState::Reviewing } = status.disposition else {
        return Ok(()); // The ORIGINAL gateway/ledger supplies denied/nonadmitted outcomes.
    };
    // A separate stop-only endpoint is available before original capture or helper
    // launch. Exact retries never enter execute and cannot create this service.
    let mut control = control::Control::new(config, request, peers)?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(()); }
    let publication = match publication {
        Some(profile) => {
            // Freeze the original recipe and actual producer image BEFORE any
            // helper answer. The prepared value retains readers, not evidence.
            let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
            Some(profile.prepare(&mut host, attempt)?)
        }
        None => None,
    };
    let now = time(); deadline.check(now)?;
    let window = ReviewWindow { commit_by: plus(now, config.timing.commit_ms)?, reveal_by: plus(now, config.timing.reveal_ms)? };
    if window.reveal_by >= deadline.logical { return Err("insufficient original action lifetime for configured review".into()); }
    let input_revision = driver.supervisor().host().map_err(debug)?.input_revision(attempt).map_err(debug)?;
    let launch = FileReviewLaunch { request, round: request, window, expected_input_revision: input_revision,
        workers: std::mem::take(&mut config.programs), limits: HelperLimits::default() };
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(()); }
    driver.start_file_process_review(&mut config.source, launch, &mut *time).map_err(debug)?;
    loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(()); }
        let report = driver.step_from_file(&mut config.source, &mut *time, None);
        let event = report.result.map_err(debug)?;
        match event {
            FileDriverEvent::Workers { .. } => pause(config.timing.poll_ms),
            FileDriverEvent::ReviewApplied { .. } if matches!(driver.phase(), FileDriverPhase::AwaitingDispatch { .. }) => break,
            FileDriverEvent::ReviewApplied { .. } | FileDriverEvent::ReviewRejected { .. }
                | FileDriverEvent::Stopped { .. } => {
                cancel_unspent(driver, request)?;
                return Ok(());
            }
            FileDriverEvent::WorkersFailed { failure, .. } => return Err(debug(failure)),
            _ => return Err(format!("unexpected original driver review event: {event:?}")),
        }
    }
    let approval = human_review(driver, reviewer, config, request, deadline, (peers, &mut control), time)?;
    let Some(approval) = approval else { cancel_unspent(driver, request)?; return Ok(()); };
    loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(()); }
        let event = match &publication {
            Some(profile) => profile.step(driver, &mut config.source, time, Some(&approval))?,
            None => driver.step_from_file(&mut config.source, &mut *time, Some(&approval)).result.map_err(debug)?,
        };
        match event {
            FileDriverEvent::Dispatched { .. } | FileDriverEvent::PublicationChecked { .. } => {}
            FileDriverEvent::PublicationUnknown { error, .. } => return Err(debug(error)),
            FileDriverEvent::Reconciled { .. } | FileDriverEvent::Stopped { .. } => return Ok(()),
            _ => return Err(format!("unexpected original driver publication event: {event:?}")),
        }
    }
}

fn human_review<F>(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer, config: &mut Config,
    request: u64, deadline: &Deadline, channels: (Option<&PeerProfile>, &mut control::Control),
    time: &mut F) -> Result<Option<FileHumanPermit>, String>
where F: FnMut() -> ElapsedTick {
    let (peers, control) = channels;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(None); }
    let path = peers.map_or_else(|| config.socket(request), |profile| profile.socket(request));
    let mut admission = Admission::new(peers)?;
    let socket = BoundSocket::bind(&path, peers)?;
    if peers.is_none() { eprintln!("Reviewer peer credentials are unchecked: legacy namespace-isolated transport"); }
    eprintln!("Independent reviewer endpoint ready: {path:?}");
    let stream = loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(None); }
        match socket.listener.accept() {
            Ok((stream, _)) => match admission.admit(stream)? {
                Some(stream) => break stream,
                None => pause(config.timing.poll_ms),
            },
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => pause(config.timing.poll_ms),
            Err(e) => return Err(debug(e)),
        }
    };
    // Peer acceptance precedes source capture, human-request creation and offer
    // construction. Rejected peers cannot burn or influence the pending request.
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(None); }
    let expires = plus(time(), config.profile.human.max_validity_ticks)?.min(deadline.logical);
    let request = driver.request_human_approval_from_file(&mut config.source, request, expires, &mut *time)
        .result.map_err(debug)?;
    let nonce = nonce()?;
    let mut connection = {
        let host = driver.supervisor().host().map_err(debug)?;
        stream.into_connection(&host, reviewer, request, nonce)?
    };
    let application = loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(None); }
        let progress = {
            let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
            connection.step(&mut host, reviewer, &mut *time).map_err(debug)?
        };
        match progress {
            ReviewerProgress::Applied(application) => break application,
            ReviewerProgress::Complete => return Err("reviewer completed without an application".into()),
            ReviewerProgress::Progress | ReviewerProgress::Blocked => pause(config.timing.poll_ms),
        }
    };
    // The human transition already committed. Receipt loss cannot rerun approval,
    // erase its key, or pretend it was rejected. Bound only the remaining delivery
    // of this historical receipt; the next ORIGINAL dispatch rereads the source.
    let drain_started = Instant::now();
    while connection.phase() != ReviewerPhase::Complete
        && drain_started.elapsed() < Duration::from_millis(config.timing.cleanup_ms)
    {
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(None); }
        let result = {
            let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
            connection.step(&mut host, reviewer, &mut *time)
        };
        match result {
            Ok(ReviewerProgress::Complete) => break,
            Ok(ReviewerProgress::Applied(_)) => return Err("duplicate reviewer application".into()),
            Ok(_) => pause(config.timing.poll_ms),
            Err(error) => { eprintln!("Committed review receipt was not delivered: {error:?}"); break; }
        }
    }
    if application.receipt.decision != ReviewDecision::Approve && application.approval.is_some() {
        return Err("unexpected key for a restrictive reviewer decision".into());
    }
    Ok(application.approval)
}
fn cancel_unspent(driver: &mut FileSupervisedDriver, request: u64) -> Result<(), String> {
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    let status = host.request_status(request).map_err(debug)?;
    if matches!(status.disposition, FileRequestDisposition::Admitted {
        stage: ActionState::Proposed | ActionState::Prepared | ActionState::Reviewing | ActionState::Authorized, ..
    }) {
        let revision = host.revision(); host.cancel_request(revision, request).map_err(debug)?;
    }
    Ok(())
}
fn stop<F>(driver: &mut FileSupervisedDriver, operation: u64, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    let request = {
        let host = driver.supervisor().host().map_err(debug)?;
        let state = host.inspect();
        match state.stop {
            Some(receipt) => receipt.request(),
            None => StopRequest { operation, expected_control_sequence: state.control.sequence,
                expected_authority_epoch: state.control.ledger.epoch },
        }
    };
    let result = driver.stop_with_recovery_reserve(request, time).map_err(debug)?;
    result.drain.map_err(debug)?;
    Ok(())
}
fn cleanup(driver: FileSupervisedDriver, limit_ms: u64, poll_ms: u64) -> usize {
    let mut released = driver.release();
    let start = Instant::now();
    if let Some(children) = &mut released.children {
        children.request_stop_all();
        while !children.all_reaped() && start.elapsed() < Duration::from_millis(limit_ms) {
            children.reap(); pause(poll_ms);
        }
        let remaining = children.statuses().values().filter(|status| status.exit.is_none()).count();
        if remaining != 0 { eprintln!("Direct children not confirmed reaped at shutdown: {remaining}"); }
        remaining
    } else { 0 }
}
fn nonce() -> Result<[u8; 32], String> {
    let mut bytes = [0; 32]; File::open("/dev/urandom").map_err(debug)?.read_exact(&mut bytes).map_err(debug)?;
    if bytes == [0; 32] { return Err("invalid zero review session".into()); }
    Ok(bytes)
}

struct BoundSocket { listener: UnixListener, path: PathBuf, device: u64, inode: u64 }
impl BoundSocket {
    fn bind(path: &Path, peers: Option<&PeerProfile>) -> Result<Self, String> {
        if let Some(profile) = peers { profile.check_directory()?; }
        // bind is exclusive: no existing path, stale socket or symlink is removed
        // to make a new service start. Retain identity before fallible setup so
        // cleanup can remove only the socket this invocation actually created.
        let listener = UnixListener::bind(path).map_err(debug)?;
        let meta = fs::symlink_metadata(path).map_err(debug)?;
        let socket = Self { listener, path: path.to_owned(), device: meta.dev(), inode: meta.ino() };
        socket.listener.set_nonblocking(true).map_err(debug)?;
        if let Some(profile) = peers { profile.secure_socket(path)?; }
        Ok(socket)
    }
}
impl Drop for BoundSocket {
    fn drop(&mut self) {
        if let Ok(meta) = fs::symlink_metadata(&self.path) {
            if meta.file_type().is_socket() && meta.dev() == self.device && meta.ino() == self.inode {
                if let Err(error) = fs::remove_file(&self.path) { eprintln!("Reviewer socket cleanup: {error:?}"); }
            }
        }
    }
}
