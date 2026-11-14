//! Paid target restrictions in the actual learned operator and reviewer loop.
//! Synthetic model fixtures establish protocol behavior, not detector quality.
use super::*;
use crate::workflow::actor_service::tower::client::operator_command;
use fa_reference::action::{ActionState, ResolvedTarget, Scope};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    client::ReviewClientProgress, wire::ReviewPacket,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, MAX_NARROW_REPLAY_EVENTS,
};
use fa_reference::tower::ReadArguments;

fn narrowing_selection(root: &Root, actor: &Profile, events: usize)
    -> (PathBuf, live_operator::Selection)
{
    let (path, _) = selection(root, actor);
    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replacen('{', &format!(
            "{{\"narrowing\":{{\"queries\":4,\"operations\":1,\"max_replay_events\":{MAX_NARROW_REPLAY_EVENTS},\"numerical_replay_events\":{events},\"authority\":\"narrow_original_targets\"}},"
        ), 1)).unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    (path, selected)
}

fn next(scope: Scope, operation: u64, retained_targets: Vec<ResolvedTarget>) -> TowerRequest {
    let address = Address::new(scope.tenant.to_string(), Noun::Run,
        scope.run.to_string(), Some(GENERATION)).unwrap();
    let mut query = TowerRequest::new(format!("learned-narrow-next-{operation}"), address, Verb::Next);
    query.arguments = ReadArguments::NarrowingNext { operation, retained_targets };
    query
}

fn number(value: &Json) -> u64 { value.as_str().unwrap().parse().unwrap() }

fn command(plan: &Json, scope: Scope, operation: u64, targets: Vec<ResolvedTarget>) -> TowerRequest {
    assert_eq!(field(plan, &["result", "kind"]).as_str(), Some("narrowing-next"));
    let value = field(plan, &["result", "value", "knowledge", "value"]);
    assert_eq!(field(value, &["can_narrow"]).as_bool(), Some(true));
    let basis = field(plan, &["result", "value", "knowledge", "basis"]);
    let mut request = next(scope, operation, targets.clone());
    request.operation_id = format!("narrow-authority-{operation}");
    request.verb = Verb::Narrow;
    request.idempotency_key = Some(operation.to_string());
    request.arguments = ReadArguments::Narrow {
        operation,
        expected: FileNarrowBasis {
            scope,
            journal_revision: number(field(basis, &["journal_revision"])),
            control_sequence: number(field(basis, &["control_sequence"])),
            authority_epoch: number(field(basis, &["authority_epoch"])),
            recorded_at: match field(basis, &["recorded_at"]) {
                Json::Null => None,
                tick => Some(ElapsedTick(number(tick))),
            },
        },
        retained_targets: targets,
    };
    assert_eq!(&strict_json::parse(encode_request(&request).unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS).unwrap(), field(value, &["action"]));
    request
}

fn exchange(socket: &mut UnixStream, request: &TowerRequest) -> Json {
    let mut bytes = encode_request(request).unwrap().into_bytes();
    bytes.push(b'\n');
    socket.write_all(&bytes).unwrap();
    let mut response = Vec::new();
    loop {
        let mut byte = [0; 1];
        socket.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
        assert!(response.len() <= MAX_REQUEST_BYTES * 4);
        if byte[0] == b'\n' { break; }
    }
    strict_json::parse(&response, KNOWLEDGE_JSON_LIMITS).unwrap()
}

fn document_exchange(root: &Root, service: &mut live_operator::Service, profile: &Path,
    label: &str, request: &TowerRequest) -> Json
{
    let path = root.0.join(format!("{label}.json"));
    fs::write(&path, encode_request(request).unwrap()).unwrap();
    let arguments = vec!["tower-operator-exchange".to_owned(),
        profile.display().to_string(), path.display().to_string()];
    let client = thread::spawn(move || {
        let mut output = Vec::new();
        operator_command(&arguments, &mut output).unwrap();
        output
    });
    let started = Instant::now();
    while !client.is_finished() {
        assert!(started.elapsed() < Duration::from_secs(25));
        service.checkpoint();
        thread::sleep(Duration::from_millis(1));
    }
    let output = client.join().unwrap();
    service.checkpoint();
    strict_json::parse(&output, KNOWLEDGE_JSON_LIMITS).unwrap()
}

