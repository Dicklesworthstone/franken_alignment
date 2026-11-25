//! Public-boundary regressions for canonical, atomic standing-oversight updates.
//! These are bounded reference cases, not evidence of a live authenticated feed.

use std::collections::{BTreeMap, BTreeSet};

use fa_reference::observation_algebra::{
    Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
};
use fa_reference::product_frontier::{
    FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey,
};
use fa_reference::standing_oversight::{
    AntichainOverflowMode, BaseFact, BaseFactKind, FindingKey, IncrementalOversightEngine,
    MAX_SNAPSHOT_FACTS, MAX_SUPPORT_ATOMS_PER_PATH, OversightSnapshot, SemanticGenerationVector,
    SignedFactDelta, StandingOversightError, StandingQueryFamily, SupportAntichain, SupportPath,
    ViewReceipt, batch_oracle,
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
        branch: 2,
        projection: 3,
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

fn engine(
    query: StandingQueryFamily,
    width: usize,
    overflow: AntichainOverflowMode,
) -> IncrementalOversightEngine {
    IncrementalOversightEngine::new(basis(), query, generations(), projection(), width, overflow)
        .unwrap()
}

fn snapshot() -> OversightSnapshot {
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers
        .accept(projection(), FrontierStage::Captured, 1)
        .unwrap();
    frontiers
        .accept(projection(), FrontierStage::Authenticated, 1)
        .unwrap();
    OversightSnapshot::new(
        generations(),
        true,
        frontiers,
        FrontierRequirement {
            key: projection(),
            stage: FrontierStage::Authenticated,
            through: 1,
            closure: None,
        },
    )
}

fn delta(fact_id: u64, kind: BaseFactKind, weight: i64) -> SignedFactDelta {
    SignedFactDelta {
        fact: BaseFact { fact_id, kind },
        weight,
    }
}

fn cohort(fact_id: u64, cohort: u64, weight: i64) -> SignedFactDelta {
    delta(
        fact_id,
        BaseFactKind::CohortFailure {
            cohort,
            failure_id: 20,
        },
        weight,
    )
}

fn known(result: Result<Knowledge<ViewReceipt>, StandingOversightError>) -> ViewReceipt {
    match result.unwrap() {
        Knowledge::Known { value, .. } => value,
        other => panic!("expected known reference receipt, received {other:?}"),
    }
}

fn path(atoms: &[u64]) -> SupportPath {
    SupportPath::new(atoms.iter().copied().collect(), BTreeSet::new()).unwrap()
}

#[test]
fn rejected_width_expansion_cannot_leave_hidden_support_after_retraction() {
    for mode in [
        AntichainOverflowMode::FailClosed,
        AntichainOverflowMode::SpillToBatchRecompute,
    ] {
        let mut supports = SupportAntichain::new(1).unwrap();
        supports.apply_delta(path(&[1]), 1, mode).unwrap();
        // A dominated support is valid even when the minimal antichain is full.
        supports.apply_delta(path(&[1, 3]), 1, mode).unwrap();
        let before = supports.clone();
        assert_eq!(
            supports.apply_delta(path(&[2]), 1, mode),
            Err(StandingOversightError::AntichainWidthExceeded {
                width: 2,
                max_width: 1,
            })
        );
        assert_eq!(supports, before);
        assert_eq!(supports.total_weight(), Ok(2));

        supports.apply_delta(path(&[1]), -1, mode).unwrap();
        assert_eq!(supports.paths(), &[path(&[1, 3])]);
        supports.apply_delta(path(&[1, 3]), -1, mode).unwrap();
        assert!(!supports.is_supported());
        assert_eq!(supports.total_weight(), Ok(0));
        supports.apply_delta(path(&[2]), 1, mode).unwrap();
        assert_eq!(supports.total_weight(), Ok(1));
    }
}

#[test]
fn rejected_retraction_keeps_the_dominating_support_until_its_successors_fit() {
    let mode = AntichainOverflowMode::FailClosed;
    let mut supports = SupportAntichain::new(1).unwrap();
    supports.apply_delta(path(&[1]), 1, mode).unwrap();
    supports.apply_delta(path(&[1, 2]), 1, mode).unwrap();
    supports.apply_delta(path(&[1, 3]), 1, mode).unwrap();
    let before = supports.clone();
    assert_eq!(
        supports.apply_delta(path(&[1]), -1, mode),
        Err(StandingOversightError::AntichainWidthExceeded {
            width: 2,
            max_width: 1,
        })
    );
    assert_eq!(supports, before);
    supports.apply_delta(path(&[1, 2]), -1, mode).unwrap();
    supports.apply_delta(path(&[1]), -1, mode).unwrap();
    assert_eq!(supports.paths(), &[path(&[1, 3])]);
    assert_eq!(supports.total_weight(), Ok(1));
}

#[test]
fn public_support_fields_cannot_bypass_nonempty_and_size_contracts() {
    let mut supports = SupportAntichain::new(2).unwrap();
    supports
        .apply_delta(path(&[1]), 1, AntichainOverflowMode::FailClosed)
        .unwrap();
    let before = supports.clone();
    let too_many: BTreeSet<u64> = (0..=MAX_SUPPORT_ATOMS_PER_PATH as u64).collect();
    for invalid in [
        SupportPath {
            positive_atoms: BTreeSet::new(),
            negative_guards: BTreeSet::from([1]),
        },
        SupportPath {
            positive_atoms: too_many.clone(),
            negative_guards: BTreeSet::new(),
        },
        SupportPath {
            positive_atoms: BTreeSet::from([2]),
            negative_guards: too_many,
        },
    ] {
        assert_eq!(
            supports.apply_delta(invalid, 1, AntichainOverflowMode::ConservativeCoarsen),
            Err(StandingOversightError::InvalidSupportPath)
        );
        assert_eq!(supports, before);
    }
    supports
        .apply_delta(path(&[2]), 1, AntichainOverflowMode::FailClosed)
        .unwrap();
    assert_eq!(supports.paths(), &[path(&[1]), path(&[2])]);
}

#[test]
fn exact_old_tuple_must_be_fully_retracted_before_its_id_is_reused() {
    let mut state = snapshot();
    let original = cohort(1, 10, 2);
    state.apply_fact_delta(original).unwrap();
    let before = state.clone();
    for weight in [-2, -1, 1] {
        assert_eq!(
            state.apply_fact_delta(cohort(1, 99, weight)),
            Err(StandingOversightError::FactIdentityMismatch)
        );
        assert_eq!(state, before);
    }
    state.apply_fact_delta(cohort(1, 10, -1)).unwrap();
    assert_eq!(
        state.apply_fact_delta(cohort(1, 99, 1)),
        Err(StandingOversightError::FactIdentityMismatch)
    );
    state.apply_fact_delta(cohort(1, 10, -1)).unwrap();
    state.apply_fact_delta(cohort(1, 99, 1)).unwrap();
    let result = batch_oracle::evaluate_batch_query(
        &state,
        StandingQueryFamily::SharedHelperFailureCohort,
        2,
    )
    .unwrap();
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings.values().next().unwrap().key.subject, 99);
    assert_eq!(
        result
            .findings
            .values()
            .next()
            .unwrap()
            .total_support_weight,
        1
    );
}

