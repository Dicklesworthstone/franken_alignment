//! Real decoder activations, computed ballots, durable phases and publication.
//! Fixed fixture probes exercise uncertainty and alarms; they do not establish
//! trained-detector quality, host isolation or hardware crash durability.
mod fixture;
use fixture::*;
use super::*;
use super::super::super::{FileHumanReviewer, FileOversightProfile};
use super::super::super::decoder::learned::FileLearnedConfig;
use crate::action::{ActionState, FrozenAction};
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::oversight::sidecar::SidecarRefinementOutcome;
use crate::round::Verdict;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];

fn begin(host: &mut FileOversight, sidecar: FileLearnedSidecar, mode: u8) -> FileLearnedProbeReview {
    let definitions = members(host, &sidecar, mode);
    host.begin_learned_probe_review(host.revision(), sidecar, schedule(), definitions, limits(), snapshot()).unwrap()
}

#[test]
fn actual_uncertainty_refines_then_computed_votes_enable_both_durable_keys_and_one_publication() {
    let root = Directory::new(); let config = config();
    let (mut host, reviewer) = owner(&root, &config);
    step(&mut host); step(&mut host);
    let (action, sidecar) = propose(&mut host);
    let original_input = host.current_learned_sidecar(&sidecar).unwrap().clone();
    assert!(host.authorize(host.revision(), 1, &original_input, snapshot()).is_err());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let mut run = begin(&mut host, sidecar, 1);
    assert_eq!(run.reservation().evaluations, 10);
    assert_eq!(drive(&mut host, &mut run), FileLearnedProbeStatus::Finished);
    assert!(run.failure().is_none());
    assert!(run.history().len() > 1);
    assert!(matches!(run.history().first(), Some(FileLearnedSidecarFinish::Refined { input_revision: 2, .. })));
    assert!(matches!(run.history().last(), Some(FileLearnedSidecarFinish::Applied { receipt: Ok(_), .. })));
    assert_eq!(run.evaluations(), run.history().len() * 2);
    assert!(run.records().values().any(|record| record.status == Some(ProbeHelperStatus::Judged(Verdict::Abstain))));
    assert!(run.records().values().any(|record| record.work.refined_groups > 0));
    let completed: Vec<_> = run.records().values().filter(|record| record.status.is_some()).collect();
    assert_eq!(completed.len(), run.evaluations());
    assert!(completed.iter().all(|record| record.commitment_queued && record.reveal_queued && record.failure.is_none()));
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().control.decisions.get(&1), Some(&Consequence::Continue));
    let input = run.input().clone();
    assert_ne!(input, original_input);
    assert_eq!(host.input_revision(1).unwrap(), run.input_revision());
    assert!(host.authorize(host.revision(), 1, &original_input, snapshot()).is_err());

    // The finished congress alone cannot publish. The genuine human role can
    // approve this exact input, but publication still awaits the automatic key
    // and the original dispatch that consumes both keys together.
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).is_err());
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(80)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).is_err());
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    assert_eq!(host.inspect().executions, 0);
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap();
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
    let publication = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(publication.payload, b"visible"); assert_eq!(publication.executions, 1);
    drop(host);
    let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(recovered.inspect().executions, 1);
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
    assert!(recovered.learned_sidecar_required());
    assert!(recovered.dispatch(recovered.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
    assert!(recovered.commit_review(recovered.revision(), 101, "alpha", 0).is_err());
}

#[test]
fn one_computed_alarm_holds_while_the_other_member_completes_its_full_roster() {
    let root = Directory::new(); let config = config();
    let (mut host, _) = owner(&root, &config); step(&mut host); step(&mut host);
    let (_, sidecar) = propose(&mut host);
    let mut definitions = members(&host, &sidecar, 0);
    let mut alarm = members(&host, &sidecar, 2);
    definitions.insert("alpha".to_owned(), alarm.remove("alpha").unwrap());
    let mut run = host.begin_learned_probe_review(host.revision(), sidecar, schedule(), definitions, limits(), snapshot()).unwrap();
    assert_eq!(drive(&mut host, &mut run), FileLearnedProbeStatus::Finished);
    assert!(run.records().iter().any(|((_, member), record)| member == "alpha"
        && record.status == Some(ProbeHelperStatus::Judged(Verdict::Hold))));
    assert!(run.records().iter().filter(|((_, member), record)| member == "beta" && record.status.is_some())
        .all(|(_, record)| record.status == Some(ProbeHelperStatus::Judged(Verdict::Allow)) && record.reveal_queued));
    assert!(host.authorize(host.revision(), 1, run.input(), snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn deadline_missing_members_never_purchase_a_residual_or_become_abstentions() {
    let root = Directory::new(); let config = config();
    let (mut host, _) = owner(&root, &config); step(&mut host); step(&mut host);
    let (_, sidecar) = propose(&mut host); let mut run = begin(&mut host, sidecar, 1);
    poll(&mut host, &mut run, 10).unwrap();
    if run.status() == FileLearnedProbeStatus::Running { poll(&mut host, &mut run, 15).unwrap(); }
    assert_eq!(run.status(), FileLearnedProbeStatus::Finished);
    assert_eq!(run.evaluations(), 0);
    assert_eq!(run.input_revision(), 1);
    assert_eq!(run.history().len(), 1);
    let FileLearnedSidecarFinish::Applied { outcome: Some(SidecarRefinementOutcome::Missing { members }), .. }
        = &run.history()[0] else { panic!("missing workers must terminate the original refinement planner"); };
    assert_eq!(members, &["alpha".to_owned(), "beta".to_owned()]);
    assert!(run.records().values().all(|record| record.status.is_none() && record.work.evaluated_probes == 0
        && !record.commitment_queued && !record.reveal_queued));
    assert!(host.authorize(host.revision(), 1, run.input(), snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn source_or_input_loss_stops_before_more_scoring_and_retains_completed_work() {
    for withdraw_input in [false, true] {
        let root = Directory::new(); let config = config();
        let (mut host, _) = owner(&root, &config); step(&mut host);
        let (_, sidecar) = propose(&mut host); let mut run = begin(&mut host, sidecar, 0);
        poll(&mut host, &mut run, 1).unwrap(); poll(&mut host, &mut run, 1).unwrap();
        let before = run.records().clone();
        assert_eq!(run.evaluations(), 2);
        assert_eq!(before.values().map(|record| record.work.evaluated_probes).sum::<usize>(), 2);
        if withdraw_input {
            host.inputs_unavailable(host.revision(), 1, run.input_revision()).unwrap();
        } else { step(&mut host); }
        assert!(poll(&mut host, &mut run, 1).is_err());
        assert_eq!(run.status(), FileLearnedProbeStatus::Failed); assert!(run.failure().is_some());
        assert_eq!(run.evaluations(), 2); assert!(run.history().is_empty());
        for (key, record) in run.records() {
            assert_eq!(record.work, before[key].work);
            assert!(!record.commitment_queued && !record.reveal_queued);
        }
        assert!(host.authorize(host.revision(), 1, run.input(), snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn stale_foreign_cancelled_and_exhausted_calls_do_not_repeat_probes() {
    for exhausted in [false, true] {
        let root = Directory::new(); let config = config();
        let (mut host, _) = owner(&root, &config); step(&mut host);
        let (_, sidecar) = propose(&mut host); let definitions = members(&host, &sidecar, 0);
        let mut plan = schedule(); if exhausted { plan.polls = 2; }
        let mut run = host.begin_learned_probe_review(host.revision(), sidecar, plan, definitions, limits(), snapshot()).unwrap();
        let before = run.records().clone();
        assert_eq!(run.advance(&mut host, 1, ElapsedTick(1), snapshot()), Err(Error::Stale.into()));
        assert_eq!(run.advance(&mut host, 0, ElapsedTick(0), snapshot()), Err(Error::Stale.into()));
        let foreign_root = Directory::new(); let (mut foreign, _) = owner(&foreign_root, &config);
        assert_eq!(run.advance(&mut foreign, 0, ElapsedTick(1), snapshot()), Err(Error::Binding.into()));
        assert_eq!(run.records(), &before); assert_eq!(run.evaluations(), 0); assert_eq!(run.revision(), 0);
        poll(&mut host, &mut run, 1).unwrap(); poll(&mut host, &mut run, 1).unwrap();
        let before = run.records().clone();
        if exhausted {
            assert_eq!(poll(&mut host, &mut run, 1), Err(Error::Limit.into()));
            assert_eq!(run.status(), FileLearnedProbeStatus::Failed);
            assert_eq!(run.failure(), Some(&JournalError::Contract(Error::Limit)));
        } else {
            assert_eq!(run.cancel(0), Err(Error::Stale));
            run.cancel(run.revision()).unwrap();
            assert_eq!(run.status(), FileLearnedProbeStatus::Cancelled);
            assert!(run.failure().is_none());
        }
        assert_eq!(poll(&mut host, &mut run, 1), Err(Error::WrongState.into()));
        assert_eq!(run.evaluations(), 2); assert!(run.history().is_empty());
        for (key, record) in run.records() { assert_eq!(record.work, before[key].work); }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn every_leased_round_refuses_manual_votes_even_after_driver_cancellation_or_loss() {
    let root = Directory::new(); let config = config();
    let (mut host, _) = owner(&root, &config); step(&mut host);
    let (_, sidecar) = propose(&mut host); let mut run = begin(&mut host, sidecar, 0);
    let input = run.input().clone();
    for cancelled in [false, true] {
        if cancelled { run.cancel(run.revision()).unwrap(); }
        for round in 101..106 {
            assert_eq!(host.commit_review(host.revision(), round, "alpha", 0), Err(Error::WrongState.into()));
            assert_eq!(host.open_reveals(host.revision(), round), Err(Error::WrongState.into()));
            assert_eq!(host.reveal_review(host.revision(), round, "alpha", Verdict::Allow, vec![1; 32]), Err(Error::WrongState.into()));
            assert_eq!(host.finish_review(host.revision(), round, Some(&input), snapshot()).err(), Some(Error::WrongState.into()));
        }
    }
    drop(run);
    assert_eq!(host.commit_review(host.revision(), 101, "alpha", 0), Err(Error::WrongState.into()));
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    drop(host);
    let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert!(recovered.commit_review(recovered.revision(), 101, "alpha", 0).is_err());
    assert_eq!(recovered.inspect().executions, 0);
    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
}

#[test]
fn an_unacknowledged_commitment_retains_numerical_work_but_exposes_no_finished_review() {
    for barrier in BARRIERS {
        let root = Directory::new(); let config = config();
        let (mut host, _) = owner(&root, &config); step(&mut host);
        let (_, sidecar) = propose(&mut host); let mut run = begin(&mut host, sidecar, 0);
        for _ in 0..32 {
            let current: Vec<_> = run.records().iter().filter(|((round, _), _)| *round == 101).map(|(_, record)| record).collect();
            if current.iter().all(|record| matches!(record.status, Some(ProbeHelperStatus::Judged(_)))) { break; }
            assert_eq!(poll(&mut host, &mut run, 1).unwrap(), FileLearnedProbeStatus::Running);
        }
        let before = run.records().clone();
        assert_eq!(before.values().filter(|record| matches!(record.status, Some(ProbeHelperStatus::Judged(_)))).count(), 2);
        assert!(before.values().all(|record| !record.commitment_queued && !record.reveal_queued));
        host.store.fail_once(barrier);
        let error = poll(&mut host, &mut run, 1).unwrap_err();
        let JournalError::Io(ref failure) = error else { panic!("expected the selected durable commitment barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(run.failure(), Some(&error));
        assert_eq!(run.status(), FileLearnedProbeStatus::Failed);
        assert_eq!(run.evaluations(), 2); assert!(run.history().is_empty());
        for (key, record) in run.records() { assert_eq!(record.work, before[key].work); assert!(!record.reveal_queued); }
        assert!(!host.clock_ready()); assert!(host.authorize(host.revision(), 1, run.input(), snapshot()).is_err());
        let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
        assert_eq!(disk.executions, 0); assert_eq!(disk.payload, b"initial");
        drop(host);
        let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.inspect().executions, 0);
        assert!(recovered.commit_review(recovered.revision(), 101, "alpha", 0).is_err());
    }
}
