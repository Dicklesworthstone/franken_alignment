//! Real retained learned review, original operator custody and fresh human keys.
//! Fixture models exercise protocol behavior, not detector or deployment quality.
use super::*;
use fa_reference::action::Scope;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::sidecar::FileLearnedSidecarFinish,
    driver::native_learned::FileNativeSupervisedDriver,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use fa_reference::tower::ReadArguments;

const FIRST: u64 = 101;
const SECOND: u64 = 102;

fn two_rounds(root: &Root) -> PathBuf {
    let path = fixture::write(root, b"allow");
    let document = fixture::recipe(root)
        .replacen("fa.learned-publication/1", "fa.learned-publication/2", 1)
        .replacen("\"rounds\":1", "\"rounds\":2", 1)
        .replacen("\"sidecar\":{",
            "\"sidecar\":{\"priority\":[{\"layer\":1,\"side\":\"key\",\"position\":3,\"head\":0}],", 1)
        .replacen("\"native_review\":{", "\"native_review\":{\"round_ids\":[101,102],", 1)
        .replacen("\"evaluations\":2", "\"evaluations\":4", 1)
        .replacen(&format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 2),
            &format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 4), 1)
        .replacen("\"sampling_entries\":8192", "\"sampling_entries\":16384", 1);
    fs::write(&path, document).unwrap();
    path
}

fn chosen(root: &Root, actor: &Profile) -> live_operator::Selection {
    let (path, _) = selection(root, actor);
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"exchange_limit\":16", "\"exchange_limit\":64")
        .replacen('{',
            "{\"convening\":{\"queries\":32,\"operations\":1,\"authority\":\"convene_held_review\"},", 1))
        .unwrap();
    live_operator::Selection::read(&path, GENERATION).unwrap()
}

fn remote(selected: &live_operator::Selection) -> live_operator::Selection {
    live_operator::Selection {
        settings: selected.settings.clone(),
        generation: selected.generation,
    }
}

fn request_address(scope: Scope) -> Address {
    Address::parse(&format!("fa://{}/attempt/request-{}-{}-{}-{}-1@{GENERATION}",
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority)).unwrap()
}

fn next(scope: Scope) -> TowerRequest {
    let mut request = TowerRequest::new("learned-held-next", request_address(scope), Verb::Next);
    request.arguments = ReadArguments::ConveneNext { round: SECOND };
    request
}

fn number(value: &Json, path: &[&str]) -> u64 {
    field(value, path).as_str().unwrap().parse().unwrap()
}

fn value<'a>(reply: &'a Json, kind: &str) -> &'a Json {
    assert_eq!(field(reply, &["result", "kind"]).as_str(), Some(kind));
    assert_eq!(field(reply, &["result", "value", "knowledge", "state"]).as_str(), Some("known"));
    field(reply, &["result", "value", "knowledge", "value"])
}

fn convene(scope: Scope, plan: &Json) -> TowerRequest {
    let plan = value(plan, "convene-next");
    assert_eq!(field(plan, &["can_convene"]).as_bool(), Some(true));
    let action = field(plan, &["action"]);
    let args = field(action, &["arguments"]);
    let basis = field(args, &["expected"]);
    assert_eq!(number(basis, &["held_round"]), FIRST);
    let mut request = TowerRequest::new(field(action, &["operation_id"]).as_str().unwrap(),
        Address::parse(field(action, &["address"]).as_str().unwrap()).unwrap(), Verb::Convene);
    request.idempotency_key = Some(field(action, &["idempotency_key"]).as_str().unwrap().into());
    request.arguments = ReadArguments::Convene {
        round: number(args, &["round"]),
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
            recorded_at: field(basis, &["recorded_at"]).as_str()
                .map(|tick| ElapsedTick(tick.parse().unwrap())),
            deadline: ElapsedTick(number(basis, &["deadline"])),
        },
    };
    assert_eq!(&strict_json::parse(encode_request(&request).unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS).unwrap(), action);
    request
}

