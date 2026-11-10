//! Actual original receipt reconciliation through authenticated operator sockets.

use super::*;
use fa_reference::action::{ActionState, Purpose};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileHoldDiagnosisBasis, MAX_RESOLUTION_OPERATIONS, MAX_RESOLUTION_QUERIES,
    MAX_RESOLUTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;

#[path = "../../../../../../tests/support/file_oversight.rs"]
mod native;

fn selection(queries: usize, operations: usize, events: usize) -> String {
    format!(
        r#"{{"queries":{queries},"operations":{operations},"max_replay_events":{events},"authority":"reconcile_existing_receipts"}}"#
    )
}

fn with_resolution(profile: &str, selected: &str) -> String {
    profile.replacen('{', &format!("{{\"resolution\":{selected},"), 1)
}

fn fixture(queries: usize, operations: usize) -> Fixture {
    let mut fixture = Fixture::configured(1, 0, |config| config.profile = native::profile());
    fixture.settings = Settings::decode(
        with_resolution(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
            &selection(queries, operations, MAX_RESOLUTION_REPLAY_EVENTS),
        ).as_bytes(),
    ).unwrap();
    fixture.settings.candidate_limit = 16;
    fixture.settings.connection_limit = 12;
    fixture
}

fn pending(fixture: &mut Fixture, publish: bool) -> u64 {
    fixture.submit(7);
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(7).unwrap().disposition
    else {
        panic!("original external request was not admitted");
    };
    let action = host.request_action(7).unwrap().clone();
    let inputs = native::inputs(&action, b"original review evidence");
    native::review_existing(&mut host, attempt, 107, &inputs);
    let revision = host.revision();
    let automatic = host.authorize(revision, attempt, &inputs, native::snapshot()).unwrap();
    let revision = host.revision();
    let request = host.request_human_approval(
        revision, 1007, attempt, &inputs, ElapsedTick(1_030),
    ).unwrap();
    let revision = host.revision();
    let human = fixture.human.approve(&mut host, revision, &request).unwrap();
    let revision = host.revision();
    host.dispatch(revision, &automatic, &human, &action, &inputs, native::snapshot()).unwrap();
    if publish {
        let revision = host.revision();
        let result = host.publish_checked(
            revision, attempt, Some(&inputs), native::snapshot(), ElapsedTick(1_001),
        ).unwrap();
        assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    }
    attempt
}

fn next(fixture: &Fixture, operation: u64) -> TowerRequest {
    let mut query = fixture.request(Verb::Explain);
    query.verb = Verb::Next;
    query.arguments = ReadArguments::ResolutionNext { operation };
    query
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

/// Recover the returned command as actual request data and compare its complete
/// canonical document with the wire action, before sending it through the peer.
fn action(fixture: &Fixture, response: &Json) -> TowerRequest {
    known(response, "resolution-next");
    let wire = field(response, &["result", "value", "knowledge", "value", "action"]);
    let arguments = wire.get("arguments").unwrap();
    let basis = arguments.get("expected").unwrap();
    let operation = number(arguments.get("operation").unwrap());
    let mut request = next(fixture, operation);
    request.operation_id = wire.get("operation_id").unwrap().as_str().unwrap().into();
    request.verb = Verb::Resolve;
    request.idempotency_key = Some(operation.to_string());
    request.arguments = ReadArguments::Resolve {
        operation,
        expected: FileHoldDiagnosisBasis {
            scope: fixture.actor.scope,
            request: number(basis.get("request").unwrap()),
            request_generation: number(basis.get("request_generation").unwrap()),
            attempt: number(basis.get("attempt").unwrap()),
            journal_revision: number(basis.get("journal_revision").unwrap()),
            control_sequence: number(basis.get("control_sequence").unwrap()),
            recorded_at: basis.get("recorded_at").filter(|tick| !tick.is_null()).map(|tick| ElapsedTick(number(tick))),
        },
    };
    assert_eq!(fixture.actor.scope.purpose, Purpose::Effect);
    let encoded = encode_request(&request).unwrap();
    assert_eq!(&strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap(), wire);
    request
}

fn remaining(service: &Service) -> (usize, usize) {
    let grant = service.active.as_ref().unwrap().resolution.as_ref().unwrap();
    (grant.remaining_queries(), grant.remaining_operations())
}

#[test]
fn resolution_profile_requires_explicit_bounded_authority_and_does_not_upgrade_old_profiles() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let original = profile_json(Path::new("/operator/resolve.sock"), identity, 1, 0);
    for profile in [original.clone(), with_resolution(&original, "null")] {
        let decoded = Settings::decode(profile.as_bytes()).unwrap();
        assert!(decoded.resolution.is_none());
        decoded.check_learned_observer().unwrap();
    }
    let selected = selection(MAX_RESOLUTION_QUERIES, MAX_RESOLUTION_OPERATIONS, MAX_RESOLUTION_REPLAY_EVENTS);
    let decoded = Settings::decode(with_resolution(
        &profile_json(Path::new("/operator/resolve.sock"), identity, 0, 0), &selected,
    ).as_bytes()).unwrap();
    assert_eq!((decoded.inspection_queries, decoded.explanation_queries), (0, 0));
    assert!(decoded.hold_diagnosis.is_none());
    assert!(decoded.check_learned_observer().is_err());
    let selected = decoded.resolution.unwrap();
    assert_eq!(selected.queries, MAX_RESOLUTION_QUERIES);
    assert_eq!(selected.operations, MAX_RESOLUTION_OPERATIONS);
    assert_eq!(selected.max_replay_events, MAX_RESOLUTION_REPLAY_EVENTS);
    for invalid in [
        "false".to_owned(), "{}".to_owned(), "[]".to_owned(),
        selection(0, 1, 1), selection(1, 0, 1), selection(1, 1, 0),
        selection(MAX_RESOLUTION_QUERIES + 1, 1, 1),
        selection(1, MAX_RESOLUTION_OPERATIONS + 1, 1),
        selection(1, 1, MAX_RESOLUTION_REPLAY_EVENTS + 1),
        selection(1, 1, 1).replace("reconcile_existing_receipts", "permit"),
        selection(1, 1, 1).replace("\"operations\":1", "\"operations\":\"1\""),
        selection(1, 1, 1).replacen('{', "{\"clock\":999999,", 1),
        selection(1, 1, 1).replacen('{', "{\"operations\":1,", 1),
    ] {
        assert!(Settings::decode(with_resolution(&original, &invalid).as_bytes()).is_err());
    }
}

