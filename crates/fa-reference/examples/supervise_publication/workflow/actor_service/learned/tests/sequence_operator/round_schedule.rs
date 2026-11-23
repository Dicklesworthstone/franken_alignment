//! Real resident operator traffic cannot consume another release's fixed IDs.
//! A permitted fresh round still preserves the original generation, forecasts,
//! independent human decisions, later Message/Finish schedules and Stop owner.
use super::*;
use fa_reference::action::Scope;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use std::sync::atomic::AtomicU64;

#[path = "process_convening.rs"]
mod process_convening;

fn convening(root: &Root, actor: &Profile) -> (PathBuf, live_operator::Selection) {
    let (path, _) = selection(root, actor);
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, original
        .replace("\"exchange_limit\":16", "\"exchange_limit\":1024")
        .replacen('{', "{\"convening\":{\"queries\":1024,\"operations\":1,\"authority\":\"convene_held_review\"},", 1))
        .unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    (path, selected)
}

fn request_address(scope: Scope, request: u64) -> Address {
    let registration = Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(GENERATION),
    )
    .unwrap();
    request_address_for(scope, &registration, request).unwrap()
}

fn next(address: &Address, round: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        format!("reserved-next-{round}"),
        address.clone(),
        Verb::Next,
    );
    request.arguments = ReadArguments::ConveneNext { round };
    request
}

fn value<'a>(reply: &'a Json, kind: &str) -> Option<&'a Json> {
    let result = reply.get("result")?;
    if result.get("kind")?.as_str()? != kind {
        return None;
    }
    let knowledge = result.get("value")?.get("knowledge")?;
    if knowledge.get("state")?.as_str()? != "known" {
        return None;
    }
    knowledge.get("value")
}

fn held(socket: &mut UnixStream, address: &Address, round: u64, maximum: usize) -> Json {
    let started = Instant::now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "resident Hold never became available"
        );
        let reply = exchange(socket, next(address, round), maximum);
        if value(&reply, "convene-next")
            .is_some_and(|plan| field(plan, &["can_convene"]).as_bool() == Some(true))
        {
            return reply;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn convene(address: &Address, scope: Scope, round: u64, plan: &Json) -> TowerRequest {
    let basis = field(plan, &["result", "value", "knowledge", "basis"]);
    let mut request = TowerRequest::new(
        format!("reserved-convene-{round}"),
        address.clone(),
        Verb::Convene,
    );
    request.idempotency_key = Some(round.to_string());
    request.arguments = ReadArguments::Convene {
        round,
        expected: FileConveneBasis {
            scope,
            request: number(basis, &["request"]),
            request_generation: number(basis, &["request_generation"]),
            attempt: number(basis, &["attempt"]),
            held_round: number(basis, &["held_round"]),
            journal_revision: number(basis, &["journal_revision"]),
            control_sequence: number(basis, &["control_sequence"]),
            authority_epoch: number(basis, &["authority_epoch"]),
            input_revision: number(basis, &["input_revision"]),
            recorded_at: basis
                .get("recorded_at")
                .filter(|value| !value.is_null())
                .map(|value| ElapsedTick(value.as_str().unwrap().parse().unwrap())),
            deadline: ElapsedTick(number(basis, &["deadline"])),
        },
    };
    request
}

fn open_fresh_round(
    selected: live_operator::Selection,
    request: u64,
    planned: Vec<u64>,
    frozen: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let scope = selected.settings.scope;
        let maximum = selected.settings.response_bytes;
        let address = request_address(scope, request);
        let mut socket = connect(&selected);
        held(&mut socket, &address, 501, maximum);
        frozen.store(true, Ordering::Release);
        // Re-read after freezing the supervisor clock; each refusal uses the
        // exact current original basis, not a stale or malformed command.
        for round in planned {
            let blocked = exchange(&mut socket, next(&address, round), maximum);
            let plan = value(&blocked, "convene-next").unwrap();
            assert_eq!(field(plan, &["can_convene"]).as_bool(), Some(false));
            assert_eq!(
                field(plan, &["block_reason"]).as_str(),
                Some("round_already_used")
            );
            assert!(field(plan, &["action"]).is_null());
            let attempted = exchange(
                &mut socket,
                convene(&address, scope, round, &blocked),
                maximum,
            );
            assert_eq!(
                field(&attempted, &["result", "value", "error", "code"]).as_str(),
                Some("idempotency_conflict")
            );
        }
        // One operation was funded. Opening this exact fresh ID proves that
        // none of the conflicting future-ID requests spent it or any roster.
        let plan = held(&mut socket, &address, 501, maximum);
        let command = convene(&address, scope, 501, &plan);
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(25));
            let reply = exchange(&mut socket, command.clone(), maximum);
            let outcome = value(&reply, "convene").unwrap();
            match field(outcome, &["outcome"]).as_str().unwrap() {
                "opened" => break,
                "pending" => thread::sleep(Duration::from_millis(1)),
                other => panic!("fresh resident successor was not opened: {other}"),
            }
        }
        ready.store(true, Ordering::Release);
    })
}

fn prepare(root: &Root, mode: Option<bool>) -> (Config, Vec<PathBuf>, Vec<resident::Entry>) {
    let (mut config, paths, mut entries) = selected(root, mode, None);
    config.timing.commit_ms = 1;
    config.timing.reveal_ms = 2;
    // The real initial helpers expire under the advancing supervisor clock.
    // One separately constructed original roster is funded before that Begin.
    let limits = &mut entries[0].loaded.native_limits.native;
    limits.evaluations *= 2;
    limits.scalar_products *= 2;
    limits.sampling_entries *= 2;
    assert_eq!(entries[0].loaded.rounds, [1]);
    assert_eq!(entries[1].loaded.rounds, [21, 22, 23]);
    assert_eq!(entries[2].loaded.rounds, [31, 32, 33]);
    (config, paths, entries)
}

