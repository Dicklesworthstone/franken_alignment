//! Opt-in navigation preserves original request identity and separate custody.
//! These are reference integration checks, not production qualification.

use super::*;
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::strict_json::Json;
use fa_reference::tower::decode_request;

const FIRST_REQUEST: u64 = 7001;
const LEGACY_ROW_FIELDS: [&str; 8] = [
    "address",
    "attempt",
    "stage",
    "target",
    "units",
    "payload_bytes",
    "proposal_policy_epoch",
    "deadline",
];

fn linked_request(adapter: &PublicationTower, cursor: Option<u64>) -> TowerRequest {
    let mut call = request(adapter);
    call.operation_id = "inspect-request-links".to_owned();
    call.arguments = ReadArguments::SituationLinks { cursor };
    call
}

fn linked(adapter: &mut PublicationTower, cursor: Option<u64>) -> PublicationResponse {
    let call = linked_request(adapter, cursor);
    let encoded = encode_request(&call).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), call);
    adapter.exchange(&access(true), encoded.as_bytes())
}

fn linked_native(response: &PublicationResponse) -> &FileInspectionResult {
    match &response.result {
        PublicationResult::SituationLinks(value) => value.native(),
        other => panic!("expected original linked situation: {other:?}"),
    }
}

fn linked_known(response: &PublicationResponse) -> &FileInspectionPage {
    let Ok(ActorKnowledge::Known { value, basis }) = linked_native(response) else {
        panic!("expected original linked Known situation: {response:?}")
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.scope, profile().delivery.scope);
    assert_eq!(basis.origin, FileSituationOrigin::AcknowledgedOwner);
    assert_eq!(basis.anchored_through, None);
    assert_eq!(response.envelope.reason_code, None);
    no_global_claims(response);
    value
}

fn document(response: &PublicationResponse) -> Json {
    strict_json::parse(
        response.to_json().unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap()
}

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value.get(name).unwrap_or_else(|| panic!("missing {name}"))
}

fn body(document: &Json) -> &Json {
    let result = field(document, "result");
    assert_eq!(field(result, "kind").as_str(), Some("situation"));
    field(result, "value")
}

fn rows(document: &Json) -> &[Json] {
    field(
        field(field(body(document), "knowledge"), "value"),
        "attempts",
    )
    .as_array()
    .unwrap()
}

fn check_linked_wire(response: &PublicationResponse, adapter: &PublicationTower) {
    let page = linked_known(response);
    let document = document(response);
    assert_eq!(
        field(body(&document), "schema").as_str(),
        Some("fa.publication-situation/2")
    );
    assert_eq!(
        field(body(&document), "current_eligibility").as_str(),
        Some("not_observed")
    );
    let expected = LEGACY_ROW_FIELDS
        .into_iter()
        .chain(["external_request", "request_address"])
        .collect::<BTreeSet<_>>();
    assert_eq!(rows(&document).len(), page.attempts().len());
    for (encoded, original) in rows(&document).iter().zip(page.attempts()) {
        let fields = encoded.as_object().unwrap();
        assert_eq!(
            fields.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            expected
        );
        assert_eq!(
            fields["address"].as_str(),
            Some(original.address.to_string().as_str())
        );
        assert_eq!(
            fields["attempt"].as_str(),
            Some(original.attempt.to_string().as_str())
        );
        match original.external_request {
            Some(key) => {
                assert_eq!(
                    fields["external_request"].as_str(),
                    Some(key.to_string().as_str())
                );
                assert_eq!(
                    fields["request_address"].as_str(),
                    Some(
                        adapter
                            .request_address(key)
                            .unwrap()
                            .canonical_string()
                            .as_str()
                    )
                );
            }
            None => {
                assert!(fields["external_request"].is_null());
                assert!(fields["request_address"].is_null());
            }
        }
    }
}

