//! Independent unit tests for the pure admission evaluator.
//!
//! Every row and observation here is a hypothetical compiler input. These
//! fixtures prove only that the evaluator discriminates reviewed identities,
//! targets, features, and graph edges as specified. They are not donor review
//! evidence, runtime/downloaded-artifact proof, or an approval of any package.

use crate::admission::{
    AdmissionSet, AdmissionSetError, DepKind, EdgeId, ObservedPackage, PackageId, ReviewedRow,
    SourceId, TargetScope, Violation, evaluate, project_observed_packages,
};
use crate::json::{Limits as JsonLimits, parse};

const TARGET_A: &str = "target-a";
const TARGET_B: &str = "target-b";
const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
const EXPECTED_GIT: &str = "git+https://example.invalid/reviewed?rev=abc#abc";
const CAPTURED_METADATA: &str = include_str!("../tests/fixtures/admission/metadata-current.json");

fn exact(source: &str) -> SourceId {
    SourceId::Exact(source.to_string())
}

fn package(source: SourceId, name: &str, version: &str) -> PackageId {
    PackageId {
        source,
        name: name.to_string(),
        version: version.to_string(),
    }
}

fn edge(to: PackageId) -> EdgeId {
    EdgeId {
        to,
        kind: DepKind::Normal,
    }
}

fn scope(features: &[&str], edges: &[EdgeId]) -> TargetScope {
    TargetScope {
        features: features
            .iter()
            .map(|feature| (*feature).to_string())
            .collect(),
        edges: edges.iter().cloned().collect(),
    }
}

fn row(id: PackageId, scopes: &[(&str, TargetScope)]) -> ReviewedRow {
    ReviewedRow {
        id,
        scopes: scopes
            .iter()
            .map(|(target, scope)| ((*target).to_string(), scope.clone()))
            .collect(),
    }
}

fn reviewed(rows: Vec<ReviewedRow>) -> AdmissionSet {
    AdmissionSet::new(rows).expect("hypothetical reviewed rows must have unique full identities")
}

fn observed(id: PackageId, features: &[&str], edges: &[EdgeId]) -> ObservedPackage {
    ObservedPackage {
        id,
        features: features
            .iter()
            .map(|feature| (*feature).to_string())
            .collect(),
        edges: edges.iter().cloned().collect(),
    }
}

fn assert_no_violations(set: &AdmissionSet, target: &str, package: &ObservedPackage) {
    assert_eq!(
        evaluate(set, target, package),
        Vec::<Violation>::new(),
        "the permitted twin must admit the same hypothetical observation"
    );
}

fn mutate_once(source: &str, from: &str, to: &str) -> String {
    assert_eq!(
        source.matches(from).count(),
        1,
        "captured Cargo-metadata anchor must occur exactly once"
    );
    source.replacen(from, to, 1)
}

#[test]
fn review_regression_projection_refuses_partially_unreadable_edge_kinds() {
    let input = r#"{"packages":[
        {"id":"a","name":"consumer","version":"1","source":"registry+example"},
        {"id":"b","name":"backend","version":"1","source":"registry+example"}],
        "resolve":{"nodes":[
            {"id":"a","features":[],"deps":[{"pkg":"b","dep_kinds":[{"kind":null,"target":null}]}]},
            {"id":"b","features":[],"deps":[]}]}}"#;
    let project = |text: &str| {
        let parsed = parse(text.as_bytes(), JsonLimits::default()).unwrap();
        let mut findings = Vec::new();
        let observations = project_observed_packages(&parsed, &mut findings);
        (observations, findings)
    };
    let (permitted, findings) = project(input);
    assert!(findings.is_empty());
    assert_eq!(permitted.len(), 2);
    assert_eq!(
        permitted
            .iter()
            .find(|p| p.id.name == "consumer")
            .unwrap()
            .edges
            .len(),
        1
    );
    let target_scoped = mutate_once(input, "\"target\":null", "\"target\":\"cfg(unix)\"");
    let (scoped_observations, scoped_findings) = project(&target_scoped);
    assert!(scoped_findings.is_empty());
    assert_eq!(scoped_observations, permitted);
    for invalid in [
        "42",
        "null",
        "{}",
        "{\"kind\":\"future-kind\",\"target\":null}",
        "{\"kind\":null}",
        "{\"kind\":null,\"target\":[]}",
    ] {
        let malformed = mutate_once(
            input,
            "\"dep_kinds\":[{\"kind\":null,\"target\":null}]",
            &format!("\"dep_kinds\":[{{\"kind\":null,\"target\":null}},{invalid}]"),
        );
        let (observations, findings) = project(&malformed);
        assert!(
            findings
                .iter()
                .any(|f| f.phase == crate::admission::Phase::Metadata
                    && f.code == "dependency_kind_unreadable"),
            "unreadable edge disappeared: {invalid}: {findings:?}"
        );
        assert!(observations.iter().all(|p| p.id.name != "consumer"));
    }
}

