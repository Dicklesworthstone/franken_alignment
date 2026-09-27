//! Original sidecar outcomes remain readable without recovering live authority.
use super::*;
use fa_reference::action::consequence::activation::probe::learned::{KvGroup, KvRow};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::sidecar::{
    FileLearnedSidecar, FileLearnedSidecarFinish as Finish, FileLearnedSidecarOutcome as Outcome,
};
use fa_reference::action::consequence::oversight::sidecar::SidecarRefinementOutcome;

fn group() -> KvGroup { KvGroup { row: KvRow { layer: 1, side: KvSide::Value, position: 1 }, head: 0 } }
fn planned(h: &mut Host, budget: SidecarCongressBudget) -> FileLearnedSidecar {
    let mut r = request(1); r.priority = vec![group()]; r.budget = budget;
    let n = h.learned_generation_inspection().unwrap().numerical;
    h.begin_learned_sidecar_plan(h.revision(), 1, n.actor_revision, r).unwrap()
}
fn prepared() -> (Directory, Host, FileHumanReviewer, FrozenAction, FileLearnedSidecar) {
    let root = Directory::new(); let (mut h, reviewer) = owner(&root, &config(true));
    step(&mut h); step(&mut h); let action = propose(&mut h, 1);
    let plan = planned(&mut h, SidecarCongressBudget::default());
    (root, h, reviewer, action, plan)
}
fn finish(h: &mut Host, plan: &mut FileLearnedSidecar, round: u64, vote: Verdict) -> Finish {
    ballot(h, 1, round, vote);
    h.finish_learned_sidecar_review(h.revision(), plan, round, snapshot()).unwrap()
}
fn same_result(a: &Finish, b: &Finish) {
    assert_eq!(a.archive(), b.archive());
    match (a, b) {
        (Finish::Refined { group: ag, input_revision: ai, .. },
            Finish::Refined { group: bg, input_revision: bi, .. }) => { assert_eq!(ag, bg); assert_eq!(ai, bi); }
        (Finish::Applied { outcome: ao, receipt: ar, .. },
            Finish::Applied { outcome: bo, receipt: br, .. }) => { assert_eq!(ao, bo); assert_eq!(ar, br); }
        _ => panic!("different original result variants"),
    }
}
fn same(a: &Outcome, b: &Outcome) {
    assert_eq!((a.attempt, a.actor_revision, a.input_revision, a.round),
        (b.attempt, b.actor_revision, b.input_revision, b.round));
    same_result(&a.result, &b.result);
}
fn read(root: &Directory, round: u64) -> Result<Outcome, JournalError> {
    Host::read_learned_sidecar_outcome(root.store(), &profile(), &config(true), round)
}

#[test]
fn original_refinement_and_application_remain_exact_after_publication_and_restart() {
    let (root, mut h, reviewer, action, mut plan) = prepared();
    let coarse = h.current_learned_sidecar(&plan).unwrap().clone();
    let actor_revision = plan.actor_revision(); let input_revision = plan.input_revision();
    let refined = finish(&mut h, &mut plan, 101, Verdict::Abstain);
    assert!(matches!(refined, Finish::Refined { .. }));
    let first = h.learned_sidecar_outcome(101).unwrap();
    assert_eq!((first.attempt, first.round, first.actor_revision, first.input_revision),
        (1, 101, actor_revision, input_revision));
    same_result(&first.result, &refined);
    assert_eq!(first.result.archive().inputs.as_ref(), &coarse);
    let rich = h.current_learned_sidecar(&plan).unwrap().clone();
    assert_ne!(rich, coarse); assert_eq!(plan.input_revision(), input_revision + 1);
    let result = finish(&mut h, &mut plan, 102, Verdict::Allow);
    assert!(matches!(result, Finish::Applied { receipt: Ok(_), .. }));
    let second = h.learned_sidecar_outcome(102).unwrap(); same_result(&second.result, &result);
    assert_eq!(second.input_revision, plan.input_revision());
    let (automatic, human) = authorize(&mut h, &reviewer, 1, &rich);
    h.dispatch(h.revision(), &automatic, &human, &action, &rich, snapshot()).unwrap();
    h.publish_checked(h.revision(), 1, Some(&rich), snapshot(), ElapsedTick(2)).unwrap();
    h.reconcile(h.revision(), 1).unwrap(); drop(h);
    let (mut h, _) = Host::open_with_learned_generation(root.store(), profile(), &config(true)).unwrap();
    same(&h.learned_sidecar_outcome(101).unwrap(), &first);
    same(&h.learned_sidecar_outcome(102).unwrap(), &second);
    same(&read(&root, 101).unwrap(), &first); same(&read(&root, 102).unwrap(), &second);
    assert!(!h.clock_ready()); assert!(h.learned_generation_inspection().unwrap().paused);
    assert!(h.current_learned_sidecar(&plan).is_err());
    assert!(h.dispatch(h.revision(), &automatic, &human, &action, &rich, snapshot()).is_err());
    assert_eq!(h.inspect().executions, 1); assert_eq!(h.inspect().control.ledger.charged, 16);
}

