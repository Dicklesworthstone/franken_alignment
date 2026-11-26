//! Indexed standing maintenance is checked against the independent public batch
//! evaluator after every accepted frontier. Work counters below describe only
//! logical indexed/oracle visits; they are not timing or total-allocation claims.

use std::collections::BTreeSet;

use fa_reference::observation_algebra::{
    Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
};
use fa_reference::product_frontier::{
    FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey,
};
use fa_reference::standing_oversight::{
    AntichainOverflowMode, BaseFact, BaseFactKind, FindingKey, IncrementalOversightEngine,
    MAX_SNAPSHOT_FACTS, OversightSnapshot, SemanticGenerationVector, SignedFactDelta,
    StandingOversightError, StandingQueryFamily, SupportPrecision, ViewReceipt, batch_oracle,
};

fn generations() -> SemanticGenerationVector {
    SemanticGenerationVector {
        model_generation: 1,
        codec_generation: 1,
        monitor_generation: 1,
        policy_generation: 1,
        calibration_generation: 1,
    }
}

fn projection() -> ProjectionKey {
    ProjectionKey {
        source: 1,
        branch: 1,
        projection: 1,
        source_epoch: 1,
    }
}

fn basis() -> Basis {
    let declared = ScopedProof::Declared {
        scope_id: 1,
        generation: 1,
    };
    Basis::new(
        BasisIdentity {
            subject_id: 1,
            question_id: 2,
            profile_id: 3,
            semantic_generation: 1,
            policy_epoch: 1,
            model_epoch: 1,
            tokenizer_epoch: 1,
            codec_epoch: 1,
            control_seq: 1,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: declared,
            complete_for_contract: declared,
            semantically_valid: declared,
            available_for_replay: declared,
        },
    )
}

fn delta(id: u64, kind: BaseFactKind, weight: i64) -> SignedFactDelta {
    SignedFactDelta {
        fact: BaseFact { fact_id: id, kind },
        weight,
    }
}
fn edge(id: u64, from: u64, to: u64, weight: i64) -> SignedFactDelta {
    delta(id, BaseFactKind::AuthorityEdge { from, to }, weight)
}
fn gate(id: u64, from: u64, to: u64, weight: i64) -> SignedFactDelta {
    delta(id, BaseFactKind::QualifiedGate { from, to }, weight)
}
fn cohort(id: u64, subject: u64, weight: i64) -> SignedFactDelta {
    delta(
        id,
        BaseFactKind::CohortFailure {
            cohort: subject,
            failure_id: 20,
        },
        weight,
    )
}
fn finding(query: StandingQueryFamily, subject: u64, target: u64) -> FindingKey {
    FindingKey {
        query,
        subject,
        target,
    }
}

struct Harness {
    engine: IncrementalOversightEngine,
    snapshot: OversightSnapshot,
    query: StandingQueryFamily,
    sequence: u64,
}

impl Harness {
    fn new(query: StandingQueryFamily, width: usize, mode: AntichainOverflowMode) -> Self {
        Self {
            engine: IncrementalOversightEngine::new(
                basis(),
                query,
                generations(),
                projection(),
                width,
                mode,
            )
            .unwrap(),
            snapshot: OversightSnapshot::new(
                generations(),
                true,
                ProductFrontiers::new(1, 4).unwrap(),
                FrontierRequirement {
                    key: projection(),
                    stage: FrontierStage::Authenticated,
                    through: 0,
                    closure: None,
                },
            ),
            query,
            sequence: 0,
        }
    }

