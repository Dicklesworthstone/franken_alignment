//! Imported learned K/V generation, original native sidecar review and two keys.
//! Complete messages within one finite generation and explicit stream finish;
//! all numerical and effect transitions belong to the existing original owners.
#[path = "learned/fields.rs"] mod fields;
#[path = "learned/anchors.rs"] mod anchors;
#[path = "learned/audit.rs"] mod audit;
#[path = "learned/fit.rs"] mod fit;
#[path = "learned/monitor.rs"] mod monitor;
#[path = "learned/native.rs"] mod native;
#[path = "learned/forecast.rs"] mod forecast;
#[path = "learned/prediction.rs"] mod prediction;
#[path = "learned/generation.rs"] mod generation;
#[path = "learned/recipe.rs"] mod recipe;
#[path = "learned/recovery.rs"] mod recovery;
#[path = "learned/review.rs"] mod review;
#[path = "learned/process.rs"] mod process;
#[path = "learned/ingress.rs"] mod ingress;
#[path = "learned/finish.rs"] mod finish;
#[path = "learned/continuation.rs"] mod continuation;
#[path = "learned/qualification.rs"] mod qualification;
#[path = "learned/identity.rs"] mod identity;
#[path = "learned/checked.rs"] mod checked;
#[path = "learned/credential.rs"] mod credential;
#[path = "learned/credential_lifecycle.rs"] mod credential_lifecycle;
#[path = "learned/stopped.rs"] mod stopped;
#[path = "learned/topology.rs"] mod topology;
#[path = "learned/sequence.rs"] mod sequence;
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

const USAGE: &str = "create-learned-generated CONFIG LEARNED_RECIPE REVIEWER_PROFILE [--continue-generation | --after-message PREVIOUS_REQUEST | --finish-after-message PREVIOUS_REQUEST] [--actor-profile ACTOR_PROFILE] [--credibility-activation CAPSULE] [--topology-profile PATH] [--history-anchor INPUT] [--retain-history-anchor NEW_OUTPUT] | create-learned-generated CONFIG LEARNED_RECIPE --resume [--finish-after-message PREVIOUS_REQUEST] [--topology-profile PATH] [--history-anchor INPUT] [--retain-history-anchor NEW_OUTPUT] | create-learned-generated CONFIG LEARNED_RECIPE (--recover-stop | --stop-admission | --drain-stopped) OPERATION [--topology-profile PATH] [--history-anchor INPUT] [--retain-history-anchor OUTPUT] [--retain-topology-checkpoint NEW_OUTPUT] [--recover-terminal-checkpoint] | create-learned-generated CONFIG LEARNED_RECIPE (--rotate-credential OPERATION EXPECTED_GENERATION NEXT_GENERATION | --revoke-credential OPERATION EXPECTED_GENERATION) --history-anchor INPUT --retain-history-anchor NEW_OUTPUT [--topology-profile PATH --retain-topology-checkpoint NEW_OUTPUT [--recover-credential-checkpoint]] | create-learned-generated CONFIG LEARNED_RECIPE --audit-review ROUND HISTORY_ANCHOR | create-learned-generated CONFIG LEARNED_RECIPE --resume (--receipts-only | --reconcile-receipts) [--finish-after-message PREVIOUS_REQUEST] [--topology-profile PATH] [--history-anchor INPUT]";

pub(super) fn sequence_command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    sequence::command(args, credibility)
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    command_with_clock(args, credibility, clock)
}

