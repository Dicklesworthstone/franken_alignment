//! Explicit live Unknown disposition keeps its original charge while an ordinary
//! schedule proceeds to another genuinely reviewed publication. Helper verdicts
//! here are synthetic control fixtures, not model-safety evidence.
use super::*;
use super::super::operator::{known_operator, settings_document, store_files};
use crate::workflow::actor_service::operator::Settings;
use fa_reference::action::Scope;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileHoldDiagnosisBasis, MAX_ABANDONMENT_REPLAY_EVENTS,
};
use fa_reference::address_resolver::Address;
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::sync::{Arc, atomic::AtomicBool, mpsc};

#[path = "abandonment/fixture.rs"]
mod fixture;

fn connect_operator(settings: &Settings) -> UnixStream {
    wait(&settings.socket);
    let socket = UnixStream::connect(&settings.socket).unwrap();
    settings.supervisor.verify(&socket).unwrap();
    socket
}

fn next(scope: Scope, operation: u64) -> String {
    let address = Address::parse(&format!(
        "fa://{}/attempt/request-{}-{}-{}-{}-7@{GENERATION}",
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority,
    )).unwrap();
    let mut query = TowerRequest::new(format!("abandon-plan-{operation}"), address, Verb::Next);
    query.arguments = ReadArguments::AbandonmentNext { operation };
    encode_request(&query).unwrap()
}

