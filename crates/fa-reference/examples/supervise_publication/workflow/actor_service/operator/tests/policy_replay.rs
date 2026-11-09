//! Explicit replay disclosure over the original owner and authenticated socket.
//! A report remains observational across reconnects and cannot authorize effects.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_POLICY_REPLAY_QUERIES;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::policy_campaign::{MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES};

fn with_replay(profile: &str, selected: &str) -> String {
    profile.replacen('{', &format!("{{\"policy_replay\":{selected},"), 1)
}

fn selection(queries: usize, cases: usize, input_bytes: usize) -> String {
    format!(
        r#"{{"queries":{queries},"cases":{cases},"input_bytes":{input_bytes},"disclosure":"policy_and_case_traces"}}"#
    )
}

fn select(fixture: &mut Fixture, queries: usize) {
    fixture.settings = Settings::decode(
        with_replay(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
            &selection(queries, 16, 1_048_576),
        )
        .as_bytes(),
    )
    .unwrap();
}

fn candidate(maximum: usize) -> Policy {
    Policy::new(2, vec![Predicate::PayloadAtMost(maximum)]).unwrap()
}

fn request(fixture: &Fixture, campaign: u64, candidate: Policy) -> TowerRequest {
    let mut request = fixture.request(Verb::PolicyReplay);
    request.arguments = ReadArguments::PolicyReplay {
        campaign,
        expected_policy_generation: fixture.config.profile.delivery.policy.generation(),
        candidate,
    };
    request.idempotency_key = Some(campaign.to_string());
    request
}

fn remaining(service: &Service) -> usize {
    service
        .active
        .as_ref()
        .unwrap()
        .policy_replay
        .as_ref()
        .unwrap()
        .remaining_queries()
}

fn report(response: &Json, cases: usize, newly_blocked: usize, remaining: usize) {
    known(response, "policy-replay");
    let value = field(response, &["result", "value"]);
    assert_eq!(
        field(value, &["schema"]).as_str(),
        Some("fa.publication-policy-replay/1")
    );
    assert_eq!(field(value, &["historical"]).as_bool(), Some(true));
    for flag in [
        "policy_changed",
        "helpers_rerun",
        "live_evidence_read",
        "effect_authority",
        "promotion_authority",
        "payload_disclosed",
        "witness_values_disclosed",
    ] {
        assert_eq!(field(value, &[flag]).as_bool(), Some(false), "{flag}");
    }
    let report = field(value, &["knowledge", "value"]);
    assert_eq!(
        field(report, &["case_count"]).as_str().unwrap(),
        cases.to_string()
    );
    assert_eq!(
        field(report, &["cases"]).as_array().unwrap().len(),
        cases
    );
    assert_eq!(
        field(report, &["newly_blocked"]).as_array().unwrap().len(),
        newly_blocked
    );
    assert!(
        field(report, &["newly_reviewable"])
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        field(report, &["requires_shadow"]).as_bool(),
        Some(false)
    );
    assert_eq!(
        field(report, &["previous_policy", "generation"]).as_str(),
        Some("1")
    );
    assert_eq!(
        field(report, &["candidate_policy", "generation"]).as_str(),
        Some("2")
    );
    assert_eq!(
        field(report, &["remaining_campaigns_at_capture"])
            .as_str()
            .unwrap(),
        remaining.to_string()
    );
}