// The command and its admission tests use the same complete parser, artifact
// selection and workflow; a test clock supplies no publication or reviewer key.
fn command_with_clock<F>(args: &[String], credibility: Option<&Path>, time: F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    if args.iter().any(|arg| credential_lifecycle::is_option(arg)) {
        // Offline credential maintenance owns its complete closed grammar. In
        // particular no earlier option parser can silently discard actor,
        // topology, receipt or Stop work before admission.
        return credential_lifecycle::command(args, credibility);
    }
    if args.iter().any(|arg| matches!(arg.as_str(), "--receipts-only" | "--reconcile-receipts")) {
        return recovery::readonly::command(args, credibility, &mut std::io::stdout().lock());
    }
    if args.iter().any(|arg| arg == "--audit-review") {
        if credibility.is_some() { return Err("historical audit cannot activate credibility".into()); }
        return audit::command(args, &mut std::io::stdout().lock());
    }
    let (args, actor_path) = ingress::take_option(args)?;
    let (args, anchors) = anchors::take_options(&args)?;
    if args.iter().any(|arg| stopped::is_option(arg)) {
        // Explicit offline Stop never enters live continuation, consumes a
        // credibility capsule or claims a reviewer/stop transport endpoint.
        return stopped::command(&args, anchors);
    }
    let (args, topology_path) = topology::take_option(&args)?;
    let (args, after_message) = continuation::take_option(&args)?;
    let (args, finishing) = finish::take_option(args)?;
    if !matches!(args.len(), 4 | 5) || args[0] != "create-learned-generated"
        || (after_message.is_some() && finishing.is_some()) { return Err(USAGE.into()); }
    let continuing = args.len() == 5;
    if continuing && (args[4] != "--continue-generation" || args[3].starts_with("--")) {
        return Err(USAGE.into());
    }
    if args[3].starts_with("--") && args[3] != "--resume" { return Err(USAGE.into()); }
    let live_continuation = continuing || after_message.is_some() || finishing.is_some();
    if credibility.is_some() && !live_continuation && args[3] != "--resume" {
        return Err(USAGE.into());
    }
    // Closed option, fit, probe and real model admission precede durable startup.
    let config = Config::read(Path::new(&args[1]))?;
    if args[3] == "--resume" {
        // Receipt recovery never reads a current helper-evidence capsule. An
        // optional capsule cannot turn historical outcome lookup into live work.
        let mut loaded = recipe::load(Path::new(&args[2]), &config, false)?;
        anchors.admit(&config, &mut loaded, true)?;
        topology::admit(&config, &mut loaded, topology_path.as_deref(), topology::Mode::Historical)?;
        let result = match finishing {
            Some(after) => recovery::resume_finish(config, loaded, after, time)?,
            None => recovery::resume(config, loaded, time)?,
        };
        return emit(result, &mut std::io::stdout().lock());
    }
    let peers = PeerProfile::read(Path::new(&args[3]))?;
    peers.check_host(&config)?;
    let actor = actor_path.map(super::Profile::read).transpose()?;
    if let Some(actor) = actor.as_ref() { actor.check_host(&config)?; }
    let mut loaded = recipe::load(Path::new(&args[2]), &config, true)?;
    anchors.admit(&config, &mut loaded, live_continuation)?;
    topology::admit(&config, &mut loaded, topology_path.as_deref(),
        if live_continuation { topology::Mode::Live } else { topology::Mode::Create })?;
    let result = match actor.as_ref() {
        Some(actor) => run_with_actor(config, loaded, &peers, live_continuation,
            finishing, after_message, credibility, Some(actor), time)?,
        None => run_qualified(config, loaded, &peers, live_continuation,
            finishing, after_message, credibility, time)?,
    };
    emit(result, &mut std::io::stdout().lock())
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

#[cfg(test)]
fn run<F>(config: Config, loaded: Loaded, peers: &PeerProfile, continuing: bool, time: F)
    -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    run_selected(config, loaded, peers, continuing, None, time)
}

#[cfg(test)]
fn run_selected<F>(config: Config, loaded: Loaded, peers: &PeerProfile, continuing: bool,
    finishing: Option<u64>, time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    run_selected_message(config, loaded, peers, continuing, finishing, None, time)
}

#[cfg(test)]
fn run_selected_message<F>(config: Config, loaded: Loaded, peers: &PeerProfile, continuing: bool,
    finishing: Option<u64>, after_message: Option<u64>, time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    run_qualified(config, loaded, peers, continuing, finishing, after_message, None, time)
}

fn run_qualified<F>(config: Config, loaded: Loaded, peers: &PeerProfile, continuing: bool,
    finishing: Option<u64>, after_message: Option<u64>, credibility: Option<&Path>,
    time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    run_with_actor(config, loaded, peers, continuing, finishing, after_message, credibility, None, time)
}