fn numeric(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn action(scope: Scope, response: &Json) -> String {
    let value = field(known_operator(response, "abandonment-next"), "value");
    assert_eq!(field(value, "stage").as_str(), Some("unknown"));
    assert_eq!(field(value, "can_abandon").as_bool(), Some(true));
    let action = field(value, "action");
    let arguments = field(action, "arguments");
    assert_eq!(field(arguments, "action").as_str(), Some("abandon_unknown"));
    let expected = field(arguments, "expected");
    let mut request = TowerRequest::new(
        field(action, "operation_id").as_str().unwrap().to_owned(),
        Address::parse(field(action, "address").as_str().unwrap()).unwrap(),
        Verb::Resolve,
    );
    request.idempotency_key = Some(field(action, "idempotency_key").as_str().unwrap().to_owned());
    request.arguments = ReadArguments::AbandonUnknown {
        operation: numeric(field(arguments, "operation")),
        expected: FileHoldDiagnosisBasis {
            scope,
            request: numeric(field(expected, "request")),
            request_generation: numeric(field(expected, "request_generation")),
            attempt: numeric(field(expected, "attempt")),
            journal_revision: numeric(field(expected, "journal_revision")),
            control_sequence: numeric(field(expected, "control_sequence")),
            recorded_at: expected.get("recorded_at").filter(|tick| !tick.is_null())
                .map(|tick| ElapsedTick(numeric(tick))),
        },
    };
    let document = encode_request(&request).unwrap();
    assert_eq!(&parse(document.as_bytes()), action);
    document
}

fn outcome_unknown(reply: &Json) {
    assert_eq!(field(knowledge(reply), "state").as_str(), Some("unknown"));
    assert_eq!(field(knowledge(reply), "reason").as_str(), Some("outcome_unknown"));
}

#[test]
fn plain_and_checked_schedules_continue_after_explicit_unknown_abandonment_without_refund() {
    for checked in [false, true] {
        let root = Directory::new();
        let prepared = fixture::prepare(&root, checked);
        let config = &prepared.config;
        let scope = config.profile.delivery.scope;
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let source = root.0.join("evidence.json");
        let source_bytes = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();
        if checked {
            fixture::second_capture(&root, config);
        }
        let first = document(config, 7, 1, "6669727374");
        let first_poll = poll(config, 7);
        let second = document(config, 8, 1, "7365636f6e64");
        let second_poll = poll(config, 8);
        let actor_peer = prepared.actor.clone();
        let operator_peer = prepared.settings.clone();
        let second_control = crate::workflow::control::socket_path(&prepared.reviewer, 8);
        let human_profile = prepared.reviewer.clone();
        let (offered, offer) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let human = std::thread::spawn(move || {
            let mut client = human_profile.connect_client_waiting(
                8, Duration::from_secs(10),
            ).unwrap();
            let started = Instant::now();
            loop {
                assert!(started.elapsed() < Duration::from_secs(10));
                match client.step().unwrap() {
                    ReviewClientProgress::NeedsDecision => {
                        offered.send(()).unwrap();
                        released.recv_timeout(Duration::from_secs(10)).unwrap();
                        client.respond(ReviewDecision::Approve).unwrap();
                    }
                    ReviewClientProgress::Complete => return,
                    _ => pause(1),
                }
            }
        });
        let peer_store = store.clone();
        let peer_bootstrap = bootstrap.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            let mut operator = connect_operator(&operator_peer);
            // The new transport obtains only its own original-request ticket
            // through an exact historical Submit; it imports no prior session.
            outcome_unknown(&raw_exchange(&mut actor, &first));
            outcome_unknown(&raw_exchange(&mut actor, &first_poll));
            let original = store_files(&peer_store);
            let withheld = raw_exchange(&mut actor, &second);
            assert_eq!(field(native(&withheld), "reason").as_str(), Some("withheld"));
            let plan = raw_exchange(&mut operator, &next(scope, 801));
            let command = action(scope, &plan);
            assert_eq!(store_files(&peer_store), original);
            assert!(!source.exists());
            let before = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            let abandoned = raw_exchange(&mut operator, &command);
            let receipt = field(known_operator(&abandoned, "abandonment"), "value");
            assert_eq!(field(receipt, "before_stage").as_str(), Some("unknown"));
            assert_eq!(field(receipt, "stage").as_str(), Some("irrecoverably_unknown"));
            assert_eq!(field(receipt, "charged_units").as_str(), Some("7"));
            wait(&second_control);
            let after = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            assert_eq!(after.revision, before.revision + 1);
            assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
            assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
            assert_eq!(after.control.ledger.available, before.control.ledger.available);
            assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
            assert_eq!(after.control.ledger.stages[&1], ActionState::IrrecoverablyUnknown);
            assert_eq!(after.executions, 0);
            outcome_unknown(&raw_exchange(&mut actor, &first_poll));
            drop(operator);
            let mut operator = connect_operator(&operator_peer);
            assert_eq!(raw_exchange(&mut operator, &command), abandoned);
            // Historical retry plus a new grant query after handoff cannot
            // replenish this SAME finite grant or release the charge.
            let blocked = raw_exchange(&mut operator, &next(scope, 802));
            let plan = field(known_operator(&blocked, "abandonment-next"), "value");
            assert_eq!(field(plan, "can_abandon").as_bool(), Some(false));
            assert_eq!(field(plan, "block_reason").as_str(), Some("irrecoverably_unknown"));
            assert!(field(plan, "action").is_null());
            fs::write(&source, source_bytes).unwrap();
            let submitted = raw_exchange(&mut actor, &second);
            assert_eq!(field(knowledge(&submitted), "state").as_str(), Some("pending"));
            offer.recv_timeout(Duration::from_secs(10)).unwrap();
            let waiting = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            assert_eq!(waiting.control.ledger.charged, 7);
            assert_eq!(waiting.executions, 0);
            assert_eq!(waiting.control.ledger.stages[&2], ActionState::Authorized);
            assert_eq!(raw_exchange(&mut operator, &command), abandoned);
            release.send(()).unwrap();
            assert!(executed(&finish(&mut actor, &second, &second_poll)));
            outcome_unknown(&raw_exchange(&mut actor, &first_poll));
            let exhausted = raw_exchange(&mut operator, &next(scope, 803));
            assert_eq!(
                field(field(&exhausted, "envelope"), "reason_code").as_str(),
                Some("BudgetExhausted"),
            );
        });
        let result = fixture::serve(prepared, || ElapsedTick(1002));
        peer.join().unwrap();
        human.join().unwrap();
        assert!(result.is_ok(), "checked={checked}: {result:?}");
        let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!((state.executions, state.payload.as_slice()), (1, b"second".as_slice()));
        assert_eq!(state.target.expected_version, 2);
        assert_eq!(state.control.ledger.stages[&1], ActionState::IrrecoverablyUnknown);
        assert_eq!(state.control.ledger.stages[&2], ActionState::Confirmed);
        assert_eq!((state.control.ledger.charged, state.control.ledger.reserved), (14, 0));
        assert_eq!(state.control.ledger.available + 14, bootstrap.delivery.total);
        assert!(state.stop.is_none());
    }
}

