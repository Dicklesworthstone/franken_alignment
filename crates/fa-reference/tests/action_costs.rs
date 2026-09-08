//! Bounded, in-memory transition measurements for the public FA-001 action API.
//!
//! These are reference-model timings only. They do not measure durable I/O,
//! Asupersync runtime scheduling, a broker, allocator/RSS usage, or production
//! latency. The reported input bytes are logical unique payload and witness
//! values supplied by this harness, not measured copies or allocation usage.
//! Byte-cap cases have exactly one witness; they do not exercise the separate
//! maximum witness-count limit.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::time::Instant;

use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, MAX_PAYLOAD_BYTES, MAX_WITNESS_BYTES,
    Purpose, ReferenceAuthority, ResolvedTarget, Scope, TrustedOutcome, VERSION,
};
use fa_reference::{Error, Judgment, ReadWitness, Snapshot};

// Fixed before the measurements begin. The cap case exercises both independently
// published action caps while remaining small enough for an ordinary test run.
const SMALL_SAMPLES: usize = 12;
const AT_CAP_SAMPLES: usize = 2;
const SMALL_PAYLOAD_BYTES: usize = 23;
const SMALL_WITNESS_BYTES: usize = 29;
const ROUTE_IDENTIFIERS_PER_ACTION: usize = 5;

const COLD_SUCCESSES: usize = 2;
const FULL_TRANSITION_SUCCESSES: usize = 10;
const WITNESS_REFUSAL_SUCCESSES: usize = 9;
const REVOCATION_REFUSAL_SUCCESSES: usize = 10;
const EXPIRY_REFUSAL_SUCCESSES: usize = 4;
const UNKNOWN_RECONCILIATION_SUCCESSES: usize = 11;
const VALIDATION_SETUP_SUCCESSES: usize = 7;
const VALIDATION_TIMED_SUCCESSES: usize = 2;
const NO_REFUSALS: usize = 0;
const ONE_REFUSAL: usize = 1;

#[derive(Clone, Copy)]
struct InputShape {
    payload_bytes: usize,
    witness_bytes: usize,
    witness_count: usize,
}

const SMALL: InputShape = InputShape {
    payload_bytes: SMALL_PAYLOAD_BYTES,
    witness_bytes: SMALL_WITNESS_BYTES,
    witness_count: 1,
};

const AT_CAP: InputShape = InputShape {
    payload_bytes: MAX_PAYLOAD_BYTES,
    witness_bytes: MAX_WITNESS_BYTES,
    witness_count: 1,
};

