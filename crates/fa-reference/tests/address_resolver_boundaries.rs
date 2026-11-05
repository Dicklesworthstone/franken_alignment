//! Public read-boundary regressions for FA-134 and plan §17.6.
//!
//! These test the reference resolver's generation and disclosure semantics.
//! CallerScope remains supplied by its caller; these do not authenticate a caller
//! or establish production-store completeness or timing noninterference.

use fa_reference::{
    address_resolver::{
        Address, AddressResolver, CallerScope, Noun, RelatedEdge, RelationKind, ResolvedObject,
        ResolverPlane,
    },
    knowledge_surface::AgentSurfaceSerializer,
    observation_algebra::{
        AuthorityKind, Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
        UnknownReason,
    },
};

fn basis(generation: u64) -> Basis {
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
            control_seq: 11,
        },
        ClaimClass::Invariant,
        ProofStates {
            authentic_origin: proof,
            complete_for_contract: proof,
            semantically_valid: proof,
            available_for_replay: proof,
        },
    )
}

fn address(kind: Noun, id: &str, generation: Option<u64>) -> Address {
    Address::new("tenant-a", kind, id, generation).unwrap()
}

fn insert(resolver: &mut AddressResolver, address: &Address, generation: u64) {
    resolver.insert_object(
        ResolvedObject {
            address: address.clone(),
            plane: address.kind.plane(),
            summary_digest: 43,
            generation,
        },
        basis(generation),
    );
}

fn fixture() -> (AddressResolver, CallerScope, Address, Address) {
    let mut resolver = AddressResolver::new();
    let source = address(Noun::Closure, "source", Some(4));
    let target = address(Noun::Evidence, "visible", Some(4));
    insert(&mut resolver, &source, 4);
    insert(&mut resolver, &target, 4);
    resolver.insert_edge(
        source.clone(),
        RelationKind::ClosureStrongRef,
        target.clone(),
    );
    (
        resolver,
        CallerScope::full_tenant_access("tenant-a"),
        source,
        target,
    )
}

fn known_edges(result: Knowledge<Vec<RelatedEdge>>) -> Vec<RelatedEdge> {
    match result {
        Knowledge::Known { value, .. } => value,
        other => panic!("expected Known edges, got {other:?}"),
    }
}

#[test]
fn current_and_unversioned_sources_keep_authorized_traversal_and_basis() {
    let (mut resolver, caller, source, target) = fixture();
    for requested in [source.clone(), source.without_generation()] {
        let result = resolver.fa_related(&caller, &requested, None);
        let Knowledge::Known {
            value,
            basis: actual,
        } = result
        else {
            panic!("current source must resolve");
        };
        assert_eq!(actual, basis(4));
        assert_eq!(value.len(), 1);
        // An unqualified query still identifies the exact source generation traversed.
        assert_eq!(value[0].from, source);
        assert_eq!(value[0].to, target);
        assert!(matches!(
            value[0].target_resolution,
            Knowledge::Known { .. }
        ));
    }
}

#[test]
fn old_and_future_source_generations_match_get_without_expanding_targets() {
    let (resolver, caller, source, _) = fixture();
    for generation in [0, 3, 5, u64::MAX] {
        let requested = Address {
            generation: Some(generation),
            ..source.clone()
        };
        let mut get_resolver = resolver.clone();
        let expected = get_resolver
            .fa_get(&caller, &requested)
            .map(|_| Vec::<RelatedEdge>::new());
        assert!(!matches!(&expected, Knowledge::Known { .. }));
        let mut related_resolver = resolver.clone();
        assert_eq!(
            related_resolver.fa_related(&caller, &requested, None),
            expected
        );
        assert_eq!(related_resolver.logs().len(), 1);
        assert_eq!(related_resolver.logs()[0].verb, "related");
        assert_eq!(
            related_resolver.logs()[0].requested_address,
            requested.canonical_string()
        );
    }
}

