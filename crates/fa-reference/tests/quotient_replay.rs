//! FA-093 bounded quotient oracle integration tests.
//!
//! Expectations include independently enumerated permutations and independent
//! adjacent-swap reachability, concrete policy results and causal negative pairs.
//! The measured fixture prints its fixed denominator and actual logical work;
//! it makes no OS-cache, process-memory, production replay or latency SLO claim.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use fa_reference::Error;
use fa_reference::action::consequence::{Consequence, Decision, Rule};
use fa_reference::full_input::{
    ActualHelperInput, ByteSpan, InputProfileBinding, Omission, OpaqueJudgment, PartKind,
    SubmittedPart,
};
use fa_reference::trace_independence::{
    DomainId, OperatorId, OperatorPair, RegisteredTraceOperation, TraceInputBindingSpec,
    TraceOperationSpec, TraceRelationRegistry,
};
use fa_reference::trace_independence::quotient::{
    MAX_QUOTIENT_CELLS, MAX_QUOTIENT_OPERATIONS, MAX_QUOTIENT_SCHEDULES, QuotientReport,
    ReplayBudget, ReplayFailure, ReplayInstruction, ReplayOccurrence, ReplayState, ReplayValue,
    canonicalize, estimate, replay, verify,
};

fn domain(value: u64) -> DomainId { DomainId::new(value).unwrap() }
fn operator(value: u64) -> OperatorId { OperatorId::new(value).unwrap() }

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

fn registry(ids: &[u64]) -> TraceRelationRegistry {
    let mut pairs = BTreeSet::new();
    for (index, left) in ids.iter().copied().enumerate() {
        for right in ids[index + 1..].iter().copied() {
            if left != right {
                pairs.insert(OperatorPair::new(operator(left), operator(right)).unwrap());
            }
        }
    }
    TraceRelationRegistry::new(pairs).unwrap()
}

fn state(values: &[(u64, u64)]) -> ReplayState {
    ReplayState::new(values.iter().map(|(key, value)| (domain(*key), *value)).collect(), true)
        .unwrap()
}

fn omission_binding(read_domain: DomainId, omission: Omission) -> TraceInputBindingSpec {
    let input = ActualHelperInput::new(
        vec![b'Q'],
        InputProfileBinding {
            profile_id: 1,
            profile_bytes: Vec::new(),
            tokenizer_epoch: 0,
            policy_epoch: 0,
            model_epoch: 0,
        },
        vec![SubmittedPart {
            span: ByteSpan { start: 0, end: 1 },
            kind: PartKind::Question,
        }],
        vec![omission],
    ).unwrap();
    TraceInputBindingSpec {
        witness: OpaqueJudgment::capture(&input, b"untrusted explanation").witness().clone(),
        evidence_source_domains: BTreeMap::new(),
        omission_domains: BTreeMap::from([(7, read_domain)]),
    }
}

fn declared(operator_id: u64, instruction: ReplayInstruction) -> TraceOperationSpec {
    let mut result = spec(operator_id);
    match instruction {
        ReplayInstruction::Read { domain }
        | ReplayInstruction::Check { domain, expected: Some(_) } => {
            result.positive_read_domains.insert(domain);
        }
        ReplayInstruction::Write { domain, .. } => { result.write_domains.insert(domain); }
        ReplayInstruction::CopyAdd { source, target, .. } => {
            result.positive_read_domains.insert(source);
            result.write_domains.insert(target);
        }
        ReplayInstruction::Check { domain, expected: None } => {
            result.inputs.push(omission_binding(domain, Omission::ClosedAbsent {
                domain_id: 7, trusted_closure_marker_id: 9,
            }));
        }
        ReplayInstruction::Consume { resource, .. } => { result.consuming_resources.insert(resource); }
        ReplayInstruction::Draw { stream } => { result.rng_streams.insert(stream); }
        ReplayInstruction::Authority { .. } => { result.authority_transition = true; }
        ReplayInstruction::Ordered { domain } => {
            result.ordered_effect = true;
            result.positive_read_domains.insert(domain);
        }
        ReplayInstruction::External { domain } => {
            result.external_effect = true;
            result.positive_read_domains.insert(domain);
        }
    }
    result
}

