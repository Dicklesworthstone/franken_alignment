//! Actual imported generation, original review owners and authenticated sockets.
//! Fixture weights and process ballots test protocol/authority, not detector quality.
use super::*;
use super::super::ingress;
use super::continuation as native;
use crate::workflow::actor_service::Profile;
use fa_reference::action::consequence::delivery::{stream::ReleaseFrame,
    persistent::{FileDeliverySnapshot, observed::reviewer::{
        client::ReviewClientProgress, wire::{ReviewDecision, ReviewPacket}}}};
use fa_reference::action::consequence::oversight::{
    actor_peer::{PeerCredentials, PeerPolicy},
    actor_wire::{WireError, WireResponse, client::{ActorClient, ActorClientState, ClientError, ClientProgress,
        ClientSessionLimits, drive::ClientDriveBudget}}, helper_workers::HelperLimits,
};
use std::{collections::BTreeMap, io::Read, os::unix::net::UnixStream, path::PathBuf,
    sync::{Arc, atomic::{AtomicBool, Ordering}}, thread};

#[derive(Clone, Copy)]
enum Model { Plain, Raw, Owned, Process }
#[derive(Clone, Copy)]
pub(super) enum Behavior { Observe, Reconnect, Cancel, InvalidThenValid }

pub(super) fn actor_profile(root: &Root, config: &Config, request: u64) -> Profile {
    let (socket, _) = UnixStream::pair().unwrap();
    let credentials = PeerCredentials::observe(&socket).unwrap();
    let peer = PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap();
    Profile { request, scope: config.profile.delivery.scope, socket: root.0.join("peers/actor.sock"),
        supervisor: peer, actor: peer, candidates: 4, connections: 4, exchanges: 4096,
        runtime_ms: 60_000, poll_ms: 1, reply_ms: 500 }
}
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
fn process_recipe(root: &Root) -> String {
    let executable = quote(std::env::current_exe().unwrap().to_str().unwrap());
    let directory = quote(root.0.to_str().unwrap());
    // Reuse the existing actual child and its exact environment protocol. The
    // test does not create another implementation of commit/reveal or cleanup.
    let child = "workflow::actor_service::learned::tests::processes::process_child";
    let members = ["alpha", "beta"].into_iter().map(|name| {
        let trace = quote(root.0.join(format!("{name}.input")).to_str().unwrap());
        format!(r#""{name}":{{"executable":{executable},"directory":{directory},"arguments":["--exact","{child}","--nocapture"],"environment":{{"FA_LEARNED_PUBLICATION_PROCESS":"allow","FA_LEARNED_PUBLICATION_MEMBER":"{name}","FA_LEARNED_PUBLICATION_TRACE":{trace}}}}}"#)
    }).collect::<Vec<_>>().join(",");
    let raw = fixture::recipe(root)
        .replace("fa.learned-publication/1", "fa.learned-publication/6")
        .replace(&format!(r#""native_roster":"{}/native-roster.json""#, root.0.display()), r#""forecast":null"#)
        .replace(r#""budget":{"rounds":1"#, r#""priority":[],"budget":{"rounds":1"#);
    let (prefix, _) = raw.split_once(r#","native_review":"#).unwrap();
    format!(r#"{prefix},"process_review":{{"rounds":[{{"id":701,"members":{{{members}}}}}],"polls":4096,"input_bytes":{},"salt_bytes":32,"starts":2}}}}"#,
        HelperLimits::default().input_bytes)
}
fn setup(root: &Root, model: Model) -> (Config, PathBuf, Loaded) {
    let config = configured(root);
    let path = if matches!(model, Model::Process) {
        let path = fixture::write(root, b"allow");
        fs::write(&path, process_recipe(root)).unwrap();
        fs::remove_file(root.0.join("native-roster.json")).unwrap(); path
    } else {
        let path = native::write(root);
        match model {
            Model::Raw => { super::forecast::install(root, &path, false, 60_000); }
            Model::Owned => { super::forecast::install(root, &path, true, 60_000); }
            _ => {},
        }
        path
    };
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap(); (config, path, loaded)
}
fn inspect(root: &Root, path: &std::path::Path) -> FileDeliverySnapshot {
    let config = configured(root); let loaded = recipe::load(path, &config, false).unwrap();
    if let Some(prediction) = loaded.generation.required_owned_pre_output_forecast() {
        FileOversight::read_publication_with_owned_learned_consistency(&config.store,
            &config.profile, &loaded.generation, prediction).unwrap().publication
    } else {
        FileOversight::read_publication_with_learned_generation(&config.store,
            &config.profile, &loaded.generation).unwrap()
    }
}
fn client(profile: &Profile) -> ActorClient<UnixStream> {
    let socket = UnixStream::connect(&profile.socket).unwrap(); profile.supervisor.verify(&socket).unwrap();
    ActorClientState::new(ClientSessionLimits::default()).unwrap().connect_unix(socket).unwrap()
}
fn next_reply(client: &mut ActorClient<UnixStream>, started: Instant) -> WireResponse {
    loop {
        assert!(started.elapsed() < Duration::from_secs(25), "original actor response timed out");
        if let ClientProgress::Response(response) = client.drive(ClientDriveBudget::default()).unwrap().progress {
            return response;
        }
        thread::sleep(Duration::from_millis(1));
    }
}
pub(super) fn actor(profile: Profile, release: LearnedTextRelease, due: ElapsedTick, behavior: Behavior)
    -> thread::JoinHandle<WireResponse>
{
    thread::spawn(move || {
        let started = Instant::now(); let mut client = client(&profile);
        let proposal = FileLearnedTextStreamActorPort::encode_release(profile.request, release, due).unwrap();
        if matches!(behavior, Behavior::InvalidThenValid) {
            let mut wrong = proposal.clone();
            wrong.target.generation = if wrong.target.generation == 1 { 2 } else { 1 };
            client.submit(profile.request, &wrong).unwrap();
            assert!(next_reply(&mut client, started).result.is_err());
            // The original client correctly retains even rejected proposals.
            // Do not weaken that binding just to exercise the server's repair
            // path: this rejected client cannot change its used request key.
            assert!(matches!(client.submit(profile.request, &proposal),
                Err(ClientError::Command(WireError::IdempotencyConflict))));
            let rejected = client.into_state();
            assert_eq!(rejected.original_proposal(profile.request), Some(&wrong));
            assert!(rejected.work().written_bytes > 0);
            // A separate explicitly provisioned client supplies the valid
            // intent after the malformed server refusal. This is NOT a retry
            // that resets the rejected client's history or spent I/O budget.
            let socket = UnixStream::connect(&profile.socket).unwrap(); profile.supervisor.verify(&socket).unwrap();
            client = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect_unix(socket).unwrap();
        }
        client.submit(profile.request, &proposal).unwrap();
        let mut first = true;
        loop {
            let response = next_reply(&mut client, started);
            if matches!(response.result, Ok(Knowledge::Known { .. })) { return response; }
            assert!(matches!(response.result, Ok(Knowledge::Pending { .. } | Knowledge::Unknown { .. })), "{response:?}");
            if first && matches!(behavior, Behavior::Reconnect) {
                let state = client.into_state(); let spent = state.work();
                let socket = UnixStream::connect(&profile.socket).unwrap(); profile.supervisor.verify(&socket).unwrap();
                client = state.connect_unix(socket).unwrap();
                assert_eq!(client.state().work(), spent);
                assert_eq!(client.state().original_proposal(profile.request), Some(&proposal));
                client.retry_submission(profile.request).unwrap();
            } else if first && matches!(behavior, Behavior::Cancel) {
                client.cancel(profile.request).unwrap();
            } else {
                thread::sleep(Duration::from_millis(5));
                client.poll(profile.request).unwrap();
            }
            first = false;
        }
    })
}
pub(super) fn human(peers: PeerProfile, request: u64, choice: ReviewDecision) -> thread::JoinHandle<ReviewPacket> {
    thread::spawn(move || {
        let started = Instant::now(); let mut client = peers.connect_client(request).unwrap();
        let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(25), "human review timed out");
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(packet.is_none()); packet = Some(client.packet().unwrap().clone());
                    client.respond(choice).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn run_peer(root: &Root, path: &std::path::Path, behavior: Behavior,
    choice: Option<ReviewDecision>, due: ElapsedTick, finishing: Option<u64>)
    -> (RunResult, WireResponse, Option<ReviewPacket>)
{
    let config = configured(root); let loaded = recipe::load(path, &config, true).unwrap();
    let peers = fixture::peers(root, &config); let profile = actor_profile(root, &config, loaded.request);
    let request = loaded.request;
    let mut peer = None; let mut reviewer = None;
    let release = if finishing.is_some() { LearnedTextRelease::Finish } else { LearnedTextRelease::Message };
    let result = run_with_actor(config, loaded, &peers, finishing.is_some(), finishing, None,
        None, Some(&profile), || {
            // Start only after the real listener exists. Socket unavailability
            // is an actual test failure, never a reason to skip the behavior.
            if peer.is_none() && profile.socket.exists() {
                peer = Some(actor(profile.clone(), release, due, behavior));
            }
            if let Some(choice) = choice {
                if reviewer.is_none() && peers.socket(request).exists() {
                    reviewer = Some(human(peers.clone(), request, choice));
                }
            }
            ElapsedTick(1000)
        });
    let response = peer.expect("actor endpoint bound").join().unwrap();
    let packet = reviewer.map(|thread| thread.join().unwrap());
    assert!(!profile.socket.exists()); assert!(!peers.socket(request).exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    (result.unwrap(), response, packet)
}

#[test]
fn actual_peer_publication_uses_all_three_original_predictor_modes_and_the_accepted_shorter_deadline() {
    for mode in [Model::Plain, Model::Raw, Model::Owned] {
        let root = Root::new(); let (_, path, _) = setup(&root, mode);
        let (result, peer, packet) = run_peer(&root, &path, Behavior::Observe,
            Some(ReviewDecision::Approve), ElapsedTick(50_000), None);
        assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
        assert_eq!(peer.result, result.response.result);
        assert!(matches!(peer.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let packet = packet.unwrap(); assert_eq!(packet.action().spec().deadline, ElapsedTick(50_000));
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        let state = inspect(&root, &path); assert_eq!(state.executions, 1); assert_eq!(state.payload, b"A");
        assert_eq!(state.control.ledger.charged, packet.action().spec().units);
        assert_eq!(state.control.ledger.reserved, 0);
        let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
        let (host, _, _, _identity) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.learned_text_stream_message_request(1).unwrap().deadline, ElapsedTick(50_000));
    }
}

#[test]
fn rejected_envelopes_and_reconnects_do_not_regenerate_or_create_another_effect() {
    for behavior in [Behavior::InvalidThenValid, Behavior::Reconnect] {
        let root = Root::new(); let (_, path, _) = setup(&root, Model::Plain);
        let (result, peer, packet) = run_peer(&root, &path, behavior,
            Some(ReviewDecision::Approve), ElapsedTick(50_000), None);
        assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.response.result, peer.result); assert!(packet.is_some());
        let state = inspect(&root, &path); assert_eq!(state.executions, 1);
        assert_eq!(state.control.ledger.stages.len(), 1);
        let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
        let (host, _, _, _identity) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
    }
}

#[test]
fn actor_cancellation_stays_request_local_and_keeps_paid_numerical_work_without_a_human_key() {
    for model in [Model::Plain, Model::Raw, Model::Owned, Model::Process] {
        let root = Root::new(); let (_, path, _) = setup(&root, model);
        let (result, peer, human) = run_peer(&root, &path, Behavior::Cancel, None, ElapsedTick(50_000), None);
        assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.response.result, peer.result); assert!(human.is_none());
        assert!(matches!(peer.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        let state = inspect(&root, &path); assert_eq!(state.executions, 0); assert!(state.payload.is_empty());
        assert_eq!(state.control.ledger.reserved, 0); assert_eq!(state.control.ledger.charged, 0);
        assert!(state.stop.is_none(), "request cancellation is not an authenticated run-wide Stop");
        let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
        let (host, _, _, _identity) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
    }
}

#[test]
fn actual_external_helpers_and_independent_human_rejection_remain_on_the_peer_path() {
    for choice in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new(); let (_, path, _) = setup(&root, Model::Process);
        let (result, peer, packet) = run_peer(&root, &path, Behavior::Observe,
            Some(choice), ElapsedTick(50_000), None);
        assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.response.result, peer.result); let packet = packet.unwrap();
        assert_eq!(packet.views().len(), 2);
        for name in ["alpha", "beta"] { assert!(!fs::read(root.0.join(format!("{name}.input"))).unwrap().is_empty()); }
        let executed = matches!(choice, ReviewDecision::Approve);
        assert_eq!(matches!(peer.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), executed);
        let state = inspect(&root, &path); assert_eq!(state.executions, u64::from(executed));
        assert_eq!(state.control.ledger.charged, if executed { packet.action().spec().units } else { 0 });
        assert_eq!(state.control.ledger.reserved, 0);
    }
}

#[test]
fn an_explicit_peer_finish_requires_a_second_review_without_sampling_another_token() {
    let root = Root::new(); let config = configured(&root);
    let first = fixture::write(&root, b"allow"); evidence(&root, &config, 1, true, false);
    let (result, _, _) = run_peer(&root, &first, Behavior::Observe,
        Some(ReviewDecision::Approve), ElapsedTick(50_000), None);
    assert!(result.failure.is_none());
    let path = root.0.join("finish.json");
    fs::write(&path, fs::read_to_string(&first).unwrap().replace("\"request\":1", "\"request\":2")
        .replace("\"object_id\":1001", "\"object_id\":1002")).unwrap();
    let loaded = recipe::load(&path, &config, true).unwrap(); assert_eq!(loaded.rounds, [2]);
    let (result, peer, packet) = run_peer(&root, &path, Behavior::Reconnect,
        Some(ReviewDecision::Approve), ElapsedTick(50_000), Some(1));
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert_eq!(peer.result, result.response.result);
    let packet = packet.unwrap();
    assert!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().is_finish());
    assert_eq!(inspect(&root, &path).executions, 2);
    let (host, _, _, _identity) = recovery::open_selected(&config, &loaded).unwrap();
    assert!(host.stream_snapshot().unwrap().confirmed.finished());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
    assert_eq!(host.learned_text_stream_finish_request(1, 2).unwrap().deadline, ElapsedTick(50_000));
}

#[test]
fn observation_phase_cannot_admit_a_missing_request_even_with_an_unused_policy_snapshot() {
    let root = Root::new(); let (mut config, _, loaded) = setup(&root, Model::Plain);
    let (host, human, _, _identity) = recovery::create_selected(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port.clone()); let mut driver = FileSupervisedDriver::new(supervisor);
    let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &human, &mut config, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
    let revision = driver.supervisor().host().unwrap().revision();
    driver.supervisor_mut().set_snapshot(revision, Some(fa_reference::Snapshot { semantic_epoch: 1,
        complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) })).unwrap();
    let profile = actor_profile(&root, &config, 1);
    let mut intake = Some(ingress::Bound::bind(&profile).unwrap()
        .attach(port, LearnedTextRelease::Message, deadline.logical).unwrap());
    let retry = Arc::new(AtomicBool::new(false)); let rejected = Arc::new(AtomicBool::new(false));
    let retry_peer = Arc::clone(&retry); let rejected_peer = Arc::clone(&rejected);
    let peer = thread::spawn(move || {
        let started = Instant::now(); let mut client = client(&profile);
        let proposal = FileLearnedTextStreamActorPort::encode_release(1, LearnedTextRelease::Message, ElapsedTick(50_000)).unwrap();
        client.submit(1, &proposal).unwrap(); assert!(next_reply(&mut client, started).result.is_err());
        rejected_peer.store(true, Ordering::Release);
        while !retry_peer.load(Ordering::Acquire) {
            assert!(started.elapsed() < Duration::from_secs(25)); thread::sleep(Duration::from_millis(1));
        }
        client.submit(1, &proposal).unwrap(); next_reply(&mut client, started)
    });
    let reads = config.source.status().read_attempts;
    let bytes = fs::read(config.store.join("delivery.bin")).unwrap(); let started = Instant::now();
    while !rejected.load(Ordering::Acquire) {
        assert!(started.elapsed() < Duration::from_secs(25));
        assert!(!ingress::observe(&mut intake, driver.supervisor_mut()).unwrap());
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(driver.supervisor().host().unwrap().revision(), revision);
    assert_eq!(config.source.status().read_attempts, reads);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
    assert!(matches!(driver.supervisor().host().unwrap().request_status(1), Err(JournalError::Contract(Error::Missing))));
    retry.store(true, Ordering::Release);
    assert!(intake.as_mut().unwrap().admit(&mut driver, &mut config, &human, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    assert_eq!(intake.as_ref().unwrap().observe_original(&driver, &mut wire, LearnedTextRelease::Message, None).unwrap(), ElapsedTick(50_000));
    // A successful intake need not have flushed the socket reply in that same
    // bounded drive. Exercise the real non-admitting response service before join.
    intake.as_mut().unwrap().finish(driver.supervisor_mut(), &deadline, &mut || ElapsedTick(1000)).unwrap();
    let response = peer.join().unwrap(); assert!(matches!(response.result, Ok(Knowledge::Pending { .. })));
    assert!(config.source.status().read_attempts > reads); assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    cancel_unspent(&mut driver, 1).unwrap(); drop(intake); assert_eq!(cleanup(driver, 1000, 1), 0);
}

#[test]
fn completed_generation_cannot_substitute_for_a_missing_actor_request() {
    let root = Root::new(); let (mut config, _, loaded) = setup(&root, Model::Plain);
    let (host, human, _, _identity) = recovery::create_selected(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor); let deadline = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &human, &mut config, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
    let before = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    assert_eq!(before.sampled_draws, 2); let reads = config.source.status().read_attempts;
    let profile = actor_profile(&root, &config, 1);
    let mut intake = ingress::Bound::bind(&profile).unwrap().attach(port, LearnedTextRelease::Message, deadline.logical).unwrap();
    let mut calls = 0;
    let error = intake.admit(&mut driver, &mut config, &human, &mut control, &deadline,
        &mut || { calls += 1; if calls >= 8 { deadline.logical } else { ElapsedTick(1000) } }).unwrap_err();
    assert!(error.contains("deadline"));
    assert!(matches!(driver.supervisor().host().unwrap().request_status(1), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical, before);
    assert_eq!(config.source.status().read_attempts, reads); drop(intake); assert_eq!(cleanup(driver, 1000, 1), 0);
}

#[test]
fn foreign_scope_and_request_refuse_before_clock_and_socket_collision_precedes_store_creation() {
    for wrong in [0, 1, 2] {
        let root = Root::new(); let (config, _, loaded) = setup(&root, Model::Plain);
        let peers = fixture::peers(&root, &config); let mut actor = actor_profile(&root, &config, 1);
        match wrong { 0 => actor.request = 2, 1 => actor.scope.run += 1, _ => actor.socket = peers.socket(1) }
        let store = config.store.clone();
        assert!(run_with_actor(config, loaded, &peers, false, None, None, None, Some(&actor),
            || panic!("invalid independent actor profile cannot read time")).is_err());
        assert!(!store.exists()); assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
    let root = Root::new(); let (config, _, loaded) = setup(&root, Model::Plain);
    let peers = fixture::peers(&root, &config); let actor = actor_profile(&root, &config, 1);
    fs::write(&actor.socket, b"keep unrelated socket path").unwrap(); let store = config.store.clone();
    assert!(run_with_actor(config, loaded, &peers, false, None, None, None, Some(&actor), || ElapsedTick(1000)).is_err());
    assert!(!store.exists()); assert_eq!(fs::read(&actor.socket).unwrap(), b"keep unrelated socket path");
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 1);
}

#[test]
fn rejected_kernel_credentials_cannot_admit_or_observe_a_learned_publication() {
    let root = Root::new(); let (config, path, loaded) = setup(&root, Model::Plain);
    let peers = fixture::peers(&root, &config); let mut actor = actor_profile(&root, &config, 1);
    actor.actor = PeerPolicy::new(if actor.actor.uid() == 0 { 1 } else { 0 }, actor.actor.gid(), actor.actor.pid()).unwrap();
    actor.candidates = 1; actor.connections = 1;
    let mut peer = None;
    let result = run_with_actor(config, loaded, &peers, false, None, None, None, Some(&actor), || {
        if peer.is_none() && actor.socket.exists() {
            let path = actor.socket.clone();
            peer = Some(thread::spawn(move || {
                let mut socket = UnixStream::connect(path).unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(25))).unwrap();
                assert_eq!(socket.read(&mut [0; 1]).unwrap(), 0);
            }));
        }
        ElapsedTick(1000)
    }).unwrap();
    peer.unwrap().join().unwrap(); assert!(result.failure.is_some()); assert_eq!(result.cleanup_pending, 0);
    let state = inspect(&root, &path); assert_eq!(state.executions, 0); assert!(state.control.ledger.stages.is_empty());
    assert!(!actor.socket.exists()); assert!(!peers.socket(1).exists());
}

#[test]
fn actor_flag_has_a_closed_live_grammar_and_does_not_upgrade_receipt_stop_or_audit_modes() {
    let base: Vec<String> = ["create-learned-generated", "/config", "/recipe", "/reviewer",
        "--actor-profile", "/actor"].into_iter().map(str::to_owned).collect();
    let (args, actor) = ingress::take_option(&base).unwrap();
    assert_eq!(args, base[..4]); assert_eq!(actor, Some(std::path::Path::new("/actor")));
    for extra in [vec!["--continue-generation"], vec!["--after-message", "1"], vec!["--finish-after-message", "1"],
        vec!["--history-anchor", "/anchor", "--retain-history-anchor", "/new-anchor"]] {
        let mut full = base.clone(); full.extend(extra.into_iter().map(str::to_owned));
        assert!(ingress::take_option(&full).is_ok());
    }
    for extra in [vec!["--actor-profile", "/other"], vec!["--history-anchor", "--continue-generation"],
        vec!["--continue-generation", "--after-message", "1"], vec!["--resume"],
        vec!["--recover-stop", "1"], vec!["--audit-review", "1", "/anchor"],
        vec!["--receipts-only"], vec!["--reconcile-receipts"], vec!["--unknown", "/value"]] {
        let mut full = base.clone(); full.extend(extra.into_iter().map(str::to_owned));
        assert!(ingress::take_option(&full).is_err());
        assert!(command(&full, None).is_err());
    }
    for index in [1, 2, 3, 5] {
        let mut wrong = base.clone(); wrong[index] = "--actor-profile".into();
        assert!(ingress::take_option(&wrong).is_err());
    }
}
