//! Exact-policy disclosure and topology enforcement keep their original owners.
//! These real socket/file regressions do not attest source truth or OS isolation.
use super::*;
use crate::workflow::actor_service::precheck::Settings as PrecheckSettings;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{
        PrecheckRequest, PrecheckWireError, PrecheckWireResponse, client::PrecheckPeerClient,
    },
};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;

fn settings(root: &Root, version: u8, queries: usize) -> PrecheckSettings {
    let disclosure = if version == 3 { "full" } else { "exact_predicates" };
    let cache = if version == 1 { "" } else { ",\"cache_entries\":4,\"cache_bytes\":262144" };
    let leakage = if version == 3 {
        ",\"leakage\":{\"policy_structure\":\"disclosed\",\"policy_literals\":\"disclosed\",\"source_values\":\"withheld\",\"helper_feedback\":\"withheld\",\"consequences\":\"withheld\",\"timing\":\"observable\",\"query_usage\":\"observable\"}"
    } else { "" };
    let text = format!(
        r#"{{"schema":"fa.actor-precheck-service/{version}","policy_disclosure":"{disclosure}","socket":"{}","expected_policy_epoch":0,"disclosure_id":81,"query_limit":{queries},"exchange_limit":8,"candidate_limit":8,"connection_limit":4,"runtime_ms":15000{cache}{leakage}}}"#,
        root.path("actors/precheck.sock").display(),
    );
    fs::write(root.path("precheck.json"), &text).unwrap();
    PrecheckSettings::decode(text.as_bytes()).unwrap()
}

fn candidate(config: &Config) -> ActorProposal {
    ActorProposal {
        target: config.profile.delivery.target,
        payload: b"network".to_vec(),
        units: 7,
        deadline: ElapsedTick(100000),
        expected_policy_epoch: 0,
    }
}

fn precheck(profile: &Profile, selected: &PrecheckSettings, proposal: &ActorProposal) -> PrecheckWireResponse {
    let socket = connect_waiting(selected.socket_path());
    let mut client = PrecheckPeerClient::connect(
        socket, profile.supervisor, PrecheckRequest { query: 7, proposal: proposal.clone() }, 0,
    ).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10), "missing precheck response");
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() { return response; }
        assert!(!report.status.closed(), "closed without the original precheck response");
        pause(1);
    }
}

fn passed(response: &PrecheckWireResponse) {
    assert_eq!(response.query, Some(7));
    let Knowledge::Known { value, basis } = response.result.as_ref().unwrap() else {
        panic!("missing exact-policy observation");
    };
    assert_eq!(*value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!((basis.query, basis.disclosure, basis.policy_epoch), (7, 81, 0));
    assert_eq!(basis.observed_at, ElapsedTick(1000));
    let encoded = String::from_utf8(response.encode().unwrap()).unwrap();
    assert!(encoded.contains("\"review_required\":true"));
}

fn args(root: &Root, checked: bool) -> Vec<String> {
    let mut args = vec![
        if checked { "serve-create-checked".into() } else { "serve-create".into() },
        root.path("config.json").display().to_string(),
        root.path("actor.json").display().to_string(),
        root.path("reviewer.json").display().to_string(),
    ];
    if checked { args.push(root.path("witness.json").display().to_string()); }
    args.extend([
        "--precheck-profile".into(), root.path("precheck.json").display().to_string(),
        "--topology-profile".into(), root.path("topology-profile.json").display().to_string(),
        "--tower-generation".into(), GENERATION.to_string(),
    ]);
    args
}

fn retained(root: &Root, state_revision: u64, checks: u64) -> TopologyCheckpoint {
    let saved = TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(root.path("source.bin")).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    ).unwrap();
    assert_eq!(saved.history_anchor().revision(), state_revision);
    assert_eq!(saved.work().checks, checks);
    assert_eq!(saved.work().reserved_read_bytes, checks * TOPOLOGY_REFRESH_READ_BYTES);
    assert_eq!(saved.last_operation(), checks);
    assert_eq!(saved.topology_bytes(), graph(false).as_bytes());
    assert_eq!(saved.inventory_bytes(), INVENTORY);
    saved
}

