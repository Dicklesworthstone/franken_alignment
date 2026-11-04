//! The actual single-request service owns both sockets; a precheck never replaces
//! source-aware Submit, independent review, or the original publication workflow.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{PrecheckRequest, PrecheckWireResponse, client::PrecheckPeerClient},
};
use std::io::Write;

fn settings(root: &Directory) -> precheck::Settings {
    precheck::Settings::decode(format!(
        r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}/actors/precheck.sock","expected_policy_epoch":0,"disclosure_id":81,"query_limit":4,"exchange_limit":4,"candidate_limit":8,"connection_limit":4,"runtime_ms":15000}}"#,
        root.0.display(),
    ).as_bytes()).unwrap()
}

fn query(profile: &Profile, socket: &Path, proposal: ActorProposal) -> PrecheckWireResponse {
    wait(socket);
    let mut client = PrecheckPeerClient::connect(UnixStream::connect(socket).unwrap(),
        profile.supervisor, PrecheckRequest { query: 7, proposal }, 0).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10), "precheck reply timed out");
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() { return response; }
        assert!(!report.status.closed(), "closed without the native response");
        pause(1);
    }
}

fn passed(response: PrecheckWireResponse) {
    assert_eq!(response.query, Some(7));
    let Ok(Knowledge::Known { value, basis }) = response.result else {
        panic!("expected the original exact precheck result");
    };
    assert_eq!(value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!((basis.query, basis.disclosure, basis.policy_epoch), (7, 81, 0));
    assert_eq!(basis.observed_at, ElapsedTick(1000));
}

#[test]
fn opt_in_precheck_then_original_submit_still_runs_both_keys_and_one_effect() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root);
    let precheck_socket = root.0.join("actors/precheck.sock");
    let selected_socket = precheck_socket.clone();
    let review_socket = reviewer.socket(7);
    let submit = proposal(&config);
    let Command::Submit { proposal: candidate, .. } = submit.clone() else { unreachable!() };
    let submit_path = root.0.join("submit.json");
    fs::write(&submit_path, encode_command(&submit).unwrap()).unwrap();
    let peer_profile = actor.clone();
    let peer = std::thread::spawn(move || {
        passed(query(&peer_profile, &selected_socket, candidate));
        assert!(!review_socket.exists(), "precheck launched an ordinary review");
        let mut output = Vec::new();
        client::submit(&peer_profile, &submit_path, &mut output).unwrap();
        decode_response(output.strip_suffix(b"\n").unwrap()).unwrap()
    });
    let human = human(reviewer.clone());
    serve_selected_with_precheck(config, &actor, &reviewer, None, false, None,
        None, Some(&settings), || ElapsedTick(1000)).unwrap();
    human.join().unwrap();
    assert!(matches!(peer.join().unwrap().result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.payload, b"network");
    assert_eq!((disk.control.ledger.charged, disk.control.ledger.reserved), (7, 0));
    assert!(disk.stop.is_none());
    assert!(!precheck_socket.exists());
    assert!(!actor.socket.exists());
    eprintln!("{{\"case\":\"precheck_then_submit\",\"executions\":1,\"charged\":7,\"precheck_effect_authority\":false}}");
}

#[test]
fn a_pass_cannot_replace_the_fresh_source_read_on_actual_submit() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root);
    let socket = root.0.join("actors/precheck.sock");
    let source_path = root.0.join("evidence.json");
    let review_socket = reviewer.socket(7);
    let changed = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 2,
        scope: config.profile.delivery.scope }, Snapshot { semantic_epoch: 1, complete: true,
        values: BTreeMap::from([(7, b"blocked".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|member|
            (member.to_owned(), format!("review this fixture: {member}").into_bytes())).collect(),
    ).unwrap().encode();
    let submit = proposal(&config);
    let Command::Submit { proposal: candidate, .. } = submit.clone() else { unreachable!() };
    let peer_profile = actor.clone();
    let peer = std::thread::spawn(move || {
        passed(query(&peer_profile, &socket, candidate));
        fs::write(source_path, changed).unwrap();
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        exchange(connect(&peer_profile), submit, &mut budget).1
    });
    serve_selected_with_precheck(config, &actor, &reviewer, None, false, None,
        None, Some(&settings), || ElapsedTick(1000)).unwrap();
    assert!(matches!(peer.join().unwrap().result,
        Ok(Knowledge::Known { value: ActorOutcome::Denied, .. })));
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 0);
    assert_eq!((disk.control.ledger.charged, disk.control.ledger.reserved), (0, 0));
    assert!(disk.stop.is_none());
    assert!(!review_socket.exists());
    eprintln!("{{\"case\":\"fresh_submit_after_precheck\",\"source_generation\":2,\"executions\":0,\"ordinary_result\":\"Denied\"}}");
}