fn occurrence(id: u64, operator_id: u64, instruction: ReplayInstruction) -> ReplayOccurrence {
    ReplayOccurrence::new(
        id, RegisteredTraceOperation::try_from_spec(declared(operator_id, instruction)).unwrap(),
        instruction,
    ).unwrap()
}

fn run(occurrences: &[ReplayOccurrence], initial: &ReplayState) -> QuotientReport {
    let ids = occurrences.iter().map(|operation| operation.operation().operator().get())
        .collect::<Vec<_>>();
    verify(&registry(&ids), occurrences, initial, ReplayBudget::default()).unwrap()
}

/// Recursive removal builds a complete permutation set independently of the
/// production lexicographic successor implementation.
fn permutations(ids: &[u64]) -> Vec<Vec<u64>> {
    if ids.is_empty() {
        return vec![Vec::new()];
    }
    let mut all = Vec::new();
    for (index, id) in ids.iter().copied().enumerate() {
        let remaining = ids.iter().copied().enumerate()
            .filter_map(|(position, value)| (position != index).then_some(value))
            .collect::<Vec<_>>();
        for suffix in permutations(&remaining) {
            let mut word = vec![id];
            word.extend(suffix);
            all.push(word);
        }
    }
    all
}

fn decision(value: &ReplayValue) -> &Decision {
    let ReplayValue::Decision(decision) = value else {
        panic!("expected the original consequence oracle's decision")
    };
    decision
}

#[test]
fn all_120_schedules_share_real_copy_resource_rng_and_policy_results() {
    let occurrences = vec![
        occurrence(50, 1, ReplayInstruction::CopyAdd { source: domain(1), target: domain(2), addend: 1 }),
        occurrence(10, 2, ReplayInstruction::CopyAdd { source: domain(1), target: domain(3), addend: 2 }),
        occurrence(40, 3, ReplayInstruction::Check { domain: domain(1), expected: Some(7) }),
        occurrence(20, 4, ReplayInstruction::Consume { resource: domain(4), units: 2 }),
        occurrence(30, 5, ReplayInstruction::Draw { stream: domain(5) }),
    ];
    let initial = state(&[(1, 7), (4, 3), (5, 9)]);
    let preserved = initial.clone();
    let admission = estimate(&occurrences, &initial).unwrap();
    let report = verify(&registry(&[1, 2, 3, 4, 5]), &occurrences, &initial, admission.required)
        .unwrap();
    assert_eq!(report.admission().required.schedules, 120);
    assert_eq!(report.progress().unreduced_replays, 120);
    assert_eq!(report.progress().compared_schedules, 120);
    assert_eq!(report.progress().representative_replays, 1);
    assert_eq!(report.progress().replay_steps, 605);
    assert_eq!(report.relation().independent_pairs, 10);
    assert_eq!(report.representatives().len(), 1);
    let representative = &report.representatives()[0];
    assert_eq!(representative.members(), 120);
    assert_eq!(representative.schedule().occurrences(), &[10, 20, 30, 40, 50]);
    assert_eq!(representative.replay().state().cells()[&domain(2)], 8);
    assert_eq!(representative.replay().state().cells()[&domain(3)], 9);
    assert_eq!(representative.replay().state().cells()[&domain(4)], 1);
    assert_eq!(representative.replay().results().len(), 5);
    assert_eq!(decision(&representative.replay().results()[3].value), &Decision {
        consequence: Consequence::Continue, rules: vec![Rule::EmpiricalRecommendation],
    });
    let all = permutations(&[50, 10, 40, 20, 30]);
    assert_eq!(all.len(), 120);
    for schedule in all {
        assert_eq!(replay(&occurrences, &initial, &schedule).unwrap(), *representative.replay());
    }
    assert_eq!(initial, preserved);
    let encoded = representative.schedule().encode();
    assert_eq!(&encoded[..8], b"FAQTRC01");
    assert_eq!(&encoded[8..16], &5u64.to_be_bytes());
    assert_eq!(encoded.len(), 56);
    assert_eq!(report.stored_trace_bytes(), 56);
    assert_eq!(report.unreduced_trace_bytes(), 6_720);
    assert!(report.progress().canonical_checks <= admission.required.canonical_checks);
    assert_eq!(report.progress().comparison_entries, admission.required.comparison_entries);
}