#[test]
fn closed_frontiers_refuse_replays_gaps_and_unclosed_batches_then_accept_the_next_batch() {
    let mut state = engine(
        StandingQueryFamily::SharedHelperFailureCohort,
        2,
        AntichainOverflowMode::FailClosed,
    );
    assert_eq!(
        state.apply_delta_and_close_frontier(generations(), &[cohort(1, 10, 1)], 0, true, true),
        Err(StandingOversightError::IncompleteProductFrontier)
    );
    let initial = known(state.apply_delta_and_close_frontier(
        generations(),
        &[cohort(1, 10, 1)],
        1,
        true,
        true,
    ));
    assert_eq!((initial.from_frontier_seq, initial.to_frontier_seq), (0, 1));
    for sequence in [1, 0, 3, u64::MAX] {
        assert_eq!(
            state.apply_delta_and_close_frontier(
                generations(),
                &[cohort(1, 10, -1)],
                sequence,
                true,
                true,
            ),
            Err(StandingOversightError::IncompleteProductFrontier)
        );
    }
    assert_eq!(
        state.apply_delta_and_close_frontier(generations(), &[], 1, true, true),
        Err(StandingOversightError::IncompleteProductFrontier)
    );
    assert_eq!(
        state.apply_delta_and_close_frontier(generations(), &[cohort(1, 10, -1)], 2, true, false),
        Err(StandingOversightError::IncompleteProductFrontier)
    );
    let replaced = known(state.apply_delta_and_close_frontier(
        generations(),
        &[cohort(1, 10, -1), cohort(1, 30, 1)],
        2,
        true,
        true,
    ));
    assert_eq!(
        (replaced.from_frontier_seq, replaced.to_frontier_seq),
        (1, 2)
    );
    assert_eq!(replaced.findings.len(), 1);
    let finding = replaced.findings.values().next().unwrap();
    assert_eq!(finding.key.subject, 30);
    assert_eq!(finding.total_support_weight, 1);
}