fn populated(root: &Directory, attempts: usize) -> (Port, support::Supervisor, ActorProposal) {
    let mut config = profile();
    config.delivery.max_attempts = attempts + 1;
    let (mut host, _reviewer) = FileOversight::create(root.store(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = support::proposal(&oversight::spec(&host, b"published"));
    let (actor, mut supervisor) = host.into_actor_gateway();
    for offset in 0..attempts {
        observe(&mut supervisor);
        actor
            .submit(FIRST_REQUEST + offset as u64, &proposal)
            .unwrap();
    }
    (actor, supervisor, proposal)
}

fn cursor(response: &PublicationResponse) -> u64 {
    linked_known(response).continuation().unwrap().id()
}

fn conflict(response: &PublicationResponse) {
    assert_eq!(
        linked_native(response),
        &Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert_eq!(
        field(body(&document(response)), "schema").as_str(),
        Some("fa.publication-situation/2")
    );
    no_global_claims(response);
}

#[test]
fn explicit_links_preserve_legacy_bytes_and_pair_original_keys_with_nullable_addresses() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    {
        let mut host = supervisor.host_mut().unwrap();
        let direct = oversight::spec(&host, b"direct");
        let revision = host.revision();
        host.propose(revision, 1, direct, support::snapshot())
            .unwrap();
    }
    for key in [701, u64::MAX] {
        observe(&mut supervisor);
        actor.submit(key, &proposal).unwrap();
    }
    let (port, grant) = supervisor.inspection_port(3).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port)).unwrap();
    let original = supervisor.host().unwrap().inspect();
    let image = files(&root);
    let legacy = inspect(&mut adapter);
    let legacy_document = document(&legacy);
    assert_eq!(
        field(body(&legacy_document), "schema").as_str(),
        Some("fa.publication-situation/1")
    );
    for row in rows(&legacy_document) {
        assert_eq!(
            row.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            LEGACY_ROW_FIELDS.into_iter().collect()
        );
    }

    let selected = linked(&mut adapter, None);
    check_linked_wire(&selected, &adapter);
    assert_eq!(
        linked_known(&selected)
            .attempts()
            .iter()
            .map(|row| (row.attempt, row.external_request))
            .collect::<Vec<_>>(),
        vec![(1, None), (2, Some(701)), (3, Some(u64::MAX))]
    );
    let selected_document = document(&selected);
    assert_eq!(
        field(&rows(&selected_document)[1], "request_address").as_str(),
        Some("fa://1/attempt/request-3-2-4-5-701@41")
    );
    assert_eq!(
        field(&rows(&selected_document)[2], "external_request").as_str(),
        Some("18446744073709551615")
    );
    assert_eq!(known(&legacy).basis(), linked_known(&selected).basis());
    assert_eq!(known(&legacy).summary(), linked_known(&selected).summary());
    let legacy_again = inspect(&mut adapter);
    assert_eq!(legacy_again.to_json(), legacy.to_json());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), original);
    assert_eq!(files(&root), image);
}

#[test]
fn decoded_request_link_reaches_original_explanation_only_with_separate_custody() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    let ticket = actor.submit(701, &proposal).unwrap();
    let (port, inspection_grant) = supervisor.inspection_port(2).unwrap();
    let mut adapter = tower(actor.clone());
    adapter.set_inspection(Some(port)).unwrap();
    let image = files(&root);
    let original = supervisor.host().unwrap().inspect();
    let selected = linked(&mut adapter, None);
    let page = linked_known(&selected);
    let selected_document = document(&selected);
    let address = Address::parse(
        field(&rows(&selected_document)[0], "request_address")
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let explain = TowerRequest::new("follow-original-request", address.clone(), Verb::Explain);
    refused(
        &adapter.dispatch(&access(true), explain.clone()),
        ReasonCode::CapabilityUnsupported,
    );

    let (explanation, explanation_grant) = supervisor.explanation_port(2).unwrap();
    adapter.set_explanation(Some(explanation)).unwrap();
    refused(
        &adapter.dispatch(&access(false), explain.clone()),
        ReasonCode::DeniedByPolicy,
    );
    assert_eq!(explanation_grant.remaining_queries(), 2);
    let explained = adapter.exchange(&access(true), encode_request(&explain).unwrap().as_bytes());
    let PublicationResult::Explanation(Ok(ActorKnowledge::Known { value, basis })) =
        &explained.result
    else {
        panic!("link did not reach the original admission: {explained:?}")
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.request, 701);
    assert_eq!(basis.attempt, page.attempts()[0].attempt);
    assert_ne!(basis.request, basis.attempt);
    assert_eq!(value.initial_state(), ActionState::Reviewing);
    assert_eq!(value.ending_state(), ActionState::Reviewing);
    assert!(explained.to_json().is_ok());
    no_global_claims(&explained);

    // Native submission happened outside this facade. Neither inspection nor
    // explanation imports that original ticket into its separate wire session.
    let get = TowerRequest::new("link-is-not-a-ticket", address, Verb::Get);
    let response = adapter.exchange(&access(true), encode_request(&get).unwrap().as_bytes());
    let PublicationResult::Request(response) = response.result else {
        panic!("request route")
    };
    assert_eq!(response.request, Some(701));
    assert_eq!(
        response.result,
        Ok(ActorKnowledge::Withheld {
            authority_required: "own_request"
        })
    );
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: 701 }
    );
    assert_eq!(inspection_grant.remaining_queries(), 1);
    assert_eq!(explanation_grant.remaining_queries(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), original);
    assert_eq!(original.executions, 0);
    assert_eq!(files(&root), image);
}

