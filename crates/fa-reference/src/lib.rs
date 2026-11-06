//! Independent, deliberately small logical models. No external effects,
//! persistence, cryptographic claims, calibration, or deployment guarantees.
#![forbid(unsafe_code)]
#![feature(peer_credentials_unix_socket)]
#![allow(
    dead_code,
    unused_imports,
    clippy::chunks_exact_to_as_chunks,
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::derivable_impls,
    clippy::duplicate_mod,
    clippy::duplicated_attributes,
    clippy::field_reassign_with_default,
    clippy::large_enum_variant,
    clippy::manual_is_multiple_of,
    clippy::map_clone,
    clippy::op_ref,
    clippy::redundant_closure,
    clippy::result_large_err,
    clippy::single_match,
    clippy::single_range_in_vec_init,
    clippy::too_many_arguments,
    clippy::write_with_newline
)]

pub mod action;
pub mod address;
pub mod address_resolver;
pub mod adversarial_vertical_slice;
pub mod canonical_json;
pub mod elicitation_signatures_manifold;
pub mod event_journal;
pub mod evidence_view;
pub mod explain_affordances;
pub mod full_input;
pub mod governance_assurance;
pub mod history;
pub mod knowledge_surface;
pub mod observation_algebra;
pub mod perimeter;
pub mod perimeter_inventory;
pub mod product_frontier;
pub mod reducer;
pub mod round;
pub mod situation_rehearsal;
pub mod standing_oversight;
pub mod strict_json;
pub mod trace_independence;
pub mod tower;
pub mod typestate_cx;
pub mod witness;