#[test]
fn topology_precheck_all_disclosures_keep_queries_finite_then_publish_with_both_keys() {
    for checked in [false, true] {
        for version in [1, 2, 3] {
            let root = Root::new();
            let config = configuration(&root);
            let (actor, reviewer) = profiles(&root, &config);
            if checked { prepare(&root, &config, ElapsedTick(100000)); }
            else { evidence(&root, &config); }
            retained_selection(&root, &root.path("source.bin"));
            let selected = settings(&root, version, 2);
            let proposal = candidate(&config);
            let submitted = proposal_document(&config);
            let poll = request_document(&config, "get");
            let store = config.store.clone();
            let bootstrap = config.profile.clone();
            let review_socket = reviewer.socket(7);
            let actor_peer = actor.clone();
            let (offered_send, offered_receive) = mpsc::channel();
            let (release_send, release_receive) = mpsc::channel();
            let human = gated_human(reviewer.clone(), ReviewDecision::Approve, offered_send, release_receive);
            let peer = std::thread::spawn(move || {
                passed(&precheck(&actor_peer, &selected, &proposal));
                let before = FileOversight::read_publication(&store, &bootstrap).unwrap();
                assert!(before.control.ledger.stages.is_empty());
                assert_eq!(before.executions, 0);
                assert!(!review_socket.exists(), "precheck started human review");
                // Reconnects and repeated query labels spend the SAME grant,
                // including cached exact-predicate and full-policy selections.
                passed(&precheck(&actor_peer, &selected, &proposal));
                assert_eq!(precheck(&actor_peer, &selected, &proposal).result,
                    Err(PrecheckWireError::Capacity));
                assert_eq!(FileOversight::read_publication(&store, &bootstrap).unwrap(), before);
                let mut socket = connect_actor(&actor_peer);
                let response = exchange(&mut socket, &submitted);
                assert_eq!(field(request_knowledge(&response), "state").as_str(), Some("pending"));
                offered_receive.recv_timeout(Duration::from_secs(10)).unwrap();
                let waiting = FileOversight::read_publication(&store, &bootstrap).unwrap();
                assert_eq!(waiting.executions, 0, "precheck bypassed the independent human key");
                assert!(!selected.socket_path().exists(), "precheck survived into effect review");
                release_send.send(()).unwrap();
                let response = terminal(&mut socket, &poll);
                let observed = request_knowledge(&response);
                assert_eq!(field(observed, "state").as_str(), Some("known"));
                assert_eq!(field(observed, "value").as_str(), Some("executed"));
            });
            let result = tower_command(&args(&root, checked), None, || ElapsedTick(1000));
            peer.join().unwrap();
            human.join().unwrap();
            assert!(result.is_ok(), "checked={checked}, disclosure={version}: {result:?}");
            let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
            assert_eq!(state.executions, 1);
            assert_eq!(state.payload, b"network");
            assert_eq!((state.control.ledger.charged, state.control.ledger.reserved), (7, 0));
            assert!(state.stop.is_none());
            assert!(retained(&root, state.revision, 2).available());
            assert!(!actor.socket.exists());
            assert!(!root.path("actors/precheck.sock").exists());
        }
    }
}