#[test]
fn replay_profile_is_optional_explicit_and_closed_with_original_native_bounds() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let path = Path::new("/operator/inspection.sock");
    for (inspection, explanation) in [(2, 0), (0, 2), (2, 2)] {
        let original = profile_json(path, identity, inspection, explanation);
        for document in [original.clone(), with_replay(&original, "null")] {
            let settings = Settings::decode(document.as_bytes()).unwrap();
            assert!(settings.policy_replay.is_none());
            assert_eq!(settings.inspection_queries, inspection);
            assert_eq!(settings.explanation_queries, explanation);
        }
    }
    let only = profile_json(path, identity, 0, 0);
    assert!(Settings::decode(with_replay(&only, "null").as_bytes()).is_err());
    let selected = selection(
        MAX_POLICY_REPLAY_QUERIES,
        MAX_REPLAY_CASES,
        MAX_REPLAY_INPUT_BYTES,
    );
    let settings = Settings::decode(with_replay(&only, &selected).as_bytes()).unwrap();
    assert_eq!(
        (settings.inspection_queries, settings.explanation_queries),
        (0, 0)
    );
    let replay = settings.policy_replay.unwrap();
    assert_eq!(replay.queries, MAX_POLICY_REPLAY_QUERIES);
    assert_eq!(replay.limits.cases, MAX_REPLAY_CASES);
    assert_eq!(replay.limits.input_bytes, MAX_REPLAY_INPUT_BYTES);
    assert_eq!(replay.disclosure, "policy_and_case_traces");

    let ordinary = profile_json(path, identity, 2, 0);
    let valid = selection(2, 16, 1_048_576);
    for invalid in [
        "true".to_owned(),
        "[]".to_owned(),
        "\"policy_and_case_traces\"".to_owned(),
        selection(0, 16, 1_048_576),
        selection(MAX_POLICY_REPLAY_QUERIES + 1, 16, 1_048_576),
        selection(2, 0, 1_048_576),
        selection(2, MAX_REPLAY_CASES + 1, 1_048_576),
        selection(2, 16, 0),
        selection(2, 16, MAX_REPLAY_INPUT_BYTES + 1),
        valid.replace("\"queries\":2,", ""),
        valid.replace("\"queries\":2", "\"queries\":\"2\""),
        valid.replace("policy_and_case_traces", "none"),
        valid.replace("policy_and_case_traces", "policy_and_selected_witnesses"),
        valid.replace(",\"disclosure\":\"policy_and_case_traces\"", ""),
        valid.replacen('{', "{\"unexpected\":1,", 1),
        valid.replacen('{', "{\"queries\":1,", 1),
    ] {
        assert!(
            Settings::decode(with_replay(&ordinary, &invalid).as_bytes()).is_err(),
            "accepted policy replay selection {invalid}"
        );
    }
    let duplicate = with_replay(&with_replay(&ordinary, &valid), "null");
    assert!(Settings::decode(duplicate.as_bytes()).is_err());
}

