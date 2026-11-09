//! Native identity, disclosure and work boundaries precede completed-review replay.
use super::*;

#[test]
fn operator_and_separate_review_grant_precede_disclosure_or_work() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let mut facade = tower(actor);
    let (inspection, inspection_grant) = supervisor.inspection_port(2).unwrap();
    let (admission, admission_grant) = supervisor.explanation_port(2).unwrap();
    facade.set_inspection(Some(inspection)).unwrap();
    facade.set_explanation(Some(admission)).unwrap();
    let before = image(&root);
    refused(
        &explain(&mut facade, REQUEST, ROUND),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(inspection_grant.remaining_queries(), 2);
    assert_eq!(admission_grant.remaining_queries(), 2);
    let mut ordinary = request(&facade, REQUEST, ROUND);
    ordinary.arguments = ReadArguments::None;
    let response = facade.dispatch(&access(true), ordinary);
    assert!(matches!(
        response.result,
        PublicationResult::Explanation(Ok(ActorKnowledge::Known { .. }))
    ));
    assert_eq!(admission_grant.remaining_queries(), 1);

    let (review, review_grant) = supervisor.review_explanation_port(2).unwrap();
    facade.set_review_explanation(Some(review)).unwrap();
    let query = request(&facade, REQUEST, ROUND);
    refused(
        &facade.dispatch(&access(false), query.clone()),
        ReasonCode::DeniedByPolicy,
    );
    let mut no_authority_scope = CallerScope::full_tenant_access("1");
    no_authority_scope.granted_authorities.clear();
    let no_authority = TowerAccess::new(no_authority_scope, true, Vec::new(), 0);
    refused(
        &facade.dispatch(&no_authority, query),
        ReasonCode::DeniedByPolicy,
    );
    assert_eq!(review_grant.remaining_queries(), 2);
    let response = explain(&mut facade, REQUEST, ROUND);
    assert_eq!(known(&response).basis().round, ROUND);
    assert_eq!(review_grant.remaining_queries(), 1);
    assert_eq!(inspection_grant.remaining_queries(), 2);
    assert_eq!(admission_grant.remaining_queries(), 1);
    assert_eq!(image(&root), before);
}

#[test]
fn closed_round_grammar_and_exact_request_mount_refuse_before_native_spending() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let (port, grant) = supervisor.review_explanation_port(2).unwrap();
    let mut facade = tower(actor);
    facade.set_review_explanation(Some(port)).unwrap();
    let query = request(&facade, REQUEST, ROUND);
    let wire = encode_request(&query).unwrap();
    assert_eq!(decode_request(wire.as_bytes()).unwrap(), query);
    let selector = format!("\"round\":\"{ROUND}\"");
    for invalid in [
        "\"round\":0",
        "\"round\":\"0\"",
        "\"round\":\"0801\"",
        "\"round\":\"+801\"",
        "\"round\":\"-1\"",
        "\"round\":null",
        "\"round\":true",
        "\"round\":\"18446744073709551616\"",
        "\"round\":\"801\",\"round\":\"802\"",
        "\"round\":\"801\",\"operator\":true",
    ] {
        let damaged = wire.replace(&selector, invalid);
        assert_ne!(damaged, wire);
        assert!(decode_request(damaged.as_bytes()).is_err());
        refused(
            &facade.exchange(&access(true), damaged.as_bytes()),
            ReasonCode::IntegrityFailure,
        );
    }
    let mut zero = query.clone();
    zero.arguments = ReadArguments::ReviewExplanation { round: 0 };
    assert!(encode_request(&zero).is_err());
    refused(
        &facade.dispatch(&access(true), zero),
        ReasonCode::IntegrityFailure,
    );
    let mut stale = query.clone();
    stale.address.generation = Some(40);
    refused(
        &facade.dispatch(&access(true), stale),
        ReasonCode::StaleGeneration,
    );
    let mut foreign = query.clone();
    foreign.address.tenant = "2".into();
    refused(
        &facade.dispatch(&access(true), foreign),
        ReasonCode::DeniedByPolicy,
    );
    let mut not_a_request = query.clone();
    not_a_request.address = facade.mount_address().clone();
    refused(
        &facade.dispatch(&access(true), not_a_request),
        ReasonCode::MissingRequiredObservation,
    );
    let mut wrong_scope = query.clone();
    wrong_scope.address.id = format!("request-3-2-99-5-{REQUEST}");
    refused(
        &facade.dispatch(&access(true), wrong_scope),
        ReasonCode::MissingRequiredObservation,
    );
    let mut wrong_selector = query;
    wrong_selector.arguments = ReadArguments::Next { max_cost: 1 };
    refused(
        &facade.dispatch(&access(true), wrong_selector),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 2);
    known(&explain(&mut facade, REQUEST, ROUND));
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn missing_unfinished_and_another_requests_completed_round_are_charged_unknown() {
    let root = Directory::new();
    let (actor, mut supervisor, _first_keys) = completed(&root);
    observe(&mut supervisor);
    let mut proposal = support::proposal(&oversight::spec(&supervisor.host().unwrap(), b"other"));
    proposal.expected_policy_epoch = supervisor.host().unwrap().inspect().control.ledger.epoch;
    actor.submit(REQUEST + 1, &proposal).unwrap();
    let other_inputs = begin(&mut supervisor, REQUEST + 1, ROUND + 1);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::votes(&mut host, ROUND + 1, Verdict::Allow);
        let revision = host.revision();
        host.finish_review(revision, ROUND + 1, Some(&other_inputs), snapshot())
            .unwrap()
            .unwrap();
    }
    let _unfinished_inputs = begin(&mut supervisor, REQUEST, ROUND + 2);
    let (port, grant) = supervisor.review_explanation_port(5).unwrap();
    let mut facade = tower(actor);
    facade.set_review_explanation(Some(port)).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    for (key, round) in [
        (REQUEST + 9, ROUND),
        (REQUEST + 1, ROUND),
        (REQUEST, ROUND + 2),
    ] {
        unavailable(&explain(&mut facade, key, round));
    }
    assert_eq!(grant.remaining_queries(), 2);
    let first = explain(&mut facade, REQUEST, ROUND);
    let second = explain(&mut facade, REQUEST + 1, ROUND + 1);
    assert_ne!(
        known(&first).basis().attempt,
        known(&second).basis().attempt
    );
    assert_eq!(known(&first).basis().request, REQUEST);
    assert_eq!(known(&second).basis().request, REQUEST + 1);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn clone_aliases_and_repeated_labels_spend_one_shared_grant_and_revocation_withholds() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let (port, grant) = supervisor.review_explanation_port(3).unwrap();
    let mut first = tower(actor.clone());
    let mut second = tower(actor);
    first.set_review_explanation(Some(port.clone())).unwrap();
    second.set_review_explanation(Some(port)).unwrap();
    let mut repeated = request(&first, REQUEST, ROUND);
    repeated.idempotency_key = Some("historical-read-is-not-a-free-retry".into());
    let original = first.dispatch(&access(true), repeated.clone());
    let again = second.dispatch(&access(true), repeated.clone());
    assert_eq!(known(&original), known(&again));
    assert_eq!(grant.remaining_queries(), 1);
    known(&first.dispatch(&access(true), repeated));
    assert_eq!(grant.remaining_queries(), 0);
    let exhausted = explain(&mut second, REQUEST, ROUND);
    assert_eq!(native(&exhausted), &Err(ActorError::Capacity));
    assert_eq!(
        exhausted.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    grant.revoke();
    for facade in [&mut first, &mut second] {
        let response = explain(facade, REQUEST, ROUND);
        assert_eq!(
            native(&response),
            &Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_review_explanation",
            })
        );
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
        response.to_json().unwrap();
    }
}

