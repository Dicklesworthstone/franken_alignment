//! Native grant discovery through the authenticated operator client. Discovery
//! cannot spend the grant or relabel another audience's observations as refusal.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorInspectionGrant;

struct NativeCapabilities {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: Option<FileSupervisorInspectionGrant>,
    storage: native::Directory,
}

impl NativeCapabilities {
    fn new() -> Self {
        let storage = native::Directory::new();
        let (actor, supervisor, _reviewer, _) = native::create(&storage);
        let (port, grant) = supervisor.inspection_port(1).unwrap();
        let mut tower =
            PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
        tower.set_inspection(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant: Some(grant),
            storage,
        }
    }

    fn dispatch(&mut self, request: &TowerRequest, operator: bool) -> String {
        let before = self.supervisor.host().unwrap().inspect();
        let canonical = self.image();
        let response = self.tower.exchange(
            &access(operator),
            encode_request(request).unwrap().as_bytes(),
        );
        if request.verb == Verb::Situation {
            assert!(matches!(
                &response.result,
                PublicationResult::Situation(Ok(ActorKnowledge::Known { .. }))
            ));
        }
        assert_eq!(self.supervisor.host().unwrap().inspect(), before);
        assert_eq!(self.image(), canonical);
        response.to_json().unwrap()
    }

    fn remaining(&self) -> usize {
        self.grant.as_ref().unwrap().remaining_queries()
    }

    fn image(&self) -> Vec<u8> {
        fs::read(self.storage.store().join("delivery.bin")).unwrap()
    }
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        Vec::new(),
        0,
    )
}

fn request() -> TowerRequest {
    let mut query = TowerRequest::new("operator-grant-discovery", mount(), Verb::Capabilities);
    query.arguments = ReadArguments::OperatorCapabilities;
    query.idempotency_key = Some("discovery-observation".into());
    query
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).unwrap())
}

fn entries(value: &Json) -> &[Json] {
    field(value, &["result", "value", "configuration", "value"])
        .as_array()
        .unwrap()
}

fn entries_mut(value: &mut Json) -> &mut Vec<Json> {
    let mut value = value;
    for key in ["result", "value", "configuration", "value"] {
        let Json::Object(object) = value else {
            panic!("native discovery object");
        };
        value = object.get_mut(key).unwrap();
    }
    let Json::Array(entries) = value else {
        panic!("native discovery entries");
    };
    entries
}

