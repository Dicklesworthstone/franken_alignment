//! Exact original window registration through the authenticated document client.
//! Every mutation remains an explicit document and every reply is observational.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileAndonWindowBasis, FileSupervisorAndonWindowGrant, MAX_ANDON_WINDOW_REPLAY_EVENTS,
};
use fa_reference::round::review::andon::AndonBudget;
use fa_reference::tower::publication::{OperatorCapabilityKind, PublicationLimits};
use fa_reference::typestate_cx::PurposeTag;

#[path = "../../../../../../../../tests/support/file_andon_window.rs"]
mod andon;

struct NativeWindow {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: FileSupervisorAndonWindowGrant,
    storage: andon::Directory,
}

impl NativeWindow {
    fn new(budget: AndonBudget) -> Self {
        let (storage, host, _human, _evaluator) = andon::ready();
        let (actor, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .andon_window_port(budget, 12, MAX_ANDON_WINDOW_REPLAY_EVENTS)
            .unwrap();
        let mut tower =
            PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default())
                .unwrap();
        tower.set_andon_window(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant,
            storage,
        }
    }

    fn command(&self, operation: u64) -> TowerRequest {
        let host = self.supervisor.host().unwrap();
        let state = host.inspect();
        let report = host.credibility_report().unwrap();
        let mut request = TowerRequest::new(
            format!("register-andon-window-{operation}"),
            mount(),
            Verb::RegisterAndonWindow,
        );
        request.idempotency_key = Some(operation.to_string());
        request.arguments = ReadArguments::RegisterAndonWindow {
            operation,
            expected: FileAndonWindowBasis {
                scope: andon::profile(0).delivery.scope,
                journal_revision: state.revision,
                control_sequence: state.control.sequence,
                authority_epoch: state.control.ledger.epoch,
                evaluation_revision: report.revision,
                congress_generation: 1,
                policy_generation: report.policy_generation,
                recorded_at: state.control.ledger.elapsed,
                accounting: host.andon_ledger().unwrap().identity(),
            },
            expected_budget: self.grant.budget(),
        };
        request
    }

    fn dispatch(&mut self, request: &TowerRequest) -> String {
        let access = TowerAccess::new(
            CallerScope {
                caller_tenant: andon::profile(0).delivery.scope.tenant.to_string(),
                allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
                granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
            },
            true,
            vec![PurposeTag::Permit],
            0,
        );
        self.tower
            .exchange(&access, encode_request(request).unwrap().as_bytes())
            .to_json()
            .unwrap()
    }
}