const SMALL_ABSENT: InputShape = InputShape {
    payload_bytes: SMALL_PAYLOAD_BYTES,
    witness_bytes: 0,
    witness_count: 1,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Outcomes {
    successes: usize,
    refusals: usize,
}

impl Outcomes {
    const fn expected(successes: usize, refusals: usize) -> Self {
        Self {
            successes,
            refusals,
        }
    }
}

fn scope() -> Scope {
    Scope {
        tenant: 41,
        principal: 42,
        run: 43,
        branch: 44,
        authority: 45,
        purpose: Purpose::Effect,
    }
}

fn action_spec(shape: InputShape, policy_epoch: u64, deadline: ElapsedTick) -> ActionSpec {
    let witness_value = vec![0xA5; shape.witness_bytes];
    ActionSpec {
        version: VERSION,
        scope: scope(),
        target: Some(ResolvedTarget {
            adapter: 51,
            object: 52,
            contract_version: 53,
            expected_version: 54,
            generation: 55,
        }),
        payload: vec![0x5A; shape.payload_bytes],
        required_witnesses: vec![ReadWitness::Exact {
            key: 61,
            value: Some(witness_value),
        }],
        policy_epoch,
        deadline,
        units: 3,
    }
}

fn absent_action_spec(shape: InputShape, policy_epoch: u64, deadline: ElapsedTick) -> ActionSpec {
    let mut spec = action_spec(shape, policy_epoch, deadline);
    spec.required_witnesses = vec![ReadWitness::Exact {
        key: 61,
        value: None,
    }];
    spec
}

fn freeze(
    shape: InputShape,
    policy_epoch: u64,
    deadline: ElapsedTick,
    outcomes: &mut Outcomes,
) -> FrozenAction {
    let action = FrozenAction::freeze(action_spec(shape, policy_epoch, deadline)).unwrap();
    outcomes.successes += 1;
    black_box(action)
}

fn freeze_absent(
    shape: InputShape,
    policy_epoch: u64,
    deadline: ElapsedTick,
    outcomes: &mut Outcomes,
) -> FrozenAction {
    let action = FrozenAction::freeze(absent_action_spec(shape, policy_epoch, deadline)).unwrap();
    outcomes.successes += 1;
    black_box(action)
}

fn snapshot_for(action: &FrozenAction, outcomes: &mut Outcomes) -> (Snapshot, Judgment) {
    let ReadWitness::Exact {
        key,
        value: Some(value),
    } = &action.spec().required_witnesses[0]
    else {
        panic!("measurement action has one exact witness with a value");
    };
    let snapshot = Snapshot {
        semantic_epoch: 71,
        complete: true,
        values: BTreeMap::from([(*key, value.clone())]),
    };
    let judgment = Judgment::capture(&snapshot, action.spec().required_witnesses.clone()).unwrap();
    outcomes.successes += 1;
    (snapshot, black_box(judgment))
}

fn authority(outcomes: &mut Outcomes) -> ReferenceAuthority {
    let authority = ReferenceAuthority::new(scope(), 9, 4).unwrap();
    outcomes.successes += 1;
    authority
}

fn observe(authority: &mut ReferenceAuthority, tick: ElapsedTick, outcomes: &mut Outcomes) {
    authority.observe_time(tick).unwrap();
    outcomes.successes += 1;
}

fn propose_prepare_review(
    authority: &mut ReferenceAuthority,
    id: u64,
    action: &FrozenAction,
    outcomes: &mut Outcomes,
) {
    authority.propose(id, action.clone()).unwrap();
    outcomes.successes += 1;
    authority.prepare(id).unwrap();
    outcomes.successes += 1;
    authority.begin_review(id).unwrap();
    outcomes.successes += 1;
}

fn authorize(
    authority: &mut ReferenceAuthority,
    id: u64,
    judgment: &Judgment,
    snapshot: &Snapshot,
    outcomes: &mut Outcomes,
) -> fa_reference::action::Permit {
    let permit = authority.authorize(id, judgment, snapshot).unwrap();
    outcomes.successes += 1;
    black_box(permit)
}

fn assert_conserved(authority: &ReferenceAuthority, total: u64) {
    let inspection = authority.inspect();
    assert_eq!(
        inspection.available + inspection.reserved + inspection.charged,
        total,
        "inspection must account for every declared right"
    );
}

fn logical_unique_action_input_bytes(shape: InputShape) -> usize {
    shape.payload_bytes + shape.witness_bytes
}

fn logical_unique_mismatched_witness_input_bytes(shape: InputShape) -> usize {
    logical_unique_action_input_bytes(shape) + shape.witness_bytes
}

fn arithmetic_median(sorted: &[u128]) -> u128 {
    let upper_index = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[upper_index]
    } else {
        let lower = sorted[upper_index - 1];
        let upper = sorted[upper_index];
        lower + (upper - lower) / 2
    }
}