fn accepted(bytes: &str, query: &TowerRequest) -> Json {
    let parsed = parse_response(bytes.as_bytes(), query).unwrap();
    operator_capabilities::check_profile(&parsed, query, native::profile().delivery.scope).unwrap();
    parsed
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

#[test]
fn authenticated_discovery_preserves_the_allowance_for_the_explicit_original_read() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8);
    let query = request();
    let situation = TowerRequest::new("explicit-original-situation", mount(), Verb::Situation);
    let expected = [query.clone(), query.clone(), situation, query];
    let documents: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, query)| root.document(&format!("discovery-{index}.json"), query))
        .collect();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeCapabilities::new();
        let canonical = native.image();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for (index, query) in expected.into_iter().enumerate() {
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
            let reply = native.dispatch(&query, true);
            let parsed = accepted(&reply, &query);
            if query.arguments == ReadArguments::OperatorCapabilities {
                let grant = field(&entries(&parsed)[0], &["grant"]);
                assert_eq!(grant.get("state").unwrap().as_str(), Some("known"));
                assert_eq!(
                    field(grant, &["value", "remaining_queries"]).as_str(),
                    Some(if index < 2 { "1" } else { "0" })
                );
            } else {
                assert_eq!(
                    field(&parsed, &["result", "kind"]).as_str(),
                    Some("situation")
                );
                assert!(reply.contains("\"state\":\"known\""));
            }
            assert_eq!(native.remaining(), usize::from(index < 2));
            assert_eq!(native.image(), canonical);
            socket.write_all(format!("{reply}\n").as_bytes()).unwrap();
            replies.push(reply);
        }
        assert_eq!(
            socket.read(&mut [0_u8]).unwrap(),
            0,
            "unrequested follow-up"
        );
        replies
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn discovery_requires_exact_schema_labels_registration_and_canonical_grant_limits() {
    let mut native = NativeCapabilities::new();
    let query = request();
    let original = native.dispatch(&query, true);
    let parsed = accepted(&original, &query);
    assert_eq!(entries(&parsed).len(), 17);
    for (from, to) in [
        ("fa.operator-capabilities/1", "fa.operator-capabilities/2"),
        (
            "\"kind\":\"operator-capabilities\"",
            "\"kind\":\"capabilities\"",
        ),
        ("\"kind\":\"operator-capabilities\"", "\"kind\":\"refused\""),
        ("mounted_operator_grants", "global_operator_authority"),
        ("host_registration", "native_effect_permission"),
        ("original_live_grant", "durable_global_grant"),
        (
            "\"current_eligibility\":\"not_observed\"",
            "\"current_eligibility\":\"known\"",
        ),
        (
            "\"unlisted_limits\":\"not_observed\"",
            "\"unlisted_limits\":\"unlimited\"",
        ),
        ("\"signed_profile\":false", "\"signed_profile\":true"),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        ("\"status\":\"ok\"", "\"status\":\"error\""),
        ("\"retry_safe\":true", "\"retry_safe\":false"),
        (
            "\"registration\":\"fa://1/run/3@11\"",
            "\"registration\":\"fa://1/run/3@12\"",
        ),
        (
            "\"registration\":\"fa://1/run/3@11\"",
            "\"registration\":\"fa://1/run/3@0\"",
        ),
        (
            "\"registration\":\"fa://1/run/3@11\"",
            "\"registration\":\"fa://1/run/3@011\"",
        ),
        (
            "\"query_address\":\"fa://1/run/3@11\"",
            "\"query_address\":\"fa://1/run/9@11\"",
        ),
        ("\"remaining_queries\":\"1\"", "\"remaining_queries\":1"),
        (
            "\"remaining_queries\":\"1\"",
            "\"remaining_queries\":\"01\"",
        ),
        (
            "\"remaining_queries\":\"1\"",
            "\"remaining_queries\":\"18446744073709551616\"",
        ),
        (
            "\"remaining_queries\":\"1\"",
            "\"remaining_queries\":\"1\",\"remaining_operations\":\"1\"",
        ),
        ("\"remaining_queries\":\"1\"", "\"unlisted_queries\":\"1\""),
        (
            "\"remaining_queries\":\"1\"",
            "\"remaining_queries\":\"1\",\"remaining_queries\":\"2\"",
        ),
        ("\"installed\":true", "\"installed\":false"),
        (
            "\"authority_required\":\"supervisor_journal_tail\"",
            "\"authority_required\":\"supervisor_inspection\"",
        ),
        ("\"purpose\":\"effect\"", "\"purpose\":\"experiment\""),
    ] {
        let damaged = original.replacen(from, to, 1);
        assert_ne!(damaged, original, "mutation missed {from}");
        assert!(
            validate_response(damaged.as_bytes(), &query).is_err(),
            "accepted {to}"
        );
    }
    for (name, number) in [
        ("tenant", 1),
        ("principal", 2),
        ("run", 3),
        ("branch", 4),
        ("authority", 5),
    ] {
        let damaged = original.replace(
            &format!("\"{name}\":\"{number}\""),
            &format!("\"{name}\":\"0\""),
        );
        assert_ne!(damaged, original);
        assert!(validate_response(damaged.as_bytes(), &query).is_err());
    }
    for (name, number) in [("principal", 2), ("branch", 4), ("authority", 5)] {
        let foreign = original.replace(
            &format!("\"{name}\":\"{number}\""),
            &format!("\"{name}\":\"{}\"", number + 10),
        );
        assert_ne!(foreign, original);
        let parsed = parse_response(foreign.as_bytes(), &query).unwrap();
        assert!(
            operator_capabilities::check_profile(&parsed, &query, native::profile().delivery.scope)
                .is_err()
        );
    }
    // The same verb cannot substitute actor-policy disclosure for operator grants.
    let mut actor_query = query.clone();
    actor_query.arguments = ReadArguments::Capabilities {
        query: 1,
        expected_policy_epoch: 0,
    };
    assert!(validate_response(original.as_bytes(), &actor_query).is_err());
    for change in 0..3 {
        let mut malformed = parsed.clone();
        let entries = entries_mut(&mut malformed);
        match change {
            0 => {
                entries.swap(0, 1);
            }
            1 => {
                entries.pop();
            }
            _ => {
                entries[1] = entries[0].clone();
            }
        }
        assert!(!operator_capabilities::matches(
            field(&malformed, &["result", "value"]),
            &query,
            field(&malformed, &["envelope"])
        ));
    }
    let mut unqualified = query;
    unqualified.address.generation = None;
    accepted(&native.dispatch(&unqualified, true), &unqualified);
    assert_eq!(native.remaining(), 1);
}