#[test]
fn generationless_queries_keep_sealed_registered_links_and_reject_envelope_rebinding() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port)).unwrap();
    let image = files(&root);
    let mut query = linked_request(&adapter, None);
    query.address.generation = None;
    let selected = adapter.exchange(&access(true), encode_request(&query).unwrap().as_bytes());
    assert_eq!(selected.envelope.address, query.address);
    let PublicationResult::SituationLinks(sealed) = &selected.result else {
        panic!("sealed result")
    };
    assert_eq!(sealed.registration(), adapter.mount_address());
    assert_eq!(sealed.query_address(), &query.address);
    check_linked_wire(&selected, &adapter);

    let mut rebound = Vec::new();
    for generation in [Some(REGISTRATION), Some(REGISTRATION + 1)] {
        let mut address = query.address.clone();
        address.generation = generation;
        rebound.push(address);
    }
    rebound.push(Address::new("2", Noun::Run, "3", None).unwrap());
    rebound.push(Address::new("1", Noun::Run, "4", None).unwrap());
    rebound.push(adapter.request_address(701).unwrap());
    for address in rebound {
        let mut forged = selected.clone();
        forged.envelope.address = address;
        assert_eq!(forged.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    }
    assert_eq!(grant.remaining_queries(), 1);
    let mut explicit = linked(&mut adapter, None);
    check_linked_wire(&explicit, &adapter);
    explicit.envelope.address.generation = None;
    assert_eq!(
        explicit.to_json(),
        Err(TowerEncodingError::InvalidKnowledge)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(files(&root), image);
}

#[test]
fn linked_continuations_keep_original_mapping_after_owner_advance_and_reject_foreign_cursors() {
    let root = Directory::new();
    let count = MAX_INSPECTION_ATTEMPTS + 2;
    let (actor, mut supervisor, proposal) = populated(&root, count);
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    let (foreign_port, foreign_grant) = supervisor.inspection_port(2).unwrap();
    let mut adapter = tower(actor.clone());
    adapter.set_inspection(Some(port)).unwrap();
    let mut foreign = tower(actor.clone());
    foreign.set_inspection(Some(foreign_port)).unwrap();
    let first = linked(&mut adapter, None);
    let foreign_first = linked(&mut foreign, None);
    conflict(&linked(&mut adapter, Some(cursor(&foreign_first))));
    assert_eq!(grant.remaining_queries(), 3);
    assert_eq!(foreign_grant.remaining_queries(), 1);

    let changed_attempt = MAX_INSPECTION_ATTEMPTS as u64 + 1;
    let changed_key = FIRST_REQUEST + changed_attempt - 1;
    let ticket = actor.submit(changed_key, &proposal).unwrap();
    actor.cancel(&ticket).unwrap();
    observe(&mut supervisor);
    actor.submit(u64::MAX, &proposal).unwrap();
    let advanced = supervisor.host().unwrap().inspect();
    let image = files(&root);
    let tail = linked(&mut adapter, Some(cursor(&first)));
    assert_eq!(linked_known(&tail).basis(), linked_known(&first).basis());
    assert_eq!(
        linked_known(&tail).summary(),
        linked_known(&first).summary()
    );
    assert_eq!(linked_known(&tail).summary().attempts, count);
    assert_eq!(linked_known(&tail).summary().terminal_attempts, 0);
    for row in linked_known(&tail).attempts() {
        assert_eq!(row.external_request, Some(FIRST_REQUEST + row.attempt - 1));
        assert_eq!(row.stage, ActionState::Reviewing);
    }
    check_linked_wire(&first, &adapter);
    check_linked_wire(&tail, &adapter);
    assert!(linked_known(&tail).enumeration_complete());
    conflict(&linked(&mut adapter, Some(cursor(&first))));
    assert_eq!(grant.remaining_queries(), 2);

    let fresh = linked(&mut adapter, None);
    assert_eq!(
        linked_known(&fresh).basis().journal_revision,
        advanced.revision
    );
    assert_eq!(linked_known(&fresh).summary().attempts, count + 1);
    assert_eq!(linked_known(&fresh).summary().terminal_attempts, 1);
    let fresh_tail = linked(&mut adapter, Some(cursor(&fresh)));
    assert_eq!(
        linked_known(&fresh_tail)
            .attempts()
            .iter()
            .map(|row| (row.attempt, row.external_request))
            .collect::<Vec<_>>(),
        vec![
            (count as u64, Some(FIRST_REQUEST + count as u64 - 1)),
            (count as u64 + 1, Some(u64::MAX)),
            (changed_attempt, Some(changed_key)),
        ]
    );
    assert_eq!(
        linked_known(&fresh_tail).attempts().last().unwrap().stage,
        ActionState::Cancelled
    );
    check_linked_wire(&fresh_tail, &adapter);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(foreign_grant.remaining_queries(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), advanced);
    assert_eq!(files(&root), image);
}

#[test]
fn linked_output_bound_refuses_atomically_without_refunding_or_renewing_original_pages() {
    let root = Directory::new();
    let count = MAX_INSPECTION_ATTEMPTS * 2 + 1;
    let (actor, supervisor, _proposal) = populated(&root, count);
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port.clone())).unwrap();
    let image = files(&root);
    let first = linked(&mut adapter, None);
    let original_bytes = first.to_json().unwrap();
    let mut bounded = first.clone();
    bounded.envelope.limits.max_response_bytes = 64;
    assert_eq!(bounded.to_json(), Err(TowerEncodingError::OutputLimit));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(first.to_json().unwrap(), original_bytes);

    let final_page = linked(&mut adapter, Some(cursor(&first)));
    assert_eq!(linked_known(&final_page).offset(), MAX_INSPECTION_ATTEMPTS);
    assert_eq!(
        linked_known(&final_page).attempts().len(),
        MAX_INSPECTION_ATTEMPTS
    );
    assert_eq!(linked_known(&final_page).summary().attempts, count);
    assert!(!linked_known(&final_page).enumeration_complete());
    assert!(linked_known(&final_page).continuation().is_none());
    check_linked_wire(&final_page, &adapter);
    assert_eq!(grant.remaining_queries(), 0);
    conflict(&linked(&mut adapter, Some(cursor(&first))));
    let exhausted = linked(&mut adapter, None);
    assert_eq!(linked_native(&exhausted), &Err(ActorError::Capacity));
    assert_eq!(
        exhausted.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    assert_eq!(
        field(body(&document(&exhausted)), "schema").as_str(),
        Some("fa.publication-situation/2")
    );
    assert_eq!(port.situation(), Err(ActorError::Capacity));
    assert_eq!(files(&root), image);
}

