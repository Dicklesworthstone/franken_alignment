//! Discover original request/review relationships before paying for a complete
//! decision explanation. These controls execute real native publication and
//! learned refinement; recorded links alone are not replay or effect authority.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/learned_refinement_explanation.rs"]
mod learned;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileRequestHistory, FileRequestHistoryDisclosure, FileRequestHistoryResult,
    FileRequestReviewCompletion, FileSupervisorRequestHistoryGrant, MAX_REVIEW_EXPLANATION_EVENTS,
};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::Knowledge;
use fa_reference::round::Verdict;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerInstallError,
    TowerRequest, Verb, decode_request, encode_request,
};
use std::collections::BTreeSet;
use support::{Directory, Port, Supervisor, create, observe, oversight, profile, ready, snapshot};

const REQUEST: u64 = 701;
const ROUND: u64 = REQUEST + 100;
const REGISTRATION: u64 = 41;

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(REGISTRATION)).unwrap()
}

fn tower(actor: Port) -> PublicationTower {
    PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap()
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        Vec::new(),
        0,
    )
}

fn query(facade: &PublicationTower, request: u64) -> TowerRequest {
    TowerRequest::new(
        "discover-original-request-reviews",
        facade.request_address(request).unwrap(),
        Verb::Related,
    )
}

fn discover(facade: &mut PublicationTower, request: u64) -> PublicationResponse {
    let query = query(facade, request);
    let text = encode_request(&query).unwrap();
    assert_eq!(decode_request(text.as_bytes()).unwrap(), query);
    facade.exchange(&access(true), text.as_bytes())
}

fn install(
    actor: Port,
    supervisor: &Supervisor,
    queries: usize,
    max_history_events: usize,
    max_reviews: usize,
) -> (PublicationTower, FileSupervisorRequestHistoryGrant) {
    let (port, grant) = supervisor
        .request_history_port(
            queries,
            max_history_events,
            max_reviews,
            FileRequestHistoryDisclosure::RecordedRequestRelationships,
        )
        .unwrap();
    let mut facade = tower(actor);
    facade.set_request_history(Some(port)).unwrap();
    (facade, grant)
}

fn native(response: &PublicationResponse) -> &FileRequestHistoryResult {
    let PublicationResult::RequestHistory(linked) = &response.result else {
        panic!("expected original request relationship observation: {response:?}");
    };
    no_global_claims(response);
    linked.native()
}

fn known(response: &PublicationResponse) -> &FileRequestHistory {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected Known original request relationships: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.scope, profile().delivery.scope);
    assert_eq!(response.envelope.reason_code, None);
    value
}

fn unavailable(response: &PublicationResponse) {
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
}

fn capacity(response: &PublicationResponse) {
    assert_eq!(native(response), &Err(ActorError::Capacity));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
    no_global_claims(response);
}