#[test]
fn exact_owner_installation_and_owner_loss_prevent_history_transfer() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let (port, grant) = supervisor.review_explanation_port(3).unwrap();
    let mut facade = tower(actor.clone());
    facade.set_review_explanation(Some(port.clone())).unwrap();
    let foreign_root = Directory::new();
    let (_foreign_actor, foreign_supervisor, _foreign_keys) = completed(&foreign_root);
    let (foreign, foreign_grant) = foreign_supervisor.review_explanation_port(1).unwrap();
    assert_eq!(
        facade.set_review_explanation(Some(foreign)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    refused(
        &explain(&mut facade, REQUEST, ROUND),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 3);
    assert_eq!(foreign_grant.remaining_queries(), 1);
    facade.set_review_explanation(Some(port.clone())).unwrap();
    let retained = explain(&mut facade, REQUEST, ROUND);
    known(&retained);
    assert_eq!(grant.remaining_queries(), 2);
    drop(supervisor);
    unavailable(&explain(&mut facade, REQUEST, ROUND));
    assert_eq!(grant.remaining_queries(), 2);
    // Retained response, actor handles and the grant cannot keep the old writer
    // lock alive. A new owner is fenced and must issue its own disclosure.
    let (reopened, _human) = FileOversight::open(root.store(), profile()).unwrap();
    let (reopened_actor, reopened_supervisor) = reopened.into_actor_gateway();
    let mut replacement = tower(reopened_actor);
    assert_eq!(
        replacement.set_review_explanation(Some(port)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    let (fresh, fresh_grant) = reopened_supervisor.review_explanation_port(1).unwrap();
    replacement.set_review_explanation(Some(fresh)).unwrap();
    let response = explain(&mut replacement, REQUEST, ROUND);
    assert_eq!(
        known(&response).basis().begin_revision,
        known(&retained).basis().begin_revision
    );
    assert!(known(&response).basis().journal_revision > known(&retained).basis().journal_revision);
    assert_eq!(fresh_grant.remaining_queries(), 0);
    unavailable(&explain(&mut facade, REQUEST, ROUND));
    assert_eq!(grant.remaining_queries(), 2);
}

#[test]
fn whole_history_bound_yields_charged_unknown_and_nearby_grant_reads_the_real_review() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let actual_events = usize::try_from(supervisor.host().unwrap().revision()).unwrap();
    assert!(actual_events > 1);
    let (small, small_grant) = supervisor
        .review_explanation_port_with_budget(1, actual_events - 1)
        .unwrap();
    let (complete, full_grant) = supervisor
        .review_explanation_port_with_budget(1, actual_events)
        .unwrap();
    let mut limited = tower(actor.clone());
    let mut enough = tower(actor);
    limited.set_review_explanation(Some(small)).unwrap();
    enough.set_review_explanation(Some(complete)).unwrap();
    let before = image(&root);
    unavailable(&explain(&mut limited, REQUEST, ROUND));
    assert_eq!(small_grant.remaining_queries(), 0);
    known(&explain(&mut enough, REQUEST, ROUND));
    assert_eq!(full_grant.remaining_queries(), 0);
    assert_eq!(image(&root), before);
}