#[test]
fn canonicalization_matches_independent_adjacent_swap_reachability_for_every_order() {
    let occurrences = vec![
        occurrence(1, 1, ReplayInstruction::CopyAdd { source: domain(2), target: domain(1), addend: 1 }),
        occurrence(2, 2, ReplayInstruction::CopyAdd { source: domain(1), target: domain(2), addend: 1 }),
        occurrence(3, 3, ReplayInstruction::Check { domain: domain(3), expected: Some(7) }),
        occurrence(4, 4, ReplayInstruction::Draw { stream: domain(4) }),
    ];
    let initial = state(&[(1, 1), (2, 2), (3, 7), (4, 9)]);
    let registry = registry(&[1, 2, 3, 4]);
    let all = permutations(&[1, 2, 3, 4]);
    assert_eq!(all.len(), 24);
    for original in all {
        let mut reachable = BTreeSet::from([original.clone()]);
        let mut pending = vec![original.clone()];
        while let Some(word) = pending.pop() {
            for position in 0..word.len() - 1 {
                // Independently specified dependency graph: only 1 and 2 conflict.
                if word[position] <= 2 && word[position + 1] <= 2 {
                    continue;
                }
                let mut next = word.clone();
                next.swap(position, position + 1);
                if reachable.insert(next.clone()) {
                    pending.push(next);
                }
            }
        }
        assert_eq!(reachable.len(), 12);
        let expected = reachable.first().unwrap();
        let canonical = canonicalize(&registry, &occurrences, &original).unwrap();
        assert_eq!(canonical.occurrences(), expected.as_slice());
        assert_eq!(canonicalize(&registry, &occurrences, canonical.occurrences()).unwrap(), canonical);
        assert_eq!(
            replay(&occurrences, &initial, &original).unwrap(),
            replay(&occurrences, &initial, canonical.occurrences()).unwrap(),
        );
    }
    let report = run(&occurrences, &initial);
    assert_eq!(report.representatives().len(), 2);
    assert!(report.representatives().iter().all(|representative| representative.members() == 12));
}

#[test]
fn cross_reads_and_shared_reads_preserve_distinct_computed_outcomes() {
    let cross = [
        occurrence(1, 1, ReplayInstruction::CopyAdd { source: domain(2), target: domain(1), addend: 1 }),
        occurrence(2, 2, ReplayInstruction::CopyAdd { source: domain(1), target: domain(2), addend: 1 }),
    ];
    let initial = state(&[(1, 1), (2, 2)]);
    let report = run(&cross, &initial);
    assert_eq!(report.representatives().len(), 2);
    let forward = replay(&cross, &initial, &[1, 2]).unwrap();
    let reverse = replay(&cross, &initial, &[2, 1]).unwrap();
    assert_eq!(forward.state().cells(), state(&[(1, 3), (2, 4)]).cells());
    assert_eq!(reverse.state().cells(), state(&[(1, 3), (2, 2)]).cells());
    assert_ne!(forward, reverse);

    let policy = [
        occurrence(1, 1, ReplayInstruction::Write { domain: domain(1), value: Some(2) }),
        occurrence(2, 2, ReplayInstruction::Check { domain: domain(1), expected: Some(1) }),
    ];
    let report = run(&policy, &initial);
    assert_eq!(report.representatives().len(), 2);
    let forward = replay(&policy, &initial, &[1, 2]).unwrap();
    let reverse = replay(&policy, &initial, &[2, 1]).unwrap();
    assert_eq!(forward.state(), reverse.state());
    assert_ne!(forward.results(), reverse.results());
    assert_eq!(decision(&forward.results()[1].value), &Decision {
        consequence: Consequence::Deny,
        rules: vec![Rule::EmpiricalRecommendation, Rule::ExactDisqualifier],
    });
    assert_eq!(decision(&reverse.results()[1].value).consequence, Consequence::Continue);
}