fn measure(
    case: &str,
    samples: usize,
    shape: InputShape,
    logical_unique_input_bytes_per_sample: usize,
    expected: Outcomes,
    run: fn(InputShape) -> Outcomes,
) {
    let mut elapsed = Vec::with_capacity(samples);
    let mut observed_total = Outcomes::default();
    for _ in 0..samples {
        let started = Instant::now();
        let observed = run(shape);
        assert_eq!(observed, expected, "unexpected outcome count for {case}");
        elapsed.push(started.elapsed().as_nanos());
        observed_total.successes += observed.successes;
        observed_total.refusals += observed.refusals;
    }

    elapsed.sort_unstable();
    let total_ns: u128 = elapsed.iter().sum();
    let median_ns = arithmetic_median(&elapsed);
    println!(
        "FA001_ACTION_COST case={case} samples={samples} expected_successes={} successes={} expected_refusals={} refusals={} failures=0 payload_bytes_per_action={} payload_byte_cap={} witness_value_bytes_per_action={} witness_value_byte_cap={} witnesses_per_action={} witness_count_cap_coverage=not_measured route_identifiers_per_action={} logical_unique_input_bytes={} input_bytes_scope=logical_payload_and_distinct_witness_values timing_scope=whole_case_fixture_construction_transitions_assertions_and_drop elapsed_ns_min={} elapsed_ns_median={median_ns} elapsed_ns_max={} elapsed_ns_total={total_ns}",
        expected.successes * samples,
        observed_total.successes,
        expected.refusals * samples,
        observed_total.refusals,
        shape.payload_bytes,
        shape.payload_bytes == MAX_PAYLOAD_BYTES,
        shape.witness_bytes,
        shape.witness_bytes == MAX_WITNESS_BYTES,
        shape.witness_count,
        ROUTE_IDENTIFIERS_PER_ACTION,
        logical_unique_input_bytes_per_sample * samples,
        elapsed[0],
        elapsed[elapsed.len() - 1],
    );
}

fn measure_validation_authorize_dispatch(
    case: &str,
    samples: usize,
    shape: InputShape,
    logical_unique_input_bytes_per_sample: usize,
) {
    let expected_setup = Outcomes::expected(VALIDATION_SETUP_SUCCESSES, NO_REFUSALS);
    let expected_timed = Outcomes::expected(VALIDATION_TIMED_SUCCESSES, NO_REFUSALS);
    let mut elapsed = Vec::with_capacity(samples);
    let mut observed_total = Outcomes::default();

    for _ in 0..samples {
        let mut setup = Outcomes::default();
        let action = freeze(shape, 0, ElapsedTick(10), &mut setup);
        let (snapshot, judgment) = snapshot_for(&action, &mut setup);
        let mut authority = authority(&mut setup);
        observe(&mut authority, ElapsedTick(1), &mut setup);
        propose_prepare_review(&mut authority, 1, &action, &mut setup);
        assert_eq!(
            setup, expected_setup,
            "unexpected validation setup outcome count"
        );

        let started = Instant::now();
        let mut timed = Outcomes::default();
        let permit = authority.authorize(1, &judgment, &snapshot).unwrap();
        timed.successes += 1;
        authority.dispatch(&permit, &action, &snapshot).unwrap();
        timed.successes += 1;
        elapsed.push(started.elapsed().as_nanos());
        assert_eq!(
            timed, expected_timed,
            "unexpected timed validation outcome count"
        );
        observed_total.successes += timed.successes;
        observed_total.refusals += timed.refusals;

        let _ = black_box(&permit);
        let inspection = black_box(authority.inspect());
        assert_eq!(inspection.stages.get(&1), Some(&ActionState::Dispatching));
        assert_eq!(inspection.charged, 3);
        assert_conserved(&authority, 9);
    }

    elapsed.sort_unstable();
    let total_ns: u128 = elapsed.iter().sum();
    let median_ns = arithmetic_median(&elapsed);
    println!(
        "FA001_ACTION_COST case={case} samples={samples} expected_successes={} successes={} expected_refusals=0 refusals=0 failures=0 payload_bytes_per_action={} payload_byte_cap={} witness_value_bytes_per_action={} witness_value_byte_cap={} witnesses_per_action={} witness_count_cap_coverage=not_measured route_identifiers_per_action={} logical_unique_input_bytes={} input_bytes_scope=logical_payload_and_distinct_witness_values timing_scope=successful_witness_validation_authorize_and_dispatch_only setup_outside_timer=true post_timer_black_box_and_inspection=true elapsed_ns_min={} elapsed_ns_median={median_ns} elapsed_ns_max={} elapsed_ns_total={total_ns}",
        expected_timed.successes * samples,
        observed_total.successes,
        shape.payload_bytes,
        shape.payload_bytes == MAX_PAYLOAD_BYTES,
        shape.witness_bytes,
        shape.witness_bytes == MAX_WITNESS_BYTES,
        shape.witness_count,
        ROUTE_IDENTIFIERS_PER_ACTION,
        logical_unique_input_bytes_per_sample * samples,
        elapsed[0],
        elapsed[elapsed.len() - 1],
    );
}