fn exchange(socket: &mut UnixStream, request: &TowerRequest) -> Json {
    let mut bytes = encode_request(request).unwrap().into_bytes();
    bytes.push(b'\n');
    socket.write_all(&bytes).unwrap();
    let mut response = Vec::new();
    loop {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
        assert!(response.len() <= MAX_REQUEST_BYTES * 4);
        if byte[0] == b'\n' { break; }
    }
    strict_json::parse(&response, KNOWLEDGE_JSON_LIMITS).unwrap()
}

fn queue(selected: &live_operator::Selection, remove_source: Option<PathBuf>)
    -> thread::JoinHandle<TowerRequest>
{
    let selected = remote(selected);
    thread::spawn(move || {
        let mut socket = connect(&selected);
        let planned = exchange(&mut socket, &next(selected.settings.scope));
        let request = convene(selected.settings.scope, &planned);
        if let Some(path) = remove_source { fs::remove_file(path).unwrap(); }
        let admitted = exchange(&mut socket, &request);
        assert_eq!(field(value(&admitted, "convene"), &["outcome"]).as_str(), Some("pending"));
        request
    })
}

fn exchange_serviced(service: &mut live_operator::Service,
    selected: &live_operator::Selection, request: &TowerRequest) -> Json
{
    let selected = remote(selected);
    let request = request.clone();
    let client = thread::spawn(move || exchange(&mut connect(&selected), &request));
    finish_client(service, client)
}

fn finish_client<T>(service: &mut live_operator::Service, client: thread::JoinHandle<T>) -> T {
    let started = Instant::now();
    while !client.is_finished() {
        assert!(started.elapsed() < Duration::from_secs(25));
        service.checkpoint();
        thread::sleep(Duration::from_millis(1));
    }
    let result = client.join().unwrap();
    service.checkpoint();
    result
}

struct Held {
    config: Config,
    loaded: Loaded,
    reviewer: FileHumanReviewer,
    peers: PeerProfile,
    run: FileNativeSupervisedDriver,
    wire: Wire,
    deadline: Deadline,
    control: Control,
    selected: live_operator::Selection,
    service: Option<live_operator::Service>,
    now: ElapsedTick,
}

fn held(root: &Root, polls: usize, selected: bool) -> Held {
    let mut config = configured(root);
    let path = two_rounds(root);
    evidence(root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    loaded.native_limits.polls = polls;
    let peers = fixture::peers(root, &config);
    let actor = actor_profile(root, &config, 1);
    let selection = chosen(root, &actor);
    selection.check(&config, &loaded, &actor, &peers, None).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let deadline = deadline();
    let service = selected.then(|| selection.start(&actor, port.clone(), &driver, &deadline).unwrap());
    let mut wire = ActorWire::new(port);
    let mut control = Control::new(&config, 1, Some(&peers)).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded,
        &deadline, &mut || ElapsedTick(1000)).unwrap();
    let mut run = review::start(driver, &mut config, &loaded,
        &deadline, &mut || ElapsedTick(1000)).unwrap();
    // Original missing-member semantics produce Hold. No fake receipt or
    // replacement evaluator is supplied; both future models were frozen above.
    run.step_from_policy_file(&mut config.source, || ElapsedTick(1000), None).result.unwrap();
    let now = run.review().round().window.reveal_by;
    run.step_from_policy_file(&mut config.source, || now, None).result.unwrap();
    assert_eq!(run.review().status(), NativeReviewStatus::Finished);
    assert_eq!(run.review().history().len(), 1);
    assert!(matches!(&run.review().history()[0],
        FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. }
        if receipt.policy.control.decision.consequence == Consequence::HoldEffect));
    assert_eq!(run.held_successor_round().unwrap().round, SECOND);
    assert!(!run.review().records()[&SECOND].started);
    assert!(!peers.socket(1).exists());
    Held { config, loaded, reviewer, peers, run, wire, deadline, control,
        selected: selection, service, now }
}

