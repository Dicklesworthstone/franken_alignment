//! Public reference-Tower composition tests. Fixtures are host-imported journal,
//! object, evidence, and grant declarations; they do not authenticate a caller or
//! demonstrate production effect authority.

use std::collections::BTreeSet;

use fa_reference::action::consequence::{Consequence, Rule};
use fa_reference::address_resolver::{
    Address, CallerScope, Noun, RelationKind, ResolvedObject, ResolverPlane,
};
use fa_reference::event_journal::{
    DomainEventJournal, JournalEvent, JournalEventKind, SourceFrontierEntry,
};
use fa_reference::explain_affordances::{
    ExplainClaimClass, PredicateNode, PredicateNodeKind, PredicateTree,
};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{
    AuthorityKind, Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof,
    UnknownReason,
};
use fa_reference::product_frontier::{
    FrontierRequirement, FrontierStage, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::{
    CONNECTED_VERBS, ENVELOPE_FIELDS, RESPONSE_SCHEMA, ReadArguments, ReasonCode, ReferenceTower,
    ResponseStatus, TowerAccess, TowerBuilder, TowerInstallError, TowerLimits, TowerRequest,
    TowerResponse, TowerValue, Verb, VerbClass,
};
use fa_reference::typestate_cx::PurposeTag;
use fa_reference::witness::DomainProjection;

const TENANT: &str = "tower-tenant";
const DOMAIN: &str = "operations";
const GENERATION: u64 = 42;
const CONTROL_SEQ: u64 = 3;
const READ_UNITS: u64 = 128;
// Replay and its plan use the genuine historical-source fixture in tower_replay.
const SNAPSHOT_READS: [Verb; 7] = [
    Verb::Get,
    Verb::Related,
    Verb::Tail,
    Verb::Situation,
    Verb::Explain,
    Verb::WhyHeld,
    Verb::Next,
];
const HOLD_REASON: &str = "await \"capture\"\nkeep \\ provenance 雪";

fn basis(control_seq: u64) -> Basis {
    Basis::new(
        BasisIdentity {
            subject_id: 701,
            question_id: 702,
            profile_id: 703,
            semantic_generation: GENERATION,
            policy_epoch: 17,
            model_epoch: 19,
            tokenizer_epoch: 23,
            codec_epoch: 29,
            control_seq,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: ScopedProof::Declared {
                scope_id: 711,
                generation: GENERATION,
            },
            complete_for_contract: ScopedProof::Declared {
                scope_id: 712,
                generation: GENERATION,
            },
            semantically_valid: ScopedProof::Unsupported { scope_id: 713 },
            available_for_replay: ScopedProof::Expired {
                scope_id: 714,
                generation: GENERATION - 1,
            },
        },
    )
}

fn address(kind: Noun, id: &str) -> Address {
    Address::new(TENANT, kind, id, Some(GENERATION)).expect("fixture address")
}

fn subject() -> Address {
    address(Noun::Closure, "held-closure")
}

fn evidence() -> Address {
    address(Noun::Evidence, "captured-evidence")
}

fn object(address: Address) -> ResolvedObject {
    ResolvedObject {
        plane: address.kind.plane(),
        address,
        summary_digest: 0x1234_abcd,
        generation: GENERATION,
    }
}

fn imported_journal() -> DomainEventJournal {
    let mut journal = DomainEventJournal::new_production(TENANT, DOMAIN);
    for (frontiers, addresses, kind, annotation) in [
        (
            vec![SourceFrontierEntry {
                source_id: 11,
                frontier_seq: 2,
            }],
            vec![address(Noun::Attempt, "attempt-9"), subject(), evidence()],
            JournalEventKind::Hold {
                attempt_id: 9,
                reason: HOLD_REASON.to_owned(),
            },
            90,
        ),
        (
            vec![
                SourceFrontierEntry {
                    source_id: 11,
                    frontier_seq: 7,
                },
                SourceFrontierEntry {
                    source_id: 22,
                    frontier_seq: 1,
                },
            ],
            vec![address(Noun::Finding, "finding-8")],
            JournalEventKind::Finding {
                finding_id: 8,
                detector_id: "detector\"\n8".to_owned(),
                severity: 2,
            },
            10,
        ),
        (
            vec![SourceFrontierEntry {
                source_id: 22,
                frontier_seq: 3,
            }],
            vec![address(Noun::Fence, "fence-6")],
            JournalEventKind::Fence {
                fence_id: 6,
                ack_count: 4,
            },
            50,
        ),
    ] {
        journal
            .emit_transition(frontiers, addresses, kind, [0x5a; 32], 7, annotation)
            .expect("host journal fixture");
    }
    journal
}

fn pending_frontier() -> FrontierRequirement {
    FrontierRequirement {
        key: ProjectionKey {
            source: 11,
            branch: 2,
            projection: 3,
            source_epoch: 4,
        },
        stage: FrontierStage::Captured,
        through: 7,
        closure: None,
    }
}

fn node(id: &str, kind: PredicateNodeKind, status: Knowledge<bool>) -> PredicateNode {
    PredicateNode::new(
        id,
        kind,
        ExplainClaimClass::ScopedProof,
        status,
        vec![evidence()],
        Vec::new(),
    )
    .expect("declared predicate fixture")
}

fn tree() -> PredicateTree {
    let imported_basis = basis(CONTROL_SEQ);
    let children = vec![
        node(
            "policy",
            PredicateNodeKind::PolicyPredicate {
                predicate_id: "no_exact_disqualifier".to_owned(),
            },
            Knowledge::known(false, imported_basis),
        ),
        node(
            "evidence",
            PredicateNodeKind::EvidenceRef { domain_id: 11 },
            Knowledge::pending(vec![pending_frontier()], 2).expect("pending fixture"),
        ),
        node(
            "contradiction",
            PredicateNodeKind::ReducerRule {
                rule: Rule::Contradiction,
            },
            Knowledge::unknown(UnknownReason::ConflictingEvidence),
        ),
        node(
            "helper",
            PredicateNodeKind::HelperVerdict {
                helper_id: 15,
                recommendation: Consequence::HoldEffect,
            },
            Knowledge::known(false, imported_basis),
        ),
    ];
    let root = PredicateNode::new(
        "root",
        PredicateNodeKind::Conjunct,
        ExplainClaimClass::ScopedProof,
        Knowledge::known(false, imported_basis),
        vec![subject()],
        children,
    )
    .expect("root fixture");
    PredicateTree::new(subject(), root).expect("bounded imported tree")
}

fn builder(limits: TowerLimits, read_units: u64) -> TowerBuilder {
    let imported_basis = basis(CONTROL_SEQ);
    let mut builder = TowerBuilder::new(imported_journal(), imported_basis, limits, read_units)
        .expect("bounded snapshot fixture");
    builder
        .insert_object(object(subject()), imported_basis)
        .expect("subject object");
    builder
        .insert_object(object(evidence()), imported_basis)
        .expect("evidence object");
    builder
        .insert_edge(subject(), RelationKind::ClosureStrongRef, evidence())
        .expect("permitted reference edge");
    builder
}

fn fixture_with_tree(limits: TowerLimits, read_units: u64, tree: PredicateTree) -> ReferenceTower {
    let mut builder = builder(limits, read_units);
    builder
        .insert_explanation(tree, basis(CONTROL_SEQ))
        .expect("imported explanation");
    builder.build()
}

fn fixture() -> ReferenceTower {
    fixture_with_tree(TowerLimits::default(), READ_UNITS, tree())
}

fn all_purposes() -> Vec<PurposeTag> {
    vec![
        PurposeTag::Capture,
        PurposeTag::Judge,
        PurposeTag::Experiment,
        PurposeTag::Permit,
    ]
}

fn access() -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        true,
        all_purposes(),
        10,
    )
}

