//! A fixed original native sidecar sequence, preserving numerical custody to handoff.
use super::{Config, Loaded, Deadline, ElapsedTick, FileSupervisedDriver, FileHumanReviewer,
    debug, plus, refresh};
use super::ingress::{self, Intake};
use super::super::super::control::Control;
use crate::publication::PreparedPublication;
use fa_reference::action::consequence::activation::monitor::learned::model::KvTap;
use fa_reference::action::consequence::delivery::persistent::observed::{
    credential::FileCredentialPermit,
    driver::native_learned::{FileNativeDriverLaunch, FileNativeSupervisedDriver},
    helpers::learned::native::{
        NativeReviewMember, NativeReviewStatus, sequence::FileNativeSidecarSequence,
    },
};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::oversight::{
    ReviewWindow,
    learned_host::sidecar::{LearnedSidecarRequest, workers::LearnedWorkerRound},
    sidecar::receiver::native::SidecarProbeQuery,
};
use std::collections::BTreeSet;

#[path = "review/reserved_convening.rs"]
mod reserved_convening;

pub(super) struct Reviewed {
    pub driver: FileSupervisedDriver,
    pub history: Option<FileNativeSidecarSequence>,
    pub process: Option<super::process::ProcessHistory>,
    pub stop_requested: bool,
    pub failure: Option<String>,
}

/// A grant is an operation ceiling, never a new native-work budget. Preserve
/// legacy one-round admission when none of its optional successors are funded;
/// progressive recipes keep their original fixed round schedule unchanged.
pub(super) fn reserved_successors(loaded: &Loaded, operations: usize) -> usize {
    if loaded.rounds.len() != 1 {
        return 0;
    }
    let Some(native) = loaded.native.as_ref() else {
        return 0;
    };
    let ceiling = operations.min(
        fa_reference::action::consequence::oversight::sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS - 1,
    );
    (1..=ceiling)
        .rev()
        .find(|count| native.check_limits(&loaded.native_limits, count + 1).is_ok())
        .unwrap_or(0)
}

#[cfg(test)]
pub(super) fn start<F>(
    driver: FileSupervisedDriver,
    config: &mut Config,
    loaded: &Loaded,
    deadline: &Deadline,
    time: &mut F,
) -> Result<FileNativeSupervisedDriver, (FileSupervisedDriver, String)>
where
    F: FnMut() -> ElapsedTick,
{
    start_with_successors(driver, config, loaded, deadline, time, 0)
}