#[test]
fn original_learned_service_convenes_fixed_successor_then_requires_fresh_human_key() {
    for approve in [false, true] {
        let root = Root::new();
        let mut f = held(&root, 4096, true);
        let numerical = f.run.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        let action = f.run.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let reservation = f.run.review().reservation();
        let old_polls = f.run.review().polls();
        let operator = queue(&f.selected, None);
        let reviewed = review::run_started(f.run, &mut f.config, &f.loaded, &f.reviewer,
            &mut f.control, &f.deadline, &mut || f.now, &mut None, None, None, &mut f.service);
        let request = finish_client(f.service.as_mut().unwrap(), operator);
        assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
        assert!(!reviewed.stop_requested && reviewed.process.is_none());
        let mut driver = reviewed.driver;
        let history = reviewed.history.unwrap();
        assert_eq!(history.reservation(), reservation);
        assert_eq!(history.history().len(), 2);
        assert!(history.polls() > old_polls);
        assert_eq!(history.polls(), history.records().values().map(|record| record.polls).sum::<usize>());
        assert!(history.records()[&SECOND].started && history.records()[&SECOND].completed);
        assert!(matches!(&history.history()[1],
            FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. }
            if receipt.policy.control.decision.consequence == Consequence::Continue));
        assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
        {
            let host = driver.supervisor().host().unwrap();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            assert_eq!(host.request_action(1).unwrap(), &action);
            assert_eq!((host.inspect().executions, host.inspect().control.ledger.charged), (0, 0));
        }
        // The actual human socket is created only after the fresh native round.
        let mut human_thread = None;
        let approval = {
            let mut ingress = |driver: &mut FileSupervisedDriver| {
                live_operator::checkpoint_driver(&mut f.service, driver);
                Ok(false)
            };
            let mut live = LiveControl { stop: f.control.share(), ingress: &mut ingress };
            let mut session = review_session::Session::new(&f.config, 1, Some(&f.peers)).unwrap();
            session.review(&mut driver, &f.reviewer, &mut f.config, &f.deadline, &mut live,
                &mut || {
                    if human_thread.is_none() && f.peers.socket(1).exists() {
                        human_thread = Some(human(f.peers.clone(), 1,
                            if approve { ReviewDecision::Approve } else { ReviewDecision::Reject }));
                    }
                    f.now
                }).unwrap()
        };
        let packet = human_thread.expect("new independent human listener").join().unwrap();
        assert_eq!(packet.action(), &action);
        assert_eq!(approval.is_some(), approve);
        if approve {
            for _ in 0..3 {
                driver.step_computed_from_policy_file(&mut f.config.source, || f.now,
                    approval.as_ref()).result.unwrap();
            }
        } else {
            cancel_unspent(&mut driver, 1).unwrap();
        }
        assert_eq!(executed(&mut f.wire, 1), approve);
        {
            let host = driver.supervisor().host().unwrap();
            let state = host.inspect();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
            assert_eq!(state.executions, u64::from(approve));
            assert_eq!(state.control.ledger.charged, if approve { action.spec().units } else { 0 });
            assert_eq!(state.control.ledger.reserved, 0);
            assert_eq!(state.payload, if approve { b"A".to_vec() } else { Vec::new() });
        }
        let before = driver.supervisor().host().unwrap().revision();
        let reads = f.config.source.status().read_attempts;
        let replay = exchange_serviced(f.service.as_mut().unwrap(), &f.selected, &request);
        assert_eq!(field(value(&replay, "convene"), &["outcome"]).as_str(), Some("opened"));
        assert_eq!(exchange_serviced(f.service.as_mut().unwrap(), &f.selected, &request), replay);
        assert_eq!(driver.supervisor().host().unwrap().revision(), before);
        assert_eq!(f.config.source.status().read_attempts, reads);
        f.service.as_mut().unwrap().retire();
        drop(f.control);
        assert_eq!(cleanup(driver, f.config.timing.cleanup_ms, f.config.timing.poll_ms), 0);
    }
}

