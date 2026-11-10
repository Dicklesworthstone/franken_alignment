//! Read the original acknowledged publication journal through a separately
//! retained supervisor grant. These public regressions use real file owners,
//! congress and human approval; a tail is neither replay nor effect authority.
#![cfg(unix)]

#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, replay::situation::FileSituationOrigin,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileJournalTailPage, FileJournalTailResult, FileSupervisorJournalTailGrant,
    FileSupervisorJournalTailPort,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorProposal, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolverPlane};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{AuthorityKind, Knowledge};
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerInstallError,
    TowerRequest, Verb, decode_request, encode_request,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use support::{Directory, Port, Supervisor, create, observe, oversight, profile, ready};

const REQUEST: u64 = 701;
const REGISTRATION: u64 = 41;
const PAYLOAD: &[u8] = b"PAYLOAD-SECRET";

fn tower(actor: Port) -> PublicationTower {
    PublicationTower::new(
        Address::new("1", Noun::Run, "3", Some(REGISTRATION)).unwrap(),
        actor,
        PublicationLimits::default(),
    )
    .unwrap()
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope {
            caller_tenant: "1".to_owned(),
            allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
            granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
        },
        operator,
        Vec::new(),
        0,
    )
}

fn capture_request(facade: &PublicationTower, after_revision: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "tail-original-journal",
        facade.mount_address().clone(),
        Verb::Tail,
    );
    request.arguments = ReadArguments::JournalTail { after_revision };
    request
}

fn page_request(facade: &PublicationTower, cursor: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        format!("tail-original-page-{cursor}"),
        facade.mount_address().clone(),
        Verb::Tail,
    );
    request.arguments = ReadArguments::JournalTailPage { cursor };
    request
}

fn exchange(facade: &mut PublicationTower, request: TowerRequest) -> PublicationResponse {
    let encoded = encode_request(&request).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), request);
    facade.exchange(&access(true), encoded.as_bytes())
}

fn capture(facade: &mut PublicationTower, after_revision: u64) -> PublicationResponse {
    let request = capture_request(facade, after_revision);
    exchange(facade, request)
}

fn continuation(facade: &mut PublicationTower, cursor: u64) -> PublicationResponse {
    let request = page_request(facade, cursor);
    exchange(facade, request)
}

fn native(response: &PublicationResponse) -> &FileJournalTailResult {
    let PublicationResult::JournalTail(linked) = &response.result else {
        panic!("expected native journal-tail observation: {response:?}");
    };
    linked.native()
}

fn page(response: &PublicationResponse) -> &FileJournalTailPage {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected original acknowledged journal page: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.scope, profile().delivery.scope);
    assert_eq!(basis.origin, FileSituationOrigin::AcknowledgedOwner);
    assert_eq!(basis.anchored_through, None);
    assert_eq!(response.envelope.reason_code, None);
    no_global_claims(response);
    value
}

fn cursor(response: &PublicationResponse) -> u64 {
    page(response).continuation().unwrap().id()
}

fn no_global_claims(response: &PublicationResponse) {
    fn unavailable<T>(knowledge: &Knowledge<T>) {
        assert!(matches!(
            knowledge,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    unavailable(&response.envelope.generation);
    unavailable(&response.envelope.control_seq);
    unavailable(&response.envelope.source_frontiers);
    unavailable(&response.envelope.budgets_remaining);
    unavailable(&response.envelope.realized_cost);
    unavailable(&response.envelope.affordances);
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
    no_global_claims(response);
}

fn conflict(response: &PublicationResponse) {
    assert_eq!(native(response), &Err(ActorError::IdempotencyConflict));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    no_global_claims(response);
}

fn unknown(response: &PublicationResponse) {
    assert_eq!(
        native(response),
        &Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::ControllerUnavailable
        }),
    );
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::LowerLayerUnverified)
    );
    no_global_claims(response);
}

fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    std::fs::read_dir(root.store())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn install(
    actor: Port,
    supervisor: &Supervisor,
    queries: usize,
    events_per_page: usize,
) -> (
    PublicationTower,
    FileSupervisorJournalTailPort,
    FileSupervisorJournalTailGrant,
) {
    let (port, grant) = supervisor
        .journal_tail_port(queries, events_per_page)
        .unwrap();
    let mut facade = tower(actor);
    facade.set_journal_tail(Some(port.clone())).unwrap();
    (facade, port, grant)
}

struct Fixture {
    root: Directory,
    actor: Port,
    supervisor: Supervisor,
    proposal: ActorProposal,
}

fn completed_publication() -> Fixture {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, mut proposal) = create(&root);
    proposal.payload = PAYLOAD.to_vec();
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, REQUEST);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.publish(revision, keys.automatic.attempt()).unwrap();
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().payload, PAYLOAD);
        // A later admission must target the original owner's current version.
        // Reusing the completed effect's old version would be a real refusal,
        // not evidence that Tail consumed the pending admission snapshot.
        proposal = support::proposal(&oversight::spec(&host, PAYLOAD));
    }
    Fixture {
        root,
        actor,
        supervisor,
        proposal,
    }
}

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value.get(name).unwrap_or_else(|| panic!("missing {name}"))
}

fn check_wire(response: &PublicationResponse) {
    let original = page(response);
    let encoded = response.to_json().unwrap();
    let document = strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
    let envelope = field(&document, "envelope").as_object().unwrap();
    assert_eq!(
        envelope.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect(),
    );
    let result = field(&document, "result");
    assert_eq!(field(result, "kind").as_str(), Some("journal-tail"));
    let body = field(result, "value");
    assert_eq!(
        field(body, "schema").as_str(),
        Some("fa.publication-journal-tail/1")
    );
    let knowledge = field(body, "knowledge");
    let basis = field(knowledge, "basis");
    assert_eq!(field(basis, "origin").as_str(), Some("acknowledged_owner"));
    for (name, expected) in [
        ("journal_revision", original.basis().journal_revision),
        ("control_sequence", original.basis().control_sequence),
    ] {
        assert_eq!(
            field(basis, name).as_str(),
            Some(expected.to_string().as_str())
        );
    }
    let value = field(knowledge, "value");
    for (name, expected) in [
        ("after_revision", original.after_revision()),
        ("through_revision", original.through_revision()),
    ] {
        assert_eq!(
            field(value, name).as_str(),
            Some(expected.to_string().as_str())
        );
    }
    assert_eq!(
        field(value, "enumeration_complete").as_bool(),
        Some(original.enumeration_complete())
    );
    match original.continuation() {
        Some(next) => assert_eq!(
            field(value, "continuation").as_str(),
            Some(next.id().to_string().as_str())
        ),
        None => assert!(field(value, "continuation").is_null()),
    }
    let rows = field(value, "events").as_array().unwrap();
    assert_eq!(rows.len(), original.events().len());
    for (row, event) in rows.iter().zip(original.events()) {
        assert_eq!(
            row.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["revision", "kind", "attempt", "request", "round"]),
        );
        assert_eq!(
            field(row, "revision").as_str(),
            Some(event.revision().to_string().as_str())
        );
        assert_eq!(field(row, "kind").as_str(), Some(event.kind().as_str()));
        for (name, identity) in [
            ("attempt", event.attempt()),
            ("request", event.request()),
            ("round", event.round()),
        ] {
            match identity {
                Some(identity) => assert_eq!(
                    field(row, name).as_str(),
                    Some(identity.to_string().as_str())
                ),
                None => assert!(field(row, name).is_null()),
            }
        }
    }
    // Both the typed projection and its wire representation must remain a
    // finite event-metadata view, not an accidental journal-byte disclosure.
    let debug = format!("{response:?}");
    for secret in [
        "PAYLOAD-SECRET",
        "PRIVATE-SOURCE-BYTES",
        "Approve this exact publication?",
        "original-alpha",
        "original-beta",
        "exact-view-file-v1",
    ] {
        let hex = secret
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(!encoded.contains(secret) && !encoded.contains(&hex));
        assert!(!debug.contains(secret) && !debug.contains(&hex));
    }
}