fn start_with_successors<F>(
    mut driver: FileSupervisedDriver,
    config: &mut Config,
    loaded: &Loaded,
    deadline: &Deadline,
    time: &mut F,
    successors: usize,
) -> Result<FileNativeSupervisedDriver, (FileSupervisedDriver, String)>
where
    F: FnMut() -> ElapsedTick,
{
    let prepared = (|| {
        let capture = refresh(&mut driver, config, deadline, time)?;
        let now = time();
        deadline.check(now)?;
        let mut previous = now;
        let mut rounds = Vec::new();
        for id in &loaded.rounds {
            let window = ReviewWindow {
                commit_by: plus(previous, config.timing.commit_ms)?,
                reveal_by: plus(previous, config.timing.reveal_ms)?,
            };
            if !(previous < window.commit_by
                && window.commit_by < window.reveal_by
                && window.reveal_by < deadline.logical)
            {
                return Err(
                    "insufficient learned review lifetime for the complete fixed schedule".into(),
                );
            }
            rounds.push(LearnedWorkerRound {
                round: *id,
                evidence_root: capture.reference_root(),
                window,
            });
            previous = window.reveal_by;
        }
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        let FileRequestDisposition::Admitted { attempt, .. } = host
            .request_status(loaded.request)
            .map_err(debug)?
            .disposition
        else {
            return Err("learned request was not admitted by its original gateway".into());
        };
        let message = host.learned_text_message(loaded.evidence).map_err(debug)?;
        let rows: BTreeSet<_> = message
            .evidence()
            .audit()
            .source()
            .groups()
            .map(|group| group.row)
            .collect();
        let request = LearnedSidecarRequest {
            identity: loaded.sidecar,
            priority: loaded.priority.clone(),
            budget: loaded.disclosure,
        };
        let current = host.learned_generation_inspection().map_err(debug)?;
        let revision = host.revision();
        let sidecar = host
            .begin_learned_sidecar_plan(
                revision,
                attempt,
                current.numerical.actor_revision,
                request,
            )
            .map_err(debug)?;
        let input = host.current_learned_sidecar(&sidecar).map_err(debug)?;
        let native = loaded
            .native
            .as_ref()
            .ok_or("no independently admitted native model roster")?;
        let total = rounds.len().checked_add(successors)
            .ok_or("learned successor roster reservation overflow")?;
        if total > fa_reference::action::consequence::oversight::sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS {
            return Err("learned successor roster inventory exceeds the original round bound".into());
        }
        native.check_limits(&loaded.native_limits, total)?;
        let make_members = || {
            let mut members = std::collections::BTreeMap::new();
            for (name, view) in input.views() {
                let mut queries = Vec::new();
                for row in &rows {
                    let tap = KvTap {
                        layer: row.layer,
                        side: row.side,
                    };
                    let probes = loaded
                        .probes
                        .get(&tap)
                        .ok_or("missing independently registered K/V probes")?;
                    for probe in probes {
                        queries.push(SidecarProbeQuery {
                            row: *row,
                            probe: probe.clone(),
                        });
                    }
                }
                members.insert(
                    name.clone(),
                    NativeReviewMember {
                        evaluator: native
                            .evaluator(name, view.actual_input().input_profile().clone())?,
                        queries,
                        salt: super::super::super::nonce()?.to_vec(),
                    },
                );
            }
            Ok::<_, String>(members)
        };
        let mut rosters = std::collections::BTreeMap::new();
        for round in &rounds {
            rosters.insert(round.round, make_members()?);
        }
        // Every original helper owner is constructed before the first ballot.
        // A later Hold may select only one of these already paid, fixed owners;
        // no response-dependent recipe, model, probe or replacement is loaded.
        let successors = (0..successors).map(|_| make_members())
            .collect::<Result<Vec<_>, _>>()?;
        let launch = FileNativeDriverLaunch {
            journal_revision: host.revision(),
            request: loaded.request,
            sidecar,
            rounds,
            rosters,
            limits: loaded.native_limits,
        };
        Ok((launch, successors, capture.snapshot().clone(), now))
    })();
    let (launch, successors, snapshot, now) = match prepared {
        Ok(value) => value,
        Err(error) => return Err((driver, error)),
    };
    let started = if successors.is_empty() {
        driver.start_native_learned_sequence(launch, snapshot, now)
    } else {
        driver.start_native_learned_sequence_with_convening(launch, snapshot, now, successors)
    };
    started.map_err(|error| {
            let error = *error;
            (error.driver, debug(error.error))
        })
}

#[cfg(test)]
pub(super) fn run<F>(driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F) -> Reviewed
where F: FnMut() -> ElapsedTick {
    if loaded.process.is_some() {
        return super::process::run(driver, config, loaded, reviewer, control, deadline, time);
    }
    run_serviced(driver, config, loaded, reviewer, control, deadline, time, &mut None)
}

