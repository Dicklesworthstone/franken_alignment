//! A finite sequence on one ORIGINAL native numerical/publication owner.
//! Every release retains independent source-only actor admission, congress and
//! human review. Only a confirmed Executed predecessor permits another entry.
#[path = "sequence/profile.rs"]
mod profile;
#[path = "sequence/precheck.rs"]
mod disclosure;
#[path = "sequence/recovery.rs"]
mod recovered;
#[cfg(test)]
#[path = "sequence/tests.rs"]
mod tests;

use super::*;
use fa_reference::action::ActionSpec;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::oversight::actor::ActorProposal;

pub(super) const MAX_ENTRIES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Release {
    Message,
    Finish,
}

pub(super) struct Entry {
    pub actor: Profile,
    pub inputs: Inputs,
    pub release: Release,
    pub precheck: Option<exact_precheck::Selection>,
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    profile::command(args, credibility)
}

/// Create the explicitly selected finite sequence on one original owner.
pub(super) fn run<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    run_selected(
        config,
        entries,
        reviewer_profile,
        publication,
        None,
        time,
        output,
    )
}

/// Every entry is independently selected before the first listener or durable
/// owner. Explicit recovery opens the original stream exactly once, after the
/// selected Message, under the SAME Stop and deadline as its live suffix.
/// The original numerical context, source custodian and invocation deadline
/// then survive every handoff without another reopen or authority reset.
pub(super) fn run_selected<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    after: Option<u64>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    run_with_operator(
        config,
        entries,
        reviewer_profile,
        publication,
        (after, None),
        time,
        output,
    )
}

