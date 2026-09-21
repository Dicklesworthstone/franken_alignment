//! Native lease and history laws; durable effect-path tests live separately.
use super::*;
use super::super::PublicationFreshnessPolicy;
use crate::action::consequence::delivery::publication_gate::{PublicationGate, PublicationLimits};
use crate::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy};
use crate::witness::refinement::RefinementBudget;
use crate::witness::refinement::index::routing::{InvalidationIndex, RoutingBudget, RoutingLimits, WitnessChange};
use std::collections::BTreeMap;

fn cut(through: u64) -> PublicationInputCut { PublicationInputCut { source: 41, through } }
fn pulse(generation: u64, through: u64, produced: u64) -> PublicationHeartbeat {
    PublicationHeartbeat { source: 41, clock_domain: 99, generation, through, produced_at: ElapsedTick(produced) }
}
fn state(enabled: bool) -> ChangeState {
    let mut freshness = FreshnessState::new(PublicationFreshnessPolicy { clock_domain: 99, max_age_ticks: 3 });
    freshness.snapshot_fallback = enabled;
    ChangeState { policy: PublicationChangePolicy { source: 41, after: 0, lookup: RoutingBudget { steps: 1000, bytes: 100000 } },
        status: PublicationChangeStatus { source: 41, through: 0, observed_through: 0, unavailable: false },
        index: InvalidationIndex::new(RoutingLimits { judgments: 1, dependencies: 1 }).unwrap(),
        last: None, freshness: Some(freshness) }
}
fn acquire(s: &mut ChangeState, h: PublicationHeartbeat, now: Option<u64>, epoch: u64) {
    assert_eq!(s.observe_heartbeat(h, now.map(ElapsedTick), epoch).unwrap().eligibility, Err(Error::Incomplete));
}
fn eligible(s: &ChangeState, through: u64, now: u64, epoch: u64) -> Result<(), Error> {
    s.snapshot_current(Some(ElapsedTick(now)), epoch, Some(cut(through)))
}

#[test]
fn explicit_fallback_never_marks_missing_notifications_complete() {
    for enabled in [false, true] {
        let mut s = state(enabled);
        acquire(&mut s, pulse(1, 300, 1), Some(1), 7);
        assert_eq!(s.status.through, 0);
        assert_eq!(s.status.observed_through, 300);
        assert!(!s.status.complete());
        assert_eq!(s.current(Some(ElapsedTick(1)), 7), Err(Error::Incomplete));
        assert_eq!(s.freshness.as_ref().unwrap().acquired_epoch, None);
        assert_eq!(eligible(&s, 300, 1, 7), if enabled { Ok(()) } else { Err(Error::Incomplete) });
        assert!(s.last.is_none(), "no synthetic invalidation report");
    }
}

#[test]
fn only_a_source_bound_snapshot_at_the_exact_observed_head_is_eligible() {
    let mut s = state(true);
    acquire(&mut s, pulse(1, 300, 1), Some(1), 7);
    for through in [0, 299, 301, u64::MAX] { assert_eq!(eligible(&s, through, 1, 7), Err(Error::Incomplete)); }
    assert_eq!(eligible(&s, 300, 1, 7), Ok(()));
    assert_eq!(s.snapshot_current(Some(ElapsedTick(1)), 7, None), Err(Error::Incomplete));
    assert_eq!(s.snapshot_current(Some(ElapsedTick(1)), 7, Some(PublicationInputCut { source: 42, through: 300 })), Err(Error::Binding));
    // Another observed future notice is not included in the acquired snapshot.
    s.status.observed_through = 301;
    assert!(eligible(&s, 300, 1, 7).is_err());
    assert!(eligible(&s, 301, 1, 7).is_err());
}

#[test]
fn snapshot_freshness_matches_half_open_producer_time_intervals() {
    for age in 1..=5_u64 {
        for produced in 0..=5_u64 {
            for observed in 0..=12_u64 {
                let mut s = state(true);
                s.freshness.as_mut().unwrap().policy.max_age_ticks = age;
                acquire(&mut s, pulse(1, 300, produced), Some(observed), 7);
                assert_eq!(eligible(&s, 300, observed, 7).is_ok(),
                    (produced..produced + age).contains(&observed));
            }
        }
    }
    let mut s = state(true);
    acquire(&mut s, pulse(1, 300, 1), Some(3), 7);
    assert_eq!(eligible(&s, 300, 4, 7), Err(Error::Stale));
}