#[test]
fn discovery_preserves_unknown_withheld_and_revoked_grants_without_inventing_scope() {
    let mut native = NativeCapabilities::new();
    let query = request();
    let unknown = {
        let _busy = native.supervisor.host_mut().unwrap();
        native
            .tower
            .exchange(&access(true), encode_request(&query).unwrap().as_bytes())
            .to_json()
            .unwrap()
    };
    let parsed = accepted(&unknown, &query);
    assert_eq!(
        field(&entries(&parsed)[0], &["grant", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(
        field(&entries(&parsed)[0], &["grant", "reason"]).as_str(),
        Some("controller_unavailable")
    );
    assert_eq!(native.remaining(), 1);
    assert!(
        validate_response(
            unknown
                .replacen("\"installed\":true", "\"installed\":false", 1)
                .as_bytes(),
            &query
        )
        .is_err()
    );
    native.grant.take().unwrap().revoke();
    let revoked = accepted(&native.dispatch(&query, true), &query);
    assert_eq!(
        entries(&revoked)[0].get("installed").unwrap().as_bool(),
        Some(true)
    );
    assert_eq!(
        field(&entries(&revoked)[0], &["grant", "state"]).as_str(),
        Some("withheld")
    );
    native.tower.set_inspection(None).unwrap();
    let absent = accepted(&native.dispatch(&query, true), &query);
    assert_eq!(
        entries(&absent)[0].get("installed").unwrap().as_bool(),
        Some(false)
    );
    assert_eq!(
        field(&entries(&absent)[0], &["grant", "state"]).as_str(),
        Some("withheld")
    );
}

#[test]
fn only_closed_original_refusals_can_replace_a_discovery_response() {
    let mut native = NativeCapabilities::new();
    let query = request();
    let refused = native.dispatch(&query, false);
    accepted(&refused, &query);
    assert!(refused.contains("\"kind\":\"refused\""));
    for (from, to) in [
        (
            "\"type\":\"PublicationRefusal\"",
            "\"type\":\"OperatorCapabilities\"",
        ),
        ("\"status\":\"error\"", "\"status\":\"ok\""),
        ("\"reason_code\":\"DeniedByPolicy\"", "\"reason_code\":null"),
        (
            "\"reason_code\":\"DeniedByPolicy\"",
            "\"reason_code\":\"invented_reason\"",
        ),
        ("\"retry_safe\":false", "\"retry_safe\":true"),
        ("\"retry_safe\":false", "\"retry_safe\":\"false\""),
        (
            "\"authority_required\":\"Verify\"",
            "\"authority_required\":\"Permit\"",
        ),
        (
            "\"type\":\"PublicationRefusal\"",
            "\"type\":\"PublicationRefusal\",\"remaining_queries\":\"1\"",
        ),
    ] {
        // Withheld also appears in the envelope; change every authority label so
        // the native refusal itself is tested, not an unrelated envelope field.
        let damaged = refused.replace(from, to);
        assert_ne!(damaged, refused);
        assert!(
            validate_response(damaged.as_bytes(), &query).is_err(),
            "accepted {to}"
        );
    }
    let mut missing = query.clone();
    missing.address.id = "9".into();
    let unavailable = native.dispatch(&missing, true);
    validate_response(unavailable.as_bytes(), &missing).unwrap();
    assert!(unavailable.contains("\"state\":\"Unknown\""));
    assert_eq!(native.remaining(), 1);
}

#[test]
fn malformed_or_foreign_discovery_emits_nothing_and_blocks_the_next_explicit_document() {
    for changed in ["refused", "scope", "budget", "actor"] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4);
        let query = request();
        let later = TowerRequest::new("must-not-spend-query", mount(), Verb::Situation);
        let documents = [
            root.document("discovery.json", &query),
            root.document("later.json", &later),
        ];
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut native = NativeCapabilities::new();
            let canonical = native.image();
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
            let original = native.dispatch(&query, true);
            let forged = match changed {
                "refused" => {
                    original.replace("\"kind\":\"operator-capabilities\"", "\"kind\":\"refused\"")
                }
                "scope" => original.replace("\"principal\":\"2\"", "\"principal\":\"22\""),
                "budget" => original.replace(
                    "\"remaining_queries\":\"1\"",
                    "\"remaining_queries\":\"01\"",
                ),
                _ => original.replace(
                    "\"kind\":\"operator-capabilities\"",
                    "\"kind\":\"capabilities\"",
                ),
            };
            assert_ne!(forged, original);
            if changed == "scope" {
                // A same-tenant/run result is structurally valid but still must
                // match the independently configured principal/branch/authority.
                let parsed = parse_response(forged.as_bytes(), &query).unwrap();
                assert!(
                    operator_capabilities::check_profile(
                        &parsed,
                        &query,
                        native::profile().delivery.scope
                    )
                    .is_err()
                );
            } else {
                assert!(validate_response(forged.as_bytes(), &query).is_err());
            }
            socket.write_all(format!("{forged}\n").as_bytes()).unwrap();
            assert_eq!(
                socket.read(&mut [0_u8]).unwrap(),
                0,
                "later document followed {changed}"
            );
            assert_eq!(native.remaining(), 1);
            assert_eq!(native.image(), canonical);
        });
        let mut output = Vec::new();
        assert!(
            operator_command(&args(&profile, &documents), &mut output).is_err(),
            "{changed}"
        );
        assert!(output.is_empty());
        server.join().unwrap();
    }
}
