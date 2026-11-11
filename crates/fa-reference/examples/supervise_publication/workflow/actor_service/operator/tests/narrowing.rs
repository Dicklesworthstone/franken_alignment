//! Authenticated ordinary-run target narrowing uses the original owner and
//! independent grants. Retained publication still needs its own complete review
//! and human key; a target ceiling never proves an unknown effect did not occur.

use super::*;
use fa_reference::action::{ActionState, ResolvedTarget};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, MAX_NARROW_OPERATIONS, MAX_NARROW_QUERIES, MAX_NARROW_REPLAY_EVENTS,
};

#[path = "../../../../../../tests/support/file_oversight.rs"]
mod native;

fn selection(queries: usize, operations: usize, events: usize) -> String {
    format!(r#"{{"queries":{queries},"operations":{operations},"max_replay_events":{events},"authority":"narrow_original_targets"}}"#)
}

fn selected(profile: &str, narrowing: &str) -> String {
    profile.replacen('{', &format!("{{\"narrowing\":{narrowing},"), 1)
}

fn fixture(queries: usize, operations: usize) -> Fixture {
    let mut fixture = Fixture::configured(1, 0, |config| {
        let clock_domain = config.profile.delivery.clock_domain;
        config.profile = native::profile();
        config.profile.delivery.clock_domain = clock_domain;
    });
    fixture.settings = Settings::decode(selected(
        &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
        &selection(queries, operations, MAX_NARROW_REPLAY_EVENTS),
    ).as_bytes()).unwrap();
    fixture.settings.candidate_limit = 16;
    fixture.settings.connection_limit = 12;
    fixture
}

fn next(fixture: &Fixture, operation: u64, retained_targets: Vec<ResolvedTarget>) -> TowerRequest {
    let mut request = fixture.request(Verb::Next);
    request.arguments = ReadArguments::NarrowingNext { operation, retained_targets };
    request
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn command(fixture: &Fixture, response: &Json, operation: u64, retained_targets: Vec<ResolvedTarget>) -> TowerRequest {
    known(response, "narrowing-next");
    let basis = field(response, &["result", "value", "knowledge", "basis"]);
    let mut request = fixture.request(Verb::Narrow);
    request.operation_id = format!("narrow-authority-{operation}");
    request.idempotency_key = Some(operation.to_string());
    request.arguments = ReadArguments::Narrow {
        operation,
        expected: FileNarrowBasis {
            scope: fixture.actor.scope,
            journal_revision: number(basis.get("journal_revision").unwrap()),
            control_sequence: number(basis.get("control_sequence").unwrap()),
            authority_epoch: number(basis.get("authority_epoch").unwrap()),
            recorded_at: basis.get("recorded_at").filter(|tick| !tick.is_null())
                .map(|tick| ElapsedTick(number(tick))),
        },
        retained_targets,
    };
    request
}

fn action(fixture: &Fixture, response: &Json, operation: u64, targets: Vec<ResolvedTarget>) -> TowerRequest {
    let request = command(fixture, response, operation, targets);
    let encoded = encode_request(&request).unwrap();
    assert_eq!(
        &strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap(),
        field(response, &["result", "value", "knowledge", "value", "action"]),
    );
    request
}

fn remaining(service: &Service) -> (usize, usize) {
    let grant = service.active.as_ref().unwrap().narrowing.as_ref().unwrap();
    (grant.remaining_queries(), grant.remaining_operations())
}

fn attempt(fixture: &Fixture, request: u64) -> u64 {
    match fixture.driver.supervisor().host().unwrap().request_status(request).unwrap().disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("request not admitted: {other:?}"),
    }
}

fn submit_current(fixture: &mut Fixture, request: u64) -> u64 {
    fixture.snapshot();
    let mut proposal = fixture.proposal();
    proposal.target = fixture.driver.supervisor().host().unwrap().inspect().target;
    let ticket = fixture.port.submit(request, &proposal).unwrap();
    assert_eq!(fixture.port.poll(&ticket), Knowledge::Pending { request });
    attempt(fixture, request)
}

fn ready(fixture: &mut Fixture, request: u64) -> native::Keys {
    let id = attempt(fixture, request);
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let action = host.request_action(request).unwrap().clone();
    let inputs = native::inputs(&action, b"original independent review evidence");
    native::review_existing(&mut host, id, request + 100, &inputs);
    let revision = host.revision();
    let automatic = host.authorize(revision, id, &inputs, native::snapshot()).unwrap();
    let revision = host.revision();
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let pending = host.request_human_approval(
        revision, request + 1_000, id, &inputs, ElapsedTick(now.0 + 30),
    ).unwrap();
    let revision = host.revision();
    let human = fixture.human.approve(&mut host, revision, &pending).unwrap();
    native::Keys { action, inputs, automatic, human, request: pending }
}

fn publish(fixture: &mut Fixture, keys: &native::Keys) {
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    native::dispatch(&mut host, keys);
    let revision = host.revision();
    host.publish(revision, keys.automatic.attempt()).unwrap();
    let revision = host.revision();
    host.reconcile(revision, keys.automatic.attempt()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&keys.automatic.attempt()], ActionState::Confirmed);
}