#[test]
fn closed_absence_conflicts_only_with_its_domain_and_missing_required_evidence_holds() {
    let absent = occurrence(1, 1, ReplayInstruction::Check { domain: domain(7), expected: None });
    let independent = [
        absent.clone(),
        occurrence(2, 2, ReplayInstruction::Write { domain: domain(8), value: Some(9) }),
    ];
    assert_eq!(run(&independent, &state(&[])).representatives().len(), 1);
    let conflicting = [
        absent,
        occurrence(2, 2, ReplayInstruction::Write { domain: domain(7), value: Some(9) }),
    ];
    let report = run(&conflicting, &state(&[]));
    assert_eq!(report.representatives().len(), 2);
    let before = replay(&conflicting, &state(&[]), &[1, 2]).unwrap();
    let after = replay(&conflicting, &state(&[]), &[2, 1]).unwrap();
    assert_eq!(decision(&before.results()[0].value).consequence, Consequence::Continue);
    assert_eq!(decision(&after.results()[0].value).consequence, Consequence::Deny);

    let required = [occurrence(1, 1, ReplayInstruction::Check {
        domain: domain(7), expected: Some(9),
    })];
    let missing = run(&required, &state(&[]));
    let judgment = decision(&missing.representatives()[0].replay().results()[0].value);
    assert_eq!(judgment.consequence, Consequence::HoldEffect);
    assert!(judgment.rules.contains(&Rule::MandatoryAbsent));
    assert!(!judgment.rules.contains(&Rule::ExactDisqualifier));
}

#[test]
fn shared_rng_resources_and_cross_category_mutations_keep_order() {
    for shared in [false, true] {
        let second = if shared { domain(1) } else { domain(2) };
        let rng = [
            occurrence(1, 1, ReplayInstruction::Draw { stream: domain(1) }),
            occurrence(2, 2, ReplayInstruction::Draw { stream: second }),
        ];
        let initial = state(&[(1, 9), (2, 9)]);
        assert_eq!(run(&rng, &initial).representatives().len(), if shared { 2 } else { 1 });
        if shared {
            let forward = replay(&rng, &initial, &[1, 2]).unwrap();
            let reverse = replay(&rng, &initial, &[2, 1]).unwrap();
            assert_eq!(forward.state(), reverse.state());
            assert_ne!(forward.results(), reverse.results());
        }
        let resources = [
            occurrence(1, 1, ReplayInstruction::Consume { resource: domain(1), units: 2 }),
            occurrence(2, 2, ReplayInstruction::Consume { resource: second, units: 3 }),
        ];
        let report = run(&resources, &state(&[(1, 4), (2, 4)]));
        assert_eq!(report.representatives().len(), if shared { 2 } else { 1 });
        for representative in report.representatives() {
            assert_eq!(
                representative.replay().results().iter()
                    .filter(|row| row.value == ReplayValue::Rejected(Error::Limit)).count(),
                usize::from(shared),
            );
        }
    }
    for mutation in [
        ReplayInstruction::Draw { stream: domain(1) },
        ReplayInstruction::Consume { resource: domain(1), units: 1 },
        ReplayInstruction::Write { domain: domain(1), value: Some(7) },
    ] {
        let conflicting = [
            occurrence(1, 1, mutation),
            occurrence(2, 2, ReplayInstruction::Read { domain: domain(1) }),
        ];
        assert_eq!(run(&conflicting, &state(&[(1, 9)])).representatives().len(), 2);
        let allowed = [
            occurrence(1, 1, mutation),
            occurrence(2, 2, ReplayInstruction::Read { domain: domain(2) }),
        ];
        assert_eq!(run(&allowed, &state(&[(1, 9), (2, 8)])).representatives().len(), 1);
    }
}

