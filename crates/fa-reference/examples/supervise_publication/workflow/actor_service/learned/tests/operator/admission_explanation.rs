//! Reconstruct original learned admission before any congress starts, using the
//! actual operator service and document client. Numerical disclosure cannot
//! replace a policy refusal, helper review, fresh source work or the human key.
use super::*;
use crate::workflow::actor_service::tower::client::operator_command;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_EXPLANATION_EVENTS;
use fa_reference::action::ActionState;
use fa_reference::tower::publication::request_address_for;

fn admission_selection(
    root: &Root,
    actor: &Profile,
    numerical_events: Option<usize>,
) -> (PathBuf, live_operator::Selection) {
    let (path, _) = selection(root, actor);
    let numerical = numerical_events.map_or_else(String::new, |events| {
        format!("\"explanation_numerical_replay_events\":{events},")
    });
    let original = fs::read_to_string(&path).unwrap();
    let document = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"explanation_queries\":0", "\"explanation_queries\":2")
        .replace(
            "\"explanation_replay_events\":0",
            &format!("\"explanation_replay_events\":{MAX_EXPLANATION_EVENTS}"),
        )
        .replace(
            "\"explanation_disclosure\":\"none\"",
            "\"explanation_disclosure\":\"policy_and_selected_witnesses\"",
        )
        .replacen('{', &format!("{{{numerical}"), 1);
    fs::write(&path, document).unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    (path, selected)
}

fn exchange_documents(
    service: &mut live_operator::Service,
    profile: &Path,
    documents: &[PathBuf],
) -> Vec<Json> {
    let mut arguments = vec![
        "tower-operator-exchange".to_owned(),
        profile.display().to_string(),
    ];
    arguments.extend(documents.iter().map(|path| path.display().to_string()));
    let peer = thread::spawn(move || {
        let mut output = Vec::new();
        operator_command(&arguments, &mut output).unwrap();
        output
    });
    let started = Instant::now();
    while !peer.is_finished() {
        assert!(started.elapsed() < Duration::from_secs(25));
        service.checkpoint();
        thread::sleep(Duration::from_millis(1));
    }
    peer.join().unwrap()
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| strict_json::parse(line, KNOWLEDGE_JSON_LIMITS).unwrap())
        .collect()
}

