//! A finite sequence on one ORIGINAL native numerical/publication owner.
//! Every release retains independent source-only actor admission, congress and
//! human review. Only a confirmed Executed predecessor permits another entry.
#[path = "sequence/anchors.rs"]
mod anchors;
#[path = "sequence/andon.rs"]
mod andon;
#[path = "sequence/evaluation.rs"]
mod evaluation;
#[path = "sequence/precheck.rs"]
mod disclosure;
#[path = "sequence/governor.rs"]
mod governor;
#[path = "sequence/profile.rs"]
mod profile;
#[path = "sequence/recovery.rs"]
mod recovered;
#[path = "sequence/qualification.rs"]
mod requalification;
#[cfg(test)]
#[path = "sequence/tests.rs"]
mod tests;

use super::*;
use fa_reference::action::ActionSpec;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::FileRecoveryRequirements;
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
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (Option<u64>, Option<&operator::Selection>),
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    run_with_qualification(
        config,
        entries,
        reviewer_profile,
        publication,
        selection,
        None,
        time,
        output,
    )
}

/// Recovery can explicitly activate one independently supplied qualification.
/// The source is consumed once before the first new release; later handoffs
/// check the original retained qualification and cannot renew its evidence.
pub(super) fn run_with_qualification<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (Option<u64>, Option<&operator::Selection>),
    credibility: Option<&Path>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    run_with_history(
        config,
        entries,
        reviewer_profile,
        publication,
        selection,
        credibility,
        None,
        time,
        output,
    )
}

/// Independently selected history never replaces the original native owner.
/// Retention consumes only acknowledged cuts and runs once after every returned
/// result, including an early recovery Fence or sealed terminal Stop.
pub(super) fn run_with_history<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (Option<u64>, Option<&operator::Selection>),
    credibility: Option<&Path>,
    history: Option<&anchors::Selection>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    run_with_governor(
        config,
        entries,
        reviewer_profile,
        publication,
        selection,
        credibility,
        history,
        None,
        time,
        output,
    )
}

/// A selected governor and numerical operator retain one original independent
/// session each. Creation installs their campaign guard in the first image;
/// neither a handoff nor a reconnect can replenish their finite allowances.
pub(super) fn run_with_governor<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (Option<u64>, Option<&operator::Selection>),
    credibility: Option<&Path>,
    history: Option<&anchors::Selection>,
    governor: Option<&governor::Settings>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    run_with_governed_recovery(
        config,
        entries,
        reviewer_profile,
        publication,
        selection,
        credibility,
        history,
        governor,
        None,
        time,
        output,
    )
}

/// Reconstruct a governed native suffix only under independently selected
/// complete requirements and history. Fresh human/governor roles are returned
/// together by the original owner's sole acknowledged recovery Fence.
pub(super) fn run_with_governed_recovery<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (Option<u64>, Option<&operator::Selection>),
    credibility: Option<&Path>,
    history: Option<&anchors::Selection>,
    governor: Option<&governor::Settings>,
    requirements: Option<&FileRecoveryRequirements>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let mut retention = anchors::Retention::new(history);
    let result = run_retained(
        config,
        entries,
        reviewer_profile,
        publication,
        selection,
        credibility,
        history,
        governor,
        requirements,
        None,
        &mut retention,
        time,
        output,
    );
    retention.finish(result)
}

/// Create the original source-backed numerical owner with evaluation and Andon
/// accounting in its first acknowledged image. The sole evaluator endpoint,
/// original grants and finite lifetime survive every Message/Finish handoff.
/// Recovery and policy governance retain their separate admission contracts.
#[cfg(test)]
pub(super) fn run_with_andon<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selected_operator: Option<&operator::Selection>,
    history: &anchors::Selection,
    settings: &super::super::andon_settings::Settings,
    evaluator: &super::super::evaluator::Settings,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    run_accounted(
        config,
        entries,
        reviewer_profile,
        publication,
        (None, selected_operator),
        history,
        andon::Selection {
            settings,
            evaluator,
            recovery: None,
            paths: None,
        },
        time,
        output,
    )
}

/// Recover one explicitly selected suffix through the SAME accounted cursor.
/// Its fresh evaluator spans all remaining Messages/Finish and old pending cases;
/// the original numerical work and accounting never acquire a replacement budget.
#[cfg(test)]
pub(super) fn run_with_andon_recovery<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (u64, Option<&operator::Selection>),
    history: &anchors::Selection,
    settings: &super::super::andon_settings::Settings,
    evaluator: &super::super::evaluator::Settings,
    recovery: &evaluation::Recovery,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    run_accounted(
        config,
        entries,
        reviewer_profile,
        publication,
        (Some(selection.0), selection.1),
        history,
        andon::Selection {
            settings,
            evaluator,
            recovery: Some(recovery),
            paths: None,
        },
        time,
        output,
    )
}

