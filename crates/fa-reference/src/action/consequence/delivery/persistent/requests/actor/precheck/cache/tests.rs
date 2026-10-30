//! Differential checks against the original evaluator, not a second policy oracle.
use super::*;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, VERSION, MAX_WITNESS_BYTES};
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use std::collections::BTreeMap;

fn fixture(policy: Policy) -> (PrecheckProjection, FileSourcePolicy, FrozenAction) {
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let target = ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 };
    let action = FrozenAction::freeze(ActionSpec { version: VERSION, scope, target: Some(target),
        payload: b"ok".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100), units: 8 }).unwrap();
    let projection = PrecheckProjection { revision: 1, scope, target, policy,
        snapshot: Snapshot { semantic_epoch: 1, complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]) },
        epoch: 0, now: ElapsedTick(1), available: 100, ceiling: None, stopped: false };
    let source = FileSourcePolicy { source: StateSource { scope, source: 51, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() };
    (projection, source, action)
}
fn policy() -> Policy { Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap() }
fn cache() -> State { State::new(PrecheckCacheLimits::default()).unwrap() }
fn compare(cache: &mut State, p: &PrecheckProjection, source: FileSourcePolicy, action: &FrozenAction)
    -> Result<Option<FilePrecheckOutcome>, Error>
{
    let original = evaluate_uncached(p, action);
    let cached = cache.evaluate(p, source, action);
    assert_eq!(cached, original);
    assert!(cache.status().entries <= cache.limits.entries);
    assert!(cache.status().retained_bytes <= cache.limits.retained_bytes);
    cached
}

#[test]
fn repeated_passes_and_denials_use_the_original_witnessed_result() {
    for value in [b"ok".as_slice(), b"no".as_slice()] {
        let (mut p, source, action) = fixture(policy());
        p.snapshot.values.insert(7, value.to_vec());
        let mut cache = cache();
        let first = compare(&mut cache, &p, source, &action).unwrap();
        assert_eq!(compare(&mut cache, &p, source, &action).unwrap(), first);
        assert_eq!((cache.status().evaluations, cache.status().hits, cache.status().entries), (1, 1, 1));
    }
}

#[test]
fn positive_negative_and_range_witnesses_invalidate_but_unrelated_keys_do_not() {
    let policy = Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::Absent { key: 9 }, Predicate::EmptyRange { start: 10, end: 20 },
        Predicate::All(vec![0, 1, 2])]).unwrap();
    for key in [7, 9, 10, 19, 20, 99] {
        let (mut p, source, action) = fixture(policy.clone()); let mut cache = cache();
        assert_eq!(compare(&mut cache, &p, source, &action).unwrap(), Some(FilePrecheckOutcome::PrecheckPassed));
        p.snapshot.values.insert(key, b"changed".to_vec());
        let changed = compare(&mut cache, &p, source, &action).unwrap();
        if key == 20 || key == 99 {
            assert_eq!(changed, Some(FilePrecheckOutcome::PrecheckPassed));
            assert_eq!((cache.status().hits, cache.status().invalidations), (1, 0));
        } else {
            assert_eq!(changed, Some(FilePrecheckOutcome::DeniedByPolicy));
            assert_eq!((cache.status().hits, cache.status().invalidations), (0, 1));
        }
    }
}

#[test]
fn unselected_boolean_arms_are_still_dependencies() {
    let policy = Policy::new(1, vec![Predicate::PayloadAtMost(100), Predicate::Absent { key: 7 },
        Predicate::Any(vec![0, 1])]).unwrap();
    let (mut p, source, action) = fixture(policy); let mut cache = cache();
    compare(&mut cache, &p, source, &action).unwrap();
    p.snapshot.values.insert(7, b"different".to_vec());
    assert_eq!(compare(&mut cache, &p, source, &action).unwrap(), Some(FilePrecheckOutcome::PrecheckPassed));
    assert_eq!((cache.status().evaluations, cache.status().hits, cache.status().invalidations), (2, 0, 1));
}

#[test]
fn a_new_earlier_range_member_cannot_hide_the_original_witness_byte_limit() {
    let policy = Policy::new(1, vec![Predicate::EmptyRange { start: 10, end: 20 }, Predicate::Not(0)]).unwrap();
    let (mut p, source, action) = fixture(policy); p.snapshot.values.insert(15, vec![1]);
    let mut cache = cache();
    assert_eq!(compare(&mut cache, &p, source, &action).unwrap(), Some(FilePrecheckOutcome::PrecheckPassed));
    // The old positive member remains present. Its Judgment alone still holds,
    // but the ORIGINAL evaluator must now retain the oversized earlier member.
    p.snapshot.values.insert(10, vec![0; MAX_WITNESS_BYTES + 1]);
    assert!(cache.entries.front().unwrap().judgment.valid_at(&p.snapshot).unwrap());
    assert_eq!(compare(&mut cache, &p, source, &action), Err(Error::Limit));
    assert_eq!((cache.status().evaluations, cache.status().hits, cache.status().invalidations), (2, 0, 1));
    assert_eq!(cache.status().entries, 0);
}

