//! The actor-only operator client talks to the real supervisor-owned sidecar.
//! Synthetic source fixtures qualify neither producer authenticity nor helpers.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::decode_precheck_response;

fn operator_query(fixture: &Fixture, service: &mut Service, selected: Settings,
    request: &PrecheckRequest) -> (Result<(), String>, Vec<u8>)
{
    let path = fixture.actor.socket.parent().unwrap().join("precheck-request.json");
    fs::write(&path, encode_precheck_request(request).unwrap()).unwrap();
    let actor = fixture.actor.clone();
    let started = Instant::now();
    let client = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = super::super::client::query(&actor, &selected, &path, started, &mut bytes);
        (result, bytes)
    });
    while !client.is_finished() {
        assert!(started.elapsed() < Duration::from_secs(10), "operator precheck did not complete");
        service.checkpoint(&fixture.driver, ElapsedTick(1_000));
        std::thread::sleep(Duration::from_millis(1));
    }
    client.join().unwrap()
}

#[test]
fn actor_only_operator_client_prechecks_then_original_submit_still_requires_review() {
    let mut fixture = Fixture::new(3, 4);
    let mut service = fixture.start();
    let before = fixture.files();
    let state = fixture.driver.supervisor().host().unwrap().inspect();
    let request = fixture.request(7, b"network");
    let (result, bytes) = operator_query(&fixture, &mut service, fixture.settings.clone(), &request);
    result.unwrap();
    let response = decode_precheck_response(bytes.strip_suffix(b"\n").unwrap(), 7, 0).unwrap();
    known(&response, FilePrecheckOutcome::PrecheckPassed, 7);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(fixture.driver.supervisor().host().unwrap().retained_requests(), 0);
    assert_eq!(fixture.config.source.status().read_attempts, 1);
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 2);

    let mut wire = ActorWire::new(fixture.port.clone());
    let submitted = fixture.driver.exchange_actor_from_file(&mut wire,
        &encode_command(&Command::Submit { request: 7, proposal: request.proposal }).unwrap(),
        &mut fixture.config.source, || ElapsedTick(1_001)).unwrap();
    assert!(matches!(submitted.response.result, Ok(Knowledge::Pending { request: 7 })));
    assert_eq!(fixture.config.source.status().read_attempts, 2);
    let host = fixture.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!((host.inspect().executions, host.inspect().control.ledger.reserved), (0, 0));
    drop(host);
    service.checkpoint(&fixture.driver, ElapsedTick(1_001));
    assert_eq!(service.retirement, Some(Retirement::OwnerChanged));
    fixture.trace_case(Some(&service));
}

#[test]
fn actor_operator_rejects_the_wrong_real_disclosure_without_printing_a_pass() {
    let mut fixture = Fixture::new(2, 4);
    let mut service = fixture.start();
    let before = fixture.files();
    let state = fixture.driver.supervisor().host().unwrap().inspect();
    let request = fixture.request(7, b"network");
    let mut wrong = fixture.settings.clone(); wrong.disclosure_id = 82;
    let (result, bytes) = operator_query(&fixture, &mut service, wrong, &request);
    assert!(result.unwrap_err().contains("disclosure"));
    assert!(bytes.is_empty());
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 1);

    // An explicitly corrected context is a NEW query, not a client-side retry or
    // a refund. It reaches the same original grant and spends its final query.
    let (result, bytes) = operator_query(&fixture, &mut service, fixture.settings.clone(), &request);
    result.unwrap();
    known(&decode_precheck_response(bytes.strip_suffix(b"\n").unwrap(), 7, 0).unwrap(),
        FilePrecheckOutcome::PrecheckPassed, 7);
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 0);
    assert_eq!(service.frames, 2);
    assert_eq!(fixture.config.source.status().read_attempts, 1);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(fixture.driver.supervisor().host().unwrap().retained_requests(), 0);
    fixture.trace_case(Some(&service));
}