#[test]
fn original_learned_service_does_not_retry_without_live_grant_or_after_fixed_window() {
    for mode in ["unselected", "revoked", "window", "actor_cancelled"] {
        let root = Root::new();
        let mut f = held(&root, 4096, mode != "unselected");
        let polls = f.run.review().polls();
        let numerical = f.run.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        let reads = f.config.source.status().read_attempts;
        if mode == "revoked" { f.service.as_mut().unwrap().retire(); }
        if mode == "window" { f.now = f.run.held_successor_round().unwrap().window.commit_by; }
        if mode == "actor_cancelled" {
            let cancelled = f.wire.exchange(&encode_command(&Command::Cancel { request: 1 }).unwrap());
            assert!(matches!(cancelled.result,
                Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        }
        let reviewed = review::run_started(f.run, &mut f.config, &f.loaded, &f.reviewer,
            &mut f.control, &f.deadline, &mut || f.now, &mut None, None, None, &mut f.service);
        assert!(reviewed.failure.is_none(), "{mode}: {:?}", reviewed.failure);
        let history = reviewed.history.unwrap();
        assert_eq!(history.polls(), polls);
        assert_eq!(history.history().len(), 1);
        assert!(!history.records()[&SECOND].started);
        assert_eq!(f.config.source.status().read_attempts, reads);
        let mut driver = reviewed.driver;
        assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical, numerical);
        cancel_unspent(&mut driver, 1).unwrap();
        assert!(!executed(&mut f.wire, 1));
        if let Some(service) = &mut f.service { service.retire(); }
        drop(f.control);
        assert_eq!(cleanup(driver, f.config.timing.cleanup_ms, f.config.timing.poll_ms), 0);
    }
}

#[test]
fn original_fixed_wait_ends_when_spent_but_executes_an_already_accepted_last_operation() {
    for abandoned in [false, true] {
        let root = Root::new();
        let mut f = held(&root, 4096, true);
        // The actual authenticated operator admits its only operation before
        // the native wrapper resumes. No test-only grant or receipt is used.
        let operator = queue(&f.selected, None);
        let request = finish_client(f.service.as_mut().unwrap(), operator);
        assert_eq!(f.service.as_ref().unwrap().remaining_convene_operations(), 0);
        assert!(f.run.has_fixed_successor());
        if abandoned {
            drop(f.run.take_pending_convene().unwrap().unwrap());
            assert!(f.run.held_successor_round().is_some());
            assert!(!f.run.has_fixed_successor());
        }
        let numerical = f.run.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        let revision = f.run.supervisor().host().unwrap().revision();
        let polls = f.run.review().polls();
        let reservation = f.run.review().reservation();
        let reads = f.config.source.status().read_attempts;
        let mut calls = 0;
        let reviewed = review::run_started(f.run, &mut f.config, &f.loaded, &f.reviewer,
            &mut f.control, &f.deadline, &mut || {
                calls += 1;
                if abandoned {
                    assert_eq!(calls, 1, "an exhausted grant without pending work must not enter a fixed-window wait");
                }
                f.now
            }, &mut None, None, None, &mut f.service);
        assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
        assert!(!reviewed.stop_requested && reviewed.process.is_none());
        let history = reviewed.history.unwrap();
        assert_eq!(history.reservation(), reservation);
        assert_eq!(history.history().len(), if abandoned { 1 } else { 2 });
        assert_eq!(history.records()[&SECOND].started, !abandoned);
        let mut driver = reviewed.driver;
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!((host.inspect().executions, host.inspect().control.ledger.charged), (0, 0));
        if abandoned {
            assert_eq!(calls, 1);
            assert_eq!(history.polls(), polls);
            assert_eq!(f.config.source.status().read_attempts, reads);
            assert_eq!(host.revision(), revision);
        } else {
            assert!(history.polls() > polls);
            assert!(f.config.source.status().read_attempts > reads);
            assert!(matches!(&history.history()[1],
                FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. }
                if receipt.policy.control.decision.consequence == Consequence::Continue));
            assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
        }
        drop(host);
        let revision = driver.supervisor().host().unwrap().revision();
        let reads = f.config.source.status().read_attempts;
        let replay = exchange_serviced(f.service.as_mut().unwrap(), &f.selected, &request);
        assert_eq!(field(value(&replay, "convene"), &["outcome"]).as_str(),
            Some(if abandoned { "abandoned" } else { "opened" }));
        assert_eq!(exchange_serviced(f.service.as_mut().unwrap(), &f.selected, &request), replay);
        assert_eq!(f.service.as_ref().unwrap().remaining_convene_operations(), 0);
        assert_eq!(driver.supervisor().host().unwrap().revision(), revision);
        assert_eq!(f.config.source.status().read_attempts, reads);
        cancel_unspent(&mut driver, 1).unwrap();
        assert!(!executed(&mut f.wire, 1));
        f.service.as_mut().unwrap().retire();
        drop(f.control);
        assert_eq!(cleanup(driver, f.config.timing.cleanup_ms, f.config.timing.poll_ms), 0);
    }
}

