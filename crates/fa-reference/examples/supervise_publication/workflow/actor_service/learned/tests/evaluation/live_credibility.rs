//! The original live learned owner retains one paid operator grant beside its
//! separately authenticated evaluator and human reviewer. Fixture verdicts test
//! service composition; they do not establish helper-quality qualification.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS;
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const GENERATION: u64 = 41;

fn operator_profile(root: &Root, actor: &Profile, numerical: bool) -> PathBuf {
    let path = root.0.join("operator.json");
    let scope = actor.scope;
    let identity = actor.supervisor;
    let paid = if numerical {
        format!("\"numerical_replay_events\":{MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS},")
    } else {
        String::new()
    };
    fs::write(&path, format!(
        r#"{{"schema":"fa.tower-operator-service/1",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "socket":"{}/peers/operator.sock",
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "operator":{{"uid":{},"gid":{},"pid":{}}},
        "candidate_limit":2,"connection_limit":2,"exchange_limit":4,
        "response_bytes":65536,"runtime_ms":60000,"poll_ms":1,
        "inspection_queries":0,"explanation_queries":0,"explanation_replay_events":0,
        "explanation_disclosure":"none",
        "credibility_promotion":{{"queries":2,"max_replay_events":{MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS},
        {paid}"authority":"promote_qualified_credibility"}}}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        root.0.display(),
        identity.uid(), identity.gid(), identity.pid().unwrap(),
        identity.uid(), identity.gid(), identity.pid().unwrap(),
    )).unwrap();
    path
}

fn next(selected: &operator::Selection) -> Json {
    wait_for(&selected.settings.socket);
    let mut socket = UnixStream::connect(&selected.settings.socket).unwrap();
    selected.settings.supervisor.verify(&socket).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(25)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(25)))
        .unwrap();
    let scope = selected.settings.scope;
    let address = Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(GENERATION),
    )
    .unwrap();
    let mut request = TowerRequest::new("joint-before-human", address, Verb::Next);
    request.arguments = ReadArguments::CredibilityPromotionNext { operation: 401 };
    let mut request = encode_request(&request).unwrap().into_bytes();
    request.push(b'\n');
    socket.write_all(&request).unwrap();
    let mut response = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        socket.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
        assert!(response.len() <= selected.settings.response_bytes + 1);
        if byte[0] == b'\n' {
            break;
        }
    }
    strict_json::parse(&response, KNOWLEDGE_JSON_LIMITS).unwrap()
}

#[test]
fn original_joint_learned_service_publishes_only_with_human_key_and_accepts_independent_truth() {
    for approve in [false, true] {
        let root = Root::new();
        let config = configured(&root);
        let path = fixture::write(&root, b"allow");
        evidence(&root, &config, 1, true, false);
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        let reviewer = fixture::peers(&root, &config);
        let actor = peers::actor_profile(&root, &config, 1);
        let mut child = EvaluatorChild::start(&root, "assess");
        let (evaluation, accounting) =
            selected_profile(&root, &config, &mut loaded, &actor, child.0.id(), true);
        let policy = andon_settings::Settings::read(&accounting).unwrap().joint;
        let operator_path = operator_profile(&root, &actor, true);
        let operator = operator::Selection::read(&operator_path, GENERATION).unwrap();
        operator.settings.check_learned_operator().unwrap();
        // The resident admission shares the same original profile pair; it
        // cannot issue a different joint contract for a later actor.
        evaluation
            .check_sequence_actors(&config, &[&actor], &reviewer, Some(&operator))
            .unwrap();
        let observed = Arc::new(AtomicBool::new(false));
        let ready = Arc::clone(&observed);
        let review_socket = reviewer.socket(1);
        let observer = std::thread::spawn(move || {
            wait_for(&review_socket);
            let selected = operator::Selection::read(&operator_path, GENERATION).unwrap();
            let plan = next(&selected);
            assert_eq!(field(&plan, &["envelope", "status"]).as_str(), Some("ok"));
            assert_eq!(
                field(&plan, &["result", "kind"]).as_str(),
                Some("credibility-promotion-next")
            );
            assert_eq!(
                field(&plan, &["result", "value", "knowledge", "state"]).as_str(),
                Some("known")
            );
            assert_eq!(
                field(
                    &plan,
                    &["result", "value", "knowledge", "value", "can_promote"]
                )
                .as_bool(),
                Some(false)
            );
            assert_eq!(
                number(
                    &plan,
                    &[
                        "result",
                        "value",
                        "knowledge",
                        "basis",
                        "evaluation_revision"
                    ]
                ),
                1
            );
            ready.store(true, Ordering::Release);
            plan
        });
        let due = plus(crate::workflow::clock(), 55_000).unwrap();
        let mut submitted = None;
        let mut approved = None;
        let cycle = run_retained_with_operator(
            config,
            &loaded,
            &reviewer,
            &actor,
            &evaluation,
            Some(&operator),
            || {
                if submitted.is_none() && actor.socket.exists() {
                    let peer = peers::actor(
                        actor.clone(),
                        LearnedTextRelease::Message,
                        due,
                        Behavior::Observe,
                    );
                    let finished = root.0.join("actor.finished");
                    submitted = Some(std::thread::spawn(move || {
                        let response = peer.join().unwrap();
                        fs::write(finished, b"original learned actor ended").unwrap();
                        response
                    }));
                }
                if approved.is_none()
                    && observed.load(Ordering::Acquire)
                    && reviewer.socket(1).exists()
                {
                    approved = Some(peers::human(
                        reviewer.clone(),
                        1,
                        if approve {
                            ReviewDecision::Approve
                        } else {
                            ReviewDecision::Reject
                        },
                    ));
                }
                crate::workflow::clock()
            },
        )
        .unwrap();
        fs::write(root.0.join("service.finished"), b"original service ended").unwrap();
        child.finish();
        let plan = observer.join().unwrap();
        assert!(cycle.failure.is_none(), "{:?}", cycle.failure);
        let response = submitted.unwrap().join().unwrap();
        assert_eq!(response.result, cycle.response.result);
        let packet = approved
            .expect("original separate human review")
            .join()
            .unwrap();
        assert_eq!(packet.views().len(), 2);
        assert_eq!(
            ReleaseFrame::decode(&packet.action().spec().payload)
                .unwrap()
                .message(),
            Some("A")
        );
        {
            let host = cycle.driver.supervisor().host().unwrap();
            let saved = host.inspect();
            assert_eq!(host.joint_credibility_policy().unwrap(), policy);
            assert_eq!(host.credibility_report().unwrap().revision, 2);
            assert_eq!(host.credibility_report().unwrap().pending_cases, 0);
            assert_eq!(host.credibility_report().unwrap().benign_origins, 1);
            assert_eq!(
                host.learned_generation_inspection()
                    .unwrap()
                    .numerical
                    .sampled_draws,
                2
            );
            assert_eq!(saved.executions, u64::from(approve));
            assert_eq!(
                saved.payload,
                if approve { b"A".to_vec() } else { Vec::new() }
            );
            assert!(host.credibility_promotion(401).is_err());
            let receipt = read_json(&root.0.join("receipt.json"));
            assert_eq!(
                number(&receipt, &["before", "control_sequence"]),
                number(&receipt, &["after", "control_sequence"])
            );
            assert_eq!(
                number(
                    &plan,
                    &[
                        "result",
                        "value",
                        "knowledge",
                        "basis",
                        "congress_generation"
                    ]
                ),
                1
            );
            assert_eq!(
                host.history_anchor().unwrap(),
                archive::load(&root.0.join("retained.anchor")).unwrap()
            );
        }
        for name in [
            "evidence.json",
            "model.json",
            "weights.safetensors",
            "native-roster.json",
        ] {
            assert!(!root.0.join(name).exists(), "late truth reopened {name}");
        }
        assert!(!operator.settings.socket.exists());
        assert!(!evaluation.peer.socket_path().exists());
        assert_eq!(cleanup(cycle.driver, 1000, 1), 0);
        drop(cycle.history);
    }
}

