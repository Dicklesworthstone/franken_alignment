//! Descriptive fixed-case costs for the bounded FA-092 reference relation.
//!
//! These are not DPOR coverage, scheduling performance, heap/allocation, RSS,
//! process-cold, or production-executor measurements. Each recorded timing is
//! retained only after its operation result satisfies the stated assertion.

use std::{
    collections::{BTreeMap, BTreeSet},
    hint::black_box,
    time::Instant,
};

use fa_reference::{
    full_input::{
        ActualHelperInput, ByteSpan, InputProfileBinding, Omission, OpaqueJudgment, PartKind,
        SubmittedPart,
    },
    trace_independence::{
        DomainId, MAX_TRACE_OPERATIONS, OperatorId, OperatorPair, RegisteredTraceOperation,
        RelationEvaluation, TraceInputBindingSpec, TraceOperationSpec, TraceRelationRegistry,
        evaluate_bounded_pairs, independent,
    },
};

const SAMPLES: usize = 12;

fn operator(value: u64) -> OperatorId {
    OperatorId::new(value).unwrap()
}

fn domain(value: u64) -> DomainId {
    DomainId::new(value).unwrap()
}

fn domains(values: &[u64]) -> BTreeSet<DomainId> {
    values.iter().copied().map(domain).collect()
}

fn spec(operator_id: u64) -> TraceOperationSpec {
    TraceOperationSpec {
        operator: operator(operator_id),
        positive_read_domains: BTreeSet::new(),
        write_domains: BTreeSet::new(),
        rng_streams: BTreeSet::new(),
        consuming_resources: BTreeSet::new(),
        authority_transition: false,
        ordered_effect: false,
        external_effect: false,
        unknown_dependencies: false,
        inputs: Vec::new(),
    }
}

fn registered(spec: TraceOperationSpec) -> RegisteredTraceOperation {
    RegisteredTraceOperation::try_from_spec(spec).unwrap()
}

fn registry_for(operator_ids: &[u64]) -> TraceRelationRegistry {
    let mut pairs = BTreeSet::new();
    for (index, left) in operator_ids.iter().enumerate() {
        for right in &operator_ids[index + 1..] {
            pairs.insert(OperatorPair::new(operator(*left), operator(*right)).unwrap());
        }
    }
    TraceRelationRegistry::new(pairs).unwrap()
}

fn input(omission: Omission) -> ActualHelperInput {
    ActualHelperInput::new(
        b"QE".to_vec(),
        InputProfileBinding {
            profile_id: 1,
            profile_bytes: Vec::new(),
            tokenizer_epoch: 0,
            policy_epoch: 0,
            model_epoch: 0,
        },
        vec![
            SubmittedPart {
                span: ByteSpan { start: 0, end: 1 },
                kind: PartKind::Question,
            },
            SubmittedPart {
                span: ByteSpan { start: 1, end: 2 },
                kind: PartKind::Evidence {
                    source_id: 7,
                    transform_id: 1,
                },
            },
        ],
        vec![omission],
    )
    .unwrap()
}

fn binding(omission: Omission) -> TraceInputBindingSpec {
    let input = input(omission);
    TraceInputBindingSpec {
        witness: OpaqueJudgment::capture(&input, b"untrusted explanation")
            .witness()
            .clone(),
        evidence_source_domains: BTreeMap::from([(7, domain(70))]),
        omission_domains: BTreeMap::from([(8, domain(80))]),
    }
}

fn valid_pair() -> RelationEvaluation {
    let registry = registry_for(&[1, 2]);
    let mut left = spec(1);
    left.positive_read_domains = domains(&[10]);
    left.write_domains = domains(&[11]);
    let mut right = spec(2);
    right.positive_read_domains = domains(&[10]);
    right.write_domains = domains(&[12]);
    let left = registered(left);
    let right = registered(right);
    assert!(independent(&registry, &left, &right));
    evaluate_bounded_pairs(&registry, &[left, right]).unwrap()
}

fn negative_domain_conflict() -> RelationEvaluation {
    let registry = registry_for(&[1, 2]);
    let mut reader = spec(1);
    reader.inputs.push(binding(Omission::ClosedAbsent {
        domain_id: 8,
        trusted_closure_marker_id: 9,
    }));
    let mut writer = spec(2);
    writer.write_domains = domains(&[80]);
    let reader = registered(reader);
    let writer = registered(writer);
    assert_eq!(reader.negative_read_domains(), &domains(&[80]));
    assert!(!independent(&registry, &reader, &writer));
    evaluate_bounded_pairs(&registry, &[reader, writer]).unwrap()
}

