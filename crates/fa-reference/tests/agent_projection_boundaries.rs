//! Public authorization and generation regressions for the reference agent surface.
//!
//! CallerScope is independently supplied trusted input. These cases establish projection
//! behavior, not authentication, side-channel timing bounds or production qualification.
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use fa_reference::address_resolver::{
    Address, AddressResolver, CallerScope, Noun, RelatedEdge, RelationKind, ResolvedObject,
    ResolverPlane,
};
use fa_reference::event_journal::{
    DomainEventJournal, JournalEvent, JournalEventKind, SourceFrontierEntry,
};
use fa_reference::observation_algebra::{
    AuthorityKind, Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
    UnknownReason,
};
use fa_reference::product_frontier::{FrontierRequirement, FrontierStage, ProjectionKey};
use fa_reference::situation_rehearsal::{
    SituationItem, SituationReport, SituationSectionKind, fa_handoff, fold_situation_report,
    project_situation_for_scope, recompute_situation_batch_oracle,
};

fn address(tenant: &str, kind: Noun, id: &str, generation: u64) -> Address {
    Address::new(tenant, kind, id, Some(generation)).expect("valid address")
}

fn basis(generation: u64, control_seq: u64) -> Basis {
    let proof = ScopedProof::Declared {
        scope_id: 1,
        generation,
    };
    Basis::new(
        BasisIdentity {
            subject_id: 1,
            question_id: 1,
            profile_id: 1,
            semantic_generation: generation,
            policy_epoch: 1,
            model_epoch: 1,
            tokenizer_epoch: 1,
            codec_epoch: 1,
            control_seq,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: proof,
            complete_for_contract: proof,
            semantically_valid: proof,
            available_for_replay: proof,
        },
    )
}

fn insert(resolver: &mut AddressResolver, addr: &Address) {
    let generation = addr.generation.expect("fixture uses exact generations");
    resolver.insert_object(
        ResolvedObject {
            address: addr.clone(),
            plane: addr.kind.plane(),
            summary_digest: 17,
            generation,
        },
        basis(generation, 7),
    );
}

fn no_judgment_scope() -> CallerScope {
    let mut caller = CallerScope::full_tenant_access("alpha");
    caller.granted_authorities.remove(&AuthorityKind::Judge);
    caller
}

fn graph(hidden_count: usize) -> (AddressResolver, Address, Vec<Address>) {
    let mut resolver = AddressResolver::new();
    let source = address("alpha", Noun::Closure, "visible-closure", 2);
    let visible = address("alpha", Noun::Evidence, "visible-evidence", 1);
    let missing = address("alpha", Noun::Evidence, "visible-missing", 1);
    insert(&mut resolver, &source);
    insert(&mut resolver, &visible);
    for i in 0..hidden_count {
        let private = address(
            "alpha",
            Noun::Finding,
            &format!("private-finding-{i}"),
            i as u64 + 1,
        );
        let foreign = address(
            "beta",
            Noun::Evidence,
            &format!("private-foreign-{i}"),
            i as u64 + 10,
        );
        if i % 2 == 0 {
            insert(&mut resolver, &private);
            insert(&mut resolver, &foreign);
        }
        resolver.insert_edge(source.clone(), RelationKind::Provenance, private);
        resolver.insert_edge(source.clone(), RelationKind::ClosureStrongRef, foreign);
    }
    resolver.insert_edge(
        source.clone(),
        RelationKind::ClosureStrongRef,
        visible.clone(),
    );
    resolver.insert_edge(source.clone(), RelationKind::Provenance, missing.clone());
    (resolver, source, vec![visible, missing])
}

fn known_edges(result: &Knowledge<Vec<RelatedEdge>>) -> &[RelatedEdge] {
    match result {
        Knowledge::Known { value, .. } => value,
        other => panic!("expected authorized graph, got {other:?}"),
    }
}