#[test]
fn narrowing_profile_is_closed_bounded_independent_and_ordinary_only() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let old = profile_json(Path::new("/operator/narrow.sock"), identity, 1, 0);
    for document in [old.clone(), selected(&old, "null")] {
        let settings = Settings::decode(document.as_bytes()).unwrap();
        assert!(settings.narrowing.is_none());
        settings.check_learned_operator().unwrap();
    }
    let only = profile_json(Path::new("/operator/narrow.sock"), identity, 0, 0);
    let settings = Settings::decode(selected(&only, &selection(
        MAX_NARROW_QUERIES, MAX_NARROW_OPERATIONS, MAX_NARROW_REPLAY_EVENTS,
    )).as_bytes()).unwrap();
    assert!(settings.resolution.is_none());
    assert!(settings.cancellation.is_none());
    assert!(settings.abandonment.is_none());
    assert!(settings.fence.is_none());
    assert!(settings.check_learned_operator().is_err());
    let chosen = settings.narrowing.unwrap();
    assert_eq!((chosen.queries, chosen.operations, chosen.max_replay_events),
        (MAX_NARROW_QUERIES, MAX_NARROW_OPERATIONS, MAX_NARROW_REPLAY_EVENTS));
    for invalid in [
        "false".to_owned(), "{}".to_owned(), "[]".to_owned(),
        selection(0, 1, 1), selection(1, 0, 1), selection(1, 1, 0),
        selection(MAX_NARROW_QUERIES + 1, 1, 1),
        selection(1, MAX_NARROW_OPERATIONS + 1, 1),
        selection(1, 1, MAX_NARROW_REPLAY_EVENTS + 1),
        selection(1, 1, 1).replace("narrow_original_targets", "stop_original_admission"),
        selection(1, 1, 1).replace("\"operations\":1", "\"operations\":\"1\""),
        selection(1, 1, 1).replacen('{', "{\"operations\":1,", 1),
        selection(1, 1, 1).replacen('{', "{\"numerical_replay_events\":1,", 1),
        selection(1, 1, 1).replacen('{', "{\"clock\":999999,", 1),
    ] {
        assert!(Settings::decode(selected(&old, &invalid).as_bytes()).is_err());
    }
}

