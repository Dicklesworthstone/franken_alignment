//! A fixed original native sidecar sequence, preserving numerical custody to handoff.
use super::{Config, Loaded, Deadline, ElapsedTick, FileSupervisedDriver, FileHumanReviewer,
    debug, plus, refresh};
use super::super::super::control::Control;
use fa_reference::action::consequence::activation::monitor::learned::model::KvTap;
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::native_learned::{FileNativeDriverLaunch, FileNativeSupervisedDriver},
    helpers::learned::native::{NativeReviewMember, NativeReviewStatus, sequence::FileNativeSidecarSequence},
};
use fa_reference::action::consequence::oversight::{
    ReviewWindow, learned_host::sidecar::{LearnedSidecarRequest, workers::LearnedWorkerRound},
    sidecar::receiver::native::SidecarProbeQuery,
};
use std::collections::BTreeSet;

pub(super) struct Reviewed {
    pub driver: FileSupervisedDriver,
    pub history: Option<FileNativeSidecarSequence>,
    pub stop_requested: bool,
    pub failure: Option<String>,
}

pub(super) fn start<F>(mut driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    deadline: &Deadline, time: &mut F)
    -> Result<FileNativeSupervisedDriver, (FileSupervisedDriver, String)>
where F: FnMut() -> ElapsedTick {
    let prepared = (|| {
        let capture = refresh(&mut driver, config, deadline, time)?;
        let now = time(); deadline.check(now)?;
        let mut previous = now;
        let mut rounds = Vec::new();
        for id in &loaded.rounds {
            let window = ReviewWindow { commit_by: plus(previous, config.timing.commit_ms)?,
                reveal_by: plus(previous, config.timing.reveal_ms)? };
            if !(previous < window.commit_by && window.commit_by < window.reveal_by
                && window.reveal_by < deadline.logical) {
                return Err("insufficient learned review lifetime for the complete fixed schedule".into());
            }
            rounds.push(LearnedWorkerRound { round: *id, evidence_root: capture.reference_root(), window });
            previous = window.reveal_by;
        }
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        let FileRequestDisposition::Admitted { attempt, .. } =
            host.request_status(loaded.request).map_err(debug)?.disposition
            else { return Err("learned request was not admitted by its original gateway".into()); };
        let message = host.learned_text_message(loaded.evidence).map_err(debug)?;
        let rows: BTreeSet<_> = message.evidence().audit().source().groups().map(|group| group.row).collect();
        let request = LearnedSidecarRequest { identity: loaded.sidecar,
            priority: loaded.priority.clone(), budget: loaded.disclosure };
        let current = host.learned_generation_inspection().map_err(debug)?;
        let revision = host.revision();
        let sidecar = host.begin_learned_sidecar_plan(revision, attempt, current.numerical.actor_revision,
            request).map_err(debug)?;
        let input = host.current_learned_sidecar(&sidecar).map_err(debug)?;
        let native = loaded.native.as_ref().ok_or("no independently admitted native model roster")?;
        let mut rosters = std::collections::BTreeMap::new();
        for round in &rounds {
            let mut members = std::collections::BTreeMap::new();
            for (name, view) in input.views() {
                let mut queries = Vec::new();
                for row in &rows {
                    let tap = KvTap { layer: row.layer, side: row.side };
                    let probes = loaded.probes.get(&tap).ok_or("missing independently registered K/V probes")?;
                    for probe in probes { queries.push(SidecarProbeQuery { row: *row, probe: probe.clone() }); }
                }
                members.insert(name.clone(), NativeReviewMember {
                    evaluator: native.evaluator(name, view.actual_input().input_profile().clone())?,
                    queries, salt: super::super::super::nonce()?.to_vec(),
                });
            }
            rosters.insert(round.round, members);
        }
        let launch = FileNativeDriverLaunch { journal_revision: host.revision(), request: loaded.request,
            sidecar, rounds, rosters, limits: loaded.native_limits };
        Ok((launch, capture.snapshot().clone(), now))
    })();
    let (launch, snapshot, now) = match prepared {
        Ok(value) => value, Err(error) => return Err((driver, error)),
    };
    driver.start_native_learned_sequence(launch, snapshot, now).map_err(|error| {
        let error = *error; (error.driver, debug(error.error))
    })
}

pub(super) fn run<F>(driver: FileSupervisedDriver, config: &mut Config, loaded: &Loaded,
    reviewer: &FileHumanReviewer, control: &mut Control, deadline: &Deadline, time: &mut F) -> Reviewed
where F: FnMut() -> ElapsedTick {
    let mut run = match start(driver, config, loaded, deadline, time) {
        Ok(run) => run, Err((driver, error)) => return Reviewed {
            driver, history: None, stop_requested: false, failure: Some(error) },
    };
    let mut stop_requested = false;
    let result = (|| {
        for _ in 0..loaded.native_limits.polls {
            deadline.check(time())?;
            // Preserve the original private native owner. Only an authenticated
            // stop peer triggers cancellation and terminal handoff; the existing
            // driver then handles its original nonce-bound stop application.
            if control.pending_peer()? {
                stop_requested = true;
                run.cancel(run.review().revision()).map_err(debug)?;
                break;
            }
            if run.review().status() != NativeReviewStatus::Running { break; }
            let round = run.review().round().round;
            let before = run.review().records().get(&round)
                .map(|record| (record.members.clone(), record.workers.clone()));
            run.step_from_policy_file(&mut config.source, &mut *time, None).result.map_err(debug)?;
            let after = run.review().records().get(&round)
                .map(|record| (record.members.clone(), record.workers.clone()));
            if run.review().status() == NativeReviewStatus::Running && before == after
                && wait_for_phase(&mut run, control, config.timing.poll_ms, deadline, time)? {
                stop_requested = true;
                break;
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
            failure = Some(format!("{}; original native cancellation: {error:?}",
                failure.unwrap_or_else(|| "native review did not finish".into())));
        }
    }
    // cancel() retires original numerical custody before fallible journal work.
    // This owner therefore has no running review to detach from its driver.
    let handoff = run.into_handoff().expect("finished or cancelled original native review");
    let mut driver = handoff.driver;
    if stop_requested {
        match control.checkpoint(&mut driver, reviewer, deadline, time) {
            Ok(true) => {}
            Ok(false) => failure = Some("authenticated stop peer did not complete the original stop".into()),
            Err(error) => failure = Some(error),
        }
    }
    Reviewed { driver, history: Some(handoff.review), stop_requested, failure }
}

// Polls and review revisions advance even when the protocol cannot. The caller
// compares only ORIGINAL member progress and worker phases before entering here.
// Waiting spends no numerical quantum or source lease; the next real quantum
// reacquires policy and time through the original owner.
pub(super) fn wait_for_phase<F>(run: &mut FileNativeSupervisedDriver, control: &mut Control,
    poll_ms: u64, deadline: &Deadline, time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let due = run.next_review_deadline().ok_or("running native review has no original phase deadline")?;
    loop {
        let now = time(); deadline.check(now)?;
        if control.pending_peer()? {
            run.cancel(run.review().revision()).map_err(debug)?;
            return Ok(true);
        }
        if now >= due { return Ok(false); }
        // Bound stop responsiveness independently of an unusually large poll
        // interval. Never advance a logical clock to simulate elapsed time.
        super::super::super::pause(poll_ms.min(50).max(1).min(due.0 - now.0));
    }
}
