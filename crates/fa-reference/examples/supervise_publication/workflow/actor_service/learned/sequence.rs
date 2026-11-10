//! A finite resident sequence using the original learned publication owners.
//! The plan selects every request and actor before startup. Each successor still
//! needs its own original forecast, native/process congress and human permit.
#[path = "sequence/profile.rs"] mod profile;
#[path = "sequence/precheck.rs"] mod disclosure;
use super::{Config, Loaded, PeerProfile, Deadline, ElapsedTick, Duration, Instant,
    FileHumanReviewer, FileSupervisedDriver, LearnedTextRelease, ActorWire, ActorOutcome, Knowledge, RequestServices,
    RequestCycle, Control, MessageContinuation, debug, plus, clock, cleanup, stop,
    anchors, campaigns, checkpoint_on_generation, topology, prediction, identity, operator,
    ingress, process, request_preflight, execute_request};
use super::super::Profile;
use fa_reference::action::{ActionState, consequence::delivery::{EndpointOutcome,
    persistent::requests::FileRequestDisposition}};
use fa_reference::action::consequence::oversight::actor_wire::WireResponse;
use std::{collections::BTreeSet, io::Write, path::Path};

pub(super) const MAX_ENTRIES: usize = 16;
const USAGE: &str = "serve-learned-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE [--continue-generation | --after-message PREVIOUS_REQUEST] [--campaign-profile PATH] [--checkpoint-before-generation PROFILE] [--credibility-activation CAPSULE] [--topology-profile PATH] [--history-anchor INPUT] [--retain-history-anchor NEW_OUTPUT] [--operator-profile OPERATOR_PROFILE --tower-generation GENERATION]";

pub(super) struct Entry {
    pub loaded: Loaded,
    pub actor: Profile,
    pub release: LearnedTextRelease,
    pub precheck: Option<super::precheck::Settings>,
}

pub(super) struct SequenceResult {
    pub responses: Vec<WireResponse>,
    pub failure: Option<String>,
    pub cleanup_pending: usize,
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    command_with_clock(args, credibility, clock)
}

pub(super) fn command_with_clock<F>(args: &[String], credibility: Option<&Path>, time: F) -> Result<(), String>
where F: FnMut() -> ElapsedTick {
    // The operator pair owns the complete live grammar before another option
    // parser can erase an incompatible historical or maintenance selection.
    let (args, operator_path) = operator::take_option(args)?;
    let (args, checkpoint_path) = checkpoint_on_generation::take_option(&args)?;
    let (args, campaign_path) = campaigns::take_option(&args)?;
    let (args, anchors) = anchors::take_options(&args)?;
    let (args, topology_path) = topology::take_option(&args)?;
    let (continuing, after) = options(&args)?;
    if credibility.is_some() && !continuing {
        return Err("sequence credibility activation requires explicit live recovery".into());
    }
    if campaign_path.is_some() && continuing {
        return Err("--campaign-profile is only for new learned histories".into());
    }
    let config = if continuing {
        Config::read_learned_recovery(Path::new(&args[1]))?
    } else {
        Config::read_learned_generation(Path::new(&args[1]))?
    };
    let peers = PeerProfile::read(Path::new(&args[3]))?;
    let mut entries = profile::load(Path::new(&args[2]), &config)?;
    validate(&config, &entries, &peers)?;
    // First compare every independently supplied recovery inventory. A creation
    // selection cannot erase a different later requirement. Then install ONE
    // original first-image guard and carry its expected inventory through the
    // resident entries; only entry one holds the creation selection.
    campaigns::admit(&config, &mut entries[0].loaded, campaign_path.as_deref(), continuing)?;
    if let Some(selected) = entries[0].loaded.creation_campaigns {
        for entry in &mut entries[1..] {
            entry.loaded.expected_campaigns = Some(selected);
        }
    }
    entries[0].loaded.checkpoint_on_generation = checkpoint_path
        .map(checkpoint_on_generation::Selection::read).transpose()?;
    anchors.admit(&config, &mut entries[0].loaded, continuing)?;
    topology::admit(&config, &mut entries[0].loaded, topology_path.as_deref(),
        if continuing { topology::Mode::Live } else { topology::Mode::Create })?;
    let operator = operator_path.map(|(path, generation)| operator::Selection::read(&path, generation))
        .transpose()?;
    let result = run_with_operator(config, entries, &peers, continuing, after, credibility,
        operator.as_ref(), time)?;
    emit(result, &mut std::io::stdout().lock())
}

