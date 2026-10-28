//! One authenticated service window over a sealed original learned receipt.
//! Replay owns the Store; transport receives only an immutable historical port.
use super::{
    Config, Cursor, Duration, Instant, Loaded, Path, anchors, custody, finish, recipe, topology,
};
use crate::workflow::actor_service::{
    ActorWire, BoundSocket, PeerSession, Profile, receipts::serve_session,
};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::FileLearnedRecoveryStatus,
    guarded::mediated::receipt::FileReceiptTopologyRecovery,
};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE --resume --serve-receipts ACTOR_PROFILE --history-anchor INPUT [--topology-profile PATH] [--finish-after-message PREVIOUS_REQUEST] [--retain-history-anchor NEW_OUTPUT [--retain-topology-checkpoint NEW_OUTPUT]] [--recover-receipt-checkpoint] (--receipts-only | --reconcile-receipts)";
const EXPIRED: &str = "learned receipt service startup deadline reached";
const ACKNOWLEDGED: &str = "learned receipt reconciliation acknowledged";

// The repair cursor owns only the one selected message/finish and the original
// checkpoint. It cannot be upgraded to general recovery or live actor admission.
enum ReceiptRecovery {
    Original(Cursor),
    Repair(FileReceiptTopologyRecovery),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super::super) enum Mode {
    Passive,
    Reconcile,
}

/// Counts describe transport, not publication success or current journal state.
#[derive(Debug)]
pub(in super::super::super) struct Report {
    pub frames: usize,
    pub candidates: u64,
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    let started = Instant::now();
    command_with(
        args,
        credibility,
        crate::workflow::clock,
        &mut || started.elapsed(),
        &mut crate::workflow::pause,
    )
    .map(|_| ())
}

// The real command and tests share complete option/file admission and one timer.
pub(in super::super::super) fn command_with<F>(
    args: &[String],
    credibility: Option<&Path>,
    clock: F,
    elapsed: &mut impl FnMut() -> Duration,
    wait: &mut impl FnMut(u64),
) -> Result<Report, String>
where
    F: FnOnce() -> ElapsedTick,
{
    let mode_at_end = matches!(args.last().map(String::as_str),
        Some("--receipts-only" | "--reconcile-receipts"));
    let (args, repair) = custody::take_repair_option(args).map_err(|_| USAGE.to_owned())?;
    if repair && !mode_at_end {
        return Err(USAGE.into());
    }
    let (args, source_output) = custody::take_option(&args)?;
    let (args, topology_path) = topology::take_option(&args)?;
    // Preserve the original fixed service prefix and mandatory input position.
    // Shared parsers may remove only independently admitted archive options.
    if credibility.is_some()
        || args.len() < 9
        || args[0] != "create-learned-generated"
        || args[3] != "--resume"
        || args[4] != "--serve-receipts"
        || args[6] != "--history-anchor"
        || [1, 2, 5, 7]
            .into_iter()
            .any(|i| args[i].is_empty() || args[i].starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mode = match args.last().map(String::as_str) {
        Some("--receipts-only") => Mode::Passive,
        Some("--reconcile-receipts") => Mode::Reconcile,
        _ => return Err(USAGE.into()),
    };
    if mode == Mode::Passive
        && (repair || source_output.is_some() || args.iter().any(|arg| arg == "--retain-history-anchor"))
    {
        return Err(USAGE.into());
    }
    custody::check_repair_options(&args, topology_path.as_deref(), source_output.as_deref(), repair)?;
    // Path-valued options precede mode removal, so a missing path cannot consume
    // a boolean flag or be repaired by removing it from a neighboring option.
    let (args, anchor_options) = anchors::take_options(&args)?;
    if !matches!(args.len(), 7 | 9) {
        return Err(USAGE.into());
    }
    let after = if args.len() == 9 {
        if args[6] != "--finish-after-message"
            || args[7].is_empty()
            || !args[7].bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(USAGE.into());
        }
        let after = args[7].parse::<u64>().map_err(|_| USAGE.to_owned())?;
        if after == 0 {
            return Err(USAGE.into());
        }
        Some(after)
    } else {
        None
    };
    let config = Config::read(Path::new(&args[1]))?;
    checkpoint(Duration::from_millis(config.timing.runtime_ms), elapsed)?;
    let actor = Profile::read(Path::new(&args[5]))?;
    let wall = Duration::from_millis(actor.runtime_ms.min(config.timing.runtime_ms));
    checkpoint(wall, elapsed)?;
    actor.check_host(&config)?;
    let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
    anchor_options.admit(&config, &mut loaded, true)?;
    topology::admit(
        &config,
        &mut loaded,
        topology_path.as_deref(),
        topology::Mode::Historical,
    )?;
    let outputs = custody::admit_repair(&config, &loaded, source_output.as_deref(), repair)?;
    let report = serve_with_custody(
        &config,
        &loaded,
        &actor,
        after,
        mode,
        outputs.as_ref(),
        clock,
        elapsed,
        wait,
    )?;
    eprintln!(
        "Learned historical receipt service ended: request={}, frames={}, candidates={}, reconcile_first={}; no live admission or cancellation was available",
        actor.request,
        report.frames,
        report.candidates,
        mode == Mode::Reconcile
    );
    Ok(report)
}

