//! Separate operator authority over one original Unknown delivery, using
//! authenticated sockets and the original owner, grants and request ledger.

use super::*;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileHoldDiagnosisBasis, MAX_ABANDONMENT_OPERATIONS, MAX_ABANDONMENT_QUERIES,
    MAX_ABANDONMENT_REPLAY_EVENTS,
};

#[path = "../../../../../../tests/support/file_oversight.rs"]
mod native;

fn selection(queries: usize, operations: usize, events: usize) -> String {
    format!(
        r#"{{"queries":{queries},"operations":{operations},"max_replay_events":{events},"authority":"abandon_unknown_requests"}}"#
    )
}

fn selected(profile: &str, abandonment: &str) -> String {
    profile.replacen('{', &format!("{{\"abandonment\":{abandonment},"), 1)
}

fn fixture(queries: usize, operations: usize) -> Fixture {
    let mut fixture = Fixture::configured(1, 0, |config| {
        let clock_domain = config.profile.delivery.clock_domain;
        config.profile = native::profile();
        config.profile.delivery.clock_domain = clock_domain;
    });
    fixture.settings = Settings::decode(
        selected(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
            &selection(queries, operations, MAX_ABANDONMENT_REPLAY_EVENTS),
        )
        .as_bytes(),
    )
    .unwrap();
    fixture.settings.candidate_limit = 16;
    fixture.settings.connection_limit = 12;
    fixture
}

fn next(fixture: &Fixture, operation: u64) -> TowerRequest {
    let mut request = fixture.request(Verb::Explain);
    request.verb = Verb::Next;
    request.arguments = ReadArguments::AbandonmentNext { operation };
    request
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn command(fixture: &Fixture, response: &Json, operation: u64) -> TowerRequest {
    known(response, "abandonment-next");
    let basis = field(response, &["result", "value", "knowledge", "basis"]);
    let mut request = next(fixture, operation);
    request.operation_id = format!("abandon-unknown-{operation}");
    request.verb = Verb::Resolve;
    request.idempotency_key = Some(operation.to_string());
    request.arguments = ReadArguments::AbandonUnknown {
        operation,
        expected: FileHoldDiagnosisBasis {
            scope: fixture.actor.scope,
            request: number(basis.get("request").unwrap()),
            request_generation: number(basis.get("request_generation").unwrap()),
            attempt: number(basis.get("attempt").unwrap()),
            journal_revision: number(basis.get("journal_revision").unwrap()),
            control_sequence: number(basis.get("control_sequence").unwrap()),
            recorded_at: basis
                .get("recorded_at")
                .filter(|tick| !tick.is_null())
                .map(|tick| ElapsedTick(number(tick))),
        },
    };
    request
}

fn action(fixture: &Fixture, response: &Json, operation: u64) -> TowerRequest {
    let request = command(fixture, response, operation);
    let encoded = encode_request(&request).unwrap();
    assert_eq!(
        &strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap(),
        field(
            response,
            &["result", "value", "knowledge", "value", "action"]
        ),
    );
    request
}

fn remaining(service: &Service) -> (usize, usize) {
    let grant = service
        .active
        .as_ref()
        .unwrap()
        .abandonment
        .as_ref()
        .unwrap();
    (grant.remaining_queries(), grant.remaining_operations())
}

fn attempt(fixture: &Fixture, request: u64) -> u64 {
    match fixture
        .driver
        .supervisor()
        .host()
        .unwrap()
        .request_status(request)
        .unwrap()
        .disposition
    {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("request not admitted: {other:?}"),
    }
}

fn dispatch(fixture: &mut Fixture, request: u64) -> u64 {
    let attempt = attempt(fixture, request);
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let action = host.request_action(request).unwrap().clone();
    let inputs = native::inputs(&action, b"original review evidence");
    native::review_existing(&mut host, attempt, request + 100, &inputs);
    let revision = host.revision();
    let automatic = host.authorize(revision, attempt, &inputs, native::snapshot()).unwrap();
    let revision = host.revision();
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let pending = host.request_human_approval(
        revision, request + 1_000, attempt, &inputs, ElapsedTick(now.0 + 30),
    ).unwrap();
    let revision = host.revision();
    let human = fixture.human.approve(&mut host, revision, &pending).unwrap();
    let revision = host.revision();
    host.dispatch(revision, &automatic, &human, &action, &inputs, native::snapshot()).unwrap();
    attempt
}

fn unknown(fixture: &mut Fixture, request: u64) -> u64 {
    let attempt = dispatch(fixture, request);
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    host.reconcile(revision, attempt).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&attempt], ActionState::Unknown);
    attempt
}