    fn close(&mut self, deltas: &[SignedFactDelta]) -> ViewReceipt {
        let next = self.sequence + 1;
        // This independent model retains only original base facts and frontiers;
        // it never reads the engine's indexes, cached results or work counters.
        let mut expected = self.snapshot.clone();
        for &change in deltas {
            expected.apply_fact_delta(change).unwrap();
        }
        expected
            .frontiers
            .accept(projection(), FrontierStage::Captured, next)
            .unwrap();
        expected
            .frontiers
            .accept(projection(), FrontierStage::Authenticated, next)
            .unwrap();
        expected.required_frontier.through = next;
        let batch =
            batch_oracle::evaluate_batch_query(&expected, self.query, MAX_SNAPSHOT_FACTS).unwrap();
        let Knowledge::Known { value, .. } = self
            .engine
            .apply_delta_and_close_frontier(expected.generations, deltas, next, true, true)
            .unwrap()
        else {
            panic!("accepted reference update must be known");
        };
        assert_eq!(
            (value.from_frontier_seq, value.to_frontier_seq),
            (self.sequence, next)
        );
        assert_eq!(
            value.findings.keys().collect::<Vec<_>>(),
            batch.findings.keys().collect::<Vec<_>>()
        );
        for (key, actual) in &value.findings {
            let exact = &batch.findings[key];
            assert_eq!(actual.total_support_weight, exact.total_support_weight);
            match actual.precision {
                SupportPrecision::ExactMinimalAntichain => assert_eq!(actual, exact),
                SupportPrecision::CoarsenedConservative {
                    retained_union_atoms,
                } => {
                    assert!(actual.minimal_supports.len() < exact.minimal_supports.len());
                    assert!(
                        actual
                            .minimal_supports
                            .iter()
                            .all(|path| exact.minimal_supports.contains(path))
                    );
                    let union: BTreeSet<_> = exact
                        .minimal_supports
                        .iter()
                        .flat_map(|path| path.positive_atoms.iter().copied())
                        .collect();
                    assert_eq!(retained_union_atoms, union.len());
                }
            }
        }
        assert!(value.batch_work_units >= batch.tuples_examined);
        self.sequence = next;
        self.snapshot = expected;
        value
    }

    fn reject(&mut self, deltas: &[SignedFactDelta], error: StandingOversightError) {
        assert_eq!(
            self.engine.apply_delta_and_close_frontier(
                self.snapshot.generations,
                deltas,
                self.sequence + 1,
                true,
                true,
            ),
            Err(error),
        );
    }
}

#[test]
fn one_changed_path_refreshes_one_finding_and_irrelevant_batches_refresh_none() {
    let query = StandingQueryFamily::UngatedSinkReachability;
    let mut h = Harness::new(query, 4, AntichainOverflowMode::FailClosed);
    let mut initial = Vec::new();
    for n in 0..60 {
        let start = 1000 + 3 * n;
        initial.push(edge(2 * n + 1, start, start + 1, 1));
        initial.push(edge(2 * n + 2, start + 1, start + 2, 1));
    }
    let first = h.close(&initial);
    assert_eq!(first.findings.len(), 60);
    let changed_key = finding(query, 1000, 1002);
    let changed = h.close(&[edge(1, 1000, 1001, 2)]);
    assert_eq!(changed.affected_findings, 1);
    assert_eq!(changed.findings[&changed_key].total_support_weight, 3);
    for (key, original) in &first.findings {
        if *key != changed_key {
            assert_eq!(&changed.findings[key], original);
        }
    }
    assert!(changed.incremental_work_units < 20);
    assert_eq!(changed.batch_work_units, 120 * 120);

    let irrelevant = h.close(&[cohort(500, 77, 1)]);
    assert_eq!(irrelevant.findings, changed.findings);
    assert_eq!(irrelevant.affected_findings, 0);
    assert_eq!(irrelevant.incremental_work_units, 1);
    assert!(irrelevant.batch_work_units > changed.batch_work_units);
    let empty = h.close(&[]);
    assert_eq!(empty.findings, changed.findings);
    assert_eq!(empty.affected_findings, 0);
    assert_eq!(empty.incremental_work_units, 0);
    assert_eq!(empty.batch_work_units, irrelevant.batch_work_units);
}