#[test]
fn review_regression_projection_refuses_duplicate_resolve_identity() {
    let parsed = parse(CAPTURED_METADATA.as_bytes(), JsonLimits::default()).unwrap();
    let mut findings = Vec::new();
    assert_eq!(project_observed_packages(&parsed, &mut findings).len(), 2);
    assert!(findings.is_empty());
    let node = "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/xtask#0.2.0\",\"dependencies\":[],\"deps\":[],\"features\":[]}";
    let malformed = mutate_once(CAPTURED_METADATA, node, &format!("{node},{node}"));
    let parsed = parse(malformed.as_bytes(), JsonLimits::default()).unwrap();
    let observed = project_observed_packages(&parsed, &mut findings);
    assert!(
        findings.iter().any(
            |f| f.phase == crate::admission::Phase::Graph && f.code == "duplicate_resolve_node"
        ),
        "duplicate node silently overwrote its predecessor: {findings:?}"
    );
    assert!(observed.is_empty(), "ambiguous graph must not be projected");
}

#[test]
fn same_edge_and_destination_are_admitted_on_a_but_refused_on_b() {
    let consumer = package(exact(REGISTRY), "consumer", "1.0.0");
    let backend = package(exact(REGISTRY), "backend", "1.0.0");
    let activated_edge = edge(backend.clone());
    let set = reviewed(vec![
        row(
            consumer.clone(),
            &[
                (TARGET_A, scope(&[], std::slice::from_ref(&activated_edge))),
                (TARGET_B, scope(&[], &[])),
            ],
        ),
        row(
            backend,
            &[(TARGET_A, scope(&[], &[])), (TARGET_B, scope(&[], &[]))],
        ),
    ]);
    let package = observed(consumer, &[], std::slice::from_ref(&activated_edge));

    assert_no_violations(&set, TARGET_A, &package);
    assert_eq!(
        evaluate(&set, TARGET_B, &package),
        vec![Violation::EdgeNotAdmitted {
            target: TARGET_B.to_string(),
            edge: activated_edge,
        }],
        "only the target changed; B must not inherit A's reviewed edge"
    );
}

#[test]
fn same_name_and_version_registry_impostor_is_not_the_reviewed_git_package() {
    let reviewed_id = package(exact(EXPECTED_GIT), "frankentorch-api", "2.0.0");
    let set = reviewed(vec![row(
        reviewed_id.clone(),
        &[(TARGET_A, scope(&[], &[]))],
    )]);
    let permitted = observed(reviewed_id, &[], &[]);
    let registry_impostor = observed(
        package(exact(REGISTRY), "frankentorch-api", "2.0.0"),
        &[],
        &[],
    );

    assert_no_violations(&set, TARGET_A, &permitted);
    assert_eq!(
        evaluate(&set, TARGET_A, &registry_impostor),
        vec![Violation::NoReviewedRow {
            id: registry_impostor.id,
        }],
        "holding name/version fixed while changing only source must refuse"
    );
}

#[test]
fn source_and_name_are_independent_full_identity_components() {
    let reviewed_id = package(exact(EXPECTED_GIT), "frankentorch-api", "2.0.0");
    let set = reviewed(vec![row(
        reviewed_id.clone(),
        &[(TARGET_A, scope(&[], &[]))],
    )]);
    let permitted = observed(reviewed_id, &[], &[]);
    let renamed = observed(
        package(exact(EXPECTED_GIT), "frankentorch-api-impostor", "2.0.0"),
        &[],
        &[],
    );

    assert_no_violations(&set, TARGET_A, &permitted);
    assert_eq!(
        evaluate(&set, TARGET_A, &renamed),
        vec![Violation::NoReviewedRow { id: renamed.id }],
        "holding source/version fixed while changing only name must refuse"
    );
}

#[test]
fn packages_sharing_one_registry_source_remain_distinct_full_identities() {
    let first = package(exact(REGISTRY), "first-package", "1.0.0");
    let second = package(exact(REGISTRY), "second-package", "1.0.0");
    let set = reviewed(vec![
        row(first.clone(), &[(TARGET_A, scope(&[], &[]))]),
        row(second.clone(), &[(TARGET_A, scope(&[], &[]))]),
    ]);

    assert_no_violations(&set, TARGET_A, &observed(first, &[], &[]));
    assert_no_violations(&set, TARGET_A, &observed(second, &[], &[]));
}

#[test]
fn duplicate_full_identity_is_rejected_instead_of_overwriting_a_reviewed_row() {
    let id = package(exact(REGISTRY), "duplicate", "1.0.0");
    let duplicate = AdmissionSet::new(vec![
        row(id.clone(), &[(TARGET_A, scope(&[], &[]))]),
        row(id.clone(), &[(TARGET_B, scope(&[], &[]))]),
    ]);

    assert_eq!(duplicate, Err(AdmissionSetError::DuplicateIdentity(id)));
}