#[test]
fn authenticated_narrowing_refunds_outside_work_and_retained_request_really_publishes() {
    let mut fixture = fixture(3, 1);
    let outside = submit_current(&mut fixture, 7);
    let _outside_keys = ready(&mut fixture, 7);
    submit_current(&mut fixture, 8);
    let advance = ready(&mut fixture, 8);
    publish(&mut fixture, &advance);
    // A real preceding publication advances the same endpoint to version 2.
    // The version-1 outside reservation remains live until explicit narrowing.
    let inside = submit_current(&mut fixture, 9);
    let target = fixture.driver.supervisor().host().unwrap().inspect().target;
    assert_eq!(target.expected_version, 2);
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(before.control.ledger.stages[&outside], ActionState::Authorized);
    assert_eq!(before.control.ledger.stages[&inside], ActionState::Reviewing);
    assert_eq!(before.control.ledger.reserved, fixture.proposal().units);
    let files = fixture.files();
    let mut service = fixture.start();
    let plan = fixture.query(&mut service, next(&fixture, 901, vec![target]));
    let command = action(&fixture, &plan, 901, vec![target]);
    assert_eq!(fixture.files(), files);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(remaining(&service), (2, 1));
    let response = fixture.query(&mut service, command.clone());
    known(&response, "narrowing");
    let receipt = field(&response, &["result", "value", "knowledge", "value"]);
    assert_eq!(receipt.get("cancelled").unwrap().as_array().unwrap(), &[Json::String(outside.to_string())]);
    assert_eq!(number(receipt.get("refunded_units").unwrap()), fixture.proposal().units);
    assert!(receipt.get("previous_ceiling").unwrap().is_null());
    let after = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.sequence, before.control.sequence + 1);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.control.ledger.stages[&outside], ActionState::Cancelled);
    assert_eq!(after.control.ledger.stages[&inside], ActionState::Reviewing);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.available, before.control.ledger.available + fixture.proposal().units);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.executions, 1);
    assert!(after.stop.is_none());
    assert_eq!(remaining(&service), (2, 0));

    // The ceiling is no human approval. This retained action now completes its
    // original committee review, independent human decision and publication.
    let retained = ready(&mut fixture, 9);
    publish(&mut fixture, &retained);
    let progressed = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(progressed.executions, 2);
    assert_eq!(progressed.control.ledger.stages[&inside], ActionState::Confirmed);
    assert_eq!(progressed.control.ledger.charged, before.control.ledger.charged + fixture.proposal().units);
    assert_eq!(progressed.control.ledger.epoch, before.control.ledger.epoch);
    let files = fixture.files();
    assert_eq!(fixture.query(&mut service, command), response);
    assert_eq!(fixture.files(), files);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), progressed);
    assert_eq!(remaining(&service), (2, 0));
    let exhausted = fixture.query(&mut service, next(&fixture, 902, Vec::new()));
    assert_eq!(field(&exhausted, &["result", "value", "knowledge", "value", "block_reason"]).as_str(), Some("operation_limit"));
    assert!(field(&exhausted, &["result", "value", "knowledge", "value", "action"]).is_null());
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    service.retire();
    assert!(service.active.is_none());
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn empty_ceiling_preserves_unknown_charge_and_cannot_restore_removed_targets() {
    let mut fixture = fixture(2, 2);
    let unresolved = submit_current(&mut fixture, 7);
    let dispatched = ready(&mut fixture, 7);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        native::dispatch(&mut host, &dispatched);
        let revision = host.revision();
        host.reconcile(revision, unresolved).unwrap();
    }
    let cancelled = submit_current(&mut fixture, 8);
    let _reserved = ready(&mut fixture, 8);
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(before.control.ledger.stages[&unresolved], ActionState::Unknown);
    let mut service = fixture.start();
    let plan = fixture.query(&mut service, next(&fixture, 903, Vec::new()));
    let response = fixture.query(&mut service, action(&fixture, &plan, 903, Vec::new()));
    known(&response, "narrowing");
    let after = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.control.ledger.stages[&unresolved], ActionState::Unknown);
    assert_eq!(after.control.ledger.stages[&cancelled], ActionState::Cancelled);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(after.executions, 0);
    let target = after.target;
    let plan = fixture.query(&mut service, next(&fixture, 904, vec![target]));
    assert!(field(&plan, &["result", "value", "knowledge", "value", "effective_targets"]).as_array().unwrap().is_empty());
    let response = fixture.query(&mut service, action(&fixture, &plan, 904, vec![target]));
    known(&response, "narrowing");
    let receipt = field(&response, &["result", "value", "knowledge", "value"]);
    assert!(receipt.get("previous_ceiling").unwrap().as_array().unwrap().is_empty());
    assert!(receipt.get("effective_targets").unwrap().as_array().unwrap().is_empty());
    assert!(receipt.get("cancelled").unwrap().as_array().unwrap().is_empty());
    assert_eq!(number(receipt.get("refunded_units").unwrap()), 0);
    let final_state = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(final_state.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(final_state.control.ledger.stages[&unresolved], ActionState::Unknown);
    assert_eq!(final_state.control.ledger.epoch, before.control.ledger.epoch);
    assert_eq!(final_state.executions, 0);
    assert_eq!(remaining(&service), (0, 0));
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn stale_narrowing_preserves_original_allowances_and_unselected_controls_stay_disabled() {
    let mut fixture = fixture(3, 1);
    submit_current(&mut fixture, 7);
    let target = fixture.driver.supervisor().host().unwrap().inspect().target;
    let mut service = fixture.start();
    let plan = fixture.query(&mut service, next(&fixture, 905, vec![target]));
    let stale = action(&fixture, &plan, 905, vec![target]);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1_001)).unwrap();
    }
    let before = fixture.files();
    let response = fixture.query(&mut service, stale);
    assert_eq!(field(&response, &["result", "value", "error", "code"]).as_str(), Some("stale_predecessor"));
    assert_eq!(remaining(&service), (2, 1));
    assert_eq!(fixture.files(), before);
    let mut proposal = fixture.request(Verb::Propose);
    proposal.idempotency_key = Some("8".into());
    proposal.arguments = ReadArguments::Propose { request: 8, proposal: fixture.proposal() };
    let mut cancel = fixture.request(Verb::Explain);
    cancel.verb = Verb::Next;
    cancel.arguments = ReadArguments::CancellationNext { operation: 906 };
    let mut hold = fixture.request(Verb::Explain);
    hold.verb = Verb::Hold;
    hold.idempotency_key = Some("hold-7".into());
    for request in [proposal, cancel, hold] {
        let response = fixture.query(&mut service, request);
        assert_eq!(field(&response, &["envelope", "reason_code"]).as_str(), Some("CapabilityUnsupported"));
    }
    assert_eq!(remaining(&service), (2, 1));
    assert_eq!(fixture.files(), before);
    let plan = fixture.query(&mut service, next(&fixture, 905, vec![target]));
    known(&fixture.query(&mut service, action(&fixture, &plan, 905, vec![target])), "narrowing");
    assert_eq!(remaining(&service), (1, 0));
}