fn request(tower: &ReferenceTower, verb: Verb) -> TowerRequest {
    let target = if matches!(verb, Verb::Tail | Verb::Situation) {
        tower.domain_address().clone()
    } else {
        subject()
    };
    TowerRequest::new("fixture-operation", target, verb)
}

#[test]
fn native_situation_cursors_cannot_select_or_charge_an_imported_snapshot() {
    let mut tower = fixture();
    for arguments in [
        ReadArguments::SituationPage { cursor: u64::MAX },
        ReadArguments::SituationLinks { cursor: None },
        ReadArguments::SituationLinks {
            cursor: Some(u64::MAX),
        },
    ] {
        let mut call = request(&tower, Verb::Situation);
        call.arguments = arguments;
        let document = fa_reference::tower::encode_request(&call).unwrap();
        for response in [
            tower.dispatch(&access(), call),
            tower.exchange(&access(), document.as_bytes()),
        ] {
            assert_eq!(response.envelope.status, ResponseStatus::Error);
            assert_eq!(
                response.envelope.reason_code,
                Some(ReasonCode::CapabilityUnsupported)
            );
        }
    }
    assert_eq!(tower.remaining_read_units(), READ_UNITS);
    let ordinary = request(&tower, Verb::Situation);
    assert_eq!(
        tower.dispatch(&access(), ordinary).envelope.status,
        ResponseStatus::Ok
    );
    assert_eq!(tower.remaining_read_units(), READ_UNITS - 1);
}

fn known<T>(value: &Knowledge<T>) -> (&T, Basis) {
    match value {
        Knowledge::Known { value, basis } => (value, *basis),
        other => panic!("expected Known, got {}", other.variant_name()),
    }
}

fn parse(encoded: &str) -> Json {
    strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).expect("bounded strict JSON")
}

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value
        .get(name)
        .unwrap_or_else(|| panic!("missing JSON field {name}"))
}

fn text<'a>(value: &'a Json, name: &str) -> &'a str {
    field(value, name)
        .as_str()
        .unwrap_or_else(|| panic!("non-string field {name}"))
}

fn number(value: &Json, name: &str) -> u64 {
    field(value, name)
        .as_u64()
        .unwrap_or_else(|| panic!("non-u64 field {name}"))
}

fn body(json: &Json) -> &Json {
    field(field(field(json, "result"), "value"), "value")
}

fn registry() -> Json {
    parse(include_str!("../../../registry/vocabulary.json"))
}

