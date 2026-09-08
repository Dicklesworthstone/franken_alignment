//! Public-boundary tests for FA-092 trace-independence declarations.
//!
//! These tests retain actual opaque input witnesses and exercise only the
//! conservative public relation. They do not claim a production executor,
//! DPOR proof, or authority to reorder unreviewed operations.

use std::collections::{BTreeMap, BTreeSet};

use fa_reference::Error;
use fa_reference::full_input::{
    ActualHelperInput, ByteSpan, InputProfileBinding, Omission, OpaqueInputWitness, OpaqueJudgment,
    PartKind, SubmittedPart,
};
use fa_reference::trace_independence::{
    DomainId, OperatorId, OperatorPair, RegisteredTraceOperation, TraceInputBindingSpec,
    TraceOperationSpec, TraceRelationRegistry, independent,
};

fn domain(value: u64) -> DomainId {
    DomainId::new(value).unwrap()
}

fn operator(value: u64) -> OperatorId {
    OperatorId::new(value).unwrap()
}

fn domains(values: &[u64]) -> BTreeSet<DomainId> {
    values.iter().copied().map(domain).collect()
}

fn retained_witness(evidence: bool, omissions: Vec<Omission>) -> OpaqueInputWitness {
    let (submitted_bytes, ordered_parts) = if evidence {
        (
            b"QE".to_vec(),
            vec![
                SubmittedPart {
                    span: ByteSpan { start: 0, end: 1 },
                    kind: PartKind::Question,
                },
                SubmittedPart {
                    span: ByteSpan { start: 1, end: 2 },
                    kind: PartKind::Evidence {
                        source_id: 71,
                        transform_id: 73,
                    },
                },
            ],
        )
    } else {
        (
            b"Q".to_vec(),
            vec![SubmittedPart {
                span: ByteSpan { start: 0, end: 1 },
                kind: PartKind::Question,
            }],
        )
    };
    let input = ActualHelperInput::new(
        submitted_bytes,
        InputProfileBinding {
            profile_id: 7,
            profile_bytes: b"reviewed-profile".to_vec(),
            tokenizer_epoch: 0,
            policy_epoch: 0,
            model_epoch: 0,
        },
        ordered_parts,
        omissions,
    )
    .unwrap();
    OpaqueJudgment::capture(&input, b"untrusted explanation")
        .witness()
        .clone()
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

fn reviewed_pair() -> TraceRelationRegistry {
    TraceRelationRegistry::new(BTreeSet::from([OperatorPair::new(
        operator(1),
        operator(2),
    )
    .unwrap()]))
    .unwrap()
}

fn reviewed_trio() -> TraceRelationRegistry {
    TraceRelationRegistry::new(BTreeSet::from([
        OperatorPair::new(operator(1), operator(2)).unwrap(),
        OperatorPair::new(operator(1), operator(3)).unwrap(),
        OperatorPair::new(operator(2), operator(3)).unwrap(),
    ]))
    .unwrap()
}

fn evidence_binding(domain_id: u64) -> TraceInputBindingSpec {
    TraceInputBindingSpec {
        witness: retained_witness(true, vec![]),
        evidence_source_domains: BTreeMap::from([(71, domain(domain_id))]),
        omission_domains: BTreeMap::new(),
    }
}

fn closed_absence_binding(domain_id: u64) -> TraceInputBindingSpec {
    TraceInputBindingSpec {
        witness: retained_witness(
            false,
            vec![Omission::ClosedAbsent {
                domain_id: 81,
                trusted_closure_marker_id: 83,
            }],
        ),
        evidence_source_domains: BTreeMap::new(),
        omission_domains: BTreeMap::from([(81, domain(domain_id))]),
    }
}

fn dependent_pairs(
    registry: &TraceRelationRegistry,
    operations: &[RegisteredTraceOperation],
) -> Vec<(usize, usize)> {
    [(0, 1), (0, 2), (1, 2)]
        .into_iter()
        .filter(|&(left, right)| !independent(registry, &operations[left], &operations[right]))
        .collect()
}

fn schedules_preserving_original_order(dependencies: &[(usize, usize)]) -> usize {
    [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ]
    .into_iter()
    .filter(|schedule| {
        dependencies.iter().all(|&(before, after)| {
            let before_position = schedule.iter().position(|&value| value == before).unwrap();
            let after_position = schedule.iter().position(|&value| value == after).unwrap();
            before_position < after_position
        })
    })
    .count()
}

#[test]
fn retained_witness_requires_exact_source_and_omission_mapping_twins() {
    let witness = retained_witness(
        true,
        vec![Omission::ClosedAbsent {
            domain_id: 81,
            trusted_closure_marker_id: 83,
        }],
    );
    let binding = TraceInputBindingSpec {
        witness: witness.clone(),
        evidence_source_domains: BTreeMap::from([(71, domain(101))]),
        omission_domains: BTreeMap::from([(81, domain(103))]),
    };
    let mut accepted = spec(1);
    accepted.inputs.push(binding);
    let accepted = registered(accepted);
    assert_eq!(accepted.retained_inputs(), std::slice::from_ref(&witness));
    assert_eq!(accepted.positive_read_domains(), &domains(&[101]));
    assert_eq!(accepted.negative_read_domains(), &domains(&[103]));

    for (source_domains, omission_domains) in [
        (BTreeMap::new(), BTreeMap::from([(81, domain(103))])),
        (
            BTreeMap::from([(71, domain(101)), (79, domain(107))]),
            BTreeMap::from([(81, domain(103))]),
        ),
        (BTreeMap::from([(71, domain(101))]), BTreeMap::new()),
        (
            BTreeMap::from([(71, domain(101))]),
            BTreeMap::from([(81, domain(103)), (89, domain(109))]),
        ),
    ] {
        let mut malformed = spec(1);
        malformed.inputs.push(TraceInputBindingSpec {
            witness: witness.clone(),
            evidence_source_domains: source_domains,
            omission_domains,
        });
        assert_eq!(
            RegisteredTraceOperation::try_from_spec(malformed),
            Err(Error::Binding)
        );
    }
}

#[test]
fn source_and_omission_ids_are_category_distinct_and_same_operator_stays_ordered() {
    let witness = retained_witness(
        true,
        vec![Omission::ClosedAbsent {
            domain_id: 71,
            trusted_closure_marker_id: 83,
        }],
    );
    let binding = || TraceInputBindingSpec {
        witness: witness.clone(),
        evidence_source_domains: BTreeMap::from([(71, domain(101))]),
        omission_domains: BTreeMap::from([(71, domain(103))]),
    };
    let mut first = spec(1);
    first.inputs.push(binding());
    let first = registered(first);
    assert_eq!(first.retained_inputs(), std::slice::from_ref(&witness));
    assert_eq!(first.positive_read_domains(), &domains(&[101]));
    assert_eq!(first.negative_read_domains(), &domains(&[103]));
    assert_ne!(first.positive_read_domains(), first.negative_read_domains());

    let mut second = spec(1);
    second.inputs.push(binding());
    let second = registered(second);
    assert_eq!(first.operator(), second.operator());
    assert!(!independent(&reviewed_pair(), &first, &second));
}

#[test]
fn closed_absence_is_a_whole_domain_negative_read_and_write_conflict() {
    let mut absent_reader = spec(1);
    absent_reader.inputs.push(TraceInputBindingSpec {
        witness: retained_witness(
            false,
            vec![Omission::ClosedAbsent {
                domain_id: 81,
                trusted_closure_marker_id: 83,
            }],
        ),
        evidence_source_domains: BTreeMap::new(),
        omission_domains: BTreeMap::from([(81, domain(103))]),
    });
    let absent_reader = registered(absent_reader);
    assert_eq!(absent_reader.negative_read_domains(), &domains(&[103]));

    let mut separate_writer = spec(2);
    separate_writer.write_domains = domains(&[107]);
    assert!(independent(
        &reviewed_pair(),
        &absent_reader,
        &registered(separate_writer)
    ));

    let mut conflicting_writer = spec(2);
    conflicting_writer.write_domains = domains(&[103]);
    assert!(!independent(
        &reviewed_pair(),
        &absent_reader,
        &registered(conflicting_writer)
    ));
}

#[test]
fn unknown_omission_kinds_are_immutable_ordering_barriers() {
    for omission in [
        Omission::Gapped {
            domain_id: 81,
            first_missing: 5,
        },
        Omission::Unsupported { domain_id: 81 },
        Omission::Redacted {
            domain_id: 81,
            transform_id: 83,
        },
    ] {
        let mut incomplete = spec(1);
        incomplete.inputs.push(TraceInputBindingSpec {
            witness: retained_witness(false, vec![omission]),
            evidence_source_domains: BTreeMap::new(),
            omission_domains: BTreeMap::from([(81, domain(103))]),
        });
        let incomplete = registered(incomplete);
        assert!(incomplete.unknown_dependencies());
        assert!(!independent(
            &reviewed_pair(),
            &incomplete,
            &registered(spec(2))
        ));
    }
}

#[test]
fn shared_witness_derived_positive_reads_commute_when_reviewed_and_clear() {
    let mut left = spec(1);
    left.inputs.push(TraceInputBindingSpec {
        witness: retained_witness(true, vec![]),
        evidence_source_domains: BTreeMap::from([(71, domain(101))]),
        omission_domains: BTreeMap::new(),
    });
    let mut right = spec(2);
    right.inputs.push(TraceInputBindingSpec {
        witness: retained_witness(true, vec![]),
        evidence_source_domains: BTreeMap::from([(71, domain(101))]),
        omission_domains: BTreeMap::new(),
    });
    assert!(independent(
        &reviewed_pair(),
        &registered(left),
        &registered(right)
    ));

    assert!(!independent(
        &TraceRelationRegistry::new(BTreeSet::new()).unwrap(),
        &registered(spec(1)),
        &registered(spec(2))
    ));
}

#[test]
fn every_declared_barrier_preserves_order_with_a_clear_reviewed_control() {
    let registry = reviewed_pair();
    let mut control_left = spec(1);
    control_left.positive_read_domains = domains(&[101]);
    control_left.write_domains = domains(&[103]);
    control_left.rng_streams = domains(&[107]);
    control_left.consuming_resources = domains(&[109]);
    let mut control_right = spec(2);
    control_right.positive_read_domains = domains(&[101]);
    control_right.write_domains = domains(&[113]);
    control_right.rng_streams = domains(&[127]);
    control_right.consuming_resources = domains(&[131]);
    assert!(independent(
        &registry,
        &registered(control_left),
        &registered(control_right)
    ));

    let mut write_left = spec(1);
    write_left.write_domains = domains(&[101]);
    let mut read_right = spec(2);
    read_right.positive_read_domains = domains(&[101]);
    assert!(!independent(
        &registry,
        &registered(write_left),
        &registered(read_right)
    ));

    let mut write_left = spec(1);
    write_left.write_domains = domains(&[101]);
    let mut write_right = spec(2);
    write_right.write_domains = domains(&[101]);
    assert!(!independent(
        &registry,
        &registered(write_left),
        &registered(write_right)
    ));

    let mut rng_left = spec(1);
    rng_left.rng_streams = domains(&[101]);
    let mut rng_right = spec(2);
    rng_right.rng_streams = domains(&[101]);
    assert!(!independent(
        &registry,
        &registered(rng_left),
        &registered(rng_right)
    ));

    let mut resource_left = spec(1);
    resource_left.consuming_resources = domains(&[101]);
    let mut resource_right = spec(2);
    resource_right.consuming_resources = domains(&[101]);
    assert!(!independent(
        &registry,
        &registered(resource_left),
        &registered(resource_right)
    ));

    let mut authority = spec(1);
    authority.authority_transition = true;
    assert!(!independent(
        &registry,
        &registered(authority),
        &registered(spec(2))
    ));

    let mut ordered = spec(1);
    ordered.ordered_effect = true;
    assert!(!independent(
        &registry,
        &registered(ordered),
        &registered(spec(2))
    ));

    let mut external = spec(1);
    external.external_effect = true;
    assert!(!independent(
        &registry,
        &registered(external),
        &registered(spec(2))
    ));

    let mut unknown = spec(1);
    unknown.unknown_dependencies = true;
    assert!(!independent(
        &registry,
        &registered(unknown),
        &registered(spec(2))
    ));
}

#[test]
fn bounded_three_operation_schedules_preserve_original_trace_order() {
    let registry = reviewed_trio();
    let mut independent_one = spec(1);
    independent_one.inputs.push(evidence_binding(101));
    let mut independent_two = spec(2);
    independent_two.inputs.push(evidence_binding(101));
    let mut independent_three = spec(3);
    independent_three.inputs.push(evidence_binding(101));

    let mut dependent_one = spec(1);
    dependent_one.inputs.push(closed_absence_binding(201));
    let mut dependent_two = spec(2);
    dependent_two.inputs.push(evidence_binding(203));
    dependent_two.write_domains = domains(&[201]);
    let mut dependent_three = spec(3);
    dependent_three.inputs.push(evidence_binding(205));
    dependent_three.write_domains = domains(&[201]);

    let mut mixed_one = spec(1);
    mixed_one.inputs.push(closed_absence_binding(301));
    let mut mixed_two = spec(2);
    mixed_two.inputs.push(evidence_binding(303));
    mixed_two.write_domains = domains(&[301]);
    let mut mixed_three = spec(3);
    mixed_three.inputs.push(evidence_binding(305));

    for (case, operations, expected_pairs, expected_schedules) in [
        (
            "fully_independent_three",
            vec![
                registered(independent_one),
                registered(independent_two),
                registered(independent_three),
            ],
            Vec::new(),
            6,
        ),
        (
            "fully_dependent_three",
            vec![
                registered(dependent_one),
                registered(dependent_two),
                registered(dependent_three),
            ],
            vec![(0, 1), (0, 2), (1, 2)],
            1,
        ),
        (
            "one_dependent_pair",
            vec![
                registered(mixed_one),
                registered(mixed_two),
                registered(mixed_three),
            ],
            vec![(0, 1)],
            3,
        ),
    ] {
        // The expected pair list is fixture-specific and independently fixes
        // the causal ordering constraint before the production relation is read.
        assert_eq!(
            schedules_preserving_original_order(&expected_pairs),
            expected_schedules
        );
        let actual_pairs = dependent_pairs(&registry, &operations);
        assert_eq!(actual_pairs, expected_pairs, "{case}");
        assert_eq!(
            schedules_preserving_original_order(&actual_pairs),
            expected_schedules,
            "{case}"
        );
        let retained_inputs = operations
            .iter()
            .map(|operation| operation.retained_inputs().len())
            .sum::<usize>();
        assert_eq!(retained_inputs, 3, "{case}");
        println!(
            "case={case} retained_inputs={retained_inputs} dependent_pairs={} valid_schedules={expected_schedules}",
            actual_pairs.len()
        );
    }
}

#[test]
fn shared_domain_mutations_conflict_across_every_dependency_category() {
    fn operation(operator_id: u64, category: usize, domain_id: u64) -> RegisteredTraceOperation {
        let mut value = spec(operator_id);
        match category {
            0 => value.positive_read_domains = domains(&[domain_id]),
            1 => value.inputs.push(closed_absence_binding(domain_id)),
            2 => value.write_domains = domains(&[domain_id]),
            3 => value.rng_streams = domains(&[domain_id]),
            4 => value.consuming_resources = domains(&[domain_id]),
            _ => unreachable!(),
        }
        registered(value)
    }
    let registry = reviewed_pair();
    for left_category in 0..5 {
        for right_category in 0..5 {
            let left = operation(1, left_category, 103);
            let disjoint = operation(2, right_category, 107);
            assert!(independent(&registry, &left, &disjoint));
            assert!(independent(&registry, &disjoint, &left));
            let overlapping = operation(2, right_category, 103);
            let read_only = left_category < 2 && right_category < 2;
            assert_eq!(
                independent(&registry, &left, &overlapping),
                read_only,
                "left category {left_category}, right category {right_category}"
            );
            assert_eq!(independent(&registry, &overlapping, &left), read_only);
        }
    }
}
