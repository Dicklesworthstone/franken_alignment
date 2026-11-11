//! One authenticated observation session follows the finite original learned
//! owner. Real numerical generation, native review and human peers continue
//! after read exhaustion; these synthetic fixtures do not qualify model quality.
use super::super::super::operator as live_operator;
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_EXPLANATION_EVENTS;
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::{
    ReadArguments, TowerRequest, Verb, encode_request, publication::request_address_for,
};
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const GENERATION: u64 = 41;

fn selection(root: &Root, actor: &Profile) -> (PathBuf, live_operator::Selection) {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let pid = identity.pid().unwrap();
    let path = root.0.join("operator.json");
    let socket = root.0.join("peers/operator.sock");
    fs::write(&path, format!(
        r#"{{"schema":"fa.tower-operator-service/1","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":{},"supervisor":{{"uid":{},"gid":{},"pid":{pid}}},"operator":{{"uid":{},"gid":{},"pid":{pid}}},"candidate_limit":4,"connection_limit":4,"exchange_limit":16,"runtime_ms":60000,"poll_ms":1,"inspection_queries":3,"abandonment":{{"queries":1,"operations":1,"max_replay_events":4096,"numerical_replay_events":4096,"authority":"abandon_unknown_requests"}},"journal_tail":{{"queries":2,"events_per_page":4,"disclosure":"redacted_journal_events"}},"request_history":{{"queries":1,"max_history_events":4096,"max_reviews":16,"disclosure":"recorded_request_relationships"}},"explanation_queries":2,"explanation_replay_events":{},"explanation_disclosure":"policy_and_selected_witnesses"}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        quote(socket.to_str().unwrap()), identity.uid(), identity.gid(), identity.uid(), identity.gid(),
        MAX_EXPLANATION_EVENTS)).unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    (path, selected)
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| &value.as_object().unwrap()[*key])
}

fn number(value: &Json, path: &[&str]) -> u64 {
    field(value, path).as_str().unwrap().parse().unwrap()
}

