//! Real learned inference, authenticated observation, separate human authority
//! and independent Stop. Fixture weights establish protocol behavior only.
#[path = "operator/admission_explanation.rs"]
mod admission_explanation;
#[path = "operator/review_explanation.rs"]
mod review_explanation;
#[path = "operator/resolution.rs"]
mod resolution;

use super::super::operator as live_operator;
use super::peer_intake::{Behavior, actor, actor_profile, human};
use super::*;
use crate::workflow::actor_service::Profile;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    peer::control::StopClientProgress, wire::ReviewDecision,
};
use fa_reference::action::consequence::oversight::{
    actor_peer::PeerPolicy, evidence_source::FileEvidenceSource,
};
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::{MAX_REQUEST_BYTES, TowerRequest, Verb, encode_request};
use std::{
    io::Read,
    os::unix::{fs::symlink, net::UnixStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

const GENERATION: u64 = 41;

fn selection(root: &Root, actor: &Profile) -> (PathBuf, live_operator::Selection) {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let path = root.0.join("operator.json");
    fs::write(&path, format!(r#"{{"schema":"fa.tower-operator-service/1","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}/peers/operator.sock","supervisor":{{"uid":{},"gid":{},"pid":{}}},"operator":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":4,"connection_limit":4,"exchange_limit":16,"runtime_ms":60000,"poll_ms":1,"inspection_queries":3,"explanation_queries":0,"explanation_replay_events":0,"explanation_disclosure":"none"}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority, root.0.display(),
        identity.uid(), identity.gid(), identity.pid().unwrap(),
        identity.uid(), identity.gid(), identity.pid().unwrap())).unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    (path, selected)
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| &value.as_object().unwrap()[*key])
}
fn summary(value: &Json, name: &str) -> u64 {
    field(
        value,
        &["result", "value", "knowledge", "value", "summary", name],
    )
    .as_str()
    .unwrap()
    .parse()
    .unwrap()
}
fn known(value: &Json) {
    assert_eq!(
        field(value, &["result", "kind"]).as_str(),
        Some("situation")
    );
    assert_eq!(
        field(value, &["result", "value", "knowledge", "state"]).as_str(),
        Some("known")
    );
    assert_eq!(
        field(value, &["result", "value", "current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!(
        field(value, &["result", "value", "knowledge", "basis", "origin"]).as_str(),
        Some("acknowledged_owner")
    );
}
fn wait_for(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "socket never appeared: {}",
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
fn situation(socket: &mut UnixStream, selected: &live_operator::Selection, id: &str) -> Json {
    let scope = selected.settings.scope;
    let mount = Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(selected.generation),
    )
    .unwrap();
    let mut bytes = encode_request(&TowerRequest::new(id, mount, Verb::Situation))
        .unwrap()
        .into_bytes();
    bytes.push(b'\n');
    socket.write_all(&bytes).unwrap();
    let mut response = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        socket.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
        assert!(response.len() <= selected.settings.response_bytes + 1);
        if byte[0] == b'\n' {
            break;
        }
    }
    strict_json::parse(&response, KNOWLEDGE_JSON_LIMITS).unwrap()
}

#[test]
fn real_command_observes_before_actor_admission_during_human_review_and_terminal_grace() {
    for process in [false, true] {
        let root = Root::new();
        let config = configured(&root);
        let path = fixture::write(&root, b"allow");
        if process {
            fs::write(&path, super::processes::process_recipe(&root, "allow")).unwrap();
        }
        evidence(&root, &config, 1, true, false);
        let peers = fixture::peers(&root, &config);
        let profile = actor_profile(&root, &config, 1);
        let (operator_path, selected) = selection(&root, &profile);
        let operator_socket = selected.settings.socket.clone();
        let mut args = super::mediated_topology::command_inputs(&root, &config, &path, &profile);
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
        let human_socket = peers.socket(1);
        let observer = thread::spawn(move || {
            let mut socket = connect(&selected);
            let first = situation(&mut socket, &selected, "before-actor");
            known(&first);
            assert_eq!(
                (
                    summary(&first, "retained_requests"),
                    summary(&first, "executions")
                ),
                (0, 0)
            );
            release_actor.store(true, Ordering::Release);
            wait_for(&human_socket);
            // The human client cannot supply its key until this original
            // observation succeeds. Removing human-phase service deadlocks.
            let awaiting = situation(&mut socket, &selected, "awaiting-human");
            known(&awaiting);
            assert_eq!(
                (
                    summary(&awaiting, "retained_requests"),
                    summary(&awaiting, "executions")
                ),
                (1, 0)
            );
            release_human.store(true, Ordering::Release);
            let started = Instant::now();
            while !finished.load(Ordering::Acquire) {
                assert!(started.elapsed() < Duration::from_secs(25));
                thread::sleep(Duration::from_millis(1));
            }
            let terminal = situation(&mut socket, &selected, "terminal-grace");
            known(&terminal);
            assert_eq!(
                (
                    summary(&terminal, "retained_requests"),
                    summary(&terminal, "executions")
                ),
                (1, 1)
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
            if reviewer.is_none() && human_ready.load(Ordering::Acquire) && peers.socket(1).exists()
            {
                reviewer = Some(human(peers.clone(), 1, ReviewDecision::Approve));
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
        assert_eq!(packet.action().spec().deadline, ElapsedTick(50_000));
        let loaded = recipe::load(&path, &config, false).unwrap();
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
        if process {
            for name in ["alpha", "beta"] {
                assert!(
                    !fs::read(root.0.join(format!("{name}.input")))
                        .unwrap()
                        .is_empty()
                );
            }
        }
        assert!(!operator_socket.exists());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn broken_operator_transport_retires_only_its_sidecar_and_the_real_publication_completes() {
    let root = Root::new();
    let config = configured(&root);
    let path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, 1);
    let (operator_path, selected) = selection(&root, &profile);
    let remote = live_operator::Selection::read(&operator_path, GENERATION).unwrap();
    let retired = Arc::new(AtomicBool::new(false));
    let release_actor = Arc::clone(&retired);
    let observer = thread::spawn(move || {
        let mut socket = connect(&remote);
        known(&situation(&mut socket, &remote, "before-malformed"));
        socket
            .write_all(&vec![b'x'; MAX_REQUEST_BYTES + 1])
            .unwrap();
        let started = Instant::now();
        while remote.settings.socket.exists() {
            assert!(started.elapsed() < Duration::from_secs(25));
            thread::sleep(Duration::from_millis(1));
        }
        // No actor intent has been submitted yet. The following real actor
        // proves this retirement did not terminate the original publication.
        release_actor.store(true, Ordering::Release);
    });
    let mut peer = None;
    let mut reviewer = None;
    let result = run_with_actor_services(
        config,
        loaded,
        &peers,
        false,
        None,
        None,
        None,
        Some(&profile),
        None,
        Some(&selected),
        || {
            if peer.is_none() && retired.load(Ordering::Acquire) && profile.socket.exists() {
                peer = Some(actor(
                    profile.clone(),
                    LearnedTextRelease::Message,
                    ElapsedTick(50_000),
                    Behavior::Observe,
                ));
            }
            if reviewer.is_none() && peers.socket(1).exists() {
                reviewer = Some(human(peers.clone(), 1, ReviewDecision::Approve));
            }
            ElapsedTick(1000)
        },
    )
    .unwrap();
    observer.join().unwrap();
    reviewer.unwrap().join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert_eq!(result.response.result, peer.unwrap().join().unwrap().result);
    assert!(matches!(
        result.response.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    ));
    assert!(!selected.settings.socket.exists());
}

#[test]
fn independent_authenticated_stop_wins_while_the_operator_holds_a_partial_frame() {
    let root = Root::new();
    let config = configured(&root);
    let path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let generation = loaded.generation.clone();
    let store = config.store.clone();
    let host_profile = config.profile.clone();
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, 1);
    let (operator_path, selected) = selection(&root, &profile);
    let remote = live_operator::Selection::read(&operator_path, GENERATION).unwrap();
    let stopping = peers.clone();
    let observer = thread::spawn(move || {
        let mut socket = connect(&remote);
        let first = situation(&mut socket, &remote, "before-stop");
        known(&first);
        assert_eq!(summary(&first, "retained_requests"), 0);
        socket.write_all(b"{\"request_id\":").unwrap();
        let mut stop = stopping.connect_stop_client(1).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(25));
            match stop.step().unwrap() {
                StopClientProgress::NeedsDecision => stop.request_stop().unwrap(),
                StopClientProgress::Complete => return stop.receipt().unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    });
    let result = run_with_actor_services(
        config,
        loaded,
        &peers,
        false,
        None,
        None,
        None,
        Some(&profile),
        None,
        Some(&selected),
        || ElapsedTick(1000),
    )
    .unwrap();
    observer.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    let state =
        FileOversight::read_publication_with_learned_generation(&store, &host_profile, &generation)
            .unwrap();
    assert_eq!(state.executions, 0);
    assert!(state.control.ledger.stages.is_empty());
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (0, 0)
    );
    assert!(state.stop.is_some());
    assert!(!selected.settings.socket.exists());
    assert!(!profile.socket.exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

#[test]
fn operator_pair_cannot_be_silently_removed_from_offline_or_ambiguous_grammar() {
    let base: Vec<_> = [
        "create-learned-generated",
        "/config",
        "/recipe",
        "/reviewer",
        "--actor-profile",
        "/actor",
        "--operator-profile",
        "/operator",
        "--tower-generation",
        "41",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for extra in [
        vec![],
        vec!["--continue-generation"],
        vec!["--after-message", "1"],
        vec!["--finish-after-message", "1"],
        vec!["--precheck-profile", "/precheck"],
        vec!["--checkpoint-before-generation", "/checkpoint"],
        vec!["--campaign-profile", "/campaign"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        let (remaining, selected) = live_operator::take_option(&args).unwrap();
        assert_eq!(selected, Some((PathBuf::from("/operator"), 41)));
        assert!(
            !remaining
                .iter()
                .any(|arg| matches!(arg.as_str(), "--operator-profile" | "--tower-generation"))
        );
    }
    let mut invalid = Vec::new();
    for extra in [
        vec!["--resume"],
        vec!["--recover-stop", "1"],
        vec!["--drain-stopped", "1"],
        vec!["--audit-review", "1", "/anchor"],
        vec!["--policy-replay", "/candidate"],
        vec!["--prepare-stream-checkpoint", "/profile"],
        vec!["--capture-stream-checkpoint", "1"],
        vec!["--operator-profile", "/again"],
        vec!["--tower-generation", "42"],
        vec!["--continue-generation", "--after-message", "1"],
        vec!["--unknown", "/value"],
    ] {
        let mut args = base.clone();
        args.extend(extra.into_iter().map(str::to_owned));
        invalid.push(args);
    }
    for generation in ["0", "-1", "+1", "18446744073709551616", ""] {
        let mut args = base.clone();
        *args.last_mut().unwrap() = generation.into();
        invalid.push(args);
    }
    for cut in [4..6, 6..8, 8..10] {
        let mut args = base.clone();
        args.drain(cut);
        invalid.push(args);
    }
    let mut receipt = base.clone();
    receipt[3] = "--resume".into();
    invalid.push(receipt);
    for args in invalid {
        let expected = live_operator::take_option(&args)
            .err()
            .expect("incompatible live selection must refuse");
        assert_eq!(
            command_with_clock(&args, None, || panic!("invalid operator mode reached time"))
                .unwrap_err(),
            expected
        );
        assert_eq!(
            crate::workflow::actor_service::command(&args, None).unwrap_err(),
            expected
        );
    }
}

#[test]
fn operator_identity_source_and_output_collisions_refuse_before_time_or_owner_startup() {
    for failure in 0..9 {
        let root = Root::new();
        let mut config = configured(&root);
        let path = fixture::write(&root, b"allow");
        let mut loaded = recipe::load(&path, &config, true).unwrap();
        let peers = fixture::peers(&root, &config);
        let profile = actor_profile(&root, &config, 1);
        let (_, mut selected) = selection(&root, &profile);
        match failure {
            0 => selected.settings.scope.run += 1,
            1 => {
                let identity = selected.settings.supervisor;
                let pid = identity.pid().unwrap();
                selected.settings.supervisor = PeerPolicy::new(
                    identity.uid(),
                    identity.gid(),
                    Some(if pid == i32::MAX as u32 {
                        pid - 1
                    } else {
                        pid + 1
                    }),
                )
                .unwrap();
            }
            2 => selected.generation = 0,
            3 => {
                config.source = FileEvidenceSource::new(
                    &selected.settings.socket,
                    51,
                    config.profile.delivery.scope,
                    1_048_576,
                )
                .unwrap()
            }
            4 => loaded.anchor_output = Some(selected.settings.socket.clone()),
            5 => selected.settings.socket = peers.socket(1),
            6 => selected.settings.socket = crate::workflow::control::socket_path(&peers, 1),
            7 => {
                symlink(&root.0, root.0.join("alias")).unwrap();
                selected.settings.socket = root.0.join("alias/peers/actor.sock");
            }
            8 => {
                symlink(&root.0, root.0.join("alias")).unwrap();
                config.store = root.0.join("alias/peers");
            }
            _ => unreachable!(),
        }
        let store = config.store.clone();
        let existed = store.exists();
        assert!(
            run_with_actor_services(
                config,
                loaded,
                &peers,
                false,
                None,
                None,
                None,
                Some(&profile),
                None,
                Some(&selected),
                || panic!("invalid selected custody reached time")
            )
            .is_err(),
            "case {failure}"
        );
        assert_eq!(store.exists(), existed);
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
    let root = Root::new();
    let (config, _, loaded) = super::mediated_topology::selected(&root, None, false);
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, 1);
    let (_, mut selected) = selection(&root, &profile);
    selected.settings.socket = root.0.join("source-1.bin");
    let store = config.store.clone();
    assert!(
        run_with_actor_services(
            config,
            loaded,
            &peers,
            false,
            None,
            None,
            None,
            Some(&profile),
            None,
            Some(&selected),
            || panic!("topology output collision reached time")
        )
        .is_err()
    );
    assert!(!store.exists());
    assert!(!selected.settings.socket.exists());
}

#[test]
fn cached_stop_failure_is_reported_after_the_original_terminal_actor_reply_is_delivered() {
    let root = Root::new();
    let mut config = configured(&root);
    let path = fixture::write(&root, b"allow");
    evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, loaded.request);
    let mut control = Control::new(&config, loaded.request, Some(&peers)).unwrap();
    let (host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port.clone());
    let mut driver = FileSupervisedDriver::new(supervisor);
    let limit = deadline();
    assert!(
        generate(
            &mut driver,
            &reviewer,
            &mut config,
            &mut control,
            &limit,
            &mut || ElapsedTick(1000)
        )
        .unwrap()
    );
    submit(
        &mut driver,
        &mut wire,
        &mut config,
        &loaded,
        &limit,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let mut intake = super::super::ingress::Bound::bind(&profile)
        .unwrap()
        .attach(port, LearnedTextRelease::Message, limit.logical)
        .unwrap();

    // A real, kernel-authenticated but malformed Stop exchange fails once.
    // No native Stop request or publication key is encoded by these bytes.
    let mut malformed =
        UnixStream::connect(crate::workflow::control::socket_path(&peers, 1)).unwrap();
    // Native Stop is a fixed-size request, not a line protocol. Supply its
    // complete invalid frame so binding failure precedes session expiry.
    // The closed native Stop protocol uses a 104-byte offer/request. Its wire
    // module stays private; this malformed peer deliberately writes raw bytes.
    malformed.write_all(&[0_u8; 104]).unwrap();
    let before_failure = fs::read(config.store.join("delivery.bin")).unwrap();
    let error = control
        .checkpoint(&mut driver, &reviewer, &limit, &mut || ElapsedTick(1000))
        .unwrap_err();
    assert!(
        error.contains("Binding"),
        "complete malformed request must fail binding: {error}"
    );
    assert_eq!(
        fs::read(config.store.join("delivery.bin")).unwrap(),
        before_failure
    );
    // The same failed control service cannot accept a replacement connection,
    // refresh its allowance, read another clock or erase the original error.
    assert_eq!(
        control
            .checkpoint(&mut driver, &reviewer, &limit, &mut || panic!(
                "cached Stop failure must not restart its exchange"
            ))
            .unwrap_err(),
        error
    );
    drop(malformed);

    // This is the same failure cleanup used by execute_request: stop and drain
    // the original admitted request before attempting its terminal response.
    stop(&mut driver, loaded.request, &mut || ElapsedTick(1000)).unwrap();
    let expected = response(&mut wire, loaded.request);
    assert!(matches!(
        expected.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::CancelledBeforeDispatch,
            ..
        })
    ));
    let before_grace = fs::read(config.store.join("delivery.bin")).unwrap();
    let source_reads = config.source.status().read_attempts;
    let peer = actor(
        profile.clone(),
        LearnedTextRelease::Message,
        limit.logical,
        Behavior::Observe,
    );
    // The actor connects only AFTER the error is cached and the owner is
    // terminal. Nothing before finish_serviced can flush this response.
    assert_eq!(
        intake
            .finish_serviced(
                &mut driver,
                &reviewer,
                &mut control,
                &limit,
                &mut || ElapsedTick(1000),
                &mut None
            )
            .unwrap_err(),
        error
    );
    assert_eq!(peer.join().unwrap().result, expected.result);
    assert_eq!(
        fs::read(config.store.join("delivery.bin")).unwrap(),
        before_grace
    );
    assert_eq!(config.source.status().read_attempts, source_reads);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    drop(intake);
    assert!(!profile.socket.exists());
    assert_eq!(cleanup(driver, 1000, 1), 0);
}


#[test]
fn unbudgeted_receipt_resolution_is_refused_before_learned_time_owner_or_listeners() {
    let root = Root::new();
    let config = configured(&root);
    let recipe_path = fixture::write(&root, b"allow");
    let loaded = recipe::load(&recipe_path, &config, true).unwrap();
    let peers = fixture::peers(&root, &config);
    let profile = actor_profile(&root, &config, 1);
    let (path, mut selected) = selection(&root, &profile);
    let document = fs::read_to_string(&path).unwrap().replacen(
        '{',
        "{\"resolution\":{\"queries\":1,\"operations\":1,\"max_replay_events\":4096,\"authority\":\"reconcile_existing_receipts\"},",
        1,
    );
    selected.settings = crate::workflow::actor_service::operator::Settings::decode(
        document.as_bytes(),
    ).unwrap();
    assert!(selected.settings.resolution.is_some());
    fs::write(&path, document).unwrap();
    assert!(live_operator::Selection::read(&path, GENERATION).is_err());
    let store = config.store.clone();
    assert!(!store.exists());
    assert!(run_with_actor_services(
        config,
        loaded,
        &peers,
        false,
        None,
        None,
        None,
        Some(&profile),
        None,
        Some(&selected),
        || panic!("unbudgeted resolution reached a learned clock or owner"),
    ).is_err());
    assert!(!store.exists());
    assert!(!selected.settings.socket.exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}