fn measure_absent_key_validation_authorize_dispatch(case: &str, samples: usize, shape: InputShape) {
    let expected_setup = Outcomes::expected(VALIDATION_SETUP_SUCCESSES, NO_REFUSALS);
    let expected_timed = Outcomes::expected(VALIDATION_TIMED_SUCCESSES, NO_REFUSALS);
    let mut elapsed = Vec::with_capacity(samples);
    let mut observed_total = Outcomes::default();

    for _ in 0..samples {
        let mut setup = Outcomes::default();
        let action = freeze_absent(shape, 0, ElapsedTick(10), &mut setup);
        let snapshot = Snapshot {
            semantic_epoch: 71,
            complete: true,
            values: BTreeMap::new(),
        };
        let judgment =
            Judgment::capture(&snapshot, action.spec().required_witnesses.clone()).unwrap();
        setup.successes += 1;
        let mut authority = authority(&mut setup);
        observe(&mut authority, ElapsedTick(1), &mut setup);
        propose_prepare_review(&mut authority, 1, &action, &mut setup);
        assert_eq!(
            setup, expected_setup,
            "unexpected absent-key validation setup outcome count"
        );

        let started = Instant::now();
        let mut timed = Outcomes::default();
        let permit = authority.authorize(1, &judgment, &snapshot).unwrap();
        timed.successes += 1;
        authority.dispatch(&permit, &action, &snapshot).unwrap();
        timed.successes += 1;
        elapsed.push(started.elapsed().as_nanos());
        assert_eq!(
            timed, expected_timed,
            "unexpected absent-key timed outcome count"
        );
        observed_total.successes += timed.successes;

        let _ = black_box(&permit);
        let inspection = black_box(authority.inspect());
        assert_eq!(inspection.stages.get(&1), Some(&ActionState::Dispatching));
        assert_eq!(inspection.charged, 3);
        assert_conserved(&authority, 9);
    }

    elapsed.sort_unstable();
    let total_ns: u128 = elapsed.iter().sum();
    let median_ns = arithmetic_median(&elapsed);
    println!(
        "FA001_ACTION_COST case={case} samples={samples} expected_successes={} successes={} expected_refusals=0 refusals=0 failures=0 payload_bytes_per_action={} payload_byte_cap=false witness_value_bytes_per_action=0 witness_value_byte_cap=false witnesses_per_action={} witness_count_cap_coverage=not_measured route_identifiers_per_action={} logical_unique_input_bytes={} input_bytes_scope=logical_payload_and_absent_key timing_scope=successful_absent_key_witness_validation_authorize_and_dispatch_only setup_outside_timer=true post_timer_black_box_and_inspection=true elapsed_ns_min={} elapsed_ns_median={median_ns} elapsed_ns_max={} elapsed_ns_total={total_ns}",
        expected_timed.successes * samples,
        observed_total.successes,
        shape.payload_bytes,
        shape.witness_count,
        ROUTE_IDENTIFIERS_PER_ACTION,
        logical_unique_action_input_bytes(shape) * samples,
        elapsed[0],
        elapsed[elapsed.len() - 1],
    );
}

fn report_process_memory(phase: &str) {
    println!(
        "FA001_ACTION_COST_MEMORY phase={phase} state=unavailable reason=reference_test_profile_prohibits_filesystem_observation scope=whole_process_not_per_operation_or_allocator"
    );
}

fn cold_construction(shape: InputShape) -> Outcomes {
    let mut outcomes = Outcomes::default();
    let action = freeze(shape, 0, ElapsedTick(10), &mut outcomes);
    let authority = authority(&mut outcomes);
    assert_eq!(action.spec().payload.len(), shape.payload_bytes);
    assert_conserved(&authority, 9);
    outcomes
}