#[test]
fn malformed_or_unprivileged_link_requests_do_not_spend_native_inspection() {
    let root = Directory::new();
    let (actor, supervisor, _reviewer, _proposal) = create(&root);
    let (port, grant) = supervisor.inspection_port(1).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port)).unwrap();
    let call = linked_request(&adapter, None);
    let encoded = encode_request(&call).unwrap();
    let image = files(&root);
    refused(
        &adapter.exchange(&access(false), encoded.as_bytes()),
        ReasonCode::DeniedByPolicy,
    );
    for arguments in [
        r#"{"request_links":false}"#,
        r#"{"request_links":"true"}"#,
        r#"{"request_links":true,"extra":true}"#,
        r#"{"request_links":true,"cursor":"0"}"#,
        r#"{"request_links":true,"cursor":1}"#,
        r#"{"request_links":true,"cursor":null}"#,
        r#"{"request_links":true,"cursor":"01"}"#,
        r#"{"request_links":true,"request_links":true}"#,
    ] {
        let malformed = encoded.replacen(r#"{"request_links":true}"#, arguments, 1);
        assert_ne!(malformed, encoded);
        refused(
            &adapter.exchange(&access(true), malformed.as_bytes()),
            ReasonCode::IntegrityFailure,
        );
    }
    let mut invalid = call;
    invalid.arguments = ReadArguments::SituationLinks { cursor: Some(0) };
    assert!(encode_request(&invalid).is_err());
    refused(
        &adapter.dispatch(&access(true), invalid),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 1);
    let complete = linked(&mut adapter, None);
    assert!(linked_known(&complete).attempts().is_empty());
    assert!(linked_known(&complete).enumeration_complete());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(files(&root), image);
}