fn next(operation: u64) -> TowerRequest {
    let mut request = TowerRequest::new("original-window-plan", mount(), Verb::Next);
    request.arguments = ReadArguments::AndonWindowNext { operation };
    request
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

fn settings(root: &Directory, budget: AndonBudget) -> PathBuf {
    let path = root.settings(credentials().pid(), 8);
    let profile = fs::read_to_string(&path).unwrap();
    let selected = format!(
        concat!(
            r#"{{"andon_window":{{"budget":{{"window":{},"start":{},"end":{},"false_stops_allowed":{},"cost_per_stop":{}}},"#,
            r#""queries":12,"max_replay_events":{},"authority":"register_original_andon_window"}},"#
        ),
        budget.window,
        budget.start,
        budget.end,
        budget.false_stops_allowed,
        budget.cost_per_stop,
        MAX_ANDON_WINDOW_REPLAY_EVENTS,
    );
    fs::write(&path, profile.replacen('{', &selected, 1)).unwrap();
    path
}

fn accepted(response: &str, request: &TowerRequest, selected: Option<AndonBudget>) -> Json {
    let parsed = parse_response(response.as_bytes(), request).unwrap();
    super::super::super::andon_window::check_profile(
        &parsed,
        request,
        andon::profile(0).delivery.scope,
        selected,
    )
    .unwrap();
    parsed
}

#[test]
fn window_plans_and_receipts_bind_the_full_predecessor_and_exact_successor() {
    let budget = andon::later_window();
    let mut native = NativeWindow::new(budget);
    let query = next(911);
    let before = native.storage.bytes();
    let plan = native.dispatch(&query);
    let parsed = accepted(&plan, &query, Some(budget));
    assert!(plan.contains("\"can_register\":true"));
    assert_eq!(native.storage.bytes(), before);
    assert_eq!(native.grant.remaining_operations(), 1);
    for selected in [
        None,
        Some(AndonBudget {
            cost_per_stop: 7,
            ..budget
        }),
    ] {
        assert!(
            super::super::super::andon_window::check_profile(
                &parsed,
                &query,
                andon::profile(0).delivery.scope,
                selected,
            )
            .is_err()
        );
    }
    for (from, to) in [
        (
            "fa.publication-andon-window-next/1",
            "fa.publication-andon-window-next/2",
        ),
        (
            "single_independently_selected_andon_window",
            "all_future_windows",
        ),
        ("live_andon_window_grant", "durable_global_operation"),
        (
            "\"window_operation\":\"911\"",
            "\"window_operation\":\"912\"",
        ),
        ("\"kind\":\"andon-window-next\"", "\"kind\":\"refused\""),
        (
            "\"verb\":\"register-andon-window\"",
            "\"verb\":\"preregister\"",
        ),
        (
            "\"credibility_disclosed\":false",
            "\"credibility_disclosed\":true",
        ),
        ("\"deployment\":\"91\"", "\"deployment\":\"0\""),
        ("\"start\":\"10\"", "\"start\":\"010\""),
        ("\"window\":\"2\"", "\"window\":\"0\""),
        ("\"can_register\":true", "\"can_register\":false"),
    ] {
        let changed = plan.replace(from, to);
        assert_ne!(changed, plan, "{from}");
        assert!(
            validate_response(changed.as_bytes(), &query).is_err(),
            "{to}"
        );
    }
    let mismatched_action = plan.replacen(
        "\"cost_per_stop\":\"18446744073709551615\"",
        "\"cost_per_stop\":\"7\"",
        1,
    );
    assert_ne!(mismatched_action, plan);
    assert!(validate_response(mismatched_action.as_bytes(), &query).is_err());
    let command = native.command(911);
    let receipt = native.dispatch(&command);
    accepted(&receipt, &command, Some(budget));
    assert!(receipt.contains("\"registered\":true"));
    for (from, to) in [
        (
            "fa.publication-andon-window-registration/1",
            "fa.publication-andon-window-registration/2",
        ),
        (
            "\"kind\":\"andon-window-registration\"",
            "\"kind\":\"refused\"",
        ),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        ("\"automatic_retry\":false", "\"automatic_retry\":true"),
        ("\"retry_safe\":false", "\"retry_safe\":true"),
        (
            "\"publication_attempted\":false",
            "\"publication_attempted\":true",
        ),
        ("\"registered\":true", "\"registered\":false"),
        (
            "\"congress_generation\":\"1\"",
            "\"congress_generation\":\"2\"",
        ),
        ("\"policy_generation\":\"1\"", "\"policy_generation\":\"2\""),
        ("\"evaluator\":\"92\"", "\"evaluator\":\"93\""),
        ("\"recorded_at\":null", "\"recorded_at\":\"0\""),
    ] {
        let changed = receipt.replace(from, to);
        assert_ne!(changed, receipt, "{from}");
        assert!(
            validate_response(changed.as_bytes(), &command).is_err(),
            "{to}"
        );
    }
    let committed = native.storage.bytes();
    assert_eq!(native.dispatch(&command), receipt);
    assert_eq!(native.storage.bytes(), committed);
    assert_eq!(native.grant.remaining_operations(), 0);

    let mut existing = NativeWindow::new(andon::initial_window());
    let before = existing.storage.bytes();
    let command = existing.command(912);
    let receipt = existing.dispatch(&command);
    accepted(&receipt, &command, Some(andon::initial_window()));
    assert!(receipt.contains("\"registered\":false"));
    assert_eq!(existing.storage.bytes(), before);
    assert_eq!(existing.grant.remaining_operations(), 0);
    assert!(
        validate_response(
            receipt
                .replace("\"registered\":false", "\"registered\":true")
                .as_bytes(),
            &command,
        )
        .is_err()
    );
}

#[test]
fn authenticated_plan_requires_a_separate_explicit_registration_and_preserves_exact_retry() {
    let root = Directory::new();
    let profile = settings(&root, andon::later_window());
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (prepared, received) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut native = NativeWindow::new(andon::later_window());
        let before = native.supervisor.host().unwrap().inspect();
        let mut replies = Vec::new();
        {
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            let query = next(913);
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
            let response = native.dispatch(&query);
            accepted(&response, &query, Some(andon::later_window()));
            socket
                .write_all(format!("{response}\n").as_bytes())
                .unwrap();
            assert_eq!(
                socket.read(&mut [0_u8; 1]).unwrap(),
                0,
                "plan caused automatic registration"
            );
            assert_eq!(native.supervisor.host().unwrap().inspect(), before);
            assert_eq!(native.grant.remaining_operations(), 1);
            replies.push(response);
        }
        let command = native.command(913);
        prepared.send(encode_request(&command).unwrap()).unwrap();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        for request in [command.clone(), command] {
            assert_eq!(
                decode_request(&read_document(&mut socket)).unwrap(),
                request
            );
            let response = native.dispatch(&request);
            accepted(&response, &request, Some(andon::later_window()));
            socket
                .write_all(format!("{response}\n").as_bytes())
                .unwrap();
            replies.push(response);
        }
        assert_eq!(socket.read(&mut [0_u8; 1]).unwrap(), 0, "unrequested retry");
        let after = native.supervisor.host().unwrap().inspect();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(after.control, before.control);
        assert_eq!(after.executions, before.executions);
        assert_eq!(native.grant.remaining_operations(), 0);
        assert_eq!(replies[1], replies[2]);
        replies
    });
    let mut output = Vec::new();
    operator_command(
        &args(&profile, &[root.document("plan.json", &next(913))]),
        &mut output,
    )
    .unwrap();
    let command = decode_request(
        received
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .as_bytes(),
    )
    .unwrap();
    let documents = [
        root.document("register.json", &command),
        root.document("explicit-retry.json", &command),
    ];
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn a_foreign_or_substituted_plan_stops_before_the_next_explicit_document() {
    for changed in ["scope", "budget"] {
        let root = Directory::new();
        let profile = settings(&root, andon::later_window());
        let listener = UnixListener::bind(root.socket()).unwrap();
        let query = next(914);
        let peer_query = query.clone();
        let server = std::thread::spawn(move || {
            let mut native = NativeWindow::new(andon::later_window());
            let saved = native.storage.bytes();
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(
                decode_request(&read_document(&mut socket)).unwrap(),
                peer_query
            );
            let original = native.dispatch(&peer_query);
            let forged = if changed == "scope" {
                original.replace("\"principal\":\"2\"", "\"principal\":\"22\"")
            } else {
                original.replace(
                    "\"cost_per_stop\":\"18446744073709551615\"",
                    "\"cost_per_stop\":\"7\"",
                )
            };
            assert_ne!(forged, original);
            // Internally consistent is insufficient: the separate connection
            // audience and budget selection must also agree.
            validate_response(forged.as_bytes(), &peer_query).unwrap();
            socket.write_all(format!("{forged}\n").as_bytes()).unwrap();
            assert_eq!(
                socket.read(&mut [0_u8; 1]).unwrap(),
                0,
                "{changed} plan was followed"
            );
            assert_eq!(native.storage.bytes(), saved);
            assert_eq!(native.grant.remaining_operations(), 1);
        });
        let documents = [
            root.document("plan.json", &query),
            root.document("later.json", &query),
        ];
        let mut output = Vec::new();
        assert!(operator_command(&args(&profile, &documents), &mut output).is_err());
        assert!(output.is_empty());
        server.join().unwrap();
    }
}

