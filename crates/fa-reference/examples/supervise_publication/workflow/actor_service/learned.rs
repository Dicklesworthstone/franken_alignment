//! Imported learned K/V generation, original native sidecar review and two keys.
//! This first command publishes one complete message; all numerical and effect
//! transitions belong to the existing owners, including restart and cancellation.
#[path = "learned/fields.rs"] mod fields;
#[path = "learned/fit.rs"] mod fit;
#[path = "learned/monitor.rs"] mod monitor;
#[path = "learned/native.rs"] mod native;
#[path = "learned/recipe.rs"] mod recipe;
#[path = "learned/recovery.rs"] mod recovery;
#[path = "learned/review.rs"] mod review;
use recipe::Loaded;
use super::super::{Config, Deadline, Duration, ElapsedTick, FileOversight, FileHumanReviewer,
    FileDriverEvent, FileDriverPhase, FileSupervisedDriver, Instant, PeerProfile,
    RecoveryReserve, RunResult, LiveControl, cancel_unspent, cleanup, clock,
    control::Control, debug, human_review, plus, stop};
use fa_reference::action::ActionState;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationStatus, GenerationStop,
};
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, requests::{FileRequestDisposition,
        actor::{FileLearnedTextStreamActorPort, LearnedTextRelease}},
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorOutcome, Knowledge},
    actor_wire::{ActorWire, Command, encode_command},
    evidence_source::EvidenceSnapshot,
};
use fa_reference::Error;
use std::{io::Write, path::Path, rc::Rc};

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE [--continue-generation] | create-learned-generated CONFIG LEARNED_RECIPE --resume";

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    if !matches!(args.len(), 4 | 5) || args[0] != "create-learned-generated"
        || credibility.is_some() { return Err(USAGE.into()); }
    let continuing = args.len() == 5;
    if continuing && (args[4] != "--continue-generation" || args[3].starts_with("--")) {
        return Err(USAGE.into());
    }
    if args[3].starts_with("--") && args[3] != "--resume" { return Err(USAGE.into()); }
    // Closed option, fit, probe and real model admission precede durable startup.
    let config = Config::read(Path::new(&args[1]))?;
    if args[3] == "--resume" {
        let loaded = recipe::load(Path::new(&args[2]), &config, false)?;
        return emit(recovery::resume(config, loaded, clock)?, &mut std::io::stdout().lock());
    }
    let peers = PeerProfile::read(Path::new(&args[3]))?;
    peers.check_host(&config)?;
    let loaded = recipe::load(Path::new(&args[2]), &config, true)?;
    emit(run(config, loaded, &peers, continuing, clock)?, &mut std::io::stdout().lock())
}

// Only the original restricted actor response reaches stdout. A failed output
// never causes a second effect, another native review or a new human approval.
fn emit(result: RunResult, output: &mut impl Write) -> Result<(), String> {
    output.write_all(&result.response.encode()).and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush()).map_err(|error|
            format!("original response output failed: {error}; no effect retry was sent"))?;
    if let Some(error) = result.failure { return Err(error); }
    if result.cleanup_pending != 0 {
        return Err(format!("{} direct helper children lack reaping confirmation", result.cleanup_pending));
    }
    if !matches!(result.response.result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })) {
        return Err("learned publication is not confirmed; inspect the original response".into());
    }
    Ok(())
}