#[test]
fn authority_ordered_external_and_witness_derived_unknown_barriers_survive() {
    let authority = [
        occurrence(1, 1, ReplayInstruction::Authority { enabled: false }),
        occurrence(2, 2, ReplayInstruction::External { domain: domain(1) }),
    ];
    let initial = state(&[(1, 7), (2, 8)]);
    assert_eq!(run(&authority, &initial).representatives().len(), 2);
    let forward = replay(&authority, &initial, &[1, 2]).unwrap();
    let reverse = replay(&authority, &initial, &[2, 1]).unwrap();
    assert_eq!(forward.state(), reverse.state());
    assert!(!forward.ordered()[0].admitted);
    assert!(reverse.ordered()[0].admitted);
    assert_eq!(decision(&forward.results()[1].value).consequence, Consequence::HoldEffect);
    assert_eq!(decision(&reverse.results()[1].value).consequence, Consequence::Continue);

    for external in [false, true] {
        let instruction = |domain| if external {
            ReplayInstruction::External { domain }
        } else {
            ReplayInstruction::Ordered { domain }
        };
        let operations = [
            occurrence(1, 1, instruction(domain(1))),
            occurrence(2, 2, instruction(domain(2))),
        ];
        assert_eq!(run(&operations, &initial).representatives().len(), 2);
        assert_ne!(
            replay(&operations, &initial, &[1, 2]).unwrap().ordered(),
            replay(&operations, &initial, &[2, 1]).unwrap().ordered(),
        );
    }
    let read = ReplayInstruction::Read { domain: domain(1) };
    let other = occurrence(2, 2, ReplayInstruction::Write { domain: domain(2), value: Some(9) });
    let allowed = [occurrence(1, 1, read), other.clone()];
    assert_eq!(run(&allowed, &initial).representatives().len(), 1);
    for omission in [
        Omission::Gapped { domain_id: 7, first_missing: 3 },
        Omission::Unsupported { domain_id: 7 },
        Omission::Redacted { domain_id: 7, transform_id: 4 },
    ] {
        let mut unknown = declared(1, read);
        unknown.inputs.push(omission_binding(domain(3), omission));
        let unknown = RegisteredTraceOperation::try_from_spec(unknown).unwrap();
        assert!(unknown.unknown_dependencies());
        let operations = [ReplayOccurrence::new(1, unknown, read).unwrap(), other.clone()];
        assert_eq!(run(&operations, &initial).representatives().len(), 2);
    }
}

#[test]
fn every_instruction_requires_all_actual_accesses_and_barriers_to_be_declared() {
    let instructions = [
        ReplayInstruction::Read { domain: domain(1) },
        ReplayInstruction::Write { domain: domain(1), value: None },
        ReplayInstruction::CopyAdd { source: domain(1), target: domain(2), addend: 1 },
        ReplayInstruction::Check { domain: domain(1), expected: Some(7) },
        ReplayInstruction::Check { domain: domain(1), expected: None },
        ReplayInstruction::Consume { resource: domain(1), units: 1 },
        ReplayInstruction::Draw { stream: domain(1) },
        ReplayInstruction::Authority { enabled: false },
        ReplayInstruction::Ordered { domain: domain(1) },
        ReplayInstruction::External { domain: domain(1) },
    ];
    for instruction in instructions {
        let complete = occurrence(1, 1, instruction);
        let report = run(&[complete], &state(&[(1, 7)]));
        assert_eq!(report.progress().compared_schedules, 1);
        assert_eq!(report.representatives()[0].replay().results().len(), 1);
        assert_eq!(
            ReplayOccurrence::new(1, RegisteredTraceOperation::try_from_spec(spec(1)).unwrap(), instruction),
            Err(Error::Binding),
        );
    }
    let copy = ReplayInstruction::CopyAdd { source: domain(1), target: domain(2), addend: 1 };
    let mut incomplete_copy = declared(1, copy);
    incomplete_copy.positive_read_domains.clear();
    assert_eq!(
        ReplayOccurrence::new(1, RegisteredTraceOperation::try_from_spec(incomplete_copy).unwrap(), copy),
        Err(Error::Binding),
    );
    let external = ReplayInstruction::External { domain: domain(1) };
    let mut incomplete_external = declared(1, external);
    incomplete_external.positive_read_domains.clear();
    assert_eq!(
        ReplayOccurrence::new(1, RegisteredTraceOperation::try_from_spec(incomplete_external).unwrap(), external),
        Err(Error::Binding),
    );
}