#[test]
fn future_clockless_and_expired_reads_never_activate_when_only_time_changes() {
    for now in [None, Some(0), Some(4)] {
        let mut s = state(true);
        acquire(&mut s, pulse(1, 300, 1), now, 7);
        assert!(eligible(&s, 300, 2, 7).is_err());
        acquire(&mut s, pulse(1, 300, 1), Some(2), 7);
        assert_eq!(eligible(&s, 300, 2, 7), Ok(()));
    }
}

#[test]
fn equivocation_and_rollback_do_not_inherit_an_earlier_snapshot_lease() {
    let mut s = state(true); let h = pulse(5, 300, 10);
    acquire(&mut s, h, Some(10), 7);
    assert_eq!(eligible(&s, 300, 10, 7), Ok(()));
    for bad in [pulse(4, 300, 10), pulse(5, 300, 11), pulse(6, 299, 11)] {
        assert!(s.observe_heartbeat(bad, Some(ElapsedTick(11)), 7).unwrap().eligibility.is_err());
        assert!(eligible(&s, 300, 11, 7).is_err());
    }
    s.freshness.as_mut().unwrap().withdraw();
    assert!(s.observe_heartbeat(h, Some(ElapsedTick(11)), 7).unwrap().eligibility.is_err());
    assert!(eligible(&s, 300, 11, 7).is_err());
    acquire(&mut s, pulse(6, 300, 11), Some(11), 7);
    assert_eq!(eligible(&s, 300, 11, 7), Ok(()));
}

#[test]
fn withdrawal_and_recovery_require_reacquisition_not_historical_eligibility() {
    let mut s = state(true); let h = pulse(1, 300, 1);
    acquire(&mut s, h, Some(1), 7);
    assert_eq!(eligible(&s, 300, 2, 8), Err(Error::Stale));
    s.freshness.as_mut().unwrap().withdraw();
    assert_eq!(s.freshness.as_ref().unwrap().last, Some(h));
    assert_eq!(eligible(&s, 300, 2, 7), Err(Error::Incomplete));
    acquire(&mut s, h, Some(2), 8);
    assert_eq!(eligible(&s, 300, 2, 8), Ok(()));
}

#[test]
fn real_tail_repair_still_requires_a_new_heartbeat_for_selective_validation() {
    let mut s = state(true);
    acquire(&mut s, pulse(1, 2, 1), Some(1), 7);
    let mut gate = PublicationGate { limits: PublicationLimits { bindings: 1,
        validation: RefinementBudget::default() }, slots: BTreeMap::new(), changes: Some(s) };
    for sequence in [1, 2] {
        gate.apply_change(PublicationChange { source: 41, sequence, change: WitnessChange::All }).unwrap();
    }
    let s = gate.changes.as_mut().unwrap();
    assert!(s.complete());
    assert_eq!(s.current(Some(ElapsedTick(2)), 7), Err(Error::Incomplete));
    assert_eq!(eligible(s, 2, 2, 7), Err(Error::Incomplete));
    assert_eq!(s.observe_heartbeat(pulse(1, 2, 1), Some(ElapsedTick(2)), 7).unwrap().eligibility, Ok(()));
}

#[test]
fn unavailable_index_and_overflow_cannot_use_exact_fallback() {
    let mut s = state(true);
    acquire(&mut s, pulse(1, u64::MAX, 1), Some(1), 7);
    assert_eq!(eligible(&s, u64::MAX, 1, 7), Ok(()));
    s.status.unavailable = true;
    assert_eq!(eligible(&s, u64::MAX, 1, 7), Err(Error::Incomplete));
    let mut s = state(true);
    acquire(&mut s, pulse(1, 300, u64::MAX - 2), Some(u64::MAX - 1), 7);
    assert!(eligible(&s, 300, u64::MAX - 1, 7).is_err());
    assert_eq!(s.status.through, 0);
}
