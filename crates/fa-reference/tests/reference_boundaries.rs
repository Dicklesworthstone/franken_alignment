//! Boundary and causal-negative tests for the reference model's public surface.
//!
//! This is an integration target, so it can only reach `fa_reference`'s public
//! API. That restriction is the point: these tests bind the *contract* the
//! production engine will be differentially tested against, not the model's
//! internals. Plan §19.2 requires the reference and the engine to be checked
//! against the written contracts independently; a test that reaches inside the
//! model would not survive that separation.
//!
//! Every test below pairs a meaningful positive observable with a causal
//! negative, and each negative names the mutation it is there to kill. Where the
//! model's present behaviour is conservative rather than exact, the test pins
//! the observed behaviour and says so, so that changing it becomes a reviewed
//! decision instead of a silent drift.
//!
//! **What green here does not prove.** Nothing about concurrency, durability,
//! crash recovery, a real broker, a real host or any cryptographic property.
//! The commitment digest in `round` is an unkeyed FNV-1a comparison value and
//! the tests below treat it as one; they check preimage framing, not collision
//! resistance. Every model under test is single-threaded, in-memory and
//! effect-free, and the unit tests inside `lib.rs` and `round.rs` remain those
//! modules' own coverage; this file adds the boundaries they leave open.
//!
//! One property is deliberately not here: `round`'s domain separator cannot be
//! pinned from an integration target, because every assertion available to this
//! file compares two commitments and the separator is a shared constant prefix.
//! It needs `frame` and `fnv1a64`, which are private, so it lives inside
//! `round.rs` as `the_domain_separator_is_part_of_the_preimage`.

use std::collections::{BTreeMap, BTreeSet};