#[test]
fn related_hides_inaccessible_edges_before_resolution_and_logging() {
    let caller = no_judgment_scope();
    for filter in [
        None,
        Some(RelationKind::ClosureStrongRef),
        Some(RelationKind::Provenance),
    ] {
        let (mut empty, source, visible) = graph(0);
        let expected = empty.fa_related(&caller, &source, filter);
        let expected_logs = empty.logs().to_vec();
        let edges = known_edges(&expected);
        assert_eq!(edges.len(), if filter.is_none() { 2 } else { 1 });
        assert!(edges.iter().all(|edge| visible.contains(&edge.to)));
        if filter != Some(RelationKind::Provenance) {
            assert!(
                edges
                    .iter()
                    .any(|edge| matches!(edge.target_resolution, Knowledge::Known { .. }))
            );
        }
        if filter != Some(RelationKind::ClosureStrongRef) {
            assert!(edges.iter().any(|edge| {
                edge.target_resolution == Knowledge::unknown(UnknownReason::MissingExactDomain)
            }));
        }

        // Hidden objects may be present, absent, differently generated, or numerous.
        // The complete visible graph and lookup log must remain identical.
        for hidden_count in [1, 2, 7] {
            let (mut populated, _, _) = graph(hidden_count);
            assert_eq!(populated.fa_related(&caller, &source, filter), expected);
            assert_eq!(populated.logs(), expected_logs.as_slice());
            assert!(populated.logs().iter().all(|entry| {
                !entry.requested_address.contains("private-")
                    && !entry.requested_address.contains("beta")
            }));
        }
    }

    // Independently granting the missing same-tenant scope reveals that permitted edge.
    let (mut resolver, source, _) = graph(1);
    let granted = resolver.fa_related(&CallerScope::full_tenant_access("alpha"), &source, None);
    let edges = known_edges(&granted);
    assert_eq!(edges.len(), 3);
    assert!(edges.iter().any(|edge| {
        edge.to.kind == Noun::Finding
            && matches!(edge.target_resolution, Knowledge::Known { .. })
    }));
    assert!(edges.iter().all(|edge| edge.to.tenant == "alpha"));
}

#[test]
fn related_checks_source_generation_before_expanding_current_edges() {
    let caller = CallerScope::full_tenant_access("alpha");
    for requested in [1, 3, u64::MAX] {
        let (mut resolver, mut source, _) = graph(1);
        source.generation = Some(requested);
        let result = resolver.fa_related(&caller, &source, None);
        if requested < 2 {
            assert_eq!(result, Knowledge::stale(basis(2, 7), 2));
        } else {
            assert_eq!(
                result,
                Knowledge::unknown(UnknownReason::IncompatibleGeneration),
            );
        }
        assert_eq!(resolver.logs().len(), 1);
        assert_eq!(resolver.logs()[0].verb, "related");
        assert_eq!(
            resolver.logs()[0].requested_address,
            source.canonical_string(),
        );
    }

    for requested in [Some(2), None] {
        let (mut resolver, mut source, _) = graph(0);
        source.generation = requested;
        let related = resolver.fa_related(&caller, &source, None);
        assert_eq!(known_edges(&related).len(), 2);
        assert!(
            known_edges(&related)
                .iter()
                .all(|edge| edge.from.generation == Some(2))
        );
        assert!(matches!(
            resolver.fa_get(&caller, &source),
            Knowledge::Known { .. }
        ));
    }
}

#[test]
fn unauthorized_source_refusal_precedes_existence_and_generation_checks() {
    let caller = CallerScope::full_tenant_access("beta");
    for generation in [0, 1, 2, 3, u64::MAX] {
        let (mut populated, mut source, _) = graph(4);
        source.generation = Some(generation);
        let mut absent = AddressResolver::new();
        let expected = Knowledge::withheld(AuthorityKind::Verify);
        assert_eq!(populated.fa_related(&caller, &source, None), expected);
        assert_eq!(absent.fa_related(&caller, &source, None), expected);
        assert_eq!(populated.logs(), absent.logs());
        assert_eq!(populated.logs().len(), 1);
    }
}

fn visible_item() -> Knowledge<SituationItem> {
    Knowledge::known(
        SituationItem {
            address: address("alpha", Noun::Attempt, "visible-attempt", 2),
            section: SituationSectionKind::HeldEffects,
            summary: "await an authorized review".to_owned(),
            decision_relevance_voi: 40,
            is_hard_floor: true,
        },
        basis(2, 7),
    )
}