#[test]
fn all_query_families_preserve_exact_fact_identity_and_resume_after_rejection() {
    let cases = [
        (
            StandingQueryFamily::UngatedSinkReachability,
            vec![
                delta(1, BaseFactKind::AuthorityEdge { from: 10, to: 30 }, 1),
                delta(2, BaseFactKind::AuthorityEdge { from: 30, to: 20 }, 1),
            ],
        ),
        (
            StandingQueryFamily::UnreviewedArtifactDependency,
            vec![delta(
                1,
                BaseFactKind::ArtifactDependency {
                    artifact: 10,
                    dependency: 20,
                },
                1,
            )],
        ),
        (
            StandingQueryFamily::InvalidatedHeldProposal,
            vec![
                delta(
                    1,
                    BaseFactKind::HeldProposalDependency {
                        proposal: 10,
                        key: 20,
                    },
                    1,
                ),
                delta(
                    2,
                    BaseFactKind::InvalidatedKey {
                        key: 20,
                        revision: 2,
                    },
                    1,
                ),
            ],
        ),
        (
            StandingQueryFamily::SharedHelperFailureCohort,
            vec![cohort(1, 10, 1)],
        ),
    ];
    for (query, inserts) in cases {
        let mut state = engine(query, 2, AntichainOverflowMode::FailClosed);
        let first =
            known(state.apply_delta_and_close_frontier(generations(), &inserts, 1, true, true));
        assert_eq!(first.findings.len(), 1);
        assert_eq!(
            state.apply_delta_and_close_frontier(
                generations(),
                &[cohort(1, 99, -1)],
                2,
                true,
                true
            ),
            Err(StandingOversightError::FactIdentityMismatch)
        );
        let retract = SignedFactDelta {
            weight: -1,
            ..inserts[0]
        };
        let removed =
            known(state.apply_delta_and_close_frontier(generations(), &[retract], 2, true, true));
        assert!(removed.findings.is_empty());
        let restored = known(state.apply_delta_and_close_frontier(
            generations(),
            &inserts[..1],
            3,
            true,
            true,
        ));
        assert_eq!(restored.findings, first.findings);
    }
}

#[test]
fn invalid_later_delta_rolls_back_every_prior_change_in_the_batch() {
    let mut state = engine(
        StandingQueryFamily::SharedHelperFailureCohort,
        2,
        AntichainOverflowMode::FailClosed,
    );
    known(state.apply_delta_and_close_frontier(generations(), &[cohort(1, 10, 1)], 1, true, true));
    assert_eq!(
        state.apply_delta_and_close_frontier(
            generations(),
            &[cohort(1, 10, -1), cohort(2, 30, 1), cohort(99, 99, -1)],
            2,
            true,
            true,
        ),
        Err(StandingOversightError::LostRetractionOrNegativeWeight)
    );
    let removed = known(state.apply_delta_and_close_frontier(
        generations(),
        &[cohort(1, 10, -1)],
        2,
        true,
        true,
    ));
    assert!(removed.findings.is_empty());
    let added = known(state.apply_delta_and_close_frontier(
        generations(),
        &[cohort(2, 30, 1)],
        3,
        true,
        true,
    ));
    assert_eq!(
        added.findings.values().next().unwrap().total_support_weight,
        1
    );
}

