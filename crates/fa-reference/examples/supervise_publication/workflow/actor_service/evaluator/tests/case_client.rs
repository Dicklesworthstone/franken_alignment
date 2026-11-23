//! The case client transports an actual original-owner observation through an
//! authenticated Unix peer. Synthetic wire mutations test the client boundary;
//! they are not evidence of detector quality or remote attestation.
use super::*;
use protocol::case::{self, CaseRequest};
use std::os::unix::net::UnixListener;
use std::thread::{self, JoinHandle};

/// A bounded test peer: no blocking accept/read can strand the test when the
/// client refuses before transmission. Both directions retain finite byte/work
/// bounds and one deadline; every successful request must terminate with EOF.
fn peer(
    listener: UnixListener,
    expected_client: PeerPolicy,
    response: Option<Vec<u8>>,
) -> JoinHandle<Result<Vec<u8>, String>> {
    listener.set_nonblocking(true).unwrap();
    thread::spawn(move || {
        let started = Instant::now();
        let mut calls = 0_usize;
        let mut checkpoint = || {
            calls += 1;
            if calls > MAX_IO_CALLS || started.elapsed() >= Duration::from_secs(10) {
                Err("bounded case-client test peer exhausted".to_owned())
            } else {
                Ok(())
            }
        };
        let mut socket = loop {
            checkpoint()?;
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if transient(&error) => thread::sleep(Duration::from_millis(1)),
                Err(error) => return Err(debug(error)),
            }
        };
        expected_client.verify(&socket).map_err(debug)?;
        socket.set_nonblocking(true).map_err(debug)?;
        let mut request = Vec::new();
        loop {
            checkpoint()?;
            let mut bytes = [0_u8; 4096];
            match socket.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    if request.len() + count > protocol::REQUEST_BYTES {
                        return Err("case-client request exceeded its byte bound".into());
                    }
                    request.extend_from_slice(&bytes[..count]);
                }
                Err(error) if transient(&error) => thread::sleep(Duration::from_millis(1)),
                Err(error) => return Err(debug(error)),
            }
        }
        if let Some(response) = response {
            if response.len() > protocol::RESPONSE_BYTES {
                return Err("case-client response exceeded its byte bound".into());
            }
            let mut sent = 0;
            while sent < response.len() {
                checkpoint()?;
                match socket.write(&response[sent..]) {
                    Ok(0) => return Err("case-client peer made no output progress".into()),
                    Ok(count) => sent += count,
                    Err(error) if transient(&error) => thread::sleep(Duration::from_millis(1)),
                    Err(error) => return Err(debug(error)),
                }
            }
            socket.shutdown(Shutdown::Write).map_err(debug)?;
        }
        Ok(request)
    })
}

fn native_response(fixture: &Fixture, round: u64) -> (CaseRequest, Vec<u8>) {
    let request = CaseRequest::prepare(&fixture.settings, round).unwrap();
    let host = fixture.driver.supervisor().host().unwrap();
    let response = request
        .observe(&host, fixture.evaluator.as_ref().unwrap())
        .unwrap();
    (request, response)
}

fn exchange(
    fixture: &Fixture,
    name: &str,
    round: u64,
    response: Vec<u8>,
) -> (Result<(), String>, Vec<u8>, Vec<u8>) {
    let socket = fixture.root.0.join(format!("{name}.sock"));
    let profile = fixture.root.0.join(format!("{name}.json"));
    fs::write(
        &profile,
        profile_json(&socket, &fixture.actor, fixture.actor.supervisor, 8),
    )
    .unwrap();
    let reader = peer(
        UnixListener::bind(&socket).unwrap(),
        fixture.actor.supervisor,
        Some(response),
    );
    let mut output = Vec::new();
    let result = client::command(
        &[
            "evaluator-case".to_owned(),
            profile.display().to_string(),
            round.to_string(),
        ],
        &mut output,
    );
    let received = reader.join().unwrap().unwrap();
    (result, output, received)
}

fn replace(wire: &[u8], from: &str, to: &str) -> Vec<u8> {
    let text = std::str::from_utf8(wire).unwrap();
    assert_eq!(text.matches(from).count(), 1, "ambiguous client mutation");
    text.replacen(from, to, 1).into_bytes()
}