fn report(hidden_count: usize, visible: bool) -> SituationReport {
    let mut items = Vec::new();
    let mut frontiers = Vec::new();
    for i in 0..hidden_count {
        let generation = i as u64 + 11;
        items.push(Knowledge::known(
            SituationItem {
                address: address("alpha", Noun::Finding, &format!("secret-{i}"), generation),
                section: SituationSectionKind::RiskTheaterFindings,
                summary: format!("private finding {i}"),
                decision_relevance_voi: 1000 + generation,
                is_hard_floor: true,
            },
            basis(generation, 500 + generation),
        ));
        frontiers.push(SourceFrontierEntry {
            source_id: 100 + i as u64,
            frontier_seq: 1000 + i as u64,
        });
    }
    if visible {
        items.push(visible_item());
    }
    SituationReport {
        tenant: "alpha".to_owned(),
        domain_id: format!("private-domain-{hidden_count}"),
        exact_at_control_seq: 100 + hidden_count as u64,
        open_frontiers: frontiers,
        pagination_cursor: Some(address(
            "alpha",
            Noun::Finding,
            "secret-cursor",
            hidden_count as u64,
        )),
        items,
    }
}

#[test]
fn partial_situations_preserve_visible_items_without_hidden_metadata() {
    let missing_authority = no_judgment_scope();
    let mut missing_plane = CallerScope::full_tenant_access("alpha");
    missing_plane
        .allowed_planes
        .remove(&ResolverPlane::DerivedProjections);

    for caller in [missing_authority, missing_plane] {
        let expected = project_situation_for_scope(&report(0, true), &caller);
        assert_eq!(
            expected.items,
            vec![visible_item(), Knowledge::withheld(AuthorityKind::Judge)],
        );
        assert_eq!(expected.domain_id, "withheld");
        assert_eq!(expected.exact_at_control_seq, 0);
        assert!(expected.open_frontiers.is_empty());
        assert_eq!(
            expected.pagination_cursor,
            Some(address("alpha", Noun::Attempt, "visible-attempt", 2)),
        );

        for hidden_count in [1, 3, 9] {
            let mut hidden = report(hidden_count, true);
            // These variants lack an address that the projection could authorize.
            hidden.items.push(
                Knowledge::pending(
                    vec![FrontierRequirement {
                        key: ProjectionKey {
                            source: hidden_count as u64,
                            branch: 17,
                            projection: 19,
                            source_epoch: 23,
                        },
                        stage: FrontierStage::Captured,
                        through: 29,
                        closure: None,
                    }],
                    1000,
                )
                .expect("valid pending frontier"),
            );
            hidden.items.push(Knowledge::stale(basis(31, 37), 41));
            hidden.items.push(Knowledge::unknown(UnknownReason::Incomplete));
            hidden.items.push(Knowledge::withheld(AuthorityKind::ExactRead));
            assert_eq!(project_situation_for_scope(&hidden, &caller), expected);
        }
    }
}

#[test]
fn fully_denied_situations_have_uniform_shape_for_empty_and_populated_reports() {
    let no_access = CallerScope {
        caller_tenant: "alpha".to_owned(),
        allowed_planes: BTreeSet::new(),
        granted_authorities: BTreeSet::new(),
    };
    let expected = project_situation_for_scope(&report(0, false), &no_access);
    assert_eq!(
        expected.items,
        vec![Knowledge::withheld(AuthorityKind::Verify)],
    );
    assert_eq!(expected.domain_id, "withheld");
    assert_eq!(expected.exact_at_control_seq, 0);
    assert!(expected.open_frontiers.is_empty());
    assert_eq!(expected.pagination_cursor, None);

    for hidden_count in [1, 4, 12] {
        for visible in [false, true] {
            let mut private = report(hidden_count, visible);
            private
                .items
                .push(Knowledge::stale(basis(100, u64::MAX), u64::MAX));
            private
                .items
                .push(Knowledge::unknown(UnknownReason::ConflictingEvidence));
            assert_eq!(project_situation_for_scope(&private, &no_access), expected);
        }
    }

    let foreign = CallerScope::full_tenant_access("beta");
    assert_eq!(
        project_situation_for_scope(&report(0, false), &foreign),
        project_situation_for_scope(&report(12, true), &foreign),
    );
}