#[test]
fn weighted_delta_joins_and_duplicate_blockers_preserve_all_independent_supports() {
    let query = StandingQueryFamily::UngatedSinkReachability;
    let key = finding(query, 10, 90);
    let mut h = Harness::new(query, 8, AntichainOverflowMode::FailClosed);
    let initial = h.close(&[
        edge(1, 10, 20, 2),
        edge(2, 20, 90, 3),
        edge(3, 10, 30, 1),
        edge(4, 30, 90, 1),
    ]);
    assert_eq!(initial.findings[&key].total_support_weight, 7);
    let multiplied = h.close(&[edge(1, 10, 20, 1), edge(2, 20, 90, 1)]);
    assert_eq!(multiplied.findings[&key].total_support_weight, 13);
    assert_eq!(multiplied.findings[&key].minimal_supports.len(), 2);
    assert_eq!(multiplied.affected_findings, 1);

    let blocked = h.close(&[gate(50, 10, 20, 2), gate(51, 10, 20, 1)]);
    assert_eq!(blocked.findings[&key].total_support_weight, 1);
    assert_eq!(h.close(&[gate(50, 10, 20, -1)]).findings, blocked.findings);
    assert_eq!(h.close(&[gate(51, 10, 20, -1)]).findings, blocked.findings);
    h.reject(
        &[gate(52, 10, 20, -1)],
        StandingOversightError::LostRetractionOrNegativeWeight,
    );
    let restored = h.close(&[gate(50, 10, 20, -1)]);
    assert_eq!(restored.findings, multiplied.findings);
    let removed = h.close(&[edge(1, 10, 20, -3)]);
    assert_eq!(removed.findings[&key].total_support_weight, 1);
    assert_eq!(
        removed.findings[&key].minimal_supports[0].positive_atoms,
        BTreeSet::from([3, 4])
    );
}

#[test]
fn invalidation_index_updates_only_matching_proposals_and_routes_reused_ids() {
    let query = StandingQueryFamily::InvalidatedHeldProposal;
    let held = |id, proposal, key, weight| {
        delta(
            id,
            BaseFactKind::HeldProposalDependency { proposal, key },
            weight,
        )
    };
    let invalidated = |id, key, revision, weight| {
        delta(id, BaseFactKind::InvalidatedKey { key, revision }, weight)
    };
    let mut h = Harness::new(query, 8, AntichainOverflowMode::FailClosed);
    let original = h.close(&[
        held(1, 10, 7, 2),
        held(2, 20, 7, 3),
        held(3, 30, 8, 1),
        invalidated(10, 7, 1, 4),
        invalidated(11, 7, 2, 5),
        invalidated(12, 8, 1, 1),
    ]);
    assert_eq!(
        original.findings[&finding(query, 10, 7)].total_support_weight,
        18
    );
    assert_eq!(
        original.findings[&finding(query, 20, 7)].total_support_weight,
        27
    );
    let changed = h.close(&[invalidated(10, 7, 1, -1)]);
    assert_eq!(changed.affected_findings, 2);
    assert_eq!(
        changed.findings[&finding(query, 10, 7)].total_support_weight,
        16
    );
    assert_eq!(
        changed.findings[&finding(query, 20, 7)].total_support_weight,
        24
    );
    assert_eq!(
        changed.findings[&finding(query, 30, 8)],
        original.findings[&finding(query, 30, 8)]
    );
    h.reject(
        &[held(1, 40, 8, -2)],
        StandingOversightError::FactIdentityMismatch,
    );
    let moved = h.close(&[held(1, 10, 7, -2), held(1, 40, 8, 2)]);
    assert_eq!(moved.affected_findings, 2);
    assert!(!moved.findings.contains_key(&finding(query, 10, 7)));
    assert_eq!(
        moved.findings[&finding(query, 40, 8)].total_support_weight,
        2
    );
    let changed_kind = h.close(&[held(1, 40, 8, -2), cohort(1, 99, 1)]);
    assert_eq!(changed_kind.affected_findings, 1);
    assert!(!changed_kind.findings.contains_key(&finding(query, 40, 8)));
    assert_eq!(changed_kind.findings.len(), 2);
}