#[test]
fn checked_join_overflow_preserves_the_next_frontier_for_a_valid_weight() {
    let mut state = engine(
        StandingQueryFamily::UngatedSinkReachability,
        1,
        AntichainOverflowMode::FailClosed,
    );
    known(state.apply_delta_and_close_frontier(
        generations(),
        &[delta(
            1,
            BaseFactKind::AuthorityEdge { from: 10, to: 20 },
            i64::MAX,
        )],
        1,
        true,
        true,
    ));
    let second = BaseFactKind::AuthorityEdge { from: 20, to: 90 };
    assert_eq!(
        state.apply_delta_and_close_frontier(generations(), &[delta(2, second, 2)], 2, true, true),
        Err(StandingOversightError::WeightOverflow)
    );
    let accepted = known(state.apply_delta_and_close_frontier(
        generations(),
        &[delta(2, second, 1)],
        2,
        true,
        true,
    ));
    assert_eq!(
        (accepted.from_frontier_seq, accepted.to_frontier_seq),
        (1, 2)
    );
    assert_eq!(
        accepted
            .findings
            .values()
            .next()
            .unwrap()
            .total_support_weight,
        i64::MAX
    );
}

#[test]
fn late_batch_oracle_refusal_cannot_install_rejected_edges_or_advance_the_frontier() {
    let mut state = engine(
        StandingQueryFamily::UngatedSinkReachability,
        1,
        AntichainOverflowMode::ConservativeCoarsen,
    );
    let original = [
        delta(1, BaseFactKind::AuthorityEdge { from: 10, to: 20 }, 1),
        delta(2, BaseFactKind::AuthorityEdge { from: 20, to: 90 }, 1),
    ];
    let first =
        known(state.apply_delta_and_close_frontier(generations(), &original, 1, true, true));
    let mut fanout = Vec::new();
    // Forty-six facts create 23 * 23 independent supports. The bounded oracle
    // refuses after the incremental engine has successfully coarsened them.
    for index in 0..23 {
        fanout.push(delta(
            100 + index,
            BaseFactKind::AuthorityEdge { from: 10, to: 30 },
            1,
        ));
        fanout.push(delta(
            200 + index,
            BaseFactKind::AuthorityEdge { from: 30, to: 90 },
            1,
        ));
    }
    assert_eq!(
        state.apply_delta_and_close_frontier(generations(), &fanout, 2, true, true),
        Err(StandingOversightError::AntichainWidthExceeded {
            width: MAX_SNAPSHOT_FACTS + 1,
            max_width: MAX_SNAPSHOT_FACTS,
        })
    );
    let gate = delta(50, BaseFactKind::QualifiedGate { from: 20, to: 90 }, 1);
    let gated = known(state.apply_delta_and_close_frontier(generations(), &[gate], 2, true, true));
    assert_eq!((gated.from_frontier_seq, gated.to_frontier_seq), (1, 2));
    assert!(gated.findings.is_empty());
    let reopened = known(state.apply_delta_and_close_frontier(
        generations(),
        &[SignedFactDelta { weight: -1, ..gate }],
        3,
        true,
        true,
    ));
    assert_eq!(reopened.findings, first.findings);
    assert_eq!(reopened.coarsened_findings, 0);
}