#[test]
fn full_tenant_scope_retains_a_real_fold_and_permitted_partial_work() {
    let mut journal = DomainEventJournal::new_production("alpha", "visible-domain");
    let held = address("alpha", Noun::Attempt, "held", 1);
    journal
        .emit_transition(
            vec![SourceFrontierEntry {
                source_id: 1,
                frontier_seq: 1,
            }],
            vec![held.clone()],
            JournalEventKind::Hold {
                attempt_id: 1,
                reason: "needs_review".to_owned(),
            },
            [1; 32],
            1,
            10,
        )
        .expect("actual hold");
    let full = fold_situation_report(&journal, 1);
    assert_eq!(
        project_situation_for_scope(&full, &CallerScope::full_tenant_access("alpha")),
        full,
    );
    assert_eq!(full.exact_at_control_seq, 1);
    assert_eq!(full.pagination_cursor, Some(held.clone()));
    assert_eq!(
        full.open_frontiers,
        vec![SourceFrontierEntry {
            source_id: 1,
            frontier_seq: 1,
        }],
    );

    let partial = project_situation_for_scope(&full, &no_judgment_scope());
    assert_eq!(partial.items[0], full.items[0]);
    assert_eq!(partial.pagination_cursor, Some(held));
    assert_eq!(partial.items.len(), 2);
}

#[test]
fn maximum_control_sequence_folds_without_inventing_a_successor() {
    let mut journal = DomainEventJournal::new_production("alpha", "max-domain");
    let held = address("alpha", Noun::Attempt, "last-attempt", 1);
    journal
        .append(
            JournalEvent::new(
                u64::MAX,
                vec![SourceFrontierEntry {
                    source_id: 1,
                    frontier_seq: u64::MAX,
                }],
                vec![held.clone()],
                JournalEventKind::Hold {
                    attempt_id: 1,
                    reason: "last_sequence".to_owned(),
                },
                [2; 32],
                1,
                10,
            )
            .expect("maximum sequence is a valid final coordinate"),
        )
        .expect("append final event");

    let folded = fold_situation_report(&journal, u64::MAX);
    assert_eq!(folded, recompute_situation_batch_oracle(&journal, u64::MAX));
    assert_eq!(folded.exact_at_control_seq, u64::MAX);
    assert_eq!(folded.items.len(), 2);
    assert!(matches!(
        &folded.items[0],
        Knowledge::Known { value, basis }
            if value.address == held && basis.control_seq() == u64::MAX
    ));
    assert_eq!(folded.items[1], Knowledge::unknown(UnknownReason::Incomplete));
    assert!(journal.fa_tail(u64::MAX, 1).events.is_empty());
}

#[test]
fn related_edges_bind_exact_source_generations_including_zero() {
    let caller = CallerScope::full_tenant_access("alpha");
    let mut resolver = AddressResolver::new();
    let source_zero = address("alpha", Noun::Closure, "versioned-source", 0);
    let source_one = address("alpha", Noun::Closure, "versioned-source", 1);
    let old = address("alpha", Noun::Evidence, "old-target", 1);
    let old_alias = address("alpha", Noun::Evidence, "old-alias-target", 1);
    let future = address("alpha", Noun::Evidence, "future-target", 1);
    let current_alias = address("alpha", Noun::Evidence, "current-alias-target", 1);
    for target in [&old, &old_alias, &future, &current_alias] {
        insert(&mut resolver, target);
    }
    insert(&mut resolver, &source_zero);
    resolver.insert_edge(source_zero.clone(), RelationKind::Provenance, old.clone());
    resolver.insert_edge(
        source_zero.without_generation(),
        RelationKind::Provenance,
        old_alias.clone(),
    );
    resolver.insert_edge(source_one.clone(), RelationKind::Provenance, future.clone());

    let at_zero = resolver.fa_related(&caller, &source_zero.without_generation(), None);
    let edges = known_edges(&at_zero);
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].to, old);
    assert_eq!(edges[1].to, old_alias);
    assert!(edges.iter().all(|edge| edge.from == source_zero));

    insert(&mut resolver, &source_one);
    resolver.insert_edge(
        source_one.without_generation(),
        RelationKind::Provenance,
        current_alias.clone(),
    );
    let log_start = resolver.logs().len();
    let at_one = resolver.fa_related(&caller, &source_one.without_generation(), None);
    let edges = known_edges(&at_one);
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].to, future);
    assert_eq!(edges[1].to, current_alias);
    assert!(edges.iter().all(|edge| edge.from == source_one));
    assert!(resolver.logs()[log_start..].iter().all(|entry| {
        entry.requested_address != old.canonical_string()
            && entry.requested_address != old_alias.canonical_string()
    }));
    assert_eq!(
        resolver.fa_related(&caller, &source_zero, None),
        Knowledge::stale(basis(1, 7), 1),
    );
}