use fa_reference::product_frontier::{
    FrontierRequirement, FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::round::{MemberOutcome, Round, Verdict, commitment};
use fa_reference::{
    Effect, Error, EvidenceFrontiers, Graph, Judgment, Operation, ReadWitness, Rights, Snapshot,
    State, independent, linear_sign,
};

// ---------------------------------------------------------------------------
// Operation independence (plan §19.7 lawful trace quotient, FA-INV-028)
// ---------------------------------------------------------------------------

/// The existing unit test uses an operation pair in which *both* cross-checks
/// fire, so deleting either one still yields `false` and the suite still passes.
/// These two cases isolate one cross-check each.
#[test]
fn a_write_read_conflict_is_detected_in_both_directions() {
    // Positive: overlapping reads with no writes commute in both orders.
    let mut left = Operation::default();
    left.reads.insert(1);
    left.reads.insert(2);
    let mut right = Operation::default();
    right.reads.insert(1);
    right.reads.insert(3);
    assert!(independent(&left, &right));
    assert!(independent(&right, &left));

    let mut writer = Operation::default();
    writer.writes.insert(9);
    let mut reader = Operation::default();
    reader.reads.insert(9);

    // Causal negative 1: only `a.writes` against `b.reads` is non-empty here.
    // Kills deleting that arm of the disjointness test.
    assert!(!independent(&writer, &reader));

    // Causal negative 2: the mirror. Only `b.writes` against `a.reads` is
    // non-empty here. Kills deleting the other arm.
    assert!(!independent(&reader, &writer));
}

fn enumerate_operations() -> Vec<Operation> {
    let mut operations = Vec::new();
    for mask in 0_u32..64 {
        let mut reads = BTreeSet::new();
        let mut writes = BTreeSet::new();
        let mut consuming_resources = BTreeSet::new();
        if mask & 1 != 0 {
            reads.insert(1_u64);
        }
        if mask & 2 != 0 {
            reads.insert(2_u64);
        }
        if mask & 4 != 0 {
            writes.insert(1_u64);
        }
        if mask & 8 != 0 {
            writes.insert(2_u64);
        }
        if mask & 16 != 0 {
            consuming_resources.insert(1_u64);
        }
        operations.push(Operation {
            reads,
            writes,
            consuming_resources,
            external_or_ordered: mask & 32 != 0,
        });
    }
    operations
}

#[test]
fn independence_is_symmetric_and_refuses_self_reordering_of_effectful_work() {
    let operations = enumerate_operations();

    // Positive, exhaustive over the enumerated space: the relation is symmetric.
    for left in &operations {
        for right in &operations {
            assert_eq!(
                independent(left, right),
                independent(right, left),
                "asymmetric verdict for {left:?} against {right:?}"
            );
        }
    }

    // Causal negative: an operation that writes, consumes a resource or is
    // externally ordered is never independent of itself, so it can never be
    // reordered against its own replica. Kills weakening any single clause to a
    // no-op, which would make some effectful operation self-commuting.
    for operation in &operations {
        let effectful = !operation.writes.is_empty()
            || !operation.consuming_resources.is_empty()
            || operation.external_or_ordered;
        assert_eq!(
            independent(operation, operation),
            !effectful,
            "self-independence wrong for {operation:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Integer probe margins (plan §10, §19.7)
// ---------------------------------------------------------------------------

const PROBE_WEIGHTS: [i64; 2] = [3, -5];
const PROBE_THRESHOLD: i128 = 4;
const PROBE_ERROR: u64 = 2;
const PROBE_ERROR_SIGNED: i64 = 2;

/// Soundness *and* completeness. The in-module exhaustive test checks
/// soundness, so a strictly more conservative bound (for example `2 * bound`)
/// passes it; the completeness half below rejects that.
#[test]
fn probe_certificates_are_sound_and_the_refusal_band_is_no_wider_than_necessary() {
    let bound = i128::from(PROBE_ERROR) * (3 + 5);
    for x in -12_i64..=12 {
        for y in -12_i64..=12 {
            let result =
                linear_sign(&PROBE_WEIGHTS, &[x, y], PROBE_THRESHOLD, PROBE_ERROR).unwrap();

            // Ground truth by enumeration over the admissible perturbation box.
            let mut any_true = false;
            let mut any_false = false;
            for dx in -PROBE_ERROR_SIGNED..=PROBE_ERROR_SIGNED {
                for dy in -PROBE_ERROR_SIGNED..=PROBE_ERROR_SIGNED {
                    let value = 3 * (x + dx) - 5 * (y + dy) - 4;
                    if value > 0 {
                        any_true = true;
                    } else {
                        any_false = true;
                    }
                }
            }

            // Soundness: a certificate is never contradicted by the box.
            match result {
                Some(true) => assert!(!any_false, "false positive certificate at ({x}, {y})"),
                Some(false) => assert!(!any_true, "false negative certificate at ({x}, {y})"),
                None => {}
            }

            // Completeness, positive side: a uniformly positive box must be
            // certified. Kills any inflation of the error bound.
            if !any_false {
                assert_eq!(
                    result,
                    Some(true),
                    "refused a certificate the bound already justifies at ({x}, {y})"
                );
            }

            // Completeness, negative side. The model is tight above but
            // conservative by exactly one point below: `Some(false)` requires
            // `margin < -bound`, while the box is already uniformly non-positive
            // at `margin == -bound`. This pins the observed behaviour at that
            // single point and the exact answer everywhere else, so relaxing
            // `<` to `<=` becomes a reviewed change rather than a silent one.
            if !any_true {
                let margin = 3 * i128::from(x) - 5 * i128::from(y) - PROBE_THRESHOLD;
                if margin == -bound {
                    assert_eq!(result, None, "conservative corner moved at ({x}, {y})");
                } else {
                    assert_eq!(result, Some(false), "missing negative certificate");
                }
            }
        }
    }
}

#[test]
fn probe_boundaries_are_named_explicitly() {
    // margin == +bound: a perturbation reaches exactly zero, which is not
    // strictly positive, so refusing is exact.
    assert_eq!(linear_sign(&[1], &[1], 0, 1), Ok(None));
    // margin == -bound: every perturbation is non-positive, so `Some(false)`
    // would also be sound. The model refuses. Characterisation, not endorsement.
    assert_eq!(linear_sign(&[1], &[-1], 0, 1), Ok(None));
    // One step beyond each boundary the model does certify.
    assert_eq!(linear_sign(&[1], &[2], 0, 1), Ok(Some(true)));
    assert_eq!(linear_sign(&[1], &[-2], 0, 1), Ok(Some(false)));
}

/// The model's own overflow test only reaches the threshold subtraction. These
/// reach the dot-product accumulation and the error-bound multiplication, so a
/// checked operation cannot be relaxed to a wrapping one unnoticed.
#[test]
fn probe_arithmetic_fails_closed_on_every_accumulator() {
    assert_eq!(
        linear_sign(
            &[i64::MAX, i64::MAX, i64::MAX],
            &[i64::MAX, i64::MAX, i64::MAX],
            0,
            0
        ),
        Err(Error::Overflow)
    );
    assert_eq!(
        linear_sign(&[i64::MIN, i64::MIN], &[0, 0], 0, u64::MAX),
        Err(Error::Overflow)
    );
    // Shape errors are refused rather than truncated to the shorter side.
    assert_eq!(linear_sign(&[1, 2], &[1], 0, 1), Err(Error::InvalidInput));
    assert_eq!(linear_sign(&[], &[], 0, 1), Err(Error::InvalidInput));
}

// ---------------------------------------------------------------------------
// Conserved rights (plan §8, §25.1, §25.3; FA-INV-001 through FA-INV-006)
// ---------------------------------------------------------------------------

fn bound_effect() -> Effect {
    Effect {
        principal: "actor".into(),
        resolved_target: "object:7".into(),
        payload: b"exact bytes".to_vec(),
        units: 3,
    }
}

fn reserved_ledger() -> (Rights, Effect) {
    let mut rights = Rights::new(10);
    let effect = bound_effect();
    rights.reserve(1, effect.clone()).unwrap();
    (rights, effect)
}

/// §25.1 makes the action digest and the scope separate conjuncts. The model's
/// in-module test perturbs `payload` only, so a comparison narrowed to a
/// payload digest would still pass it. Each field is perturbed alone here.
#[test]
fn dispatch_binds_every_field_of_the_reserved_effect() {
    // Positive: the exact reserved effect dispatches, and rights stay conserved.
    let (mut rights, effect) = reserved_ledger();
    rights.dispatch(1, &effect).unwrap();
    assert_eq!(rights.state(1), Ok(State::Dispatched));
    assert!(rights.conserved());

    let mut altered_units = effect.clone();
    altered_units.units += 1;
    let mut altered_principal = effect.clone();
    altered_principal.principal.push('!');
    let mut altered_target = effect.clone();
    altered_target.resolved_target.push('!');
    let mut altered_payload = effect.clone();
    altered_payload.payload.push(0);

    // Causal negative: one perturbed field is enough, whichever it is. Kills
    // narrowing the binding to any proper subset of the effect.
    for candidate in [
        altered_units,
        altered_principal,
        altered_target,
        altered_payload,
    ] {
        let (mut rights, _) = reserved_ledger();
        assert_eq!(rights.dispatch(1, &candidate), Err(Error::Binding));
        assert_eq!(
            rights.state(1),
            Ok(State::Reserved),
            "a refused dispatch must leave the reservation exactly where it was"
        );
        assert!(rights.conserved());
    }
}

/// "Reservation resurrection" is a named bounded-model obligation (plan §19.4)
/// that the reference model's prior coverage did not reach.
#[test]
fn a_terminal_reservation_id_can_never_be_reused() {
    // Positive: an aborted reservation returns its units exactly once.
    let (mut rights, effect) = reserved_ledger();
    assert_eq!(rights.available(), 7);
    rights.abort_before_dispatch(1).unwrap();
    assert_eq!(rights.available(), 10);
    assert_eq!(rights.state(1), Ok(State::Aborted));
    assert!(rights.conserved());

    // Causal negative: the identifier is consumed as an identity, not only as
    // units. Kills any implementation that drops terminal entries from the
    // ledger, which is exactly how a resurrected reservation would arise.
    assert_eq!(rights.reserve(1, effect.clone()), Err(Error::Duplicate));
    assert_eq!(
        rights.available(),
        10,
        "a refused re-reservation must not charge anything"
    );
    assert!(rights.conserved());

    // The same holds on the committed side of the state machine.
    let mut committed = Rights::new(10);
    committed.reserve(2, effect.clone()).unwrap();
    committed.dispatch(2, &effect).unwrap();
    committed.reconcile(2, true).unwrap();
    assert_eq!(committed.state(2), Ok(State::Committed));
    assert_eq!(committed.reserve(2, effect), Err(Error::Duplicate));
    assert!(committed.conserved());
}

#[test]
fn the_revocation_floor_only_ever_moves_forward() {
    // Positive: revoking fences an undispatched reservation without touching
    // dispatched history.
    let mut rights = Rights::new(10);
    let effect = bound_effect();
    assert_eq!(rights.epoch(), 0);
    rights.reserve(1, effect.clone()).unwrap();
    rights.revoke_epoch().unwrap();
    assert_eq!(rights.epoch(), 1);
    assert_eq!(rights.dispatch(1, &effect), Err(Error::Stale));
    rights.abort_before_dispatch(1).unwrap();

    // Causal negative: no ledger operation moves the floor at all, in either
    // direction - containment reset included, which is the half of FA-INV-034
    // that says rewinding the actor never rewinds the policy epoch. Every
    // state-changing public method is exercised here, so adding any epoch
    // rollback or "restore floor" path fails this assertion.
    let floor = rights.epoch();
    rights.reserve(2, effect.clone()).unwrap();
    rights.dispatch(2, &effect).unwrap();
    rights.mark_unknown(2).unwrap();
    rights.reconcile(2, false).unwrap();
    rights.reserve(3, effect.clone()).unwrap();
    rights.reset_to_checkpoint(&BTreeSet::new()).unwrap();
    assert_eq!(
        rights.epoch(),
        floor,
        "no ledger operation may move the revocation floor"
    );
    rights.revoke_epoch().unwrap();
    assert!(rights.epoch() > floor);
    assert!(rights.conserved());
}

/// The counter is deliberately out-of-band: it survives the actor's amnesia, so
/// no ordinary ledger traffic may move it and no reset may move anything else.
#[test]
fn the_incident_counter_moves_only_on_reset() {
    let mut rights = Rights::new(10);
    let effect = bound_effect();
    assert_eq!(rights.incident_count(), 0);

    // Causal negative: the whole non-reset surface leaves the counter alone.
    // Kills incrementing it from any other transition.
    rights.reserve(1, effect.clone()).unwrap();
    rights.dispatch(1, &effect).unwrap();
    rights.mark_unknown(1).unwrap();
    rights.reconcile(1, true).unwrap();
    rights.revoke_epoch().unwrap();
    rights.reserve(2, effect.clone()).unwrap();
    rights.abort_before_dispatch(2).unwrap();
    assert_eq!(
        rights.incident_count(),
        0,
        "only containment reset moves the incident counter"
    );

    // Positive: reset moves the counter and nothing else. Nothing is `Reserved`
    // at this point, so a refund here would be a refund of history.
    let floor = rights.epoch();
    let available = rights.available();
    rights.reset_to_checkpoint(&BTreeSet::new()).unwrap();
    assert_eq!(rights.incident_count(), 1);
    assert_eq!(
        rights.epoch(),
        floor,
        "reset never rewinds the policy epoch"
    );
    assert_eq!(
        rights.available(),
        available,
        "reset never refunds dispatched or committed history"
    );
    // `spent` has no public accessor; conservation pins it, because available
    // and the held set are both known to be unchanged.
    assert!(rights.conserved());

    // Causal negative: the counter accumulates across resets rather than being
    // restored with the actor. Kills resetting it inside `reset_to_checkpoint`.
    rights.reset_to_checkpoint(&BTreeSet::new()).unwrap();
    assert_eq!(rights.incident_count(), 2);
    assert!(rights.conserved());
}

// ---------------------------------------------------------------------------
// Frozen commit-reveal transcript (FA-INV-010, plan §9.2)
// ---------------------------------------------------------------------------

/// The module header of `round` states that the framed fields distinguish
/// preimages. Its own binding test perturbs one field at a time, and every one
/// of those pairs differs with or without the length prefixes, so the framing
/// itself needs a colliding split to be pinned.
#[test]
fn commitment_field_framing_prevents_a_boundary_collision() {
    // Positive: the function is deterministic on identical input.
    let split_left = commitment(1, "ab", b"c", Verdict::Allow, b"s").unwrap();
    assert_eq!(
        split_left,
        commitment(1, "ab", b"c", Verdict::Allow, b"s").unwrap()
    );

    // Causal negative: `member` and `evidence_root` are the only two adjacent
    // variable-length fields in the preimage, so moving one byte across their
    // boundary is the collision that framing exists to prevent. Unframed, both
    // sides concatenate to "abc" and these commitments compare equal. Kills
    // dropping the length prefix from the framing helper.
    let split_right = commitment(1, "a", b"bc", Verdict::Allow, b"s").unwrap();
    assert_ne!(split_left, split_right);

    let longer_left = commitment(2, "committee-member", b"root", Verdict::Deny, b"s").unwrap();
    let longer_right = commitment(2, "committee", b"-memberroot", Verdict::Deny, b"s").unwrap();
    assert_ne!(longer_left, longer_right);
}

/// FA-INV-010 stated at the transcript level rather than on the digest
/// function: a stolen commitment is inert outside the round that minted it.
#[test]
fn a_commitment_cannot_be_replayed_into_another_round_or_member() {
    let root: &[u8] = b"evidence-root-a";
    let minted = commitment(7, "alice", root, Verdict::Deny, b"salt-a").unwrap();

    // Positive: in its own round, for its own member, the commitment opens.
    let mut own = Round::new(7, root).unwrap();
    own.add_member("alice").unwrap();
    own.commit("alice", minted).unwrap();
    own.open_reveals().unwrap();
    own.reveal("alice", Verdict::Deny, b"salt-a").unwrap();
    assert_eq!(
        own.outcome("alice"),
        Ok(MemberOutcome::Revealed(Verdict::Deny))
    );

    // Causal negative: a transcript accepts an opaque digest at commit time, so
    // the refusal has to happen at reveal. A different round id cannot open it,
    // and the member stays Missing rather than counting toward the round.
    let mut other_round = Round::new(8, root).unwrap();
    other_round.add_member("alice").unwrap();
    other_round.commit("alice", minted).unwrap();
    other_round.open_reveals().unwrap();
    assert_eq!(
        other_round.reveal("alice", Verdict::Deny, b"salt-a"),
        Err(Error::Binding)
    );
    assert_eq!(other_round.outcome("alice"), Ok(MemberOutcome::Missing));

    // Causal negative: nor can another member open it inside the right round.
    let mut other_member = Round::new(7, root).unwrap();
    other_member.add_member("bob").unwrap();
    other_member.commit("bob", minted).unwrap();
    other_member.open_reveals().unwrap();
    assert_eq!(
        other_member.reveal("bob", Verdict::Deny, b"salt-a"),
        Err(Error::Binding)
    );
    assert_eq!(other_member.outcome("bob"), Ok(MemberOutcome::Missing));

    // Causal negative: nor can the same member under a different evidence root,
    // which is what stops a round being re-run against substituted evidence.
    let mut other_root = Round::new(7, b"evidence-root-b").unwrap();
    other_root.add_member("alice").unwrap();
    other_root.commit("alice", minted).unwrap();
    other_root.open_reveals().unwrap();
    assert_eq!(
        other_root.reveal("alice", Verdict::Deny, b"salt-a"),
        Err(Error::Binding)
    );
    assert_eq!(other_root.outcome("alice"), Ok(MemberOutcome::Missing));
}

/// Plan §25.3, stated as arithmetic rather than prose.
#[test]
fn an_unknown_outcome_stays_charged_until_it_is_resolved() {
    let mut rights = Rights::new(10);
    let mut seven = bound_effect();
    seven.units = 7;

    // Positive: the reservation charges the budget on the way out.
    rights.reserve(1, seven.clone()).unwrap();
    rights.dispatch(1, &seven).unwrap();
    rights.mark_unknown(1).unwrap();
    assert_eq!(rights.available(), 3);
    assert!(rights.conserved());

    // Causal negative: a second seven-unit action cannot be admitted while the
    // first outcome is unknown, and cancellation cannot refund it. Kills any
    // refund-on-timeout path, which would expose fourteen units against a
    // ten-unit budget.
    let mut second = seven.clone();
    second.payload = b"second".to_vec();
    assert_eq!(rights.reserve(2, second), Err(Error::Limit));
    assert_eq!(rights.abort_before_dispatch(1), Err(Error::WrongState));
    assert_eq!(rights.available(), 3);
    assert!(rights.conserved());

    // Only a trusted outcome resolves it, and only once.
    rights.reconcile(1, false).unwrap();
    assert_eq!(rights.available(), 10);
    assert_eq!(rights.reconcile(1, false), Err(Error::WrongState));
    assert!(rights.conserved());
}

// ---------------------------------------------------------------------------
// Evidence frontiers (FA-INV-022) and read witnesses (FA-INV-021, FA-INV-023)
// ---------------------------------------------------------------------------

/// `EvidenceFrontiers::covers_closed_scope` is the legacy prefix-coverage
/// predicate: it answers whether sequences 1..=n were judged with no hole. It
/// does **not** establish an authenticated terminal endpoint, so a true answer
/// is not evidence that nothing beyond n exists - plan §25.4 is explicit that a
/// verifier with no independent knowledge of the true head cannot detect a
/// removed suffix. Terminal closure needs the explicit closing marker that the
/// product frontier work adds; nothing here claims absence.
#[test]
fn prefix_coverage_requires_contiguity_not_membership() {
    let mut frontiers = EvidenceFrontiers::new(8).unwrap();

    // Positive: a contiguous run carried through all three stages is covered as
    // a prefix.
    for sequence in 1..=3 {
        frontiers.capture(sequence).unwrap();
        frontiers.verify(sequence).unwrap();
        frontiers.judge(sequence).unwrap();
    }
    assert!(frontiers.covers_closed_scope(3));

    // Causal negative: judging a later item across a gap must not extend the
    // covered prefix. Kills replacing the contiguity test with a membership
    // test, which would report a prefix as covered while a member of it was
    // never judged.
    frontiers.capture(5).unwrap();
    frontiers.verify(5).unwrap();
    frontiers.judge(5).unwrap();
    assert!(!frontiers.covers_closed_scope(4));
    assert!(!frontiers.covers_closed_scope(5));
    assert!(frontiers.covers_closed_scope(3));

    // Filling the hole is what extends the prefix, and only to 5 - the
    // predicate says nothing about whether 6 exists.
    frontiers.capture(4).unwrap();
    frontiers.verify(4).unwrap();
    frontiers.judge(4).unwrap();
    assert!(frontiers.covers_closed_scope(5));
}

#[test]
fn a_stage_cannot_be_skipped_and_the_window_fails_closed() {
    let mut frontiers = EvidenceFrontiers::new(4).unwrap();

    // Causal negative: judgment cannot outrun verification, and verification
    // cannot outrun capture. Kills collapsing the three frontiers into one.
    assert_eq!(frontiers.verify(1), Err(Error::Incomplete));
    assert_eq!(frontiers.judge(1), Err(Error::Incomplete));
    frontiers.capture(1).unwrap();
    assert_eq!(frontiers.judge(1), Err(Error::Incomplete));

    // Positive: the ordered path succeeds and covers the prefix 1..=1. That
    // is prefix coverage only; it asserts nothing about a terminal endpoint.
    frontiers.verify(1).unwrap();
    frontiers.judge(1).unwrap();
    assert!(frontiers.covers_closed_scope(1));

    // Causal negative: an item beyond the admission window is refused and the
    // frontier is left exactly as it was.
    assert_eq!(frontiers.capture(9), Err(Error::Limit));
    assert_eq!(frontiers.verify(9), Err(Error::Incomplete));
    assert!(frontiers.covers_closed_scope(1));
}

fn witness_snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::from([(1_u64, b"artifact".to_vec())]),
    }
}

#[test]
fn a_negative_domain_must_be_a_nonempty_half_open_range() {
    let base = witness_snapshot();
    let mut captured = base.clone();
    let judgment = Judgment::capture(
        &captured,
        vec![ReadWitness::EmptyRange { start: 10, end: 20 }],
    )
    .unwrap();

    // Positive: a write outside the declared domain does not disturb the claim.
    captured.values.insert(30, vec![]);
    assert_eq!(judgment.valid_at(&captured), Ok(true));

    // Causal negative: the end of the range is exclusive, the start inclusive.
    // Kills silently widening or narrowing the declared domain by one.
    let mut at_end = base.clone();
    at_end.values.insert(20, vec![]);
    assert_eq!(judgment.valid_at(&at_end), Ok(true));
    let mut inside = base.clone();
    inside.values.insert(19, vec![]);
    assert_eq!(judgment.valid_at(&inside), Ok(false));
    let mut at_start = base.clone();
    at_start.values.insert(10, vec![]);
    assert_eq!(judgment.valid_at(&at_start), Ok(false));

    // Causal negative: a degenerate or inverted domain is not a claim and is
    // refused at capture, so an unsound witness is unconstructible.
    assert_eq!(
        Judgment::capture(&base, vec![ReadWitness::EmptyRange { start: 5, end: 5 }]),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        Judgment::capture(&base, vec![ReadWitness::EmptyRange { start: 6, end: 5 }]),
        Err(Error::InvalidInput)
    );

    // Causal negative: missing completeness is an error, never a bare `false`.
    // Kills degrading an unknown domain into a negative answer.
    let mut incomplete = base;
    incomplete.complete = false;
    assert_eq!(judgment.valid_at(&incomplete), Err(Error::Incomplete));
}

// ---------------------------------------------------------------------------
// Scoped authority cuts (FA-INV-024, plan §19.7)
// ---------------------------------------------------------------------------

/// "A bypass edge added after a cut certificate" is on the plan's required
/// negative list and was not reached by the reference model's prior coverage.
#[test]
fn a_bypass_edge_invalidates_an_earlier_cut_certificate() {
    let cut = BTreeSet::from([1_usize]);

    // Positive: on the declared graph the cut disconnects the sink, and the
    // dominator view agrees with the cut view.
    let declared = Graph::new(4, &[(0, 1), (1, 3)]).unwrap();
    assert_eq!(declared.cut_disconnects(&[0], &[3], &cut), Ok(true));
    assert_eq!(declared.dominates(0, 1, 3), Ok(Some(true)));

    // Causal negative: one added bypass edge and the same cut no longer
    // disconnects anything. A cut is a claim about a complete declared graph
    // and never transfers to a different one. Kills caching or reusing a cut
    // certificate across graph revisions.
    let bypassed = Graph::new(4, &[(0, 1), (1, 3), (0, 2), (2, 3)]).unwrap();
    assert_eq!(bypassed.cut_disconnects(&[0], &[3], &cut), Ok(false));
    assert_eq!(bypassed.dominates(0, 1, 3), Ok(Some(false)));

    // Causal negative: an empty sink set is not a disconnection claim, and an
    // out-of-range node is refused rather than quietly ignored.
    assert_eq!(
        declared.cut_disconnects(&[0], &[], &cut),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        declared.cut_disconnects(&[0], &[9], &cut),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        declared.reachable(&[], &BTreeSet::new()),
        Err(Error::InvalidInput)
    );
}

// ---------------------------------------------------------------------------
// Empty closed product-frontier domains (plan §7.8, FA-INV-022)
// ---------------------------------------------------------------------------

fn review_regression_empty_projection_key(projection: u64) -> ProjectionKey {
    ProjectionKey {
        source: 7,
        branch: 11,
        projection,
        source_epoch: 1,
    }
}

fn review_regression_empty_close(key: ProjectionKey, generation: u64) -> TrustedClosingMarker {
    TrustedClosingMarker {
        key,
        final_sequence: 0,
        marker_generation: generation,
    }
}

fn review_regression_empty_closed_requirement(
    key: ProjectionKey,
    generation: u64,
) -> FrontierRequirement {
    FrontierRequirement {
        key,
        stage: FrontierStage::Judged,
        through: 0,
        closure: Some(generation),
    }
}

/// A closed empty scope is a valid negative-evidence case only after a trusted
/// terminal observation. It must not require a fabricated positive sequence.
#[test]
fn review_regression_empty_terminal_marker_establishes_only_its_exact_closed_scope() {
    let first = review_regression_empty_projection_key(1);
    let other = review_regression_empty_projection_key(2);
    let mut frontiers = ProductFrontiers::new(2, 8).unwrap();

    // Causal negative: no received data alone is not a closed negative claim.
    assert_eq!(
        frontiers.satisfies(review_regression_empty_closed_requirement(first, 9)),
        Ok(false)
    );

    // Positive: an explicit trusted observation of terminal sequence zero
    // establishes a closed empty domain without inventing sequence one.
    assert_eq!(
        frontiers.record_close(review_regression_empty_close(first, 9)),
        Ok(())
    );
    assert_eq!(
        frontiers.satisfies(review_regression_empty_closed_requirement(first, 9)),
        Ok(true)
    );

    // Causal negatives: a marker generation and projection identity are both
    // part of the closed-scope obligation, so neither can be borrowed.
    assert_eq!(
        frontiers.satisfies(review_regression_empty_closed_requirement(first, 10)),
        Ok(false)
    );
    assert_eq!(
        frontiers.satisfies(review_regression_empty_closed_requirement(other, 9)),
        Ok(false)
    );

    // A terminal-zero close rejects every later source position while retaining
    // the zero frontier rather than admitting a post-close extension.
    assert_eq!(
        frontiers.accept(first, FrontierStage::Captured, 1),
        Err(Error::WrongState)
    );
    assert_eq!(frontiers.frontier(first, FrontierStage::Captured), Ok(0));

    // A zero positive-only requirement remains invalid: zero is meaningful
    // only when paired with an explicit closing marker.
    assert_eq!(
        frontiers.satisfies(FrontierRequirement {
            key: first,
            stage: FrontierStage::Judged,
            through: 0,
            closure: None,
        }),
        Err(Error::InvalidInput)
    );
}

/// An empty marker consumes one configured projection slot. A refused second
/// marker must leave the first closed scope exactly intact.
#[test]
fn review_regression_empty_terminal_marker_respects_capacity_without_mutation() {
    let first = review_regression_empty_projection_key(1);
    let second = review_regression_empty_projection_key(2);
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();

    frontiers
        .record_close(review_regression_empty_close(first, 9))
        .unwrap();
    let before_second_marker = frontiers.clone();
    assert_eq!(
        frontiers.record_close(review_regression_empty_close(second, 9)),
        Err(Error::Limit)
    );
    assert_eq!(frontiers, before_second_marker);
    assert_eq!(
        frontiers.satisfies(review_regression_empty_closed_requirement(first, 9)),
        Ok(true)
    );
    assert_eq!(
        frontiers.satisfies(review_regression_empty_closed_requirement(second, 9)),
        Ok(false)
    );
}