fn options(args: &[String]) -> Result<(bool, Option<u64>), String> {
    if !(4..=6).contains(&args.len()) || args[0] != "serve-learned-generated"
        || args[1..4].iter().any(|value| value.is_empty() || value.starts_with("--")) {
        return Err(USAGE.into());
    }
    match args.len() {
        4 => Ok((false, None)),
        5 if args[4] == "--continue-generation" => Ok((true, None)),
        6 if args[4] == "--after-message" => {
            if args[5].is_empty() || !args[5].bytes().all(|value| value.is_ascii_digit()) {
                return Err(USAGE.into());
            }
            let after = args[5].parse::<u64>().map_err(|_| USAGE.to_owned())?;
            if after == 0 { return Err(USAGE.into()); }
            Ok((true, Some(after)))
        }
        _ => Err(USAGE.into()),
    }
}

/// No role, time observation or store access occurs during plan admission.
/// Future entries cannot introduce another source custodian or recovery floor.
fn validate(config: &Config, entries: &[Entry], peers: &PeerProfile) -> Result<(), String> {
    if !(2..=MAX_ENTRIES).contains(&entries.len()) {
        return Err("a learned sequence requires 2..=16 independently selected entries".into());
    }
    peers.check_host(config)?;
    let original = &entries[0].loaded;
    let mut requests = BTreeSet::new();
    let mut objects = BTreeSet::new();
    let mut rounds = BTreeSet::new();
    let mut sockets = BTreeSet::new();
    let mut identity_checks = BTreeSet::new();
    let mut identity_sequence = 0;
    if original.generation.required_computed_identity_registration()
        .is_some_and(|registration| entries.len() > registration.policy.max_checks) {
        return Err("sequence exceeds the original identity check allowance".into());
    }
    for (index, entry) in entries.iter().enumerate() {
        let loaded = &entry.loaded;
        super::checked::check_selection(config, loaded)?;
        ingress::check(config, loaded, peers, &entry.actor)?;
        if !loaded.generation.allows_text_stream_continuation()
            || loaded.generation != original.generation || loaded.stream != original.stream {
            return Err("every sequence entry must retain the exact original finite generation and stream".into());
        }
        if loaded.expected_policy != original.expected_policy {
            return Err("every sequence entry must retain the independently selected effective policy".into());
        }
        if loaded.expected_campaigns != original.expected_campaigns {
            return Err("every sequence entry must retain the independently selected campaign guard".into());
        }
        if let Some(selected) = loaded.checkpoint_on_generation {
            selected.check(config, loaded)?;
        }
        let same_credential = match (&original.credential, &loaded.credential) {
            (None, None) => true,
            (Some(original), Some(selected)) => original.same_resident_selection(selected),
            _ => false,
        };
        if !same_credential {
            return Err("every sequence entry must retain the original credential policy, epoch and independent inputs".into());
        }
        if let Some(identity) = &loaded.identity {
            if !identity_checks.insert(identity.check) || identity.measurement_sequence <= identity_sequence {
                return Err("sequence identity checks must be distinct with increasing measurement sequences".into());
            }
            identity_sequence = identity.measurement_sequence;
        }
        if loaded.request == 0 || !requests.insert(loaded.request)
            || loaded.sidecar.object_id == 0 || !objects.insert(loaded.sidecar.object_id)
            || loaded.rounds.is_empty() || loaded.rounds.iter().any(|round| !rounds.insert(*round)) {
            return Err("sequence requests, sidecar objects and review rounds must be distinct and nonzero".into());
        }
        if loaded.rounds.contains(&0) || loaded.native.is_some() == loaded.process.is_some() {
            return Err("each sequence entry requires one original native or process review schedule".into());
        }
        if !sockets.insert(entry.actor.socket.clone()) {
            return Err("sequence actor socket paths must be distinct".into());
        }
        if matches!(entry.release, LearnedTextRelease::Finish)
            && (index == 0 || index + 1 != entries.len()) {
            return Err("sequence Finish is permitted only as the last entry after a Message".into());
        }
        if index != 0 && (loaded.recovery_anchor.is_some() || loaded.anchor_output.is_some()
            || loaded.topology.is_some() || loaded.floor.journal_revision != 0
            || loaded.floor.control_sequence != 0 || loaded.floor.authority_epoch != 0
            || loaded.creation_campaigns.is_some() || loaded.checkpoint_on_generation.is_some()) {
            return Err("only the first sequence entry selects recovery, topology, creation guards, checkpoint capture and retained outputs".into());
        }
        for other in entries {
            if entry.actor.socket == peers.socket(other.loaded.request)
                || entry.actor.socket == crate::workflow::control::socket_path(peers, other.loaded.request) {
                return Err("sequence actor, review and stop socket namespaces overlap".into());
            }
        }
    }
    disclosure::check(config, entries, peers)?;
    Ok(())
}