/// One explicitly selected operator retains its original native grants,
/// authenticated session and elapsed lifetime through every release. Recovery
/// installs it only after the original replay and suffix admission complete.
pub(super) fn run_with_operator<F>(
    mut config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (Option<u64>, Option<&operator::Selection>),
    mut time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let (after, selected_operator) = selection;
    profile::validate_selected(&config, &entries, reviewer_profile, after)?;
    if let Some(selected) = selected_operator {
        // Direct callers receive the same whole-sequence scope and canonical
        // endpoint admission as the file loader, before time or owner setup.
        for entry in &entries {
            selected.check(&config, &entry.actor, reviewer_profile)?;
        }
    }
    // The independently selected endpoints share one invocation's namespace.
    // Check their cross-role aliases before any clock, listener or live owner.
    disclosure::check(
        &config,
        &entries
            .iter()
            .map(|entry| (&entry.actor, entry.precheck.as_ref()))
            .collect::<Vec<_>>(),
        reviewer_profile,
        selected_operator,
    )?;
    let first = &entries[0];
    let selected = bootstrap::selection(&config, &first.inputs, publication)?;
    let start = time();
    let global = Deadline {
        logical: plus(start, config.timing.runtime_ms)?,
        started: Instant::now(),
        wall: Duration::from_millis(config.timing.runtime_ms),
    };
    let first_deadline = entry_deadline(&global, &first.actor, start)?;
    // Exclusive transport ownership precedes creation OR cooperative recovery.
    // The same Stop candidate quota must cover replay and every live release.
    // No stale path is removed to make this invocation start.
    let first_socket = BoundSocket::bind(&first.actor.socket, None)?;
    first.actor.secure_socket()?;
    let mut control = Control::new(&config, first.actor.request, Some(reviewer_profile))?;
    let (host, reviewer) = if after.is_some() {
        bootstrap::open_with_checkpoints(
            &config,
            &first.inputs,
            selected,
            Some(&mut control),
            &first_deadline,
            &mut time,
        )?
    } else {
        bootstrap::prepare(&config, &first.inputs, selected, false)?
    };
    if !host.generated_text_stream_required().map_err(debug)?
        || !host.publication_guard_required()
        || host.file_source_status().map(|status| status.policy) != Some(config.source_policy)
        || host.journal_capacity().map_err(debug)?.reserve() != Some(RecoveryReserve::terminal())
        || host.publication_validation_profile().map_err(debug)?
            != publication.map(|profile| profile.limits)
    {
        return Err("native sequence source, reserve and publication contracts must match".into());
    }
    if let Some(after) = after {
        // The original opener has already applied its single recovery Fence.
        // Inspect the complete suffix before a source read, fresh qualification,
        // reference output or numerical command can start new work.
        recovered::preflight(&host, &entries, after, first_deadline.logical)?;
    }
    if let Some(selected) = &first.precheck {
        super::super::precheck::check_owner_epoch(&selected.settings, &host)?;
    }
    host.check_credibility().map_err(debug)?;
    let (port, supervisor) = host.into_generated_text_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let operator = selected_operator
        .map(|selected| selected.start(&first.actor, port.clone(), &driver, &first_deadline))
        .transpose()?;
    control.install_operator(operator);
    // These are bounded executable configurations, not children or review
    // capabilities. A clone for a successor is installed ONLY after the old
    // request's retained child owner confirms complete reaping.
    let programs = config.programs.clone();
    let mut first_socket = Some(first_socket);
    let mut first_deadline = Some(first_deadline);
    let mut previous = after;
    let mut completed = 0;
    let mut automatic_cleanup_attempted = false;
    let mut pending_operator_reply = None;
    let work = (|| {
        for (index, entry) in entries.iter().enumerate() {
            // Each selected entry owns at most one bounded reply interval.
            // Retiring a predecessor or stopping before actor intake must not
            // discard an operator receipt before this interval can be served.
            pending_operator_reply = Some((entry.actor.reply_ms, entry.actor.poll_ms));
            if index != 0 {
                let request =
                    previous.expect("a resident successor retains its completed predecessor");
                if !retire(
                    &mut driver,
                    request,
                    &config,
                    (&mut control, &reviewer, &global),
                    &mut time,
                )? {
                    return Ok(());
                }
                config.programs = programs.clone();
            }
            let now = time();
            let deadline = match first_deadline.take() {
                Some(deadline) => {
                    deadline.check(now)?;
                    deadline
                }
                None => entry_deadline(&global, &entry.actor, now)?,
            };
            if control.checkpoint(&mut driver, &reviewer, &deadline, &mut time)? {
                return Ok(());
            }
            let socket = match first_socket.take() {
                Some(socket) => socket,
                None => {
                    let socket = BoundSocket::bind(&entry.actor.socket, None)?;
                    entry.actor.secure_socket()?;
                    socket
                }
            };
            let inbox = FileActorInbox::new(
                entry.actor.actor,
                ActorWire::new(port.clone()),
                entry.actor.channels(),
                entry.actor.connections,
            )
            .map_err(debug)?;
            let pool =
                FileActorPool::for_requests(vec![(entry.actor.request, inbox)]).map_err(debug)?;
            let mut ingress = Intake {
                socket,
                pool,
                profile: entry.actor.clone(),
                attempted: 0,
            };
            let cycle = execute_entry(
                &mut driver,
                &mut config,
                entry,
                previous,
                EntryServices {
                    control: &mut control,
                    reviewer: &reviewer,
                    peers: reviewer_profile,
                    publication,
                    deadline: &deadline,
                    ingress: &mut ingress,
                },
                &mut time,
                output,
            );
            if matches!(&cycle, Ok(true)) {
                completed += 1;
            }
            let cycle = cycle.map_err(|error| {
                stop_after_failure(
                    &mut driver,
                    &control,
                    first.actor.request,
                    &mut automatic_cleanup_attempted,
                    error,
                    &mut time,
                )
            });
            // Keep only the preceding actor's bounded reply opportunity.
            // Stop remains ahead of actor traffic, including the handoff gap.
            // An already attempted automatic drain must never be retried by
            // the shared Stop observer during this historical reply window.
            let grace = reply_grace(
                &mut driver,
                &mut ingress,
                (&mut control, &reviewer, &global),
                !automatic_cleanup_attempted,
                &mut time,
            );
            pending_operator_reply = None;
            ingress.pool.revoke_all();
            drop(ingress);
            match (cycle, grace) {
                (Err(error), Err(reply)) => {
                    return Err(format!("{error}; actor reply: {reply}"));
                }
                (Err(error), _) | (_, Err(error)) => return Err(error),
                (Ok(accepted), Ok(stopped)) => {
                    if !accepted || stopped {
                        return Ok(());
                    }
                }
            }
            previous = Some(entry.actor.request);
        }
        Ok(())
    })();
    let mut failure = work.err().map(|error| {
        stop_after_failure(
            &mut driver,
            &control,
            first.actor.request,
            &mut automatic_cleanup_attempted,
            error,
            &mut time,
        )
    });
    if let Some(timing) = pending_operator_reply
        && control.has_operator()
    {
        // No actor intake was left alive on this early exit. Retain only the
        // same operator and original Stop for this entry's unused reply window,
        // under the invocation deadline and the operator's original lifetime.
        // A failed automatic drain is already final and must not be retried.
        let grace = reply_grace_selected(
            &mut driver,
            None,
            timing,
            (&mut control, &reviewer, &global),
            !automatic_cleanup_attempted,
            &mut time,
        );
        if let Err(reply) = grace {
            failure = Some(match failure.take() {
                Some(error) => format!("{error}; operator reply: {reply}"),
                None => reply,
            });
        }
    }
    if let Some(mut operator) = control.take_operator() {
        operator.retire();
    }
    // The original driver is consumed exactly once. No sequence entry can
    // detach the owner while its direct children still require cleanup.
    let pending = cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms);
    if let Some(error) = failure {
        return Err(format!("{error}; helper cleanup pending={pending}"));
    }
    if pending != 0 {
        return Err(format!(
            "{pending} direct helper children lack reaping confirmation"
        ));
    }
    if completed != entries.len() {
        return Err(format!(
            "native sequence ended after {completed} of {} confirmed Executed releases; no successor admitted",
            entries.len()
        ));
    }
    Ok(())
}