fn known(response: &Json, kind: &str) {
    assert_eq!(field(response, &["result", "kind"]).as_str(), Some(kind));
    assert_eq!(
        field(response, &["result", "value", "knowledge", "state"]).as_str(),
        Some("known")
    );
    assert_eq!(
        field(response, &["result", "value", "current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!(
        field(
            response,
            &["result", "value", "knowledge", "basis", "origin"]
        )
        .as_str(),
        Some("acknowledged_owner")
    );
}

fn unknown_explanation(response: &Json) {
    assert_eq!(
        field(response, &["result", "kind"]).as_str(),
        Some("explanation")
    );
    assert_eq!(
        field(response, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(
        field(response, &["envelope", "reason_code"]).as_str(),
        Some("LowerLayerUnverified")
    );
    assert_eq!(
        field(response, &["result", "value", "effect_authority"]).as_bool(),
        Some(false)
    );
    assert_eq!(
        field(response, &["result", "value", "current_eligibility"]).as_str(),
        Some("not_observed")
    );
}

fn wait_for(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "original socket never appeared: {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn connect(selected: &live_operator::Selection) -> UnixStream {
    wait_for(&selected.settings.socket);
    let socket = UnixStream::connect(&selected.settings.socket).unwrap();
    selected.settings.supervisor.verify(&socket).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(25)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(25)))
        .unwrap();
    socket
}

fn exchange(socket: &mut UnixStream, request: TowerRequest, maximum: usize) -> Json {
    let mut request = encode_request(&request).unwrap().into_bytes();
    request.push(b'\n');
    socket.write_all(&request).unwrap();
    let mut response = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        socket.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
        assert!(response.len() <= maximum + 1);
        if byte[0] == b'\n' {
            break;
        }
    }
    strict_json::parse(&response, KNOWLEDGE_JSON_LIMITS).unwrap()
}

#[derive(Clone)]
struct Stages {
    initial: Arc<AtomicBool>,
    second: Arc<AtomicBool>,
    final_entry: Arc<AtomicBool>,
}
impl Stages {
    fn actors(&self) -> [Option<Actor>; 3] {
        [&self.initial, &self.second, &self.final_entry]
            .map(|stage| stage.load(Ordering::Acquire).then_some(Actor::Observe))
    }
}

/// Each actor waits for its predecessor's observable cut. This establishes that
/// a new socket is attached to the original history and that exhausted grants
/// cannot be replenished by either an actor handoff or operator reconnect.
fn observe(
    selected: live_operator::Selection,
    entries: &[resident::Entry],
) -> (Stages, JoinHandle<()>) {
    let stages = Stages {
        initial: Arc::new(AtomicBool::new(false)),
        second: Arc::new(AtomicBool::new(false)),
        final_entry: Arc::new(AtomicBool::new(false)),
    };
    let advanced = stages.clone();
    let second_actor = entries[1].actor.socket.clone();
    let final_actor = entries[2].actor.socket.clone();
    let scope = entries[0].actor.scope;
    let registration = Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(GENERATION),
    )
    .unwrap();
    let first_request =
        request_address_for(scope, &registration, entries[0].loaded.request).unwrap();
    let first_key = entries[0].loaded.request;
    let peer = thread::spawn(move || {
        let maximum = selected.settings.response_bytes;
        let mut socket = connect(&selected);
        let initial = exchange(
            &mut socket,
            TowerRequest::new("sequence-initial", registration.clone(), Verb::Situation),
            maximum,
        );
        known(&initial, "situation");
        assert_eq!(
            number(
                &initial,
                &[
                    "result",
                    "value",
                    "knowledge",
                    "value",
                    "summary",
                    "retained_requests"
                ]
            ),
            0
        );
        assert_eq!(
            number(
                &initial,
                &[
                    "result",
                    "value",
                    "knowledge",
                    "value",
                    "summary",
                    "executions"
                ]
            ),
            0
        );
        // The independently selected control grant exists on this original
        // learned owner before actor intake. Missing work is Unknown, and the
        // entered query spends its finite allowance without native replay.
        let mut abandonment = TowerRequest::new(
            "sequence-abandonment-before-intake", first_request.clone(), Verb::Next,
        );
        abandonment.arguments = ReadArguments::AbandonmentNext { operation: 9_007 };
        let missing = exchange(&mut socket, abandonment, maximum);
        assert_eq!(field(&missing, &["result", "kind"]).as_str(), Some("abandonment-next"));
        assert_eq!(
            field(&missing, &["result", "value", "knowledge", "state"]).as_str(),
            Some("unknown")
        );
        advanced.initial.store(true, Ordering::Release);

        wait_for(&second_actor);
        let later = exchange(
            &mut socket,
            TowerRequest::new("sequence-second", registration.clone(), Verb::Situation),
            maximum,
        );
        known(&later, "situation");
        assert_eq!(
            number(
                &later,
                &[
                    "result",
                    "value",
                    "knowledge",
                    "value",
                    "summary",
                    "retained_requests"
                ]
            ),
            1
        );
        assert_eq!(
            number(
                &later,
                &[
                    "result",
                    "value",
                    "knowledge",
                    "value",
                    "summary",
                    "executions"
                ]
            ),
            1
        );
        assert!(
            number(
                &later,
                &["result", "value", "knowledge", "basis", "journal_revision"]
            ) > number(
                &initial,
                &["result", "value", "knowledge", "basis", "journal_revision"]
            )
        );
        let earlier_rows = field(
            &later,
            &["result", "value", "knowledge", "value", "attempts"],
        )
        .as_array()
        .unwrap();
        assert_eq!(earlier_rows.len(), 1);
        assert_eq!(
            field(&earlier_rows[0], &["request_address"]).as_str(),
            Some(first_request.canonical_string().as_str())
        );
        assert_eq!(
            field(&earlier_rows[0], &["stage"]).as_str(),
            Some("confirmed")
        );
        let mut tail = TowerRequest::new("sequence-first-tail", registration.clone(), Verb::Tail);
        tail.arguments = ReadArguments::JournalTail { after_revision: 0 };
        let first_tail = exchange(&mut socket, tail, maximum);
        known(&first_tail, "journal-tail");
        let original_basis = field(&first_tail, &["result", "value", "knowledge", "basis"]);
        let original_page = field(&first_tail, &["result", "value", "knowledge", "value"]);
        let cursor = number(original_page, &["continuation"]);
        assert_eq!(number(original_page, &["offset"]), 0);
        assert_eq!(number(original_page, &["after_revision"]), 0);
        assert_eq!(number(original_page, &["through_revision"]), 4);
        assert_eq!(
            field(original_page, &["events"]).as_array().unwrap().len(),
            4
        );
        assert_eq!(
            field(original_page, &["enumeration_complete"]).as_bool(),
            Some(false)
        );
        let first = exchange(
            &mut socket,
            TowerRequest::new(
                "sequence-first-admission",
                first_request.clone(),
                Verb::Explain,
            ),
            maximum,
        );
        // Event-only historical grants do not include numerical reconstruction
        // work. A completed original request must still report honest Unknown.
        unknown_explanation(&first);
        // Direct request relationships require no numerical explanation replay
        // and remain a separate original grant from both Tail and Explain.
        let history = exchange(
            &mut socket,
            TowerRequest::new(
                "sequence-original-history",
                first_request.clone(),
                Verb::Related,
            ),
            maximum,
        );
        known(&history, "request-history");
        assert_eq!(
            number(
                &history,
                &["result", "value", "knowledge", "basis", "request"]
            ),
            first_key
        );
        let history = field(&history, &["result", "value", "knowledge", "value"]);
        assert_eq!(field(history, &["stage"]).as_str(), Some("confirmed"));
        assert_eq!(
            field(history, &["enumeration_complete"]).as_bool(),
            Some(true)
        );
        let reviews = field(history, &["reviews"]).as_array().unwrap();
        assert!(!reviews.is_empty());
        for review in reviews {
            let explanation = field(review, &["explain_request"]);
            assert_eq!(field(explanation, &["verb"]).as_str(), Some("explain"));
            assert_eq!(
                field(explanation, &["address"]).as_str(),
                Some(first_request.canonical_string().as_str())
            );
            assert_eq!(
                field(explanation, &["arguments", "round"]),
                field(review, &["round"])
            );
        }
        drop(socket);

        let mut socket = connect(&selected);
        advanced.second.store(true, Ordering::Release);

        wait_for(&final_actor);
        let last = exchange(
            &mut socket,
            TowerRequest::new("sequence-final", registration.clone(), Verb::Situation),
            maximum,
        );
        known(&last, "situation");
        assert_eq!(
            number(
                &last,
                &[
                    "result",
                    "value",
                    "knowledge",
                    "value",
                    "summary",
                    "retained_requests"
                ]
            ),
            2
        );
        assert_eq!(
            number(
                &last,
                &[
                    "result",
                    "value",
                    "knowledge",
                    "value",
                    "summary",
                    "executions"
                ]
            ),
            2
        );
        assert!(
            number(
                &last,
                &["result", "value", "knowledge", "basis", "journal_revision"]
            ) > number(
                &later,
                &["result", "value", "knowledge", "basis", "journal_revision"]
            )
        );
        let later_rows = field(
            &last,
            &["result", "value", "knowledge", "value", "attempts"],
        )
        .as_array()
        .unwrap();
        assert_eq!(later_rows.len(), 2);
        assert_eq!(later_rows[0], earlier_rows[0]);
        // This is the same operator service after a real actor handoff and a
        // reconnect. Its second query must consume the earlier immutable cut,
        // not refresh metadata from the second completed publication.
        let mut tail =
            TowerRequest::new("sequence-retained-tail", registration.clone(), Verb::Tail);
        tail.arguments = ReadArguments::JournalTailPage { cursor };
        let continued = exchange(&mut socket, tail, maximum);
        known(&continued, "journal-tail");
        assert_eq!(
            field(&continued, &["result", "value", "knowledge", "basis"]),
            original_basis
        );
        assert!(
            number(
                &last,
                &["result", "value", "knowledge", "basis", "journal_revision"]
            ) > number(original_basis, &["journal_revision"])
        );
        let continued_page = field(&continued, &["result", "value", "knowledge", "value"]);
        assert_eq!(number(continued_page, &["offset"]), 4);
        assert_eq!(number(continued_page, &["after_revision"]), 0);
        let ending = number(original_basis, &["journal_revision"]);
        assert_eq!(number(continued_page, &["through_revision"]), ending.min(8));
        assert_eq!(
            field(continued_page, &["enumeration_complete"]).as_bool(),
            Some(ending <= 8)
        );
        assert!(
            field(continued_page, &["continuation"]).is_null(),
            "the original two-query grant must be exhausted"
        );
        for (index, event) in field(continued_page, &["events"])
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert_eq!(number(event, &["revision"]), index as u64 + 5);
            let request = field(event, &["request"]);
            if !request.is_null() {
                assert_eq!(request.as_str(), Some(first_key.to_string().as_str()));
            }
        }
        let mut tail =
            TowerRequest::new("sequence-no-tail-renewal", registration.clone(), Verb::Tail);
        tail.arguments = ReadArguments::JournalTail { after_revision: 0 };
        let exhausted_tail = exchange(&mut socket, tail, maximum);
        assert_eq!(
            field(&exhausted_tail, &["result", "value", "error", "code"]).as_str(),
            Some("capacity")
        );
        let exhausted_history = exchange(
            &mut socket,
            TowerRequest::new(
                "sequence-history-no-renewal",
                first_request.clone(),
                Verb::Related,
            ),
            maximum,
        );
        assert_eq!(
            field(&exhausted_history, &["result", "value", "error", "code"]).as_str(),
            Some("capacity"),
            "actor handoff or operator reconnect renewed request-history work"
        );
        let exhausted = exchange(
            &mut socket,
            TowerRequest::new("sequence-reconnected", registration, Verb::Situation),
            maximum,
        );
        assert_eq!(
            field(&exhausted, &["envelope", "reason_code"]).as_str(),
            Some("BudgetExhausted")
        );
        let retained = exchange(
            &mut socket,
            TowerRequest::new(
                "sequence-retained-admission",
                first_request.clone(),
                Verb::Explain,
            ),
            maximum,
        );
        unknown_explanation(&retained);
        let exhausted = exchange(
            &mut socket,
            TowerRequest::new("sequence-retained-work", first_request.clone(), Verb::Explain),
            maximum,
        );
        assert_eq!(
            field(&exhausted, &["envelope", "reason_code"]).as_str(),
            Some("BudgetExhausted")
        );
        // Neither a resident handoff nor reconnect can provision another
        // abandonment query or operation. All three real publications still
        // complete through their original native and human review gates.
        let mut abandonment = TowerRequest::new(
            "sequence-abandonment-no-renewal",
            first_request,
            Verb::Next,
        );
        abandonment.arguments = ReadArguments::AbandonmentNext { operation: 9_007 };
        let exhausted_abandonment = exchange(&mut socket, abandonment, maximum);
        assert_eq!(
            field(&exhausted_abandonment, &["result", "value", "error", "code"]).as_str(),
            Some("capacity"),
            "actor handoff or operator reconnect renewed abandonment work"
        );
        advanced.final_entry.store(true, Ordering::Release);
    });
    (stages, peer)
}

