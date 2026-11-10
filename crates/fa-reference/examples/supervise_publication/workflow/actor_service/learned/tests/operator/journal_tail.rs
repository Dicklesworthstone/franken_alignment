//! Real learned publication with independently gated human approval. Historical
//! metadata remains available through the original owner without numerical replay.
use super::*;
use std::collections::BTreeSet;

fn tail_selection(root: &Root, actor: &Profile) -> (PathBuf, live_operator::Selection) {
    let (path, _) = selection(root, actor);
    let original = fs::read_to_string(&path).unwrap();
    let document = original.replacen('{',
        "{\"journal_tail\":{\"queries\":32,\"events_per_page\":16,\"disclosure\":\"redacted_journal_events\"},", 1)
        .replace("\"exchange_limit\":16", "\"exchange_limit\":64");
    fs::write(&path, document).unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    assert!(
        selected
            .settings
            .explanation_numerical_replay_events
            .is_none()
    );
    assert!(selected.settings.review_explanation.is_none());
    (path, selected)
}

fn tail(
    profile: &Path,
    selected: &live_operator::Selection,
    after: u64,
) -> (u64, BTreeSet<String>) {
    let scope = selected.settings.scope;
    let mount = Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(GENERATION),
    )
    .unwrap();
    let args = vec![
        "tower-tail".into(),
        profile.display().to_string(),
        mount.canonical_string(),
        "--after-revision".into(),
        after.to_string(),
        "--max-pages".into(),
        "16".into(),
    ];
    let mut output = Vec::new();
    crate::command_with_output(args, &mut output).unwrap();
    let mut through = after;
    let mut kinds = BTreeSet::new();
    let mut complete = false;
    let mut basis = None;
    for line in output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let response = strict_json::parse(line, KNOWLEDGE_JSON_LIMITS).unwrap();
        assert_eq!(
            field(&response, &["result", "kind"]).as_str(),
            Some("journal-tail")
        );
        assert_eq!(
            field(&response, &["result", "value", "knowledge", "state"]).as_str(),
            Some("known")
        );
        assert_eq!(
            field(&response, &["result", "value", "disclosure"]).as_str(),
            Some("redacted_journal_events")
        );
        let current = field(&response, &["result", "value", "knowledge", "basis"]);
        if let Some(original) = &basis {
            assert_eq!(current, original);
        } else {
            basis = Some(current.clone());
        }
        let page = field(&response, &["result", "value", "knowledge", "value"]);
        for event in field(page, &["events"]).as_array().unwrap() {
            assert_eq!(
                event.as_object().unwrap().len(),
                5,
                "private event data leaked"
            );
            let revision = field(event, &["revision"])
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();
            assert_eq!(revision, through + 1);
            through = revision;
            kinds.insert(field(event, &["kind"]).as_str().unwrap().to_owned());
        }
        assert_eq!(
            field(page, &["through_revision"]).as_str(),
            Some(through.to_string().as_str())
        );
        complete = field(page, &["enumeration_complete"]).as_bool().unwrap();
    }
    assert!(complete);
    assert_eq!(
        field(basis.as_ref().unwrap(), &["journal_revision"]).as_str(),
        Some(through.to_string().as_str())
    );
    (through, kinds)
}

#[test]
fn real_learned_history_is_read_before_admission_during_human_wait_and_after_execution() {
    let root = Root::new();
    let config = configured(&root);
    let path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, false).unwrap();
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, loaded.request);
    let (operator_path, selected) = tail_selection(&root, &profile);
    let operator_socket = selected.settings.socket.clone();
    let mut args = super::super::mediated_topology::command_inputs(&root, &config, &path, &profile);
    args.splice(4..4, ["--tower-generation".into(), GENERATION.to_string()]);
    args.extend([
        "--operator-profile".into(),
        operator_path.display().to_string(),
    ]);
    let actor_ready = Arc::new(AtomicBool::new(false));
    let human_ready = Arc::new(AtomicBool::new(false));
    let actor_done = Arc::new(AtomicBool::new(false));
    let (release_actor, release_human, finished) = (
        Arc::clone(&actor_ready),
        Arc::clone(&human_ready),
        Arc::clone(&actor_done),
    );
    let human_socket = peers.socket(loaded.request);
    let journal = config.store.join("delivery.bin");
    let observer = thread::spawn(move || {
        wait_for(&selected.settings.socket);
        let (initial_revision, initial) = tail(&operator_path, &selected, 0);
        assert!(initial.contains("learned_enabled"));
        assert!(!initial.contains("text_message"));
        release_actor.store(true, Ordering::Release);
        wait_for(&human_socket);
        let before = fs::read(&journal).unwrap();
        let (awaiting_revision, awaiting) = tail(&operator_path, &selected, initial_revision);
        for kind in ["text_message", "finish_review", "request_human"] {
            assert!(
                awaiting.contains(kind),
                "missing real learned event {kind}: {awaiting:?}"
            );
        }
        // Generation precedes actor admission and may finish before the first
        // capture. Both original, gapless suffixes together must contain it.
        for kind in ["learned_step_begin", "learned_step"] {
            assert!(
                initial.contains(kind) || awaiting.contains(kind),
                "missing real generation event {kind}"
            );
        }
        for kind in [
            "dispatch",
            "publish",
            "publish_checked",
            "publish_credentialed",
        ] {
            assert!(
                !awaiting.contains(kind),
                "tail replaced the independent human key"
            );
        }
        assert_eq!(
            fs::read(&journal).unwrap(),
            before,
            "historical tail changed numerical or publication state"
        );
        release_human.store(true, Ordering::Release);
        let started = Instant::now();
        while !finished.load(Ordering::Acquire) {
            assert!(started.elapsed() < Duration::from_secs(25));
            thread::sleep(Duration::from_millis(1));
        }
        let (_, terminal) = tail(&operator_path, &selected, awaiting_revision);
        assert!(terminal.contains("human_decision"));
        assert!(terminal.contains("dispatch"));
        assert!(
            ["publish", "publish_checked", "publish_credentialed"]
                .iter()
                .any(|kind| terminal.contains(*kind))
        );
    });
    let mut peer: Option<thread::JoinHandle<_>> = None;
    let mut reviewer = None;
    command_with_clock(&args, None, || {
        if peer.is_none() && actor_ready.load(Ordering::Acquire) && profile.socket.exists() {
            peer = Some(actor(
                profile.clone(),
                LearnedTextRelease::Message,
                ElapsedTick(50_000),
                Behavior::Observe,
            ));
        }
        if reviewer.is_none()
            && human_ready.load(Ordering::Acquire)
            && peers.socket(loaded.request).exists()
        {
            reviewer = Some(human(
                peers.clone(),
                loaded.request,
                ReviewDecision::Approve,
            ));
        }
        if peer.as_ref().is_some_and(thread::JoinHandle::is_finished) {
            actor_done.store(true, Ordering::Release);
        }
        ElapsedTick(1000)
    })
    .unwrap();
    observer.join().unwrap();
    assert!(matches!(
        peer.unwrap().join().unwrap().result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    ));
    let packet = reviewer.unwrap().join().unwrap();
    let state = FileOversight::read_publication_with_learned_generation(
        &config.store,
        &config.profile,
        &loaded.generation,
    )
    .unwrap();
    assert_eq!((state.executions, state.payload), (1, b"A".to_vec()));
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (packet.action().spec().units, 0)
    );
    assert!(!operator_socket.exists());
}
