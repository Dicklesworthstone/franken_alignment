use super::*;
use super::super::{PublicationChange, PublicationChangePolicy, PublicationChangeStatus};
use super::super::super::{PublicationGate, PublicationLimits};
use crate::witness::refinement::index::routing::{InvalidationIndex, RoutingBudget, RoutingLimits, WitnessChange};
use crate::witness::refinement::RefinementBudget;
use std::collections::BTreeMap;

fn policy() -> PublicationFreshnessPolicy { PublicationFreshnessPolicy { clock_domain: 99, max_age_ticks: 3 } }
fn pulse(generation: u64, through: u64, tick: u64) -> PublicationHeartbeat {
    PublicationHeartbeat { source: 41, clock_domain: 99, generation, through, produced_at: ElapsedTick(tick) }
}
fn state() -> ChangeState {
    ChangeState { policy: PublicationChangePolicy { source: 41, after: 0,
        lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 } },
        status: PublicationChangeStatus { source: 41, through: 0, observed_through: 0, unavailable: false },
        index: InvalidationIndex::new(RoutingLimits { judgments: 1, dependencies: 1 }).unwrap(),
        last: None, freshness: Some(FreshnessState::new(policy())) }
}
fn observe(state: &mut ChangeState, heartbeat: PublicationHeartbeat, now: u64) -> Result<(), Error> {
    state.observe_heartbeat(heartbeat, Some(ElapsedTick(now)), 7).unwrap().eligibility
}

#[test]
fn repeated_observation_never_moves_the_producer_anchored_deadline() {
    let mut s = state(); let h = pulse(1, 0, 1);
    assert_eq!(s.current(Some(ElapsedTick(1)), 7), Err(Error::Incomplete));
    assert_eq!(observe(&mut s, h, 1), Ok(()));
    assert_eq!(observe(&mut s, h, 3), Ok(()));
    assert_eq!(s.current(Some(ElapsedTick(4)), 7), Err(Error::Stale));
    assert_eq!(observe(&mut s, h, 4), Err(Error::Stale));
    assert_eq!(s.freshness.as_ref().unwrap().last, Some(h));
    assert_eq!(observe(&mut s, pulse(2, 0, 4), 4), Ok(()));
}

#[test]
fn all_small_age_boundaries_match_an_independent_half_open_interval() {
    for age in 1..=8_u64 {
        for produced in 0..=8_u64 {
            for now in 0..=20_u64 {
                let mut s = state(); s.freshness.as_mut().unwrap().policy.max_age_ticks = age;
                let actual = observe(&mut s, pulse(1, 0, produced), now);
                let expected = if (produced..produced + age).contains(&now) { Ok(()) } else { Err(Error::Stale) };
                assert_eq!(actual, expected, "age={age} produced={produced} now={now}");
            }
        }
    }
}

#[test]
fn a_future_or_clockless_observation_never_becomes_live_without_another_acquisition() {
    let mut s = state(); let h = pulse(1, 0, 2);
    assert_eq!(observe(&mut s, h, 1), Err(Error::Stale));
    assert_eq!(s.current(Some(ElapsedTick(2)), 7), Err(Error::Stale));
    assert_eq!(observe(&mut s, h, 2), Ok(()));
    s.freshness.as_mut().unwrap().withdraw();
    assert_eq!(s.observe_heartbeat(h, None, 7).unwrap().eligibility, Err(Error::Incomplete));
    assert_eq!(s.current(Some(ElapsedTick(3)), 7), Err(Error::Incomplete));
    assert_eq!(observe(&mut s, h, 3), Ok(()));
}

#[test]
fn generation_equivocation_cannot_be_repaired_with_the_old_quiet_copy() {
    let mut s = state(); let original = pulse(5, 0, 10);
    assert_eq!(observe(&mut s, original, 10), Ok(()));
    assert_eq!(observe(&mut s, pulse(4, 0, 10), 10), Err(Error::Stale));
    assert_eq!(observe(&mut s, pulse(5, 0, 11), 11), Err(Error::Binding));
    s.freshness.as_mut().unwrap().withdraw();
    assert_eq!(observe(&mut s, original, 11), Err(Error::Binding));
    assert_eq!(observe(&mut s, pulse(6, 0, 9), 11), Err(Error::Stale));
    assert_eq!(observe(&mut s, pulse(6, 0, 11), 11), Ok(()));
    assert_eq!(s.freshness.as_ref().unwrap().last, Some(pulse(6, 0, 11)));
}