pub(in super::super::super) fn serve<F>(
    config: &Config,
    loaded: &Loaded,
    actor: &Profile,
    after: Option<u64>,
    mode: Mode,
    clock: F,
    elapsed: &mut impl FnMut() -> Duration,
    wait: &mut impl FnMut(u64),
) -> Result<Report, String>
where
    F: FnOnce() -> ElapsedTick,
{
    let outputs = match mode {
        Mode::Passive => {
            if loaded.anchor_output.is_some() {
                return Err("passive receipt service cannot export a successor anchor".into());
            }
            None
        }
        Mode::Reconcile => custody::admit(config, loaded, None)?,
    };
    serve_with_custody(
        config,
        loaded,
        actor,
        after,
        mode,
        outputs.as_ref(),
        clock,
        elapsed,
        wait,
    )
}

fn serve_with_custody<F>(
    config: &Config,
    loaded: &Loaded,
    actor: &Profile,
    after: Option<u64>,
    mode: Mode,
    outputs: Option<&custody::Outputs>,
    clock: F,
    elapsed: &mut impl FnMut() -> Duration,
    wait: &mut impl FnMut(u64),
) -> Result<Report, String>
where
    F: FnOnce() -> ElapsedTick,
{
    let wall = Duration::from_millis(actor.runtime_ms.min(config.timing.runtime_ms));
    checkpoint(wall, elapsed)?;
    if loaded.request == 0 || loaded.request != actor.request {
        return Err("learned receipt service request does not match the actor profile".into());
    }
    if loaded.recovery_anchor.is_none()
        || (loaded.anchor_output.is_some() && outputs.is_none())
        || (mode == Mode::Passive && outputs.is_some())
    {
        return Err("learned receipt service requires an independent input anchor and separately admitted reconciliation outputs".into());
    }
    if let Some(after) = after {
        finish::check_ids(loaded.request, after)?;
    }
    actor.check_host(config)?;
    if let Some(outputs) = outputs {
        // A create-new archive must not name the socket that this service is
        // about to bind. Check canonical parents before replay or settlement.
        outputs.check_service_socket(&actor.socket)?;
    }
    checkpoint(wall, elapsed)?;
    // The existing cursor chooses exact plain/raw/owned guards and checks the
    // independently supplied anchor, complete recipe and original terminal reserve.
    let mut run = if outputs.is_some_and(custody::Outputs::repair_requested) {
        ReceiptRecovery::Repair(super::super::selected::begin_receipt_checkpoint_repair(
            config, loaded, after,
        )?)
    } else {
        ReceiptRecovery::Original(Cursor::begin(config, loaded)?)
    };
    loop {
        checkpoint(wall, elapsed)?;
        let progress = match &run {
            ReceiptRecovery::Original(run) => run.progress(),
            ReceiptRecovery::Repair(run) => run.progress(),
        };
        match progress.status {
            FileLearnedRecoveryStatus::Replaying => {
                match &mut run {
                    ReceiptRecovery::Original(run) => run.advance(progress.replayed_events),
                    ReceiptRecovery::Repair(run) => run.advance(progress.replayed_events, 1),
                }.map_err(crate::config::debug)?;
            }
            FileLearnedRecoveryStatus::Ready => break,
            _ => return Err("original learned receipt replay is unavailable".into()),
        }
    }
    // Existence, kind and exact accepted finish predecessor precede socket
    // binding, clock and mutation. A missing selection cannot spend a fence.
    match &run {
        ReceiptRecovery::Original(run) => run.receipt(loaded.request, after).map(|_| ()),
        ReceiptRecovery::Repair(run) => run.check_request(),
    }.map_err(crate::config::debug)?;
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    checkpoint(wall, elapsed)?;
    let unconfirmed = |error| {
        format!(
            "learned receipt service finalization unconfirmed: {error:?}; no effect retry was sent"
        )
    };
    let (port, acknowledged) = match (mode, run) {
        (Mode::Passive, ReceiptRecovery::Original(run)) => (
            run.finish_receipt_port(loaded.request, after).map_err(unconfirmed)?,
            None,
        ),
        (Mode::Passive, ReceiptRecovery::Repair(_)) => {
            return Err("passive receipt service cannot repair checkpoint custody".into());
        }
        (Mode::Reconcile, ReceiptRecovery::Repair(run)) => {
            // A complete saved-cut triplet is only confirmed and re-exported;
            // the native cursor cannot read this clock or append another event.
            let (port, result) = run.finish_reconciled_receipt_port_with_history(clock)
                .map_err(unconfirmed)?;
            (port, Some(result))
        }
        (Mode::Reconcile, ReceiptRecovery::Original(run)) => {
            if outputs.is_some() {
                let result = match topology::history(loaded)? {
                    Some(history) => {
                        let saved = history.checkpoint().ok_or(
                            "receipt service custody requires an exact v2 source checkpoint",
                        )?;
                        run.finish_reconciled_receipt_port_with_checkpoint(
                            loaded.request,
                            after,
                            saved,
                            clock,
                        )
                    }
                    None => run.finish_reconciled_receipt_port_with_history(
                        loaded.request,
                        after,
                        clock,
                    ),
                }.map_err(unconfirmed)?;
                (result.0, Some(result.1))
            } else {
                (run.finish_reconciled_receipt_port(loaded.request, after, clock)
                    .map_err(unconfirmed)?, None)
            }
        }
    };
    if let Some(result) = acknowledged {
        // A fresh replacement or existing canonical triplet has ACKNOWLEDGED.
        // Both independent exports precede a late deadline or actor exchange.
        let outputs = outputs.ok_or("receipt service acknowledged without selected custody outputs")?;
        outputs.retain(config, loaded, &result).map_err(|error| format!(
            "{ACKNOWLEDGED} at revision {}; custody retention failed: {error}; no receipt was served and no effect retry was sent",
            result.history_anchor().revision()))?;
    }
    // The recovered Store lock, live generation state and roles are gone.
    // Caller configuration and immutable recipes may remain; neither reaches
    // transport. The port retains only the selected exact envelope and outcome.
    // A newer writer cannot make this saved observation current by polling it.
    let serving = (|| {
        checkpoint(wall, elapsed)?;
        let mut session = PeerSession::new(
            actor.actor,
            ActorWire::new(port),
            actor.channels(),
            actor.connections,
        )
        .map_err(crate::config::debug)?;
        let (frames, candidates) = serve_session(&mut session, actor, socket, wall, elapsed, wait)?;
        Ok(Report { frames, candidates })
    })();
    serving.map_err(|error: String| match mode {
        Mode::Passive => error,
        Mode::Reconcile => format!("{ACKNOWLEDGED}; service failed: {error}; inspect with --receipts-only; no effect retry was sent"),
    })
}

fn checkpoint(wall: Duration, elapsed: &mut impl FnMut() -> Duration) -> Result<(), String> {
    if elapsed() >= wall {
        Err(EXPIRED.into())
    } else {
        Ok(())
    }
}
