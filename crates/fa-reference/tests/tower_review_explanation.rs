//! Completed original congress decisions, actual applications and effect linkage.
//! Ballots are controlled test inputs; no helper independence or deployment claim.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "tower_review_explanation/boundaries.rs"]
mod boundaries;
#[path = "support/file_oversight_actor.rs"]
mod support;
#[path = "tower_review_explanation/wire.rs"]
mod wire;

use fa_reference::Error;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileReviewExplanation, FileReviewExplanationResult, MAX_REVIEW_EXPLANATION_EVENTS,
};
use fa_reference::action::consequence::gate::containment::session::policy::{
    Policy, Predicate, Truth,
};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::consequence::{Consequence, Rule};
use fa_reference::action::{ActionState, ElapsedTick};
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
const PRIVATE_PAYLOAD: &[u8] = b"PRIVATE-REVIEW";

fn tower(actor: Port) -> PublicationTower {
    PublicationTower::new(
        Address::new("1", Noun::Run, "3", Some(41)).unwrap(),
        actor,
        PublicationLimits::default(),
    )
    .unwrap()
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        Vec::new(),
        0,
    )
}

fn request(facade: &PublicationTower, key: u64, round: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "explain-original-review",
        facade.request_address(key).unwrap(),
        Verb::Explain,
    );
    request.arguments = ReadArguments::ReviewExplanation { round };
    request
}

fn explain(facade: &mut PublicationTower, key: u64, round: u64) -> PublicationResponse {
    let request = request(facade, key, round);
    facade.exchange(&access(true), encode_request(&request).unwrap().as_bytes())
}

fn native(response: &PublicationResponse) -> &FileReviewExplanationResult {
    let PublicationResult::ReviewExplanation(linked) = &response.result else {
        panic!("expected original completed-review observation: {response:?}");
    };
    assert_eq!(linked.registration(), &response.envelope.address);
    no_global_claims(response);
    linked.native()
}

fn known(response: &PublicationResponse) -> &FileReviewExplanation {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected Known completed original review: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    let PublicationResult::ReviewExplanation(linked) = &response.result else {
        unreachable!()
    };
    assert_eq!(linked.round(), basis.round);
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

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
    no_global_claims(response);
}