#[test]
fn missing_joint_or_numerical_grant_refuses_before_live_learned_owner_work() {
    for joint in [false, true] {
        let root = Root::new();
        let config = configured(&root);
        let path = fixture::write(&root, b"allow");
        evidence(&root, &config, 1, true, false);
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        let actor = peers::actor_profile(&root, &config, 1);
        let reviewer = fixture::peers(&root, &config);
        let accounting = if joint {
            joint_accounting_file(&root)
        } else {
            accounting_file(&root)
        };
        let peer = peer_file(
            &root,
            &actor,
            std::process::id().checked_add(100_000).unwrap(),
        );
        let operator_path = operator_profile(&root, &actor, !joint);
        let operator = operator::Selection::read(&operator_path, GENERATION);
        let (remaining, options) =
            evaluation::take_options(&super::args(&root, &accounting, &peer), None).unwrap();
        let selected = options.unwrap().read().unwrap();
        let (_, history) = archive::take_options(&remaining).unwrap();
        history.admit(&config, &mut loaded, false).unwrap();
        if !joint {
            let operator = operator.as_ref().unwrap();
            assert!(
                selected
                    .check(
                        &config,
                        &loaded,
                        Some(&actor),
                        &reviewer,
                        (Some(operator), None),
                        false,
                        None,
                        None,
                        None
                    )
                    .is_err()
            );
            assert!(
                selected
                    .check_sequence_actors(&config, &[&actor], &reviewer, Some(operator),)
                    .is_err()
            );
        } else {
            assert!(operator.is_err());
        }
        let mut args =
            super::super::mediated_topology::command_inputs(&root, &config, &path, &actor);
        args.extend([
            "--andon".into(),
            accounting.display().to_string(),
            "--evaluator-profile".into(),
            peer.display().to_string(),
            "--retain-history-anchor".into(),
            root.0.join("never.anchor").display().to_string(),
            "--operator-profile".into(),
            operator_path.display().to_string(),
            "--tower-generation".into(),
            GENERATION.to_string(),
        ]);
        let source = fs::read(config.source.path()).unwrap();
        let error = command_with_clock(&args, None, || {
            panic!("incomplete learned joint/operator profiles acquired a clock")
        })
        .unwrap_err();
        assert!(
            error.contains(if joint {
                "explicit numerical replay budget"
            } else {
                "requires the original joint evaluation/Andon profile"
            }),
            "{error}"
        );
        assert!(!config.store.exists());
        assert!(!actor.socket.exists());
        assert!(!root.0.join("peers/operator.sock").exists());
        assert!(!selected.peer.socket_path().exists());
        assert!(!root.0.join("never.anchor").exists());
        assert_eq!(fs::read(config.source.path()).unwrap(), source);
    }
}