fn entry_deadline(
    global: &Deadline,
    actor: &Profile,
    now: ElapsedTick,
) -> Result<Deadline, String> {
    global.check(now)?;
    let deadline = Deadline {
        logical: global.logical.min(plus(now, actor.runtime_ms)?),
        // Preserve the invocation's original wall origin even if the supplied
        // logical clock stalls. Later entries can only shorten its horizon.
        started: global.started,
        wall: global.wall.min(
            global
                .started
                .elapsed()
                .saturating_add(Duration::from_millis(actor.runtime_ms)),
        ),
    };
    deadline.check(now)?;
    Ok(deadline)
}

struct EntryServices<'a> {
    control: &'a mut Control,
    reviewer: &'a FileHumanReviewer,
    peers: &'a PeerProfile,
    publication: Option<&'a PublicationProfile>,
    deadline: &'a Deadline,
    ingress: &'a mut Intake,
}

fn execute_entry<F>(
    driver: &mut FileSupervisedDriver,
    config: &mut Config,
    entry: &Entry,
    previous: Option<u64>,
    services: EntryServices<'_>,
    time: &mut F,
    output: &mut impl Write,
) -> Result<bool, String>
where
    F: FnMut() -> ElapsedTick,
{
    let EntryServices {
        control,
        reviewer,
        peers,
        publication,
        deadline,
        ingress,
    } = services;
    let prepared = {
        let host = driver.supervisor().host().map_err(debug)?;
        if let Some(selected) = &entry.precheck {
            // An independently granted restriction may advance the epoch
            // between releases. Bind this row to its actual handoff before
            // source refresh or numerical work; never rewrite its context.
            super::super::precheck::check_owner_epoch(&selected.settings, &host)?;
        }
        match (entry.release, previous) {
            (Release::Message, Some(after)) => Some(continuation::prepare(
                &host,
                entry.actor.request,
                &entry.inputs,
                after,
            )?),
            (Release::Finish, Some(after)) => {
                if finish::select(
                    &host,
                    entry.actor.request,
                    &entry.inputs,
                    after,
                    deadline.logical,
                )?
                .is_some()
                {
                    return Err("a new native sequence cannot recover a recorded Finish".into());
                }
                None
            }
            (Release::Message, None) => None,
            (Release::Finish, None) => {
                return Err("native sequence Finish needs its original Message".into());
            }
        }
    };
    if !qualification::prepare(
        driver,
        config,
        None,
        (&mut *control, reviewer, deadline),
        &mut *time,
    )? {
        return Ok(false);
    }
    let proposal = match entry.release {
        Release::Finish => {
            let Some(proposal) = finish::prepare(
                driver,
                config,
                entry.actor.request,
                previous.expect("Finish predecessor checked above"),
                (&mut *control, reviewer, deadline),
                &mut *time,
            )?
            else {
                return Ok(false);
            };
            proposal
        }
        Release::Message => {
            let complete = if let Some(prepared) = prepared {
                continuation::start(
                    driver,
                    config,
                    &entry.inputs,
                    prepared,
                    (&mut *control, reviewer, deadline),
                    &mut *time,
                )?
            } else {
                generate(
                    driver,
                    config,
                    &entry.inputs,
                    (&mut *control, reviewer, deadline),
                    &mut *time,
                )?
            };
            if !complete {
                return Ok(false);
            }
            completed_message_proposal(driver, entry.actor.request, &entry.inputs, deadline)?
        }
    };
    // The source-only port deliberately accepts both original release kinds.
    // Pin this independently selected entry's complete original action before
    // actor intake so another valid source intent cannot change the sequence.
    let expected_action = selected_action(driver, entry, &proposal)?;
    if !disclosure::admit(
        driver,
        config,
        entry,
        &proposal,
        EntryServices {
            control: &mut *control,
            reviewer,
            peers,
            publication,
            deadline,
            ingress: &mut *ingress,
        },
        time,
        output,
    )? {
        return Ok(false);
    }
    if !check_selected_admission(driver, entry, previous, &proposal, &expected_action)? {
        return Ok(false);
    }
    // LiveControl services the shared original Stop before this callback and
    // checks any newly acknowledged operator Fence before more workflow work.
    // Take and restore the SAME service; no native grants, transport allowance
    // or wall origin are recreated for another review.
    let mut operator = control.take_operator();
    let mut pump = |driver: &mut FileSupervisedDriver| {
        if let Some(service) = &mut operator {
            service.checkpoint();
        }
        ingress.observe(driver)?;
        let status = driver
            .supervisor()
            .host()
            .map_err(debug)?
            .request_status(entry.actor.request)
            .map_err(debug)?;
        Ok(matches!(
            status.disposition,
            FileRequestDisposition::Admitted {
                stage: ActionState::Cancelled
                    | ActionState::Denied
                    | ActionState::Confirmed
                    | ActionState::ConfirmedNotExecuted,
                ..
            }
        ))
    };
    let reviewed = execute_serviced(
        driver,
        reviewer,
        config,
        entry.actor.request,
        deadline,
        ExecuteServices {
            peers: Some(peers),
            publication,
            ingress: &mut pump,
            stop: Some(control.share_stop()),
        },
        &mut *time,
    );
    control.install_operator(operator);
    reviewed?;
    confirmed(driver, entry.actor.request)
}