#[test]
fn every_admission_budget_refuses_one_short_and_accepts_the_exact_requirement() {
    let occurrences = [
        occurrence(1, 1, ReplayInstruction::Write { domain: domain(1), value: Some(7) }),
        occurrence(2, 2, ReplayInstruction::Check { domain: domain(2), expected: Some(8) }),
    ];
    let initial = state(&[(2, 8)]);
    let preserved = initial.clone();
    let required = estimate(&occurrences, &initial).unwrap().required;
    let mut short = Vec::new();
    let mut budget = required;
    budget.schedules -= 1;
    short.push(budget);
    let mut budget = required;
    budget.replay_steps -= 1;
    short.push(budget);
    let mut budget = required;
    budget.canonical_checks -= 1;
    short.push(budget);
    let mut budget = required;
    budget.comparison_entries -= 1;
    short.push(budget);
    let mut budget = required;
    budget.retained_bytes -= 1;
    short.push(budget);
    for budget in short {
        assert_eq!(
            verify(&registry(&[1, 2]), &occurrences, &initial, budget),
            Err(ReplayFailure::Admission(Error::Limit)),
        );
        assert_eq!(initial, preserved);
    }
    assert_eq!(
        verify(&registry(&[1, 2]), &occurrences, &initial, required)
            .unwrap().progress().compared_schedules, 2,
    );
    let mut excessive = ReplayBudget::default();
    excessive.schedules += 1;
    assert_eq!(
        verify(&registry(&[1, 2]), &occurrences, &initial, excessive),
        Err(ReplayFailure::Admission(Error::Limit)),
    );

    let full = ReplayState::new(
        (1..=MAX_QUOTIENT_CELLS as u64).map(|value| (domain(value), value)).collect(), true,
    ).unwrap();
    let new_cell = [occurrence(1, 1, ReplayInstruction::Write {
        domain: domain(MAX_QUOTIENT_CELLS as u64 + 1), value: Some(9),
    })];
    assert_eq!(estimate(&new_cell, &full), Err(Error::Limit));
    let existing_cell = [occurrence(1, 1, ReplayInstruction::Write {
        domain: domain(MAX_QUOTIENT_CELLS as u64), value: Some(9),
    })];
    assert!(estimate(&existing_cell, &full).is_ok());
    assert_eq!(
        ReplayState::new(
            (1..=MAX_QUOTIENT_CELLS as u64 + 1).map(|value| (domain(value), value)).collect(), true,
        ),
        Err(Error::Limit),
    );
}

