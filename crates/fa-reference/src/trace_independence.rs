//! Conservative registered trace-operation independence for the reference model.
//!
//! Every identifier in this module denotes a whole declared domain, not an
//! individual key. That is intentionally coarse: a write anywhere in a domain
//! conflicts with a positive or negative read of that domain. Registry entries
//! are caller-reviewed reference-contract input, not proof that the named
//! operators commute in a production executor.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use crate::Error;
use crate::full_input::{Omission, OpaqueInputWitness, PartKind};

/// Maximum declared domains in any one dependency class after input-derived
/// domains have been unioned into the validated operation.
pub const MAX_TRACE_DOMAINS: usize = 256;
/// Maximum retained opaque input bindings for one operation.
pub const MAX_TRACE_INPUT_BINDINGS: usize = 16;
/// Maximum caller-reviewed distinct operator pairs in one registry.
pub const MAX_TRACE_OPERATOR_PAIRS: usize = 128;
/// Maximum operations considered by one bounded relation evaluation.
pub const MAX_TRACE_OPERATIONS: usize = 16;

/// A nonzero registered operator identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperatorId(NonZeroU64);

impl OperatorId {
    pub fn new(value: u64) -> Result<Self, Error> {
        NonZeroU64::new(value).map(Self).ok_or(Error::InvalidInput)
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// A nonzero whole-domain identifier, deliberately not a key identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DomainId(NonZeroU64);

impl DomainId {
    pub fn new(value: u64) -> Result<Self, Error> {
        NonZeroU64::new(value).map(Self).ok_or(Error::InvalidInput)
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

/// A normalized, distinct pair of operators reviewed for reference commutation.
///
/// Same-operator invocations are intentionally ordered: this initial registry
/// admits only distinct operator IDs, producing conservative false positives
/// rather than disguising absent same-operator support as complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperatorPair {
    first: OperatorId,
    second: OperatorId,
}

impl OperatorPair {
    pub fn new(left: OperatorId, right: OperatorId) -> Result<Self, Error> {
        if left == right {
            return Err(Error::InvalidInput);
        }
        let (first, second) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        Ok(Self { first, second })
    }
}

/// Public, mutable construction input. It is consumed and validated before a
/// relation can observe it; it cannot clear witness-derived barriers afterward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceOperationSpec {
    pub operator: OperatorId,
    pub positive_read_domains: BTreeSet<DomainId>,
    pub write_domains: BTreeSet<DomainId>,
    pub rng_streams: BTreeSet<DomainId>,
    pub consuming_resources: BTreeSet<DomainId>,
    pub authority_transition: bool,
    pub ordered_effect: bool,
    pub external_effect: bool,
    pub unknown_dependencies: bool,
    pub inputs: Vec<TraceInputBindingSpec>,
}

/// Explicit mapping of every evidence source and omission in one retained input.
///
/// The mapping makes input provenance part of the caller-reviewed declaration;
/// the raw bytes and an opaque explanation are not parsed into semantic reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceInputBindingSpec {
    pub witness: OpaqueInputWitness,
    pub evidence_source_domains: BTreeMap<u64, DomainId>,
    pub omission_domains: BTreeMap<u64, DomainId>,
}

/// A bounded registry of reviewed, distinct operator-pair labels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceRelationRegistry {
    reviewed_commutative_pairs: BTreeSet<OperatorPair>,
}

impl TraceRelationRegistry {
    pub fn new(reviewed_commutative_pairs: BTreeSet<OperatorPair>) -> Result<Self, Error> {
        if reviewed_commutative_pairs.len() > MAX_TRACE_OPERATOR_PAIRS {
            return Err(Error::Limit);
        }
        Ok(Self {
            reviewed_commutative_pairs,
        })
    }