fn unknown_refusal() -> RelationEvaluation {
    let registry = registry_for(&[1, 2]);
    let mut incomplete = spec(1);
    incomplete.inputs.push(binding(Omission::Gapped {
        domain_id: 8,
        first_missing: 3,
    }));
    let incomplete = registered(incomplete);
    let complete = registered(spec(2));
    assert!(incomplete.unknown_dependencies());
    assert!(!independent(&registry, &incomplete, &complete));
    evaluate_bounded_pairs(&registry, &[incomplete, complete]).unwrap()
}

fn maximum_matrix() -> RelationEvaluation {
    let operator_ids = (1..=MAX_TRACE_OPERATIONS as u64).collect::<Vec<_>>();
    let registry = registry_for(&operator_ids);
    let operations = operator_ids
        .iter()
        .map(|operator_id| {
            let mut operation = spec(*operator_id);
            operation.positive_read_domains = domains(&[*operator_id]);
            operation.write_domains = domains(&[*operator_id + MAX_TRACE_OPERATIONS as u64]);
            registered(operation)
        })
        .collect::<Vec<_>>();
    let independently_counted_positions = operations.len() * (operations.len() - 1) / 2;
    assert_eq!(independently_counted_positions, 120);
    let evaluation = evaluate_bounded_pairs(&registry, &operations).unwrap();
    assert_eq!(evaluation.evaluated_pairs, independently_counted_positions);
    assert_eq!(
        evaluation.independent_pairs,
        independently_counted_positions
    );
    assert_eq!(evaluation.ordered_pairs, 0);
    evaluation
}

fn run_case(
    case: &str,
    logical_operation_count: usize,
    logical_domain_count: usize,
    expected: RelationEvaluation,
    sample: fn() -> RelationEvaluation,
) {
    let mut timings = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let observed = sample();
        let elapsed = started.elapsed().as_nanos();
        assert_eq!(observed, expected, "{case} measured result changed");
        timings.push(elapsed);
    }
    timings.sort_unstable();
    let median = (timings[SAMPLES / 2 - 1] + timings[SAMPLES / 2]) / 2;
    println!(
        "case={case} samples={SAMPLES} evaluated_pairs={} independent_pairs={} ordered_pairs={} logical_operation_count={logical_operation_count} logical_domain_count={logical_domain_count} elapsed_ns_min={} elapsed_ns_median={} elapsed_ns_max={} elapsed_ns_total={} timing_scope=fresh_public_api_construction_and_relation_assertion memory_measurement=unavailable_no_heap_claim",
        expected.evaluated_pairs,
        expected.independent_pairs,
        expected.ordered_pairs,
        timings[0],
        median,
        timings[SAMPLES - 1],
        timings.iter().sum::<u128>(),
    );
}

#[test]
fn fixed_trace_relation_costs_are_descriptive_and_assert_all_outcomes() {
    run_case(
        "registered_independent_pair",
        2,
        3,
        RelationEvaluation {
            evaluated_pairs: 1,
            independent_pairs: 1,
            ordered_pairs: 0,
        },
        valid_pair,
    );
    run_case(
        "closed_negative_domain_write_conflict",
        2,
        2,
        RelationEvaluation {
            evaluated_pairs: 1,
            independent_pairs: 0,
            ordered_pairs: 1,
        },
        negative_domain_conflict,
    );
    run_case(
        "gapped_input_unknown_dependency_refusal",
        2,
        2,
        RelationEvaluation {
            evaluated_pairs: 1,
            independent_pairs: 0,
            ordered_pairs: 1,
        },
        unknown_refusal,
    );
    run_case(
        "maximum_sixteen_operation_matrix",
        MAX_TRACE_OPERATIONS,
        2 * MAX_TRACE_OPERATIONS,
        RelationEvaluation {
            evaluated_pairs: 120,
            independent_pairs: 120,
            ordered_pairs: 0,
        },
        maximum_matrix,
    );
}

/// Only a root-selected fresh release invocation of this one test may be
/// reported as first-evaluation process evidence. It makes no OS/page-cache,
/// CPU-cold, heap, allocation, or production scheduling claim.
#[test]
fn cold_first_evaluation_of_registered_valid_pair() {
    let started = Instant::now();
    let evaluation = valid_pair();
    let elapsed_ns = started.elapsed().as_nanos();
    assert_eq!(
        evaluation,
        RelationEvaluation {
            evaluated_pairs: 1,
            independent_pairs: 1,
            ordered_pairs: 0,
        }
    );
    black_box(evaluation);
    println!(
        "FA092_TRACE_RELATION_COLD case=registered_independent_pair samples=1 evaluated_pairs=1 independent_pairs=1 ordered_pairs=0 logical_operation_count=2 logical_domain_count=3 elapsed_ns={elapsed_ns} proof_scope=fresh_process_exact_release_invocation_only no_claim=os_page_cache_cpu_cold_heap_allocation_or_production_scheduling"
    );
}