fn assert_wire_envelope(json: &Json) {
    let registry = registry();
    let expected: BTreeSet<_> = field(&registry, "envelope_fields")
        .as_array()
        .expect("registry fields")
        .iter()
        .map(|value| value.as_str().expect("field name"))
        .collect();
    let actual: BTreeSet<_> = field(json, "envelope")
        .as_object()
        .expect("envelope object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(expected.len(), 16);
    assert_eq!(actual, expected);
    assert_eq!(
        text(field(json, "envelope"), "schema_version"),
        RESPONSE_SCHEMA
    );
    assert_eq!(json.as_object().expect("response object").len(), 2);
}

fn next_verbs(response: &TowerResponse) -> Vec<&str> {
    let (TowerValue::Next(affordances), _) = known(&response.result) else {
        panic!("expected next payload");
    };
    affordances
        .iter()
        .map(|affordance| affordance.verb)
        .collect()
}

#[test]
fn registry_verbs_classes_and_envelope_are_exact_not_a_second_vocabulary() {
    let registry = registry();
    let verbs = field(&registry, "verbs")
        .as_array()
        .expect("registered verbs");
    let registered: BTreeSet<_> = verbs.iter().map(|entry| text(entry, "verb")).collect();
    let implemented: BTreeSet<_> = Verb::ALL.iter().map(|verb| verb.as_str()).collect();
    assert_eq!(verbs.len(), 38);
    assert_eq!(registered.len(), 38);
    assert_eq!(implemented, registered);
    for entry in verbs {
        let verb: Verb = text(entry, "verb")
            .parse()
            .expect("every registered verb parses");
        let expected = match text(entry, "class") {
            "read" => VerbClass::Read,
            "rehearsal" => VerbClass::Rehearsal,
            "mutation" => VerbClass::Mutation,
            other => panic!("unknown registry class {other}"),
        };
        assert_eq!(verb.class(), expected, "{}", verb.as_str());
    }
    let connected: BTreeSet<_> = Verb::ALL
        .into_iter()
        .filter(|verb| verb.is_connected())
        .collect();
    assert_eq!(connected, CONNECTED_VERBS.into_iter().collect());
    assert_eq!(connected.len(), 9);
    for retired in field(&registry, "retired_verbs")
        .as_array()
        .expect("retired verbs")
    {
        assert!(
            retired
                .as_str()
                .expect("retired name")
                .parse::<Verb>()
                .is_err()
        );
    }
    for unknown in ["GET", "get ", "", "skip-floor"] {
        assert!(unknown.parse::<Verb>().is_err());
    }
    let registered_fields: Vec<_> = field(&registry, "envelope_fields")
        .as_array()
        .expect("fields")
        .iter()
        .map(|value| value.as_str().expect("field name"))
        .collect();
    assert_eq!(registered_fields, ENVELOPE_FIELDS);
}

#[test]
fn all_seven_reads_return_full_bodies_exact_basis_and_an_immutable_journal() {
    let mut tower = fixture();
    let expected_tree = tree();
    let expected_events: Vec<_> = imported_journal().events().cloned().collect();
    // Branch experiments have their own native-history fixture in tower_replay.
    for (index, verb) in SNAPSHOT_READS.into_iter().enumerate() {
        let query = request(&tower, verb);
        let response = tower.dispatch(&access(), query);
        assert_eq!(
            response.envelope.status,
            ResponseStatus::Ok,
            "{}",
            verb.as_str()
        );
        assert_eq!(response.envelope.reason_code, None);
        assert_eq!(
            known(&response.envelope.generation),
            (&GENERATION, basis(CONTROL_SEQ))
        );
        assert_eq!(
            known(&response.envelope.control_seq),
            (&CONTROL_SEQ, basis(CONTROL_SEQ))
        );
        assert_eq!(*known(&response.envelope.realized_cost).0, 1);
        assert_eq!(
            *known(&response.envelope.budgets_remaining).0,
            READ_UNITS - index as u64 - 1
        );
        assert!(response.envelope.retry_safe);
        assert!(response.envelope.missing.is_empty());
        assert!(response.envelope.would_change_this.is_empty());
        let json = parse(&response.to_json().expect("connected response encoding"));
        assert_wire_envelope(&json);
        assert_eq!(text(field(&json, "result"), "state"), "Known");
        let (value, observed_basis) = known(&response.result);
        assert_eq!(observed_basis, basis(CONTROL_SEQ));
        let expected_kind = match value {
            TowerValue::Object(value) => {
                assert_eq!(verb, Verb::Get);
                assert_eq!(value, &object(subject()));
                assert_eq!(text(body(&json), "address"), subject().canonical_string());
                assert_eq!(number(body(&json), "summary_digest"), value.summary_digest);
                "object"
            }
            TowerValue::Related(edges) => {
                assert_eq!(verb, Verb::Related);
                assert_eq!(edges.len(), 1);
                assert_eq!(edges[0].from, subject());
                assert_eq!(edges[0].to, evidence());
                assert_eq!(known(&edges[0].target_resolution).0, &object(evidence()));
                let edge = &body(&json).as_array().expect("edge bodies")[0];
                assert_eq!(text(edge, "to"), evidence().canonical_string());
                assert_eq!(text(field(edge, "target_resolution"), "state"), "Known");
                "related"
            }
            TowerValue::Tail(batch) => {
                assert_eq!(verb, Verb::Tail);
                assert_eq!(batch.events, expected_events);
                assert!(batch.gaps.is_empty());
                assert_eq!(batch.next_cursor_seq, CONTROL_SEQ);
                let events = field(body(&json), "events")
                    .as_array()
                    .expect("event bodies");
                assert_eq!(events.len(), expected_events.len());
                for (wire, event) in events.iter().zip(&expected_events) {
                    assert_eq!(wire, &parse(&event.encode_wire_canonical()));
                }
                assert_eq!(text(field(&events[0], "detail"), "reason"), HOLD_REASON);
                "tail"
            }
            TowerValue::Situation(report) => {
                assert_eq!(verb, Verb::Situation);
                assert_eq!(report.exact_at_control_seq, CONTROL_SEQ);
                assert_eq!(report.items.len(), 3);
                for item in &report.items {
                    let (item, item_basis) = known(item);
                    let producing_cut = match item.address.kind {
                        Noun::Attempt => 1,
                        Noun::Finding => 2,
                        Noun::Fence => 3,
                        other => panic!("unexpected situation subject {other:?}"),
                    };
                    assert_eq!(item_basis, basis(producing_cut));
                }
                let items = field(body(&json), "items")
                    .as_array()
                    .expect("situation items");
                assert_eq!(items.len(), 3);
                assert!(
                    items
                        .iter()
                        .any(|item| text(field(item, "value"), "summary").contains(HOLD_REASON))
                );
                for item in items {
                    let identity = field(field(item, "basis"), "identity");
                    assert_eq!(number(identity, "semantic_generation"), GENERATION);
                    assert_eq!(number(identity, "model_epoch"), 19);
                    let producing_cut = match text(field(item, "value"), "section") {
                        "held_effects" => 1,
                        "risk_theater_findings" => 2,
                        "authority" => 3,
                        other => panic!("unexpected situation section {other}"),
                    };
                    assert_eq!(number(identity, "control_seq"), producing_cut);
                }
                "situation"
            }
            TowerValue::Explain(explanation) => {
                assert_eq!(verb, Verb::Explain);
                assert_eq!(explanation, &expected_tree);
                assert_eq!(number(body(&json), "node_count"), 5);
                let root = field(body(&json), "root");
                assert_eq!(field(field(root, "status"), "value").as_bool(), Some(false));
                let children = field(root, "children")
                    .as_array()
                    .expect("predicate children");
                assert_eq!(text(field(&children[1], "status"), "state"), "Pending");
                assert_eq!(text(field(&children[2], "status"), "state"), "Unknown");
                "explain"
            }
            TowerValue::WhyHeld(report) => {
                assert_eq!(verb, Verb::WhyHeld);
                assert_eq!(report.exact_at_control_seq, CONTROL_SEQ);
                assert_eq!(report.items.len(), 5);
                assert_eq!(report.items[0].node.status, expected_tree.root.status);
                for (item, expected) in report.items[1..].iter().zip(&expected_tree.root.children) {
                    assert_eq!(item.node.status, expected.status);
                    assert_eq!(item.node.evidence_addresses, expected.evidence_addresses);
                }
                assert_eq!(
                    field(body(&json), "items")
                        .as_array()
                        .expect("why-held bodies")
                        .len(),
                    5
                );
                "why-held"
            }
            TowerValue::Next(affordances) => {
                assert_eq!(verb, Verb::Next);
                assert_eq!(
                    next_verbs(&response),
                    ["narrow", "resolve", "convene", "rehearse"]
                );
                assert_eq!(
                    field(field(&json, "result"), "value")
                        .as_object()
                        .expect("typed payload")
                        .len(),
                    2
                );
                let wire = body(&json).as_array().expect("affordance bodies");
                assert_eq!(wire.len(), affordances.len());
                for (wire, affordance) in wire.iter().zip(affordances) {
                    assert_eq!(text(wire, "affordance_id"), affordance.affordance_id);
                    assert_eq!(text(wire, "target"), affordance.target.canonical_string());
                    assert_eq!(number(wire, "cost_estimate"), affordance.cost_estimate);
                    assert_eq!(
                        number(wire, "expected_information_gain"),
                        affordance.expected_information_gain
                    );
                }
                "next"
            }
            TowerValue::Replay(_) => panic!("a read cannot produce a branch experiment"),
            TowerValue::ReplayPlan(_) => panic!("replay planning uses its original-source fixture"),
        };
        assert_eq!(
            text(field(field(&json, "result"), "value"), "kind"),
            expected_kind
        );
        if matches!(verb, Verb::Tail | Verb::Situation) {
            assert_eq!(
                known(&response.envelope.source_frontiers).0,
                &vec![
                    SourceFrontierEntry {
                        source_id: 11,
                        frontier_seq: 7
                    },
                    SourceFrontierEntry {
                        source_id: 22,
                        frontier_seq: 3
                    },
                ]
            );
        }
        assert_eq!(tower.journal_control_seq(), CONTROL_SEQ);
    }
    assert_eq!(tower.remaining_read_units(), READ_UNITS - 7);
}

#[test]
fn denied_scope_role_and_partial_domain_do_not_disclose_snapshot_or_budget() {
    let mut missing_plane = CallerScope::full_tenant_access(TENANT);
    missing_plane
        .allowed_planes
        .remove(&ResolverPlane::ExperimentStore);
    let mut missing_authority = CallerScope::full_tenant_access(TENANT);
    missing_authority
        .granted_authorities
        .remove(&AuthorityKind::Probe);
    let cases = [
        (
            TowerAccess::new(
                CallerScope::full_tenant_access("other-tenant"),
                true,
                all_purposes(),
                10,
            ),
            // This fixture is a production snapshot. Branch replay has its own
            // populated/empty admission controls in tower_replay.
            SNAPSHOT_READS.to_vec(),
        ),
        (
            TowerAccess::new(
                CallerScope::full_tenant_access(TENANT),
                false,
                all_purposes(),
                10,
            ),
            vec![Verb::Tail, Verb::Situation, Verb::WhyHeld, Verb::Next],
        ),
        (
            TowerAccess::new(missing_plane, true, all_purposes(), 10),
            vec![
                Verb::Tail,
                Verb::Situation,
                Verb::Explain,
                Verb::WhyHeld,
                Verb::Next,
            ],
        ),
        (
            TowerAccess::new(missing_authority, true, all_purposes(), 10),
            vec![
                Verb::Tail,
                Verb::Situation,
                Verb::Explain,
                Verb::WhyHeld,
                Verb::Next,
            ],
        ),
    ];
    for (denied_access, verbs) in cases {
        for verb in verbs {
            let mut populated = fixture();
            let empty_basis = Basis::new(
                BasisIdentity {
                    semantic_generation: 99,
                    ..basis(0).identity()
                },
                ClaimClass::Hypothesis,
                basis(0).proofs(),
            );
            let mut empty = TowerBuilder::new(
                DomainEventJournal::new_production(TENANT, DOMAIN),
                empty_basis,
                TowerLimits::default(),
                7,
            )
            .expect("different private snapshot")
            .build();
            let query = request(&populated, verb);
            let existing = populated.dispatch(&denied_access, query.clone());
            let absent = empty.dispatch(&denied_access, query);
            assert_eq!(
                existing,
                absent,
                "denied {} must not reveal imported state",
                verb.as_str()
            );
            assert_eq!(existing.to_json(), absent.to_json());
            assert_eq!(
                existing.envelope.reason_code,
                Some(ReasonCode::DeniedByPolicy)
            );
            assert!(matches!(existing.result, Knowledge::Withheld { .. }));
            for observed in [
                &existing.envelope.generation,
                &existing.envelope.control_seq,
                &existing.envelope.budgets_remaining,
                &existing.envelope.realized_cost,
            ] {
                assert!(matches!(observed, Knowledge::Withheld { .. }));
            }
            assert!(matches!(
                existing.envelope.source_frontiers,
                Knowledge::Withheld { .. }
            ));
            assert!(matches!(
                existing.envelope.affordances,
                Knowledge::Withheld { .. }
            ));
            assert_eq!(populated.remaining_read_units(), READ_UNITS);
            assert_eq!(empty.remaining_read_units(), 7);
            assert_eq!(populated.journal_control_seq(), CONTROL_SEQ);
            assert_eq!(empty.journal_control_seq(), 0);
            let query = request(&populated, verb);
            assert!(matches!(
                populated.dispatch(&access(), query).result,
                Knowledge::Known { .. }
            ));
        }
    }
}

#[test]
fn partial_get_and_related_preserve_object_cut_without_disclosing_global_accounting() {
    let old_basis = basis(1);
    let make = |mut journal: DomainEventJournal, extra_event: bool, units| {
        if extra_event {
            journal
                .emit_transition(
                    Vec::new(),
                    vec![subject()],
                    JournalEventKind::Proposal { attempt_id: 99 },
                    [0xa5; 32],
                    12,
                    100,
                )
                .expect("private later event");
        }
        let snapshot_basis = basis(journal.last_control_seq());
        let mut builder = TowerBuilder::new(journal, snapshot_basis, TowerLimits::default(), units)
            .expect("later snapshot");
        builder
            .insert_object(object(subject()), old_basis)
            .expect("earlier object");
        builder.build()
    };
    let mut first = make(imported_journal(), false, 10);
    let mut later = make(imported_journal(), true, 20);
    let mut partial_scope = CallerScope::full_tenant_access(TENANT);
    partial_scope
        .allowed_planes
        .remove(&ResolverPlane::ExperimentStore);
    let partial = TowerAccess::new(partial_scope, false, Vec::new(), 0);
    for verb in [Verb::Get, Verb::Related] {
        let query = request(&first, verb);
        let response = first.dispatch(&partial, query.clone());
        let later_response = later.dispatch(&partial, query);
        assert_eq!(response, later_response);
        assert_eq!(response.to_json(), later_response.to_json());
        assert_eq!(known(&response.result).1, old_basis);
        assert_eq!(known(&response.envelope.control_seq), (&1, old_basis));
        assert!(matches!(
            response.envelope.budgets_remaining,
            Knowledge::Withheld { .. }
        ));
        assert!(matches!(
            response.envelope.realized_cost,
            Knowledge::Withheld { .. }
        ));
        assert_eq!(response.envelope.status, ResponseStatus::Ok);
    }
    assert_eq!(first.remaining_read_units(), 8);
    assert_eq!(later.remaining_read_units(), 18);
    let query = request(&first, Verb::Get);
    let operator_view = first.dispatch(&access(), query);
    assert_eq!(*known(&operator_view.envelope.budgets_remaining).0, 7);
    assert_eq!(
        known(&operator_view.envelope.realized_cost).1,
        basis(CONTROL_SEQ)
    );
    assert_eq!(known(&operator_view.envelope.control_seq), (&1, old_basis));
}

#[test]
fn stale_and_future_cuts_are_not_relabelled_as_current_reads() {
    for verb in SNAPSHOT_READS {
        let mut tower = fixture();
        for requested_generation in [0, GENERATION - 1, GENERATION + 1, u64::MAX] {
            let mut query = request(&tower, verb);
            query.address.generation = Some(requested_generation);
            let response = tower.dispatch(&access(), query);
            assert_eq!(response.envelope.status, ResponseStatus::Error);
            assert_eq!(
                response.envelope.reason_code,
                Some(ReasonCode::StaleGeneration)
            );
            if requested_generation < GENERATION {
                assert_eq!(
                    response.result,
                    Knowledge::stale(basis(CONTROL_SEQ), GENERATION)
                );
            } else {
                assert_eq!(
                    response.result,
                    Knowledge::unknown(UnknownReason::IncompatibleGeneration)
                );
            }
            assert_eq!(*known(&response.envelope.realized_cost).0, 1);
            let json = parse(&response.to_json().expect("uncertain response"));
            assert!(field(&json, "result").get("value").is_none());
            assert_eq!(tower.journal_control_seq(), CONTROL_SEQ);
        }
        let mut query = request(&tower, verb);
        query.address.generation = None;
        assert!(matches!(
            tower.dispatch(&access(), query).result,
            Knowledge::Known { .. }
        ));
        assert_eq!(tower.remaining_read_units(), READ_UNITS - 5);
    }
}

#[test]
fn admitted_unknown_reads_spend_budget_but_invalid_queries_and_exhaustion_do_not() {
    let limits = TowerLimits {
        max_read_units: 2,
        ..TowerLimits::default()
    };
    let mut tower = fixture_with_tree(limits, 2, tree());
    for arguments in [
        ReadArguments::Tail {
            since_seq: 0,
            max_events: 0,
        },
        ReadArguments::Tail {
            since_seq: 0,
            max_events: limits.max_page_events + 1,
        },
        ReadArguments::Related { relation: None },
    ] {
        let mut query = request(&tower, Verb::Tail);
        query.arguments = arguments;
        let response = tower.dispatch(&access(), query);
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::BudgetExhausted)
        );
        assert_eq!(*known(&response.envelope.realized_cost).0, 0);
        assert_eq!(tower.remaining_read_units(), 2);
    }
    let mut invalid = request(&tower, Verb::Get);
    invalid.operation_id = "x".repeat(129);
    let refusal = tower.dispatch(&access(), invalid);
    assert_eq!(
        refusal.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert_eq!(refusal.envelope.operation_id, "<invalid-request>");
    assert_eq!(tower.remaining_read_units(), 2);
    let missing = TowerRequest::new("missing", address(Noun::Closure, "missing"), Verb::Get);
    let response = tower.dispatch(&access(), missing);
    assert_eq!(
        response.result,
        Knowledge::unknown(UnknownReason::MissingExactDomain)
    );
    assert_eq!(*known(&response.envelope.realized_cost).0, 1);
    assert_eq!(tower.remaining_read_units(), 1);
    let query = request(&tower, Verb::Get);
    assert!(matches!(
        tower.dispatch(&access(), query.clone()).result,
        Knowledge::Known { .. }
    ));
    let exhausted = tower.dispatch(&access(), query);
    assert_eq!(
        exhausted.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    assert!(!matches!(exhausted.result, Knowledge::Known { .. }));
    assert_eq!(*known(&exhausted.envelope.realized_cost).0, 0);
    assert_eq!(tower.remaining_read_units(), 0);
    assert_eq!(tower.journal_control_seq(), CONTROL_SEQ);
}

#[test]
fn tail_pages_keep_control_order_despite_clock_order_and_preserve_gap_uncertainty() {
    let mut tower = fixture();
    let mut first = request(&tower, Verb::Tail);
    first.arguments = ReadArguments::Tail {
        since_seq: 0,
        max_events: 2,
    };
    let first_response = tower.dispatch(&access(), first);
    let (TowerValue::Tail(first), _) = known(&first_response.result) else {
        panic!("tail page");
    };
    assert_eq!(
        first
            .events
            .iter()
            .map(|event| event.control_seq)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        first
            .events
            .iter()
            .map(|event| event.wall_clock_annotation_ms)
            .collect::<Vec<_>>(),
        [90, 10]
    );
    let mut second = request(&tower, Verb::Tail);
    second.arguments = ReadArguments::Tail {
        since_seq: first.next_cursor_seq,
        max_events: 2,
    };
    let second_response = tower.dispatch(&access(), second);
    let (TowerValue::Tail(second), _) = known(&second_response.result) else {
        panic!("tail page");
    };
    assert_eq!(
        second
            .events
            .iter()
            .map(|event| event.control_seq)
            .collect::<Vec<_>>(),
        [3]
    );
    let mut last = request(&tower, Verb::Tail);
    last.arguments = ReadArguments::Tail {
        since_seq: u64::MAX,
        max_events: 1,
    };
    let last_response = tower.dispatch(&access(), last);
    let (TowerValue::Tail(last), _) = known(&last_response.result) else {
        panic!("empty tail");
    };
    assert!(last.events.is_empty());
    assert_eq!(last.next_cursor_seq, u64::MAX);

    let mut journal = DomainEventJournal::new_production(TENANT, DOMAIN);
    let event = JournalEvent::new(
        3,
        Vec::new(),
        vec![address(Noun::Attempt, "attempt-gap")],
        JournalEventKind::Hold {
            attempt_id: 1,
            reason: "gap fixture".to_owned(),
        },
        [9; 32],
        0,
        1,
    )
    .expect("gapped source event");
    journal.append(event).expect("explicit gap is retained");
    let mut gapped = TowerBuilder::new(journal, basis(3), TowerLimits::default(), 2)
        .expect("bounded gapped import")
        .build();
    for verb in [Verb::Tail, Verb::Situation] {
        let query = request(&gapped, verb);
        let response = gapped.dispatch(&access(), query);
        let (value, observed_basis) = known(&response.result);
        assert_eq!(
            observed_basis.proofs().complete_for_contract,
            ScopedProof::Gapped {
                scope_id: 712,
                first_missing: 1
            }
        );
        assert_eq!(
            observed_basis.proofs().semantically_valid,
            basis(3).proofs().semantically_valid
        );
        assert!(
            response
                .result
                .as_known(ClaimClass::BoundedModel, GENERATION)
                .is_err()
        );
        match value {
            TowerValue::Tail(batch) => {
                assert_eq!(batch.events.len(), 1);
                assert_eq!(batch.gaps.len(), 1);
                assert_eq!(batch.gaps[0].0.missing_from_seq, 1);
                assert_eq!(batch.gaps[0].0.missing_through_seq, 2);
                assert_eq!(batch.gaps[0].1, UnknownReason::Incomplete);
            }
            TowerValue::Situation(report) => {
                assert!(report.items.iter().any(|item| matches!(
                    item,
                    Knowledge::Unknown {
                        reason: UnknownReason::Incomplete
                    }
                )));
                for item in &report.items {
                    if let Knowledge::Known { basis, .. } = item {
                        assert_eq!(*basis, observed_basis);
                    }
                }
            }
            _ => panic!("unexpected gapped read"),
        }
    }
    assert_eq!(gapped.journal_control_seq(), 3);
}

#[test]
fn hidden_explanation_references_withhold_every_explanation_surface() {
    for verb in [Verb::Explain, Verb::WhyHeld, Verb::Next] {
        let mut hidden_tree = tree();
        let secret = Address::new("secret-tenant", Noun::Evidence, "secret-object", Some(9))
            .expect("hidden reference");
        hidden_tree.root.children[1].evidence_addresses = vec![secret.clone()];
        let hidden_tree =
            PredicateTree::new(subject(), hidden_tree.root).expect("recomputed host tree");
        let mut hidden = fixture_with_tree(TowerLimits::default(), READ_UNITS, hidden_tree);
        let query = request(&hidden, verb);
        let response = hidden.dispatch(&access(), query);
        assert!(matches!(response.result, Knowledge::Withheld { .. }));
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
        let encoded = response.to_json().expect("bounded hidden result");
        assert!(!encoded.contains("secret-tenant"));
        assert!(!encoded.contains("secret-object"));
        assert!(!encoded.contains("aff-resolve-evidence"));
        assert!(field(&parse(&encoded), "result").get("value").is_none());
        assert_eq!(hidden.journal_control_seq(), CONTROL_SEQ);
        let mut visible = fixture();
        let query = request(&visible, verb);
        assert!(matches!(
            visible.dispatch(&access(), query).result,
            Knowledge::Known { .. }
        ));
    }
}

#[test]
fn next_and_why_held_respect_host_purposes_budget_and_request_narrowing() {
    for (purposes, host_budget, request_budget, expected) in [
        (
            all_purposes(),
            10,
            10,
            vec!["narrow", "resolve", "convene", "rehearse"],
        ),
        (all_purposes(), 2, u64::MAX, vec!["narrow", "resolve"]),
        (all_purposes(), 10, 1, vec!["narrow"]),
        (vec![PurposeTag::Capture], 10, 10, vec!["resolve"]),
        (vec![PurposeTag::Judge], 4, 10, Vec::new()),
        (vec![PurposeTag::Judge], 5, 10, vec!["convene"]),
        (Vec::new(), 10, 10, Vec::new()),
    ] {
        let grants = TowerAccess::new(
            CallerScope::full_tenant_access(TENANT),
            true,
            purposes.clone(),
            host_budget,
        );
        let mut tower = fixture();
        let mut query = request(&tower, Verb::Next);
        query.arguments = ReadArguments::Next {
            max_cost: request_budget,
        };
        let response = tower.dispatch(&grants, query);
        assert_eq!(next_verbs(&response), expected);
        let (TowerValue::Next(offered), _) = known(&response.result) else {
            panic!("next result");
        };
        assert_eq!(known(&response.envelope.affordances).0, offered);
        assert!(
            offered
                .iter()
                .all(|item| purposes.contains(&item.authority_required)
                    && item.cost_estimate <= host_budget.min(request_budget))
        );
        let query = request(&tower, Verb::WhyHeld);
        let why = tower.dispatch(&grants, query);
        let permitted = known(&why.envelope.affordances).0;
        let (TowerValue::WhyHeld(report), _) = known(&why.result) else {
            panic!("why-held result");
        };
        for item in &report.items {
            assert!(
                item.affordances
                    .iter()
                    .all(|affordance| permitted.contains(affordance))
            );
            assert!(item.affordances.iter().all(|affordance| {
                purposes.contains(&affordance.authority_required)
                    && affordance.cost_estimate <= host_budget
            }));
        }
        assert_eq!(tower.journal_control_seq(), CONTROL_SEQ);
    }
}

#[test]
fn explanation_retains_all_six_nested_knowledge_variants_and_their_original_basis() {
    let frontier = FrontierRequirement {
        stage: FrontierStage::Authenticated,
        closure: Some(8),
        ..pending_frontier()
    };
    let marker = TrustedClosingMarker {
        key: frontier.key,
        final_sequence: frontier.through,
        marker_generation: 8,
    };
    let absent = Knowledge::<bool>::absent_from_closed_frontier(
        DomainProjection::new(11, 12, frontier.key),
        13,
        14,
        15,
        frontier,
        marker,
    )
    .expect("declared closed domain");
    let old_basis = Basis::new(
        BasisIdentity {
            semantic_generation: 37,
            ..basis(2).identity()
        },
        ClaimClass::Hypothesis,
        basis(2).proofs(),
    );
    let independent_basis = Basis::new(
        BasisIdentity {
            subject_id: 901,
            question_id: 902,
            profile_id: 903,
            ..basis(3).identity()
        },
        ClaimClass::Hypothesis,
        ProofStates {
            authentic_origin: ScopedProof::Unsupported { scope_id: 904 },
            ..basis(3).proofs()
        },
    );
    let statuses = vec![
        Knowledge::known(false, independent_basis),
        Knowledge::pending(vec![pending_frontier()], 17).expect("pending"),
        Knowledge::unknown(UnknownReason::LowerLayerUnverified),
        Knowledge::withheld(AuthorityKind::ExactRead),
        Knowledge::stale(old_basis, GENERATION),
        absent,
    ];
    let mut root = node(
        "all-states-root",
        PredicateNodeKind::Conjunct,
        Knowledge::known(true, basis(3)),
    );
    root.children = statuses
        .iter()
        .enumerate()
        .map(|(index, status)| {
            node(
                &format!("state-{index}"),
                PredicateNodeKind::Conjunct,
                status.clone(),
            )
        })
        .collect();
    let imported = PredicateTree::new(subject(), root).expect("all-state tree");
    let mut tower = fixture_with_tree(TowerLimits::default(), READ_UNITS, imported);
    for verb in [Verb::Explain, Verb::WhyHeld] {
        let query = request(&tower, verb);
        let response = tower.dispatch(&access(), query);
        assert_eq!(known(&response.result).1, basis(3));
        let json = parse(&response.to_json().expect("all-state encoding"));
        let (value, _) = known(&response.result);
        let wire_nodes = match value {
            TowerValue::Explain(tree) => {
                assert_eq!(
                    tree.root
                        .children
                        .iter()
                        .map(|node| node.status.clone())
                        .collect::<Vec<_>>(),
                    statuses
                );
                field(field(body(&json), "root"), "children")
                    .as_array()
                    .expect("children")
                    .iter()
                    .collect::<Vec<_>>()
            }
            TowerValue::WhyHeld(report) => {
                assert_eq!(
                    report
                        .items
                        .iter()
                        .map(|item| item.node.status.clone())
                        .collect::<Vec<_>>(),
                    statuses
                );
                field(body(&json), "items")
                    .as_array()
                    .expect("held items")
                    .iter()
                    .map(|item| field(item, "node"))
                    .collect::<Vec<_>>()
            }
            _ => panic!("explanation operation"),
        };
        for (node, expected) in wire_nodes.iter().zip(&statuses) {
            let status = field(node, "status");
            assert_eq!(text(status, "state"), expected.variant_name());
            if let Some(expected_basis) = expected.basis() {
                let identity = field(field(status, "basis"), "identity");
                assert_eq!(
                    number(identity, "semantic_generation"),
                    expected_basis.semantic_generation()
                );
                assert_eq!(
                    number(identity, "control_seq"),
                    expected_basis.control_seq()
                );
            }
        }
    }
}

fn wide_tree() -> PredicateTree {
    let mut root = node(
        "wide-root",
        PredicateNodeKind::Conjunct,
        Knowledge::known(false, basis(3)),
    );
    root.children = (0..64)
        .map(|index| {
            node(
                &format!("leaf-{index}-{}", "x".repeat(80)),
                PredicateNodeKind::EvidenceRef { domain_id: index },
                Knowledge::known(false, basis(3)),
            )
        })
        .collect();
    PredicateTree::new(subject(), root).expect("bounded wide fixture")
}

#[test]
fn response_byte_cap_refuses_complete_work_without_truncating_known_or_refunding_budget() {
    let mut permitted = fixture_with_tree(TowerLimits::default(), 3, wide_tree());
    let query = request(&permitted, Verb::Explain);
    let large = permitted.dispatch(&access(), query);
    assert!(matches!(large.result, Knowledge::Known { .. }));
    let encoded = large.to_json().expect("permitted wide response");
    assert!(
        encoded.len() > 16 * 1024,
        "fixture must actually cross the smaller bound"
    );
    let limits = TowerLimits {
        max_response_bytes: 16 * 1024,
        ..TowerLimits::default()
    };
    let mut bounded = fixture_with_tree(limits, 3, wide_tree());
    let query = request(&bounded, Verb::Explain);
    let refused = bounded.dispatch(&access(), query);
    assert_eq!(refused.envelope.status, ResponseStatus::Error);
    assert_eq!(
        refused.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    assert_eq!(
        refused.result,
        Knowledge::unknown(UnknownReason::Incomplete)
    );
    assert_eq!(bounded.remaining_read_units(), 2);
    assert_eq!(*known(&refused.envelope.realized_cost).0, 1);
    let refusal_json = refused
        .to_json()
        .expect("bounded refusal itself must encode");
    assert!(refusal_json.len() <= limits.max_response_bytes);
    assert!(
        field(&parse(&refusal_json), "result")
            .get("value")
            .is_none()
    );
    assert_eq!(bounded.journal_control_seq(), CONTROL_SEQ);
    let query = request(&bounded, Verb::Get);
    assert!(matches!(
        bounded.dispatch(&access(), query).result,
        Knowledge::Known { .. }
    ));
    assert_eq!(bounded.remaining_read_units(), 1);
}

#[test]
fn permitted_sixteen_level_tree_reaches_the_wire_without_a_shallower_accidental_limit() {
    let mut root = node(
        "level-16",
        PredicateNodeKind::Conjunct,
        Knowledge::known(false, basis(3)),
    );
    for depth in (1..16).rev() {
        let mut parent = node(
            &format!("level-{depth}"),
            PredicateNodeKind::Conjunct,
            Knowledge::known(false, basis(3)),
        );
        parent.children.push(root);
        root = parent;
    }
    let tree = PredicateTree::new(subject(), root).expect("permitted maximum depth");
    assert_eq!(tree.max_depth, 16);
    let mut tower = fixture_with_tree(TowerLimits::default(), 1, tree);
    let query = request(&tower, Verb::Explain);
    let response = tower.dispatch(&access(), query);
    assert!(matches!(response.result, Knowledge::Known { .. }));
    let json = parse(
        &response
            .to_json()
            .expect("maximum depth is wire representable"),
    );
    assert_eq!(number(body(&json), "max_depth"), 16);
    let mut node = field(body(&json), "root");
    for depth in 1..=16 {
        assert_eq!(text(node, "node_id"), format!("level-{depth}"));
        let children = field(node, "children").as_array().expect("node children");
        if depth == 16 {
            assert!(children.is_empty());
        } else {
            assert_eq!(children.len(), 1);
            node = &children[0];
        }
    }
}

#[test]
fn every_unconnected_registered_verb_refuses_without_read_charge_or_journal_transition() {
    let mut tower = fixture();
    let before = tower.remaining_read_units();
    let original_events: Vec<_> = imported_journal().events().cloned().collect();
    let mut refused = 0;
    for verb in Verb::ALL {
        if verb.is_connected() {
            continue;
        }
        let mut query = request(&tower, verb);
        query.idempotency_key = Some("request-does-not-grant-effect-authority".to_owned());
        let response = tower.dispatch(&access(), query.clone());
        let retry = tower.dispatch(&access(), query);
        assert_eq!(response, retry);
        assert_eq!(response.envelope.status, ResponseStatus::Error);
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::CapabilityUnsupported)
        );
        assert_eq!(
            response.result,
            Knowledge::unknown(UnknownReason::Unavailable)
        );
        assert_eq!(
            response.envelope.retry_safe,
            verb.class() == VerbClass::Read
        );
        assert_eq!(tower.remaining_read_units(), before);
        assert_eq!(tower.journal_control_seq(), CONTROL_SEQ);
        let json = parse(&response.to_json().expect("unsupported response"));
        assert_wire_envelope(&json);
        assert!(field(&json, "result").get("value").is_none());
        refused += 1;
    }
    assert_eq!(refused, 29);
    let query = request(&tower, Verb::Tail);
    let response = tower.dispatch(&access(), query);
    let (TowerValue::Tail(batch), _) = known(&response.result) else {
        panic!("tail after refusals");
    };
    assert_eq!(batch.events, original_events);
    assert_eq!(tower.remaining_read_units(), before - 1);
}