#[test]
fn final_closed_replacement_does_not_enforce_a_transient_antichain_width() {
    let query = StandingQueryFamily::SharedHelperFailureCohort;
    let key = finding(query, 10, 20);
    let mut h = Harness::new(query, 1, AntichainOverflowMode::FailClosed);
    h.close(&[cohort(1, 10, 1)]);
    // The final support fits even though processing +new before -old has two.
    let replaced = h.close(&[cohort(2, 10, 1), cohort(1, 10, -1)]);
    assert_eq!(replaced.affected_findings, 1);
    assert_eq!(
        replaced.findings[&key].minimal_supports[0].positive_atoms,
        BTreeSet::from([2])
    );
    h.reject(
        &[cohort(3, 10, 1)],
        StandingOversightError::AntichainWidthExceeded {
            width: 2,
            max_width: 1,
        },
    );
    let accepted = h.close(&[cohort(3, 30, 1)]);
    assert_eq!(accepted.findings.len(), 2);
    assert_eq!(accepted.findings[&key], replaced.findings[&key]);
}

#[test]
fn coarsened_and_spilled_supports_survive_idle_cuts_and_expose_hidden_alternatives() {
    let query = StandingQueryFamily::UngatedSinkReachability;
    let key = finding(query, 10, 90);
    for mode in [
        AntichainOverflowMode::ConservativeCoarsen,
        AntichainOverflowMode::SpillToBatchRecompute,
    ] {
        let mut h = Harness::new(query, 1, mode);
        let first = h.close(&[
            edge(1, 10, 20, 1),
            edge(2, 20, 90, 1),
            edge(3, 10, 30, 1),
            edge(4, 30, 90, 1),
        ]);
        assert_eq!(first.findings[&key].total_support_weight, 2);
        assert_eq!(
            first.coarsened_findings,
            usize::from(mode == AntichainOverflowMode::ConservativeCoarsen)
        );
        assert_eq!(
            first.spilled_to_batch_recompute,
            mode == AntichainOverflowMode::SpillToBatchRecompute
        );
        for unrelated in [vec![cohort(99, 99, 1)], Vec::new()] {
            let unchanged = h.close(&unrelated);
            assert_eq!(unchanged.affected_findings, 0);
            assert_eq!(unchanged.findings, first.findings);
            assert_eq!(unchanged.coarsened_findings, first.coarsened_findings);
            assert_eq!(
                unchanged.spilled_to_batch_recompute,
                first.spilled_to_batch_recompute
            );
        }
        // Retract a support omitted from the coarsened rendering.
        let one = h.close(&[edge(3, 10, 30, -1)]);
        assert_eq!(one.findings[&key].total_support_weight, 1);
        assert_eq!(one.coarsened_findings, 0);
        assert!(!one.spilled_to_batch_recompute);
        h.close(&[edge(3, 10, 30, 1)]);
        // The previously hidden support must now become the complete result.
        let revealed = h.close(&[edge(1, 10, 20, -1)]);
        assert_eq!(
            revealed.findings[&key].minimal_supports[0].positive_atoms,
            BTreeSet::from([3, 4])
        );
        assert_eq!(revealed.findings[&key].total_support_weight, 1);
        assert_eq!(revealed.coarsened_findings, 0);
        assert!(!revealed.spilled_to_batch_recompute);
        assert_eq!(
            revealed.findings[&key].precision,
            SupportPrecision::ExactMinimalAntichain
        );
    }
}