fn run_with_actor<F>(mut config: Config, loaded: Loaded, peers: &PeerProfile, continuing: bool,
    finishing: Option<u64>, after_message: Option<u64>, credibility: Option<&Path>,
    actor: Option<&super::Profile>, mut time: F) -> Result<RunResult, String>
where F: FnMut() -> ElapsedTick {
    if credibility.is_some() && !continuing {
        return Err("credibility activation requires explicit learned continuation or finish".into());
    }
    peers.check_host(&config)?;
    if let Some(actor) = actor { ingress::check(&config, &loaded, peers, actor)?; }
    if let Some(after) = finishing { finish::check_ids(loaded.request, after)?; }
    if let Some(after) = after_message {
        continuation::check_ids(loaded.request, after)?;
        if !loaded.generation.allows_text_stream_continuation() {
            return Err("learned continuation requires the explicitly admitted finite-message recipe".into());
        }
    }
    checked::check_selection(&config, &loaded)?;
    let start = time();
    let lifetime = loaded.ttl_ms.min(config.timing.runtime_ms);
    let lifetime = actor.map_or(lifetime, |actor| lifetime.min(actor.runtime_ms));
    let mut deadline = Deadline { logical: plus(start, lifetime)?, started: Instant::now(),
        wall: Duration::from_millis(lifetime) };
    deadline.check(start)?;
    // Missing/mismatched independent secrets cannot leave a new owner or consume
    // a recovery fence. The same original deadline bounds these finite reads.
    let mut credential = credential::prepare(&loaded, continuing, &deadline, &mut time)?;
    // Claim the real authenticated stop endpoint BEFORE store creation/open.
    // The same service and deadline cover replay, generation, review and dispatch.
    let mut control = Control::new(&config, loaded.request, Some(peers))?;
    let bound_actor = actor.map(ingress::Bound::bind).transpose()?;
    let topology::Started { custody: (host, reviewer, observer, identity_observer), source: topology_source,
        output: topology_output } = topology::start(&config, &loaded, continuing,
            &mut control, &deadline, &mut time)?;
    let mut prediction = observer.map(|observer| prediction::Session::new(loaded.request, observer));
    let mut identity = identity_observer.map(identity::Session::new);
    prediction::check(&host, prediction.as_ref())?;
    let message_continuation = request_preflight(&host, &loaded, continuing,
        finishing, after_message, credibility)?;
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().map_err(debug)?;
    let release = if finishing.is_some() { LearnedTextRelease::Finish } else { LearnedTextRelease::Message };
    let mut intake = bound_actor.map(|bound| bound.attach(port.clone(), release, deadline.logical)).transpose()?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    topology::attach(&mut driver, topology_source)?;
    let mut wire = ActorWire::new(port);
    let RequestCycle { driver, response, failure, history, process } = execute_request(
        driver, &mut config, &loaded, RequestServices { peers, reviewer: &reviewer,
            prediction: &mut prediction, identity: &mut identity, credential: &mut credential,
            intake: &mut intake, wire: &mut wire,
            finishing, message_continuation, credibility, stop_operation: loaded.request },
        &mut control, &mut deadline, &mut time);
    // The anchor exporter preserves incoming errors while still trying its own
    // output; retain source custody first so either output can survive the other.
    let failure = topology::retain(topology_output.as_ref(), &driver, failure);
    let failure = anchors::retain(&loaded, &driver, failure);
    let cleanup_started = Instant::now();
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    let remaining = config.timing.cleanup_ms.saturating_sub(
        u64::try_from(cleanup_started.elapsed().as_millis()).unwrap_or(u64::MAX));
    let cleanup_pending = cleanup_pending.saturating_add(process.map_or(0,
        |history| history.cleanup(remaining, config.timing.poll_ms)));
    // Retain original numerical history until the authority has been drained or
    // released. Its counters are evidence of work, never either publication key.
    drop(history);
    Ok(RunResult { response, failure, cleanup_pending })
}

type MessageContinuation = fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedTextContinuation;