#[test]
fn silence_plan_only_wrong_action_and_stop_cannot_advance_the_unknown_schedule() {
    for mode in ["silence", "plan_only", "wrong_action", "stop"] {
        let root = Directory::new();
        let mut prepared = fixture::prepare(&root, false);
        prepared.actor.reply_ms = 100;
        let config = &prepared.config;
        let scope = config.profile.delivery.scope;
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let source = root.0.join("evidence.json");
        fs::remove_file(&source).unwrap();
        let first = document(config, 7, 1, "6669727374");
        let first_poll = poll(config, 7);
        let second = document(config, 8, 1, "7365636f6e64");
        let actor_peer = prepared.actor.clone();
        let operator_peer = prepared.settings.clone();
        let reviewer = prepared.reviewer.clone();
        let expire = Arc::new(AtomicBool::new(false));
        let peer_expire = Arc::clone(&expire);
        let peer_store = store.clone();
        let peer_bootstrap = bootstrap.clone();
        let peer = std::thread::spawn(move || {
            let mut actor = connect(&actor_peer);
            let mut operator = connect_operator(&operator_peer);
            // The new transport obtains only its own original-request ticket
            // through an exact historical Submit; it imports no prior session.
            outcome_unknown(&raw_exchange(&mut actor, &first));
            outcome_unknown(&raw_exchange(&mut actor, &first_poll));
            let unchanged = store_files(&peer_store);
            if mode == "plan_only" || mode == "wrong_action" {
                let plan = raw_exchange(&mut operator, &next(scope, 801));
                let command = action(scope, &plan);
                if mode == "wrong_action" {
                    let refused = raw_exchange(
                        &mut operator, &command.replace("abandon_unknown", "cancel_undispatched"),
                    );
                    assert_eq!(
                        field(field(&refused, "envelope"), "reason_code").as_str(),
                        Some("CapabilityUnsupported"),
                    );
                }
            } else if mode == "silence" {
                // A connected but incomplete peer contributes no explicit action.
                operator.write_all(b"{\"schema_version\":").unwrap();
            }
            let refused = raw_exchange(&mut actor, &second);
            assert_eq!(field(native(&refused), "reason").as_str(), Some("withheld"));
            assert_eq!(store_files(&peer_store), unchanged);
            assert!(!source.exists());
            let before = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
            assert_eq!(before.control.ledger.stages[&1], ActionState::Unknown);
            assert_eq!(before.control.ledger.charged, 7);
            if mode == "stop" {
                let mut output = Vec::new();
                assert!(crate::peers::stop::request(&reviewer, 7, &mut output).is_err());
                let receipt = parse(&output);
                assert_eq!(field(&receipt, "status").as_str(), Some("stopped_pending"));
                assert_eq!(field(&receipt, "acknowledged").as_bool(), Some(true));
                assert_eq!(field(&receipt, "drained").as_bool(), Some(false));
            } else {
                // The original logical deadline ends this same wait. It does
                // not cause an implicit Resolve or publication.
                peer_expire.store(true, Ordering::SeqCst);
            }
        });
        let result = fixture::serve(prepared, || {
            ElapsedTick(if expire.load(Ordering::SeqCst) { 100000 } else { 1002 })
        });
        peer.join().unwrap();
        assert!(result.is_err(), "{mode}: {result:?}");
        let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(state.executions, 0);
        assert_eq!(state.target.expected_version, 1);
        assert_eq!(state.control.ledger.stages.len(), 1);
        assert_eq!(state.control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!((state.control.ledger.charged, state.control.ledger.reserved), (7, 0));
        assert_eq!(state.control.ledger.available + 7, bootstrap.delivery.total);
        assert!(state.stop.is_some());
    }
}

#[test]
fn unselected_abandonment_keeps_original_unresolved_handoff_refusal() {
    let root = Directory::new();
    let mut prepared = fixture::prepare(&root, false);
    prepared.settings.abandonment = None;
    prepared.settings.inspection_queries = 1;
    prepared.actor.reply_ms = 1;
    let store = prepared.config.store.clone();
    let bootstrap = prepared.config.profile.clone();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let result = fixture::serve(prepared, || ElapsedTick(1002));
    assert!(result.is_err());
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.stages.len(), 1);
    assert_eq!(state.control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(state.control.ledger.charged, 7);
    assert!(state.stop.is_some());
}