#[test]
fn resident_fresh_successor_preserves_later_exact_message_finish_and_owned_forecast() {
    for mode in [None, Some(true)] {
        let root = Root::new();
        let (config, paths, entries) = prepare(&root, mode);
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let planned = entries
            .iter()
            .skip(1)
            .flat_map(|entry| entry.loaded.rounds.iter().copied())
            .collect();
        let (path, selected) = convening(&root, &entries[0].actor);
        let time = AtomicU64::new(1000);
        let frozen = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let observer = open_fresh_round(
            live_operator::Selection::read(&path, GENERATION).unwrap(),
            entries[0].loaded.request,
            planned,
            Arc::clone(&frozen),
            Arc::clone(&ready),
        );
        let result = resident::run_with_operator(
            config,
            entries,
            &reviewer,
            false,
            None,
            None,
            Some(&selected),
            || {
                let decision = ready
                    .load(Ordering::Acquire)
                    .then_some(ReviewDecision::Approve);
                peers.poll([Some(Actor::Observe); 3], [decision; 3]);
                ElapsedTick(if frozen.load(Ordering::Acquire) {
                    time.load(Ordering::SeqCst)
                } else {
                    time.fetch_add(1, Ordering::SeqCst)
                })
            },
        )
        .unwrap();
        observer.join().unwrap();
        let (responses, packets) = peers.join();
        // Audit fixed identities and original action binding before recovery
        // adds a fence. Later refinement slots must retain their exact IDs.
        for (index, round) in [(0, 1), (0, 501), (1, 21), (2, 31)] {
            let generation = &reader.loaded.generation;
            let cursor = if let Some(predictor) = generation.required_owned_pre_output_forecast() {
                FileOversight::begin_read_review_replay_with_owned_learned_consistency(
                    &reader.config.store,
                    &reader.config.profile,
                    generation,
                    predictor,
                    round,
                )
            } else {
                FileOversight::begin_read_review_replay_with_learned_generation(
                    &reader.config.store,
                    &reader.config.profile,
                    generation,
                    round,
                )
            }
            .unwrap();
            let audit = cursor
                .include_refinements()
                .unwrap()
                .complete_completion()
                .unwrap();
            assert_eq!(
                audit.archive().inputs.action(),
                packets[index].as_ref().unwrap().action()
            );
            if round == 1 || round == 501 {
                let fa_reference::action::consequence::delivery::persistent::observed::replay::FileReviewCompletion::Applied(applied) = audit
                    else { panic!("initial and explicitly convened rounds must have original decisions"); };
                assert_eq!(
                    applied
                        .application()
                        .as_ref()
                        .unwrap()
                        .policy
                        .control
                        .decision
                        .consequence,
                    if round == 1 {
                        Consequence::HoldEffect
                    } else {
                        Consequence::Continue
                    }
                );
            }
        }
        // Includes exact final numerical draws, three distinct human sessions,
        // three original forecasts, unchanged epoch and conserved publication.
        completed(&reader, &result, responses, packets, mode);
        quiescent(&root);
    }
}

#[test]
fn resident_stop_after_permitted_successor_blocks_next_message_and_finish() {
    let root = Root::new();
    let (config, paths, entries) = prepare(&root, Some(true));
    let reader = Reader::new(&root, &paths[0]);
    let mut peers = Peers::new(&root, &config, &entries);
    let reviewer = peers.reviewer.clone();
    let planned = entries
        .iter()
        .skip(1)
        .flat_map(|entry| entry.loaded.rounds.iter().copied())
        .collect();
    let (path, selected) = convening(&root, &entries[0].actor);
    let time = AtomicU64::new(1000);
    let frozen = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(false));
    let observer = open_fresh_round(
        live_operator::Selection::read(&path, GENERATION).unwrap(),
        entries[0].loaded.request,
        planned,
        Arc::clone(&frozen),
        Arc::clone(&ready),
    );
    let mut controller = None;
    let result = resident::run_with_operator(
        config,
        entries,
        &reviewer,
        false,
        None,
        None,
        Some(&selected),
        || {
            peers.poll([Some(Actor::Observe); 3], [None; 3]);
            if ready.load(Ordering::Acquire) && reviewer.socket(1).exists() && controller.is_none()
            {
                controller = Some(stopping(peers.profiles[0].clone(), reviewer.clone()));
            }
            ElapsedTick(if frozen.load(Ordering::Acquire) {
                time.load(Ordering::SeqCst)
            } else {
                time.fetch_add(1, Ordering::SeqCst)
            })
        },
    )
    .unwrap();
    observer.join().unwrap();
    let receipt = controller
        .expect("the fresh review still required the independent human key")
        .join()
        .unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    assert_eq!(receipt.binding.operation, 1);
    assert_eq!(result.cleanup_pending, 0);
    assert_eq!(result.responses.len(), 1);
    only_first(&peers);
    let (responses, packets) = peers.join();
    assert!(!confirmed(responses[0].as_ref().unwrap()));
    assert!(packets.iter().all(Option::is_none));
    let state = reader.snapshot();
    assert_eq!(state.stop.unwrap().request().operation, 1);
    assert_eq!(state.executions, 0);
    assert!(state.payload.is_empty());
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (0, 0)
    );
    quiescent(&root);
}
