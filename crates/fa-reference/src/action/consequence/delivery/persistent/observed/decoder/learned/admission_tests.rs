//! Original numerical owners and real journals, including the ordinary/recovery
//! boundary. These controls do not qualify monitor quality or durable hardware.
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::RecoveryReserve;

fn limited_owner(root: &Directory, reserve: bool, slots: usize) -> FileOversight {
    let mut profile = profile();
    let bootstrap = if reserve { 3 } else { 2 };
    let tail = if reserve { RecoveryReserve::terminal().events } else { 0 };
    profile.delivery.limits.events = bootstrap + slots + tail;
    let (mut host, _) = FileOversight::create(root.store(), profile).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    if reserve {
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    }
    host.enable_learned_generation(host.revision(), config(false, 1)).unwrap();
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, slots);
    host
}

#[test]
fn a_lone_ordinary_slot_cannot_admit_an_unfinishable_step_intent() {
    for reserve in [false, true] {
        let root = Directory::new();
        let mut host = limited_owner(&root, reserve, 1);
        let before = host.learned_generation_inspection().unwrap();
        let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        let capacity = host.journal_capacity().unwrap();
        let n = &before.numerical;
        assert_eq!(host.begin_learned_step(host.revision(), n.actor_revision, n.position),
            Err(JournalError::Contract(Error::Limit)));
        assert_eq!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).err(),
            Some(JournalError::Contract(Error::Limit)));
        assert_eq!(host.learned_generation_inspection().unwrap(), before);
        assert_eq!(host.journal_capacity().unwrap(), capacity);
        assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
        assert!(host.storage_failure().is_none());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn exactly_two_ordinary_slots_complete_original_inference_without_spending_the_tail() {
    for reserve in [false, true] {
        let root = Directory::new();
        let mut host = limited_owner(&root, reserve, 2);
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
        assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 1);
        host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
        let after = host.learned_generation_inspection().unwrap();
        assert!(after.pending.is_none());
        assert_eq!(after.numerical.position, n.position + 1);
        assert_eq!(after.numerical.work.admitted_tokens, n.work.admitted_tokens + 1);
        assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
        let remaining = host.journal_capacity().unwrap().remaining().events;
        assert_eq!(remaining, if reserve { RecoveryReserve::terminal().events } else { 0 });
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn intervening_work_cannot_make_completion_compute_into_terminal_recovery_space() {
    let root = Directory::new();
    let mut host = limited_owner(&root, true, 3);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
    let before = host.learned_generation_inspection().unwrap();
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    assert_eq!(host.complete_learned_step(host.revision(), n.actor_revision, n.position).err(),
        Some(JournalError::Contract(Error::Limit)));
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    assert_eq!(before.pending, Some(LearnedStepIntent { actor_revision: n.actor_revision, position: n.position }));
    assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
    assert!(host.storage_failure().is_none());
    assert!(host.journal_capacity().unwrap().terminal_space_remaining());
}

#[test]
fn stale_and_poisoned_owners_refuse_before_reporting_capacity() {
    let root = Directory::new();
    let mut host = limited_owner(&root, true, 1);
    let before = host.learned_generation_inspection().unwrap();
    let n = &before.numerical;
    assert_eq!(host.begin_learned_step(host.revision() - 1, n.actor_revision, n.position),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    host.store.fail_once(JournalIo::Stage);
    assert!(matches!(host.observe_time(host.revision(), ElapsedTick(2)), Err(JournalError::Io(_))));
    assert_eq!(host.begin_learned_step(host.revision(), n.actor_revision, n.position),
        Err(JournalError::Unavailable));
    assert_eq!(host.revision(), before.journal_revision);
}

use crate::action::consequence::activation::tensor::kv::decoder::monitoring::restart::KvRestartBudget;
use crate::action::consequence::activation::tensor::kv::model::MAX_MODEL_KV_VALUES;
use crate::action::consequence::delivery::persistent::observed::containment::FileResetRequest;
use crate::action::consequence::gate::ReviewBinding;

fn reset_owner(root: &Directory, reserve: bool, slots: usize)
    -> (FileOversight, checkpoint::FileLearnedCheckpoint, FileResetRequest, KvRestartBudget)
{
    let mut host = limited_owner(root, reserve, slots + 5);
    step(&mut host).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    let saved = host.capture_learned_checkpoint(host.revision(), 1, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap();
    step(&mut host).unwrap();
    let state = host.inspect();
    let request = FileResetRequest { operation: 900, expected_control_sequence: state.control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: state.control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![state.target] };
    let budget = KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: host.machine.broker.hosted_learned_original().unwrap().policy().allowance() };
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, slots);
    (host, saved, request, budget)
}

#[test]
fn a_lone_ordinary_slot_cannot_admit_a_checkpoint_reset_intent() {
    for reserve in [false, true] {
        let root = Directory::new();
        let (mut host, saved, request, budget) = reset_owner(&root, reserve, 1);
        let before = host.learned_generation_inspection().unwrap();
        let usage = host.learned_recovery_usage().unwrap();
        let actor = host.actor_snapshot().unwrap();
        let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        assert_eq!(host.begin_learned_reset(host.revision(), &saved, request.clone(), budget),
            Err(JournalError::Contract(Error::Limit)));
        assert_eq!(host.reset_learned_checkpoint(host.revision(), &saved, request, budget).err(),
            Some(JournalError::Contract(Error::Limit)));
        assert!(host.pending_learned_reset().unwrap().is_none());
        assert_eq!(host.learned_generation_inspection().unwrap(), before);
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        assert_eq!(host.actor_snapshot().unwrap(), actor);
        assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
        assert!(host.storage_failure().is_none());
    }
}

#[test]
fn exactly_two_slots_restore_the_original_checkpoint_and_historical_retries_need_no_space() {
    for reserve in [false, true] {
        let root = Directory::new();
        let (mut host, saved, request, budget) = reset_owner(&root, reserve, 2);
        host.begin_learned_reset(host.revision(), &saved, request.clone(), budget).unwrap();
        assert!(host.pending_learned_reset().unwrap().is_some());
        let receipt = host.complete_learned_reset(host.revision(), 900).unwrap().unwrap();
        assert!(receipt.control.restored);
        assert_eq!(receipt.control.incident_count, 1);
        assert!(host.pending_learned_reset().unwrap().is_none());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, saved.info().position);
        assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
        assert_eq!(host.journal_capacity().unwrap().remaining().events,
            if reserve { RecoveryReserve::terminal().events } else { 0 });
        let before = host.learned_generation_inspection().unwrap();
        let usage = host.learned_recovery_usage().unwrap();
        let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        host.begin_learned_reset(0, &saved, request.clone(), budget).unwrap();
        assert_eq!(host.reset_learned_checkpoint(0, &saved, request, budget).unwrap().unwrap().control, receipt.control);
        assert_eq!(host.complete_learned_reset(0, 900).unwrap().unwrap().control, receipt.control);
        assert_eq!(host.learned_generation_inspection().unwrap(), before);
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}