fn run<F>(mut config: Config, loaded: Loaded, peers: &PeerProfile, continuing: bool, mut time: F)
    -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    peers.check_host(&config)?;
    let start = time();
    let lifetime = loaded.ttl_ms.min(config.timing.runtime_ms);
    let deadline = Deadline { logical: plus(start, lifetime)?, started: Instant::now(),
        wall: Duration::from_millis(lifetime) };
    deadline.check(start)?;
    // Claim the real authenticated stop endpoint BEFORE store creation. The
    // same bounded service survives numerical generation, review and dispatch.
    let mut control = Control::new(&config, loaded.request, Some(peers))?;
    let (host, reviewer) = if continuing { recovery::open(&config, &loaded)? }
        else { recovery::create(&config, &loaded)? };
    if continuing {
        match host.request_status(loaded.request) {
            Err(JournalError::Contract(Error::Missing)) => {}
            Ok(_) => return Err("learned request already exists; use --resume for its original receipt".into()),
            Err(error) => return Err(debug(error)),
        }
        host.check_credibility().map_err(debug)?;
    }
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let prepared = (|| {
        if !generate(&mut driver, &reviewer, &mut config, &mut control, &deadline, &mut time)? {
            return Ok(false);
        }
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut time)?;
        let status = driver.supervisor().host().map_err(debug)?.request_status(loaded.request).map_err(debug)?;
        Ok(matches!(status.disposition, FileRequestDisposition::Admitted {
            stage: ActionState::Reviewing, .. }))
    })();
    let mut failure = None;
    let mut history = None;
    match prepared {
        Err(error) => failure = Some(error),
        Ok(false) => {}
        Ok(true) => {
            let reviewed = review::run(driver, &mut config, &loaded, &reviewer,
                &mut control, &deadline, &mut time);
            driver = reviewed.driver;
            history = reviewed.history;
            failure = reviewed.failure;
            if failure.is_none() && !reviewed.stop_requested {
                let result = (|| {
                    if !matches!(driver.phase(), FileDriverPhase::AwaitingDispatch { .. }) {
                        cancel_unspent(&mut driver, loaded.request)?;
                        return Ok(());
                    }
                    let mut no_ingress = |_: &mut FileSupervisedDriver| Ok(false);
                    let mut live = LiveControl { stop: control, ingress: &mut no_ingress };
                    let approval = human_review(&mut driver, &reviewer, &mut config,
                        loaded.request, &deadline, (Some(peers), &mut live), &mut time)?;
                    let Some(approval) = approval else {
                        cancel_unspent(&mut driver, loaded.request)?; return Ok(());
                    };
                    loop {
                        deadline.check(time())?;
                        if live.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? { return Ok(()); }
                        let event = driver.step_computed_from_policy_file(&mut config.source,
                            &mut time, Some(&approval)).result.map_err(debug)?;
                        match event {
                            FileDriverEvent::Dispatched { .. } | FileDriverEvent::PublicationChecked { .. } => {}
                            FileDriverEvent::Reconciled { .. } | FileDriverEvent::Stopped { .. } => return Ok(()),
                            FileDriverEvent::PublicationUnknown { error, .. } => return Err(debug(error)),
                            _ => return Err(format!("unexpected original learned publication event: {event:?}")),
                        }
                    }
                })();
                failure = result.err();
            }
        }
    }
    if let Some(error) = failure.take() {
        failure = Some(match stop(&mut driver, loaded.request, &mut time) {
            Ok(()) => error, Err(stopping) => format!("{error}; stop/drain: {stopping}"),
        });
    }
    let response = wire.exchange(&encode_command(&Command::Poll { request: loaded.request }).map_err(debug)?);
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    // Retain original numerical history until the authority has been drained or
    // released. Its counters are evidence of work, never either publication key.
    drop(history);
    Ok(RunResult { response, failure, cleanup_pending })
}

fn generate<F>(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer, config: &mut Config,
    control: &mut Control, deadline: &Deadline, time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    refresh(driver, config, deadline, time)?;
    {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        let current = host.learned_generation_inspection().map_err(debug)?;
        if current.paused {
            // Exact recovered KV, sampler, pending intent and lifetime work are
            // retained. Resume changes neither numerical budget nor effect epoch.
            let revision = host.revision();
            host.resume_learned_generation(revision, current.numerical.actor_revision,
                current.numerical.position).map_err(debug)?;
        }
    }
    loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
        let current = driver.supervisor().host().map_err(debug)?
            .learned_generation_inspection().map_err(debug)?;
        if !current.numerical.status.is_active() {
            return match current.numerical.status {
                GenerationStatus::Finished(GenerationStop::StopToken(_)) => Ok(true),
                _ => Err(format!("learned generation did not finish at a quiet monitored stop: {:?}",
                    current.numerical.status)),
            };
        }
        // Read-start and post-read clocks precede EVERY original numerical
        // operation. A retained write-ahead intent completes once, never resets.
        refresh(driver, config, deadline, time)?;
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        let revision = host.revision();
        host.advance_learned_generation(revision, current.numerical.actor_revision,
            current.numerical.position).map_err(debug)?.map_err(debug)?;
    }
}

fn submit<F>(driver: &mut FileSupervisedDriver,
    wire: &mut ActorWire<FileLearnedTextStreamActorPort>, config: &mut Config, loaded: &Loaded,
    deadline: &Deadline, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    // A closed source-only envelope: the actor supplies no output bytes, logits,
    // prompt edits, helper inputs, automatic key or human approval.
    let proposal = FileLearnedTextStreamActorPort::encode_release(loaded.request,
        LearnedTextRelease::Message, deadline.logical).map_err(debug)?;
    let document = encode_command(&Command::Submit { request: loaded.request, proposal }).map_err(debug)?;
    deadline.check(time())?;
    let response = driver.supervisor_mut().exchange_learned_text_stream_actor_from_policy_file(
        wire, &document, &mut config.source, &mut *time).map_err(debug)?.response;
    if response.result.is_err() { return Err("original learned source gateway refused publication".into()); }
    Ok(())
}

fn refresh<F>(driver: &mut FileSupervisedDriver, config: &mut Config, deadline: &Deadline,
    time: &mut F) -> Result<Rc<EvidenceSnapshot>, String>
where F: FnMut() -> ElapsedTick {
    let before = time(); deadline.check(before)?;
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    let revision = host.revision();
    let captured = host.refresh_file_source(revision, &mut config.source, before).map_err(debug)?;
    let after = time(); deadline.check(after)?;
    let revision = host.revision();
    host.observe_time(revision, after).map_err(debug)?;
    // Original read-start freshness must still hold AFTER parsing/persistence.
    // Clock acknowledgement alone does not validate the policy source's lease.
    let current = host.capture_file_policy_state().map_err(debug)?;
    if current.snapshot() != captured.snapshot() { return Err(debug(Error::Binding)); }
    Ok(captured)
}

#[cfg(test)]
#[path = "learned/tests.rs"] mod tests;
