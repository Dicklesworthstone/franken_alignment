//! Authenticated narrowing keeps the actual mediated review and source owner.
//! Controlled helpers exercise publication control, not empirical model quality.
use super::*;
use fa_reference::action::{ResolvedTarget, Scope};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{
    ReviewerClient, ReviewClientPhase,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowBasis, MAX_NARROW_REPLAY_EVENTS,
};
use fa_reference::address_resolver::Address;
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};

fn selected_operator(root: &Root, config: &Config) -> Settings {
    // Admit the base fixture before replacing its read grant with this explicit
    // independent control grant. A zero-grant base profile is invalid.
    let _ = operator_profile(root, config, 1, 0);
    let text = fs::read_to_string(root.path("operator.json")).unwrap()
        .replace("\"inspection_queries\":1", "\"inspection_queries\":0")
        .replacen('{', &format!(
            "{{\"narrowing\":{{\"queries\":2,\"operations\":1,\"max_replay_events\":{MAX_NARROW_REPLAY_EVENTS},\"authority\":\"narrow_original_targets\"}},"
        ), 1);
    Settings::decode(text.as_bytes()).unwrap()
}

fn next(scope: Scope, operation: u64, retained_targets: Vec<ResolvedTarget>) -> String {
    let mut request = TowerRequest::new(
        format!("mediated-narrow-plan-{operation}"),
        Address::parse(&format!("fa://{}/run/{}@{GENERATION}", scope.tenant, scope.run)).unwrap(),
        Verb::Next,
    );
    request.arguments = ReadArguments::NarrowingNext { operation, retained_targets };
    encode_request(&request).unwrap()
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn action(scope: Scope, response: &Json, retained_targets: Vec<ResolvedTarget>) -> String {
    let plan = field(known(response, "narrowing-next"), "value");
    assert_eq!(field(plan, "can_narrow").as_bool(), Some(true));
    let action = field(plan, "action");
    let arguments = field(action, "arguments");
    let basis = field(arguments, "expected");
    let mut request = TowerRequest::new(
        field(action, "operation_id").as_str().unwrap(),
        Address::parse(field(action, "address").as_str().unwrap()).unwrap(),
        Verb::Narrow,
    );
    request.idempotency_key = Some(field(action, "idempotency_key").as_str().unwrap().to_owned());
    request.arguments = ReadArguments::Narrow {
        operation: number(field(arguments, "operation")),
        expected: FileNarrowBasis {
            scope,
            journal_revision: number(field(basis, "journal_revision")),
            control_sequence: number(field(basis, "control_sequence")),
            authority_epoch: number(field(basis, "authority_epoch")),
            recorded_at: basis.get("recorded_at").filter(|tick| !tick.is_null())
                .map(|tick| ElapsedTick(number(tick))),
        },
        retained_targets,
    };
    let encoded = encode_request(&request).unwrap();
    assert_eq!(&parse(encoded.as_bytes()), action);
    encoded
}

fn offer(profile: &PeerProfile) -> ReviewerClient<UnixStream> {
    let mut client = profile.connect_client_waiting(7, Duration::from_secs(10)).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        match client.step().unwrap() {
            ReviewClientProgress::NeedsDecision => return client,
            ReviewClientProgress::Complete => panic!("offer completed without an independent decision"),
            _ => pause(1),
        }
    }
}

fn withdrawn(mut client: ReviewerClient<UnixStream>) {
    client.respond(ReviewDecision::Approve).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        match client.step() {
            Err(_) => {
                assert_eq!(client.phase(), ReviewClientPhase::Failed);
                assert!(client.receipt().is_none());
                return;
            }
            Ok(ReviewClientProgress::Complete | ReviewClientProgress::NeedsDecision) => {
                panic!("withdrawn human packet supplied current authority");
            }
            _ => pause(1),
        }
    }
}

fn approve(mut client: ReviewerClient<UnixStream>) {
    client.respond(ReviewDecision::Approve).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        match client.step().unwrap() {
            ReviewClientProgress::Complete => {
                assert_eq!(client.receipt().unwrap().decision, ReviewDecision::Approve);
                return;
            }
            ReviewClientProgress::NeedsDecision => panic!("original decision was lost"),
            _ => pause(1),
        }
    }
}

fn final_bypass(checks: u64) -> String {
    graph(true)
        .replace("\"checks\":32", &format!("\"checks\":{checks}"))
        .replace(&format!("\"read_bytes\":{}", 32 * TOPOLOGY_REFRESH_READ_BYTES),
            &format!("\"read_bytes\":{}", checks * TOPOLOGY_REFRESH_READ_BYTES))
}