#[test]
fn abandonment_profile_is_separate_closed_and_numerical_work_is_explicit() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let old = profile_json(Path::new("/operator/cancel.sock"), identity, 1, 0);
    for document in [old.clone(), selected(&old, "null")] {
        let settings = Settings::decode(document.as_bytes()).unwrap();
        assert!(settings.abandonment.is_none());
        settings.check_learned_operator().unwrap();
    }
    let only = profile_json(Path::new("/operator/cancel.sock"), identity, 0, 0);
    let maximum = selection(
        MAX_ABANDONMENT_QUERIES,
        MAX_ABANDONMENT_OPERATIONS,
        MAX_ABANDONMENT_REPLAY_EVENTS,
    );
    let settings = Settings::decode(selected(&only, &maximum).as_bytes()).unwrap();
    assert!(settings.resolution.is_none());
    assert!(settings.cancellation.is_none());
    assert!(settings.fence.is_none());
    assert_eq!(
        (settings.inspection_queries, settings.explanation_queries),
        (0, 0)
    );
    assert!(settings.check_learned_operator().is_err());
    let chosen = settings.abandonment.unwrap();
    assert_eq!(chosen.queries, MAX_ABANDONMENT_QUERIES);
    assert_eq!(chosen.operations, MAX_ABANDONMENT_OPERATIONS);
    assert_eq!(chosen.max_replay_events, MAX_ABANDONMENT_REPLAY_EVENTS);
    for invalid in [
        "false".to_owned(),
        "{}".to_owned(),
        "[]".to_owned(),
        selection(0, 1, 1),
        selection(1, 0, 1),
        selection(1, 1, 0),
        selection(MAX_ABANDONMENT_QUERIES + 1, 1, 1),
        selection(1, MAX_ABANDONMENT_OPERATIONS + 1, 1),
        selection(1, 1, MAX_ABANDONMENT_REPLAY_EVENTS + 1),
        selection(1, 1, 1).replace(
            "abandon_unknown_requests",
            "reconcile_existing_receipts",
        ),
        selection(1, 1, 1).replace("\"operations\":1", "\"operations\":\"1\""),
        selection(1, 1, 1).replacen('{', "{\"clock\":999999,", 1),
        selection(1, 1, 1).replacen('{', "{\"operations\":1,", 1),
    ] {
        assert!(Settings::decode(selected(&old, &invalid).as_bytes()).is_err());
    }
    let base = selection(3, 2, 64);
    let numerical =
        |events: &str| base.replacen('{', &format!("{{\"numerical_replay_events\":{events},"), 1);
    let accepted = Settings::decode(selected(&only, &numerical("128")).as_bytes()).unwrap();
    accepted.check_learned_operator().unwrap();
    assert_eq!(
        accepted.abandonment.unwrap().numerical_replay_events,
        Some(128)
    );
    for invalid in [
        "0",
        "129",
        "null",
        "true",
        "\"1\"",
        "1.5",
        "-1",
        "[]",
        "{}",
        "18446744073709551616",
    ] {
        assert!(
            Settings::decode(selected(&only, &numerical(invalid)).as_bytes()).is_err(),
            "accepted invalid numerical abandonment budget {invalid}"
        );
    }
    let duplicate = numerical("1").replacen('{', "{\"numerical_replay_events\":1,", 1);
    assert!(Settings::decode(selected(&only, &duplicate).as_bytes()).is_err());
}


