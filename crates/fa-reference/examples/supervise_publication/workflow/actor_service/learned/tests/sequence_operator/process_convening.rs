//! A later resident release retains the same operator, numerical owner and
//! predictor while its original process schedule crosses an actual Hold.
use super::*;

fn remaining_operations(socket: &mut UnixStream, scope: Scope, maximum: usize) -> u64 {
    let registration = Address::new(
        scope.tenant.to_string(), Noun::Run, scope.run.to_string(), Some(GENERATION),
    ).unwrap();
    let mut request = TowerRequest::new("resident-process-grant", registration, Verb::Capabilities);
    request.arguments = ReadArguments::OperatorCapabilities;
    let reply = exchange(socket, request, maximum);
    assert_eq!(field(&reply, &["result", "kind"]).as_str(), Some("operator-capabilities"));
    let entry = field(&reply, &["result", "value", "configuration", "value"])
        .as_array().unwrap().iter()
        .find(|entry| field(entry, &["capability"]).as_str() == Some("convening"))
        .expect("the original resident convening grant stays mounted");
    assert_eq!(field(entry, &["grant", "state"]).as_str(), Some("known"));
    number(entry, &["grant", "value", "remaining_operations"])
}

#[test]
fn later_resident_process_hold_preserves_future_rounds_shared_grant_and_owned_prediction() {
    let root = Root::new();
    // Exactly three forecasts fund Message, Message and Finish. A fresh
    // congress for the second action cannot draw another prediction or token.
    let (config, paths, mut entries) = selected(&root, Some(true), Some(3));
    process_second(&root, &config, &paths[1], Some(true));
    let process_recipe = fs::read_to_string(&paths[1]).unwrap()
        .replace("::tests::processes::process_child",
            "::tests::operator_tests::process_convening::process_child")
        .replace("FA_LEARNED_PUBLICATION_PROCESS", "FA_FIXED_PROCESS_MODE")
        .replace("FA_LEARNED_PUBLICATION_MEMBER", "FA_FIXED_PROCESS_MEMBER")
        .replace("FA_LEARNED_PUBLICATION_TRACE", "FA_FIXED_PROCESS_TRACE")
        // The original weight-5 alpha Hold is below run suspension; beta
        // remains Allow. All later fixed cohorts retain Allow/Allow.
        .replacen(r#""FA_FIXED_PROCESS_MODE":"allow""#,
            r#""FA_FIXED_PROCESS_MODE":"hold""#, 1);
    fs::write(&paths[1], process_recipe).unwrap();
    entries[1].loaded = recipe::load(&paths[1], &config, true).unwrap();
    assert!(entries[0].loaded.native.is_some());
    assert!(entries[1].loaded.process.is_some() && entries[1].loaded.native.is_none());
    assert!(entries[2].loaded.native.is_some());
    assert_eq!(entries[1].loaded.rounds, [21, 22, 23]);
    assert_eq!(entries[0].loaded.generation, entries[1].loaded.generation);
    let reader = Reader::new(&root, &paths[0]);
    let mut peers = Peers::new(&root, &config, &entries);
    let reviewer = peers.reviewer.clone();
    let (operator_path, selected) = convening(&root, &entries[0].actor);
    let remote = live_operator::Selection::read(&operator_path, GENERATION).unwrap();
    let request = entries[1].loaded.request;
    let future_round = entries[2].loaded.rounds[0];
    let final_actor = entries[2].actor.socket.clone();
    let second_human = reviewer.socket(request);
    let traces = ["alpha", "beta"].map(|name|
        root.0.join(format!("process-21-{name}.input")));
    let continued = Arc::new(AtomicBool::new(false));
    let final_ready = Arc::new(AtomicBool::new(false));
    let allow_human = Arc::clone(&continued);
    let allow_final = Arc::clone(&final_ready);
    let observer = thread::spawn(move || {
        let scope = remote.settings.scope;
        let maximum = remote.settings.response_bytes;
        let address = request_address(scope, request);
        let mut socket = connect(&remote);
        assert_eq!(remaining_operations(&mut socket, scope, maximum), 1);
        // Startup can take real time. Wait for both actual first-cohort inputs
        // before spending bounded Next queries on the acknowledged decision.
        for trace in traces { wait_for(&trace); }
        held(&mut socket, &address, 22, maximum);
        assert!(!second_human.exists(), "the held process action has no human key");
        let blocked = exchange(&mut socket, next(&address, future_round), maximum);
        let plan = value(&blocked, "convene-next").unwrap();
        assert_eq!(field(plan, &["can_convene"]).as_bool(), Some(false));
        assert_eq!(field(plan, &["block_reason"]).as_str(), Some("round_already_used"));
        assert!(field(plan, &["action"]).is_null());
        let refusal = exchange(&mut socket,
            convene(&address, scope, future_round, &blocked), maximum);
        assert_eq!(field(&refusal, &["result", "value", "error", "code"]).as_str(),
            Some("idempotency_conflict"));
        assert_eq!(remaining_operations(&mut socket, scope, maximum), 1);

        let plan = held(&mut socket, &address, 22, maximum);
        let command = convene(&address, scope, 22, &plan);
        let started = Instant::now();
        let opened = loop {
            assert!(started.elapsed() < Duration::from_secs(25));
            let reply = exchange(&mut socket, command.clone(), maximum);
            match field(value(&reply, "convene").unwrap(), &["outcome"]).as_str().unwrap() {
                "opened" => break reply,
                "pending" => thread::sleep(Duration::from_millis(1)),
                other => panic!("original fixed process successor was not opened: {other}"),
            }
        };
        for _ in 0..3 {
            assert_eq!(exchange(&mut socket, command.clone(), maximum), opened);
        }
        assert_eq!(remaining_operations(&mut socket, scope, maximum), 0);
        allow_human.store(true, Ordering::Release);
        // Keep the third actor unsubmitted until the same authenticated
        // operator has observed its spent grant across another real handoff.
        wait_for(&final_actor);
        assert_eq!(remaining_operations(&mut socket, scope, maximum), 0);
        assert_eq!(exchange(&mut socket, command, maximum), opened);
        allow_final.store(true, Ordering::Release);
    });
    let mut first_confirmed = false;
    let result = resident::run_with_operator(
        config, entries, &reviewer, false, None, None, Some(&selected), || {
            if !first_confirmed && peers.profiles[1].socket.exists() {
                let state = reader.snapshot();
                assert_eq!(state.executions, 1);
                assert_eq!(state.payload, b"A");
                assert_eq!(state.control.ledger.reserved, 0);
                first_confirmed = true;
            }
            peers.poll(
                [Some(Actor::Observe), Some(Actor::Observe),
                    final_ready.load(Ordering::Acquire).then_some(Actor::Observe)],
                [Some(ReviewDecision::Approve),
                    continued.load(Ordering::Acquire).then_some(ReviewDecision::Approve),
                    Some(ReviewDecision::Approve)],
            );
            ElapsedTick(1000)
        },
    ).unwrap();
    observer.join().unwrap();
    assert!(first_confirmed);
    let (responses, packets) = peers.join();
    let generation = &reader.loaded.generation;
    let predictor = generation.required_owned_pre_output_forecast().unwrap();
    for (index, round) in [(0, 1), (1, 21), (1, 22), (2, 31)] {
        let audit = FileOversight::begin_read_review_replay_with_owned_learned_consistency(
            &reader.config.store, &reader.config.profile, generation, predictor, round,
        ).unwrap().include_refinements().unwrap().complete_completion().unwrap();
        assert_eq!(audit.archive().inputs.action(),
            packets[index].as_ref().unwrap().action());
        if round == 21 || round == 22 {
            let fa_reference::action::consequence::delivery::persistent::observed::replay::FileReviewCompletion::Applied(applied) = audit
                else { panic!("both process decisions must be durably applied"); };
            assert_eq!(applied.application().as_ref().unwrap()
                .policy.control.decision.consequence,
                if round == 21 { Consequence::HoldEffect } else { Consequence::Continue });
        }
    }
    for name in ["alpha", "beta"] {
        assert_eq!(
            fs::read(root.0.join(format!("process-21-{name}.input"))).unwrap(),
            fs::read(root.0.join(format!("process-22-{name}.input"))).unwrap(),
            "both process congresses keep the same original owned sidecar",
        );
        assert!(!root.0.join(format!("process-23-{name}.input")).exists());
    }
    // Exact four numerical draws, three forecasts and distinct human sessions,
    // original epoch, confirmed Message/Message/Finish and conserved charges.
    completed(&reader, &result, responses, packets, Some(true));
    quiescent(&root);
}