#[test]
fn policy_bytes_generations_source_context_and_semantics_never_alias() {
    for dimension in 0..6 {
        let (mut p, mut source, action) = fixture(policy()); let mut cache = cache();
        compare(&mut cache, &p, source, &action).unwrap();
        match dimension {
            0 => p.policy = Policy::new(2, p.policy.nodes().to_vec()).unwrap(),
            1 => p.policy = Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"no".to_vec() }]).unwrap(),
            2 => source.source.generation += 1,
            3 => p.snapshot.semantic_epoch += 1,
            4 => p.epoch += 1,
            _ => p.scope.principal += 1,
        }
        compare(&mut cache, &p, source, &action).unwrap();
        assert_eq!((cache.status().evaluations, cache.status().hits, cache.status().invalidations), (2, 0, 1));
    }
}

#[test]
fn the_entire_frozen_action_is_the_key_not_a_digest_or_query_label() {
    for dimension in 0..11 {
        let (p, source, action) = fixture(policy()); let mut cache = cache();
        compare(&mut cache, &p, source, &action).unwrap();
        let mut spec = action.spec().clone();
        match dimension {
            0 => spec.payload = b"other".to_vec(),
            1 => spec.scope.principal += 1,
            2 => spec.target.as_mut().unwrap().adapter += 1,
            3 => spec.target.as_mut().unwrap().object += 1,
            4 => spec.target.as_mut().unwrap().contract_version += 1,
            5 => spec.target.as_mut().unwrap().expected_version += 1,
            6 => spec.target.as_mut().unwrap().generation += 1,
            7 => spec.policy_epoch += 1,
            8 => spec.deadline.0 += 1,
            9 => spec.units += 1,
            _ => spec.required_witnesses.push(ReadWitness::Exact { key: 8, value: None }),
        }
        compare(&mut cache, &p, source, &FrozenAction::freeze(spec).unwrap()).unwrap();
        assert_eq!((cache.status().evaluations, cache.status().hits), (2, 0));
    }
}

#[test]
fn incomplete_snapshots_neither_hit_nor_populate_even_when_the_root_is_known() {
    for policy in [policy(), Policy::new(1, vec![Predicate::PayloadAtMost(100)]).unwrap()] {
        let (mut p, source, action) = fixture(policy); let mut cache = cache();
        compare(&mut cache, &p, source, &action).unwrap();
        p.snapshot.complete = false;
        compare(&mut cache, &p, source, &action).unwrap();
        compare(&mut cache, &p, source, &action).unwrap();
        assert_eq!((cache.status().evaluations, cache.status().hits, cache.status().entries), (3, 0, 0));
        p.snapshot.complete = true;
        compare(&mut cache, &p, source, &action).unwrap();
        assert_eq!(cache.status().evaluations, 4);
    }
}

#[test]
fn exact_and_one_short_byte_limits_never_change_the_policy_outcome() {
    let (p, source, action) = fixture(policy());
    let evaluation = p.policy.evaluate(&action, &p.snapshot).unwrap();
    let exact = context_bytes(&p.policy).unwrap() + entry_bytes(&action, evaluation.witnesses(), 0).unwrap();
    for bytes in [1, exact - 1, exact] {
        let mut cache = State::new(PrecheckCacheLimits { entries: 1, retained_bytes: bytes }).unwrap();
        compare(&mut cache, &p, source, &action).unwrap();
        compare(&mut cache, &p, source, &action).unwrap();
        assert_eq!(cache.status().hits, u64::from(bytes == exact));
        assert_eq!(cache.status().entries, usize::from(bytes == exact));
    }
}

#[test]
fn bounded_fifo_eviction_recomputes_without_refunding_or_resetting_statistics() {
    let (p, source, action) = fixture(policy());
    let mut cache = State::new(PrecheckCacheLimits { entries: 1, ..PrecheckCacheLimits::default() }).unwrap();
    compare(&mut cache, &p, source, &action).unwrap();
    let mut other = action.spec().clone(); other.payload = b"new".to_vec();
    compare(&mut cache, &p, source, &FrozenAction::freeze(other).unwrap()).unwrap();
    compare(&mut cache, &p, source, &action).unwrap();
    assert_eq!((cache.status().evaluations, cache.status().hits, cache.status().evictions), (3, 0, 2));
    cache.clear_context();
    assert_eq!((cache.status().entries, cache.status().retained_bytes), (0, 0));
    assert_eq!((cache.status().evaluations, cache.status().evictions), (3, 2));
}

#[test]
fn independently_bounded_capacity_rejects_zero_and_one_over_maximum() {
    for limits in [PrecheckCacheLimits { entries: 0, retained_bytes: 1 },
        PrecheckCacheLimits { entries: 1, retained_bytes: 0 }] {
        assert_eq!(limits.validate(), Err(Error::InvalidInput));
    }
    for limits in [PrecheckCacheLimits { entries: MAX_PRECHECK_CACHE_ENTRIES + 1, retained_bytes: 1 },
        PrecheckCacheLimits { entries: 1, retained_bytes: MAX_PRECHECK_CACHE_BYTES + 1 }] {
        assert_eq!(limits.validate(), Err(Error::Limit));
    }
    assert!(State::new(PrecheckCacheLimits { entries: MAX_PRECHECK_CACHE_ENTRIES,
        retained_bytes: MAX_PRECHECK_CACHE_BYTES }).is_ok());
}