    fn contains(&self, left: OperatorId, right: OperatorId) -> bool {
        OperatorPair::new(left, right)
            .map(|pair| self.reviewed_commutative_pairs.contains(&pair))
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValidatedTraceOperation {
    operator: OperatorId,
    positive_read_domains: BTreeSet<DomainId>,
    negative_read_domains: BTreeSet<DomainId>,
    write_domains: BTreeSet<DomainId>,
    rng_streams: BTreeSet<DomainId>,
    consuming_resources: BTreeSet<DomainId>,
    authority_transition: bool,
    ordered_effect: bool,
    external_effect: bool,
    unknown_dependencies: bool,
    retained_inputs: Vec<OpaqueInputWitness>,
}

/// A validated operation with immutable, witness-derived dependency barriers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredTraceOperation {
    validated: ValidatedTraceOperation,
}

impl RegisteredTraceOperation {
    pub fn try_from_spec(spec: TraceOperationSpec) -> Result<Self, Error> {
        if spec.inputs.len() > MAX_TRACE_INPUT_BINDINGS {
            return Err(Error::Limit);
        }
        check_domain_set(&spec.positive_read_domains)?;
        check_domain_set(&spec.write_domains)?;
        check_domain_set(&spec.rng_streams)?;
        check_domain_set(&spec.consuming_resources)?;

        let mut positive_read_domains = spec.positive_read_domains;
        let mut negative_read_domains = BTreeSet::new();
        let mut unknown_dependencies = spec.unknown_dependencies;
        let mut retained_inputs = Vec::with_capacity(spec.inputs.len());

        for binding in spec.inputs {
            let actual_input = binding.witness.actual_input();
            let evidence_sources = actual_input
                .ordered_parts()
                .iter()
                .filter_map(|part| match &part.kind {
                    PartKind::Evidence { source_id, .. } => Some(*source_id),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            if !exact_mapping(&evidence_sources, &binding.evidence_source_domains) {
                return Err(Error::Binding);
            }
            positive_read_domains.extend(binding.evidence_source_domains.values().copied());

            let omissions = actual_input
                .omissions()
                .iter()
                .map(omission_domain_id)
                .collect::<BTreeSet<_>>();
            if !exact_mapping(&omissions, &binding.omission_domains) {
                return Err(Error::Binding);
            }
            for omission in actual_input.omissions() {
                let domain = binding.omission_domains[&omission_domain_id(omission)];
                match omission {
                    Omission::ClosedAbsent { .. } => {
                        negative_read_domains.insert(domain);
                    }
                    Omission::Gapped { .. }
                    | Omission::Unsupported { .. }
                    | Omission::Redacted { .. } => unknown_dependencies = true,
                }
            }
            retained_inputs.push(binding.witness);
        }

        check_domain_set(&positive_read_domains)?;
        check_domain_set(&negative_read_domains)?;

        Ok(Self {
            validated: ValidatedTraceOperation {
                operator: spec.operator,
                positive_read_domains,
                negative_read_domains,
                write_domains: spec.write_domains,
                rng_streams: spec.rng_streams,
                consuming_resources: spec.consuming_resources,
                authority_transition: spec.authority_transition,
                ordered_effect: spec.ordered_effect,
                external_effect: spec.external_effect,
                unknown_dependencies,
                retained_inputs,
            },
        })
    }

    pub fn operator(&self) -> OperatorId {
        self.validated.operator
    }

    pub fn positive_read_domains(&self) -> &BTreeSet<DomainId> {
        &self.validated.positive_read_domains
    }

    pub fn negative_read_domains(&self) -> &BTreeSet<DomainId> {
        &self.validated.negative_read_domains
    }

    pub fn unknown_dependencies(&self) -> bool {
        self.validated.unknown_dependencies
    }

    pub fn retained_inputs(&self) -> &[OpaqueInputWitness] {
        &self.validated.retained_inputs
    }
}

/// Returns whether the two operations may be reordered by this conservative
/// reference contract. A `true` result is neither a DPOR proof nor a production
/// executor authorization.
pub fn independent(
    registry: &TraceRelationRegistry,
    left: &RegisteredTraceOperation,
    right: &RegisteredTraceOperation,
) -> bool {
    let left = &left.validated;
    let right = &right.validated;

    registry.contains(left.operator, right.operator)
        && !left.unknown_dependencies
        && !right.unknown_dependencies
        && !left.authority_transition
        && !right.authority_transition
        && !left.ordered_effect
        && !right.ordered_effect
        && !left.external_effect
        && !right.external_effect
        && !mutation_conflicts(left, right)
        && !mutation_conflicts(right, left)
}

// All dependency classes share the same whole-domain namespace. RNG state
// advancement and resource consumption are mutations just as declared writes
// are; a collision with any other access must preserve order. Read/read overlap
// alone remains harmless under the caller-reviewed operator law.
fn mutation_conflicts(left: &ValidatedTraceOperation, right: &ValidatedTraceOperation) -> bool {
    left.write_domains
        .iter()
        .chain(&left.rng_streams)
        .chain(&left.consuming_resources)
        .any(|domain| {
            right.positive_read_domains.contains(domain)
                || right.negative_read_domains.contains(domain)
                || right.write_domains.contains(domain)
                || right.rng_streams.contains(domain)
                || right.consuming_resources.contains(domain)
        })
}

/// Count evidence retained from exhaustively evaluating distinct unordered
/// operation-position pairs in a bounded operation set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationEvaluation {
    pub evaluated_pairs: usize,
    pub independent_pairs: usize,
    pub ordered_pairs: usize,
}

pub fn evaluate_bounded_pairs(
    registry: &TraceRelationRegistry,
    operations: &[RegisteredTraceOperation],
) -> Result<RelationEvaluation, Error> {
    if operations.len() > MAX_TRACE_OPERATIONS {
        return Err(Error::Limit);
    }
    let evaluated_pairs = operations
        .len()
        .checked_mul(operations.len().saturating_sub(1))
        .ok_or(Error::Overflow)?
        / 2;
    let independent_pairs = operations
        .iter()
        .enumerate()
        .flat_map(|(index, left)| {
            operations[index + 1..]
                .iter()
                .map(move |right| (left, right))
        })
        .filter(|(left, right)| independent(registry, left, right))
        .count();
    Ok(RelationEvaluation {
        evaluated_pairs,
        independent_pairs,
        ordered_pairs: evaluated_pairs - independent_pairs,
    })
}

fn check_domain_set(domains: &BTreeSet<DomainId>) -> Result<(), Error> {
    if domains.len() > MAX_TRACE_DOMAINS {
        return Err(Error::Limit);
    }
    Ok(())
}

fn exact_mapping(keys: &BTreeSet<u64>, mapping: &BTreeMap<u64, DomainId>) -> bool {
    keys.len() == mapping.len() && keys.iter().all(|key| mapping.contains_key(key))
}

fn omission_domain_id(omission: &Omission) -> u64 {
    match omission {
        Omission::ClosedAbsent { domain_id, .. }
        | Omission::Gapped { domain_id, .. }
        | Omission::Unsupported { domain_id }
        | Omission::Redacted { domain_id, .. } => *domain_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::full_input::{
        ActualHelperInput, ByteSpan, InputProfileBinding, OpaqueJudgment, SubmittedPart,
    };

    fn operator(value: u64) -> OperatorId {
        OperatorId::new(value).unwrap()
    }

    fn domain(value: u64) -> DomainId {
        DomainId::new(value).unwrap()
    }

    fn domains(values: &[u64]) -> BTreeSet<DomainId> {
        values.iter().copied().map(domain).collect()
    }

    fn input(evidence_sources: &[u64], omissions: Vec<Omission>) -> ActualHelperInput {
        let mut bytes = vec![b'Q'];
        let mut parts = vec![SubmittedPart {
            span: ByteSpan { start: 0, end: 1 },
            kind: PartKind::Question,
        }];
        for source_id in evidence_sources {
            let start = bytes.len();
            bytes.push(b'E');
            parts.push(SubmittedPart {
                span: ByteSpan {
                    start,
                    end: start + 1,
                },
                kind: PartKind::Evidence {
                    source_id: *source_id,
                    transform_id: 1,
                },
            });
        }
        ActualHelperInput::new(
            bytes,
            InputProfileBinding {
                profile_id: 1,
                profile_bytes: Vec::new(),
                tokenizer_epoch: 0,
                policy_epoch: 0,
                model_epoch: 0,
            },
            parts,
            omissions,
        )
        .unwrap()
    }

    fn witness(evidence_sources: &[u64], omissions: Vec<Omission>) -> OpaqueInputWitness {
        let input = input(evidence_sources, omissions);
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

    fn all_distinct_pairs(operators: &[u64]) -> TraceRelationRegistry {
        let mut pairs = BTreeSet::new();
        for (index, left) in operators.iter().enumerate() {
            for right in &operators[index + 1..] {
                pairs.insert(OperatorPair::new(operator(*left), operator(*right)).unwrap());
            }
        }
        TraceRelationRegistry::new(pairs).unwrap()
    }

    #[test]
    fn registered_disjoint_operations_and_shared_read_only_pairs_commute() {
        let registry = all_distinct_pairs(&[1, 2]);
        let mut left = spec(1);
        left.positive_read_domains = domains(&[10]);
        left.write_domains = domains(&[11]);
        let mut right = spec(2);
        right.positive_read_domains = domains(&[10]);
        right.write_domains = domains(&[12]);

        assert!(independent(
            &registry,
            &registered(left),
            &registered(right)
        ));
    }

    #[test]
    fn isolated_read_write_and_barrier_conflicts_preserve_order() {
        let registry = all_distinct_pairs(&[1, 2]);
        let mut reader = spec(1);
        reader.positive_read_domains = domains(&[10]);
        let mut writer = spec(2);
        writer.write_domains = domains(&[10]);
        assert!(!independent(
            &registry,
            &registered(reader),
            &registered(writer)
        ));

        let closed = TraceInputBindingSpec {
            witness: witness(
                &[7],
                vec![Omission::ClosedAbsent {
                    domain_id: 8,
                    trusted_closure_marker_id: 9,
                }],
            ),
            evidence_source_domains: BTreeMap::from([(7, domain(70))]),
            omission_domains: BTreeMap::from([(8, domain(80))]),
        };
        let mut negative_reader = spec(1);
        negative_reader.inputs.push(closed);
        let mut negative_writer = spec(2);
        negative_writer.write_domains = domains(&[80]);
        let negative_reader = registered(negative_reader);
        assert_eq!(negative_reader.positive_read_domains(), &domains(&[70]));
        assert_eq!(negative_reader.negative_read_domains(), &domains(&[80]));
        assert_eq!(negative_reader.retained_inputs().len(), 1);
        assert!(!independent(
            &registry,
            &negative_reader,
            &registered(negative_writer)
        ));
    }

    #[test]
    fn registered_barriers_individually_preserve_order() {
        let registry = all_distinct_pairs(&[1, 2]);

        let mut left = spec(1);
        left.rng_streams = domains(&[20]);
        let mut right = spec(2);
        right.rng_streams = domains(&[20]);
        assert!(!independent(
            &registry,
            &registered(left),
            &registered(right)
        ));

        let mut left = spec(1);
        left.consuming_resources = domains(&[21]);
        let mut right = spec(2);
        right.consuming_resources = domains(&[21]);
        assert!(!independent(
            &registry,
            &registered(left),
            &registered(right)
        ));

        let mut left = spec(1);
        left.authority_transition = true;
        assert!(!independent(
            &registry,
            &registered(left),
            &registered(spec(2))
        ));

        let mut left = spec(1);
        left.ordered_effect = true;
        assert!(!independent(
            &registry,
            &registered(left),
            &registered(spec(2))
        ));

        let mut left = spec(1);
        left.external_effect = true;
        assert!(!independent(
            &registry,
            &registered(left),
            &registered(spec(2))
        ));

        let mut left = spec(1);
        left.unknown_dependencies = true;
        assert!(!independent(
            &registry,
            &registered(left),
            &registered(spec(2))
        ));
    }

    #[test]
    fn malformed_bindings_and_derived_unknown_dependencies_are_rejected_or_preserved() {
        let closed_witness = witness(
            &[7],
            vec![Omission::ClosedAbsent {
                domain_id: 8,
                trusted_closure_marker_id: 9,
            }],
        );
        let mut missing_source = spec(1);
        missing_source.inputs.push(TraceInputBindingSpec {
            witness: closed_witness.clone(),
            evidence_source_domains: BTreeMap::new(),
            omission_domains: BTreeMap::from([(8, domain(80))]),
        });
        assert_eq!(
            RegisteredTraceOperation::try_from_spec(missing_source),
            Err(Error::Binding)
        );

        let mut extra_source = spec(1);
        extra_source.inputs.push(TraceInputBindingSpec {
            witness: closed_witness.clone(),
            evidence_source_domains: BTreeMap::from([(7, domain(70)), (99, domain(99))]),
            omission_domains: BTreeMap::from([(8, domain(80))]),
        });
        assert_eq!(
            RegisteredTraceOperation::try_from_spec(extra_source),
            Err(Error::Binding)
        );

        let mut missing_omission = spec(1);
        missing_omission.inputs.push(TraceInputBindingSpec {
            witness: closed_witness.clone(),
            evidence_source_domains: BTreeMap::from([(7, domain(70))]),
            omission_domains: BTreeMap::new(),
        });
        assert_eq!(
            RegisteredTraceOperation::try_from_spec(missing_omission),
            Err(Error::Binding)
        );

        let mut extra_omission = spec(1);
        extra_omission.inputs.push(TraceInputBindingSpec {
            witness: closed_witness,
            evidence_source_domains: BTreeMap::from([(7, domain(70))]),
            omission_domains: BTreeMap::from([(8, domain(80)), (99, domain(99))]),
        });
        assert_eq!(
            RegisteredTraceOperation::try_from_spec(extra_omission),
            Err(Error::Binding)
        );

        let registry = all_distinct_pairs(&[1, 2]);
        for omission in [
            Omission::Gapped {
                domain_id: 8,
                first_missing: 3,
            },
            Omission::Unsupported { domain_id: 8 },
            Omission::Redacted {
                domain_id: 8,
                transform_id: 4,
            },
        ] {
            let mut incomplete = spec(1);
            incomplete.inputs.push(TraceInputBindingSpec {
                witness: witness(&[], vec![omission]),
                evidence_source_domains: BTreeMap::new(),
                omission_domains: BTreeMap::from([(8, domain(80))]),
            });
            let incomplete = registered(incomplete);
            assert!(incomplete.unknown_dependencies());
            assert!(!independent(&registry, &incomplete, &registered(spec(2))));
        }
        assert_eq!(OperatorId::new(0), Err(Error::InvalidInput));
        assert_eq!(DomainId::new(0), Err(Error::InvalidInput));
        assert_eq!(
            OperatorPair::new(operator(1), operator(1)),
            Err(Error::InvalidInput)
        );
    }

    #[test]
    fn bounded_pair_evaluation_exhausts_distinct_unordered_pairs() {
        let operator_ids = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
        let registry = all_distinct_pairs(&operator_ids);
        let mut operations = Vec::new();

        let mut first_reader = spec(1);
        first_reader.positive_read_domains = domains(&[1]);
        first_reader.write_domains = domains(&[2]);
        operations.push(registered(first_reader));
        let mut second_reader = spec(2);
        second_reader.positive_read_domains = domains(&[1]);
        second_reader.write_domains = domains(&[3]);
        operations.push(registered(second_reader));
        let mut read_conflict = spec(3);
        read_conflict.write_domains = domains(&[1]);
        operations.push(registered(read_conflict));

        let mut negative_reader = spec(4);
        negative_reader.inputs.push(TraceInputBindingSpec {
            witness: witness(
                &[],
                vec![Omission::ClosedAbsent {
                    domain_id: 4,
                    trusted_closure_marker_id: 1,
                }],
            ),
            evidence_source_domains: BTreeMap::new(),
            omission_domains: BTreeMap::from([(4, domain(4))]),
        });
        operations.push(registered(negative_reader));
        let mut negative_conflict = spec(5);
        negative_conflict.write_domains = domains(&[4]);
        operations.push(registered(negative_conflict));

        let mut first_rng = spec(6);
        first_rng.rng_streams = domains(&[5]);
        operations.push(registered(first_rng));
        let mut second_rng = spec(7);
        second_rng.rng_streams = domains(&[5]);
        operations.push(registered(second_rng));
        let mut first_resource = spec(8);
        first_resource.consuming_resources = domains(&[6]);
        operations.push(registered(first_resource));
        let mut second_resource = spec(9);
        second_resource.consuming_resources = domains(&[6]);
        operations.push(registered(second_resource));

        let mut authority = spec(10);
        authority.authority_transition = true;
        operations.push(registered(authority));
        let mut ordered = spec(11);
        ordered.ordered_effect = true;
        operations.push(registered(ordered));
        let mut external = spec(12);
        external.external_effect = true;
        operations.push(registered(external));
        let mut unknown = spec(13);
        unknown.unknown_dependencies = true;
        operations.push(registered(unknown));

        let evaluation = evaluate_bounded_pairs(&registry, &operations).unwrap();
        assert_eq!(evaluation.evaluated_pairs, 78);
        assert_eq!(evaluation.independent_pairs, 31);
        assert_eq!(evaluation.ordered_pairs, 47);
    }

    #[test]
    fn bounded_pair_evaluation_counts_only_distinct_unordered_positions() {
        let registry = all_distinct_pairs(&[1, 2]);
        let pair = vec![registered(spec(1)), registered(spec(2))];
        assert_eq!(
            evaluate_bounded_pairs(&registry, &pair).unwrap(),
            RelationEvaluation {
                evaluated_pairs: 1,
                independent_pairs: 1,
                ordered_pairs: 0,
            }
        );
        assert_eq!(
            evaluate_bounded_pairs(&registry, &[]).unwrap(),
            RelationEvaluation {
                evaluated_pairs: 0,
                independent_pairs: 0,
                ordered_pairs: 0,
            }
        );
        assert_eq!(
            evaluate_bounded_pairs(&registry, &[registered(spec(1))]).unwrap(),
            RelationEvaluation {
                evaluated_pairs: 0,
                independent_pairs: 0,
                ordered_pairs: 0,
            }
        );
    }

    #[test]
    fn bounds_cover_initial_and_post_union_domain_sizes_and_trace_counts() {
        let mut too_many_domains = spec(1);
        too_many_domains.positive_read_domains =
            (1..=MAX_TRACE_DOMAINS as u64 + 1).map(domain).collect();
        assert_eq!(
            RegisteredTraceOperation::try_from_spec(too_many_domains),
            Err(Error::Limit)
        );

        let mut post_union = spec(1);
        post_union.positive_read_domains = (1..MAX_TRACE_DOMAINS as u64).map(domain).collect();
        post_union.inputs.push(TraceInputBindingSpec {
            witness: witness(&[1, 2], Vec::new()),
            evidence_source_domains: BTreeMap::from([
                (1, domain(MAX_TRACE_DOMAINS as u64)),
                (2, domain(MAX_TRACE_DOMAINS as u64 + 1)),
            ]),
            omission_domains: BTreeMap::new(),
        });
        assert_eq!(
            RegisteredTraceOperation::try_from_spec(post_union),
            Err(Error::Limit)
        );

        let pairs = (2..=MAX_TRACE_OPERATOR_PAIRS as u64 + 2)
            .map(|right| OperatorPair::new(operator(1), operator(right)).unwrap())
            .collect();
        assert_eq!(TraceRelationRegistry::new(pairs), Err(Error::Limit));

        let mut too_many_inputs = spec(1);
        for _ in 0..=MAX_TRACE_INPUT_BINDINGS {
            too_many_inputs.inputs.push(TraceInputBindingSpec {
                witness: witness(&[], Vec::new()),
                evidence_source_domains: BTreeMap::new(),
                omission_domains: BTreeMap::new(),
            });
        }
        assert_eq!(
            RegisteredTraceOperation::try_from_spec(too_many_inputs),
            Err(Error::Limit)
        );

        let operation = registered(spec(1));
        let operations = vec![operation; MAX_TRACE_OPERATIONS + 1];
        assert_eq!(
            evaluate_bounded_pairs(&all_distinct_pairs(&[1, 2]), &operations),
            Err(Error::Limit)
        );
    }
}