#[test]
fn read_only_outcome_verification_neither_takes_the_writer_lock_nor_claims_an_effect() {
    let (root, mut h, reviewer, action, mut plan) = prepared();
    finish(&mut h, &mut plan, 101, Verdict::Allow);
    let result = h.learned_sidecar_outcome(101).unwrap();
    let input = h.current_learned_sidecar(&plan).unwrap().clone();
    let (automatic, human) = authorize(&mut h, &reviewer, 1, &input);
    h.dispatch(h.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    let bytes = root.bytes(); let before = h.inspect();
    let numerical = h.learned_generation_inspection().unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"not a canonical outcome").unwrap();
    for _ in 0..2 { same(&read(&root, 101).unwrap(), &result); }
    assert_eq!(root.bytes(), bytes); assert_eq!(h.inspect(), before);
    assert_eq!(h.learned_generation_inspection().unwrap(), numerical);
    assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"not a canonical outcome");
    assert!(matches!(Host::open_with_learned_generation(root.store(), profile(), &config(true)), Err(JournalError::Busy)));
    assert_eq!(h.inspect().executions, 0); assert_eq!(h.inspect().control.ledger.charged, 16);
    assert!(read(&root, 999).is_err());
    assert_eq!(Host::read_learned_sidecar_outcome(root.store(), &profile(), &config(false), 101).err(),
        Some(JournalError::Contract(Error::Binding)));
    drop(h);
    let (h, _) = Host::open_with_learned_generation(root.store(), profile(), &config(true)).unwrap();
    same(&h.learned_sidecar_outcome(101).unwrap(), &result);
    assert_eq!(h.inspect().executions, 0); assert_eq!(h.inspect().control.ledger.charged, 16);
}

#[test]
fn source_changes_retire_live_handles_but_do_not_rewrite_acknowledged_outcomes() {
    let (root, mut h, _, _, mut plan) = prepared();
    finish(&mut h, &mut plan, 101, Verdict::Abstain);
    let result = h.learned_sidecar_outcome(101).unwrap();
    ballot(&mut h, 1, 102, Verdict::Allow); step(&mut h);
    assert!(h.current_learned_sidecar(&plan).is_err());
    assert!(h.finish_learned_sidecar_review(h.revision(), &mut plan, 102, snapshot()).is_err());
    same(&h.learned_sidecar_outcome(101).unwrap(), &result); same(&read(&root, 101).unwrap(), &result);
    assert_eq!(h.learned_sidecar_outcome(102).err(), Some(JournalError::Contract(Error::Missing)));
    assert_eq!(h.inspect().executions, 0);
}

#[test]
fn unavailable_refinement_and_missing_workers_are_retained_as_distinct_original_results() {
    for missing in [false, true] {
        let root = Directory::new(); let (mut h, _) = owner(&root, &config(true));
        step(&mut h); step(&mut h); propose(&mut h, 1);
        let mut plan = planned(&mut h, SidecarCongressBudget { rounds: 1, ..SidecarCongressBudget::default() });
        if missing {
            h.begin_review(h.revision(), 1, 101, [9; 32], window(), snapshot()).unwrap();
            assert!(h.finish_learned_sidecar_review(h.revision(), &mut plan, 101, snapshot()).is_err());
            assert_eq!(h.learned_sidecar_outcome(101).err(), Some(JournalError::Contract(Error::Missing)));
            h.observe_time(h.revision(), ElapsedTick(30)).unwrap();
        } else { ballot(&mut h, 1, 101, Verdict::Abstain); }
        let result = h.finish_learned_sidecar_review(h.revision(), &mut plan, 101, snapshot()).unwrap();
        let Finish::Applied { outcome: Some(outcome), receipt: Ok(_), .. } = &result else { panic!("original held result"); };
        if missing { assert!(matches!(outcome, SidecarRefinementOutcome::Missing { .. })); }
        else { assert!(matches!(outcome, SidecarRefinementOutcome::BudgetExhausted { .. })); }
        let record = h.learned_sidecar_outcome(101).unwrap(); same_result(&record.result, &result);
        let input = h.current_learned_sidecar(&plan).unwrap().clone();
        assert!(h.authorize(h.revision(), 1, &input, snapshot()).is_err());
        drop(h); same(&read(&root, 101).unwrap(), &record);
    }
}