#[test]
fn mediated_plain_and_checked_services_renew_human_approval_after_narrowing_and_recheck_final_topology() {
    for checked in [false, true] {
        for late_bypass in [false, true] {
            let root = Root::new();
            let config = configuration(&root);
            evidence(&root, &config);
            let (mut actor, reviewer) = profiles(&root, &config);
            actor.reply_ms = 1000;
            let operator = selected_operator(&root, &config);
            let archive = root.path("narrowed-source.bin");
            let selected = selected_checks(&root, &archive, 2);
            let publication = checked.then(|| prepare(&root, &config, ElapsedTick(100000)).0);
            let proposal = proposal_document(&config);
            let poll = request_document(&config, "get");
            let scope = config.profile.delivery.scope;
            let target = config.profile.delivery.target;
            let store = config.store.clone();
            let bootstrap = config.profile.clone();
            let peer_store = store.clone();
            let peer_bootstrap = bootstrap.clone();
            let actor_peer = actor.clone();
            let operator_peer = operator.clone();
            let human_peer = reviewer.clone();
            let peer = std::thread::spawn(move || {
                let mut actor = connect_actor(&actor_peer);
                let submitted = exchange(&mut actor, &proposal);
                assert_eq!(field(request_knowledge(&submitted), "state").as_str(), Some("pending"));
                let original_client = offer(&human_peer);
                let original = original_client.packet().unwrap().clone();
                let mut operator = connect_operator(&operator_peer);
                let before = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
                assert_eq!(before.control.ledger.stages[&1], ActionState::Reviewing);
                let plan = exchange(&mut operator, &next(scope, 811, vec![target]));
                let command = action(scope, &plan, vec![target]);
                let response = exchange(&mut operator, &command);
                let receipt = field(known(&response, "narrowing"), "value");
                assert_eq!(field(receipt, "refunded_units").as_str(), Some("0"));
                assert!(field(receipt, "cancelled").as_array().unwrap().is_empty());
                let narrowed = FileOversight::read_publication(&peer_store, &peer_bootstrap).unwrap();
                assert_eq!(narrowed.control.sequence, before.control.sequence + 1);
                assert_eq!(narrowed.control.ledger.epoch, before.control.ledger.epoch);
                assert_eq!(narrowed.control.ledger.charged, 0);
                assert_eq!(narrowed.executions, 0);
                withdrawn(original_client);
                let fresh_client = offer(&human_peer);
                let fresh = fresh_client.packet().unwrap();
                assert_ne!(fresh.binding().request, original.binding().request);
                assert_ne!(fresh.binding().session, original.binding().session);
                assert_eq!(fresh.binding().attempt, original.binding().attempt);
                assert_eq!(fresh.control_sequence(), original.control_sequence() + 1);
                assert_eq!(fresh.input_revision(), original.input_revision());
                assert_eq!(fresh.policy_generation(), original.policy_generation());
                assert_eq!(fresh.action(), original.action());
                assert_eq!(fresh.views(), original.views());
                assert_eq!(FileOversight::read_publication(&peer_store, &peer_bootstrap)
                    .unwrap().executions, 0);
                approve(fresh_client);
                if !late_bypass {
                    assert_executed(&finish(&mut actor, &proposal, &poll));
                    assert_eq!(exchange(&mut operator, &command), response);
                    let blocked = exchange(&mut operator, &next(scope, 812, vec![target]));
                    assert_eq!(field(field(known(&blocked, "narrowing-next"), "value"),
                        "block_reason").as_str(), Some("operation_limit"));
                }
            });
            let mut changed_at_dispatch = false;
            let result = service::serve_tower_with_topology(
                config, &actor, &reviewer, publication.as_ref(),
                Options { requests: &[7], open: false, credibility: None },
                GENERATION, Some(&operator), selected, || {
                    if late_bypass && !changed_at_dispatch
                        && let Ok(state) = FileOversight::read_publication(&store, &bootstrap)
                        && state.control.ledger.charged == 7 && state.executions == 0
                    {
                        fs::write(root.path("topology.json"), final_bypass(2)).unwrap();
                        changed_at_dispatch = true;
                    }
                    ElapsedTick(1000)
                },
            );
            peer.join().unwrap();
            assert_eq!(changed_at_dispatch, late_bypass);
            let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
            assert_eq!(state.executions, u64::from(!late_bypass));
            assert_eq!(state.control.ledger.reserved, 0);
            let stage = state.control.ledger.stages[&1];
            if !late_bypass {
                assert!(result.is_ok(), "checked={checked}: {result:?}");
                assert_eq!(stage, ActionState::Confirmed);
                assert_eq!(state.control.ledger.charged, 7);
                assert_eq!(state.control.ledger.epoch, 0);
                assert!(state.stop.is_none());
            } else if state.control.ledger.charged == 0 {
                // Only the original endpoint's recorded nonexecution may
                // release an already dispatched charge.
                assert_eq!(stage, ActionState::ConfirmedNotExecuted);
            } else {
                assert_eq!(state.control.ledger.charged, 7);
                assert!(matches!(stage, ActionState::Dispatching | ActionState::Unknown
                    | ActionState::IrrecoverablyUnknown));
            }
            assert_eq!(state.control.ledger.available + state.control.ledger.charged,
                bootstrap.delivery.total);
            let saved = TopologyCheckpoint::read_trusted(
                &mut Cursor::new(fs::read(archive).unwrap()), MAX_TOPOLOGY_CHECKPOINT_BYTES,
            ).unwrap();
            assert_eq!(saved.history_anchor().revision(), state.revision);
            assert_eq!(saved.work().checks, 2);
            assert_eq!(saved.work().reserved_read_bytes, 2 * TOPOLOGY_REFRESH_READ_BYTES);
            assert_eq!(saved.last_operation(), 2);
            assert_eq!(saved.available(), !late_bypass);
            assert!(!actor.socket.exists() && !operator.socket.exists());
            assert!(!reviewer.socket(7).exists());
        }
    }
}