/// Original read-only guards before a live request starts. A sequence and the
/// single-request command select the same request/numerical predecessor.
fn request_preflight(host: &FileOversight, loaded: &Loaded, continuing: bool,
    finishing: Option<u64>, after_message: Option<u64>, credibility: Option<&Path>)
    -> Result<Option<MessageContinuation>, String>
{
    if continuing {
        match host.request_status(loaded.request) {
            Err(JournalError::Contract(Error::Missing)) => {}
            Ok(_) => return Err("learned request already exists; use --resume for its original receipt".into()),
            Err(error) => return Err(debug(error)),
        }
        // Preserve the existing omission refusal before live preparation. A
        // caller may retry with fresh qualification instead of terminally
        // stopping the history just because the option was omitted.
        if credibility.is_none() { host.check_credibility().map_err(debug)?; }
    }
    if let Some(after) = finishing { finish::check_fresh(host, loaded, after)?; }
    // This read-only command binds request IDs and the numerical predecessor,
    // which held-out activation preserves. Invalid predecessors remain a
    // pre-intake refusal; continuation repeats the native checks after activation.
    let message_continuation = after_message.map(|after|
        host.prepare_learned_text_stream_continuation(after, loaded.request).map_err(debug)).transpose()?;
    if continuing && finishing.is_none() && after_message.is_none()
        && host.current_learned_text_continuation().map_err(debug)?
            .is_some_and(|command| command.request() != loaded.request) {
        return Err("continued learned generation belongs to a different original request".into());
    }
    Ok(message_continuation)
}

struct RequestServices<'a> {
    peers: &'a PeerProfile,
    reviewer: &'a FileHumanReviewer,
    prediction: &'a mut Option<prediction::Session>,
    identity: &'a mut Option<identity::Session>,
    credential: &'a mut credential::Session,
    intake: &'a mut Option<ingress::Intake>,
    wire: &'a mut ActorWire<FileLearnedTextStreamActorPort>,
    finishing: Option<u64>,
    message_continuation: Option<MessageContinuation>,
    credibility: Option<&'a Path>,
    stop_operation: u64,
}

struct RequestCycle {
    driver: FileSupervisedDriver,
    response: fa_reference::action::consequence::oversight::actor_wire::WireResponse,
    failure: Option<String>,
    history: Option<fa_reference::action::consequence::delivery::persistent::observed::helpers::learned::native::sequence::FileNativeSidecarSequence>,
    process: Option<process::ProcessHistory>,
}

/// One original generation/review/two-key/publication cycle. Retain the same
/// authority, topology and numerical owner for either final cleanup or the next
/// independently selected request; this function never opens another store.
fn execute_request<F>(mut driver: FileSupervisedDriver, config: &mut Config,
    loaded: &Loaded, services: RequestServices<'_>, control: &mut Control,
    deadline: &mut Deadline, time: &mut F) -> RequestCycle