#[test]
fn producer_ahead_exposes_holes_and_repair_does_not_activate_the_saved_heartbeat() {
    let mut gate = PublicationGate { limits: PublicationLimits { bindings: 1,
        validation: RefinementBudget::default() }, slots: BTreeMap::new(), changes: Some(state()) };
    let future = pulse(1, 2, 1);
    assert_eq!(observe(gate.changes.as_mut().unwrap(), future, 1), Err(Error::Incomplete));
    assert_eq!(gate.changes.as_ref().unwrap().status.through, 0);
    assert_eq!(gate.changes.as_ref().unwrap().status.observed_through, 2);
    for sequence in [1, 2] {
        gate.apply_change(PublicationChange { source: 41, sequence, change: WitnessChange::All }).unwrap();
    }
    let s = gate.changes.as_mut().unwrap();
    assert!(s.complete());
    assert_eq!(s.current(Some(ElapsedTick(2)), 7), Err(Error::Incomplete));
    assert_eq!(observe(s, future, 2), Ok(()));
    // Neither producer coverage nor its time may regress on a new generation.
    assert_eq!(observe(s, pulse(2, 1, 2), 2), Err(Error::Stale));
}

#[test]
fn newer_feed_records_require_a_heartbeat_for_that_exact_complete_prefix() {
    let mut gate = PublicationGate { limits: PublicationLimits { bindings: 1,
        validation: RefinementBudget::default() }, slots: BTreeMap::new(), changes: Some(state()) };
    assert_eq!(observe(gate.changes.as_mut().unwrap(), pulse(1, 0, 1), 1), Ok(()));
    gate.apply_change(PublicationChange { source: 41, sequence: 1, change: WitnessChange::All }).unwrap();
    let s = gate.changes.as_mut().unwrap();
    assert_eq!(s.current(Some(ElapsedTick(1)), 7), Err(Error::Incomplete));
    assert_eq!(observe(s, pulse(2, 1, 1), 1), Ok(()));
}

#[test]
fn recovery_epoch_and_explicit_withdrawal_preserve_history_but_not_current_eligibility() {
    let mut s = state(); let h = pulse(1, 0, 1);
    assert_eq!(observe(&mut s, h, 1), Ok(()));
    assert_eq!(s.current(Some(ElapsedTick(2)), 8), Err(Error::Stale));
    assert_eq!(s.observe_heartbeat(h, Some(ElapsedTick(2)), 8).unwrap().eligibility, Ok(()));
    s.freshness.as_mut().unwrap().withdraw();
    assert_eq!(s.freshness.as_ref().unwrap().last, Some(h));
    assert_eq!(s.current(Some(ElapsedTick(2)), 8), Err(Error::Incomplete));
    assert_eq!(s.observe_heartbeat(h, Some(ElapsedTick(4)), 8).unwrap().eligibility, Err(Error::Stale));
}

#[test]
fn malformed_identity_clock_and_overflow_have_no_permitting_neighbor() {
    assert_eq!(PublicationFreshnessPolicy { clock_domain: 0, ..policy() }.check(), Err(Error::InvalidInput));
    assert_eq!(PublicationFreshnessPolicy { max_age_ticks: 0, ..policy() }.check(), Err(Error::InvalidInput));
    let mut s = state();
    assert_eq!(s.observe_heartbeat(PublicationHeartbeat { source: 42, ..pulse(1, 0, 1) }, Some(ElapsedTick(1)), 7), Err(Error::Binding));
    assert_eq!(observe(&mut s, PublicationHeartbeat { clock_domain: 100, ..pulse(1, 0, 1) }, 1), Err(Error::Binding));
    assert_eq!(observe(&mut s, pulse(0, 0, 1), 1), Err(Error::InvalidInput));
    assert_eq!(observe(&mut s, pulse(1, 0, u64::MAX - 3), u64::MAX - 1), Ok(()));
    assert_eq!(s.current(Some(ElapsedTick(u64::MAX)), 7), Err(Error::Stale));
    assert_eq!(observe(&mut s, pulse(2, 0, u64::MAX - 2), u64::MAX - 1), Err(Error::Overflow));
}