#[test]
fn authenticated_policy_replay_keeps_original_cut_and_budget_across_reconnects() {
    let mut fixture = Fixture::new(1, 0);
    fixture.submit(7);
    select(&mut fixture, 2);
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let before = fixture.files();
    let first_request = request(&fixture, 71, candidate(1));
    let mut service = fixture.start();
    let first = fixture.query(&mut service, first_request.clone());
    report(&first, 1, 1, 1);
    assert_eq!(remaining(&service), 1);
    assert!(service.active.as_ref().unwrap().inspection.is_none());
    assert!(service.active.as_ref().unwrap().explanation.is_none());
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.files(), before);

    // A later actor request advances the SAME owner while this grant and socket
    // registration survive. Its exact retry must retain the first historical cut.
    fixture.submit(8);
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let before = fixture.files();
    let retried = fixture.query(&mut service, first_request);
    report(&retried, 1, 1, 1);
    assert_eq!(
        field(&retried, &["result", "value", "knowledge"]),
        field(&first, &["result", "value", "knowledge"])
    );
    assert_eq!(remaining(&service), 1);
    let second = fixture.query(&mut service, request(&fixture, 72, candidate(512)));
    report(&second, 2, 0, 0);
    assert_ne!(
        field(
            &second,
            &["result", "value", "knowledge", "basis", "journal_revision"]
        ),
        field(
            &first,
            &["result", "value", "knowledge", "basis", "journal_revision"]
        )
    );
    assert_eq!(remaining(&service), 0);
    let exhausted = fixture.query(&mut service, request(&fixture, 73, candidate(1)));
    assert_eq!(
        field(&exhausted, &["envelope", "reason_code"]).as_str(),
        Some("BudgetExhausted")
    );
    assert_eq!(remaining(&service), 0);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .session
            .status()
            .connections_admitted,
        4
    );
    assert_eq!(service.frames, 4);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    service.retire();
    assert!(service.active.is_none());
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn rejected_kernel_identity_cannot_spend_or_observe_policy_replay() {
    let mut fixture = Fixture::new(1, 0);
    fixture.submit(7);
    select(&mut fixture, 2);
    let before = fixture.files();
    let replay = request(&fixture, 71, candidate(1));
    let mut rejected = fixture.settings.clone();
    let pid = rejected.operator.pid().unwrap();
    rejected.operator = PeerPolicy::new(
        rejected.operator.uid(),
        rejected.operator.gid(),
        Some(if pid == i32::MAX as u32 {
            pid - 1
        } else {
            pid + 1
        }),
    )
    .unwrap();
    let mut service = Service::start(
        &rejected,
        &fixture.actor,
        fixture.port.clone(),
        &fixture.driver,
        REGISTRATION,
        &deadline(),
    )
    .unwrap();
    let mut socket = UnixStream::connect(&rejected.socket).unwrap();
    let mut bytes = encode_request(&replay).unwrap().into_bytes();
    bytes.push(b'\n');
    socket.write_all(&bytes).unwrap();
    service.checkpoint();
    assert_eq!(service.candidates, 1);
    assert_eq!(service.frames, 0);
    assert_eq!(remaining(&service), 2);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .policy_replay
            .as_ref()
            .unwrap()
            .retained_bytes(),
        0
    );
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .session
            .status()
            .connections_admitted,
        0
    );
    socket.set_nonblocking(true).unwrap();
    match socket.read(&mut [0_u8]) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
            ) => {}
        other => panic!("rejected policy replay peer received transport: {other:?}"),
    }
    assert_eq!(fixture.files(), before);
    service.retire();
    let mut accepted = fixture.start();
    report(&fixture.query(&mut accepted, replay), 1, 1, 1);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn ordinary_operator_has_no_replay_disclosure_and_explicit_replay_cannot_issue_permits() {
    let mut fixture = Fixture::new(2, 0);
    fixture.submit(7);
    let replay = request(&fixture, 71, candidate(1));
    let before = fixture.files();
    let mut ordinary = fixture.start();
    let refused = fixture.query(&mut ordinary, replay.clone());
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("PurposeMismatch")
    );
    assert!(ordinary.active.as_ref().unwrap().policy_replay.is_none());
    assert_eq!(
        ordinary
            .active
            .as_ref()
            .unwrap()
            .inspection
            .as_ref()
            .unwrap()
            .remaining_queries(),
        2
    );
    assert_eq!(fixture.files(), before);
    ordinary.retire();

    select(&mut fixture, 2);
    let mut selected = fixture.start();
    known(&fixture.query(&mut selected, replay), "policy-replay");
    fixture.snapshot();
    let before = fixture.files();
    let mut proposal = fixture.request(Verb::Propose);
    proposal.arguments = ReadArguments::Propose {
        request: 8,
        proposal: fixture.proposal(),
    };
    proposal.idempotency_key = Some("8".into());
    let refused = fixture.query(&mut selected, proposal);
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("PurposeMismatch")
    );
    assert_eq!(remaining(&selected), 1);
    assert_eq!(fixture.files(), before);
    // The same unconsumed admission snapshot still belongs to the actor.
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(
        fixture.port.poll(&ticket),
        Knowledge::Pending { request: 8 }
    );
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn replay_custody_rejects_a_numerically_equal_foreign_owner_before_socket_creation() {
    let mut fixture = Fixture::new(1, 0);
    fixture.submit(7);
    select(&mut fixture, 2);
    let foreign = Fixture::new(1, 0);
    let before = fixture.files();
    let foreign_before = foreign.files();
    assert_eq!(fixture.actor.scope, foreign.actor.scope);
    assert!(
        Service::start(
            &fixture.settings,
            &fixture.actor,
            foreign.port.clone(),
            &fixture.driver,
            REGISTRATION,
            &deadline(),
        )
        .is_err()
    );
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.files(), before);
    assert_eq!(foreign.files(), foreign_before);
    let mut valid = fixture.start();
    known(
        &fixture.query(&mut valid, request(&fixture, 71, candidate(1))),
        "policy-replay",
    );
    assert_eq!(remaining(&valid), 1);
    assert_eq!(fixture.files(), before);
}

#[test]
fn cached_policy_report_cannot_keep_a_dropped_owner_or_its_authority_lock_alive() {
    let mut fixture = Fixture::new(1, 0);
    fixture.submit(7);
    select(&mut fixture, 2);
    let replay = request(&fixture, 71, candidate(1));
    let mut service = fixture.start();
    known(
        &fixture.query(&mut service, replay.clone()),
        "policy-replay",
    );
    assert_eq!(remaining(&service), 1);
    drop(fixture.driver);
    let (_reopened, _) =
        FileOversight::open(&fixture.config.store, fixture.config.profile.clone()).unwrap();
    let before = files(&fixture.config.store);
    let lost = query(&fixture.settings.socket, &mut service, replay);
    assert_eq!(
        field(&lost, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(
        field(&lost, &["result", "value", "knowledge", "reason"]).as_str(),
        Some("controller_unavailable")
    );
    assert_eq!(remaining(&service), 1);
    assert_eq!(files(&fixture.config.store), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    service.retire();
    assert!(!fixture.settings.socket.exists());
}