where F: FnMut() -> ElapsedTick {
    let RequestServices { peers, reviewer, prediction, identity, credential, intake, wire, finishing,
        message_continuation, credibility, stop_operation } = services;
    let mut publication = None;
    let prepared = (|| {
        // Bind only to this genuinely created/recovered owner. A resident run
        // retains this same permit; retries never reload secrets or renew it.
        credential.bind(loaded, &*driver.supervisor().host().map_err(debug)?)?;
        if !qualification::prepare(&mut driver, config, credibility,
            (&mut *control, reviewer, deadline), time)? { return Ok(false); }
        if driver.topology().is_some() {
            if control.checkpoint(&mut driver, reviewer, deadline, time)? { return Ok(false); }
            topology::certify(&mut driver, deadline, time)?;
        }
        let release = if let Some(after) = finishing {
            if !finish::prepare(&mut driver, reviewer, config, loaded, after,
                control, deadline, time)? { return Ok(false); }
            if let Some(prediction) = prediction.as_mut() {
                let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
                prediction.finish_request(&mut host)?;
            }
            if !identity::prepare(&mut driver, config, loaded, identity,
                (&mut *control, reviewer, deadline), time)? { return Ok(false); }
            LearnedTextRelease::Finish
        } else {
            if let Some(command) = message_continuation {
                if !continuation::prepare(&mut driver, reviewer, config, command,
                    control, deadline, time)? { return Ok(false); }
            }
            if !identity::prepare(&mut driver, config, loaded, identity,
                (&mut *control, reviewer, deadline), time)? { return Ok(false); }
            if !generate_selected(&mut driver, reviewer, config, control,
                deadline, time, prediction.as_mut())? {
                return Ok(false);
            }
            LearnedTextRelease::Message
        };
        if let Some(intake) = intake.as_mut() {
            if !intake.admit(&mut driver, config, reviewer, control, deadline, time)? {
                return Ok(false);
            }
            let accepted_deadline = intake.observe_original(&driver, wire, release, finishing)?;
            deadline.logical = deadline.logical.min(accepted_deadline);
            deadline.check(time())?;
        } else {
            match release {
                LearnedTextRelease::Message => submit(&mut driver, wire, config, loaded, deadline, time)?,
                LearnedTextRelease::Finish => submit_release(&mut driver, wire, config, loaded,
                    release, deadline, time)?,
            }
        }
        let status = driver.supervisor().host().map_err(debug)?.request_status(loaded.request).map_err(debug)?;
        let FileRequestDisposition::Admitted { attempt, stage: ActionState::Reviewing, .. } = status.disposition
            else { return Ok(false); };
        if let Some(selected) = &loaded.publication {
            if control.checkpoint(&mut driver, reviewer, deadline, time)? { return Ok(false); }
            deadline.check(time())?;
            let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
            // Capture the independently selected ORIGINAL image against this
            // actual Message or Finish before any native/process helper starts.
            publication = Some(selected.prepare_with_clock(&mut host, attempt, &mut *time)?);
            deadline.check(time())?;
        }
        Ok(true)
    })();
    let mut failure = None;
    let mut history = None;
    let mut process = None;
    match prepared {
        Err(error) => failure = Some(error),
        Ok(false) => {}
        Ok(true) => {
            let reviewed = review::run_serviced_checked_with_credential(driver, config, loaded, reviewer,
                control, deadline, time, intake, publication.as_ref(), credential.permit());
            driver = reviewed.driver;
            history = reviewed.history;
            process = reviewed.process;
            failure = reviewed.failure;
            if failure.is_none() && !reviewed.stop_requested {
                let result = (|| {
                    if !matches!(driver.phase(), FileDriverPhase::AwaitingDispatch { .. }) {
                        cancel_unspent(&mut driver, loaded.request)?;
                        return Ok(());
                    }
                    let mut actor_ingress = |driver: &mut FileSupervisedDriver|
                        ingress::observe(intake, driver.supervisor_mut());
                    let mut live = LiveControl { stop: control.share(), ingress: &mut actor_ingress };
                    let approval = human_review(&mut driver, reviewer, config,
                        loaded.request, deadline, (Some(peers), &mut live), time)?;
                    let Some(approval) = approval else {
                        cancel_unspent(&mut driver, loaded.request)?; return Ok(());
                    };
                    loop {
                        deadline.check(time())?;
                        if live.checkpoint(&mut driver, reviewer, deadline, time)? { return Ok(()); }
                        let event = match publication.as_mut() {
                            Some(publication) => publication.step_learned_or_wait_with_credential(&mut driver,
                                &mut config.source, time, Some(&approval), credential.permit())?,
                            None => Some(match credential.permit() {
                                Some(permit) => driver.step_computed_from_policy_file_with_credential(
                                    &mut config.source, &mut *time, Some(&approval), permit),
                                None => driver.step_computed_from_policy_file(&mut config.source,
                                    &mut *time, Some(&approval)),
                            }.result.map_err(debug)?),
                        };
                        let Some(event) = event else {
                            // This prepared request owns one finite retry budget
                            // across the whole loop. Yield without replacing its
                            // action, review, approval, reservation or deadline;
                            // Stop and actor cancellation are checked above.
                            super::super::pause(config.timing.poll_ms.clamp(1, 50));
                            continue;
                        };
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
        failure = Some(match stop(&mut driver, stop_operation, time) {
            Ok(()) => error, Err(stopping) => format!("{error}; stop/drain: {stopping}"),
        });
    }
    // Receipt delivery cannot restart generation, review, reconciliation or an
    // effect. Preserve transport failure alongside the original actor outcome.
    if let Some(intake) = intake.as_mut() {
        if let Err(error) = intake.finish(driver.supervisor_mut(), deadline, time) {
            let error = format!("learned actor response service failed: {error}; no effect retry was sent");
            failure = Some(match failure { Some(original) => format!("{original}; {error}"), None => error });
        }
    }
    let response = wire.exchange(&encode_command(&Command::Poll { request: loaded.request })
        .expect("validated request ID fits the bounded original Poll frame"));
    RequestCycle { driver, response, failure, history, process }
}

#[cfg(test)]
fn generate<F>(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer, config: &mut Config,
    control: &mut Control, deadline: &Deadline, time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    generate_selected(driver, reviewer, config, control, deadline, time, None)
}

fn generate_selected<F>(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer,
    config: &mut Config, control: &mut Control, deadline: &Deadline, time: &mut F,
    mut prediction: Option<&mut prediction::Session>) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    prediction::check(&*driver.supervisor().host().map_err(debug)?, prediction.as_deref())?;
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
    let mut numerical: Option<generation::Session> = None;
    loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
        let current = driver.supervisor().host().map_err(debug)?
            .learned_generation_inspection().map_err(debug)?;
        if !current.numerical.status.is_active() {
            if let Some(prediction) = prediction.as_deref() {
                prediction.complete_message(&*driver.supervisor().host().map_err(debug)?)?;
            }
            return match current.numerical.status {
                GenerationStatus::Finished(GenerationStop::StopToken(_)) => {
                    if let Some(numerical) = numerical.take() {
                        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
                        numerical.retain_for_control(&mut host)?;
                    }
                    Ok(true)
                }
                _ => Err(format!("learned generation did not finish at a quiet monitored stop: {:?}",
                    current.numerical.status)),
            };
        }
        // Read-start and post-read clocks precede EVERY original numerical
        // operation. A retained write-ahead intent completes once, never resets.
        refresh(driver, config, deadline, time)?;
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        prediction::check(&host, prediction.as_deref())?;
        if current.numerical.status == GenerationStatus::Generating {
            if let Some(prediction) = prediction.as_deref_mut() {
                // Prompt replay custody has no role in forecasted sampling.
                if let Some(numerical) = numerical.take() {
                    numerical.retain_for_control(&mut host)?;
                }
                if !prediction.has_message() {
                    prediction.begin_message(&mut host)?;
                    // Forecast acknowledgment precedes a NEW normal stop,
                    // clock and policy-source quantum before the first draw.
                    // Acquisition never lends its elapsed time to generation.
                    continue;
                }
                prediction.advance_quantum(&mut host)?;
                continue;
            }
        }
        numerical.get_or_insert_with(generation::Session::new).advance_quantum(&mut host)?;
    }
}

fn submit<F>(driver: &mut FileSupervisedDriver,
    wire: &mut ActorWire<FileLearnedTextStreamActorPort>, config: &mut Config, loaded: &Loaded,
    deadline: &Deadline, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    submit_release(driver, wire, config, loaded, LearnedTextRelease::Message, deadline, time)
}

fn submit_release<F>(driver: &mut FileSupervisedDriver,
    wire: &mut ActorWire<FileLearnedTextStreamActorPort>, config: &mut Config, loaded: &Loaded,
    release: LearnedTextRelease, deadline: &Deadline, time: &mut F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    // A closed source-only envelope: the actor supplies no output bytes, logits,
    // prompt edits, helper inputs, automatic key or human approval.
    let proposal = FileLearnedTextStreamActorPort::encode_release(loaded.request,
        release, deadline.logical).map_err(debug)?;
    let document = encode_command(&Command::Submit { request: loaded.request, proposal }).map_err(debug)?;
    deadline.check(time())?;
    let response = driver.supervisor_mut().exchange_learned_text_stream_actor_from_policy_file(
        wire, &document, &mut config.source, &mut *time).map_err(debug)?.response;
    if let Err(error) = response.result {
        return Err(format!("original learned source gateway refused publication: {error:?}"));
    }
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
    // Clock acknowledgment alone does not validate the policy source's lease.
    let current = host.capture_file_policy_state().map_err(debug)?;
    if current.snapshot() != captured.snapshot() { return Err(debug(Error::Binding)); }
    Ok(captured)
}

#[cfg(test)]
#[path = "learned/tests.rs"] mod tests;