#[test]
fn refreshing_source_does_not_let_its_old_address_read_current_edges() {
    let (mut resolver, caller, old_source, _) = fixture();
    let current_source = Address {
        generation: Some(5),
        ..old_source.clone()
    };
    insert(&mut resolver, &current_source, 5);
    let new_target = address(Noun::Annotation, "new-target", Some(5));
    insert(&mut resolver, &new_target, 5);
    resolver.insert_edge(
        current_source.clone(),
        RelationKind::AnnotationTarget,
        new_target.clone(),
    );
    let result = resolver.fa_related(&caller, &old_source, None);
    assert!(matches!(
        result,
        Knowledge::Stale {
            current_generation: 5,
            ..
        }
    ));
    assert_eq!(resolver.logs().len(), 1);
    let current_edges = known_edges(resolver.fa_related(
        &caller,
        &current_source,
        Some(RelationKind::AnnotationTarget),
    ));
    assert_eq!(current_edges.len(), 1);
    assert_eq!(current_edges[0].to, new_target);
}

#[test]
fn an_authorized_missing_source_does_not_expose_dangling_edges() {
    let (mut resolver, caller, _, target) = fixture();
    let missing = address(Noun::Closure, "missing-source", Some(4));
    resolver.insert_edge(missing.clone(), RelationKind::ClosureStrongRef, target);
    assert_eq!(
        resolver.fa_related(&caller, &missing, None),
        Knowledge::unknown(UnknownReason::MissingExactDomain)
    );
    assert_eq!(resolver.logs().len(), 1);
    assert_eq!(resolver.logs()[0].verb, "related");
    assert_eq!(resolver.logs()[0].authorization_decision, "allow_missing");
}

#[test]
fn hidden_cross_tenant_edges_do_not_change_results_serialization_or_logs() {
    let (baseline, caller, source, _) = fixture();
    let hidden = Address::new("tenant-b", Noun::Witness, "secret", Some(4)).unwrap();
    for target_exists in [false, true] {
        let mut expected_resolver = baseline.clone();
        let expected = expected_resolver.fa_related(&caller, &source, None);
        let mut actual_resolver = baseline.clone();
        if target_exists {
            insert(&mut actual_resolver, &hidden, 4);
        }
        for relation in [
            RelationKind::ClosureStrongRef,
            RelationKind::Provenance,
            RelationKind::SupportSetMember,
        ] {
            actual_resolver.insert_edge(source.clone(), relation, hidden.clone());
        }
        let actual = actual_resolver.fa_related(&caller, &source, None);
        assert_eq!(actual, expected);
        assert_eq!(
            AgentSurfaceSerializer::serialize_safety_field("related", &actual),
            AgentSurfaceSerializer::serialize_safety_field("related", &expected)
        );
        assert_eq!(actual_resolver.logs(), expected_resolver.logs());
    }
}

#[test]
fn missing_plane_and_missing_authority_each_hide_target_metadata_before_resolution() {
    let (baseline, caller, source, visible) = fixture();
    let hidden = address(Noun::Finding, "private-finding", Some(4));
    let mut plane_denied = caller.clone();
    plane_denied
        .allowed_planes
        .remove(&ResolverPlane::DerivedProjections);
    let mut authority_denied = caller;
    authority_denied
        .granted_authorities
        .remove(&AuthorityKind::Judge);
    for scope in [plane_denied, authority_denied] {
        let mut expected_resolver = baseline.clone();
        let expected = expected_resolver.fa_related(&scope, &source, None);
        let mut actual_resolver = baseline.clone();
        insert(&mut actual_resolver, &hidden, 4);
        actual_resolver.insert_edge(
            source.clone(),
            RelationKind::ThoughtGraphEdge,
            hidden.clone(),
        );
        let actual = actual_resolver.fa_related(&scope, &source, None);
        assert_eq!(actual, expected);
        assert_eq!(actual_resolver.logs(), expected_resolver.logs());
        let edges = known_edges(actual);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].to, visible);

        let full_scope = CallerScope::full_tenant_access("tenant-a");
        let restored = known_edges(actual_resolver.fa_related(&full_scope, &source, None));
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[1].to, hidden);
        assert!(matches!(
            restored[1].target_resolution,
            Knowledge::Known { .. }
        ));
    }
}