#[test]
fn original_policy_application_refusal_is_not_erased_or_promoted_during_recovery() {
    let root = Directory::new(); let mut p = profile();
    p.delivery.policy = Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap();
    let (mut h, _) = Host::create_guarded_with_learned_generation(root.store(), p.clone(), &guards(), None, config(true)).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); step(&mut h); step(&mut h);
    let good = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) };
    h.propose(h.revision(), 1, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(target()), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: h.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, good.clone()).unwrap();
    let mut plan = planned(&mut h, SidecarCongressBudget::default());
    h.begin_review(h.revision(), 1, 101, [9; 32], window(), good).unwrap();
    h.commit_review(h.revision(), 101, "reviewer", commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    h.open_reveals(h.revision(), 101).unwrap();
    h.reveal_review(h.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    let changed = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"revoked".to_vec())]) };
    let result = h.finish_learned_sidecar_review(h.revision(), &mut plan, 101, changed).unwrap();
    assert!(matches!(result, Finish::Applied { receipt: Err(_), .. }));
    let record = h.learned_sidecar_outcome(101).unwrap(); same_result(&record.result, &result);
    assert_eq!(h.inspect().executions, 0); drop(h);
    same(&Host::read_learned_sidecar_outcome(root.store(), &p, &config(true), 101).unwrap(), &record);
    let (h, _) = Host::open_with_learned_generation(root.store(), p, &config(true)).unwrap();
    same(&h.learned_sidecar_outcome(101).unwrap(), &record);
    assert_eq!(h.inspect().executions, 0);
}

#[test]
fn failed_completion_write_cannot_expose_a_candidate_outcome() {
    let (root, mut h, _, _, mut plan) = prepared();
    ballot(&mut h, 1, 101, Verdict::Abstain);
    let bytes = root.bytes(); let input_revision = plan.input_revision();
    std::fs::write(root.store().join("delivery.pending"), b"unresolved staging").unwrap();
    assert!(h.finish_learned_sidecar_review(h.revision(), &mut plan, 101, snapshot()).is_err());
    assert!(h.storage_failure().is_some()); assert_eq!(plan.input_revision(), input_revision);
    assert_eq!(h.learned_sidecar_outcome(101).err(), Some(JournalError::Unavailable));
    assert_eq!(root.bytes(), bytes);
    assert_eq!(read(&root, 101).err(), Some(JournalError::Contract(Error::Missing)));
    drop(h);
    let (mut h, _) = Host::open_with_learned_generation(root.store(), profile(), &config(true)).unwrap();
    assert_eq!(h.learned_sidecar_outcome(101).err(), Some(JournalError::Contract(Error::Missing)));
    let n = h.learned_generation_inspection().unwrap().numerical;
    h.observe_time(h.revision(), ElapsedTick(2)).unwrap();
    h.resume_learned_generation(h.revision(), n.actor_revision, n.position).unwrap();
    propose(&mut h, 2);
    let mut fresh = h.begin_learned_sidecar_plan(h.revision(), 2, n.actor_revision, request(2)).unwrap();
    ballot(&mut h, 2, 102, Verdict::Allow);
    let result = h.finish_learned_sidecar_review(h.revision(), &mut fresh, 102, snapshot()).unwrap();
    same_result(&h.learned_sidecar_outcome(102).unwrap().result, &result);
}

#[test]
fn an_old_valid_completion_does_not_hide_a_corrupted_later_witness() {
    let (root, mut h, _, _, mut plan) = prepared();
    finish(&mut h, &mut plan, 101, Verdict::Abstain);
    finish(&mut h, &mut plan, 102, Verdict::Allow);
    let first = read(&root, 101).unwrap(); same(&h.learned_sidecar_outcome(101).unwrap(), &first);
    let mut bytes = root.bytes();
    let offsets: Vec<_> = bytes.windows(8).enumerate().filter_map(|(i, w)| (w == b"FALSFIN\x01").then_some(i)).collect();
    assert_eq!(offsets.len(), 2);
    bytes[offsets[1] + 8] ^= 1; // Change only original comparison material in the later event.
    drop(h); std::fs::write(root.store().join("delivery.bin"), &bytes).unwrap();
    assert!(read(&root, 101).is_err()); assert!(read(&root, 102).is_err());
    assert!(Host::open_with_learned_generation(root.store(), profile(), &config(true)).is_err());
    assert_eq!(root.bytes(), bytes);
}