#[test]
fn import_boundaries_refuse_mismatched_bases_limits_and_duplicate_predicate_ids() {
    let invalid_limits = TowerLimits {
        max_response_bytes: 16 * 1024 - 1,
        ..TowerLimits::default()
    };
    assert!(matches!(
        TowerBuilder::new(imported_journal(), basis(3), invalid_limits, 1),
        Err(TowerInstallError::InvalidLimits)
    ));
    assert!(matches!(
        TowerBuilder::new(imported_journal(), basis(2), TowerLimits::default(), 1),
        Err(TowerInstallError::SnapshotMismatch)
    ));
    assert!(matches!(
        TowerBuilder::new(imported_journal(), basis(3), TowerLimits::default(), 257),
        Err(TowerInstallError::InvalidLimits)
    ));
    let mut import = builder(TowerLimits::default(), 1);
    assert_eq!(
        import.insert_object(object(subject()), basis(3)),
        Err(TowerInstallError::DuplicateObject)
    );
    let wrong_plane = ResolvedObject {
        plane: ResolverPlane::ExperimentStore,
        ..object(address(Noun::Closure, "wrong-plane"))
    };
    assert_eq!(
        import.insert_object(wrong_plane, basis(3)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    assert_eq!(
        import.insert_object(object(address(Noun::Evidence, "future-cut")), basis(4)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    let mut duplicate = tree();
    duplicate.root.children[1].node_id = duplicate.root.children[0].node_id.clone();
    let duplicate =
        PredicateTree::new(subject(), duplicate.root).expect("native tree permits repeated labels");
    assert_eq!(
        import.insert_explanation(duplicate, basis(3)),
        Err(TowerInstallError::InvalidExplanation)
    );
    assert_eq!(
        import.insert_explanation(tree(), basis(2)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    for mismatched_identity in [
        BasisIdentity {
            semantic_generation: GENERATION - 1,
            ..basis(3).identity()
        },
        BasisIdentity {
            policy_epoch: 18,
            ..basis(3).identity()
        },
        BasisIdentity {
            model_epoch: 20,
            ..basis(3).identity()
        },
        BasisIdentity {
            tokenizer_epoch: 24,
            ..basis(3).identity()
        },
        BasisIdentity {
            codec_epoch: 30,
            ..basis(3).identity()
        },
        BasisIdentity {
            control_seq: 2,
            ..basis(3).identity()
        },
    ] {
        let mut mismatched = tree();
        mismatched.root.children[0].status = Knowledge::known(
            false,
            Basis::new(
                mismatched_identity,
                ClaimClass::BoundedModel,
                basis(3).proofs(),
            ),
        );
        let mismatched =
            PredicateTree::new(subject(), mismatched.root).expect("recomputed mismatch tree");
        assert_eq!(
            import.insert_explanation(mismatched, basis(3)),
            Err(TowerInstallError::SnapshotMismatch)
        );
    }
    import
        .insert_explanation(tree(), basis(3))
        .expect("corrected explanation imports");
    let mut tower = import.build();
    let query = request(&tower, Verb::Explain);
    assert!(matches!(
        tower.dispatch(&access(), query).result,
        Knowledge::Known { .. }
    ));

    let mut regressed = imported_journal();
    regressed
        .emit_transition(
            vec![SourceFrontierEntry {
                source_id: 11,
                frontier_seq: 6,
            }],
            vec![subject()],
            JournalEventKind::Proposal { attempt_id: 88 },
            [0; 32],
            0,
            100,
        )
        .expect("journal accepts source-coordinate regression as imported data");
    assert!(matches!(
        TowerBuilder::new(regressed, basis(4), TowerLimits::default(), 1),
        Err(TowerInstallError::SnapshotMismatch)
    ));
    let mut advanced = imported_journal();
    advanced
        .emit_transition(
            vec![SourceFrontierEntry {
                source_id: 11,
                frontier_seq: 8,
            }],
            vec![subject()],
            JournalEventKind::Proposal { attempt_id: 88 },
            [0; 32],
            0,
            100,
        )
        .expect("advanced source coordinate");
    assert!(TowerBuilder::new(advanced, basis(4), TowerLimits::default(), 1).is_ok());
}
