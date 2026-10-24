//! External reviewers on the original learned process sequence and action owner.
#[path = "process/config.rs"] mod config;
pub(super) use config::ProcessRecipe;
use super::{Config, Loaded, Deadline, ElapsedTick, FileSupervisedDriver, FileHumanReviewer,
    debug, plus, refresh, Duration, Instant};
use super::ingress::{self, Intake};
use super::super::super::control::Control;
use super::review::Reviewed;
use crate::publication::PreparedPublication;
use fa_reference::action::consequence::delivery::persistent::{
    requests::FileRequestDisposition,
    observed::{
        driver::process_learned::{FileProcessDriverLaunch, FileProcessSupervisedDriver},
        helpers::{
            processes::HelperRoundAdmission,
            learned_sockets::processes::sequence::{FileLearnedProcessReview, LearnedProcessStatus},
        },
    },
};
use fa_reference::action::consequence::oversight::{
    ReviewWindow, helper_processes::HelperChildren,
    learned_host::sidecar::{LearnedSidecarRequest, workers::{LearnedWorkerRound, LearnedWorkerSchedule}},
};

/// Keep original cleanup custody through publication, cancellation and drain.
/// A missing owner after an uncertain start is not a reap acknowledgement.
pub(super) enum ProcessHistory {
    Review(FileLearnedProcessReview),
    Partial { children: Option<HelperChildren>, unresolved: bool },
}

impl ProcessHistory {
    pub(super) fn reap(&mut self) {
        match self {
            Self::Review(review) => review.reap(),
            Self::Partial { children: Some(children), .. } => { children.request_stop_all(); },
            Self::Partial { children: None, .. } => {},
        }
    }

    pub(super) fn pending(&self) -> usize {
        match self {
            Self::Review(review) => {
                let count = review.records().values().flat_map(|record| record.processes.values())
                    .filter(|status| status.exit.is_none()).count();
                count.max(usize::from(!review.all_reaped()))
            }
            Self::Partial { children, unresolved } => children.as_ref()
                .map_or(0, |children| children.statuses().values().filter(|status| status.exit.is_none()).count())
                .saturating_add(usize::from(*unresolved)),
        }
    }

    pub(super) fn cleanup(mut self, milliseconds: u64, poll_ms: u64) -> usize {
        let started = Instant::now();
        loop {
            self.reap();
            let pending = self.pending();
            if pending == 0 || started.elapsed() >= Duration::from_millis(milliseconds) { return pending; }
            let remaining = Duration::from_millis(milliseconds).saturating_sub(started.elapsed());
            let pause = poll_ms.clamp(1, 50).min(u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX));
            if pause == 0 { return self.pending(); }
            super::super::super::pause(pause);
        }
    }
}

type StartFailure = (FileSupervisedDriver, String, Option<ProcessHistory>);

pub(super) fn start<F>(mut driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    deadline: &Deadline, time: &mut F) -> Result<FileProcessSupervisedDriver, StartFailure>
where F: FnMut() -> ElapsedTick {
    let prepared = (|| {
        let process = loaded.process.as_ref().ok_or("missing explicit learned process recipe")?;
        let capture = refresh(&mut driver, config, deadline, time)?;
        let now = time(); deadline.check(now)?;
        let mut previous = now;
        let mut rounds = Vec::new();
        for id in &loaded.rounds {
            let window = ReviewWindow { commit_by: plus(previous, config.timing.commit_ms)?,
                reveal_by: plus(previous, config.timing.reveal_ms)? };
            if !(previous < window.commit_by && window.commit_by < window.reveal_by
                && window.reveal_by < deadline.logical) {
                return Err("insufficient learned review lifetime for the complete process schedule".into());
            }
            rounds.push(LearnedWorkerRound { round: *id, evidence_root: capture.reference_root(), window });
            previous = window.reveal_by;
        }
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        let FileRequestDisposition::Admitted { attempt, .. } =
            host.request_status(loaded.request).map_err(debug)?.disposition
            else { return Err("learned request was not admitted by its original gateway".into()); };
        let current = host.learned_generation_inspection().map_err(debug)?;
        let revision = host.revision();
        let sidecar = host.begin_learned_sidecar_plan(revision, attempt, current.numerical.actor_revision,
            LearnedSidecarRequest { identity: loaded.sidecar, priority: loaded.priority.clone(),
                budget: loaded.disclosure }).map_err(debug)?;
        let launch = FileProcessDriverLaunch { journal_revision: host.revision(), request: loaded.request,
            sidecar, schedule: LearnedWorkerSchedule { rounds, helpers: process.helpers, polls: process.polls },
            programs: process.programs.clone(), limits: process.limits };
        Ok((launch, capture.snapshot().clone()))
    })();
    let (launch, snapshot) = match prepared {
        Ok(value) => value, Err(error) => return Err((driver, error, None)),
    };
    // The original constructor calls this trusted clock again AFTER child
    // launch. A captured pre-launch tick cannot lend time to external work.
    driver.start_process_learned_sequence(launch, snapshot, &mut *time).map_err(|error| {
        let error = *error;
        let history = ProcessHistory::Partial { children: error.children,
            unresolved: error.admission == HelperRoundAdmission::Unknown };
        (error.driver, debug(error.error), Some(history))
    })
}