fn successful_transition(shape: InputShape) -> Outcomes {
    let mut outcomes = Outcomes::default();
    let action = freeze(shape, 0, ElapsedTick(10), &mut outcomes);
    let (snapshot, judgment) = snapshot_for(&action, &mut outcomes);
    let mut authority = authority(&mut outcomes);
    observe(&mut authority, ElapsedTick(1), &mut outcomes);
    propose_prepare_review(&mut authority, 1, &action, &mut outcomes);
    let permit = authorize(&mut authority, 1, &judgment, &snapshot, &mut outcomes);
    authority.dispatch(&permit, &action, &snapshot).unwrap();
    outcomes.successes += 1;
    authority
        .record_trusted_outcome(1, TrustedOutcome::Executed)
        .unwrap();
    outcomes.successes += 1;
    assert_eq!(authority.inspect().available, 6);
    assert_eq!(authority.inspect().charged, 3);
    assert_conserved(&authority, 9);
    outcomes
}

fn mismatched_witness_refusal(shape: InputShape) -> Outcomes {
    let mut outcomes = Outcomes::default();
    let action = freeze(shape, 0, ElapsedTick(10), &mut outcomes);
    let (snapshot, judgment) = snapshot_for(&action, &mut outcomes);
    let mut authority = authority(&mut outcomes);
    observe(&mut authority, ElapsedTick(1), &mut outcomes);
    propose_prepare_review(&mut authority, 1, &action, &mut outcomes);
    let permit = authorize(&mut authority, 1, &judgment, &snapshot, &mut outcomes);
    let mut changed_snapshot = snapshot.clone();
    changed_snapshot
        .values
        .insert(61, vec![0x00; shape.witness_bytes]);
    let before = authority.inspect();
    assert_eq!(
        authority.dispatch(&permit, &action, &changed_snapshot),
        Err(Error::Binding)
    );
    outcomes.refusals += 1;
    assert_eq!(
        authority.inspect(),
        before,
        "mismatched witness mutated ledger"
    );
    authority.dispatch(&permit, &action, &snapshot).unwrap();
    outcomes.successes += 1;
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Dispatching),
        "the exact witness must still reach dispatch after the mismatch refusal"
    );
    assert_conserved(&authority, 9);
    outcomes
}

fn revocation_refusal(shape: InputShape) -> Outcomes {
    let mut outcomes = Outcomes::default();
    let action = freeze(shape, 0, ElapsedTick(10), &mut outcomes);
    let (snapshot, judgment) = snapshot_for(&action, &mut outcomes);
    let mut authority = authority(&mut outcomes);
    observe(&mut authority, ElapsedTick(1), &mut outcomes);
    propose_prepare_review(&mut authority, 1, &action, &mut outcomes);
    let permit = authorize(&mut authority, 1, &judgment, &snapshot, &mut outcomes);
    authority.revoke_epoch().unwrap();
    outcomes.successes += 1;
    let before = authority.inspect();
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot),
        Err(Error::Stale)
    );
    outcomes.refusals += 1;
    assert_eq!(authority.inspect(), before, "stale permit mutated ledger");
    authority.cancel(1).unwrap();
    outcomes.successes += 1;
    assert_eq!(authority.inspect().available, 9);
    assert_conserved(&authority, 9);
    outcomes
}

fn expiry_refusal(shape: InputShape) -> Outcomes {
    let mut outcomes = Outcomes::default();
    let action = freeze(shape, 0, ElapsedTick(10), &mut outcomes);
    let mut authority = authority(&mut outcomes);
    observe(&mut authority, ElapsedTick(10), &mut outcomes);
    authority.propose(1, action).unwrap();
    outcomes.successes += 1;
    let before = authority.inspect();
    assert_eq!(authority.prepare(1), Err(Error::Stale));
    outcomes.refusals += 1;
    assert_eq!(authority.inspect(), before, "expired action mutated ledger");
    assert_conserved(&authority, 9);
    outcomes
}