#[test]
fn absent_narrowing_and_exhausted_replay_do_not_mutate_or_mint_work() {
    let old = Fixture::new(1, 0);
    let mut service = old.start();
    let before = old.files();
    let response = old.query(&mut service, next(&old, 907, Vec::new()));
    assert_eq!(field(&response, &["result", "kind"]).as_str(), Some("refused"));
    assert!(service.active.as_ref().unwrap().narrowing.is_none());
    assert_eq!(old.files(), before);

    let mut limited = fixture(1, 1);
    limited.settings.narrowing.as_mut().unwrap().max_replay_events = 1;
    submit_current(&mut limited, 7);
    let before = limited.files();
    let state = limited.driver.supervisor().host().unwrap().inspect();
    let mut service = limited.start();
    let plan = limited.query(&mut service, next(&limited, 908, Vec::new()));
    assert_eq!(field(&plan, &["result", "value", "knowledge", "value", "block_reason"]).as_str(), Some("replay_limit"));
    let response = limited.query(&mut service, command(&limited, &plan, 908, Vec::new()));
    assert_eq!(field(&response, &["result", "value", "error", "code"]).as_str(), Some("capacity"));
    assert_eq!(remaining(&service), (0, 1));
    assert_eq!(limited.files(), before);
    assert_eq!(limited.driver.supervisor().host().unwrap().inspect(), state);
}