#[test]
fn occurrence_identities_are_unambiguous_and_unregistered_or_repeated_operators_stay_ordered() {
    let first = occurrence(9, 1, ReplayInstruction::Write { domain: domain(1), value: Some(7) });
    let same_operator = occurrence(2, 1, ReplayInstruction::Write { domain: domain(2), value: Some(8) });
    let repeated = [first.clone(), same_operator];
    assert_eq!(run(&repeated, &state(&[])).representatives().len(), 2);

    let distinct = [
        first.clone(),
        occurrence(2, 2, ReplayInstruction::Write { domain: domain(2), value: Some(8) }),
    ];
    assert_eq!(run(&distinct, &state(&[])).representatives().len(), 1);
    assert_eq!(
        verify(&TraceRelationRegistry::new(BTreeSet::new()).unwrap(),
            &distinct, &state(&[]), ReplayBudget::default()).unwrap().representatives().len(),
        2,
    );
    assert_eq!(
        canonicalize(&registry(&[1, 2]), &distinct, &[9, 9]), Err(Error::Duplicate),
    );
    for malformed in [&[9][..], &[9, 2, 3][..], &[9, 99][..]] {
        assert_eq!(canonicalize(&registry(&[1, 2]), &distinct, malformed), Err(Error::Binding));
        assert_eq!(replay(&distinct, &state(&[]), malformed), Err(Error::Binding));
    }
    assert_eq!(estimate(&[first.clone(), first.clone()], &state(&[])), Err(Error::Duplicate));
    assert_eq!(
        ReplayOccurrence::new(0, first.operation().clone(), first.instruction()),
        Err(Error::InvalidInput),
    );
    let too_many = (1..=MAX_QUOTIENT_OPERATIONS as u64 + 1)
        .map(|id| occurrence(id, id, ReplayInstruction::Write { domain: domain(id), value: Some(id) }))
        .collect::<Vec<_>>();
    assert_eq!(estimate(&too_many, &state(&[])), Err(Error::Limit));
}

#[test]
fn failed_operations_keep_complete_occurrence_results_and_schedule_counts() {
    let operations = [
        occurrence(1, 1, ReplayInstruction::CopyAdd {
            source: domain(1), target: domain(2), addend: 1,
        }),
        occurrence(2, 2, ReplayInstruction::Write { domain: domain(3), value: Some(9) }),
    ];
    for (initial, expected) in [
        (state(&[]), Error::Missing),
        (state(&[(1, u64::MAX)]), Error::Overflow),
    ] {
        let report = run(&operations, &initial);
        assert_eq!(report.admission().required.schedules, 2);
        assert_eq!(report.progress().compared_schedules, 2);
        assert_eq!(report.progress().unreduced_replays, 2);
        assert_eq!(report.progress().replay_steps, 6);
        assert_eq!(report.representatives().len(), 1);
        let result = report.representatives()[0].replay();
        assert_eq!(result.results().len(), 2);
        assert_eq!(result.results()[0].value, ReplayValue::Rejected(expected));
        assert!(!result.state().cells().contains_key(&domain(2)));
        assert_eq!(result.state().cells()[&domain(3)], 9);
        for schedule in [vec![1, 2], vec![2, 1]] {
            assert_eq!(replay(&operations, &initial, &schedule).unwrap(), *result);
        }
    }
    let success = run(&operations, &state(&[(1, 6)]));
    let result = success.representatives()[0].replay();
    assert_eq!(result.results()[0].value, ReplayValue::Written);
    assert_eq!(result.state().cells()[&domain(2)], 7);
    assert_eq!(result.state().cells()[&domain(3)], 9);
}

#[test]
fn all_40320_schedules_at_the_declared_limit_have_one_representative() {
    let operations = (1..=MAX_QUOTIENT_OPERATIONS as u64)
        .map(|id| occurrence(id, id, ReplayInstruction::Write { domain: domain(id), value: Some(id) }))
        .collect::<Vec<_>>();
    let report = run(&operations, &state(&[]));
    assert_eq!(report.admission().required.schedules, MAX_QUOTIENT_SCHEDULES);
    assert_eq!(report.progress().unreduced_replays, MAX_QUOTIENT_SCHEDULES);
    assert_eq!(report.progress().compared_schedules, MAX_QUOTIENT_SCHEDULES);
    assert_eq!(report.representatives().len(), 1);
    assert_eq!(report.representatives()[0].members(), MAX_QUOTIENT_SCHEDULES);
    assert_eq!(report.progress().replay_steps, (MAX_QUOTIENT_SCHEDULES + 1) * MAX_QUOTIENT_OPERATIONS);
    assert_eq!(report.stored_trace_bytes(), 80);
    assert_eq!(report.unreduced_trace_bytes(), 3_225_600);
    assert_eq!(report.representatives()[0].replay().state().cells().len(), 8);
    assert!(report.progress().canonical_checks <= report.admission().required.canonical_checks);
}