#[test]
fn negative_guard_hash_collisions_do_not_join_exact_blocker_indexes() {
    let a = (10, 2_000_006);
    let b = (11, 1_000_003);
    assert_eq!(a.0 * 1_000_003 + a.1, b.0 * 1_000_003 + b.1);
    let query = StandingQueryFamily::UnreviewedArtifactDependency;
    let mut h = Harness::new(query, 4, AntichainOverflowMode::FailClosed);
    let dep = |id, pair: (u64, u64)| {
        delta(
            id,
            BaseFactKind::ArtifactDependency {
                artifact: pair.0,
                dependency: pair.1,
            },
            1,
        )
    };
    let review = |id, pair: (u64, u64), weight| {
        delta(
            id,
            BaseFactKind::ReviewedDependency {
                artifact: pair.0,
                dependency: pair.1,
            },
            weight,
        )
    };
    let first = h.close(&[dep(1, a), dep(2, b)]);
    let blocked = h.close(&[review(3, a, 2), review(4, a, 1)]);
    assert_eq!(blocked.affected_findings, 1);
    assert!(!blocked.findings.contains_key(&finding(query, a.0, a.1)));
    assert_eq!(
        blocked.findings[&finding(query, b.0, b.1)],
        first.findings[&finding(query, b.0, b.1)]
    );
    assert_eq!(h.close(&[review(3, a, -2)]).findings, blocked.findings);
    assert_eq!(h.close(&[review(4, a, -1)]).findings, first.findings);

    let query = StandingQueryFamily::UngatedSinkReachability;
    let mut h = Harness::new(query, 4, AntichainOverflowMode::FailClosed);
    let first = h.close(&[
        edge(1, a.0, a.1, 1),
        edge(2, a.1, 30, 1),
        edge(3, b.0, b.1, 1),
        edge(4, b.1, 40, 1),
    ]);
    let blocked = h.close(&[gate(5, a.0, a.1, 1)]);
    assert_eq!(blocked.affected_findings, 1);
    assert_eq!(blocked.findings.len(), 1);
    assert_eq!(
        blocked.findings[&finding(query, b.0, 40)],
        first.findings[&finding(query, b.0, 40)]
    );
}

#[test]
fn snapshot_limit_rollback_and_bucket_pruning_preserve_bounded_current_state() {
    let query = StandingQueryFamily::SharedHelperFailureCohort;
    let mut h = Harness::new(query, 1, AntichainOverflowMode::FailClosed);
    for start in [1, 257] {
        let changes: Vec<_> = (start..start + 256).map(|id| cohort(id, id, 1)).collect();
        h.close(&changes);
    }
    let full = h.close(&[]);
    assert_eq!(full.findings.len(), MAX_SNAPSHOT_FACTS);
    h.reject(
        &[cohort(1, 1, -1), cohort(513, 513, 1), cohort(514, 514, 1)],
        StandingOversightError::SnapshotLimitExceeded,
    );
    let rolled_back = h.close(&[]);
    assert_eq!(rolled_back.findings, full.findings);
    assert_eq!(rolled_back.retained_state_bytes, full.retained_state_bytes);
    let replaced = h.close(&[cohort(1, 1, -1), cohort(513, 513, 1)]);
    assert_eq!(replaced.affected_findings, 2);
    assert!(!replaced.findings.contains_key(&finding(query, 514, 20)));
    assert_eq!(replaced.findings.len(), MAX_SNAPSHOT_FACTS);

    let mut tiny = Harness::new(query, 1, AntichainOverflowMode::FailClosed);
    let empty_bytes = tiny.close(&[]).retained_state_bytes;
    let occupied_bytes = tiny.close(&[cohort(1, 100, 1)]).retained_state_bytes;
    for subject in 101..201 {
        let moved = tiny.close(&[cohort(1, subject - 1, -1), cohort(1, subject, 1)]);
        assert_eq!(moved.findings.len(), 1);
        assert_eq!(moved.affected_findings, 2);
        assert_eq!(moved.retained_state_bytes, occupied_bytes);
    }
    let empty = tiny.close(&[cohort(1, 200, -1)]);
    assert!(empty.findings.is_empty());
    assert_eq!(empty.retained_state_bytes, empty_bytes);
}

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn generated_fact(id: u64, seed: u64) -> BaseFact {
    let a = 1 + (seed / 7) % 7;
    let b = 1 + (seed / 49) % 7;
    let kind = match seed % 7 {
        0 => BaseFactKind::AuthorityEdge { from: a, to: b },
        1 => BaseFactKind::QualifiedGate { from: a, to: b },
        2 => BaseFactKind::ArtifactDependency {
            artifact: a,
            dependency: b,
        },
        3 => BaseFactKind::ReviewedDependency {
            artifact: a,
            dependency: b,
        },
        4 => BaseFactKind::HeldProposalDependency {
            proposal: a,
            key: b,
        },
        5 => BaseFactKind::InvalidatedKey {
            key: b,
            revision: a,
        },
        _ => BaseFactKind::CohortFailure {
            cohort: a,
            failure_id: b,
        },
    };
    BaseFact { fact_id: id, kind }
}

