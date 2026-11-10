//! Real authenticated native admission Stop, with original retained liabilities.

use super::*;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileFenceBasis, MAX_FENCE_REPLAY_EVENTS,
};

#[path = "../../../../../../tests/support/file_oversight.rs"]
mod native;

fn selection(events: usize) -> String {
    format!(r#"{{"max_replay_events":{events},"authority":"stop_original_admission"}}"#)
}

fn with_fence(profile: &str, selected: &str) -> String {
    profile.replacen('{', &format!("{{\"fence\":{selected},"), 1)
}

fn select(fixture: &mut Fixture, events: usize) {
    fixture.settings = Settings::decode(with_fence(
        &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
        &selection(events),
    ).as_bytes()).unwrap();
    fixture.settings.candidate_limit = 16;
    fixture.settings.connection_limit = 12;
}

fn fixture(events: usize) -> Fixture {
    let mut fixture = Fixture::configured(1, 0, |config| config.profile = native::profile());
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        // Only bootstrap time exists. Reserve is installed before original work.
        host.enable_recovery_reserve(revision, RecoveryReserve::terminal()).unwrap();
    }
    select(&mut fixture, events);
    fixture
}

fn command(fixture: &Fixture, operation: u64) -> TowerRequest {
    let host = fixture.driver.supervisor().host().unwrap();
    let before = host.inspect();
    let mut request = fixture.request(Verb::Fence);
    request.idempotency_key = Some(operation.to_string());
    request.arguments = ReadArguments::Fence {
        operation,
        expected: FileFenceBasis {
            scope: fixture.actor.scope,
            journal_revision: before.revision,
            control_sequence: before.control.sequence,
            authority_epoch: before.control.ledger.epoch,
        },
    };
    request
}

fn remaining(service: &Service) -> usize {
    service.active.as_ref().unwrap().fence.as_ref().unwrap().remaining_operations()
}

fn dispatched(fixture: &mut Fixture) -> u64 {
    fixture.submit(7);
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(7).unwrap().disposition
    else {
        panic!("original request not admitted");
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
    attempt
}

#[test]
fn fence_profile_is_explicit_closed_and_learned_work_is_separately_budgeted() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let original = profile_json(Path::new("/operator/fence.sock"), identity, 1, 0);
    for document in [original.clone(), with_fence(&original, "null")] {
        let settings = Settings::decode(document.as_bytes()).unwrap();
        assert!(settings.fence.is_none());
        settings.check_learned_operator().unwrap();
    }
    let selected = selection(MAX_FENCE_REPLAY_EVENTS);
    let settings = Settings::decode(with_fence(
        &profile_json(Path::new("/operator/fence.sock"), identity, 0, 0),
        &selected,
    ).as_bytes()).unwrap();
    assert!(settings.resolution.is_none());
    assert_eq!(settings.inspection_queries, 0);
    assert_eq!(settings.fence.as_ref().unwrap().max_replay_events, MAX_FENCE_REPLAY_EVENTS);
    assert!(settings.check_learned_operator().is_err());
    let numerical = selected.replacen('{', "{\"numerical_replay_events\":1,", 1);
    Settings::decode(with_fence(&original, &numerical).as_bytes()).unwrap()
        .check_learned_operator().unwrap();
    for invalid in [
        "false".to_owned(), "{}".to_owned(), "[]".to_owned(),
        selection(0), selection(MAX_FENCE_REPLAY_EVENTS + 1),
        selected.replace("stop_original_admission", "permit"),
        selected.replacen('{', "{\"operations\":2,", 1),
        selected.replacen('{', "{\"clock\":123,", 1),
        selected.replacen('{', "{\"max_replay_events\":1,", 1),
        selected.replacen('{', "{\"numerical_replay_events\":null,", 1),
        selected.replacen('{', "{\"numerical_replay_events\":0,", 1),
        selection(1).replacen('{', "{\"numerical_replay_events\":2,", 1),
    ] {
        assert!(Settings::decode(with_fence(&original, &invalid).as_bytes()).is_err());
    }
}