fn no_global_claims(response: &PublicationResponse) {
    fn unknown<T>(value: &Knowledge<T>) {
        assert!(matches!(
            value,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    unknown(&response.envelope.generation);
    unknown(&response.envelope.control_seq);
    unknown(&response.envelope.source_frontiers);
    unknown(&response.envelope.budgets_remaining);
    unknown(&response.envelope.realized_cost);
    unknown(&response.envelope.affordances);
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn completed(root: &Directory) -> (Port, Supervisor, oversight::Keys) {
    let (actor, mut supervisor, reviewer, mut proposal) = create(root);
    proposal.payload = PRIVATE_PAYLOAD.to_vec();
    observe(&mut supervisor);
    let ticket = actor.submit(REQUEST, &proposal).unwrap();
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: REQUEST }
    );
    let keys = ready(&mut supervisor, &reviewer, REQUEST);
    (actor, supervisor, keys)
}

fn begin(supervisor: &mut Supervisor, key: u64, round: u64) -> CommitteeInput {
    let mut host = supervisor.host_mut().unwrap();
    let attempt = support::attempt(&host, key);
    let action = host.request_action(key).unwrap().clone();
    let inputs = oversight::inputs(&action, b"PRIVATE-SOURCE-BYTES");
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
fn completed_congress_and_actual_two_key_publication_keep_three_distinct_cuts() {
    let root = Directory::new();
    let (actor, mut supervisor, keys) = completed(&root);
    let original = supervisor.host().unwrap().review_replay(ROUND).unwrap();
    let (port, grant) = supervisor
        .review_explanation_port_with_budget(2, MAX_REVIEW_EXPLANATION_EVENTS)
        .unwrap();
    let mut facade = tower(actor);
    facade.set_review_explanation(Some(port)).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let response = explain(&mut facade, REQUEST, ROUND);
    let first = known(&response);
    let basis = first.basis();
    assert_eq!((basis.request, basis.round), (REQUEST, ROUND));
    assert_ne!(basis.attempt, REQUEST);
    assert_eq!(basis.scope, profile().delivery.scope);
    assert_eq!(basis.begin_revision, original.begin_revision());
    assert_eq!(
        basis.begin_control_sequence,
        original.anchor().policy.expected_control_sequence
    );
    assert_eq!(basis.started_at, original.anchor().started_at);
    assert_eq!(
        basis.completed_revision,
        original.completed_snapshot().revision
    );
    assert_eq!(
        basis.completed_control_sequence,
        original.completed_snapshot().control.sequence
    );
    assert_eq!(basis.journal_revision, before.revision);
    assert_eq!(basis.control_sequence, before.control.sequence);
    assert!(basis.begin_revision < basis.completed_revision);
    assert!(basis.completed_revision < basis.journal_revision);
    assert_eq!(first.policy(), &original.anchor().policy.policy);
    assert_eq!(first.evaluation(), &original.archive().policy.evaluation);
    assert_eq!(first.congress(), &original.anchor().policy.congress);
    assert_eq!(first.tally(), &original.archive().policy.tally);
    assert_eq!(first.decision(), &original.archive().policy.decision);
    assert_eq!(first.evaluation().result(), Truth::Satisfied);
    assert_eq!(first.decision().consequence, Consequence::Continue);
    let application = first.application().unwrap();
    let native_application = original.application().as_ref().unwrap();
    assert_eq!(
        application.control_sequence,
        native_application.policy.control.sequence
    );
    assert_eq!(
        application.input_revision,
        native_application.input_revision
    );
    assert_eq!(application.consequence, Consequence::Continue);
    assert!(first.missing().is_empty());
    assert!(first.abstained().is_empty());
    assert!(first.effect().dispatch().is_none());
    assert!(first.effect().outcome().is_none());
    assert!(first.effect().reconciliation().is_none());
    assert!(!first.effect().selected_review_dispatched());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
    assert_eq!(grant.remaining_queries(), 1);
    let initial_wire = document(&response);
    wire::projection(&response);
    let initial_effect = field(
        &initial_wire,
        &["result", "value", "knowledge", "value", "effect"],
    );
    for part in ["dispatch", "outcome", "reconciliation"] {
        assert_eq!(
            field(initial_effect, &[part, "state"]).as_str(),
            Some("unknown")
        );
        assert_eq!(
            field(initial_effect, &[part, "reason"]).as_str(),
            Some("not_observed")
        );
    }

    // Reading retains the real automatic and human keys; only their native
    // dispatch and original endpoint publication establish an executed effect.
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        host.publish(revision, keys.automatic.attempt()).unwrap();
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let published = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let response = explain(&mut facade, REQUEST, ROUND);
    let later = known(&response);
    assert_eq!(later.basis().begin_revision, basis.begin_revision);
    assert_eq!(later.basis().completed_revision, basis.completed_revision);
    assert!(later.basis().journal_revision > basis.journal_revision);
    assert_eq!(later.policy(), first.policy());
    assert_eq!(later.evaluation(), first.evaluation());
    assert_eq!(later.tally(), first.tally());
    assert_eq!(later.application(), first.application());
    assert_eq!(later.completed_state(), first.completed_state());
    assert_eq!(later.ending_state(), ActionState::Confirmed);
    assert!(later.effect().selected_review_dispatched());
    let dispatch = later.effect().dispatch().unwrap();
    assert_eq!(dispatch.authorization().round(), ROUND);
    assert_eq!(
        dispatch.authorization().review_revision(),
        basis.completed_revision
    );
    assert_eq!(dispatch.human().request, keys.human.request());
    let outcome = EndpointOutcome::Executed {
        resulting_version: 2,
    };
    assert_eq!(later.effect().outcome().unwrap().outcome(), outcome);
    assert_eq!(
        later.effect().reconciliation().unwrap().result(),
        Ok(Reconciliation::Resolved(outcome))
    );
    assert_eq!(published.executions, 1);
    assert_eq!(published.control.ledger.charged, 16);
    assert_eq!(supervisor.host().unwrap().inspect(), published);
    assert_eq!(image(&root), bytes);
    assert_eq!(grant.remaining_queries(), 0);
    let wire = document(&response);
    let effect = field(&wire, &["result", "value", "knowledge", "value", "effect"]);
    assert_eq!(
        field(effect, &["dispatch", "value", "selected_review"]).as_bool(),
        Some(true)
    );
    assert_eq!(
        field(effect, &["outcome", "value", "outcome", "kind"]).as_str(),
        Some("executed")
    );
    assert_eq!(
        field(effect, &["reconciliation", "value", "result", "state"]).as_str(),
        Some("resolved")
    );
}

#[test]
fn replayable_continue_does_not_hide_an_original_stale_application() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let inputs = begin(&mut supervisor, REQUEST, ROUND);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::votes(&mut host, ROUND, Verdict::Allow);
        let changed = oversight::inputs(inputs.action(), b"a changed original helper packet");
        let revision = host.revision();
        assert_eq!(
            host.finish_review(revision, ROUND, Some(&changed), snapshot())
                .unwrap(),
            Err(Error::Stale)
        );
    }
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let (port, grant) = supervisor.review_explanation_port(1).unwrap();
    let mut facade = tower(actor);
    facade.set_review_explanation(Some(port)).unwrap();
    let response = explain(&mut facade, REQUEST, ROUND);
    let review = known(&response);
    assert_eq!(review.decision().consequence, Consequence::Continue);
    assert_eq!(review.evaluation().result(), Truth::Satisfied);
    assert_eq!(review.application(), Err(Error::Stale));
    assert!(review.effect().dispatch().is_none());
    assert!(review.effect().outcome().is_none());
    let wire = document(&response);
    let body = field(&wire, &["result", "value", "knowledge", "value"]);
    assert_eq!(
        field(body, &["decision", "consequence"]).as_str(),
        Some("continue")
    );
    assert_eq!(
        field(body, &["application", "state"]).as_str(),
        Some("refused")
    );
    assert_eq!(
        field(body, &["application", "code"]).as_str(),
        Some("stale")
    );
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
    assert_eq!(before.executions, 0);
    assert_eq!(before.control.ledger.reserved, 0);
}

#[test]
fn missing_and_abstaining_members_remain_distinct_original_reducer_inputs() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer, proposal) = create(&root);
    observe(&mut supervisor);
    actor.submit(REQUEST, &proposal).unwrap();
    let inputs = begin(&mut supervisor, REQUEST, ROUND);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::commit(&mut host, ROUND, "alpha", Verdict::Abstain);
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(6)).unwrap();
        let revision = host.revision();
        host.open_reveals(revision, ROUND).unwrap();
        let revision = host.revision();
        host.reveal_review(
            revision,
            ROUND,
            "alpha",
            Verdict::Abstain,
            oversight::salt("alpha"),
        )
        .unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(11)).unwrap();
        let revision = host.revision();
        host.finish_review(revision, ROUND, Some(&inputs), snapshot())
            .unwrap()
            .unwrap();
    }
    let (port, grant) = supervisor.review_explanation_port(1).unwrap();
    let mut facade = tower(actor);
    facade.set_review_explanation(Some(port)).unwrap();
    let before = image(&root);
    let response = explain(&mut facade, REQUEST, ROUND);
    let review = known(&response);
    assert_eq!(review.missing(), &["beta".to_owned()]);
    assert_eq!(review.abstained(), &["alpha".to_owned()]);
    assert_eq!(review.tally().permit_weight, 0);
    assert_eq!(review.decision().consequence, Consequence::HoldEffect);
    assert!(review.decision().rules.contains(&Rule::MandatoryAbsent));
    let wire = document(&response);
    let congress = field(
        &wire,
        &["result", "value", "knowledge", "value", "congress"],
    );
    let members = field(congress, &["members"]).as_array().unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(field(&members[0], &["member"]).as_str(), Some("alpha"));
    assert_eq!(field(&members[0], &["outcome"]).as_str(), Some("abstained"));
    assert_eq!(field(&members[1], &["member"]).as_str(), Some("beta"));
    assert_eq!(field(&members[1], &["outcome"]).as_str(), Some("missing"));
    assert!(field(&members[1], &["committed_at"]).is_null());
    assert!(field(&members[1], &["revealed_at"]).is_null());
    assert_eq!(image(&root), before);
    assert_eq!(grant.remaining_queries(), 0);
}