#[test]
fn mixed_weighted_mutation_histories_match_full_batch_results_at_every_cut() {
    for query in [
        StandingQueryFamily::UngatedSinkReachability,
        StandingQueryFamily::UnreviewedArtifactDependency,
        StandingQueryFamily::InvalidatedHeldProposal,
        StandingQueryFamily::SharedHelperFailureCohort,
    ] {
        let mut h = Harness::new(query, MAX_SNAPSHOT_FACTS, AntichainOverflowMode::FailClosed);
        let mut random = 0x81c9_e402_ab51_763d;
        let initial: Vec<_> = (1..=24)
            .map(|id| SignedFactDelta {
                fact: generated_fact(id, next_random(&mut random)),
                weight: 1,
            })
            .collect();
        h.close(&initial);
        for _ in 0..96 {
            let id = 1 + next_random(&mut random) % 24;
            let choice = next_random(&mut random);
            let mut changes = Vec::new();
            if let Some(&(fact, weight)) = h.snapshot.facts.get(&id) {
                if choice % 3 == 0 || weight == 3 {
                    changes.push(SignedFactDelta {
                        fact,
                        weight: -weight,
                    });
                    changes.push(SignedFactDelta {
                        fact: generated_fact(id, choice / 3),
                        weight: 1,
                    });
                } else {
                    changes.push(SignedFactDelta {
                        fact,
                        weight: if choice % 2 == 0 { 1 } else { -1 },
                    });
                }
            } else {
                changes.push(SignedFactDelta {
                    fact: generated_fact(id, choice),
                    weight: 1,
                });
            }
            h.close(&changes);
        }
        let shifted = SemanticGenerationVector {
            policy_generation: 2,
            ..generations()
        };
        h.engine.advance_semantic_generations(shifted).unwrap();
        h.snapshot.generations = shifted;
        let rebuilt = h.close(&[]);
        assert_eq!(rebuilt.generation_rebuilds, 1);
        assert_eq!(rebuilt.affected_findings, 0);
    }
}

#[test]
fn graph_endpoint_selection_preserves_backward_and_forward_joins() {
    let query = StandingQueryFamily::UngatedSinkReachability;
    let mut h = Harness::new(query, 4, AntichainOverflowMode::FailClosed);
    let first = h.close(&[
        edge(1, 10, 20, 1),
        edge(2, 20, 90, 1),
        edge(3, 10, 30, 1),
        edge(4, 10, 40, 1),
        edge(5, 10, 50, 1),
        edge(6, 100, 110, 1),
        edge(7, 110, 190, 1),
    ]);
    assert_eq!(first.findings.len(), 2);
    // Four outgoing rows against one incoming row selects the backward join.
    let backward = h.close(&[edge(1, 10, 20, 1)]);
    assert_eq!(backward.affected_findings, 1);
    assert_eq!(
        backward.findings[&finding(query, 10, 90)].total_support_weight,
        2
    );
    assert_eq!(
        backward.findings[&finding(query, 100, 190)],
        first.findings[&finding(query, 100, 190)]
    );
    // Equal singleton endpoints select the forward join in the same owner.
    let forward = h.close(&[edge(7, 110, 190, 2)]);
    assert_eq!(forward.affected_findings, 1);
    assert_eq!(
        forward.findings[&finding(query, 100, 190)].total_support_weight,
        3
    );
    let blocked = h.close(&[gate(9, 20, 90, 1)]);
    assert!(!blocked.findings.contains_key(&finding(query, 10, 90)));
    assert_eq!(
        blocked.findings[&finding(query, 100, 190)],
        forward.findings[&finding(query, 100, 190)]
    );
}
