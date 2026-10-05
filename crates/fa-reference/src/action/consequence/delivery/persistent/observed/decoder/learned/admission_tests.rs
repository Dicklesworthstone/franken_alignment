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