#[test]
fn unauthorized_sources_are_indistinguishable_before_generation_or_graph_lookup() {
    let (baseline, _, source, _) = fixture();
    let caller = CallerScope::full_tenant_access("tenant-b");
    let mut expected_resolver = baseline.clone();
    let expected = expected_resolver.fa_related(&caller, &source, None);
    assert!(matches!(&expected, Knowledge::Withheld { .. }));
    for (id, generation) in [
        ("source", Some(0)),
        ("source", Some(u64::MAX)),
        ("missing-source", Some(4)),
    ] {
        let requested = address(Noun::Closure, id, generation);
        let mut resolver = baseline.clone();
        assert_eq!(resolver.fa_related(&caller, &requested, None), expected);
        assert_eq!(resolver.logs().len(), 1);
        assert_eq!(resolver.logs()[0].verb, "related");
        assert_eq!(resolver.logs()[0].authorization_decision, "deny_withheld");
    }
}

#[test]
fn permitted_target_uncertainty_and_relation_filter_are_preserved() {
    let (mut resolver, caller, source, visible) = fixture();
    let old = address(Noun::Evidence, "old-target", Some(3));
    let current = Address {
        generation: Some(4),
        ..old.clone()
    };
    insert(&mut resolver, &current, 4);
    let future = Address {
        generation: Some(5),
        ..current
    };
    let missing = address(Noun::Evidence, "missing-target", Some(4));
    for target in [&old, &future, &missing] {
        resolver.insert_edge(source.clone(), RelationKind::Provenance, target.clone());
    }
    let edges = known_edges(resolver.fa_related(&caller, &source, Some(RelationKind::Provenance)));
    assert_eq!(edges.len(), 3);
    assert_eq!(edges[0].to, old);
    assert!(matches!(
        edges[0].target_resolution,
        Knowledge::Stale {
            current_generation: 4,
            ..
        }
    ));
    assert_eq!(
        edges[1].target_resolution,
        Knowledge::unknown(UnknownReason::IncompatibleGeneration)
    );
    assert_eq!(
        edges[2].target_resolution,
        Knowledge::unknown(UnknownReason::MissingExactDomain)
    );
    assert!(edges.iter().all(|edge| edge.to != visible));
    assert!(
        resolver
            .logs()
            .iter()
            .all(|entry| { entry.requested_address != visible.canonical_string() })
    );
}

#[test]
fn malformed_public_address_fields_cannot_resolve_even_after_unchecked_insertion() {
    for (tenant, id) in [
        ("tenant-a", ""),
        ("tenant-a", "bad/id"),
        ("tenant-a", "bad@4"),
        ("tenant-a", "bad\nline"),
        ("tenant-a", "bad\"quote"),
        ("", "source"),
        ("bad/tenant", "source"),
        ("bad\ntenant", "source"),
    ] {
        let malformed = Address {
            tenant: tenant.to_owned(),
            kind: Noun::Closure,
            id: id.to_owned(),
            generation: Some(4),
        };
        let caller = CallerScope::full_tenant_access(tenant);
        let mut resolver = AddressResolver::new();
        insert(&mut resolver, &malformed, 4);
        let target = Address::new("tenant-a", Noun::Evidence, "visible", Some(4)).unwrap();
        resolver.insert_edge(malformed.clone(), RelationKind::ClosureStrongRef, target);
        assert_eq!(
            resolver.fa_get(&caller, &malformed),
            Knowledge::unknown(UnknownReason::MissingExactDomain)
        );
        assert_eq!(
            resolver.fa_related(&caller, &malformed, None),
            Knowledge::unknown(UnknownReason::MissingExactDomain)
        );
        assert_eq!(resolver.logs().len(), 2);
        assert_eq!(resolver.logs()[1].verb, "related");
    }
}

#[test]
fn malformed_target_addresses_are_filtered_without_changing_visible_results() {
    let (mut expected_resolver, caller, source, _) = fixture();
    let mut actual_resolver = expected_resolver.clone();
    for id in ["", "bad/id", "bad@4", "bad\nline"] {
        let malformed = Address {
            tenant: "tenant-a".to_owned(),
            kind: Noun::Evidence,
            id: id.to_owned(),
            generation: Some(4),
        };
        insert(&mut actual_resolver, &malformed, 4);
        actual_resolver.insert_edge(source.clone(), RelationKind::Provenance, malformed);
    }
    assert_eq!(
        actual_resolver.fa_related(&caller, &source, None),
        expected_resolver.fa_related(&caller, &source, None)
    );
    assert_eq!(actual_resolver.logs(), expected_resolver.logs());
}