pub(super) fn run_checked<F>(driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
    publication: Option<&PreparedPublication<'_>>) -> Reviewed
where F: FnMut() -> ElapsedTick {
    run_serviced_checked(driver, config, loaded, reviewer, control, deadline, time, &mut None, publication)
}

#[cfg(test)]
pub(super) fn run_serviced<F>(driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
    intake: &mut Option<Intake>) -> Reviewed
where F: FnMut() -> ElapsedTick {
    run_serviced_checked(driver, config, loaded, reviewer, control, deadline, time, intake, None)
}

pub(super) fn run_serviced_checked<F>(driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
    intake: &mut Option<Intake>, publication: Option<&PreparedPublication<'_>>) -> Reviewed
where F: FnMut() -> ElapsedTick {
    run_serviced_checked_with_credential(driver, config, loaded, reviewer, control,
        deadline, time, intake, publication, None)
}

pub(super) fn run_serviced_checked_with_credential<F>(driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
    intake: &mut Option<Intake>, publication: Option<&PreparedPublication<'_>>,
    credential: Option<&FileCredentialPermit>) -> Reviewed
where F: FnMut() -> ElapsedTick {
    run_serviced_checked_with_operator(driver, config, loaded, reviewer, control,
        deadline, time, intake, publication, credential, &mut None)
}

pub(super) fn run_serviced_checked_with_operator<F>(mut driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
    intake: &mut Option<Intake>, publication: Option<&PreparedPublication<'_>>,
    credential: Option<&FileCredentialPermit>, operator: &mut Option<super::operator::Service>) -> Reviewed
where F: FnMut() -> ElapsedTick {
    if loaded.publication.is_some() != publication.is_some() {
        return Reviewed { driver, history: None, process: None, stop_requested: false,
            failure: Some("learned review requires its original selected publication source".into()) };
    }
    if loaded.process.is_some() {
        return super::process::run_serviced_checked_with_operator(driver, config, loaded, reviewer,
            control, deadline, time, intake, publication, credential, operator);
    }
    match control.checkpoint(&mut driver, reviewer, deadline, time) {
        Ok(false) => {},
        result => return Reviewed { driver, history: None, process: None,
            stop_requested: matches!(result, Ok(true)), failure: result.err() },
    }
    super::operator::checkpoint_driver(operator, &mut driver);
    match control.native_stop(&mut driver, time) {
        Ok(false) => {},
        result => return Reviewed { driver, history: None, process: None,
            stop_requested: matches!(result, Ok(true)), failure: result.err() },
    }
    match ingress::observe(intake, driver.supervisor_mut()) {
        Ok(false) => {},
        result => return Reviewed { driver, history: None, process: None,
            stop_requested: false, failure: result.err() },
    }
    let remaining = operator.as_ref()
        .map_or(0, super::operator::Service::remaining_convene_operations);
    let successors = reserved_successors(loaded, remaining);
    let run = match start_with_successors(driver, config, loaded, deadline, time, successors) {
        Ok(run) => run, Err((driver, error)) => return Reviewed {
            driver, history: None, process: None, stop_requested: false, failure: Some(error) },
    };
    run_started(run, config, loaded, reviewer, control, deadline, time,
        intake, publication, credential, operator)
}

/// Drive the same original owner from initial startup through every explicitly
/// selected successor, keeping all native reservations and work in one wrapper.
pub(super) fn run_started<F>(mut run: FileNativeSupervisedDriver, config: &mut Config,
    loaded: &Loaded, reviewer: &FileHumanReviewer, control: &mut Control,
    deadline: &Deadline, time: &mut F, intake: &mut Option<Intake>,
    publication: Option<&PreparedPublication<'_>>, credential: Option<&FileCredentialPermit>,
    operator: &mut Option<super::operator::Service>) -> Reviewed
where F: FnMut() -> ElapsedTick {
    let mut stop_requested = false;
    let result = (|| {
        loop {
            deadline.check(time())?;
            // Preserve separate run-wide authenticated stop and request-local
            // actor cancellation. Neither path exposes the private native owner.
            if control.pending_peer()? {
                stop_requested = true;
                run.cancel(run.review().revision()).map_err(debug)?;
                break;
            }
            super::operator::checkpoint_supervisor(operator, run.supervisor_mut());
            if run.supervisor().host().map_err(debug)?.inspect().stop.is_some() {
                stop_requested = true;
                // Retire the original numerical/process owner before handing
                // its driver to native stop cleanup. Cancel is a journal no-op
                // for the already cancelled request.
                if run.review().status() == NativeReviewStatus::Running {
                    run.cancel(run.review().revision()).map_err(debug)?;
                }
                break;
            }
            if ingress::observe(intake, run.supervisor_mut())? {
                run.cancel(run.review().revision()).map_err(debug)?;
                break;
            }
            if run.review().status() != NativeReviewStatus::Running {
                if continue_held(&mut run, config, control, deadline, time, intake,
                    operator, &mut stop_requested)?
                    || reserved_convening::continue_held(&mut run, config, control, deadline,
                        time, intake, operator, &mut stop_requested)? {
                    continue;
                }
                break;
            }
            // Waiting for an explicit successor spends no numerical quantum.
            // The SAME original sequence owns the cumulative poll ceiling.
            if run.review().polls() >= loaded.native_limits.polls {
                return Err("original native sidecar review exhausted its finite poll budget".into());
            }
            let round = run.review().round().round;
            let before = run
                .review()
                .records()
                .get(&round)
                .map(|record| (record.members.clone(), record.workers.clone()));
            match publication {
                Some(publication) => { publication.step_native_learned_with_credential(&mut run,
                    &mut config.source, time, None, credential)?; }
                None => { match credential {
                    Some(credential) => run.step_from_policy_file_with_credential(&mut config.source, &mut *time, None, credential),
                    None => run.step_from_policy_file(&mut config.source, &mut *time, None),
                }.result.map_err(debug)?; }
            }
            let after = run.review().records().get(&round)
                .map(|record| (record.members.clone(), record.workers.clone()));
            if run.review().status() == NativeReviewStatus::Running && before == after {
                let stopped = if intake.is_some() || operator.is_some() {
                    wait_for_phase_serviced(&mut run, control, config.timing.poll_ms, deadline, time, intake, operator)?
                } else { wait_for_phase(&mut run, control, config.timing.poll_ms, deadline, time)? };
                if stopped { stop_requested = true; break; }
                // Request-local cancellation may retire the review while the
                // wait correctly reports that no run-wide Stop was received.
                if run.review().status() != NativeReviewStatus::Running { break; }
            }
        }
        if run.review().status() == NativeReviewStatus::Running {
            return Err("original native sidecar review exhausted its finite poll budget".into());
        }
        Ok(())
    })();
    let mut failure = result.err();
    if run.review().status() == NativeReviewStatus::Running {
        if let Err(error) = run.cancel(run.review().revision()) {
            failure = Some(format!(
                "{}; original native cancellation: {error:?}",
                failure.unwrap_or_else(|| "native review did not finish".into())
            ));
        }
    }
    // cancel() retires original numerical custody before fallible journal work.
    // This owner therefore has no running review to detach from its driver.
    let handoff = run
        .into_handoff()
        .expect("finished or cancelled original native review");
    let mut driver = handoff.driver;
    if stop_requested {
        match control.checkpoint(&mut driver, reviewer, deadline, time) {
            Ok(true) => {}
            Ok(false) => {
                failure = Some("authenticated stop peer did not complete the original stop".into())
            }
            Err(error) => failure = Some(error),
        }
    }
    Reviewed { driver, history: Some(handoff.review), process: None, stop_requested, failure }
}

/// Retain the original native owner while an independently admitted operator
/// selects its next UNUSED roster. Nothing can replace models, input, windows,
/// numerical allowances or the invocation deadline after seeing the Hold.
fn continue_held<F>(run: &mut FileNativeSupervisedDriver, config: &mut Config,
    control: &mut Control, deadline: &Deadline, time: &mut F,
    intake: &mut Option<Intake>, operator: &mut Option<super::operator::Service>,
    stop_requested: &mut bool) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let Some(selected) = run.held_successor_round() else { return Ok(false); };
    if !run.has_fixed_successor() { return Ok(false); }
    loop {
        let now = time();
        deadline.check(now)?;
        if control.pending_peer()? {
            *stop_requested = true;
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(false);
        }
        super::operator::checkpoint_supervisor(operator, run.supervisor_mut());
        if run.supervisor().host().map_err(debug)?.inspect().stop.is_some() {
            *stop_requested = true;
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(false);
        }
        if ingress::observe(intake, run.supervisor_mut())? {
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(false);
        }
        // Retirement revokes the ORIGINAL grant. Exhaustion with no pending
        // ticket ends the wait; an already accepted last operation survives.
        // The selected slot keeps the window fixed before the first review.
        if now >= selected.window.commit_by
            || !run.has_fixed_successor()
            || run.held_successor_round().is_none_or(|current| current.round != selected.round)
        {
            return Ok(false);
        }
        if let Some(ticket) = run.take_pending_convene().map_err(debug)? {
            if ticket.round() != selected.round {
                return Err("learned successor does not match the original reserved round".into());
            }
            deadline.check(time())?;
            // The original wrapper checks current source and topology, consumes
            // this one ticket and opens its preprovisioned round. It performs no
            // actor sampling or replacement helper provisioning.
            run.convene_from_policy_file(&mut config.source, ticket, &mut *time)
                .result.map_err(debug)?;
            return Ok(true);
        }
        super::super::super::pause(config.timing.poll_ms.clamp(1, 50)
            .min(selected.window.commit_by.0 - now.0));
    }
}

// Polls and review revisions advance even when the protocol cannot. The caller
// compares only ORIGINAL member progress and worker phases before entering here.
// Waiting spends no numerical quantum or source lease; the next real quantum
// reacquires policy and time through the original owner.
pub(super) fn wait_for_phase<F>(run: &mut FileNativeSupervisedDriver, control: &mut Control,
    poll_ms: u64, deadline: &Deadline, time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    wait_for_phase_serviced(run, control, poll_ms, deadline, time, &mut None, &mut None)
}

fn wait_for_phase_serviced<F>(run: &mut FileNativeSupervisedDriver, control: &mut Control,
    poll_ms: u64, deadline: &Deadline, time: &mut F, intake: &mut Option<Intake>,
    operator: &mut Option<super::operator::Service>) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let due = run.next_review_deadline().ok_or("running native review has no original phase deadline")?;
    loop {
        let now = time();
        deadline.check(now)?;
        if control.pending_peer()? {
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(true);
        }
        super::operator::checkpoint_supervisor(operator, run.supervisor_mut());
        if run.supervisor().host().map_err(debug)?.inspect().stop.is_some() {
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(true);
        }
        if ingress::observe(intake, run.supervisor_mut())? {
            run.cancel(run.review().revision()).map_err(debug)?;
            // Request cancellation is already recorded by its original gateway;
            // it must not masquerade as a nonce-bearing run-wide Stop command.
            return Ok(false);
        }
        if now >= due { return Ok(false); }
        // Bound stop responsiveness independently of an unusually large poll
        // interval. Never advance a logical clock to simulate elapsed time.
        super::super::super::pause(poll_ms.min(50).max(1).min(due.0 - now.0));
    }
}