include!("logical_core.rs");

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(1, b"artifact".to_vec())]),
        }
    }

    fn effect() -> Effect {
        Effect {
            principal: "actor".into(),
            resolved_target: "object:7".into(),
            payload: b"exact bytes".to_vec(),
            units: 3,
        }
    }

    #[test]
    fn witness_reuses_unrelated_changes() {
        let mut s = snapshot();
        let j = Judgment::capture(
            &s,
            vec![ReadWitness::Exact {
                key: 1,
                value: Some(b"artifact".to_vec()),
            }],
        )
        .unwrap();
        s.values.insert(99, vec![0]);
        assert_eq!(j.valid_at(&s), Ok(true));
        s.values.insert(1, vec![1]);
        assert_eq!(j.valid_at(&s), Ok(false));
    }

    #[test]
    fn absent_key_detects_insertion() {
        let mut s = snapshot();
        let j = Judgment::capture(
            &s,
            vec![ReadWitness::Exact {
                key: 7,
                value: None,
            }],
        )
        .unwrap();
        s.values.insert(7, vec![]);
        assert_eq!(j.valid_at(&s), Ok(false));
    }

    #[test]
    fn empty_range_detects_phantom() {
        let mut s = snapshot();
        let j =
            Judgment::capture(&s, vec![ReadWitness::EmptyRange { start: 10, end: 20 }]).unwrap();
        s.values.insert(20, vec![]);
        assert_eq!(j.valid_at(&s), Ok(true));
        s.values.insert(19, vec![]);
        assert_eq!(j.valid_at(&s), Ok(false));
    }

    #[test]
    fn epoch_and_incompleteness_invalidate() {
        let mut s = snapshot();
        let j = Judgment::capture(&s, vec![]).unwrap();
        s.semantic_epoch += 1;
        assert_eq!(j.valid_at(&s), Ok(false));
        s.complete = false;
        assert_eq!(j.valid_at(&s), Err(Error::Incomplete));
    }

    #[test]
    fn invalid_witness_is_not_admitted() {
        let s = snapshot();
        assert_eq!(
            Judgment::capture(&s, vec![ReadWitness::EmptyRange { start: 2, end: 2 }]),
            Err(Error::InvalidInput)
        );
        assert_eq!(
            Judgment::capture(
                &s,
                vec![ReadWitness::Exact {
                    key: 1,
                    value: None
                }]
            ),
            Err(Error::Binding)
        );
    }

    #[test]
    fn frontier_does_not_skip_a_gap() {
        let mut f = Frontier::new(8).unwrap();
        f.accept(3).unwrap();
        f.accept(1).unwrap();
        assert_eq!(f.contiguous(), 1);
        f.accept(2).unwrap();
        assert_eq!(f.contiguous(), 3);
        f.accept(2).unwrap();
        assert_eq!(f.contiguous(), 3);
    }

    #[test]
    fn frontier_limits_fail_without_mutation() {
        let mut f = Frontier::new(2).unwrap();
        let before = f.clone();
        assert_eq!(f.accept(3), Err(Error::Limit));
        assert_eq!(f.accept(0), Err(Error::InvalidInput));
        assert_eq!(f, before);
    }

    #[test]
    fn evidence_stages_are_distinct() {
        let mut f = EvidenceFrontiers::new(10).unwrap();
        assert_eq!(f.verify(1), Err(Error::Incomplete));
        f.capture(1).unwrap();
        assert_eq!(f.judge(1), Err(Error::Incomplete));
        f.verify(1).unwrap();
        f.judge(1).unwrap();
        assert!(f.covers_closed_scope(1));
        assert!(!f.covers_closed_scope(2));
    }

    #[test]
    fn rights_unknown_cannot_be_refunded_by_cancel() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        r.mark_unknown(1).unwrap();
        assert_eq!(r.abort_before_dispatch(1), Err(Error::WrongState));
        assert_eq!(r.available(), 7);
        assert!(r.conserved());
        r.reconcile(1, true).unwrap();
        assert!(r.conserved());
        assert_eq!(r.reconcile(1, false), Err(Error::WrongState));
    }

    #[test]
    fn effect_binding_and_one_shot_dispatch() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        let mut changed = e.clone();
        changed.payload.push(0);
        assert_eq!(r.dispatch(1, &changed), Err(Error::Binding));
        r.dispatch(1, &e).unwrap();
        assert_eq!(r.dispatch(1, &e), Err(Error::WrongState));
        assert!(r.conserved());
    }

    #[test]
    fn revocation_fences_reserved_not_history() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.revoke_epoch().unwrap();
        assert_eq!(r.dispatch(1, &e), Err(Error::Stale));
        r.abort_before_dispatch(1).unwrap();
        r.reserve(2, e.clone()).unwrap();
        r.dispatch(2, &e).unwrap();
        r.revoke_epoch().unwrap();
        assert_eq!(r.state(2), Ok(State::Dispatched));
        assert!(r.conserved());
    }

    #[test]
    fn reserve_budget_and_duplicate_fail_closed() {
        let mut r = Rights::new(3);
        r.reserve(1, effect()).unwrap();
        assert_eq!(r.reserve(1, effect()), Err(Error::Duplicate));
        assert_eq!(r.reserve(2, effect()), Err(Error::Limit));
        assert!(r.conserved());
    }

    #[test]
    fn resolved_non_effect_returns_rights_once() {
        let mut r = Rights::new(3);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        r.mark_unknown(1).unwrap();
        r.reconcile(1, false).unwrap();
        assert_eq!(r.available(), 3);
        assert_eq!(r.reconcile(1, false), Err(Error::WrongState));
        assert!(r.conserved());
    }

    #[test]
    fn reset_keeps_spent_and_epoch() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        r.reconcile(1, true).unwrap();
        r.revoke_epoch().unwrap();
        let spent = r.spent;
        let epoch = r.epoch();
        let available = r.available();

        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();

        assert_eq!(r.state(1), Ok(State::Committed));
        assert_eq!(r.spent, spent);
        assert_eq!(r.epoch(), epoch);
        assert_eq!(r.available(), available);
        assert_eq!(r.incident_count(), 1);
        assert!(r.conserved());
    }

    #[test]
    fn reset_increments_incident_counter() {
        let mut r = Rights::new(10);
        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();
        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();
        assert_eq!(r.incident_count(), 2);
        assert!(r.conserved());
    }

    #[test]
    fn reset_keeps_dispatched_liability() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        let available = r.available();
        let spent = r.spent;

        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();

        assert_eq!(r.state(1), Ok(State::Dispatched));
        assert_eq!(r.available(), available);
        assert_eq!(r.spent, spent);
        assert_eq!(r.incident_count(), 1);
        assert!(r.conserved());
    }

    #[test]
    fn reset_cannot_refund_unknown() {
        let mut r = Rights::new(10);
        let e = effect();
        r.reserve(1, e.clone()).unwrap();
        r.dispatch(1, &e).unwrap();
        r.mark_unknown(1).unwrap();
        let available = r.available();
        let spent = r.spent;
        let epoch = r.epoch();

        r.reset_to_checkpoint(&BTreeSet::new()).unwrap();

        assert_eq!(r.state(1), Ok(State::Unknown));
        assert_eq!(r.available(), available);
        assert_eq!(r.spent, spent);
        assert_eq!(r.epoch(), epoch);
        assert_eq!(r.incident_count(), 1);
        assert!(r.conserved());
    }

    #[test]
    fn reset_refunds_only_reserved_outside_keep() {
        let mut r = Rights::new(10);
        let kept = effect();
        let mut refunded = effect();
        refunded.resolved_target = "object:8".into();
        refunded.units = 5;
        r.reserve(1, kept).unwrap();
        r.reserve(2, refunded).unwrap();

        r.reset_to_checkpoint(&BTreeSet::from([1])).unwrap();

        assert_eq!(r.state(1), Ok(State::Reserved));
        assert_eq!(r.state(2), Ok(State::Aborted));
        assert_eq!(r.available(), 7);
        assert_eq!(r.incident_count(), 1);
        assert!(r.conserved());
    }

    #[test]
    fn reset_overflow_is_before_mutation() {
        let mut r = Rights::new(10);
        r.reserve(1, effect()).unwrap();
        r.incident_count = u64::MAX;
        let before = r.clone();

        assert_eq!(
            r.reset_to_checkpoint(&BTreeSet::new()),
            Err(Error::Overflow)
        );
        assert_eq!(r, before);
    }

    #[test]
    fn cross_reads_defeat_disjoint_writes() {
        let a = Operation {
            reads: BTreeSet::from([2]),
            writes: BTreeSet::from([1]),
            ..Operation::default()
        };
        let b = Operation {
            reads: BTreeSet::from([1]),
            writes: BTreeSet::from([2]),
            ..Operation::default()
        };
        assert!(!independent(&a, &b));
    }

    #[test]
    fn resources_and_order_barriers_block_reordering() {
        let mut a = Operation::default();
        let mut b = Operation::default();
        assert!(independent(&a, &b));
        a.consuming_resources.insert(7);
        b.consuming_resources.insert(7);
        assert!(!independent(&a, &b));
        b.consuming_resources.clear();
        b.external_or_ordered = true;
        assert!(!independent(&a, &b));
    }

    #[test]
    fn dominator_and_bypass_are_distinguished() {
        let g = Graph::new(3, &[(0, 1), (1, 2)]).unwrap();
        assert_eq!(g.dominates(0, 1, 2), Ok(Some(true)));
        let bypass = Graph::new(3, &[(0, 1), (1, 2), (0, 2)]).unwrap();
        assert_eq!(bypass.dominates(0, 1, 2), Ok(Some(false)));
        let empty = Graph::new(3, &[]).unwrap();
        assert_eq!(empty.dominates(0, 1, 2), Ok(None));
    }

    #[test]
    fn graph_limits_and_empty_claims_are_rejected() {
        assert_eq!(Graph::new(129, &[]), Err(Error::Limit));
        assert_eq!(Graph::new(2, &[(0, 2)]), Err(Error::InvalidInput));
        let g = Graph::new(2, &[]).unwrap();
        assert_eq!(
            g.cut_disconnects(&[0], &[], &BTreeSet::new()),
            Err(Error::InvalidInput)
        );
    }

    #[test]
    fn exhaustive_four_vertex_cut_and_dominator_consistency() {
        let pairs: Vec<_> = (0..4)
            .flat_map(|a| (0..4).filter(move |&b| a != b).map(move |b| (a, b)))
            .collect();
        for mask in 0..(1_u32 << pairs.len()) {
            let edges: Vec<_> = pairs
                .iter()
                .enumerate()
                .filter_map(|(i, &edge)| ((mask & (1 << i)) != 0).then_some(edge))
                .collect();
            let g = Graph::new(4, &edges).unwrap();
            if let Some(dominates) = g.dominates(0, 1, 3).unwrap() {
                assert_eq!(
                    dominates,
                    g.cut_disconnects(&[0], &[3], &BTreeSet::from([1])).unwrap()
                );
            }
        }
    }

    #[test]
    fn integer_probe_requests_refinement_at_margin() {
        assert_eq!(linear_sign(&[2, -1], &[10, 2], 0, 1), Ok(Some(true)));
        assert_eq!(linear_sign(&[1], &[1], 0, 1), Ok(None));
        assert_eq!(linear_sign(&[1], &[-4], 0, 1), Ok(Some(false)));
        assert_eq!(linear_sign(&[], &[], 0, 1), Err(Error::InvalidInput));
        assert_eq!(linear_sign(&[0], &[0], i128::MIN, 0), Err(Error::Overflow));
    }

    #[test]
    fn integer_probe_bound_matches_exhaustive_small_errors() {
        for x in -10..=10 {
            for y in -10..=10 {
                if let Some(sign) = linear_sign(&[2, -3], &[x, y], 1, 2).unwrap() {
                    for dx in -2..=2 {
                        for dy in -2..=2 {
                            assert_eq!(2 * (x + dx) - 3 * (y + dy) - 1 > 0, sign);
                        }
                    }
                }
            }
        }
    }
}
