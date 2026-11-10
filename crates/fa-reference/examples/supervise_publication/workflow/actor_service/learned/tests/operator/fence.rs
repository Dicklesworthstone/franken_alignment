//! Stop the actual learned owner through the authenticated operator service.
//! Numerical work is reserved explicitly; the historical receipt grants no drain.
use super::*;
use crate::workflow::actor_service::tower::client::operator_command;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileFenceBasis, MAX_FENCE_REPLAY_EVENTS,
};
use fa_reference::tower::ReadArguments;

fn fence_selection(
    root: &Root,
    actor: &Profile,
    events: usize,
) -> (PathBuf, live_operator::Selection) {
    let (path, _) = selection(root, actor);
    let original = fs::read_to_string(&path).unwrap();
    let document = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replacen(
            '{',
            &format!(
                "{{\"fence\":{{\"max_replay_events\":{events},\"numerical_replay_events\":{events},\"authority\":\"stop_original_admission\"}},"
            ),
            1,
        );
    fs::write(&path, document).unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    assert_eq!(selected.settings.inspection_queries, 0);
    assert!(selected.settings.resolution.is_none());
    (path, selected)
}

fn exchange(
    root: &Root,
    service: &mut live_operator::Service,
    profile: &Path,
    label: &str,
    request: &TowerRequest,
) -> Json {
    let path = root.0.join(format!("{label}.json"));
    fs::write(&path, encode_request(request).unwrap()).unwrap();
    let args = vec![
        "tower-operator-exchange".to_owned(),
        profile.display().to_string(),
        path.display().to_string(),
    ];
    // The actual owner and its non-Send custodian remain on this thread.
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
    strict_json::parse(&output, KNOWLEDGE_JSON_LIMITS).unwrap()
}

#[test]
fn learned_operator_fence_cancels_original_work_without_source_reads_or_new_generation() {
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
    assert!(matches!(
        driver.step_computed_from_policy_file(
            &mut config.source, || ElapsedTick(1000), None,
        ).result.unwrap(),
        FileDriverEvent::AwaitingHuman { .. },
    ));
    let original = driver.supervisor().host().unwrap().inspect();
    assert_eq!((original.executions, original.control.ledger.charged), (0, 0));
    assert!(original.control.ledger.reserved > 0);
    let numerical = driver.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.sampled_draws, 2);
    let events = usize::try_from(original.revision).unwrap() + 1;
    assert!(events > 1 && events <= MAX_FENCE_REPLAY_EVENTS);
    let journal = config.store.join("delivery.bin");
    let original_bytes = fs::read(&journal).unwrap();
    let source_work = config.source.status();
    fs::write(root.0.join("evidence.json"), b"unavailable current source").unwrap();

    let (profile_path, selected) = fence_selection(&root, &actor, events);
    selected.check(&config, &loaded, &actor, &peers, None).unwrap();
    let mut service = selected.start(&actor, port, &driver, &limit).unwrap();
    assert_eq!(fs::read(&journal).unwrap(), original_bytes);
    let mount = Address::new(
        actor.scope.tenant.to_string(), Noun::Run,
        actor.scope.run.to_string(), Some(GENERATION),
    ).unwrap();
    let mut command = TowerRequest::new("learned-admission-stop", mount, Verb::Fence);
    command.idempotency_key = Some("1201".to_owned());
    command.arguments = ReadArguments::Fence {
        operation: 1201,
        expected: FileFenceBasis {
            scope: actor.scope,
            journal_revision: original.revision,
            control_sequence: original.control.sequence,
            authority_epoch: original.control.ledger.epoch,
        },
    };
    let acknowledged = exchange(
        &root, &mut service, &profile_path, "learned-fence", &command,
    );
    assert_eq!(field(&acknowledged, &["result", "kind"]).as_str(), Some("fence"));
    assert_eq!(
        field(&acknowledged, &["result", "value", "knowledge", "state"]).as_str(),
        Some("known"),
    );
    let progress = field(
        &acknowledged, &["result", "value", "knowledge", "value", "progress"],
    );
    assert_eq!(field(progress, &["endpoint_fenced"]).as_bool(), Some(false));
    assert_eq!(field(progress, &["drained"]).as_bool(), Some(false));
    assert_eq!(field(progress, &["reserved_units"]).as_str(), Some("0"));
    let stopped = driver.supervisor().host().unwrap().inspect();
    assert_eq!(stopped.revision, original.revision + 1);
    assert!(stopped.stop.is_some());
    assert_eq!(stopped.control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!((stopped.control.ledger.charged, stopped.control.ledger.reserved), (0, 0));
    assert_eq!(stopped.control.ledger.elapsed, original.control.ledger.elapsed);
    assert_eq!(stopped.executions, original.executions);
    assert_eq!(stopped.payload, original.payload);
    assert_eq!(driver.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(config.source.status(), source_work);
    let stopped_bytes = fs::read(&journal).unwrap();

    // This reconnect keeps the same custodian after its full-cut allowance was
    // spent. Exact retry cannot run numerical replay, publish or append Stop.
    assert_eq!(
        exchange(&root, &mut service, &profile_path, "same-learned-fence", &command),
        acknowledged,
    );
    assert!(matches!(
        driver.step_computed_from_policy_file(
            &mut config.source,
            || panic!("stopped learned driver sampled a clock"),
            None,
        ).result.unwrap(),
        FileDriverEvent::Stopped { stage: ActionState::Cancelled, .. },
    ));
    assert!(matches!(
        response(&mut wire, loaded.request).result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }),
    ));
    assert_eq!(config.source.status(), source_work);
    assert_eq!(fs::read(&journal).unwrap(), stopped_bytes);
    assert_eq!(driver.supervisor().host().unwrap().inspect(), stopped);
    assert_eq!(driver.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical, numerical);
    drop(service);
    assert!(!selected.settings.socket.exists());
    assert_eq!(cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms), 0);
}