#[test]
fn unqualified_edges_without_a_source_never_attach_to_later_objects() {
    let caller = CallerScope::full_tenant_access("alpha");
    let mut resolver = AddressResolver::new();
    let source = address("alpha", Noun::Closure, "late-source", 2);
    let unbound = address("alpha", Noun::Evidence, "unbound-target", 1);
    let exact = address("alpha", Noun::Evidence, "exact-target", 1);
    let bound = address("alpha", Noun::Evidence, "bound-target", 1);
    for target in [&unbound, &exact, &bound] {
        insert(&mut resolver, target);
    }
    resolver.insert_edge(
        source.without_generation(),
        RelationKind::Provenance,
        unbound.clone(),
    );
    resolver.insert_edge(source.clone(), RelationKind::Provenance, exact.clone());
    assert_eq!(
        resolver.fa_related(&caller, &source.without_generation(), None),
        Knowledge::unknown(UnknownReason::MissingExactDomain),
    );

    insert(&mut resolver, &source);
    resolver.insert_edge(
        source.without_generation(),
        RelationKind::Provenance,
        bound.clone(),
    );
    let result = resolver.fa_related(&caller, &source.without_generation(), None);
    let edges = known_edges(&result);
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].to, exact);
    assert_eq!(edges[1].to, bound);
    assert!(edges.iter().all(|edge| edge.from == source));
    assert!(
        resolver
            .logs()
            .iter()
            .all(|entry| entry.requested_address != unbound.canonical_string())
    );

    let replaced = address("alpha", Noun::Closure, "late-source", 3);
    insert(&mut resolver, &replaced);
    let result = resolver.fa_related(&caller, &replaced.without_generation(), None);
    assert!(known_edges(&result).is_empty());
}

fn emit_event(
    journal: &mut DomainEventJournal,
    frontier_seq: u64,
    addresses: Vec<Address>,
    kind: JournalEventKind,
) {
    journal
        .emit_transition(
            vec![SourceFrontierEntry {
                source_id: 1,
                frontier_seq,
            }],
            addresses,
            kind,
            [3; 32],
            1,
            frontier_seq,
        )
        .expect("valid journal event");
}

fn held_journal() -> (DomainEventJournal, Address) {
    let mut journal = DomainEventJournal::new_production("alpha", "shared-visible-domain");
    let held = address("alpha", Noun::Attempt, "held-attempt", 1);
    emit_event(
        &mut journal,
        1,
        vec![held.clone()],
        JournalEventKind::Hold {
            attempt_id: 1,
            reason: "needs_review".to_owned(),
        },
    );
    (journal, held)
}

#[test]
fn latest_handoff_discovery_tracks_the_current_generation_only() {
    let (mut journal, held) = held_journal();
    let mut resolver = AddressResolver::new();
    let first = fa_handoff(
        "first-handoff",
        1,
        fold_situation_report(&journal, 1),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        1,
        &mut resolver,
    )
    .expect("first handoff");
    emit_event(
        &mut journal,
        2,
        vec![held],
        JournalEventKind::Outcome {
            attempt_id: 1,
            outcome_class: "resolved".to_owned(),
        },
    );
    let second = fa_handoff(
        "second-handoff",
        2,
        fold_situation_report(&journal, 2),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        2,
        &mut resolver,
    )
    .expect("second handoff");
    assert_eq!(first.situation.items.len(), 1);
    assert!(second.situation.items.is_empty());

    let caller = CallerScope::full_tenant_access("alpha");
    let latest = address("alpha", Noun::Handoff, "latest", 2);
    for query in [latest.without_generation(), latest.clone()] {
        let result = resolver.fa_related(&caller, &query, Some(RelationKind::Provenance));
        let edges = known_edges(&result);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, latest);
        assert_eq!(edges[0].to, second.handoff_address);
        assert!(matches!(
            &edges[0].target_resolution,
            Knowledge::Known { value, .. } if value.generation == 2
        ));
    }
    assert!(matches!(
        resolver.fa_related(
            &caller,
            &address("alpha", Noun::Handoff, "latest", 1),
            Some(RelationKind::Provenance),
        ),
        Knowledge::Stale {
            current_generation: 2,
            ..
        }
    ));
    assert!(matches!(
        resolver.fa_get(&caller, &first.handoff_address),
        Knowledge::Known { value, .. } if value.address == first.handoff_address
    ));
}