#[test]
fn returned_operator_action_reconciles_original_publication_and_retries_without_another_mutation() {
    let mut fixture = fixture(3, 1);
    let attempt = pending(&mut fixture, true);
    let mut service = fixture.start();
    let before = fixture.files();
    let state = fixture.driver.supervisor().host().unwrap().inspect();
    let plan = fixture.query(&mut service, next(&fixture, 901));
    let command = action(&fixture, &plan);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(remaining(&service), (2, 1));
    let resolved = fixture.query(&mut service, command.clone());
    known(&resolved, "resolution");
    assert_eq!(field(&resolved, &["result", "value", "may_write_journal"]).as_bool(), Some(true));
    assert_eq!(field(&resolved, &["result", "value", "knowledge", "value", "stage"]).as_str(), Some("confirmed"));
    let settled = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(settled.executions, 1);
    assert_eq!(settled.control.ledger.stages.get(&attempt), Some(&ActionState::Confirmed));
    assert_eq!(settled.control.ledger.charged, state.control.ledger.charged);
    let saved = fixture.files();
    assert_ne!(saved, before);
    assert_eq!(remaining(&service), (2, 0));
    assert_eq!(fixture.query(&mut service, command.clone()), resolved);
    assert_eq!(fixture.files(), saved);
    assert_eq!(remaining(&service), (2, 0));
    let mut changed = command;
    if let ReadArguments::Resolve { expected, .. } = &mut changed.arguments {
        expected.control_sequence += 1;
    }
    let conflict = fixture.query(&mut service, changed);
    assert_eq!(field(&conflict, &["result", "value", "error", "code"]).as_str(), Some("idempotency_conflict"));
    assert_eq!(fixture.files(), saved);
    let terminal = fixture.query(&mut service, next(&fixture, 902));
    assert_eq!(field(&terminal, &["result", "value", "knowledge", "value", "block_reason"]).as_str(), Some("terminal"));
    assert!(field(&terminal, &["result", "value", "knowledge", "value", "action"]).is_null());
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn resolution_authority_cannot_submit_cancel_or_consume_the_actor_admission_snapshot() {
    let mut fixture = fixture(2, 1);
    fixture.submit(7);
    fixture.snapshot();
    let before = fixture.files();
    let mut service = fixture.start();
    let mut proposed = fixture.request(Verb::Propose);
    proposed.arguments = ReadArguments::Propose { request: 8, proposal: fixture.proposal() };
    proposed.idempotency_key = Some("8".into());
    let mut hold = fixture.request(Verb::Explain);
    hold.verb = Verb::Hold;
    hold.idempotency_key = Some("hold-7".into());
    for request in [proposed, hold] {
        let refused = fixture.query(&mut service, request);
        assert_eq!(field(&refused, &["envelope", "reason_code"]).as_str(), Some("CapabilityUnsupported"));
    }
    assert_eq!(remaining(&service), (2, 1));
    assert_eq!(fixture.files(), before);
    let plan = fixture.query(&mut service, next(&fixture, 903));
    assert_eq!(field(&plan, &["result", "value", "knowledge", "value", "block_reason"]).as_str(), Some("not_dispatched"));
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(fixture.port.poll(&ticket), Knowledge::Pending { request: 8 });
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn stale_operator_action_refuses_before_work_and_receipt_absence_keeps_the_liability() {
    let mut fixture = fixture(3, 2);
    let attempt = pending(&mut fixture, false);
    let mut service = fixture.start();
    let first = fixture.query(&mut service, next(&fixture, 904));
    let stale = action(&fixture, &first);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1_001)).unwrap();
    }
    fixture.snapshot();
    let before = fixture.files();
    let refused = fixture.query(&mut service, stale);
    assert_eq!(field(&refused, &["result", "value", "error", "code"]).as_str(), Some("stale_predecessor"));
    assert_eq!(remaining(&service), (2, 2));
    assert_eq!(fixture.files(), before);
    let current = fixture.query(&mut service, next(&fixture, 904));
    let command = action(&fixture, &current);
    let result = fixture.query(&mut service, command);
    known(&result, "resolution");
    assert_eq!(field(&result, &["result", "value", "knowledge", "value", "outcome", "kind"]).as_str(), Some("awaiting_resolution"));
    let state = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.stages.get(&attempt), Some(&ActionState::Unknown));
    assert_eq!(state.control.ledger.charged, fixture.proposal().units);
    // The actual mutation invalidated the older untagged one-use admission view.
    assert!(fixture.port.submit(8, &fixture.proposal()).is_err());
    assert_eq!(remaining(&service), (1, 1));
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}