#[test]
fn public_snapshot_maps_are_validated_before_delta_admission_and_oracle_queries() {
    let valid = cohort(1, 10, 1);
    let mut state = snapshot();
    state.apply_fact_delta(valid).unwrap();
    assert_eq!(
        batch_oracle::evaluate_batch_query(
            &state,
            StandingQueryFamily::SharedHelperFailureCohort,
            1
        )
        .unwrap()
        .findings
        .len(),
        1,
    );
    for (facts, error) in [
        (
            BTreeMap::from([(99, (valid.fact, 1))]),
            StandingOversightError::FactIdentityMismatch,
        ),
        (
            BTreeMap::from([(1, (valid.fact, 0))]),
            StandingOversightError::NonPositiveSnapshotWeight,
        ),
        (
            BTreeMap::from([(1, (valid.fact, -1))]),
            StandingOversightError::NonPositiveSnapshotWeight,
        ),
    ] {
        let mut invalid = snapshot();
        invalid.facts = facts;
        let before = invalid.clone();
        assert_eq!(invalid.apply_fact_delta(cohort(2, 30, 1)), Err(error));
        assert_eq!(invalid, before);
        assert_eq!(
            batch_oracle::evaluate_batch_query(
                &invalid,
                StandingQueryFamily::SharedHelperFailureCohort,
                1
            ),
            Err(error)
        );
    }
    let mut oversized = snapshot();
    for id in 0..=MAX_SNAPSHOT_FACTS as u64 {
        let fact = cohort(id, id, 1).fact;
        oversized.facts.insert(id, (fact, 1));
    }
    let before = oversized.clone();
    assert_eq!(
        oversized.apply_fact_delta(cohort(0, 0, -1)),
        Err(StandingOversightError::SnapshotLimitExceeded)
    );
    assert_eq!(oversized, before);
    assert_eq!(
        batch_oracle::evaluate_batch_query(
            &oversized,
            StandingQueryFamily::SharedHelperFailureCohort,
            1
        ),
        Err(StandingOversightError::SnapshotLimitExceeded)
    );
    let empty = snapshot();
    for invalid_width in [0, MAX_SNAPSHOT_FACTS + 1] {
        assert_eq!(
            batch_oracle::evaluate_batch_query(
                &empty,
                StandingQueryFamily::SharedHelperFailureCohort,
                invalid_width
            ),
            Err(StandingOversightError::InvalidAntichainLimit)
        );
    }
    assert!(
        batch_oracle::evaluate_batch_query(
            &empty,
            StandingQueryFamily::SharedHelperFailureCohort,
            1
        )
        .unwrap()
        .findings
        .is_empty()
    );
}

#[test]
fn generation_rebuilds_preserve_independent_support_and_cannot_accept_old_deltas() {
    let query = StandingQueryFamily::SharedHelperFailureCohort;
    let mut state = engine(query, 2, AntichainOverflowMode::FailClosed);
    known(state.apply_delta_and_close_frontier(
        generations(),
        &[cohort(1, 10, 1), cohort(2, 10, 1)],
        1,
        true,
        true,
    ));
    let next = SemanticGenerationVector {
        policy_generation: 2,
        ..generations()
    };
    state.advance_semantic_generations(next).unwrap();
    state.advance_semantic_generations(next).unwrap();
    assert_eq!(state.generation_rebuilds(), 1);
    let stale = StandingOversightError::MixedSemanticGeneration {
        expected: next,
        observed: generations(),
    };
    assert_eq!(
        state.advance_semantic_generations(generations()),
        Err(stale)
    );
    assert_eq!(
        state.apply_delta_and_close_frontier(generations(), &[cohort(1, 10, -1)], 2, true, true),
        Err(stale)
    );
    let remaining =
        known(state.apply_delta_and_close_frontier(next, &[cohort(1, 10, -1)], 2, true, true));
    assert_eq!(remaining.generations, next);
    assert_eq!(remaining.generation_rebuilds, 1);
    let key = FindingKey {
        query,
        subject: 10,
        target: 20,
    };
    assert_eq!(remaining.findings[&key].total_support_weight, 1);
    assert_eq!(remaining.findings[&key].minimal_supports, vec![path(&[2])]);
}
