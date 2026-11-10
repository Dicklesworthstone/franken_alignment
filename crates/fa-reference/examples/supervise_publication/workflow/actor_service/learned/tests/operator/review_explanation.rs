//! Explicit numerical replay over the real live operator and document client.
//! Historical disclosure cannot replace the still-required original human key.
use super::*;
use crate::workflow::actor_service::tower::client::operator_command;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_REVIEW_EXPLANATION_EVENTS;
use fa_reference::tower::{ReadArguments, publication::request_address_for};

fn review_selection(
    root: &Root,
    actor: &Profile,
    numerical_events: Option<usize>,
) -> (PathBuf, live_operator::Selection) {
    let (path, _) = selection(root, actor);
    let numerical = numerical_events.map_or_else(String::new, |events| {
        format!(",\"numerical_replay_events\":{events}")
    });
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original.replacen(
            '{',
            &format!(
                "{{\"review_explanation\":{{\"queries\":2,\"max_replay_events\":{MAX_REVIEW_EXPLANATION_EVENTS},\"disclosure\":\"policy_and_congress_trace\"{numerical}}},"
            ),
            1,
        ),
    )
    .unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    (path, selected)
}

fn exchange_documents(profile: &Path, documents: &[PathBuf]) -> Vec<Json> {
    let mut arguments = vec![
        "tower-operator-exchange".to_owned(),
        profile.display().to_string(),
    ];
    arguments.extend(documents.iter().map(|path| path.display().to_string()));
    let mut output = Vec::new();
    operator_command(&arguments, &mut output).unwrap();
    output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| strict_json::parse(line, KNOWLEDGE_JSON_LIMITS).unwrap())
        .collect()
}

fn review_state(response: &Json, expected: &str) {
    assert_eq!(
        field(response, &["result", "kind"]).as_str(),
        Some("review-explanation")
    );
    assert_eq!(
        field(response, &["result", "value", "knowledge", "state"]).as_str(),
        Some(expected)
    );
    assert_eq!(
        field(response, &["result", "value", "current_eligibility"]).as_str(),
        Some("not_observed")
    );
    for marker in [
        "effect_authority",
        "helpers_rerun",
        "live_evidence_read",
        "payload_disclosed",
        "helper_inputs_disclosed",
    ] {
        assert_eq!(
            field(response, &["result", "value", marker]).as_bool(),
            Some(false)
        );
    }
}

fn observe_completed_congress(numerical_events: Option<usize>) -> usize {
    let root = Root::new();
    let config = configured(&root);
    let path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, false).unwrap();
    let round = loaded.rounds[0];
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, loaded.request);
    let (operator_path, selected) = review_selection(&root, &profile, numerical_events);
    let operator_socket = selected.settings.socket.clone();
    let registration = Address::new(
        profile.scope.tenant.to_string(),
        Noun::Run,
        profile.scope.run.to_string(),
        Some(GENERATION),
    )
    .unwrap();
    let request_address =
        request_address_for(profile.scope, &registration, loaded.request).unwrap();
    let documents: Vec<_> = (0..3)
        .map(|index| {
            let mut request = TowerRequest::new(
                format!("learned-review-{index}"),
                request_address.clone(),
                Verb::Explain,
            );
            request.arguments = ReadArguments::ReviewExplanation { round };
            let path = root.0.join(format!("review-{index}.json"));
            fs::write(&path, encode_request(&request).unwrap()).unwrap();
            path
        })
        .collect();
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
        let mut socket = connect(&selected);
        let initial = situation(&mut socket, &selected, "review-before-actor");
        known(&initial);
        assert_eq!(summary(&initial, "retained_requests"), 0);
        release_actor.store(true, Ordering::Release);
        wait_for(&human_socket);
        let awaiting = situation(&mut socket, &selected, "review-awaiting-human");
        known(&awaiting);
        assert_eq!(
            (
                summary(&awaiting, "retained_requests"),
                summary(&awaiting, "executions")
            ),
            (1, 0)
        );
        let required = field(
            &awaiting,
            &["result", "value", "knowledge", "basis", "journal_revision"],
        )
        .as_str()
        .unwrap()
        .parse::<usize>()
        .unwrap();
        assert!(required > 1);
        let before = fs::read(&journal).unwrap();
        drop(socket);

        let first = exchange_documents(&operator_path, &documents[..1]);
        assert_eq!(first.len(), 1);
        let expected = if numerical_events.is_some_and(|events| events >= required) {
            "known"
        } else {
            "unknown"
        };
        review_state(&first[0], expected);
        if expected == "known" {
            assert_eq!(
                field(
                    &first[0],
                    &["result", "value", "knowledge", "basis", "journal_revision"]
                )
                .as_str(),
                Some(required.to_string().as_str())
            );
            let congress = field(
                &first[0],
                &["result", "value", "knowledge", "value", "congress"],
            );
            for count in ["member_count", "commit_count", "reveal_count"] {
                assert_eq!(field(congress, &[count]).as_str(), Some("2"));
            }
            for member in field(congress, &["members"]).as_array().unwrap() {
                assert_eq!(field(member, &["outcome"]).as_str(), Some("allow"));
            }
        }
        // A new authenticated client observes the same custodian. The complete
        // first cut cannot be refunded by response output or disconnect.
        let later = exchange_documents(&operator_path, &documents[1..]);
        assert_eq!(later.len(), 2);
        review_state(&later[0], "unknown");
        assert_eq!(
            field(&later[1], &["result", "value", "error", "code"]).as_str(),
            Some("capacity")
        );
        assert_eq!(fs::read(&journal).unwrap(), before);
        release_human.store(true, Ordering::Release);
        let started = Instant::now();
        while !finished.load(Ordering::Acquire) {
            assert!(started.elapsed() < Duration::from_secs(25));
            thread::sleep(Duration::from_millis(1));
        }
        let mut socket = connect(&selected);
        let terminal = situation(&mut socket, &selected, "review-terminal-grace");
        known(&terminal);
        assert_eq!(
            (
                summary(&terminal, "retained_requests"),
                summary(&terminal, "executions")
            ),
            (1, 1)
        );
        required
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
    let required = observer.join().unwrap();
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
    required
}

#[test]
fn original_learned_congress_needs_exact_selected_numerical_work_and_never_supplies_the_human_key()
{
    let required = observe_completed_congress(None);
    assert_eq!(observe_completed_congress(Some(required - 1)), required);
    assert_eq!(observe_completed_congress(Some(required)), required);
}