#[test]
fn unrelated_private_events_do_not_refresh_visible_item_provenance() {
    let (visible_journal, held) = held_journal();
    let visible_report = fold_situation_report(&visible_journal, 1);
    let caller = no_judgment_scope();
    let expected = project_situation_for_scope(&visible_report, &caller);
    assert!(matches!(
        &expected.items[0],
        Knowledge::Known { basis, .. } if basis.control_seq() == 1
    ));

    // These independent journals share the same authorized event and its causal basis.
    // Later private suffixes may advance the whole-domain cut, never that item's basis.
    for hidden_count in [1_u64, 3, 7] {
        let (mut journal, _) = held_journal();
        for index in 0..hidden_count {
            emit_event(
                &mut journal,
                index + 2,
                vec![address(
                    "alpha",
                    Noun::Finding,
                    &format!("private-finding-{index}"),
                    index + 1,
                )],
                JournalEventKind::Finding {
                    finding_id: index + 1,
                    detector_id: "private_detector".to_owned(),
                    severity: 4,
                },
            );
        }
        let cut = hidden_count + 1;
        let report = fold_situation_report(&journal, cut);
        assert_eq!(report, recompute_situation_batch_oracle(&journal, cut));
        assert_eq!(report.exact_at_control_seq, cut);
        assert_eq!(report.items.len(), hidden_count as usize + 1);
        assert_eq!(project_situation_for_scope(&report, &caller), expected);
        assert_eq!(
            project_situation_for_scope(&report, &CallerScope::full_tenant_access("alpha")),
            report,
        );

        // An actual update to the permitted item has its own truthful causal sequence.
        emit_event(
            &mut journal,
            cut + 1,
            vec![held.clone()],
            JournalEventKind::Hold {
                attempt_id: 1,
                reason: "review_updated".to_owned(),
            },
        );
        let updated = project_situation_for_scope(
            &fold_situation_report(&journal, cut + 1),
            &caller,
        );
        assert_ne!(updated, expected);
        assert!(matches!(
            &updated.items[0],
            Knowledge::Known { value, basis }
                if value.address == held
                    && value.summary.contains("review_updated")
                    && basis.control_seq() == cut + 1
        ));
        assert_eq!(updated.items.len(), 2);
        assert_eq!(updated.exact_at_control_seq, 0);
    }
}

#[test]
fn full_scope_still_authorizes_the_pagination_cursor() {
    let (journal, _) = held_journal();
    let report = fold_situation_report(&journal, 1);
    let caller = CallerScope::full_tenant_access("alpha");
    assert_eq!(project_situation_for_scope(&report, &caller), report);

    let mut foreign_cursor = report.clone();
    foreign_cursor.pagination_cursor =
        Some(address("beta", Noun::Attempt, "private-cursor", 99));
    let mut expected = report;
    expected.pagination_cursor = None;
    assert_eq!(
        project_situation_for_scope(&foreign_cursor, &caller),
        expected,
    );
}

#[test]
fn item_producers_require_typed_subjects_and_preserve_known_state_on_omission() {
    let cases = [
        (
            Noun::Attempt,
            JournalEventKind::Hold {
                attempt_id: 1,
                reason: "typed_hold".to_owned(),
            },
        ),
        (
            Noun::Checkpoint,
            JournalEventKind::Reset {
                checkpoint_id: 1,
                incident_counter: 3,
            },
        ),
        (
            Noun::Finding,
            JournalEventKind::Finding {
                finding_id: 1,
                detector_id: "typed_detector".to_owned(),
                severity: 4,
            },
        ),
        (
            Noun::Fence,
            JournalEventKind::Fence {
                fence_id: 1,
                ack_count: 3,
            },
        ),
    ];
    for (noun, kind) in cases {
        let mut journal = DomainEventJournal::new_production("alpha", "typed-subjects");
        let subject = address("alpha", noun, "correct-subject", 3);
        let unrelated = address("alpha", Noun::Evidence, "unrelated-evidence", 1);
        emit_event(
            &mut journal,
            1,
            vec![unrelated.clone(), subject.clone()],
            kind.clone(),
        );
        let valid = fold_situation_report(&journal, 1);
        assert_eq!(valid.items.len(), 1);
        assert!(matches!(
            &valid.items[0],
            Knowledge::Known { value, basis }
                if value.address == subject && basis.control_seq() == 1
        ));

        emit_event(&mut journal, 2, vec![unrelated.clone()], kind.clone());
        let incomplete = fold_situation_report(&journal, 2);
        assert_eq!(
            incomplete,
            recompute_situation_batch_oracle(&journal, 2),
        );
        assert_eq!(
            incomplete.items,
            vec![
                valid.items[0].clone(),
                Knowledge::unknown(UnknownReason::Incomplete),
            ],
        );

        let mut missing = DomainEventJournal::new_production("alpha", "typed-subjects");
        emit_event(&mut missing, 1, vec![unrelated], kind);
        assert_eq!(
            fold_situation_report(&missing, 1).items,
            vec![Knowledge::unknown(UnknownReason::Incomplete)],
        );
    }
}