fn unknown_reconciliation(shape: InputShape) -> Outcomes {
    let mut outcomes = Outcomes::default();
    let action = freeze(shape, 0, ElapsedTick(10), &mut outcomes);
    let (snapshot, judgment) = snapshot_for(&action, &mut outcomes);
    let mut authority = authority(&mut outcomes);
    observe(&mut authority, ElapsedTick(1), &mut outcomes);
    propose_prepare_review(&mut authority, 1, &action, &mut outcomes);
    let permit = authorize(&mut authority, 1, &judgment, &snapshot, &mut outcomes);
    authority.dispatch(&permit, &action, &snapshot).unwrap();
    outcomes.successes += 1;
    authority.mark_unknown(1).unwrap();
    outcomes.successes += 1;
    assert_eq!(authority.inspect().charged, 3);
    assert_conserved(&authority, 9);
    authority
        .record_trusted_outcome(1, TrustedOutcome::NotExecuted)
        .unwrap();
    outcomes.successes += 1;
    assert_eq!(authority.inspect().available, 9);
    assert_eq!(authority.inspect().charged, 0);
    assert_conserved(&authority, 9);
    outcomes
}

#[test]
fn bounded_public_action_cost_harness() {
    let mode = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "FA001_ACTION_COST_CONTEXT mode={mode} model=reference_in_memory no_claim=durable_io_asupersync_runtime_broker_or_production_latency"
    );
    report_process_memory("before");

    measure(
        "cold_construction_small",
        SMALL_SAMPLES,
        SMALL,
        logical_unique_action_input_bytes(SMALL),
        Outcomes::expected(COLD_SUCCESSES, NO_REFUSALS),
        cold_construction,
    );
    measure(
        "cold_construction_at_cap",
        AT_CAP_SAMPLES,
        AT_CAP,
        logical_unique_action_input_bytes(AT_CAP),
        Outcomes::expected(COLD_SUCCESSES, NO_REFUSALS),
        cold_construction,
    );
    measure(
        "authorize_dispatch_small",
        SMALL_SAMPLES,
        SMALL,
        logical_unique_action_input_bytes(SMALL),
        Outcomes::expected(FULL_TRANSITION_SUCCESSES, NO_REFUSALS),
        successful_transition,
    );
    measure(
        "authorize_dispatch_at_cap",
        AT_CAP_SAMPLES,
        AT_CAP,
        logical_unique_action_input_bytes(AT_CAP),
        Outcomes::expected(FULL_TRANSITION_SUCCESSES, NO_REFUSALS),
        successful_transition,
    );
    measure(
        "witness_validation_mismatch",
        SMALL_SAMPLES,
        SMALL,
        logical_unique_mismatched_witness_input_bytes(SMALL),
        Outcomes::expected(WITNESS_REFUSAL_SUCCESSES, ONE_REFUSAL),
        mismatched_witness_refusal,
    );
    measure(
        "policy_churn_revocation",
        SMALL_SAMPLES,
        SMALL,
        logical_unique_action_input_bytes(SMALL),
        Outcomes::expected(REVOCATION_REFUSAL_SUCCESSES, ONE_REFUSAL),
        revocation_refusal,
    );
    measure(
        "expiry_failure",
        SMALL_SAMPLES,
        SMALL,
        logical_unique_action_input_bytes(SMALL),
        Outcomes::expected(EXPIRY_REFUSAL_SUCCESSES, ONE_REFUSAL),
        expiry_refusal,
    );
    measure(
        "unknown_reconciliation",
        SMALL_SAMPLES,
        SMALL,
        logical_unique_action_input_bytes(SMALL),
        Outcomes::expected(UNKNOWN_RECONCILIATION_SUCCESSES, NO_REFUSALS),
        unknown_reconciliation,
    );
    measure_validation_authorize_dispatch(
        "witness_validation_authorize_dispatch_small",
        SMALL_SAMPLES,
        SMALL,
        logical_unique_action_input_bytes(SMALL),
    );
    measure_validation_authorize_dispatch(
        "witness_validation_authorize_dispatch_at_cap",
        AT_CAP_SAMPLES,
        AT_CAP,
        logical_unique_action_input_bytes(AT_CAP),
    );
    measure_absent_key_validation_authorize_dispatch(
        "witness_validation_absent_key_small",
        SMALL_SAMPLES,
        SMALL_ABSENT,
    );
    report_process_memory("after");
}