#[test]
fn learned_held_source_loss_and_original_poll_limit_never_replenish_review_work() {
    for remove_source in [false, true] {
        let root = Root::new();
        let mut f = held(&root, if remove_source { 4096 } else { 4 }, true);
        let numerical = f.run.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        let polls = f.run.review().polls();
        let reservation = f.run.review().reservation();
        let operator = queue(&f.selected, remove_source.then(|| f.config.source.path().to_path_buf()));
        let reviewed = review::run_started(f.run, &mut f.config, &f.loaded, &f.reviewer,
            &mut f.control, &f.deadline, &mut || f.now, &mut None, None, None, &mut f.service);
        finish_client(f.service.as_mut().unwrap(), operator);
        assert!(reviewed.failure.is_some());
        let history = reviewed.history.unwrap();
        assert_eq!(history.reservation(), reservation);
        assert_eq!(history.history().len(), 1);
        if remove_source {
            assert_eq!(history.polls(), polls);
            assert!(!history.records()[&SECOND].started);
        } else {
            assert_eq!(history.polls(), f.loaded.native_limits.polls);
            assert!(history.records()[&SECOND].started);
            assert!(!history.records()[&SECOND].completed);
        }
        let driver = reviewed.driver;
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!((host.inspect().executions, host.inspect().control.ledger.charged,
            host.inspect().control.ledger.reserved), (0, 0, 0));
        drop(host);
        f.service.as_mut().unwrap().retire();
        drop(f.control);
        assert_eq!(cleanup(driver, f.config.timing.cleanup_ms, f.config.timing.poll_ms), 0);
    }
}

#[test]
fn independent_stop_interrupts_a_held_native_sequence_with_partial_operator_input() {
    let root = Root::new();
    let mut f = held(&root, 4096, true);
    let polls = f.run.review().polls();
    let numerical = f.run.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    let selected = remote(&f.selected);
    let peers = f.peers.clone();
    let stopping = thread::spawn(move || {
        let mut socket = connect(&selected);
        socket.write_all(b"{\"request_id\":").unwrap();
        let mut client = peers.connect_stop_client(1).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(25));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    });
    let reviewed = review::run_started(f.run, &mut f.config, &f.loaded, &f.reviewer,
        &mut f.control, &f.deadline, &mut || f.now, &mut None, None, None, &mut f.service);
    stopping.join().unwrap();
    assert!(reviewed.stop_requested);
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let history = reviewed.history.unwrap();
    assert_eq!(history.polls(), polls);
    assert!(!history.records()[&SECOND].started);
    let driver = reviewed.driver;
    let host = driver.supervisor().host().unwrap();
    assert!(host.inspect().stop.is_some());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    drop(host);
    f.service.as_mut().unwrap().retire();
    drop(f.control);
    assert_eq!(cleanup(driver, f.config.timing.cleanup_ms, f.config.timing.poll_ms), 0);
}

#[test]
fn learned_convening_admission_requires_actual_original_native_selection() {
    for mode in ["native", "process", "raw", "owned", "undeclared_native"] {
        let root = Root::new();
        let config = configured(&root);
        let path = fixture::write(&root, b"allow");
        if mode == "process" {
            fs::write(&path, super::super::processes::process_recipe(&root, "allow")).unwrap();
        } else if mode == "raw" || mode == "owned" {
            super::super::forecast::install(&root, &path, mode == "owned", 60_000);
        }
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        if mode == "undeclared_native" {
            loaded.native = None;
        }
        let actor = actor_profile(&root, &config, 1);
        let peers = fixture::peers(&root, &config);
        let selected = chosen(&root, &actor);
        let result = selected.check(&config, &loaded, &actor, &peers, None);
        // Pinned raw and owned predictors can retain their original native
        // sequence. Neither process ballots nor a missing native owner can.
        assert_eq!(
            result.is_ok(),
            !matches!(mode, "process" | "undeclared_native"),
            "{mode}: {result:?}"
        );
        assert!(!config.store.exists());
        assert!(!selected.settings.socket.exists());
        assert!(!peers.socket(1).exists());
    }
}