#[test]
fn original_command_renews_human_approval_after_narrowing_in_each_learned_mode() {
    for (process, predictor) in [(false, None), (true, None),
        (false, Some(false)), (false, Some(true))]
    {
        run_command(process, predictor, false);
    }
}

#[test]
fn original_command_cancels_an_excluded_learned_review_without_publishing_or_stopping() {
    run_command(false, None, true);
}

#[test]
fn original_cycle_without_actor_transport_retires_an_excluded_pending_human_review() {
    use super::super::super::{RequestServices, credential, execute_request};
    let root = Root::new();
    let mut config = configured(&root);
    let path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, loaded.request);
    let (_, selected) = narrowing_selection(&root, &profile, MAX_NARROW_REPLAY_EVENTS);
    selected.check(&config, &loaded, &profile, &peers, None).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let driver = FileSupervisedDriver::new(supervisor);
    let mut limit = deadline();
    let mut time = || ElapsedTick(1000);
    let mut credential = credential::prepare(&loaded, false, &limit, &mut time).unwrap();
    let mut service = Some(selected.start(&profile, port.clone(), &driver, &limit).unwrap());
    let mut wire = ActorWire::new(port);
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let human_profile = peers.clone();
    let operator = thread::spawn(move || {
        wait_for(&human_profile.socket(1));
        let mut pending = human_profile.connect_client(1).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(25));
            if matches!(pending.step().unwrap(), ReviewClientProgress::NeedsDecision) { break; }
            thread::sleep(Duration::from_millis(1));
        }
        let mut socket = connect(&selected);
        let plan = exchange(&mut socket, &next(profile.scope, 1503, Vec::new()));
        let request = command(&plan, profile.scope, 1503, Vec::new());
        let acknowledged = exchange(&mut socket, &request);
        assert_eq!(field(&acknowledged, &["result", "value", "knowledge", "state"]).as_str(), Some("known"));
        // No human decision exists. Dropping this withdrawn connection must
        // retire Cancelled, not wait for actor-transport observation or Stop.
        drop(pending);
    });
    let cycle = execute_request(driver, &mut config, &loaded, RequestServices {
        peers: &peers, reviewer: &reviewer, prediction: &mut None, identity: &mut None,
        credential: &mut credential, intake: &mut None, wire: &mut wire,
        operator: &mut service, finishing: None, message_continuation: None,
        credibility: None, stop_operation: loaded.request,
    }, &mut control, &mut limit, &mut time);
    operator.join().unwrap();
    assert!(cycle.failure.is_none(), "{:?}", cycle.failure);
    assert!(matches!(cycle.response.result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_native(cycle.history.as_ref().unwrap());
    let state = cycle.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!((state.executions, state.control.ledger.charged, state.control.ledger.reserved), (0, 0, 0));
    assert!(state.stop.is_none());
    assert_eq!(cycle.driver.supervisor().host().unwrap().learned_generation_inspection()
        .unwrap().numerical.sampled_draws, 2);
    service.as_mut().unwrap().retire();
    assert_eq!(cleanup(cycle.driver, config.timing.cleanup_ms, config.timing.poll_ms), 0);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

fn run_command(process: bool, predictor: Option<bool>, exclude: bool) {
    let root = Root::new();
    let config = configured(&root);
    let path = fixture::write(&root, b"allow");
    if process {
        fs::write(&path, super::super::processes::process_recipe(&root, "allow")).unwrap();
    } else if let Some(owned) = predictor {
        super::super::forecast::install(&root, &path, owned, 60_000);
    }
    evidence(&root, &config, 1, true, false);
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, 1);
    let (operator_path, selected) = narrowing_selection(&root, &profile, MAX_NARROW_REPLAY_EVENTS);
    let operator_socket = selected.settings.socket.clone();
    let mut args = super::super::mediated_topology::command_inputs(&root, &config, &path, &profile);
    args.splice(4..4, ["--tower-generation".into(), GENERATION.to_string()]);
    args.extend(["--operator-profile".into(), operator_path.display().to_string()]);
    let reviewer_profile = peers.clone();
    let target = config.profile.delivery.target;
    let scope = profile.scope;
    let reviewer = thread::spawn(move || {
        wait_for(&reviewer_profile.socket(1));
        let mut old = reviewer_profile.connect_client(1).unwrap();
        let started = Instant::now();
        let previous = loop {
            assert!(started.elapsed() < Duration::from_secs(25));
            match old.step().unwrap() {
                ReviewClientProgress::NeedsDecision => break old.packet().unwrap().clone(),
                ReviewClientProgress::Complete => panic!("old review completed without a decision"),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        };
        // Keep the old independent connection pending while the original
        // numerical owner commits this separately authorized restriction.
        let mut operator = connect(&selected);
        let targets = if exclude { Vec::new() } else { vec![target] };
        let plan = exchange(&mut operator, &next(scope, 1501, targets.clone()));
        let request = command(&plan, scope, 1501, targets);
        let acknowledged = exchange(&mut operator, &request);
        assert_eq!(field(&acknowledged, &["result", "kind"]).as_str(), Some("narrowing"));
        assert_eq!(field(&acknowledged, &["result", "value", "knowledge", "state"]).as_str(), Some("known"));
        let receipt = field(&acknowledged, &["result", "value", "knowledge", "value"]);
        assert_eq!(field(receipt, &["refunded_units"]).as_str(), Some("0"));
        assert_eq!(field(receipt, &["cancelled"]).as_array().unwrap().len(), usize::from(exclude));
        assert_eq!(exchange(&mut operator, &request), acknowledged);
        drop(old);
        if exclude { return (previous, None); }
        // SAME listening endpoint and finite candidate allowance, but a new
        // independent connection, request identity, nonce and human decision.
        let renewed = human(reviewer_profile, 1, ReviewDecision::Approve).join().unwrap();
        assert_ne!(renewed.binding().request, previous.binding().request);
        assert_ne!(renewed.binding().session, previous.binding().session);
        assert_eq!(renewed.binding().attempt, previous.binding().attempt);
        assert_eq!(renewed.control_sequence(), previous.control_sequence() + 1);
        assert_eq!(renewed.action(), previous.action());
        assert_eq!(renewed.input_revision(), previous.input_revision());
        assert_eq!(renewed.policy_generation(), previous.policy_generation());
        assert_eq!(renewed.views(), previous.views());
        (previous, Some(renewed))
    });
    let mut actor_thread = None;
    command_with_clock(&args, None, || {
        if actor_thread.is_none() && profile.socket.exists() {
            actor_thread = Some(actor(profile.clone(), LearnedTextRelease::Message,
                ElapsedTick(50_000), Behavior::Observe));
        }
        ElapsedTick(1000)
    }).unwrap();
    let (previous, renewed): (ReviewPacket, Option<ReviewPacket>) = reviewer.join().unwrap();
    let response = actor_thread.expect("original actor listener bound").join().unwrap();
    let expected = if exclude { ActorOutcome::CancelledBeforeDispatch } else { ActorOutcome::Executed };
    assert!(matches!(response.result, Ok(Knowledge::Known { value, .. }) if value == expected));
    let loaded = recipe::load(&path, &config, false).unwrap();
    let state = if let Some(predictor) = loaded.generation.required_owned_pre_output_forecast() {
        FileOversight::read_publication_with_owned_learned_consistency(&config.store,
            &config.profile, &loaded.generation, predictor).unwrap().publication
    } else {
        FileOversight::read_publication_with_learned_generation(&config.store,
            &config.profile, &loaded.generation).unwrap()
    };
    assert_eq!(state.executions, u64::from(!exclude));
    assert_eq!(state.control.ledger.stages[&1],
        if exclude { ActionState::Cancelled } else { ActionState::Confirmed });
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.charged,
        if exclude { 0 } else { previous.action().spec().units });
    assert_eq!(state.control.ledger.epoch, previous.action().spec().policy_epoch);
    assert!(state.stop.is_none());
    assert_eq!(state.payload, if exclude { Vec::new() } else { b"A".to_vec() });
    assert_eq!(renewed.is_some(), !exclude);
    assert!(!operator_socket.exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

#[test]
fn document_client_reports_short_numerical_allowance_then_exact_paid_restriction() {
    let root = Root::new();
    let mut config = configured(&root);
    let path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port.clone());
    let limit = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &limit, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &limit,
        &mut || ElapsedTick(1000)).unwrap();
    let profile = actor_profile(&root, &config, loaded.request);
    let original = driver.supervisor().host().unwrap().inspect();
    let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
    let stream = driver.supervisor().host().unwrap().stream_snapshot().unwrap();
    let journal = config.store.join("delivery.bin");
    let original_bytes = fs::read(&journal).unwrap();
    let required = original.revision as usize + 1;
    assert!(required > 1);
    let source_work = config.source.status();
    fs::write(root.0.join("evidence.json"), b"source unavailable for any new effect").unwrap();
    for allowance in [required - 1, required] {
        let (path, selected) = narrowing_selection(&root, &profile, allowance);
        let mut service = selected.start(&profile, port.clone(), &driver, &limit).unwrap();
        let plan = document_exchange(&root, &mut service, &path, "learned-narrow-plan",
            &next(profile.scope, 1502, Vec::new()));
        let value = field(&plan, &["result", "value", "knowledge", "value"]);
        if allowance < required {
            assert_eq!(field(value, &["can_narrow"]).as_bool(), Some(false));
            assert_eq!(field(value, &["block_reason"]).as_str(), Some("numerical_replay_limit"));
            assert!(field(value, &["action"]).is_null());
            assert_eq!(driver.supervisor().host().unwrap().inspect(), original);
            assert_eq!(fs::read(&journal).unwrap(), original_bytes);
        } else {
            let request = command(&plan, profile.scope, 1502, Vec::new());
            let acknowledged = document_exchange(&root, &mut service, &path,
                "learned-narrow-apply", &request);
            assert_eq!(field(&acknowledged, &["result", "value", "knowledge", "state"]).as_str(), Some("known"));
            let after = driver.supervisor().host().unwrap().inspect();
            assert_eq!(after.revision, original.revision + 1);
            assert_eq!(after.control.sequence, original.control.sequence + 1);
            assert_eq!(after.control.ledger.epoch, original.control.ledger.epoch);
            assert_eq!(after.control.ledger.stages[&1], ActionState::Cancelled);
            assert_eq!((after.executions, after.control.ledger.charged), (0, 0));
            let bytes = fs::read(&journal).unwrap();
            assert_eq!(document_exchange(&root, &mut service, &path,
                "learned-narrow-retry", &request), acknowledged);
            assert_eq!(fs::read(&journal).unwrap(), bytes);
        }
        assert_eq!(config.source.status(), source_work);
        let current = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
        assert_eq!(current.numerical, numerical.numerical);
        assert_eq!(current.paused, numerical.paused);
        assert_eq!(current.pending, numerical.pending);
        let current = driver.supervisor().host().unwrap().stream_snapshot().unwrap();
        assert_eq!(current.confirmed_target, stream.confirmed_target);
        assert_eq!(current.confirmed, stream.confirmed);
        assert_eq!(current.published, stream.published);
        assert_eq!(current.pending, stream.pending);
        service.retire();
        assert!(!selected.settings.socket.exists());
    }
    assert_eq!(cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms), 0);
}