fn run_accounted<F>(
    config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (Option<u64>, Option<&operator::Selection>),
    history: &anchors::Selection,
    accounting: andon::Selection<'_>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let mut retention = anchors::Retention::new(Some(history));
    let result = run_retained(
        config, entries, reviewer_profile, publication, selection, None,
        Some(history), None, None, Some(accounting), &mut retention, time, output,
    );
    retention.finish(result)
}

fn run_retained<F>(
    mut config: Config,
    entries: Vec<Entry>,
    reviewer_profile: &PeerProfile,
    publication: Option<&PublicationProfile>,
    selection: (Option<u64>, Option<&operator::Selection>),
    credibility: Option<&Path>,
    history: Option<&anchors::Selection>,
    selected_governor: Option<&governor::Settings>,
    requirements: Option<&FileRecoveryRequirements>,
    accounting: Option<andon::Selection<'_>>,
    retention: &mut anchors::Retention<'_>,
    mut time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let (after, selected_operator) = selection;
    governor::check_selected(
        selected_governor,
        selected_operator,
        after,
        credibility,
        requirements.is_some(),
    )?;
    governor::check_publication(selected_governor, publication)?;
    requalification::check_mode(after, credibility)?;
    profile::validate_selected(&config, &entries, reviewer_profile, after)?;
    if let Some(accounting) = accounting {
        accounting.check_mode(after)?;
        if credibility.is_some()
            || selected_governor.is_some()
            || requirements.is_some()
            || entries.iter().any(|entry| entry.precheck.is_some())
        {
            return Err(andon::INVALID.into());
        }
        accounting.check_configuration(&config, publication)?;
        accounting.check_actors(
            &config,
            &entries.iter().map(|entry| &entry.actor).collect::<Vec<_>>(),
            reviewer_profile,
            selected_operator,
        )?;
        let history = history.ok_or(andon::INVALID)?;
        accounting.check_history(history)?;
        if let Some(publication) = publication {
            let files = publication.input_paths().iter()
                .map(|path| path.as_path()).collect::<Vec<_>>();
            accounting.check_files(&files)?;
            let endpoints = accounting.endpoints(
                &entries.iter().map(|entry| &entry.actor).collect::<Vec<_>>(),
                reviewer_profile, selected_operator,
            )?;
            andon::check_endpoint_files(&endpoints, &files)?;
            history.check_files(&files)?;
        }
    }
    governor::check_recovery(
        selected_governor,
        requirements,
        history,
        &config,
        &entries[0].inputs,
    )?;
    governor::check_with_recovery(
        selected_governor,
        selected_operator,
        &config,
        &entries
            .iter()
            .map(|entry| (&entry.actor, entry.precheck.as_ref()))
            .collect::<Vec<_>>(),
        reviewer_profile,
        after,
        credibility,
        history,
        requirements.is_some(),
    )?;
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
    requalification::check_paths(
        &config,
        &entries
            .iter()
            .map(|entry| (&entry.actor, entry.precheck.as_ref()))
            .collect::<Vec<_>>(),
        reviewer_profile,
        selected_operator,
        credibility,
    )?;
    if let Some(history) = history {
        history.check(
            &config,
            &entries
                .iter()
                .map(|entry| (&entry.actor, entry.precheck.as_ref()))
                .collect::<Vec<_>>(),
            reviewer_profile,
            selected_operator,
            after,
            credibility,
        )?;
    }
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
    let (host, reviewer, governor_role, evaluator_role) = if let Some(accounting) = accounting {
        let (host, reviewer, evaluator) = match accounting.recovery {
            Some(recovery) => {
                let mut stopped = None;
                let opened = recovery.open(
                    &config, &first.inputs, selected, accounting,
                    history.ok_or(andon::INVALID)?, &mut control, &first_deadline,
                    &mut time, &mut stopped,
                );
                retention.observe_stop(stopped);
                opened?
            }
            None => bootstrap::prepare_andon(
                &config, &first.inputs, selected, accounting.settings,
            )?,
        };
        (host, reviewer, None, Some(evaluator))
    } else if let Some(requirements) = requirements {
        let anchor = history
            .and_then(anchors::Selection::input)
            .ok_or("native governed recovery lost its independently selected anchor")?;
        let mut stopped = None;
        let opened = governor::recovery::open(
            &config,
            &first.inputs,
            selected,
            requirements,
            anchor,
            &mut control,
            &first_deadline,
            &mut time,
            retention.requested().then_some(&mut stopped),
        );
        retention.observe_stop(stopped);
        let (host, roles) = opened?;
        (host, roles.human, roles.policy_governor, None)
    } else if after.is_some() {
        let mut stopped = None;
        let opened = bootstrap::open_with_retained_history(
            &config,
            &first.inputs,
            selected,
            Some(&mut control),
            &first_deadline,
            &mut time,
            history.and_then(anchors::Selection::input),
            retention.requested().then_some(&mut stopped),
        );
        retention.observe_stop(stopped);
        let (host, reviewer) = opened?;
        (host, reviewer, None, None)
    } else if let Some(settings) = selected_governor {
        let (host, reviewer, governor) = bootstrap::prepare_governed(
            &config,
            &first.inputs,
            selected,
            settings.campaign_requirement(),
        )?;
        (host, reviewer, Some(governor), None)
    } else {
        let (host, reviewer) = bootstrap::prepare(&config, &first.inputs, selected, false)?;
        (host, reviewer, None, None)
    };
    // No fallible startup admission can discard an acknowledged create/Fence
    // cut. These checks are read-only; a live driver captures its later cut below.
    retention.observe(&host);
    if selected_governor.is_some() != governor_role.is_some() {
        return Err("native governance lost its freshly fenced governor custody".into());
    }
    if accounting.is_some() != evaluator_role.is_some() {
        return Err("native accounting lost its original independent evaluator custody".into());
    }
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
    requalification::check_epoch(first.precheck.as_ref(), &host, credibility.is_some())?;
    if credibility.is_none() {
        host.check_credibility().map_err(debug)?;
    }
    let (port, supervisor) = host.into_generated_text_actor_gateway().map_err(debug)?;
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut operator = match selected_operator
        .map(|selected| selected.start(&first.actor, port.clone(), &driver, &first_deadline))
        .transpose()
    {
        Ok(operator) => operator,
        Err(error) => {
            retention.observe_driver(&driver);
            return Err(error);
        }
    };
    if let (Some(settings), Some(role)) = (selected_governor, governor_role) {
        let installed = crate::workflow::actor_service::governor::Service::start(
            settings,
            &first.actor,
            role,
            &first_deadline,
        )
        .and_then(|governor| {
            operator
                .as_mut()
                .ok_or_else(|| {
                    "native governor lost its independently selected operator".to_owned()
                })?
                .attach_governor(governor)
        });
        if let Err(error) = installed {
            retention.observe_driver(&driver);
            return Err(error);
        }
    }
    control.install_operator(operator);
    if let (Some(accounting), Some(role)) = (accounting, evaluator_role) {
        let evaluator = match super::super::evaluator::Service::start(
            accounting.evaluator,
            &first.actor,
            role,
            &driver,
            &global,
        ) {
            Ok(service) => service,
            Err(error) => {
                retention.observe_driver(&driver);
                return Err(error);
            }
        };
        control.install_evaluator(Some(evaluator));
    }
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
                if index == 0 { credibility } else { None },
                EntryServices {
                    control: &mut control,
                    reviewer: &reviewer,
                    peers: reviewer_profile,
                    publication,
                    deadline: &deadline,
                    ingress: &mut ingress,
                    governed: selected_governor.is_some(),
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
            let terminal = index + 1 == entries.len() || !matches!(&cycle, Ok(true));
            let cycle = if terminal && cycle.is_ok() {
                // A final Hold still needs independent case discovery and a
                // label. Keep only the original evaluation/receipt opportunity;
                // no successor generation, helper review or publication starts.
                evaluate_after_work(
                    &mut driver,
                    &mut ingress,
                    (&mut control, &reviewer, &global),
                    &mut time,
                )
                .and(cycle)
            } else {
                cycle
            };
            // Intermediate successful Messages retain the same evaluator for
            // later assessment. Terminal-only reply work cannot admit a new
            // label, even if a peer already queued a partial frame.
            if terminal {
                control.finish_evaluator();
            }
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
    control.finish_evaluator();
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
        && (control.has_operator() || control.has_evaluator())
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
    if let Some(mut evaluator) = control.take_evaluator() {
        evaluator.retire();
    }
    // Capture only this owner's acknowledged prefix before helper cleanup
    // consumes it. A poisoned owner cannot turn an ambiguous result into export.
    retention.observe_driver(&driver);
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
    governed: bool,
}

fn execute_entry<F>(
    driver: &mut FileSupervisedDriver,
    config: &mut Config,
    entry: &Entry,
    previous: Option<u64>,
    credibility: Option<&Path>,
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
        governed,
    } = services;
    let prepared = {
        let host = driver.supervisor().host().map_err(debug)?;
        // A selected activation has one checked successor epoch. Every other
        // handoff uses the actual retained epoch; neither rewrites a disclosure.
        requalification::check_epoch(entry.precheck.as_ref(), &host, credibility.is_some())?;
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
        credibility,
        (&mut *control, reviewer, deadline),
        &mut *time,
    )? {
        return Ok(false);
    }
    if let Some(selected) = &entry.precheck {
        // Activation and its independent Stop/operator checkpoint have finished.
        // Bind the actual resulting owner before any new source or inference.
        let host = driver.supervisor().host().map_err(debug)?;
        super::super::precheck::check_owner_epoch(&selected.settings, &host)?;
    }
    let mut proposal = match entry.release {
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
    let mut expected_action = selected_action(driver, entry, &proposal)?;
    if !disclosure::admit(
        driver,
        config,
        entry,
        previous,
        (&mut proposal, &mut expected_action),
        EntryServices {
            control: &mut *control,
            reviewer,
            peers,
            publication,
            deadline,
            ingress: &mut *ingress,
            governed,
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
    let mut evaluator = control.take_evaluator();
    let mut pump = |driver: &mut FileSupervisedDriver| {
        if let Some(service) = &mut operator {
            service.checkpoint_driver(driver);
        }
        if let Some(service) = &mut evaluator {
            // The shared Stop checkpoint runs before this callback, and the
            // evaluator itself refuses a newly acknowledged operator Fence.
            service.checkpoint_driver(driver);
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
    control.install_evaluator(evaluator);
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

/// Preserve a final held case's independent labeling opportunity after forward
/// work ends. The same evaluator and original Stop/operator services retain
/// their spent budgets and deadline. Actor failure cannot consume the remaining
/// evaluator opportunity, and no request workflow is restarted here.
fn evaluate_after_work<F>(
    driver: &mut FileSupervisedDriver,
    ingress: &mut Intake,
    controls: (&mut Control, &FileHumanReviewer, &Deadline),
    time: &mut F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let (control, reviewer, deadline) = controls;
    let mut actor_failed = false;
    let mut failure = None;
    while control.has_live_evaluator() && deadline.check(time()).is_ok() {
        if driver.supervisor().host().map_or(true, |host| {
            host.storage_failure().is_some() || host.inspect().stop.is_some()
        }) {
            break;
        }
        // Control preserves the original Stop -> operator -> native Stop ->
        // evaluator ordering, including independently granted operator Fence.
        match control.checkpoint(driver, reviewer, deadline, &mut *time) {
            Ok(false) => {}
            Ok(true) => break,
            Err(error) => {
                failure.get_or_insert(error);
                break;
            }
        }
        if !actor_failed && let Err(error) = ingress.observe(driver) {
            failure.get_or_insert(error);
            actor_failed = true;
            ingress.pool.revoke_all();
        }
        pause(ingress.profile.poll_ms);
    }
    failure.map_or(Ok(()), Err)
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
    let operator_selected = control.has_operator() || control.has_evaluator();
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
                    control.checkpoint_evaluator(driver);
                }
                Ok(false) => {}
                Err(error) => {
                    // A failed Stop checkpoint cannot be followed by fresh
                    // evaluator intake. Only a response already produced under
                    // an earlier successful checkpoint may drain now.
                    control.finish_evaluator();
                    if !operator_selected {
                        return Err(error);
                    }
                    failure.get_or_insert(error);
                    stop_failed = true;
                    control.checkpoint_operator();
                    control.checkpoint_evaluator(driver);
                }
            }
        } else {
            // Automatic failure cleanup already owns Stop/drain. Do not repeat
            // it merely to deliver the original operator's historical reply.
            control.checkpoint_operator();
            control.checkpoint_evaluator(driver);
        }
        if !actor_failed && let Some(ingress) = ingress.as_deref_mut() {
            if let Err(error) = ingress.observe(driver) {
                control.finish_evaluator();
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