#[test]
fn authenticated_abandonment_preserves_liability_other_requests_and_original_retry() {
    let mut fixture = fixture(3, 1);
    fixture.settings.abandonment.as_mut().unwrap().numerical_replay_events = Some(1);
    fixture.submit(7);
    let abandoned = unknown(&mut fixture, 7);
    fixture.submit(8);
    fixture.snapshot();
    let unrelated = attempt(&fixture, 8);
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(before.control.ledger.charged, fixture.proposal().units);
    let files = fixture.files();
    let mut service = fixture.start();
    let plan = fixture.query(&mut service, next(&fixture, 901));
    let command = action(&fixture, &plan, 901);
    assert_eq!(fixture.files(), files);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(remaining(&service), (2, 1));

    let response = fixture.query(&mut service, command.clone());
    known(&response, "abandonment");
    assert_eq!(
        field(&response, &["result", "value", "knowledge", "value", "stage"]).as_str(),
        Some("irrecoverably_unknown")
    );
    assert_eq!(
        field(&response, &["result", "value", "knowledge", "value", "charged_units"]).as_str(),
        Some(fixture.proposal().units.to_string().as_str())
    );
    let after = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.ledger.stages[&abandoned], ActionState::IrrecoverablyUnknown);
    assert_eq!(after.control.ledger.stages[&unrelated], before.control.ledger.stages[&unrelated]);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.executions, before.executions);
    assert!(after.stop.is_none());
    assert_eq!(remaining(&service), (2, 0));
    assert_eq!(
        service.active.as_ref().unwrap().abandonment.as_ref().unwrap()
            .remaining_numerical_replay_events(),
        Some(1)
    );
    // The original uncertainty remains charged. Independent ordinary work may
    // proceed only after another trusted admission observation.
    assert!(fixture.port.submit(9, &fixture.proposal()).is_err());
    fixture.submit(9);
    fixture.snapshot();
    let progressed = fixture.driver.supervisor().host().unwrap().inspect();
    let files = fixture.files();
    assert_eq!(fixture.query(&mut service, command.clone()), response);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), progressed);
    assert_eq!(fixture.files(), files);
    assert_eq!(remaining(&service), (2, 0));
    // A same-grant cached retry must not destroy the later unused snapshot.
    let ticket = fixture.port.submit(10, &fixture.proposal()).unwrap();
    assert_eq!(fixture.port.poll(&ticket), Knowledge::Pending { request: 10 });
    let files = fixture.files();
    let mut changed = command;
    if let ReadArguments::AbandonUnknown { expected, .. } = &mut changed.arguments {
        expected.control_sequence += 1;
    }
    let conflict = fixture.query(&mut service, changed);
    assert_eq!(
        field(&conflict, &["result", "value", "error", "code"]).as_str(),
        Some("idempotency_conflict")
    );
    assert_eq!(fixture.files(), files);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    service.retire();
    assert!(service.active.is_none());
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn abandonment_authority_does_not_enable_intake_hold_or_unselected_reconciliation() {
    let mut fixture = fixture(3, 2);
    fixture.submit(7);
    fixture.snapshot();
    let before = fixture.files();
    let mut service = fixture.start();
    let mut proposal = fixture.request(Verb::Propose);
    proposal.idempotency_key = Some("8".into());
    proposal.arguments = ReadArguments::Propose {
        request: 8,
        proposal: fixture.proposal(),
    };
    let mut hold = fixture.request(Verb::Explain);
    hold.verb = Verb::Hold;
    hold.idempotency_key = Some("hold-7".into());
    for request in [proposal, hold] {
        let refused = fixture.query(&mut service, request);
        assert_eq!(
            field(&refused, &["envelope", "reason_code"]).as_str(),
            Some("CapabilityUnsupported")
        );
    }
    let mut legacy = next(&fixture, 902);
    legacy.arguments = ReadArguments::ResolutionNext { operation: 902 };
    let refused = fixture.query(&mut service, legacy);
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    let mut cancel = next(&fixture, 902);
    cancel.arguments = ReadArguments::CancellationNext { operation: 902 };
    let refused = fixture.query(&mut service, cancel);
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    assert_eq!(remaining(&service), (3, 2));
    assert_eq!(fixture.files(), before);
    // The refused operator intake did not consume the actor's native view.
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(
        fixture.port.poll(&ticket),
        Knowledge::Pending { request: 8 }
    );
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}