fn active(status: LearnedProcessStatus) -> bool {
    matches!(status, LearnedProcessStatus::Running | LearnedProcessStatus::Draining)
}

#[cfg(test)]
pub(super) fn run<F>(driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F) -> Reviewed
where F: FnMut() -> ElapsedTick {
    run_serviced(driver, config, loaded, reviewer, control, deadline, time, &mut None)
}

#[cfg(test)]
pub(super) fn run_serviced<F>(driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
    intake: &mut Option<Intake>) -> Reviewed
where F: FnMut() -> ElapsedTick {
    run_serviced_checked(driver, config, loaded, reviewer, control, deadline, time, intake, None)
}

pub(super) fn run_serviced_checked<F>(mut driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F,
    intake: &mut Option<Intake>, publication: Option<&PreparedPublication<'_>>) -> Reviewed
where F: FnMut() -> ElapsedTick {
    if loaded.publication.is_some() != publication.is_some() {
        return Reviewed { driver, history: None, process: None, stop_requested: false,
            failure: Some("learned process review requires its original selected publication source".into()) };
    }
    match ingress::observe(intake, driver.supervisor_mut()) {
        Ok(false) => {},
        result => return Reviewed { driver, history: None, process: None,
            stop_requested: false, failure: result.err() },
    }
    let mut run = match start(driver, config, loaded, deadline, time) {
        Ok(run) => run,
        Err((driver, error, process)) => return Reviewed {
            driver, history: None, process, stop_requested: false, failure: Some(error) },
    };
    let mut stop_requested = false;
    let result = (|| {
        let polls = loaded.process.as_ref().ok_or("missing explicit learned process recipe")?.polls;
        for _ in 0..polls {
            deadline.check(time())?;
            if control.pending_peer()? {
                stop_requested = true;
                if active(run.review().status()) { run.cancel(run.review().revision()).map_err(debug)?; }
                break;
            }
            if ingress::observe(intake, run.supervisor_mut())? {
                run.cancel(run.review().revision()).map_err(debug)?;
                break;
            }
            if !active(run.review().status()) { break; }
            match publication {
                Some(publication) => { publication.step_process_learned(&mut run,
                    &mut config.source, time, None)?; }
                None => { run.step_from_policy_file(&mut config.source, &mut *time, None).result.map_err(debug)?; }
            }
            if active(run.review().status()) {
                // Children compute asynchronously. Yield only one short real
                // interval, then reacquire source and time for the next quantum.
                // Waiting straight to commit cutoff could miss their response.
                let now = time(); deadline.check(now)?;
                let remaining = run.next_review_deadline().map_or(config.timing.poll_ms,
                    |due| due.0.saturating_sub(now.0));
                let pause = config.timing.poll_ms.clamp(1, 50).min(remaining);
                if pause != 0 { super::super::super::pause(pause); }
            }
        }
        if active(run.review().status()) {
            return Err("original learned process review exhausted its finite poll budget".into());
        }
        if let Some(error) = run.review().failure() { return Err(debug(error)); }
        Ok(())
    })();
    let mut failure = result.err();
    if active(run.review().status()) {
        if let Err(error) = run.cancel(run.review().revision()) {
            failure = Some(format!("{}; original process cancellation: {error:?}",
                failure.unwrap_or_else(|| "process review did not finish".into())));
        }
    }
    let handoff = run.into_handoff().expect("finished or cancelled original process review");
    let mut driver = handoff.driver;
    if stop_requested {
        match control.checkpoint(&mut driver, reviewer, deadline, time) {
            Ok(true) => {},
            Ok(false) => failure = Some("authenticated stop peer did not complete the original stop".into()),
            Err(error) => failure = Some(error),
        }
    }
    Reviewed { driver, history: None, process: Some(ProcessHistory::Review(handoff.review)),
        stop_requested, failure }
}