fn entry_deadline(global: &Deadline, entry: &Entry, now: ElapsedTick) -> Result<Deadline, String> {
    global.check(now)?;
    let lifetime = entry.loaded.ttl_ms.min(entry.actor.runtime_ms);
    let logical = global.logical.min(plus(now, lifetime)?);
    // Retain the run's original wall origin, while bounding this request even
    // when the logical clock stalls. No entry can extend the global deadline.
    let wall = global.wall.min(global.started.elapsed()
        .saturating_add(Duration::from_millis(lifetime)));
    let deadline = Deadline { logical, started: global.started, wall };
    deadline.check(now)?;
    Ok(deadline)
}

fn confirmed(driver: &FileSupervisedDriver, request: u64, response: &WireResponse)
    -> Result<bool, String>
{
    if !matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })) {
        return Ok(false);
    }
    let host = driver.supervisor().host().map_err(debug)?;
    if !matches!(host.request_status(request).map_err(debug)?.disposition,
        FileRequestDisposition::Admitted { stage: ActionState::Confirmed, .. })
        || !matches!(host.request_resolution(request).map_err(debug)?,
            Some(EndpointOutcome::Executed { .. })) {
        return Err("original sequence response has no confirmed Executed receipt".into());
    }
    Ok(true)
}

/// Retain both original child owners while polling. A failed or interrupted
/// cleanup can be stopped and reported; it never grants the next cohort a slot.
fn retire<F>(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer,
    process: &mut Option<process::ProcessHistory>, request: u64, config: &Config,
    control: &mut Control, deadline: &Deadline, operator: &mut Option<operator::Service>,
    actor: &Profile, time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let started = Instant::now();
    loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
        if let Some(service) = operator.as_mut() { service.checkpoint(); }
        let stopping = control.native_stop(driver, time);
        if !matches!(&stopping, Ok(false)) {
            // A Fence may arrive between completed requests, after the prior
            // actor session has ended. Keep only this original operator's
            // finite reply opportunity; no successor source or cohort starts.
            let reply = Instant::now();
            let window = Duration::from_millis(actor.reply_ms);
            while reply.elapsed() < window && deadline.check(time()).is_ok() {
                operator::checkpoint(operator);
                if let Some(history) = process.as_mut() { history.reap(); }
                let remaining = window.saturating_sub(reply.elapsed());
                let pause = actor.poll_ms.min(50)
                    .min(u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX));
                if pause == 0 { break; }
                crate::workflow::pause(pause);
            }
            return stopping.map(|_| false);
        }
        if let Some(history) = process.as_mut() { history.reap(); }
        let direct = driver.retire_completed_request(request).map_err(debug)?;
        if direct && process.as_ref().is_none_or(|history| history.pending() == 0) {
            *process = None;
            return Ok(true);
        }
        if started.elapsed() >= Duration::from_millis(config.timing.cleanup_ms) {
            return Err("previous learned helper cohort is not confirmed reaped; no successor admitted".into());
        }
        crate::workflow::pause(config.timing.poll_ms.clamp(1, 50));
    }
}

#[cfg(test)]
pub(super) fn run<F>(config: Config, entries: Vec<Entry>, peers: &PeerProfile,
    continuing: bool, after_message: Option<u64>, credibility: Option<&Path>, time: F)
    -> Result<SequenceResult, String>
where F: FnMut() -> ElapsedTick {
    run_with_operator(config, entries, peers, continuing, after_message, credibility, None, time)
}