#[test]
fn original_publication_lifecycle_has_exact_ordered_metadata_without_private_content() {
    let mut f = completed_publication();
    let (mut facade, _port, grant) = install(f.actor.clone(), &f.supervisor, 3, 64);
    observe(&mut f.supervisor);
    let before = f.supervisor.host().unwrap().inspect();
    let image = files(&f.root);
    let response = capture(&mut facade, 0);
    let original = page(&response);
    let kinds = original
        .events()
        .iter()
        .map(|event| event.kind().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            "time",
            "submit_request",
            "inputs",
            "begin_review",
            "commit",
            "commit",
            "open_reveals",
            "reveal",
            "reveal",
            "finish_review",
            "authorize",
            "request_human",
            "human_decision",
            "dispatch",
            "publish",
            "reconcile",
        ]
    );
    assert_eq!(
        original
            .events()
            .iter()
            .map(|event| event.revision())
            .collect::<Vec<_>>(),
        (1..=before.revision).collect::<Vec<_>>()
    );
    assert_eq!(original.basis().journal_revision, before.revision);
    assert_eq!(original.basis().control_sequence, before.control.sequence);
    assert_ne!(before.revision, before.control.sequence);
    assert_eq!(original.after_revision(), 0);
    assert_eq!(original.through_revision(), before.revision);
    assert!(original.enumeration_complete());
    assert!(original.continuation().is_none());
    let submitted = &original.events()[1];
    assert_eq!(submitted.request(), Some(REQUEST));
    assert_eq!(
        submitted.attempt(),
        None,
        "submission records only the external key"
    );
    assert_eq!(original.events()[2].attempt(), Some(1));
    assert_eq!(original.events()[2].request(), None);
    assert_ne!(submitted.request(), original.events()[2].attempt());
    for event in original
        .events()
        .iter()
        .filter(|event| event.kind().as_str() == "request_human")
    {
        assert_eq!(
            event.request(),
            None,
            "human-request identity is not the actor's external key"
        );
        assert_eq!(event.attempt(), Some(1));
    }
    assert_eq!(original.events()[3].round(), Some(REQUEST + 100));
    check_wire(&response);
    assert_eq!(files(&f.root), image);
    assert_eq!(f.supervisor.host().unwrap().inspect(), before);
    assert_eq!(grant.remaining_queries(), 2);
    // The read did not consume or replace the original one-use admission view.
    let ticket = f.actor.submit(REQUEST + 1, &f.proposal).unwrap();
    assert_eq!(
        f.actor.poll(&ticket),
        ActorKnowledge::Pending {
            request: REQUEST + 1
        }
    );
}