#[test]
fn one_operator_observes_every_original_learned_mode_without_renewing_budget_on_handoff_or_reconnect()
 {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, mode, None);
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let (path, selected) = selection(&root, &entries[0].actor);
        let (stages, operator) = observe(
            live_operator::Selection::read(&path, GENERATION).unwrap(),
            &entries,
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
                peers.poll(stages.actors(), [Some(ReviewDecision::Approve); 3]);
                ElapsedTick(1000)
            },
        )
        .unwrap();
        operator.join().unwrap();
        assert_eq!(peers.seen_actor, [true; 3]);
        assert_eq!(peers.seen_human, [true; 3]);
        let (replies, packets) = peers.join();
        // The existing causal owner checks include all three human sessions,
        // exact numerical position/draws, original forecast state, final stream
        // and one conserved ledger. Read exhaustion cannot suppress publication.
        completed(&reader, &result, replies, packets, mode);
        assert!(!selected.settings.socket.exists());
        quiescent(&root);
    }
}

#[test]
fn sequence_operator_preflights_every_scheduled_socket_and_output_before_time_or_store() {
    for collision in 0..10 {
        let root = Root::new();
        let (config, _, mut entries) = selected(&root, None, None);
        let reviewer = fixture::peers(&root, &config);
        let (_, mut selected) = selection(&root, &entries[0].actor);
        match collision {
            0..=2 => selected.settings.socket = entries[collision].actor.socket.clone(),
            3..=5 => {
                selected.settings.socket = reviewer.socket(entries[collision - 3].loaded.request)
            }
            6..=8 => {
                selected.settings.socket =
                    socket_path(&reviewer, entries[collision - 6].loaded.request)
            }
            9 => entries[0].loaded.anchor_output = Some(selected.settings.socket.clone()),
            _ => unreachable!(),
        }
        let store = config.store.clone();
        let result = resident::run_with_operator(
            config,
            entries,
            &reviewer,
            false,
            None,
            None,
            Some(&selected),
            || panic!("colliding operator selection must refuse before time"),
        );
        assert!(result.is_err(), "collision {collision} was admitted");
        assert!(!store.exists());
        quiescent(&root);
    }
}

