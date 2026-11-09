//! Complete bounded projection, sealed linkage and imported-snapshot separation.
use super::*;
use fa_reference::address_resolver::ResolvedObject;
use fa_reference::event_journal::DomainEventJournal;
use fa_reference::explain_affordances::{
    ExplainClaimClass, PredicateNode, PredicateNodeKind, PredicateTree,
};
use fa_reference::observation_algebra::{
    Basis, BasisIdentity, ClaimClass, ProofStates, ScopedProof,
};
use fa_reference::tower::{TowerBuilder, TowerLimits, TowerValue};

pub(super) fn projection(response: &PublicationResponse) {
    let native = known(response);
    let encoded = response.to_json().unwrap();
    for secret in [
        PRIVATE_PAYLOAD,
        b"PRIVATE-SOURCE-BYTES",
        b"original-alpha",
        b"original-beta",
    ] {
        assert!(!encoded.contains(std::str::from_utf8(secret).unwrap()));
        let hex: String = secret.iter().map(|byte| format!("{byte:02x}")).collect();
        assert!(!encoded.contains(&hex));
    }
    let wire = document(response);
    let envelope = field(&wire, &["envelope"]).as_object().unwrap();
    assert_eq!(
        envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect()
    );
    assert_eq!(
        field(&wire, &["result", "kind"]).as_str(),
        Some("review-explanation")
    );
    let value = field(&wire, &["result", "value"]);
    assert_eq!(
        field(value, &["schema"]).as_str(),
        Some("fa.publication-review-explanation/1")
    );
    assert_eq!(
        field(value, &["scope"]).as_str(),
        Some("historical_completed_review")
    );
    assert_eq!(
        field(value, &["round"]).as_str(),
        Some(ROUND.to_string().as_str())
    );
    assert_eq!(field(value, &["historical"]).as_bool(), Some(true));
    assert_eq!(
        field(value, &["current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!(
        field(value, &["current_hold_reason", "state"]).as_str(),
        Some("unknown")
    );
    for flag in [
        "helpers_rerun",
        "live_evidence_read",
        "effect_authority",
        "payload_disclosed",
        "witness_values_disclosed",
        "helper_inputs_disclosed",
        "commitment_salts_disclosed",
    ] {
        assert_eq!(field(value, &[flag]).as_bool(), Some(false), "{flag}");
    }
    let body = field(value, &["knowledge", "value"]);
    assert_eq!(
        body.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        [
            "action",
            "policy",
            "evaluation",
            "congress",
            "decision",
            "application",
            "effect",
            "input_revision",
            "evidence_root",
            "timing"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        field(body, &["evaluation", "result"]).as_str(),
        Some("satisfied")
    );
    assert!(field(body, &["evaluation"]).get("witnesses").is_none());
    let congress = field(body, &["congress"]);
    assert_eq!(field(congress, &["member_count"]).as_str(), Some("2"));
    assert_eq!(field(congress, &["commit_count"]).as_str(), Some("2"));
    assert_eq!(field(congress, &["reveal_count"]).as_str(), Some("2"));
    assert_eq!(
        field(congress, &["permit_weight"]).as_str(),
        Some(native.tally().permit_weight.to_string().as_str())
    );
    for member in field(congress, &["members"]).as_array().unwrap() {
        assert_eq!(field(member, &["outcome"]).as_str(), Some("allow"));
        assert!(member.get("salt").is_none());
        assert!(member.get("inputs").is_none());
        assert_eq!(
            field(member, &["declared_weight"])
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap(),
            native.congress().members[field(member, &["member"]).as_str().unwrap()].weight
        );
    }
}

#[test]
fn sealed_known_unknown_withheld_and_exhausted_results_cannot_be_rebound() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let (port, grant) = supervisor.review_explanation_port(2).unwrap();
    let mut facade = tower(actor);
    facade.set_review_explanation(Some(port)).unwrap();
    let original = explain(&mut facade, REQUEST, ROUND);
    known(&original);
    let unknown = explain(&mut facade, REQUEST, ROUND + 1);
    unavailable(&unknown);
    let exhausted = explain(&mut facade, REQUEST, ROUND);
    assert_eq!(native(&exhausted), &Err(ActorError::Capacity));
    grant.revoke();
    let withheld = explain(&mut facade, REQUEST, ROUND);
    assert!(matches!(
        native(&withheld),
        Ok(ActorKnowledge::Withheld { .. })
    ));
    for response in [original, unknown, exhausted, withheld] {
        response.to_json().unwrap();
        for address in [
            facade.request_address(REQUEST + 1).unwrap(),
            Address::new(
                "1",
                Noun::Attempt,
                format!("request-3-2-9-5-{REQUEST}"),
                Some(41),
            )
            .unwrap(),
            Address::new(
                "1",
                Noun::Attempt,
                format!("request-4-2-4-5-{REQUEST}"),
                Some(41),
            )
            .unwrap(),
            Address::new(
                "2",
                Noun::Attempt,
                format!("request-3-2-4-5-{REQUEST}"),
                Some(41),
            )
            .unwrap(),
            Address::new(
                "1",
                Noun::Attempt,
                format!("request-3-2-4-5-{REQUEST}"),
                Some(42),
            )
            .unwrap(),
            facade.mount_address().clone(),
        ] {
            let mut changed = response.clone();
            changed.envelope.address = address;
            assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
    }
}

#[test]
fn complete_output_refusal_keeps_spent_work_and_larger_bound_returns_the_real_policy() {
    let root = Directory::new();
    let literal = vec![0xaa; 20_000];
    let nodes = vec![
        Predicate::PayloadIs(literal),
        Predicate::PayloadAtMost(16),
        Predicate::Any(vec![0, 1]),
    ];
    let mut configured = profile();
    configured.delivery.policy = Policy::new(1, nodes.clone()).unwrap();
    let (mut host, reviewer) = FileOversight::create(root.store(), configured).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = support::proposal(&oversight::spec(&host, b"published"));
    let (actor, mut supervisor) = host.into_actor_gateway();
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let _keys = ready(&mut supervisor, &reviewer, REQUEST);
    let (port, grant) = supervisor.review_explanation_port(2).unwrap();
    let mut bounded = tower(actor.clone());
    bounded.set_review_explanation(Some(port.clone())).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let response = explain(&mut bounded, REQUEST, ROUND);
    refused(&response, ReasonCode::BudgetExhausted);
    assert!(!response.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 1);
    let mut larger = PublicationTower::new(
        bounded.mount_address().clone(),
        actor,
        PublicationLimits {
            max_response_bytes: 128 * 1024,
        },
    )
    .unwrap();
    larger.set_review_explanation(Some(port)).unwrap();
    let response = explain(&mut larger, REQUEST, ROUND);
    assert_eq!(known(&response).policy().nodes(), nodes.as_slice());
    assert!(response.to_json().unwrap().len() > PublicationLimits::default().max_response_bytes);
    assert!(response.to_json().unwrap().contains("aaaa"));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(
        native(&explain(&mut bounded, REQUEST, ROUND)),
        &Err(ActorError::Capacity)
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn review_selector_never_falls_back_to_a_valid_imported_explanation_or_spends_its_budget() {
    let address = Address::new(
        "1",
        Noun::Attempt,
        format!("request-3-2-4-5-{REQUEST}"),
        Some(41),
    )
    .unwrap();
    let declared = ScopedProof::Declared {
        scope_id: 71,
        generation: 41,
    };
    let basis = Basis::new(
        BasisIdentity {
            subject_id: 72,
            question_id: 73,
            profile_id: 74,
            semantic_generation: 41,
            policy_epoch: 0,
            model_epoch: 1,
            tokenizer_epoch: 1,
            codec_epoch: 1,
            control_seq: 0,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: declared,
            complete_for_contract: declared,
            semantically_valid: ScopedProof::Unsupported { scope_id: 75 },
            available_for_replay: declared,
        },
    );
    let mut builder = TowerBuilder::new(
        DomainEventJournal::new_production("1", "review-snapshot"),
        basis,
        TowerLimits::default(),
        2,
    )
    .unwrap();
    builder
        .insert_object(
            ResolvedObject {
                plane: address.kind.plane(),
                address: address.clone(),
                summary_digest: 0xdec0_1ded,
                generation: 41,
            },
            basis,
        )
        .unwrap();
    let node = PredicateNode::new(
        "declared-policy",
        PredicateNodeKind::PolicyPredicate {
            predicate_id: "host-imported-declaration".into(),
        },
        ExplainClaimClass::ScopedProof,
        Knowledge::known(true, basis),
        vec![address.clone()],
        Vec::new(),
    )
    .unwrap();
    let tree = PredicateTree::new(address.clone(), node).unwrap();
    builder.insert_explanation(tree.clone(), basis).unwrap();
    let mut imported = builder.build();
    let mut selected = TowerRequest::new("native-review-only", address.clone(), Verb::Explain);
    selected.arguments = ReadArguments::ReviewExplanation { round: ROUND };
    let denied = imported.dispatch(&access(true), selected);
    assert_eq!(
        denied.envelope.reason_code,
        Some(ReasonCode::CapabilityUnsupported)
    );
    assert_eq!(imported.remaining_read_units(), 2);
    let plain = imported.dispatch(
        &access(true),
        TowerRequest::new("ordinary-imported-explanation", address, Verb::Explain),
    );
    let Knowledge::Known {
        value: TowerValue::Explain(observed),
        ..
    } = plain.result
    else {
        panic!("nearby ordinary snapshot read must remain available: {plain:?}");
    };
    assert_eq!(observed, tree);
    assert_eq!(imported.remaining_read_units(), 1);
}