#[test]
fn empty_and_singleton_populations_keep_their_exact_denominators() {
    let empty = run(&[], &state(&[]));
    assert_eq!(empty.admission().required.schedules, 1);
    assert_eq!(empty.progress().unreduced_replays, 1);
    assert_eq!(empty.progress().representative_replays, 1);
    assert_eq!(empty.progress().compared_schedules, 1);
    assert_eq!(empty.progress().replay_steps, 0);
    assert_eq!(empty.representatives().len(), 1);
    assert_eq!(empty.representatives()[0].members(), 1);
    assert_eq!(empty.stored_trace_bytes(), 16);
    assert_eq!(empty.representatives()[0].schedule().encode().len(), 16);

    let singleton = [occurrence(7, 9, ReplayInstruction::Read { domain: domain(1) })];
    let report = run(&singleton, &state(&[(1, 8)]));
    assert_eq!(report.progress().unreduced_replays, 1);
    assert_eq!(report.progress().representative_replays, 1);
    assert_eq!(report.progress().replay_steps, 2);
    assert_eq!(report.representatives()[0].replay().results()[0].value, ReplayValue::Observed(Some(8)));
}

#[test]
fn first_policy_replay_reports_fixed_population_trace_bytes_and_cold_work() {
    let operations = [
        occurrence(1, 1, ReplayInstruction::Write { domain: domain(1), value: Some(2) }),
        occurrence(2, 2, ReplayInstruction::Check { domain: domain(1), expected: Some(1) }),
        occurrence(3, 3, ReplayInstruction::Consume { resource: domain(2), units: 1 }),
    ];
    let initial = state(&[(1, 1), (2, 2)]);
    let started = Instant::now();
    let report = run(&operations, &initial);
    let elapsed = started.elapsed();
    assert_eq!(report.admission().required.schedules, 6);
    assert_eq!(report.progress().compared_schedules, 6);
    assert_eq!(report.representatives().len(), 2);
    assert_eq!(report.progress().replay_steps, 24);
    assert_eq!(report.unreduced_trace_bytes(), 240);
    assert_eq!(report.stored_trace_bytes(), 80);
    println!(
        "quotient_fixture occurrences={} admitted_schedules={} compared_schedules={} representatives={} unreduced_trace_bytes={} stored_trace_bytes={} cold_replay_steps={} canonical_checks={} comparison_entries={} mismatches=0 timing_samples=1 first_call_elapsed_ns={} scope=bounded_logical_order_trace",
        report.admission().occurrences,
        report.admission().required.schedules,
        report.progress().compared_schedules,
        report.representatives().len(),
        report.unreduced_trace_bytes(),
        report.stored_trace_bytes(),
        report.progress().replay_steps,
        report.progress().canonical_checks,
        report.progress().comparison_entries,
        elapsed.as_nanos(),
    );
}

#[test]
fn every_cross_category_mutation_pair_keeps_shared_cells_ordered() {
    for kind in 0..3 {
        let left = if kind == 1 {
            ReplayInstruction::Draw { stream: domain(1) }
        } else {
            ReplayInstruction::Consume { resource: domain(1), units: 1 }
        };
        for shared in [false, true] {
            let target = if shared { domain(1) } else { domain(2) };
            let right = if kind == 2 {
                ReplayInstruction::Draw { stream: target }
            } else {
                ReplayInstruction::Write { domain: target, value: Some(7) }
            };
            let operations = [occurrence(1, 1, left), occurrence(2, 2, right)];
            let initial = state(&[(1, 9), (2, 9)]);
            let report = run(&operations, &initial);
            assert_eq!(report.representatives().len(), if shared { 2 } else { 1 });
            let forward = replay(&operations, &initial, &[1, 2]).unwrap();
            let reverse = replay(&operations, &initial, &[2, 1]).unwrap();
            if shared {
                assert_ne!(forward, reverse);
            } else {
                assert_eq!(forward, reverse);
            }
        }
    }
}