#[test]
fn finding_payloads_cannot_borrow_an_unrelated_attempts_authority() {
    let (visible_journal, _) = held_journal();
    let caller = no_judgment_scope();
    let expected = project_situation_for_scope(
        &fold_situation_report(&visible_journal, 1),
        &caller,
    );
    let (mut journal, _) = held_journal();
    let unrelated = address("alpha", Noun::Attempt, "unrelated-attempt", 1);
    let finding = address("alpha", Noun::Finding, "restricted-finding", 2);
    emit_event(
        &mut journal,
        2,
        vec![unrelated.clone(), finding.clone()],
        JournalEventKind::Finding {
            finding_id: 9,
            detector_id: "restricted_detector".to_owned(),
            severity: 4,
        },
    );
    let report = fold_situation_report(&journal, 2);
    assert_eq!(project_situation_for_scope(&report, &caller), expected);
    let full = project_situation_for_scope(
        &report,
        &CallerScope::full_tenant_access("alpha"),
    );
    assert_eq!(full, report);
    assert!(full.items.iter().any(|item| matches!(
        item,
        Knowledge::Known { value, basis }
            if value.address == finding
                && value.summary.contains("restricted_detector")
                && basis.control_seq() == 2
    )));

    // A journal-valid event without a Finding subject must not manufacture a Verify item.
    emit_event(
        &mut journal,
        3,
        vec![unrelated],
        JournalEventKind::Finding {
            finding_id: 9,
            detector_id: "invalid_replacement".to_owned(),
            severity: 5,
        },
    );
    let incomplete = fold_situation_report(&journal, 3);
    assert!(incomplete.items.contains(&Knowledge::unknown(UnknownReason::Incomplete)));
    assert!(incomplete.items.contains(&full.items[0]));
    assert_eq!(project_situation_for_scope(&incomplete, &caller), expected);
}

#[test]
fn retractions_require_the_complete_held_attempt_identity() {
    for kind in [
        JournalEventKind::Permit {
            attempt_id: 1,
            cost_units: 1,
        },
        JournalEventKind::Outcome {
            attempt_id: 1,
            outcome_class: "resolved".to_owned(),
        },
    ] {
        let (journal, held) = held_journal();
        let prior = fold_situation_report(&journal, 1).items;
        let unrelated = address("alpha", Noun::Attempt, "unrelated-attempt", 1);
        let other_generation = address("alpha", Noun::Attempt, "held-attempt", 2);
        let unqualified = held.without_generation();
        for addresses in [
            vec![address("alpha", Noun::Evidence, "no-attempt", 1)],
            vec![unrelated.clone()],
            vec![other_generation.clone()],
            vec![unqualified],
        ] {
            let (mut mismatched, _) = held_journal();
            emit_event(&mut mismatched, 2, addresses, kind.clone());
            let report = fold_situation_report(&mismatched, 2);
            assert_eq!(
                report.items,
                vec![
                    prior[0].clone(),
                    Knowledge::unknown(UnknownReason::Incomplete),
                ],
            );
            assert_eq!(report, recompute_situation_batch_oracle(&mismatched, 2));
        }

        // Exact identity can occur after other typed and untyped related addresses.
        let (mut matched, _) = held_journal();
        emit_event(
            &mut matched,
            2,
            vec![
                address("alpha", Noun::Evidence, "related-evidence", 1),
                unrelated,
                other_generation,
                held,
            ],
            kind,
        );
        let report = fold_situation_report(&matched, 2);
        assert!(report.items.is_empty());
        assert_eq!(report, recompute_situation_batch_oracle(&matched, 2));
    }
}