#[test]
fn stale_abandonment_and_dispatching_requests_preserve_work_and_liability() {
    let mut original = fixture(4, 2);
    original.submit(7);
    unknown(&mut original, 7);
    let mut service = original.start();
    let plan = original.query(&mut service, next(&original, 903));
    let stale = action(&original, &plan, 903);
    {
        let mut host = original.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1_001)).unwrap();
    }
    let files = original.files();
    let response = original.query(&mut service, stale);
    assert_eq!(
        field(&response, &["result", "value", "error", "code"]).as_str(),
        Some("stale_predecessor")
    );
    assert_eq!(original.files(), files);
    assert_eq!(remaining(&service), (3, 2));
    // The neighboring permitted cut can still enter this original grant.
    let plan = original.query(&mut service, next(&original, 903));
    let fresh = action(&original, &plan, 903);
    known(&original.query(&mut service, fresh), "abandonment");
    assert_eq!(remaining(&service), (2, 1));

    let mut pending = fixture(2, 1);
    pending.submit(7);
    let attempt = dispatch(&mut pending, 7);
    let state = pending.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.control.ledger.stages[&attempt], ActionState::Dispatching);
    let files = pending.files();
    let mut other = pending.start();
    let plan = pending.query(&mut other, next(&pending, 904));
    assert_eq!(
        field(&plan, &["result", "value", "knowledge", "value", "block_reason"]).as_str(),
        Some("not_unknown")
    );
    assert!(field(&plan, &["result", "value", "knowledge", "value", "action"]).is_null());
    let refusal = pending.query(&mut other, command(&pending, &plan, 904));
    assert_eq!(
        field(&refusal, &["result", "value", "error", "code"]).as_str(),
        Some("unsupported")
    );
    assert_eq!(remaining(&other), (1, 1));
    assert_eq!(pending.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(pending.files(), files);
    assert_eq!(pending.config.source.status().read_attempts, 0);
}

#[test]
fn existing_profiles_do_not_acquire_abandonment_and_replay_limits_preserve_the_owner() {
    let mut old = Fixture::new(1, 0);
    old.submit(7);
    let mut service = old.start();
    let before = old.files();
    let response = old.query(&mut service, next(&old, 905));
    assert_eq!(
        field(&response, &["result", "kind"]).as_str(),
        Some("refused")
    );
    assert!(service.active.as_ref().unwrap().abandonment.is_none());
    assert_eq!(old.files(), before);

    let mut limited = fixture(1, 1);
    limited
        .settings
        .abandonment
        .as_mut()
        .unwrap()
        .max_replay_events = 1;
    limited.submit(7);
    unknown(&mut limited, 7);
    let before = limited.files();
    let state = limited.driver.supervisor().host().unwrap().inspect();
    let mut service = limited.start();
    let plan = limited.query(&mut service, next(&limited, 906));
    assert_eq!(
        field(
            &plan,
            &["result", "value", "knowledge", "value", "block_reason"]
        )
        .as_str(),
        Some("replay_limit")
    );
    let response = limited.query(&mut service, command(&limited, &plan, 906));
    assert_eq!(
        field(&response, &["result", "value", "error", "code"]).as_str(),
        Some("capacity")
    );
    assert_eq!(remaining(&service), (0, 1));
    assert_eq!(limited.files(), before);
    assert_eq!(limited.driver.supervisor().host().unwrap().inspect(), state);
}