#[test]
fn malformed_precheck_ingress_cannot_stop_the_ordinary_workflow() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root);
    let socket = root.0.join("actors/precheck.sock");
    let submit_path = root.0.join("submit.json");
    fs::write(&submit_path, encode_command(&proposal(&config)).unwrap()).unwrap();
    let peer_profile = actor.clone();
    let peer = std::thread::spawn(move || {
        wait(&socket);
        let mut broken = UnixStream::connect(&socket).unwrap();
        broken.write_all(b"{").unwrap();
        broken.shutdown(std::net::Shutdown::Write).unwrap();
        let started = Instant::now();
        while socket.exists() {
            assert!(started.elapsed() < Duration::from_secs(10), "sidecar did not retire");
            pause(1);
        }
        let mut output = Vec::new();
        client::submit(&peer_profile, &submit_path, &mut output).unwrap();
        decode_response(output.strip_suffix(b"\n").unwrap()).unwrap()
    });
    let human = human(reviewer.clone());
    serve_selected_with_precheck(config, &actor, &reviewer, None, false, None,
        None, Some(&settings), || ElapsedTick(1000)).unwrap();
    human.join().unwrap();
    assert!(matches!(peer.join().unwrap().result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert!(disk.stop.is_none(), "read-only precheck traffic invoked ordinary stop");
    eprintln!("{{\"case\":\"truncated_precheck_isolated\",\"executions\":1,\"ordinary_stop\":false}}");
}

#[test]
fn precheck_option_owns_its_grammar_before_any_other_mode_can_consume_it() {
    for (mode, count) in [("serve-create", 4), ("serve-open", 4),
        ("serve-create-checked", 5), ("serve-open-checked", 5)] {
        let mut args = vec![mode.to_owned()];
        args.extend((1..count).map(|index| format!("file-{index}")));
        let original = args.clone();
        args.extend(["--precheck-profile".into(), "precheck.json".into()]);
        let (selected, path) = take_precheck_option(&args).unwrap();
        assert_eq!(selected, original);
        assert_eq!(path, Some(Path::new("precheck.json")));
    }
    let unchanged = vec!["actor-submit".into(), "profile".into(), "document".into()];
    assert_eq!(take_precheck_option(&unchanged).unwrap(), (unchanged.as_slice(), None));
    for values in [
        vec!["actor-submit", "a", "b", "--precheck-profile", "p"],
        vec!["create-generated", "c", "a", "r", "--precheck-profile", "p"],
        vec!["serve-create", "c", "a", "r", "--requests", "7,8", "--precheck-profile", "p"],
        vec!["serve-create", "c", "a", "r", "--precheck-profile", "p", "--peers", "a"],
        vec!["serve-create", "c", "a", "r", "--precheck-profile"],
        vec!["serve-create", "c", "a", "r", "--precheck-profile", ""],
        vec!["serve-create", "c", "a", "r", "--precheck-profile", "p", "--precheck-profile", "q"],
        vec!["serve-create", "c", "--receipts-only", "r", "--precheck-profile", "p"],
        vec!["serve-create", "c", "a", "r", "--topology-profile", "t", "--precheck-profile", "p"],
    ] {
        let args: Vec<String> = values.into_iter().map(str::to_owned).collect();
        assert!(take_precheck_option(&args).is_err(), "accepted {args:?}");
        assert!(command(&args, None).is_err(), "dispatched unsupported {args:?}");
    }
    eprintln!("{{\"case\":\"precheck_closed_grammar\",\"allowed_modes\":4,\"rejected_combinations\":9}}");
}
