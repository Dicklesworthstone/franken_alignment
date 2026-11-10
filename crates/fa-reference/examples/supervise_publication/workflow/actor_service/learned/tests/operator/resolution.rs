//! Real generated output and independent native/human approval, followed by
//! explicitly budgeted reconciliation through the ordinary operator client.
use super::*;
use crate::workflow::actor_service::tower::client::operator_command;
use fa_reference::action::{ActionState, Scope};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileHoldDiagnosisBasis, MAX_RESOLUTION_REPLAY_EVENTS,
};
use fa_reference::tower::{ReadArguments, publication::request_address_for};

fn resolution_selection(
    root: &Root,
    actor: &Profile,
    numerical_events: usize,
) -> (PathBuf, live_operator::Selection) {
    let (path, _) = selection(root, actor);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replacen(
            '{',
            &format!(
                "{{\"resolution\":{{\"queries\":4,\"operations\":2,\"max_replay_events\":{MAX_RESOLUTION_REPLAY_EVENTS},\"numerical_replay_events\":{numerical_events},\"authority\":\"reconcile_existing_receipts\"}},"
            ),
            1,
        );
    fs::write(&path, selected).unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    assert_eq!(selected.settings.inspection_queries, 0);
    assert!(selected.settings.review_explanation.is_none());
    (path, selected)
}

fn next(scope: Scope, request: u64, operation: u64) -> TowerRequest {
    let mount = Address::new(
        scope.tenant.to_string(), Noun::Run, scope.run.to_string(), Some(GENERATION),
    ).unwrap();
    let mut query = TowerRequest::new(
        format!("learned-next-{operation}"),
        request_address_for(scope, &mount, request).unwrap(),
        Verb::Next,
    );
    query.arguments = ReadArguments::ResolutionNext { operation };
    query
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn command(plan: &Json, scope: Scope, operation: u64, offered: bool) -> TowerRequest {
    let basis = field(plan, &["result", "value", "knowledge", "basis"]);
    let mut request = next(scope, number(field(basis, &["request"])), operation);
    request.operation_id = format!("resolve-{operation}");
    request.verb = Verb::Resolve;
    request.idempotency_key = Some(operation.to_string());
    request.arguments = ReadArguments::Resolve {
        operation,
        expected: FileHoldDiagnosisBasis {
            scope,
            request: number(field(basis, &["request"])),
            request_generation: number(field(basis, &["request_generation"])),
            attempt: number(field(basis, &["attempt"])),
            journal_revision: number(field(basis, &["journal_revision"])),
            control_sequence: number(field(basis, &["control_sequence"])),
            recorded_at: match field(basis, &["recorded_at"]) {
                Json::Null => None,
                value => Some(ElapsedTick(number(value))),
            },
        },
    };
    let action = field(plan, &["result", "value", "knowledge", "value", "action"]);
    if offered {
        let encoded = encode_request(&request).unwrap();
        assert_eq!(
            &strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap(),
            action,
        );
    } else {
        assert!(action.is_null());
    }
    request
}

fn exchange(
    root: &Root,
    service: &mut live_operator::Service,
    profile: &Path,
    label: &str,
    requests: &[TowerRequest],
) -> Vec<Json> {
    let mut args = vec![
        "tower-operator-exchange".to_owned(),
        profile.display().to_string(),
    ];
    for (index, request) in requests.iter().enumerate() {
        let path = root.0.join(format!("{label}-{index}.json"));
        fs::write(&path, encode_request(request).unwrap()).unwrap();
        args.push(path.display().to_string());
    }
    // Only encoded documents cross the thread boundary. The original owner,
    // service and weak native grants remain on this thread throughout.
    let client = thread::spawn(move || {
        let mut output = Vec::new();
        operator_command(&args, &mut output).map(|()| output)
    });
    let started = Instant::now();
    while !client.is_finished() {
        service.checkpoint();
        assert!(started.elapsed() < Duration::from_secs(25));
        thread::sleep(Duration::from_millis(1));
    }
    let output = client.join().unwrap().unwrap();
    service.checkpoint();
    output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| strict_json::parse(line, KNOWLEDGE_JSON_LIMITS).unwrap())
        .collect()
}