/// One independently provisioned operator session follows the original owner,
/// including every helper handoff. New messages never reissue its grants or
/// replenish its query, connection, framing or elapsed allowance.
pub(super) fn run_with_operator<F>(mut config: Config, entries: Vec<Entry>, peers: &PeerProfile,
    continuing: bool, after_message: Option<u64>, credibility: Option<&Path>,
    selected_operator: Option<&operator::Selection>, mut time: F)
    -> Result<SequenceResult, String>
where F: FnMut() -> ElapsedTick {
    validate(&config, &entries, peers)?;
    if let Some(selected) = selected_operator {
        // The first entry owns retained outputs; every entry contributes its
        // independently selected actor, human-review and Stop socket names.
        // Admit all of them before sampling time or creating any owner/listener.
        for entry in &entries {
            selected.check(&config, &entry.loaded, &entry.actor, peers, entry.precheck.as_ref())?;
        }
    }
    if !continuing && (after_message.is_some() || credibility.is_some()) {
        return Err("sequence predecessor and credibility activation require explicit live recovery".into());
    }
    if continuing && entries[0].loaded.creation_campaigns.is_some() {
        return Err("resident recovery cannot install a creation campaign guard".into());
    }
    if !continuing {
        super::recipe::check_creation(&config, &entries[0].loaded)?;
    }
    if let Some(after) = after_message {
        super::continuation::check_ids(entries[0].loaded.request, after)?;
    }
    let start = time();
    let lifetime = entries.iter().map(|entry| entry.actor.runtime_ms)
        .min().unwrap().min(config.timing.runtime_ms);
    let global = Deadline { logical: plus(start, lifetime)?, started: Instant::now(),
        wall: Duration::from_millis(lifetime) };
    let first = &entries[0];
    let deadline = entry_deadline(&global, first, start)?;
    // The whole fixed sequence retains ONE original credential capability.
    // Preflight its independent inputs before any socket/store/recovery work.
    let mut credential = super::credential::prepare(&first.loaded, continuing, &deadline, &mut time)?;
    // Only this fixed operation names the authenticated stop endpoint. Sharing
    // it with each request preserves one socket, candidate quota and outcome.
    let mut control = Control::new(&config, first.loaded.request, Some(peers))?;
    let mut initial_actor = Some(ingress::Bound::bind(&first.actor)?);
    let topology::Started { custody: (host, reviewer, observer, identity_observer), source, output } =
        topology::start(&config, &first.loaded, continuing, &mut control, &deadline, &mut time)?;
    if let Some(selected) = first.loaded.checkpoint_on_generation {
        selected.check_owner(&host)?;
    }
    let mut initial_deadline = Some(deadline);
    let mut prediction = observer.map(|observer| prediction::Session::new(first.loaded.request, observer));
    let mut identity = identity_observer.map(identity::Session::new);
    prediction::check(&host, prediction.as_ref())?;
    let mut initial_continuation = request_preflight(&host, &first.loaded, continuing,
        None, after_message, credibility)?;
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    topology::attach(&mut driver, source)?;
    let mut operator = selected_operator.map(|selected|
        selected.start(&first.actor, port.clone(), &driver, &global)).transpose()?;
    let mut wire = ActorWire::new(port.clone());
    let mut responses = Vec::with_capacity(entries.len());
    let mut histories = Vec::with_capacity(entries.len());
    let mut process = None;
    let mut failure = None;
    let mut terminal = None;
    let mut previous = after_message;
    for (index, entry) in entries.iter().enumerate() {
        if index != 0 {
            match retire(&mut driver, &reviewer, &mut process, previous.unwrap(),
                &config, &mut control, &global, &mut operator, &entries[index - 1].actor, &mut time) {
                Ok(true) => {},
                Ok(false) => { terminal = Some("learned sequence stopped before its next request".into()); break; }
                Err(error) => { failure = Some(error); break; }
            }
        }
        let finishing = if matches!(entry.release, LearnedTextRelease::Finish) { previous } else { None };
        let prepared = (|| {
            let now = time();
            let deadline = match initial_deadline.take() {
                Some(deadline) => { deadline.check(now)?; deadline },
                None => entry_deadline(&global, entry, now)?,
            };
            let continuation: Option<MessageContinuation> = if index == 0 {
                initial_continuation.take()
            } else {
                let host = driver.supervisor().host().map_err(debug)?;
                let command = request_preflight(&host, &entry.loaded, true, finishing,
                    if finishing.is_none() { previous } else { None }, None)?;
                if let Some(prediction) = prediction.as_mut() {
                    prediction.select_request(&host, entry.loaded.request)?;
                }
                command
            };
            // Bind only this independently selected actor, after the previous
            // complete receipt and BOTH original helper cleanup owners permit it.
            let bound = match initial_actor.take() {
                Some(bound) => bound,
                None => ingress::Bound::bind(&entry.actor)?,
            };
            let mut intake = bound.attach(port.clone(), entry.release, deadline.logical)?;
            if let Some(settings) = entry.precheck.as_ref() {
                intake.select_precheck(settings, peers);
            }
            let intake = Some(intake);
            Ok::<_, String>((deadline, continuation, intake))
        })();
        let (mut deadline, message_continuation, mut intake) = match prepared {
            Ok(value) => value, Err(error) => { failure = Some(error); break; },
        };
        let cycle = execute_request(driver, &mut config, &entry.loaded,
            RequestServices { peers, reviewer: &reviewer, prediction: &mut prediction,
                identity: &mut identity, credential: &mut credential,
                intake: &mut intake, operator: &mut operator, wire: &mut wire, finishing, message_continuation,
                credibility: if index == 0 { credibility } else { None },
                stop_operation: first.loaded.request },
            &mut control, &mut deadline, &mut time);
        let RequestCycle { driver: current, response, failure: failed, history, process: cohort } = cycle;
        driver = current; process = cohort;
        if let Some(history) = history { histories.push(history); }
        let accepted = confirmed(&driver, entry.loaded.request, &response);
        responses.push(response);
        // The response service is already closed. Its old credentials and
        // request selection cannot flow into a successor admission.
        drop(intake);
        if let Some(error) = failed { failure = Some(error); break; }
        match accepted {
            Ok(true) => {},
            Ok(false) => {
                terminal = Some(format!("learned sequence ended at request {} without an Executed receipt",
                    entry.loaded.request));
                break;
            }
            Err(error) => { failure = Some(error); break; }
        }
        previous = Some(entry.loaded.request);
    }
    // The observational endpoint ends with this finite service. Retirement
    // revokes its original custodians, not publication rights or receipts.
    if let Some(service) = operator.as_mut() { service.retire(); }
    if let Some(error) = failure.take() {
        let already_stopped = driver.supervisor().host().ok()
            .is_some_and(|host| host.inspect().stop.is_some());
        failure = Some(if already_stopped { error } else {
            match stop(&mut driver, first.loaded.request, &mut time) {
                Ok(()) => error, Err(stopping) => format!("{error}; sequence stop/drain: {stopping}"),
            }
        });
    }
    // A request-local refusal may leave a healthy retained source. Preserve its
    // checkpoint before reporting the incomplete sequence; no new work follows.
    let failure = topology::retain(output.as_ref(), &driver, failure);
    let failure = anchors::retain(&first.loaded, &driver, failure);
    let failure = match (failure, terminal) {
        (Some(error), Some(terminal)) => Some(format!("{error}; {terminal}")),
        (None, Some(terminal)) => Some(terminal),
        (failure, None) => failure,
    };
    let started = Instant::now();
    let cleanup_pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    let remaining = config.timing.cleanup_ms.saturating_sub(
        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    let cleanup_pending = cleanup_pending.saturating_add(process.map_or(0,
        |history| history.cleanup(remaining, config.timing.poll_ms)));
    drop(histories);
    Ok(SequenceResult { responses, failure, cleanup_pending })
}

fn emit(result: SequenceResult, output: &mut impl Write) -> Result<(), String> {
    for response in &result.responses {
        output.write_all(&response.encode()).and_then(|()| output.write_all(b"\n"))
            .and_then(|()| output.flush()).map_err(|error|
                format!("original sequence response output failed: {error}; no effect retry was sent"))?;
    }
    if let Some(error) = result.failure { return Err(error); }
    if result.cleanup_pending != 0 {
        return Err(format!("{} direct learned helper children lack reaping confirmation", result.cleanup_pending));
    }
    if result.responses.iter().any(|response| !matches!(response.result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))) {
        return Err("learned sequence is not confirmed; inspect its original responses".into());
    }
    Ok(())
}