/// Project the original bounded cumulative frame from the completed native
/// output or explicit Finish. This is an immutable builder: it observes no new
/// source, advances no numerical state and creates no review or effect key.
fn selected_action(
    driver: &FileSupervisedDriver,
    entry: &Entry,
    proposal: &ActorProposal,
) -> Result<ActionSpec, String> {
    let host = driver.supervisor().host().map_err(debug)?;
    let action = match entry.release {
        Release::Message => {
            let text = host
                .decoder_text_generation(entry.inputs.generation)
                .map_err(debug)?;
            let bytes = text.result().map_err(debug)?.bytes().map_err(debug)?;
            let message = std::str::from_utf8(bytes).map_err(debug)?;
            host.stream_message_spec(message, proposal.deadline)
                .map_err(debug)?
        }
        Release::Finish => host.stream_finish_spec(proposal.deadline).map_err(debug)?,
    };
    if action.target != Some(proposal.target)
        || action.policy_epoch != proposal.expected_policy_epoch
        || action.deadline != proposal.deadline
    {
        return Err("native sequence source reference changed before actor intake".into());
    }
    Ok(action)
}

/// A valid native source reference is still only actor-selected input. The
/// original retained source identity AND frozen full action must equal this
/// entry's independent selection before any helper, human or effect work.
fn check_selected_admission(
    driver: &FileSupervisedDriver,
    entry: &Entry,
    previous: Option<u64>,
    proposal: &ActorProposal,
    expected_action: &ActionSpec,
) -> Result<bool, String> {
    let host = driver.supervisor().host().map_err(debug)?;
    if matches!(
        host.request_status(entry.actor.request)
            .map_err(debug)?
            .disposition,
        FileRequestDisposition::NotAdmitted(_)
    ) {
        return Ok(false);
    }
    let original = match entry.release {
        Release::Message => {
            let source = host
                .decoder_text_message_request(entry.actor.request)
                .map_err(|error| {
                    format!("native sequence selected Message intent mismatch: {error:?}")
                })?;
            FileGeneratedTextActorPort::encode_message(source).map_err(debug)?
        }
        Release::Finish => {
            let after = previous.ok_or("native sequence Finish lacks its selected predecessor")?;
            let source = host
                .decoder_text_finish_request(after, entry.actor.request)
                .map_err(|error| {
                    format!("native sequence selected Finish intent mismatch: {error:?}")
                })?;
            FileGeneratedTextActorPort::encode_finish(
                entry.actor.request,
                source.target,
                source.expected_policy_epoch,
                source.deadline,
            )
            .map_err(debug)?
        }
    };
    if &original != proposal
        || host
            .request_action(entry.actor.request)
            .map_err(debug)?
            .spec()
            != expected_action
    {
        return Err(
            "native sequence admitted intent differs from the selected original release".into(),
        );
    }
    Ok(true)
}

fn confirmed(driver: &FileSupervisedDriver, request: u64) -> Result<bool, String> {
    let host = driver.supervisor().host().map_err(debug)?;
    if !matches!(
        host.request_status(request).map_err(debug)?.disposition,
        FileRequestDisposition::Admitted {
            stage: ActionState::Confirmed,
            ..
        }
    ) {
        return Ok(false);
    }
    if !matches!(
        host.request_resolution(request).map_err(debug)?,
        Some(EndpointOutcome::Executed { .. })
    ) {
        return Err("confirmed native sequence request lacks its original Executed receipt".into());
    }
    Ok(true)
}