#[test]
fn admitted_edge_with_no_reviewed_destination_is_refused_as_dangling() {
    let consumer = package(exact(REGISTRY), "consumer", "1.0.0");
    let absent_destination = package(exact(REGISTRY), "missing-backend", "1.0.0");
    let admitted_edge = edge(absent_destination);
    let set = reviewed(vec![row(
        consumer.clone(),
        &[(TARGET_A, scope(&[], std::slice::from_ref(&admitted_edge)))],
    )]);
    let package = observed(consumer, &[], std::slice::from_ref(&admitted_edge));

    assert_eq!(
        evaluate(&set, TARGET_A, &package),
        vec![Violation::EdgeDestinationNotReviewed {
            target: TARGET_A.to_string(),
            edge: admitted_edge,
        }],
        "an edge listed in scope still fails when its exact destination lacks a reviewed row"
    );
}

#[test]
fn feature_only_injected_transitive_edge_is_refused_even_when_features_match() {
    let consumer = package(exact(REGISTRY), "consumer", "1.0.0");
    let backend = package(exact(REGISTRY), "optional-backend", "1.0.0");
    let injected_edge = edge(backend.clone());
    let set = reviewed(vec![
        row(
            consumer.clone(),
            &[(TARGET_A, scope(&["feature-enables-backend"], &[]))],
        ),
        row(backend, &[(TARGET_A, scope(&[], &[]))]),
    ]);
    let package = observed(
        consumer,
        &["feature-enables-backend"],
        std::slice::from_ref(&injected_edge),
    );

    assert_eq!(
        evaluate(&set, TARGET_A, &package),
        vec![Violation::EdgeNotAdmitted {
            target: TARGET_A.to_string(),
            edge: injected_edge,
        }],
        "matching features cannot launder a feature-only transitive edge"
    );
}

#[test]
fn projected_cargo_metadata_reaches_evaluator_with_real_feature_and_dev_edge() {
    // This is a Cargo-shaped regression fixture, not donor evidence: it starts
    // from the captured two-local-package metadata and injects one activated
    // feature plus one dev edge. The evaluator receives only the production
    // projection, never a hand-constructed ObservedPackage.
    let fa_node = concat!(
        "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/crates/fa-reference#0.2.0\",",
        "\"dependencies\":[],\"deps\":[],\"features\":[]}"
    );
    let injected = concat!(
        "{\"id\":\"path+file:///Users/jemanuel/projects/franken_alignment/crates/fa-reference#0.2.0\",",
        "\"dependencies\":[],\"deps\":[{\"name\":\"xtask\",",
        "\"pkg\":\"path+file:///Users/jemanuel/projects/franken_alignment/xtask#0.2.0\",",
        "\"dep_kinds\":[{\"kind\":\"dev\",\"target\":null}]}],",
        "\"features\":[\"projection-exercise\"]}"
    );
    let metadata = mutate_once(CAPTURED_METADATA, fa_node, injected);
    let document = parse(metadata.as_bytes(), JsonLimits::default())
        .expect("the single mutation preserves Cargo metadata JSON");
    let mut findings = Vec::new();
    let projected = project_observed_packages(&document, &mut findings);

    assert!(
        findings.is_empty(),
        "a fully resolvable Cargo-shaped edge must project without structural findings: {findings:?}"
    );
    let fa_reference = projected
        .iter()
        .find(|package| package.id.name == "fa-reference")
        .expect("the resolve-backed fa-reference package must be projected");
    assert!(
        fa_reference.features.contains("projection-exercise") && fa_reference.features.len() == 1,
        "the resolve node's actual feature set must reach the observation"
    );
    let projected_edge = fa_reference
        .edges
        .iter()
        .find(|edge| edge.kind == DepKind::Dev)
        .expect("the resolve dependency must reach the observation as a dev edge")
        .clone();
    assert_eq!(projected_edge.to.name, "xtask");
    assert_eq!(projected_edge.to.version, "0.2.0");
    assert_eq!(
        projected_edge.to.source.clone(),
        SourceId::WorkspacePath {
            manifest_path: "xtask/Cargo.toml".to_string(),
        },
        "a projected edge must retain its exact destination identity"
    );

    let set = reviewed(vec![
        row(fa_reference.id.clone(), &[(TARGET_A, scope(&[], &[]))]),
        row(projected_edge.to.clone(), &[(TARGET_A, scope(&[], &[]))]),
    ]);
    assert_eq!(
        evaluate(&set, TARGET_A, fa_reference),
        vec![
            Violation::FeatureSetMismatch {
                expected: Vec::new(),
                observed: vec!["projection-exercise".to_string()],
            },
            Violation::EdgeNotAdmitted {
                target: TARGET_A.to_string(),
                edge: projected_edge,
            },
        ],
        "the evaluator must receive the production-projected feature and edge, not fabricated empties"
    );
}