fn result_value(response: &Json, kind: &str) {
    assert_eq!(field(response, &["result", "kind"]).as_str(), Some(kind));
    assert_eq!(
        field(response, &["result", "value", "knowledge", "state"]).as_str(),
        Some("known"),
    );
    for marker in ["clock_read", "live_evidence_read", "effect_authority", "payload_disclosed"] {
        assert_eq!(
            field(response, &["result", "value", marker]).as_bool(),
            Some(false),
        );
    }
}

#[test]
fn learned_operator_reconciles_real_receipts_with_exact_budget_and_preserves_charged_misses() {
    for publish in [true, false] {
        let root = Root::new();
        let mut config = configured(&root);
        let path = fixture::write(&root, b"allow");
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let peers = fixture::peers(&root, &config);
        let actor = actor_profile(&root, &config, loaded.request);
        let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut wire = ActorWire::new(port.clone());
        let mut driver = FileSupervisedDriver::new(supervisor);
        let limit = deadline();
        let mut control = Control::new(&config, loaded.request, None).unwrap();
        assert!(generate(
            &mut driver, &reviewer, &mut config, &mut control,
            &limit, &mut || ElapsedTick(1000),
        ).unwrap());
        submit(
            &mut driver, &mut wire, &mut config, &loaded,
            &limit, &mut || ElapsedTick(1000),
        ).unwrap();
        let reviewed = review::run(
            driver, &mut config, &loaded, &reviewer, &mut control,
            &limit, &mut || ElapsedTick(1000),
        );
        assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
        assert_native(reviewed.history.as_ref().unwrap());
        let mut driver = reviewed.driver;
        let offer = driver.request_learned_human_approval_from_policy_file(
            &mut config.source, 701, ElapsedTick(50_000), || ElapsedTick(1000),
        ).result.unwrap();
        let human = {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            reviewer.approve(&mut host, revision, &offer).unwrap()
        };
        assert!(matches!(
            driver.step_computed_from_policy_file(
                &mut config.source, || ElapsedTick(1000), Some(&human),
            ).result.unwrap(),
            FileDriverEvent::Dispatched { .. },
        ));
        if publish {
            assert!(matches!(
                driver.step_computed_from_policy_file(
                    &mut config.source, || ElapsedTick(1000), None,
                ).result.unwrap(),
                FileDriverEvent::PublicationChecked { .. },
            ));
        }
        let original = driver.supervisor().host().unwrap().inspect();
        assert_eq!(original.executions, u64::from(publish));
        assert!(original.control.ledger.charged > 0);
        let generation = driver.supervisor().host().unwrap()
            .learned_generation_inspection().unwrap();
        let required = usize::try_from(original.revision).unwrap() + 1;
        assert!(required > 1 && required <= MAX_RESOLUTION_REPLAY_EVENTS);
        let journal = config.store.join("delivery.bin");
        let original_bytes = fs::read(&journal).unwrap();
        let reads = config.source.status().read_attempts;

        // The same exact original numerical owner is first offered a one-short
        // allowance. Both planning and a direct explicit command refuse before
        // changing the native image or entering any new numerical generation.
        let (profile_path, selected) = resolution_selection(&root, &actor, required - 1);
        selected.check(&config, &loaded, &actor, &peers, None).unwrap();
        let mut service = selected.start(&actor, port.clone(), &driver, &limit).unwrap();
        let query = next(actor.scope, loaded.request, 901);
        let plans = exchange(&root, &mut service, &profile_path, "short-next", &[query.clone()]);
        result_value(&plans[0], "resolution-next");
        assert_eq!(
            field(&plans[0], &["result", "value", "knowledge", "value", "block_reason"]).as_str(),
            Some("numerical_replay_limit"),
        );
        let attempted = command(&plans[0], actor.scope, 901, false);
        let refused = exchange(&root, &mut service, &profile_path, "short-resolve", &[attempted]);
        assert_eq!(
            field(&refused[0], &["result", "value", "error", "code"]).as_str(),
            Some("capacity"),
        );
        assert_eq!(fs::read(&journal).unwrap(), original_bytes);
        assert_eq!(driver.supervisor().host().unwrap().inspect(), original);
        assert_eq!(
            driver.supervisor().host().unwrap().learned_generation_inspection().unwrap(),
            generation,
        );
        drop(service);

        // Explicit replacement provisioning supplies exactly one full-cut
        // reservation. A healthy reconnect must never replenish this custodian.
        let (profile_path, selected) = resolution_selection(&root, &actor, required);
        let mut service = selected.start(&actor, port.clone(), &driver, &limit).unwrap();
        let plans = exchange(&root, &mut service, &profile_path, "next", &[query]);
        result_value(&plans[0], "resolution-next");
        let resolve = command(&plans[0], actor.scope, 901, true);
        assert_eq!(fs::read(&journal).unwrap(), original_bytes);
        let acknowledged = exchange(
            &root, &mut service, &profile_path, "resolve", &[resolve.clone()],
        );
        result_value(&acknowledged[0], "resolution");
        let expected_stage = if publish { "confirmed" } else { "unknown" };
        assert_eq!(
            field(&acknowledged[0], &["result", "value", "knowledge", "value", "stage"]).as_str(),
            Some(expected_stage),
        );
        assert_eq!(
            field(&acknowledged[0], &["result", "value", "knowledge", "value", "outcome", "kind"]).as_str(),
            Some(if publish { "resolved" } else { "awaiting_resolution" }),
        );
        let settled = driver.supervisor().host().unwrap().inspect();
        assert_eq!(settled.revision, original.revision + 1);
        assert_eq!(settled.executions, original.executions);
        assert_eq!(settled.payload, original.payload);
        assert_eq!(settled.control.ledger.charged, original.control.ledger.charged);
        let settled_bytes = fs::read(&journal).unwrap();
        let repeated = exchange(&root, &mut service, &profile_path, "repeat", &[resolve]);
        assert_eq!(repeated, acknowledged);
        assert_eq!(fs::read(&journal).unwrap(), settled_bytes);
        let later = exchange(
            &root, &mut service, &profile_path, "later",
            &[next(actor.scope, loaded.request, 902)],
        );
        assert_eq!(
            field(&later[0], &["result", "value", "knowledge", "value", "block_reason"]).as_str(),
            Some(if publish { "terminal" } else { "numerical_replay_limit" }),
        );
        assert!(field(
            &later[0], &["result", "value", "knowledge", "value", "action"],
        ).is_null());
        assert_eq!(fs::read(&journal).unwrap(), settled_bytes);
        assert_eq!(config.source.status().read_attempts, reads);
        let mut expected_generation = generation.clone();
        expected_generation.journal_revision = settled.revision;
        assert_eq!(
            driver.supervisor().host().unwrap().learned_generation_inspection().unwrap(),
            expected_generation,
        );
        if publish {
            // The original job observes the already-settled native state before
            // any clock/source callback. It cannot publish the learned output again.
            assert!(matches!(
                driver.step_computed_from_policy_file(
                    &mut config.source,
                    || panic!("settled driver sampled another clock"),
                    None,
                ).result.unwrap(),
                FileDriverEvent::Stopped { stage: ActionState::Confirmed, .. },
            ));
            assert!(executed(&mut wire, loaded.request));
            assert_eq!(fs::read(&journal).unwrap(), settled_bytes);
            assert_eq!(config.source.status().read_attempts, reads);
        } else {
            assert!(matches!(
                response(&mut wire, loaded.request).result,
                Ok(Knowledge::Unknown { .. }),
            ));
        }
        drop(service);
        assert!(!selected.settings.socket.exists());
    }
}