#[test]
fn malformed_or_substituted_receipts_stop_without_retrying_the_entered_mutation() {
    for changed in ["budget", "accounting"] {
        let root = Directory::new();
        let profile = settings(&root, andon::later_window());
        let listener = UnixListener::bind(root.socket()).unwrap();
        let (prepared, received) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let mut native = NativeWindow::new(andon::later_window());
            let command = native.command(915);
            prepared.send(encode_request(&command).unwrap()).unwrap();
            let before = native.supervisor.host().unwrap().revision();
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(
                decode_request(&read_document(&mut socket)).unwrap(),
                command
            );
            let original = native.dispatch(&command);
            let forged = if changed == "budget" {
                original.replace(
                    "\"cost_per_stop\":\"18446744073709551615\"",
                    "\"cost_per_stop\":\"7\"",
                )
            } else {
                original.replace("\"evaluator\":\"92\"", "\"evaluator\":\"93\"")
            };
            assert_ne!(forged, original);
            assert!(validate_response(forged.as_bytes(), &command).is_err());
            socket.write_all(format!("{forged}\n").as_bytes()).unwrap();
            assert_eq!(
                socket.read(&mut [0_u8; 1]).unwrap(),
                0,
                "invalid receipt was followed"
            );
            assert_eq!(native.supervisor.host().unwrap().revision(), before + 1);
            assert_eq!(native.grant.remaining_operations(), 0);
        });
        let command = decode_request(
            received
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
        let documents = [
            root.document("register.json", &command),
            root.document("must-not-retry.json", &command),
        ];
        let mut output = Vec::new();
        assert!(operator_command(&args(&profile, &documents), &mut output).is_err());
        assert!(output.is_empty());
        server.join().unwrap();
    }
}