fn explanation_state(response: &Json, expected: &str) {
    assert_eq!(field(response, &["result", "kind"]).as_str(), Some("explanation"));
    assert_eq!(
        field(response, &["result", "value", "knowledge", "state"]).as_str(),
        Some(expected)
    );
    assert_eq!(
        field(response, &["result", "value", "current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!(
        field(response, &["result", "value", "effect_authority"]).as_bool(),
        Some(false)
    );
    assert_eq!(
        field(response, &["result", "value", "current_hold_reason", "state"]).as_str(),
        Some("unknown")
    );
}

#[test]
fn real_learned_admission_and_exact_denial_are_explained_before_any_congress() {
    for allowed in [true, false] {
        let root = Root::new();
        let mut config = configured(&root);
        let path = fixture::write(&root, b"allow");
        evidence(&root, &config, 1, allowed, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        let mut wire = ActorWire::new(port.clone());
        let deadline = super::super::deadline();
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        assert!(generate(
            &mut driver, &reviewer, &mut config, &mut control, &deadline,
            &mut || ElapsedTick(1000),
        ).unwrap());
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline,
            &mut || ElapsedTick(1000)).unwrap();

        // This is the original source-derived admission, before helper review.
        // A denied action still has its own frozen policy/evidence decision.
        let owner = driver.supervisor().host().unwrap().inspect();
        let numerical = driver.supervisor().host().unwrap()
            .learned_generation_inspection().unwrap();
        assert_eq!(numerical.numerical.sampled_draws, 2);
        let expected_state = if allowed { ActionState::Reviewing } else { ActionState::Denied };
        assert_eq!(owner.control.ledger.stages[&1], expected_state);
        assert_eq!(owner.executions, 0);
        assert_eq!((owner.control.ledger.charged, owner.control.ledger.reserved), (0, 0));
        let original_outcome = super::super::response(&mut wire, loaded.request);
        if allowed {
            assert_eq!(original_outcome.result, Ok(Knowledge::Pending { request: loaded.request }));
        } else {
            assert!(matches!(original_outcome.result,
                Ok(Knowledge::Known { value: ActorOutcome::Denied, .. })));
        }
        let required = usize::try_from(owner.revision).unwrap();
        assert!(required > 1 && required <= MAX_EXPLANATION_EVENTS);
        let journal = fs::read(config.store.join("delivery.bin")).unwrap();
        let source_work = config.source.status();
        let source_path = root.0.join("evidence.json");
        let source_bytes = fs::read(&source_path).unwrap();
        // A live read of these current bytes would fail. Historical disclosure
        // must reconstruct the retained original admission instead.
        fs::write(&source_path, b"unavailable current source").unwrap();
        let profile = actor_profile(&root, &config, loaded.request);
        let peers = fixture::peers(&root, &config);
        let mount = Address::new(profile.scope.tenant.to_string(), Noun::Run,
            profile.scope.run.to_string(), Some(GENERATION)).unwrap();
        let address = request_address_for(profile.scope, &mount, loaded.request).unwrap();
        let documents: Vec<_> = (0..3).map(|index| {
            let request = TowerRequest::new(format!("learned-admission-{index}"),
                address.clone(), Verb::Explain);
            let path = root.0.join(format!("admission-{index}.json"));
            fs::write(&path, encode_request(&request).unwrap()).unwrap();
            path
        }).collect();

        for allowance in [None, Some(required - 1), Some(required)] {
            // Each grant is independently provisioned by this trusted host.
            // Within a grant, reconnecting cannot restore any numerical work.
            let (operator_path, selected) = admission_selection(&root, &profile, allowance);
            selected.check(&config, &loaded, &profile, &peers, None).unwrap();
            let mut service = selected.start(&profile, port.clone(), &driver, &deadline).unwrap();
            let first = exchange_documents(&mut service, &operator_path, &documents[..1]);
            assert_eq!(first.len(), 1);
            let expected = if allowance == Some(required) { "known" } else { "unknown" };
            explanation_state(&first[0], expected);
            if expected == "known" {
                let report = field(&first[0], &["result", "value", "knowledge", "value"]);
                assert_eq!(field(report, &["initial_state"]).as_str(),
                    Some(if allowed { "reviewing" } else { "denied" }));
                assert_eq!(field(report, &["ending_state"]).as_str(),
                    Some(if allowed { "reviewing" } else { "denied" }));
                assert_eq!(field(report, &["result"]).as_str(),
                    Some(if allowed { "satisfied" } else { "violated" }));
                assert_eq!(field(report, &["witness_values_disclosed"]).as_bool(), Some(false));
                assert!(!field(report, &["predicate_tree", "nodes"]).as_array().unwrap().is_empty());
                let basis = field(&first[0], &["result", "value", "knowledge", "basis"]);
                assert_eq!(field(basis, &["request"]).as_str(), Some("1"));
                assert_eq!(field(basis, &["journal_revision"]).as_str(),
                    Some(required.to_string().as_str()));
            }
            let remaining = exchange_documents(&mut service, &operator_path, &documents[1..]);
            assert_eq!(remaining.len(), 2);
            explanation_state(&remaining[0], "unknown");
            assert_eq!(field(&remaining[1], &["result", "value", "error", "code"]).as_str(),
                Some("capacity"));
            service.retire();
            assert!(!selected.settings.socket.exists());
            assert!(!peers.socket(loaded.request).exists());
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), journal);
            assert_eq!(config.source.status(), source_work);
            assert_eq!(fs::read(&source_path).unwrap(), b"unavailable current source");
            assert_eq!(driver.supervisor().host().unwrap().inspect(), owner);
            assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection().unwrap(),
                numerical);
            assert_eq!(super::super::response(&mut wire, loaded.request).result,
                original_outcome.result);
        }
        fs::write(&source_path, source_bytes).unwrap();

        if allowed {
            // Historical disclosure did not provide either approval key. The
            // original congress and separately approved human offer still run.
            let reviewed = review::run(driver, &mut config, &loaded, &reviewer,
                &mut control, &deadline, &mut || ElapsedTick(1000));
            assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
            driver = reviewed.driver;
            let history = reviewed.history.unwrap();
            super::super::assert_native(&history);
            assert!(matches!(driver.step_computed_from_policy_file(
                &mut config.source, || ElapsedTick(1000), None).result.unwrap(),
                FileDriverEvent::AwaitingHuman { .. }));
            let offer = driver.request_learned_human_approval_from_policy_file(
                &mut config.source, loaded.rounds[0], ElapsedTick(50_000),
                || ElapsedTick(1000)).result.unwrap();
            let human = {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                reviewer.approve(&mut host, revision, &offer).unwrap()
            };
            assert!(matches!(driver.step_computed_from_policy_file(
                &mut config.source, || ElapsedTick(1000), Some(&human)).result.unwrap(),
                FileDriverEvent::Dispatched { .. }));
            assert!(matches!(driver.step_computed_from_policy_file(
                &mut config.source, || ElapsedTick(1000), None).result.unwrap(),
                FileDriverEvent::PublicationChecked { .. }));
            assert!(matches!(driver.step_computed_from_policy_file(
                &mut config.source, || ElapsedTick(1000), None).result.unwrap(),
                FileDriverEvent::Reconciled { .. }));
            assert!(super::super::executed(&mut wire, loaded.request));
            let state = driver.supervisor().host().unwrap().inspect();
            assert_eq!((state.executions, state.payload), (1, b"A".to_vec()));
            assert_eq!(state.control.ledger.reserved, 0);
            assert!(state.control.ledger.charged > 0);
            drop(history);
        } else {
            assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        }
        assert_eq!(cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms), 0);
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