#[test]
fn authenticated_fence_preserves_sent_charges_cancels_reserved_work_and_retries_across_connections() {
    let mut fixture = fixture(MAX_FENCE_REPLAY_EVENTS);
    let sent = dispatched(&mut fixture);
    fixture.submit(8);
    let reserved = match fixture.driver.supervisor().host().unwrap()
        .request_status(8).unwrap().disposition
    {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("second request not admitted: {other:?}"),
    };
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(before.control.ledger.charged, 8);
    assert_eq!(before.control.ledger.reserved, 8);
    let mut service = fixture.start();
    let request = command(&fixture, 901);
    let response = fixture.query(&mut service, request.clone());
    known(&response, "fence");
    assert_eq!(field(&response, &["result", "value", "scope"]).as_str(), Some("original_domain_admission"));
    assert_eq!(field(&response, &["result", "value", "drain_attempted"]).as_bool(), Some(false));
    let progress = field(&response, &["result", "value", "knowledge", "value", "progress"]);
    assert_eq!(progress.get("endpoint_fenced").and_then(Json::as_bool), Some(false));
    assert_eq!(progress.get("drained").and_then(Json::as_bool), Some(false));
    assert_eq!(progress.get("charged_units").and_then(Json::as_str), Some("8"));
    assert_eq!(progress.get("reserved_units").and_then(Json::as_str), Some("0"));
    assert_eq!(field(&response, &["result", "value", "knowledge", "value", "actor_halted", "state"]).as_str(), Some("unknown"));
    assert_eq!(field(&response, &["result", "value", "knowledge", "value", "fleet_fenced", "state"]).as_str(), Some("unknown"));
    let after = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.ledger.stages[&sent], ActionState::Unknown);
    assert_eq!(after.control.ledger.stages[&reserved], ActionState::Cancelled);
    assert_eq!(after.control.ledger.charged, 8);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.executions, 0);
    assert_eq!(after.payload, before.payload);
    assert_eq!(remaining(&service), 0);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let files = fixture.files();

    // query opens a new authenticated connection; it keeps the original grant.
    assert_eq!(fixture.query(&mut service, request.clone()), response);
    assert_eq!(fixture.files(), files);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), after);
    let mut changed = request;
    if let ReadArguments::Fence { expected, .. } = &mut changed.arguments {
        expected.journal_revision += 1;
    }
    let conflict = fixture.query(&mut service, changed);
    assert_eq!(field(&conflict, &["result", "value", "error", "code"]).as_str(), Some("idempotency_conflict"));
    let exhausted = fixture.query(&mut service, command(&fixture, 902));
    assert_eq!(field(&exhausted, &["result", "value", "error", "code"]).as_str(), Some("capacity"));
    assert!(fixture.port.submit(9, &fixture.proposal()).is_err());
    assert_eq!(fixture.files(), files);
    service.retire();
    assert!(service.active.is_none());
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn stale_predecessor_and_unselected_actor_mutation_spend_no_fence_work() {
    let mut fixture = fixture(MAX_FENCE_REPLAY_EVENTS);
    fixture.submit(7);
    let before = fixture.files();
    let state = fixture.driver.supervisor().host().unwrap().inspect();
    let mut service = fixture.start();
    let mut stale = command(&fixture, 1001);
    if let ReadArguments::Fence { expected, .. } = &mut stale.arguments {
        expected.control_sequence += 1;
    }
    let response = fixture.query(&mut service, stale);
    assert_eq!(field(&response, &["result", "value", "error", "code"]).as_str(), Some("stale_predecessor"));
    let mut proposal = fixture.request(Verb::Propose);
    proposal.idempotency_key = Some("99".into());
    proposal.arguments = ReadArguments::Propose { request: 99, proposal: fixture.proposal() };
    let response = fixture.query(&mut service, proposal);
    assert_eq!(field(&response, &["result", "kind"]).as_str(), Some("refused"));
    assert_eq!(remaining(&service), 1);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
    known(&fixture.query(&mut service, command(&fixture, 1001)), "fence");
}

#[test]
fn missing_reserve_insufficient_replay_and_old_profiles_never_stop_the_owner() {
    let mut missing = Fixture::new(1, 0);
    select(&mut missing, MAX_FENCE_REPLAY_EVENTS);
    let before = missing.files();
    assert!(Service::start(
        &missing.settings, &missing.actor, missing.port.clone(), &missing.driver,
        REGISTRATION, &deadline(),
    ).is_err());
    assert!(!missing.settings.socket.exists());
    assert_eq!(missing.files(), before);
    assert_eq!(missing.config.source.status().read_attempts, 0);

    let limited = fixture(1);
    let mut service = limited.start();
    let before = limited.files();
    let response = limited.query(&mut service, command(&limited, 1101));
    assert_eq!(field(&response, &["result", "value", "error", "code"]).as_str(), Some("capacity"));
    assert_eq!(remaining(&service), 1);
    assert_eq!(limited.files(), before);
    assert!(limited.driver.supervisor().host().unwrap().inspect().stop.is_none());

    let old = Fixture::new(1, 0);
    let mut service = old.start();
    let before = old.files();
    let response = old.query(&mut service, command(&old, 1201));
    assert_eq!(field(&response, &["result", "kind"]).as_str(), Some("refused"));
    assert!(service.active.as_ref().unwrap().fence.is_none());
    assert!(old.driver.supervisor().host().unwrap().inspect().stop.is_none());
    assert_eq!(old.files(), before);
}