#[test]
fn a_later_actor_cannot_change_the_original_operator_server_identity() {
    let root = Root::new();
    let (config, _, mut entries) = selected(&root, None, None);
    let reviewer = fixture::peers(&root, &config);
    let (_, selected) = selection(&root, &entries[0].actor);
    let original = entries[2].actor.supervisor;
    let pid = original.pid().unwrap();
    entries[2].actor.supervisor = PeerPolicy::new(
        original.uid(),
        original.gid(),
        Some(if pid == i32::MAX as u32 {
            pid - 1
        } else {
            pid + 1
        }),
    )
    .unwrap();
    let store = config.store.clone();
    let result = resident::run_with_operator(
        config,
        entries,
        &reviewer,
        false,
        None,
        None,
        Some(&selected),
        || panic!("foreign later server must refuse before time"),
    );
    assert!(result.is_err());
    assert!(!store.exists());
    quiescent(&root);
}

#[test]
fn the_real_sequence_command_composes_the_operator_pair_and_retains_one_final_history() {
    let root = Root::new();
    let (config, paths, entries) = selected(&root, Some(true), None);
    let reader = Reader::new(&root, &paths[0]);
    let mut peers = Peers::new(&root, &config, &entries);
    let (operator_path, operator_selection) = selection(&root, &entries[0].actor);
    let (stages, operator) = observe(operator_selection, &entries);
    let mut plan = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let inputs = super::super::mediated_topology::command_inputs(
            &root,
            &config,
            &paths[index],
            &entry.actor,
        );
        let actor = root
            .0
            .join(format!("sequence-actor-{}.json", entry.loaded.request));
        fs::rename(&inputs[5], &actor).unwrap();
        plan.push(format!(
            r#"{{"recipe":{},"actor_profile":{},"release":"{}"}}"#,
            quote(paths[index].to_str().unwrap()),
            quote(actor.to_str().unwrap()),
            if index == 2 { "finish" } else { "message" }
        ));
    }
    let sequence = root.0.join("sequence.json");
    fs::write(
        &sequence,
        format!(
            r#"{{"schema":"fa.learned-publication-sequence/1","entries":[{}]}}"#,
            plan.join(",")
        ),
    )
    .unwrap();
    let history = root.0.join("sequence-history.bin");
    let args = vec![
        "serve-learned-generated".to_owned(),
        root.0.join("command-host.json").display().to_string(),
        sequence.display().to_string(),
        root.0.join("command-reviewer.json").display().to_string(),
        "--tower-generation".into(),
        GENERATION.to_string(),
        "--retain-history-anchor".into(),
        history.display().to_string(),
        "--operator-profile".into(),
        operator_path.display().to_string(),
    ];
    resident::command_with_clock(&args, None, || {
        assert!(
            !history.exists(),
            "retained history belongs to the final sequence cut"
        );
        peers.poll(stages.actors(), [Some(ReviewDecision::Approve); 3]);
        ElapsedTick(1000)
    })
    .unwrap();
    operator.join().unwrap();
    let (responses, packets) = peers.join();
    assert!(
        responses
            .into_iter()
            .all(|response| response.as_ref().is_some_and(confirmed))
    );
    let charged = packets
        .into_iter()
        .map(|packet| packet.unwrap().action().spec().units)
        .sum::<u64>();
    let state = reader.snapshot();
    assert_eq!(state.executions, 3);
    assert_eq!(state.payload, b"AA");
    assert_eq!(state.control.ledger.charged, charged);
    assert_eq!(state.control.ledger.reserved, 0);
    assert!(state.stop.is_none());
    assert!(history.exists());
    quiescent(&root);
}