#[test]
fn topology_precheck_cannot_replace_fresh_policy_evidence_on_submit() {
    let root = Root::new();
    let config = configuration(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    retained_selection(&root, &root.path("source.bin"));
    let selected = settings(&root, 2, 2);
    let proposal = candidate(&config);
    let submitted = proposal_document(&config);
    let source_path = root.path("evidence.json");
    let changed = EvidenceSnapshot::new(
        EvidenceIdentity { source: 51, generation: 2, scope: config.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true,
            values: BTreeMap::from([(7, b"blocked".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|name|
            (name.to_owned(), format!("review this fixture: {name}").into_bytes())).collect(),
    ).unwrap().encode();
    let actor_peer = actor.clone();
    let peer = std::thread::spawn(move || {
        passed(&precheck(&actor_peer, &selected, &proposal));
        fs::write(&source_path, changed).unwrap();
        let response = exchange(&mut connect_actor(&actor_peer), &submitted);
        let observed = request_knowledge(&response);
        assert_eq!(field(observed, "state").as_str(), Some("known"));
        assert_eq!(field(observed, "value").as_str(), Some("denied"));
    });
    let result = tower_command(&args(&root, false), None, || ElapsedTick(1000));
    peer.join().unwrap();
    assert!(result.is_ok(), "{result:?}");
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 0);
    assert_eq!((state.control.ledger.charged, state.control.ledger.reserved), (0, 0));
    assert_eq!(state.control.ledger.stages[&1], ActionState::Denied);
    assert!(!reviewer.socket(7).exists());
    retained(&root, state.revision, 1);
}

#[test]
fn topology_precheck_pass_does_not_replace_the_final_cut_or_erase_a_charged_dispatch() {
    for missing in [false, true] {
        let root = Root::new();
        let config = configuration(&root);
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        retained_selection(&root, &root.path("source.bin"));
        let selected = settings(&root, 1, 2);
        let proposal = candidate(&config);
        let submitted = proposal_document(&config);
        let actor_peer = actor.clone();
        let peer = std::thread::spawn(move || {
            passed(&precheck(&actor_peer, &selected, &proposal));
            let response = exchange(&mut connect_actor(&actor_peer), &submitted);
            assert_eq!(field(request_knowledge(&response), "state").as_str(), Some("pending"));
        });
        let human = human(reviewer.clone(), ReviewDecision::Approve);
        let mut dispatched = false;
        let result = tower_command(&args(&root, false), None, || {
            if !dispatched
                && let Ok(state) = FileOversight::read_publication(&config.store, &config.profile)
                && state.executions == 0 && state.control.ledger.charged == 7
            {
                dispatched = true;
                if missing { fs::remove_file(root.path("inventory.json")).unwrap(); }
                else { fs::write(root.path("topology.json"), graph(true)).unwrap(); }
            }
            ElapsedTick(1000)
        });
        peer.join().unwrap();
        human.join().unwrap();
        assert!(dispatched, "final-cut boundary was not reached: {result:?}");
        assert!(result.is_err());
        let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 0);
        assert_eq!(state.payload, b"initial");
        assert_eq!(state.control.ledger.reserved, 0);
        if state.control.ledger.charged == 0 {
            assert_eq!(state.control.ledger.stages[&1], ActionState::ConfirmedNotExecuted);
        } else {
            assert_eq!(state.control.ledger.charged, 7);
            assert!(matches!(state.control.ledger.stages[&1],
                ActionState::Dispatching | ActionState::Unknown | ActionState::IrrecoverablyUnknown));
        }
        assert_eq!(state.control.ledger.available + state.control.ledger.charged,
            config.profile.delivery.total);
        assert!(!retained(&root, state.revision, 2).available());
        assert!(!root.path("actors/precheck.sock").exists());
    }
}

#[test]
fn topology_precheck_partial_frames_cannot_delay_independent_stop_or_refresh_sources() {
    let root = Root::new();
    let mut config = configuration(&root);
    config.programs.clear();
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let selected = retained_selection(&root, &root.path("source.bin"));
    let settings = settings(&root, 1, 2);
    let proposal = candidate(&config);
    let source_path = root.path("evidence.json");
    let inventory_path = root.path("inventory.json");
    let actor_peer = actor.clone();
    let precheck_peer = settings.clone();
    let stop_peer = reviewer.clone();
    let submitted = proposal_document(&config);
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let peer = std::thread::spawn(move || {
        passed(&precheck(&actor_peer, &precheck_peer, &proposal));
        fs::remove_file(source_path).unwrap();
        fs::remove_file(inventory_path).unwrap();
        let mut partial = connect_waiting(precheck_peer.socket_path());
        partial.write_all(b"{").unwrap();
        let mut actor = connect_actor(&actor_peer);
        actor.write_all(&submitted.as_bytes()[..submitted.len() / 2]).unwrap();
        let mut output = Vec::new();
        crate::peers::stop::request(&stop_peer, 7, &mut output).unwrap();
        assert_eq!(field(&parse(&output), "status").as_str(), Some("stopped_drained"));
    });
    serve_selected_ingress(config, &actor, &reviewer, None, false, None,
        Some(selected), Some(&settings), Some(GENERATION), None, || ElapsedTick(1000)).unwrap();
    peer.join().unwrap();
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 0);
    assert!(state.control.ledger.stages.is_empty());
    assert_eq!((state.control.ledger.charged, state.control.ledger.reserved), (0, 0));
    assert_eq!(state.stop.unwrap().request().operation, 7);
    retained(&root, state.revision, 1);
    assert!(!root.path("evidence.json").exists());
    assert!(!root.path("inventory.json").exists());
    assert!(!settings.socket_path().exists());
    assert!(!actor.socket.exists());
}

#[test]
fn topology_checkpoint_cannot_replace_a_retired_precheck_socket_even_through_a_parent_alias() {
    for alias in [false, true] {
        let root = Root::new();
        let config = configuration(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let settings = settings(&root, 1, 2);
        let output = if alias {
            let parent = root.path("aliased-actors");
            std::os::unix::fs::symlink(root.path("actors"), &parent).unwrap();
            parent.join("precheck.sock")
        } else { settings.socket_path().to_path_buf() };
        let selected = retained_selection(&root, &output);
        fs::remove_file(root.path("topology.json")).unwrap();
        fs::remove_file(root.path("inventory.json")).unwrap();
        let store = config.store.clone();
        let error = serve_selected_ingress(config, &actor, &reviewer, None, false, None,
            Some(selected), Some(&settings), Some(GENERATION), None,
            || panic!("output alias crossed time/source/store admission")).unwrap_err();
        assert_eq!(error, "topology checkpoint and precheck socket must be distinct");
        assert!(!store.exists());
        assert!(!output.exists());
        assert!(!actor.socket.exists());
        assert!(!root.path("evidence.json").exists());
    }
}

#[test]
fn an_initial_bypass_cannot_expose_a_topology_precheck_disclosure() {
    let root = Root::new();
    let mut config = configuration(&root);
    config.programs.clear();
    let (actor, reviewer) = profiles(&root, &config);
    let selected = selection(&root, true);
    let settings = settings(&root, 1, 2);
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let error = serve_selected_ingress(config, &actor, &reviewer, None, false, None,
        Some(selected), Some(&settings), Some(GENERATION), None, || ElapsedTick(1000)).unwrap_err();
    assert!(error.contains("initial live-service topology was not certified"), "{error}");
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert!(state.control.ledger.stages.is_empty());
    assert_eq!(state.executions, 0);
    assert!(!root.path("evidence.json").exists());
    assert!(!settings.socket_path().exists());
    assert!(!actor.socket.exists());
}