#[test]
fn case_client_returns_actual_zero_epoch_case_without_labels_or_native_changes() {
    let mut fixture = Fixture::new();
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        credibility::review(&mut host, 1, Verdict::Allow);
        assert_eq!(host.inspect().control.ledger.epoch, 0);
    }
    let (request, response) = native_response(&fixture, 101);
    assert!(case::validate(&request, &response).unwrap());
    let before = fixture.image();
    let (result, output, received) = exchange(&fixture, "known", 101, response.clone());
    result.unwrap();
    assert_eq!(received, request.wire);
    let mut expected = response;
    expected.push(b'\n');
    assert_eq!(output, expected);
    assert!(case::validate(&request, &output).unwrap());
    assert_eq!(fixture.image(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let host = fixture.driver.supervisor().host().unwrap();
    assert!(host.evaluation_history(101).unwrap().is_empty());
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn case_client_authenticates_the_supervisor_before_sending_any_case_bytes() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let mut expected_server = fixture.actor.clone();
    expected_server.supervisor = PeerPolicy::new(
        fixture.actor.supervisor.uid(),
        fixture.actor.supervisor.gid(),
        Some(std::process::id() + 3),
    )
    .unwrap();
    let socket = fixture.root.0.join("wrong-server.sock");
    let profile = fixture.root.0.join("wrong-server.json");
    fs::write(
        &profile,
        profile_json(&socket, &expected_server, fixture.actor.supervisor, 8),
    )
    .unwrap();
    let reader = peer(
        UnixListener::bind(&socket).unwrap(),
        fixture.actor.supervisor,
        None,
    );
    let before = fixture.image();
    let mut output = Vec::new();
    let error = client::command(
        &[
            "evaluator-case".to_owned(),
            profile.display().to_string(),
            "101".to_owned(),
        ],
        &mut output,
    )
    .unwrap_err();
    assert!(error.contains("unverified supervisor"));
    assert!(reader.join().unwrap().unwrap().is_empty());
    assert!(output.is_empty());
    assert_eq!(fixture.image(), before);
}

#[test]
fn case_client_rejects_request_round_numeric_authority_and_disclosure_mutations_before_output() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let (request, response) = native_response(&fixture, 101);
    assert!(case::validate(&request, &response).unwrap());
    let before = fixture.image();
    let mutations = [
        ("\"deployment\":101", "\"deployment\":999"),
        ("\"value\":{\"round\":\"101\"", "\"value\":{\"round\":\"102\""),
        ("\"attempt\":\"1\"", "\"attempt\":\"01\""),
        ("\"attempt\":\"1\"", "\"attempt\":\"0\""),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        ("\"effect_authority\":false", "\"effect_authority\":false,\"payload\":\"private\""),
        ("\"state\":\"known\"", "\"state\":\"absent\""),
        ("fa.live-evaluator-case/1", "fa.live-evaluator-case/2"),
    ];
    for (index, (from, to)) in mutations.into_iter().enumerate() {
        let changed = replace(&response, from, to);
        let (result, output, received) =
            exchange(&fixture, &format!("mutated-{index}"), 101, changed);
        assert!(result.is_err(), "accepted mutation {to}");
        assert!(output.is_empty(), "printed unvalidated mutation {to}");
        assert_eq!(received, request.wire);
        assert_eq!(fixture.image(), before);
    }
    // A complete positive response still succeeds after the independent
    // negative exchanges; neither the case nor its original labels changed.
    let (result, output, received) = exchange(&fixture, "unchanged", 101, response);
    result.unwrap();
    assert!(case::validate(&request, &output).unwrap());
    assert_eq!(received, request.wire);
    assert_eq!(fixture.image(), before);
    let host = fixture.driver.supervisor().host().unwrap();
    assert!(host.evaluation_history(101).unwrap().is_empty());
}

#[test]
fn unknown_case_client_prints_explicit_unknown_and_returns_failure_without_a_label() {
    let mut fixture = Fixture::new();
    fixture.review(1);
    let (request, response) = native_response(&fixture, 999);
    assert!(!case::validate(&request, &response).unwrap());
    let before = fixture.image();
    let (result, output, received) = exchange(&fixture, "unknown", 999, response.clone());
    assert!(result.unwrap_err().contains("original evaluation case is unknown"));
    assert_eq!(received, request.wire);
    let mut expected = response;
    expected.push(b'\n');
    assert_eq!(output, expected);
    assert!(!case::validate(&request, &output).unwrap());
    assert_eq!(fixture.image(), before);
    let host = fixture.driver.supervisor().host().unwrap();
    assert!(host.evaluation_history(101).unwrap().is_empty());
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
}

#[test]
fn case_client_checks_closed_canonical_arguments_before_profile_io() {
    let fixture = Fixture::new();
    let missing = fixture.root.0.join("unread-missing-profile.json");
    assert!(!missing.exists());
    let profile = missing.display().to_string();
    let mut output = Vec::new();
    // Canonical identities, including full-width u64, reach the same missing
    // profile error. Invalid identities must refuse before this read boundary.
    let missing_error = client::command(
        &["evaluator-case".to_owned(), profile.clone(), "1".to_owned()],
        &mut output,
    )
    .unwrap_err();
    assert_eq!(
        client::command(
            &[
                "evaluator-case".to_owned(),
                profile.clone(),
                u64::MAX.to_string(),
            ],
            &mut output,
        )
        .unwrap_err(),
        missing_error,
    );
    for round in [
        "", "0", "00", "01", "-1", "+1", "1.0", "1e2", " 1", "1 ",
        "18446744073709551616",
    ] {
        let error = client::command(
            &[
                "evaluator-case".to_owned(),
                profile.clone(),
                round.to_owned(),
            ],
            &mut output,
        )
        .unwrap_err();
        assert_ne!(error, missing_error, "invalid round reached profile I/O: {round}");
    }
    for args in [
        Vec::new(),
        vec!["evaluator-case".to_owned(), profile.clone()],
        vec!["case".to_owned(), profile.clone(), "1".to_owned()],
        vec!["evaluator-case".to_owned(), String::new(), "1".to_owned()],
        vec!["evaluator-case".to_owned(), "--profile".to_owned(), "1".to_owned()],
        vec![
            "evaluator-case".to_owned(),
            profile,
            "1".to_owned(),
            "extra".to_owned(),
        ],
    ] {
        assert!(client::command(&args, &mut output).unwrap_err().starts_with("usage:"));
    }
    assert!(output.is_empty());
    assert!(!missing.exists());
}