fn no_global_claims(response: &PublicationResponse) {
    fn unobserved<T>(value: &Knowledge<T>) {
        assert!(matches!(
            value,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    unobserved(&response.envelope.generation);
    unobserved(&response.envelope.control_seq);
    unobserved(&response.envelope.source_frontiers);
    unobserved(&response.envelope.budgets_remaining);
    unobserved(&response.envelope.realized_cost);
    unobserved(&response.envelope.affordances);
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn completed(root: &Directory) -> (Port, Supervisor, oversight::Keys) {
    let (actor, mut supervisor, reviewer, mut proposal) = create(root);
    proposal.payload = b"PRIVATE-REQUEST".to_vec();
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let keys = ready(&mut supervisor, &reviewer, REQUEST);
    (actor, supervisor, keys)
}

fn begin(supervisor: &mut Supervisor, request: u64, round: u64) -> CommitteeInput {
    let mut host = supervisor.host_mut().unwrap();
    let attempt = support::attempt(&host, request);
    let action = host.request_action(request).unwrap().clone();
    let inputs = oversight::inputs(&action, b"PRIVATE-HISTORY-SOURCE");
    let revision = host.revision();
    let input_revision = host.input_revision(attempt).unwrap();
    host.record_inputs(revision, attempt, input_revision, inputs.clone())
        .unwrap();
    let revision = host.revision();
    let window = oversight::window(&host);
    host.begin_review(
        revision,
        attempt,
        round,
        oversight::ROOT,
        window,
        snapshot(),
    )
    .unwrap();
    inputs
}

fn finish(supervisor: &mut Supervisor, round: u64, inputs: &CommitteeInput) {
    let mut host = supervisor.host_mut().unwrap();
    oversight::votes(&mut host, round, Verdict::Allow);
    let revision = host.revision();
    host.finish_review(revision, round, Some(inputs), snapshot())
        .unwrap()
        .unwrap();
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, name| {
        value
            .get(name)
            .unwrap_or_else(|| panic!("missing field {name}"))
    })
}

fn document(response: &PublicationResponse) -> Json {
    strict_json::parse(
        response.to_json().unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap()
}

#[test]
fn admitted_empty_history_is_distinct_from_missing_request_and_never_mutates_owner() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let (mut facade, grant) = install(actor, &supervisor, 2, 1024, 8);
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);

    let response = discover(&mut facade, REQUEST);
    let history = known(&response);
    assert!(history.admitted());
    assert_eq!(
        history.attempt(),
        Some(support::attempt(&supervisor.host().unwrap(), REQUEST))
    );
    assert_ne!(history.attempt(), Some(REQUEST));
    assert_eq!(history.basis().request, REQUEST);
    assert_eq!(
        history.basis().request_generation,
        supervisor
            .host()
            .unwrap()
            .request_status(REQUEST)
            .unwrap()
            .generation
    );
    assert_eq!(history.basis().journal_revision, before.revision);
    assert_eq!(history.basis().control_sequence, before.control.sequence);
    assert!(history.reviews().is_empty());
    assert_eq!(history.incomplete_reviews(), 0);
    unavailable(&discover(&mut facade, REQUEST + 1));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn recorded_completion_link_does_not_turn_a_stale_application_into_approval() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let inputs = begin(&mut supervisor, REQUEST, ROUND);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::votes(&mut host, ROUND, Verdict::Allow);
        let changed = oversight::inputs(inputs.action(), b"different retained helper input");
        let revision = host.revision();
        assert_eq!(
            host.finish_review(revision, ROUND, Some(&changed), snapshot())
                .unwrap(),
            Err(fa_reference::Error::Stale)
        );
    }
    let (mut facade, history_grant) = install(actor, &supervisor, 1, 1024, 8);
    let (review, review_grant) = supervisor.review_explanation_port(1).unwrap();
    facade.set_review_explanation(Some(review)).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let response = discover(&mut facade, REQUEST);
    let history = known(&response);
    assert_eq!(history.reviews().len(), 1);
    assert_eq!(
        history.reviews()[0].completion_kind(),
        FileRequestReviewCompletion::ApplicationAttempted
    );
    let PublicationResult::RequestHistory(linked) = &response.result else {
        unreachable!()
    };
    let explanation = facade.dispatch(&access(true), linked.explain_requests().unwrap().remove(0));
    let PublicationResult::ReviewExplanation(linked) = &explanation.result else {
        unreachable!()
    };
    let Ok(ActorKnowledge::Known { value, .. }) = linked.native() else {
        unreachable!()
    };
    assert_eq!(value.application(), Err(fa_reference::Error::Stale));
    assert!(value.effect().dispatch().is_none());
    assert_eq!(history_grant.remaining_queries(), 0);
    assert_eq!(review_grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn completed_links_exclude_other_requests_and_pending_rounds_and_keep_original_cuts() {
    let root = Directory::new();
    let (actor, mut supervisor, _keys) = completed(&root);
    let original = supervisor.host().unwrap().review_replay(ROUND).unwrap();
    observe(&mut supervisor);
    let proposal = support::proposal(&oversight::spec(&supervisor.host().unwrap(), b"other"));
    actor.submit(REQUEST + 1, &proposal).unwrap();
    let other = begin(&mut supervisor, REQUEST + 1, ROUND + 1);
    finish(&mut supervisor, ROUND + 1, &other);
    let _pending = begin(&mut supervisor, REQUEST, ROUND + 2);
    let (mut facade, grant) = install(actor, &supervisor, 2, 1024, 8);
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);

    let response = discover(&mut facade, REQUEST);
    let history = known(&response);
    assert_eq!(history.reviews().len(), 1);
    let link = &history.reviews()[0];
    assert_eq!(link.round(), ROUND);
    assert_eq!(link.begin_revision(), original.begin_revision());
    assert_eq!(
        link.completed_revision(),
        original.completed_snapshot().revision
    );
    assert_eq!(
        link.completion_kind(),
        FileRequestReviewCompletion::ApplicationAttempted
    );
    assert_eq!(history.incomplete_reviews(), 1);
    assert!(link.begin_revision() < link.completed_revision());
    assert!(link.completed_revision() < history.basis().journal_revision);
    let other = discover(&mut facade, REQUEST + 1);
    assert_eq!(known(&other).reviews().len(), 1);
    assert_eq!(known(&other).reviews()[0].round(), ROUND + 1);
    assert_ne!(known(&other).attempt(), history.attempt());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn discovery_actions_reach_the_original_completed_review_and_later_effect() {
    let root = Directory::new();
    let (actor, mut supervisor, keys) = completed(&root);
    let (mut facade, grant) = install(actor, &supervisor, 2, 1024, 8);
    let (explanation, explanation_grant) = supervisor.review_explanation_port(2).unwrap();
    facade.set_review_explanation(Some(explanation)).unwrap();
    let first = discover(&mut facade, REQUEST);
    let first_history = known(&first).clone();
    let PublicationResult::RequestHistory(linked) = &first.result else {
        unreachable!()
    };
    let actions = linked.explain_requests().unwrap();
    assert_eq!(actions.len(), 1);
    let action = &actions[0];
    assert_eq!(action.verb, Verb::Explain);
    assert_eq!(action.address, facade.request_address(REQUEST).unwrap());
    assert_eq!(action.address.generation, Some(REGISTRATION));
    assert_eq!(
        action.arguments,
        ReadArguments::ReviewExplanation { round: ROUND }
    );
    assert_eq!(explanation_grant.remaining_queries(), 2);

    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let explained = facade.exchange(&access(true), encode_request(action).unwrap().as_bytes());
    let PublicationResult::ReviewExplanation(review) = &explained.result else {
        panic!("discovered action must select original review: {explained:?}");
    };
    let Ok(ActorKnowledge::Known { value, basis }) = review.native() else {
        panic!("original discovered review must be replayable");
    };
    assert_eq!(basis.round, first_history.reviews()[0].round());
    assert_eq!(
        basis.begin_revision,
        first_history.reviews()[0].begin_revision()
    );
    assert_eq!(
        basis.completed_revision,
        first_history.reviews()[0].completed_revision()
    );
    assert_eq!(value.ending_state(), ActionState::Authorized);
    assert!(value.effect().dispatch().is_none());
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(explanation_grant.remaining_queries(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);

    // Neither discovery nor explanation consumes the real two approval keys.
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.publish(revision, keys.automatic.attempt()).unwrap();
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let published = supervisor.host().unwrap().inspect();
    let response = discover(&mut facade, REQUEST);
    let later = known(&response);
    assert_eq!(later.reviews(), first_history.reviews());
    assert_eq!(later.stage(), Some(ActionState::Confirmed));
    assert!(later.basis().journal_revision > first_history.basis().journal_revision);
    assert!(later.basis().request_generation > first_history.basis().request_generation);
    assert_eq!(first_history.stage(), Some(ActionState::Authorized));
    assert_eq!(published.executions, 1);
    let explained = facade.dispatch(&access(true), action.clone());
    let PublicationResult::ReviewExplanation(review) = &explained.result else {
        unreachable!()
    };
    let Ok(ActorKnowledge::Known { value, .. }) = review.native() else {
        unreachable!()
    };
    assert!(value.effect().selected_review_dispatched());
    assert_eq!(value.ending_state(), ActionState::Confirmed);
    assert_eq!(supervisor.host().unwrap().inspect(), published);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(explanation_grant.remaining_queries(), 0);
}

#[test]
fn strict_scope_generation_and_arguments_precede_separate_discovery_work() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let mut facade = tower(actor);
    let (review, review_grant) = supervisor.review_explanation_port(1).unwrap();
    facade.set_review_explanation(Some(review)).unwrap();
    refused(
        &discover(&mut facade, REQUEST),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(review_grant.remaining_queries(), 1);
    let (port, grant) = supervisor
        .request_history_port(
            2,
            1024,
            8,
            FileRequestHistoryDisclosure::RecordedRequestRelationships,
        )
        .unwrap();
    facade.set_request_history(Some(port)).unwrap();
    let original = query(&facade, REQUEST);
    refused(
        &facade.dispatch(&access(false), original.clone()),
        ReasonCode::DeniedByPolicy,
    );
    let mut unauthorized = CallerScope::full_tenant_access("1");
    unauthorized.granted_authorities.clear();
    refused(
        &facade.dispatch(
            &TowerAccess::new(unauthorized, true, Vec::new(), 0),
            original.clone(),
        ),
        ReasonCode::DeniedByPolicy,
    );
    let mut stale = original.clone();
    stale.address.generation = Some(REGISTRATION - 1);
    refused(
        &facade.dispatch(&access(true), stale),
        ReasonCode::StaleGeneration,
    );
    let mut foreign = original.clone();
    foreign.address.tenant = "2".into();
    refused(
        &facade.dispatch(&access(true), foreign),
        ReasonCode::DeniedByPolicy,
    );
    let mut wrong_scope = original.clone();
    wrong_scope.address.id = format!("request-3-2-99-5-{REQUEST}");
    refused(
        &facade.dispatch(&access(true), wrong_scope),
        ReasonCode::MissingRequiredObservation,
    );
    let mut run = original.clone();
    run.address = facade.mount_address().clone();
    refused(
        &facade.dispatch(&access(true), run),
        ReasonCode::MissingRequiredObservation,
    );
    let mut wrong_arguments = original.clone();
    wrong_arguments.arguments = ReadArguments::ReviewExplanation { round: ROUND };
    refused(
        &facade.dispatch(&access(true), wrong_arguments),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(review_grant.remaining_queries(), 1);

    // Generationless discovery may observe the current registration, but every
    // proposed explanation is pinned to the exact registered owner generation.
    let mut current = original;
    current.address.generation = None;
    let response = facade.dispatch(&access(true), current);
    known(&response);
    let PublicationResult::RequestHistory(linked) = &response.result else {
        unreachable!()
    };
    assert_eq!(linked.registration(), &mount());
    assert_eq!(linked.query_address().generation, None);
    assert!(
        linked
            .explain_requests()
            .unwrap()
            .iter()
            .all(|action| action.address.generation == Some(REGISTRATION))
    );
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn complete_scan_and_review_caps_refuse_without_truncating_a_successful_enumeration() {
    let root = Directory::new();
    let (actor, mut supervisor, _keys) = completed(&root);
    let inputs = begin(&mut supervisor, REQUEST, ROUND + 1);
    finish(&mut supervisor, ROUND + 1, &inputs);
    let before = supervisor.host().unwrap().inspect();
    let events = before.revision as usize;
    let bytes = image(&root);
    let (mut short, short_grant) = install(actor.clone(), &supervisor, 1, events - 1, 2);
    let (mut few, few_grant) = install(actor.clone(), &supervisor, 1, events, 1);
    let (mut complete, complete_grant) = install(actor, &supervisor, 1, events, 2);
    capacity(&discover(&mut short, REQUEST));
    capacity(&discover(&mut few, REQUEST));
    let response = discover(&mut complete, REQUEST);
    assert_eq!(
        known(&response)
            .reviews()
            .iter()
            .map(|review| review.round())
            .collect::<Vec<_>>(),
        [ROUND, ROUND + 1]
    );
    assert_eq!(known(&response).incomplete_reviews(), 0);
    assert_eq!(
        field(
            &document(&response),
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "enumeration_complete"
            ]
        )
        .as_bool(),
        Some(true)
    );
    assert_eq!(short_grant.remaining_queries(), 0);
    assert_eq!(few_grant.remaining_queries(), 0);
    assert_eq!(complete_grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn shared_grants_revoke_across_facades_and_cannot_move_to_equal_scope_owners() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let (port, grant) = supervisor
        .request_history_port(
            3,
            1024,
            8,
            FileRequestHistoryDisclosure::RecordedRequestRelationships,
        )
        .unwrap();
    let mut first = tower(actor.clone());
    let mut second = tower(actor);
    first.set_request_history(Some(port.clone())).unwrap();
    second.set_request_history(Some(port.clone())).unwrap();
    let original = discover(&mut first, REQUEST);
    assert_eq!(known(&original), known(&discover(&mut second, REQUEST)));
    assert_eq!(grant.remaining_queries(), 1);

    let other_root = Directory::new();
    let (_actor, other_supervisor, _keys) = completed(&other_root);
    let (other, other_grant) = other_supervisor
        .request_history_port(
            1,
            1024,
            8,
            FileRequestHistoryDisclosure::RecordedRequestRelationships,
        )
        .unwrap();
    assert_eq!(
        first.set_request_history(Some(other)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    refused(
        &discover(&mut first, REQUEST),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(other_grant.remaining_queries(), 1);
    first.set_request_history(Some(port)).unwrap();
    known(&discover(&mut first, REQUEST));
    assert_eq!(grant.remaining_queries(), 0);
    capacity(&discover(&mut second, REQUEST));
    grant.revoke();
    for facade in [&mut first, &mut second] {
        let response = discover(facade, REQUEST);
        assert_eq!(
            native(&response),
            &Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_request_history"
            })
        );
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
    }
    // Keeping a captured response and weak facade is not custody of the writer.
    drop(supervisor);
    let (_reopened, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(known(&original).reviews()[0].round(), ROUND);
}

#[test]
fn owner_loss_and_conflicting_borrow_never_return_a_cached_history() {
    let root = Directory::new();
    let (actor, mut supervisor, _keys) = completed(&root);
    let (mut facade, grant) = install(actor, &supervisor, 2, 1024, 8);
    let original = discover(&mut facade, REQUEST);
    let borrow = supervisor.host_mut().unwrap();
    unavailable(&discover(&mut facade, REQUEST));
    assert_eq!(grant.remaining_queries(), 1);
    drop(borrow);
    drop(supervisor);
    unavailable(&discover(&mut facade, REQUEST));
    assert_eq!(grant.remaining_queries(), 1);
    let (_reopened, _reviewer) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(known(&original).reviews()[0].round(), ROUND);
}

#[test]
fn retained_not_admitted_request_never_borrows_another_attempts_reviews() {
    let root = Directory::new();
    let (actor, mut supervisor, _keys) = completed(&root);
    let mut refused_proposal =
        support::proposal(&oversight::spec(&supervisor.host().unwrap(), b"declined"));
    refused_proposal.expected_policy_epoch += 1;
    observe(&mut supervisor);
    actor.submit(REQUEST + 1, &refused_proposal).unwrap();
    let (mut facade, grant) = install(actor, &supervisor, 1, 1024, 8);
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let response = discover(&mut facade, REQUEST + 1);
    let history = known(&response);
    assert!(!history.admitted());
    assert_eq!(history.attempt(), None);
    assert_eq!(history.stage(), None);
    assert!(history.reviews().is_empty());
    assert_eq!(history.incomplete_reviews(), 0);
    let PublicationResult::RequestHistory(linked) = &response.result else {
        unreachable!()
    };
    assert!(linked.explain_requests().unwrap().is_empty());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
}

#[test]
fn bounded_wire_exposes_only_recorded_links_and_seals_original_request_correlation() {
    let root = Directory::new();
    let (actor, supervisor, _keys) = completed(&root);
    let (mut facade, grant) = install(actor, &supervisor, 2, 1024, 8);
    let mut original_query = query(&facade, REQUEST);
    original_query.idempotency_key = Some("explicit-historical-observation".into());
    let response = facade.dispatch(&access(true), original_query);
    known(&response);
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let encoded = response.to_json().unwrap();
    for secret in [
        "PRIVATE-REQUEST",
        "PRIVATE-SOURCE-BYTES",
        "original-alpha",
        "original-beta",
    ] {
        assert!(!encoded.contains(secret));
        let hex = secret
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(!encoded.contains(&hex));
    }
    let wire = document(&response);
    assert_eq!(
        field(&wire, &["envelope"])
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect()
    );
    assert_eq!(
        field(&wire, &["result", "kind"]).as_str(),
        Some("request-history")
    );
    let outer = field(&wire, &["result", "value"]);
    assert_eq!(
        field(outer, &["schema"]).as_str(),
        Some("fa.publication-request-history/1")
    );
    assert_eq!(field(outer, &["operation"]).as_str(), Some("related"));
    assert_eq!(field(outer, &["historical"]).as_bool(), Some(true));
    assert_eq!(
        field(outer, &["current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!(
        field(outer, &["disclosure"]).as_str(),
        Some("recorded_request_relationships")
    );
    let body = field(outer, &["knowledge", "value"]);
    assert_eq!(field(body, &["enumeration_complete"]).as_bool(), Some(true));
    let rows = field(body, &["reviews"]).as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        field(&rows[0], &["completion_kind"]).as_str(),
        Some("application_attempted")
    );
    let PublicationResult::RequestHistory(linked) = &response.result else {
        unreachable!()
    };
    let action = linked.explain_requests().unwrap().remove(0);
    let expected = strict_json::parse(
        encode_request(&action).unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap();
    assert_eq!(field(&rows[0], &["explain_request"]), &expected);

    for field in 0..5 {
        let mut changed = response.clone();
        match field {
            0 => changed.envelope.operation_id.push_str("-different"),
            1 => changed.envelope.idempotency_key = Some("another".into()),
            2 => changed.envelope.address.generation = Some(REGISTRATION + 1),
            3 => changed.envelope.address.id = facade.request_address(REQUEST + 1).unwrap().id,
            4 => changed.envelope.address.tenant = "2".into(),
            _ => unreachable!(),
        }
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    }
    let mut short = response.clone();
    short.envelope.limits.max_response_bytes = 1;
    assert_eq!(short.to_json(), Err(TowerEncodingError::OutputLimit));
    assert_eq!(response.to_json().unwrap(), encoded);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);

    // Unknowns have no basis to authenticate them, so their outer correlation
    // must remain sealed too; neither can be moved to an existing request.
    let unknown = discover(&mut facade, REQUEST + 1);
    unavailable(&unknown);
    let mut changed = unknown.clone();
    changed.envelope.address = facade.request_address(REQUEST).unwrap();
    assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn actual_learned_refinement_discovery_preserves_numerical_work_and_reaches_fresh_review() {
    let root = learned::Directory::new();
    let mut state = learned::prepared(&root);
    let (history, history_grant) = state
        .supervisor
        .request_history_port(
            2,
            4096,
            8,
            FileRequestHistoryDisclosure::RecordedRequestRelationships,
        )
        .unwrap();
    let (reviews, review_grant) = state
        .supervisor
        .review_explanation_port_with_numerical_budget(
            2,
            MAX_REVIEW_EXPLANATION_EVENTS,
            MAX_REVIEW_EXPLANATION_EVENTS * 2,
        )
        .unwrap();
    let mut facade = PublicationTower::new_learned_observer(
        mount(),
        state.actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    facade.set_request_history(Some(history)).unwrap();
    facade.set_review_explanation(Some(reviews)).unwrap();
    let before = state.supervisor.host().unwrap().inspect();
    let numerical = state
        .supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let bytes = learned::image(&root);

    let response = discover(&mut facade, learned::REQUEST);
    let value = known(&response);
    assert_eq!(value.reviews().len(), 1);
    assert_eq!(value.reviews()[0].round(), learned::REFINEMENT_ROUND);
    assert_eq!(
        value.reviews()[0].completion_kind(),
        FileRequestReviewCompletion::EvidenceRefined
    );
    assert_eq!(
        value.reviews()[0].completed_revision(),
        state.refinement_revision
    );
    assert_eq!(value.stage(), Some(ActionState::Reviewing));
    assert_eq!(review_grant.remaining_queries(), 2);
    assert_eq!(
        review_grant.remaining_numerical_replay_events(),
        Some(MAX_REVIEW_EXPLANATION_EVENTS * 2)
    );
    assert_eq!(state.supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        state
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(learned::image(&root), bytes);

    let PublicationResult::RequestHistory(linked) = &response.result else {
        unreachable!()
    };
    let action = linked.explain_requests().unwrap().remove(0);
    let explanation = facade.dispatch(&access(true), action);
    let PublicationResult::RefinementExplanation(refinement) = &explanation.result else {
        panic!("discovered refinement must reach original numerical replay: {explanation:?}");
    };
    let Ok(ActorKnowledge::Known { value, .. }) = refinement.native() else {
        unreachable!()
    };
    assert_eq!(value.group(), learned::group());
    assert_eq!(value.basis().round, learned::REFINEMENT_ROUND);
    assert_eq!(review_grant.remaining_queries(), 1);
    assert_eq!(
        review_grant.remaining_numerical_replay_events(),
        Some(MAX_REVIEW_EXPLANATION_EVENTS * 2 - before.revision as usize)
    );

    let applied_revision = learned::publish(&mut state);
    let published = state.supervisor.host().unwrap().inspect();
    let numerical = state
        .supervisor
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap();
    let bytes = learned::image(&root);
    let response = discover(&mut facade, learned::REQUEST);
    let value = known(&response);
    assert_eq!(value.reviews().len(), 2);
    assert_eq!(value.reviews()[0].round(), learned::REFINEMENT_ROUND);
    assert_eq!(
        value.reviews()[0].completion_kind(),
        FileRequestReviewCompletion::EvidenceRefined
    );
    assert_eq!(value.reviews()[1].round(), learned::APPLIED_ROUND);
    assert_eq!(
        value.reviews()[1].completion_kind(),
        FileRequestReviewCompletion::ApplicationAttempted
    );
    assert_eq!(value.reviews()[1].completed_revision(), applied_revision);
    assert_eq!(value.stage(), Some(ActionState::Confirmed));
    let PublicationResult::RequestHistory(linked) = &response.result else {
        unreachable!()
    };
    let action = linked.explain_requests().unwrap().remove(1);
    let explanation = facade.dispatch(&access(true), action);
    let PublicationResult::ReviewExplanation(applied) = &explanation.result else {
        unreachable!()
    };
    let Ok(ActorKnowledge::Known { value, .. }) = applied.native() else {
        unreachable!()
    };
    assert_eq!(value.basis().round, learned::APPLIED_ROUND);
    assert!(value.effect().selected_review_dispatched());
    assert_eq!(value.application().unwrap().input_revision, 2);
    assert_eq!(review_grant.remaining_queries(), 0);
    assert_eq!(history_grant.remaining_queries(), 0);
    assert_eq!(state.supervisor.host().unwrap().inspect(), published);
    assert_eq!(
        state
            .supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(learned::image(&root), bytes);
}