/// A terminal job is not enough: only the confirmed original effect permits
/// handoff, and its retained helper cohort must actually be reaped first.
fn retire<F>(
    driver: &mut FileSupervisedDriver,
    request: u64,
    config: &Config,
    controls: (&mut Control, &FileHumanReviewer, &Deadline),
    time: &mut F,
) -> Result<bool, String>
where
    F: FnMut() -> ElapsedTick,
{
    let (control, reviewer, deadline) = controls;
    if !confirmed(driver, request)? {
        return Err("native sequence predecessor has no confirmed Executed receipt".into());
    }
    let started = Instant::now();
    loop {
        deadline.check(time())?;
        if control.checkpoint(driver, reviewer, deadline, &mut *time)? {
            return Ok(false);
        }
        if driver.retire_completed_request(request).map_err(debug)? {
            return Ok(true);
        }
        if started.elapsed() >= Duration::from_millis(config.timing.cleanup_ms) {
            return Err(
                "previous native helper cohort is not confirmed reaped; no successor admitted"
                    .into(),
            );
        }
        pause(config.timing.poll_ms.clamp(1, 50));
    }
}

fn reply_grace<F>(
    driver: &mut FileSupervisedDriver,
    ingress: &mut Intake,
    controls: (&mut Control, &FileHumanReviewer, &Deadline),
    pump_stop: bool,
    time: &mut F,
) -> Result<bool, String>
where
    F: FnMut() -> ElapsedTick,
{
    let timing = (ingress.profile.reply_ms, ingress.profile.poll_ms);
    reply_grace_selected(driver, Some(ingress), timing, controls, pump_stop, time)
}

/// The selected entry's original reply interval also covers exits before an
/// actor inbox exists. No replacement transport, grant or deadline is issued.
fn reply_grace_selected<F>(
    driver: &mut FileSupervisedDriver,
    mut ingress: Option<&mut Intake>,
    timing: (u64, u64),
    controls: (&mut Control, &FileHumanReviewer, &Deadline),
    pump_stop: bool,
    time: &mut F,
) -> Result<bool, String>
where
    F: FnMut() -> ElapsedTick,
{
    let (control, reviewer, deadline) = controls;
    let (reply_ms, poll_ms) = timing;
    let started = Instant::now();
    let window = Duration::from_millis(reply_ms);
    let mut stopped = false;
    let operator_selected = control.has_operator();
    let mut actor_failed = false;
    let mut stop_failed = false;
    let mut failure = None;
    while started.elapsed() < window && deadline.check(time()).is_ok() {
        if pump_stop && !stop_failed {
            match control.checkpoint(driver, reviewer, deadline, &mut *time) {
                Ok(true) => {
                    stopped = true;
                    // A terminal Stop checkpoint deliberately returns before
                    // operator work. Its original bounded reply channel remains
                    // independent of that completed one-use cleanup.
                    control.checkpoint_operator();
                }
                Ok(false) => {}
                Err(error) => {
                    if !operator_selected {
                        return Err(error);
                    }
                    failure.get_or_insert(error);
                    stop_failed = true;
                    control.checkpoint_operator();
                }
            }
        } else {
            // Automatic failure cleanup already owns Stop/drain. Do not repeat
            // it merely to deliver the original operator's historical reply.
            control.checkpoint_operator();
        }
        if !actor_failed
            && let Some(ingress) = ingress.as_deref_mut()
        {
            if let Err(error) = ingress.observe(driver) {
                if !operator_selected {
                    return Err(error);
                }
                actor_failed = true;
                failure.get_or_insert(error);
            }
        }
        let remaining = window.saturating_sub(started.elapsed());
        let delay = poll_ms
            .min(50)
            .min(u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX));
        if delay == 0 {
            break;
        }
        pause(delay);
    }
    failure.map_or(Ok(stopped), Err)
}

fn stop_after_failure<F>(
    driver: &mut FileSupervisedDriver,
    control: &Control,
    operation: u64,
    attempted: &mut bool,
    error: String,
    time: &mut F,
) -> String
where
    F: FnMut() -> ElapsedTick,
{
    if *attempted || control.native_cleanup_started() {
        return error;
    }
    *attempted = true;
    match stop(driver, operation, &mut *time) {
        Ok(()) => error,
        Err(stopping) => format!("{error}; sequence stop/drain: {stopping}"),
    }
}