#[test]
fn every_explicit_predecessor_and_budget_is_checked_before_connecting() {
    for changed in ["scope", "budget", "missing_selection", "missing_plan_selection"] {
        let root = Directory::new();
        let profile = if matches!(changed, "missing_selection" | "missing_plan_selection") {
            root.settings(credentials().pid(), 8)
        } else {
            settings(&root, andon::later_window())
        };
        let listener = UnixListener::bind(root.socket()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let native = NativeWindow::new(andon::later_window());
        let mut foreign = native.command(916);
        if let ReadArguments::RegisterAndonWindow {
            expected,
            expected_budget,
            ..
        } = &mut foreign.arguments
        {
            match changed {
                "scope" => expected.scope.principal += 1,
                "budget" => expected_budget.cost_per_stop = 7,
                _ => {}
            }
        } else {
            unreachable!();
        }
        let mut documents = vec![root.document("plan.json", &next(916))];
        if changed != "missing_plan_selection" {
            documents.push(root.document("foreign.json", &foreign));
        }
        let before = native.storage.bytes();
        let mut output = Vec::new();
        assert!(
            operator_command(&args(&profile, &documents), &mut output).is_err(),
            "{changed}"
        );
        assert!(output.is_empty());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(native.storage.bytes(), before);
        assert_eq!(native.grant.remaining_queries(), 12);
        assert_eq!(native.grant.remaining_operations(), 1);
    }
}

#[test]
fn authenticated_discovery_with_an_omitted_selection_preserves_the_exact_window_grant() {
    let root = Directory::new();
    // An older connection profile remains sufficient for observational discovery.
    let profile = root.settings(credentials().pid(), 8);
    let mut query = TowerRequest::new("window-discovery", mount(), Verb::Capabilities);
    query.arguments = ReadArguments::OperatorCapabilities;
    let document = root.document("capabilities.json", &query);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeWindow::new(andon::later_window());
        let before = native.storage.bytes();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
        let response = native.dispatch(&query);
        let parsed = parse_response(response.as_bytes(), &query).unwrap();
        operator_capabilities::check_profile(&parsed, &query, andon::profile(0).delivery.scope)
            .unwrap();
        let entries = parsed
            .get("result")
            .unwrap()
            .get("value")
            .unwrap()
            .get("configuration")
            .unwrap()
            .get("value")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(entries.len(), 17);
        let entry = entries
            .iter()
            .find(|entry| {
                entry.get("capability").and_then(Json::as_str)
                    == Some(OperatorCapabilityKind::AndonWindow.as_str())
            })
            .unwrap();
        assert_eq!(entry.get("route_connected").unwrap().as_bool(), Some(true));
        assert_eq!(entry.get("purpose_allowed").unwrap().as_bool(), Some(true));
        assert_eq!(entry.get("installed").unwrap().as_bool(), Some(true));
        let grant = entry.get("grant").unwrap();
        assert_eq!(grant.get("state").unwrap().as_str(), Some("known"));
        let counters = grant.get("value").unwrap();
        assert_eq!(
            counters.get("remaining_queries").unwrap().as_str(),
            Some("12")
        );
        assert_eq!(
            counters.get("remaining_operations").unwrap().as_str(),
            Some("1")
        );
        assert_eq!(
            counters.get("replay_event_limit").unwrap().as_str(),
            Some(MAX_ANDON_WINDOW_REPLAY_EVENTS.to_string().as_str())
        );
        assert!(counters.get("remaining_numerical_replay_events").is_none());
        for (from, to) in [
            ("\"register-andon-window\"", "\"preregister\""),
            (
                "\"remaining_operations\":\"1\"",
                "\"remaining_operations\":\"01\"",
            ),
            (
                "\"remaining_operations\":\"1\"",
                "\"remaining_operations\":1",
            ),
            ("\"replay_event_limit\"", "\"selected_window\""),
        ] {
            let changed = response.replace(from, to);
            assert_ne!(changed, response, "{from}");
            assert!(
                validate_response(changed.as_bytes(), &query).is_err(),
                "{to}"
            );
        }
        socket
            .write_all(format!("{response}\n").as_bytes())
            .unwrap();
        assert_eq!(
            socket.read(&mut [0_u8; 1]).unwrap(),
            0,
            "discovery caused a follow-up"
        );
        assert_eq!(native.storage.bytes(), before);
        assert_eq!(native.grant.remaining_queries(), 12);
        assert_eq!(native.grant.remaining_operations(), 1);
        response
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &[document]), &mut output).unwrap();
    assert_eq!(output, format!("{}\n", server.join().unwrap()).as_bytes());
}