#[test]
fn continuations_keep_one_capture_while_new_history_advances_the_original_owner() {
    let mut f = completed_publication();
    let (mut facade, _port, grant) = install(f.actor.clone(), &f.supervisor, 16, 3);
    let first = capture(&mut facade, 0);
    let basis = page(&first).basis();
    let initial_cut = basis.journal_revision;
    {
        let mut host = f.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    observe(&mut f.supervisor);
    let pending = f.actor.submit(REQUEST + 1, &f.proposal).unwrap();
    f.actor.cancel(&pending).unwrap();
    let current = f.supervisor.host().unwrap().inspect();
    assert!(current.revision > initial_cut);
    let image = files(&f.root);
    let mut responses = vec![first];
    while let Some(next) = page(responses.last().unwrap()).continuation() {
        let next = next.id();
        responses.push(continuation(&mut facade, next));
    }
    let mut revisions = Vec::new();
    for (index, response) in responses.iter().enumerate() {
        let original = page(response);
        assert_eq!(original.basis(), basis);
        assert_eq!(original.after_revision(), 0);
        assert_eq!(original.offset(), index * 3);
        assert_eq!(
            original.through_revision(),
            original.events().last().unwrap().revision()
        );
        revisions.extend(original.events().iter().map(|event| event.revision()));
        check_wire(response);
    }
    assert_eq!(revisions, (1..=initial_cut).collect::<Vec<_>>());
    assert!(page(responses.last().unwrap()).enumeration_complete());
    assert_eq!(
        page(responses.last().unwrap()).through_revision(),
        initial_cut
    );
    let fresh = capture(&mut facade, initial_cut);
    assert_eq!(page(&fresh).basis().journal_revision, current.revision);
    assert_eq!(page(&fresh).after_revision(), initial_cut);
    assert_eq!(
        page(&fresh)
            .events()
            .iter()
            .map(|event| event.kind().as_str())
            .collect::<Vec<_>>(),
        ["time", "submit_request", "cancel"]
    );
    assert!(page(&fresh).enumeration_complete());
    check_wire(&fresh);
    assert_eq!(grant.remaining_queries(), 16 - responses.len() - 1);
    assert_eq!(files(&f.root), image);
    assert_eq!(f.supervisor.host().unwrap().inspect(), current);
}

#[test]
fn an_unresolved_dispatch_stays_unknown_and_charged_through_every_tail_read() {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    let ticket = actor.submit(REQUEST, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, REQUEST);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Unknown {
            reason: ActorUnknown::OutcomeUnknown
        }
    );
    let before = supervisor.host().unwrap().inspect();
    assert_eq!(before.control.ledger.charged, 16);
    assert_eq!(before.executions, 0);
    let (mut facade, port, grant) = install(actor.clone(), &supervisor, 3, 64);
    let image = files(&root);
    let response = capture(&mut facade, 0);
    assert_eq!(
        page(&response).events().last().unwrap().kind().as_str(),
        "reconcile"
    );
    assert!(
        !page(&response)
            .events()
            .iter()
            .any(|event| event.kind().as_str() == "publish")
    );
    check_wire(&response);
    let observed = port.tail(before.revision).unwrap();
    assert!(
        matches!(observed, ActorKnowledge::Known { value, .. } if value.events().is_empty() && value.enumeration_complete())
    );
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Unknown {
            reason: ActorUnknown::OutcomeUnknown
        }
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(files(&root), image);
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn entry_requires_the_original_explicit_grant_and_privileged_registered_mount() {
    let f = completed_publication();
    let mut facade = tower(f.actor.clone());
    refused(&capture(&mut facade, 0), ReasonCode::CapabilityUnsupported);
    let (port, grant) = f.supervisor.journal_tail_port(2, 64).unwrap();
    facade.set_journal_tail(Some(port)).unwrap();
    let request = capture_request(&facade, 0);
    refused(
        &facade.dispatch(&access(false), request.clone()),
        ReasonCode::DeniedByPolicy,
    );
    let mut stale = request.clone();
    stale.address.generation = Some(REGISTRATION + 1);
    refused(
        &facade.dispatch(&access(true), stale),
        ReasonCode::StaleGeneration,
    );
    let mut other_request = request.clone();
    other_request.address = facade.request_address(REQUEST).unwrap();
    refused(
        &facade.dispatch(&access(true), other_request),
        ReasonCode::MissingRequiredObservation,
    );
    let mut other_tenant = request;
    other_tenant.address = Address::new("2", Noun::Run, "3", Some(REGISTRATION)).unwrap();
    let denied = facade.dispatch(&access(true), other_tenant);
    assert!(matches!(denied.result, PublicationResult::Refused(_)));
    assert_eq!(grant.remaining_queries(), 2);
    let image = files(&f.root);
    check_wire(&capture(&mut facade, 0));
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(files(&f.root), image);
}

#[test]
fn forged_retired_and_cross_facade_cursor_numbers_do_not_rebind_the_pending_capture() {
    let f = completed_publication();
    let (mut first, port, grant) = install(f.actor.clone(), &f.supervisor, 12, 2);
    let mut second = tower(f.actor);
    second.set_journal_tail(Some(port.clone())).unwrap();
    let image = files(&f.root);
    let initial = capture(&mut first, 0);
    let original = cursor(&initial);
    conflict(&continuation(&mut first, u64::MAX));
    conflict(&continuation(&mut second, original));
    assert_eq!(grant.remaining_queries(), 11);
    let continued = continuation(&mut first, original);
    assert_eq!(page(&continued).offset(), 2);
    conflict(&continuation(&mut first, original));
    assert_eq!(grant.remaining_queries(), 10);
    let replacement = capture(&mut second, 0);
    conflict(&continuation(&mut first, cursor(&continued)));
    assert_eq!(grant.remaining_queries(), 9);
    check_wire(&continuation(&mut second, cursor(&replacement)));
    assert_eq!(grant.remaining_queries(), 8);
    assert_eq!(files(&f.root), image);
}

#[test]
fn native_clone_advancement_and_replacement_preserve_one_shared_finite_allowance() {
    let f = completed_publication();
    let (mut facade, port, grant) = install(f.actor, &f.supervisor, 5, 2);
    let first = capture(&mut facade, 0);
    let old_cursor = page(&first).continuation().unwrap().clone();
    let clone = port.clone();
    let ActorKnowledge::Known {
        value: native_page,
        basis,
    } = clone.continue_tail(&old_cursor).unwrap()
    else {
        panic!("native clone lost the original capture");
    };
    assert_eq!(basis, page(&first).basis());
    assert_eq!(native_page.offset(), 2);
    assert_eq!(grant.remaining_queries(), 3);
    conflict(&continuation(&mut facade, old_cursor.id()));
    assert_eq!(
        port.continue_tail(&old_cursor),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 3);
    let ActorKnowledge::Known { value: third, .. } = port
        .continue_tail(native_page.continuation().unwrap())
        .unwrap()
    else {
        panic!("native continuation unavailable");
    };
    assert_eq!(third.offset(), 4);
    let replacement = capture(&mut facade, 8);
    assert_eq!(page(&replacement).after_revision(), 8);
    assert_eq!(
        port.continue_tail(third.continuation().unwrap()),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 1);
    let last = continuation(&mut facade, cursor(&replacement));
    assert!(!page(&last).enumeration_complete());
    assert!(page(&last).continuation().is_none());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(clone.tail(0), Err(ActorError::Capacity));
}

#[test]
fn exhausted_queries_report_truncation_without_a_false_complete_page_or_unusable_cursor() {
    let f = completed_publication();
    let (mut facade, port, grant) = install(f.actor.clone(), &f.supervisor, 2, 2);
    let image = files(&f.root);
    let first = capture(&mut facade, 0);
    let final_page = continuation(&mut facade, cursor(&first));
    assert_eq!(page(&final_page).events().len(), 2);
    assert_eq!(page(&final_page).through_revision(), 4);
    assert_eq!(
        page(&final_page).basis().journal_revision,
        f.supervisor.host().unwrap().revision()
    );
    assert!(!page(&final_page).enumeration_complete());
    assert!(page(&final_page).continuation().is_none());
    check_wire(&final_page);
    let exhausted = capture(&mut facade, 0);
    assert_eq!(native(&exhausted), &Err(ActorError::Capacity));
    assert_eq!(
        exhausted.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    let mut reconnected = tower(f.actor);
    reconnected.set_journal_tail(Some(port)).unwrap();
    assert_eq!(
        native(&capture(&mut reconnected, 0)),
        &Err(ActorError::Capacity)
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(files(&f.root), image);
}

#[test]
fn dropping_or_revoking_original_custody_withholds_even_a_previously_captured_page() {
    for revoke in [false, true] {
        let f = completed_publication();
        let (mut facade, port, grant) = install(f.actor, &f.supervisor, 3, 2);
        let first = capture(&mut facade, 0);
        let image = files(&f.root);
        if revoke {
            grant.revoke();
        } else {
            drop(grant);
        }
        for result in [
            continuation(&mut facade, cursor(&first)),
            capture(&mut facade, 0),
        ] {
            assert_eq!(
                native(&result),
                &Ok(ActorKnowledge::Withheld {
                    authority_required: "supervisor_journal_tail"
                })
            );
            assert_eq!(
                result.envelope.reason_code,
                Some(ReasonCode::DeniedByPolicy)
            );
            assert!(
                result
                    .to_json()
                    .unwrap()
                    .contains("supervisor_journal_tail")
            );
        }
        assert_eq!(
            port.tail(0),
            Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_journal_tail"
            })
        );
        assert_eq!(files(&f.root), image);
    }
}

#[test]
fn same_numeric_scope_does_not_supply_original_owner_or_replacement_custody() {
    let first = completed_publication();
    let second = completed_publication();
    let (mut facade, _port, grant) = install(first.actor.clone(), &first.supervisor, 3, 2);
    let captured = capture(&mut facade, 0);
    let (foreign, foreign_grant) = second.supervisor.journal_tail_port(3, 2).unwrap();
    assert_eq!(
        facade.set_journal_tail(Some(foreign)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    refused(
        &continuation(&mut facade, cursor(&captured)),
        ReasonCode::CapabilityUnsupported,
    );
    refused(&capture(&mut facade, 0), ReasonCode::CapabilityUnsupported);
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(foreign_grant.remaining_queries(), 3);
    let (replacement, replacement_grant) = first.supervisor.journal_tail_port(3, 2).unwrap();
    facade.set_journal_tail(Some(replacement)).unwrap();
    let fresh = capture(&mut facade, 0);
    assert_ne!(cursor(&fresh), cursor(&captured));
    conflict(&continuation(&mut facade, cursor(&captured)));
    check_wire(&continuation(&mut facade, cursor(&fresh)));
    assert_eq!(replacement_grant.remaining_queries(), 1);
}

#[test]
fn owner_borrow_fault_loss_and_reopening_cannot_turn_cached_metadata_into_authority() {
    let mut f = completed_publication();
    let (mut facade, port, grant) = install(f.actor, &f.supervisor, 6, 2);
    let first = capture(&mut facade, 0);
    {
        let _host = f.supervisor.host_mut().unwrap();
        unknown(&continuation(&mut facade, cursor(&first)));
    }
    conflict(&continuation(&mut facade, cursor(&first)));
    assert_eq!(grant.remaining_queries(), 5);
    let second = capture(&mut facade, 0);
    drop(f.supervisor);
    let image = files(&f.root);
    unknown(&continuation(&mut facade, cursor(&second)));
    assert_eq!(grant.remaining_queries(), 4);
    assert_eq!(files(&f.root), image);
    let (host, _reviewer) = FileOversight::open(f.root.store(), profile()).unwrap();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let mut reopened = tower(actor);
    assert_eq!(
        reopened.set_journal_tail(Some(port)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    let (fresh_port, fresh_grant) = supervisor.journal_tail_port(3, 2).unwrap();
    reopened.set_journal_tail(Some(fresh_port)).unwrap();
    let recovered = capture(&mut reopened, 0);
    assert!(page(&recovered).basis().journal_revision > page(&second).basis().journal_revision);
    assert_ne!(cursor(&recovered), cursor(&second));
    conflict(&continuation(&mut reopened, cursor(&second)));
    {
        let mut host = supervisor.host_mut().unwrap();
        std::fs::write(
            f.root.store().join("delivery.pending"),
            b"occupied canonical stage",
        )
        .unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let faulted_image = files(&f.root);
    unknown(&continuation(&mut reopened, cursor(&recovered)));
    unknown(&capture(&mut reopened, 0));
    assert_eq!(fresh_grant.remaining_queries(), 2);
    assert_eq!(files(&f.root), faulted_image);
}

#[test]
fn empty_exact_boundary_and_future_starts_distinguish_closed_history_from_unknown() {
    let root = Directory::new();
    let (host, _reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    assert_eq!(host.revision(), 0);
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (mut facade, _port, grant) = install(actor, &supervisor, 5, 2);
    let empty = capture(&mut facade, 0);
    assert!(page(&empty).events().is_empty());
    assert!(page(&empty).enumeration_complete());
    assert_eq!(page(&empty).through_revision(), 0);
    assert!(page(&empty).continuation().is_none());
    check_wire(&empty);
    {
        let mut host = supervisor.host_mut().unwrap();
        for tick in [1, 2] {
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(tick)).unwrap();
        }
    }
    let exact = capture(&mut facade, 0);
    assert_eq!(page(&exact).events().len(), 2);
    assert!(page(&exact).enumeration_complete());
    assert!(page(&exact).continuation().is_none());
    let at_cut = capture(&mut facade, 2);
    assert!(page(&at_cut).events().is_empty());
    assert!(page(&at_cut).enumeration_complete());
    assert_eq!(page(&at_cut).through_revision(), 2);
    unknown(&capture(&mut facade, 3));
    assert_eq!(grant.remaining_queries(), 1);
    check_wire(&capture(&mut facade, 1));
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn malformed_wire_inputs_and_retired_snapshot_arguments_never_spend_a_native_query() {
    let f = completed_publication();
    let (mut facade, _port, grant) = install(f.actor, &f.supervisor, 2, 2);
    let first = capture(&mut facade, 0);
    let pending = cursor(&first);
    let valid = encode_request(&capture_request(&facade, 0)).unwrap();
    for malformed in [
        valid.replace("\"after_revision\":\"0\"", "\"after_revision\":0"),
        valid.replace("\"after_revision\":\"0\"", "\"after_revision\":\"00\""),
        valid.replace(
            "\"after_revision\":\"0\"",
            "\"after_revision\":\"0\",\"cursor\":\"1\"",
        ),
        valid.replace(
            "\"after_revision\":\"0\"",
            "\"after_revision\":\"0\",\"max_events\":\"64\"",
        ),
        valid.replace("\"after_revision\":\"0\"", "\"cursor\":\"0\""),
    ] {
        assert_ne!(malformed, valid, "test must actually change the request");
        assert!(decode_request(malformed.as_bytes()).is_err());
        let result = facade.exchange(&access(true), malformed.as_bytes());
        assert!(matches!(result.result, PublicationResult::Refused(_)));
    }
    let mut snapshot = capture_request(&facade, 0);
    snapshot.arguments = ReadArguments::Tail {
        since_seq: 0,
        max_events: 2,
    };
    refused(
        &facade.dispatch(&access(true), snapshot),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 1);
    let complete = continuation(&mut facade, pending);
    assert_eq!(page(&complete).offset(), 2);
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn sealed_results_reject_relabelled_registration_and_output_limits_do_not_renew_work() {
    let f = completed_publication();
    let (mut facade, _port, grant) = install(f.actor, &f.supervisor, 2, 2);
    let original = capture(&mut facade, 0);
    let PublicationResult::JournalTail(linked) = &original.result else {
        panic!("tail result");
    };
    assert_eq!(linked.registration(), facade.mount_address());
    assert_eq!(linked.query_address(), &original.envelope.address);
    check_wire(&original);
    for mutation in 0..3 {
        let mut altered = original.clone();
        match mutation {
            0 => altered.envelope.address.generation = None,
            1 => altered.envelope.address.id.push('9'),
            _ => altered.envelope.address.tenant = "2".to_owned(),
        }
        assert_eq!(altered.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    }
    let mut tiny = original.clone();
    tiny.envelope.limits.max_response_bytes = 1;
    assert_eq!(tiny.to_json(), Err(TowerEncodingError::OutputLimit));
    assert_eq!(grant.remaining_queries(), 1);
    check_wire(&continuation(&mut facade, cursor(&original)));
    assert_eq!(grant.remaining_queries(), 0);
}
