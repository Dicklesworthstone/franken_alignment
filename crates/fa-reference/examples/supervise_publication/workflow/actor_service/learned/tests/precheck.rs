//! Learned release precheck uses real generation and the original immutable
//! cumulative action. Socket tests continue through native helpers and humans.
use super::*;
use super::super::precheck;
use super::peer_intake::{Behavior, actor, actor_profile, human};
use crate::workflow::actor_service::Profile;
use fa_reference::action::consequence::delivery::{
    persistent::{FileDeliverySnapshot, requests::actor::{
        FileActorPrecheckPort, FilePrecheckOutcome, FilePrecheckDisclosure,
        precheck_wire::{PrecheckRequest, PrecheckWireResponse, client::PrecheckPeerClient},
    }, observed::reviewer::{wire::{ReviewDecision, ReviewPacket},
        peer::control::StopClientProgress}},
    stream::ReleaseFrame,
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    actor::{ActorError, ActorProposal}, actor_transport::DriveBudget,
    evidence_source::EvidenceIdentity,
};
use std::{collections::BTreeMap, io::Write, os::unix::net::UnixStream, path::Path,
    sync::{Arc, atomic::{AtomicBool, Ordering}}, thread};

fn settings(root: &Root, epoch: u64) -> precheck::Settings {
    precheck::Settings::decode(format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}/peers/precheck.sock","expected_policy_epoch":{epoch},"disclosure_id":81,"query_limit":8,"exchange_limit":8,"candidate_limit":8,"connection_limit":8,"runtime_ms":60000}}"#,
        root.0.display()).as_bytes()).unwrap()
}

fn intent(request: u64, release: LearnedTextRelease, epoch: u64) -> ActorProposal {
    let mut proposal = FileLearnedTextStreamActorPort::encode_release(request, release,
        ElapsedTick(50_000)).unwrap();
    proposal.expected_policy_epoch = epoch;
    proposal
}

fn query(profile: &Profile, socket: &Path, proposal: ActorProposal, epoch: u64)
    -> PrecheckWireResponse
{
    let mut client = PrecheckPeerClient::connect(UnixStream::connect(socket).unwrap(),
        profile.supervisor, PrecheckRequest { query: 9, proposal }, epoch).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(25), "learned precheck response timed out");
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() { return response; }
        assert!(!report.status.closed(), "precheck closed without its native response");
        thread::sleep(Duration::from_millis(1));
    }
}

fn known(response: PrecheckWireResponse, expected: FilePrecheckOutcome, epoch: u64) {
    assert_eq!(response.query, Some(9));
    let Ok(Knowledge::Known { value, basis }) = response.result else {
        panic!("expected original learned precheck result: {response:?}");
    };
    assert_eq!(value, expected);
    assert_eq!((basis.query, basis.disclosure, basis.policy_epoch), (9, 81, epoch));
    assert_eq!(basis.observed_at, ElapsedTick(1000));
}

fn disclose(driver: &FileSupervisedDriver, request: u64, release: LearnedTextRelease)
    -> (FileActorPrecheckPort, FilePrecheckDisclosure)
{
    let revision = driver.supervisor().host().unwrap().revision();
    driver.supervisor().disclose_learned_text_stream_precheck(revision, 81, 16,
        request, release, ElapsedTick(50_000)).unwrap()
}

fn generated(root: &Root, maximum: usize)
    -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire)
{
    let mut config = configured(root);
    config.profile.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(maximum),
        Predicate::UnitsAtMost(maximum as u64), Predicate::All(vec![0, 1])]).unwrap();
    let path = super::continuation::write(root);
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let wire = ActorWire::new(port);
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline(), &mut || ElapsedTick(1000)).unwrap());
    driver.prepare_file_intake(&mut config.source, || ElapsedTick(1000)).result.unwrap();
    (config, loaded, reviewer, driver, wire)
}

#[test]
fn learned_precheck_evaluates_actual_frame_and_units_without_exposing_raw_proposal_fallback() {
    for maximum in [FileLearnedTextStreamActorPort::INTENT_BYTES, 512] {
        let root = Root::new();
        let (mut config, loaded, _, mut driver, mut wire) = generated(&root, maximum);
        let before = fs::read(config.store.join("delivery.bin")).unwrap();
        let reads = config.source.status().read_attempts;
        let revision = driver.supervisor().host().unwrap().revision();
        assert!(driver.supervisor().disclose_exact_precheck(revision, 82, 4).is_err(),
            "specialized learned owners must still refuse raw-proposal disclosure");
        let (port, _grant) = disclose(&driver, 1, LearnedTextRelease::Message);
        let proposed = intent(1, LearnedTextRelease::Message, 0);
        assert_eq!(proposed.payload.len(), FileLearnedTextStreamActorPort::INTENT_BYTES);
        let outcome = if maximum == 512 { FilePrecheckOutcome::PrecheckPassed }
            else { FilePrecheckOutcome::DeniedByPolicy };
        assert!(matches!(port.check(9, &proposed).unwrap(),
            Knowledge::Known { value, .. } if value == outcome));
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
        assert_eq!(config.source.status().read_attempts, reads);
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        assert!(matches!(driver.supervisor().host().unwrap().request_status(1),
            Err(JournalError::Contract(Error::Missing))));
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline(),
            &mut || ElapsedTick(1000)).unwrap();
        let status = driver.supervisor().host().unwrap().request_status(1).unwrap();
        assert_eq!(matches!(status.disposition,
            FileRequestDisposition::Admitted { stage: ActionState::Reviewing, .. }), maximum == 512);
        assert!(!matches!(port.check(10, &proposed).unwrap(), Knowledge::Known { .. }),
            "actual source-aware admission must invalidate the original grant");
        assert!(driver.supervisor().disclose_learned_text_stream_precheck(
            driver.supervisor().host().unwrap().revision(), 83, 1, 1,
            LearnedTextRelease::Message, ElapsedTick(50_000)).is_err(),
            "a recorded request cannot issue another learned disclosure");
        if maximum == 512 { cancel_unspent(&mut driver, 1).unwrap(); }
        assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

#[test]
fn learned_precheck_binds_canonical_request_kind_epoch_and_original_deadline() {
    let root = Root::new(); let (config, _, _, driver, _) = generated(&root, 512);
    let (port, grant) = disclose(&driver, 1, LearnedTextRelease::Message);
    let original = intent(1, LearnedTextRelease::Message, 0);
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    for changed in 0..8 {
        let mut candidate = original.clone();
        match changed {
            0 => candidate = intent(2, LearnedTextRelease::Message, 0),
            1 => candidate = intent(1, LearnedTextRelease::Finish, 0),
            2 => candidate.target.object += 1,
            3 => candidate.units += 1,
            4 => { candidate.payload.push(0); candidate.units += 1; }
            5 => candidate.deadline = ElapsedTick(50_001),
            6 => candidate.deadline = ElapsedTick(0),
            _ => candidate.payload[0] ^= 1,
        }
        assert_eq!(port.check(10 + changed, &candidate), Err(ActorError::MalformedProposal));
    }
    let mut stale = original.clone(); stale.expected_policy_epoch = 1;
    assert!(matches!(port.check(30, &stale).unwrap(), Knowledge::Stale { generation: 1, current: 0 }));
    assert!(matches!(port.check(31, &original).unwrap(),
        Knowledge::Known { value: FilePrecheckOutcome::PrecheckPassed, .. }));
    assert_eq!(grant.remaining_queries(), 6);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    grant.revoke();
    assert!(matches!(port.check(32, &original).unwrap(), Knowledge::Withheld { .. }));
    assert_eq!(cleanup(driver, 1000, 1), 0);
}

#[derive(Clone, Copy)]
enum PeerAction { Submit, ChangeSource, Truncate }

fn run_network(root: &Root, path: &Path, selected: PeerAction, continuing: bool,
    finishing: Option<u64>, after_message: Option<u64>, epoch: u64)
    -> (RunResult, Option<ReviewPacket>, FileDeliverySnapshot)
{
    let config = configured(root); let loaded = recipe::load(path, &config, true).unwrap();
    assert!(config.programs.is_empty(), "actual native helpers are required");
    let generation = loaded.generation.clone(); let host_profile = config.profile.clone();
    let request = loaded.request; let store = config.store.clone();
    let source_path = root.0.join("evidence.json");
    let denied = EvidenceSnapshot::new(EvidenceIdentity { scope: config.profile.delivery.scope,
        source: 51, generation: 2 }, fa_reference::Snapshot { semantic_epoch: 2, complete: true,
        values: BTreeMap::from([(7, b"blocked".to_vec())]) },
        BTreeMap::from([("alpha".to_owned(), Vec::new()), ("beta".to_owned(), Vec::new())])).unwrap().encode();
    let peers = fixture::peers(root, &config); let actor_profile = actor_profile(root, &config, request);
    let settings = settings(root, epoch); let socket = root.0.join("peers/precheck.sock");
    let mut peer = None; let mut reviewer = None;
    let release = if finishing.is_some() { LearnedTextRelease::Finish } else { LearnedTextRelease::Message };
    let result = run_with_actor_precheck(config, loaded, &peers, continuing, finishing,
        after_message, None, Some(&actor_profile), Some(&settings), || {
            if peer.is_none() && socket.exists() {
                let profile = actor_profile.clone(); let socket = socket.clone();
                let journal = store.join("delivery.bin"); let review_socket = peers.socket(request);
                let source = source_path.clone(); let denied = denied.clone();
                peer = Some(thread::spawn(move || {
                    if matches!(selected, PeerAction::Truncate) {
                        let mut broken = UnixStream::connect(&socket).unwrap();
                        broken.write_all(b"{").unwrap();
                        broken.shutdown(std::net::Shutdown::Write).unwrap();
                        let started = Instant::now();
                        while socket.exists() {
                            assert!(started.elapsed() < Duration::from_secs(25), "truncated sidecar did not retire");
                            thread::sleep(Duration::from_millis(1));
                        }
                    } else {
                        let before = fs::read(&journal).unwrap();
                        known(query(&profile, &socket, intent(request, release, epoch), epoch),
                            FilePrecheckOutcome::PrecheckPassed, epoch);
                        assert_eq!(fs::read(&journal).unwrap(), before, "a query changed the original owner");
                        assert!(!review_socket.exists(), "precheck started human review");
                        if matches!(selected, PeerAction::ChangeSource) { fs::write(source, denied).unwrap(); }
                    }
                    actor(profile, release, ElapsedTick(50_000), Behavior::Observe).join().unwrap()
                }));
            }
            if reviewer.is_none() && peers.socket(request).exists() {
                assert!(!socket.exists(), "disclosure survived into independent human review");
                reviewer = Some(human(peers.clone(), request, ReviewDecision::Approve));
            }
            ElapsedTick(1000)
        }).unwrap();
    let observed = peer.expect("actual learned precheck endpoint bound").join().unwrap();
    assert_eq!(result.response.result, observed.result);
    let packet = reviewer.map(|thread| thread.join().unwrap());
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert!(!socket.exists()); assert!(!actor_profile.socket.exists());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    let state = if let Some(prediction) = generation.required_owned_pre_output_forecast() {
        FileOversight::read_publication_with_owned_learned_consistency(&store,
            &host_profile, &generation, prediction).unwrap().publication
    } else {
        FileOversight::read_publication_with_learned_generation(&store, &host_profile, &generation).unwrap()
    };
    (result, packet, state)
}

#[test]
fn authenticated_precheck_then_learned_release_runs_native_helpers_and_human_in_all_predictor_modes() {
    for prediction in [None, Some(false), Some(true)] {
        let root = Root::new(); let config = configured(&root);
        let path = super::continuation::write(&root);
        if let Some(owned) = prediction { super::forecast::install(&root, &path, owned, 60_000); }
        evidence(&root, &config, 1, true, false);
        let (result, packet, state) = run_network(&root, &path, PeerAction::Submit, false, None, None, 0);
        assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let packet = packet.unwrap(); assert_eq!(packet.views().len(), 2);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        assert_eq!((state.executions, state.payload.as_slice()), (1, b"A".as_slice()));
        assert_eq!(state.control.ledger.charged, packet.action().spec().units);
        assert_eq!(state.control.ledger.reserved, 0); assert!(state.stop.is_none());
    }
}

#[test]
fn prechecked_release_still_reacquires_current_source_and_truncated_precheck_cannot_stop_it() {
    for action in [PeerAction::ChangeSource, PeerAction::Truncate] {
        let root = Root::new(); let config = configured(&root);
        let path = super::continuation::write(&root); evidence(&root, &config, 1, true, false);
        let (result, packet, state) = run_network(&root, &path, action, false, None, None, 0);
        if matches!(action, PeerAction::ChangeSource) {
            assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Denied, .. })));
            assert!(packet.is_none()); assert_eq!(state.executions, 0);
            assert_eq!(state.control.ledger.charged, 0);
        } else {
            assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
            assert!(packet.is_some()); assert_eq!(state.executions, 1);
        }
        assert_eq!(state.control.ledger.reserved, 0); assert!(state.stop.is_none());
    }
}

#[test]
fn continued_message_and_explicit_finish_precheck_use_recovered_epoch_and_actual_cumulative_frame() {
    let root = Root::new(); let config = configured(&root);
    let first = super::continuation::write(&root); evidence(&root, &config, 1, true, false);
    run_network(&root, &first, PeerAction::Submit, false, None, None, 0);
    let (second, _) = super::continuation::next(&root, &config, 2);
    let (_, packet, state) = run_network(&root, &second, PeerAction::Submit, true, None, Some(1), 1);
    let packet = packet.unwrap(); let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A"]); assert_eq!(frame.message(), Some("A"));
    assert_eq!(state.executions, 2); assert_eq!(state.payload, b"AA");
    let (last, _) = super::continuation::next(&root, &config, 3);
    let (_, packet, state) = run_network(&root, &last, PeerAction::Submit, true, Some(2), None, 2);
    let packet = packet.unwrap(); let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
    assert!(frame.is_finish()); assert_eq!(frame.prior_messages(), &["A", "A"]);
    assert_eq!(state.executions, 3); assert_eq!(state.payload, b"AA");
}

#[test]
fn precheck_does_not_hide_prior_confirmed_messages_from_exact_policy() {
    let root = Root::new();
    let first_size = fa_reference::action::consequence::delivery::stream::STREAM_HEADER_BYTES + 1;
    let (mut config, first, reviewer, mut driver, mut wire) = generated(&root, first_size);
    let (port, _grant) = disclose(&driver, 1, LearnedTextRelease::Message);
    assert!(matches!(port.check(1, &intent(1, LearnedTextRelease::Message, 0)).unwrap(),
        Knowledge::Known { value: FilePrecheckOutcome::PrecheckPassed, .. }));
    submit(&mut driver, &mut wire, &mut config, &first, &deadline(), &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = super::continuation::review_current(&mut config, &first, &reviewer, driver, &deadline());
    let approval = super::continuation::approve(&mut driver, &mut config, &reviewer, 1);
    super::continuation::publish(&mut driver, &mut config, &approval);
    assert!(executed(&mut wire, 1));
    let (_, second) = super::continuation::next(&root, &config, 2);
    let command = driver.supervisor().host().unwrap().prepare_learned_text_stream_continuation(1, 2).unwrap();
    let mut control = Control::new(&config, 2, None).unwrap();
    assert!(super::super::continuation::prepare(&mut driver, &reviewer, &mut config, command,
        &mut control, &deadline(), &mut || ElapsedTick(1000)).unwrap());
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline(), &mut || ElapsedTick(1000)).unwrap());
    driver.prepare_file_intake(&mut config.source, || ElapsedTick(1000)).result.unwrap();
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    let (next, _grant) = disclose(&driver, 2, LearnedTextRelease::Message);
    assert!(matches!(next.check(2, &intent(2, LearnedTextRelease::Message, 0)).unwrap(),
        Knowledge::Known { value: FilePrecheckOutcome::DeniedByPolicy, .. }));
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    submit(&mut driver, &mut wire, &mut config, &second, &deadline(), &mut || ElapsedTick(1000)).unwrap();
    assert!(matches!(response(&mut wire, 2).result, Ok(Knowledge::Known { value: ActorOutcome::Denied, .. })));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 1);
    drop(history); assert_eq!(cleanup(driver, 1000, 1), 0);
}

#[test]
fn stop_after_precheck_retires_disclosure_without_admitting_or_publishing() {
    let root = Root::new(); let config = configured(&root);
    let path = super::continuation::write(&root); evidence(&root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap(); let generation = loaded.generation.clone();
    let store = config.store.clone(); let host_profile = config.profile.clone();
    let peers = fixture::peers(&root, &config); let profile = actor_profile(&root, &config, 1);
    let settings = settings(&root, 0); let socket = root.0.join("peers/precheck.sock");
    let queried = Arc::new(AtomicBool::new(false)); let mut peer = None;
    let result = run_with_actor_precheck(config, loaded, &peers, false, None, None,
        None, Some(&profile), Some(&settings), || {
            if peer.is_none() && socket.exists() {
                let actor = profile.clone(); let socket = socket.clone(); let peers = peers.clone();
                let queried = Arc::clone(&queried);
                peer = Some(thread::spawn(move || {
                    known(query(&actor, &socket, intent(1, LearnedTextRelease::Message, 0), 0),
                        FilePrecheckOutcome::PrecheckPassed, 0);
                    queried.store(true, Ordering::Release);
                    let mut stop = peers.connect_stop_client(1).unwrap(); let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(25));
                        match stop.step().unwrap() {
                            StopClientProgress::NeedsDecision => stop.request_stop().unwrap(),
                            StopClientProgress::Complete => return stop.receipt().unwrap(),
                            _ => thread::sleep(Duration::from_millis(1)),
                        }
                    }
                }));
            }
            ElapsedTick(1000)
        }).unwrap();
    peer.unwrap().join().unwrap(); assert!(queried.load(Ordering::Acquire));
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert!(!socket.exists()); assert!(!profile.socket.exists()); assert!(!peers.socket(1).exists());
    let state = FileOversight::read_publication_with_learned_generation(&store, &host_profile, &generation).unwrap();
    assert_eq!(state.executions, 0); assert!(state.control.ledger.stages.is_empty());
    assert_eq!((state.control.ledger.charged, state.control.ledger.reserved), (0, 0));
    assert!(state.stop.is_some());
}

#[test]
fn learned_precheck_grammar_and_public_profile_refuse_before_time_or_owner_creation() {
    let base: Vec<String> = ["create-learned-generated", "/config", "/recipe", "/reviewer",
        "--actor-profile", "/actor", "--precheck-profile", "/precheck"].into_iter().map(str::to_owned).collect();
    assert_eq!(precheck::take_option(&base).unwrap().1, Some(Path::new("/precheck")));
    for extra in [vec!["--continue-generation"], vec!["--after-message", "1"],
        vec!["--finish-after-message", "1"], vec!["--history-anchor", "/anchor"]] {
        let mut args = base.clone(); args.extend(extra.into_iter().map(str::to_owned));
        assert!(precheck::take_option(&args).is_ok());
    }
    for extra in [vec!["--precheck-profile", "/again"], vec!["--resume"],
        vec!["--recover-stop", "1"], vec!["--audit-review", "1", "/anchor"],
        vec!["--receipts-only"], vec!["--policy-replay", "/candidate"],
        vec!["--capture-stream-checkpoint", "1"], vec!["--unknown", "/value"]] {
        let mut args = base.clone(); args.extend(extra.into_iter().map(str::to_owned));
        assert!(precheck::take_option(&args).is_err());
        assert!(command_with_clock(&args, None, || panic!("invalid mode cannot reach the clock")).is_err());
    }
    let mut actorless = base.clone(); actorless.drain(4..6);
    assert!(precheck::take_option(&actorless).is_err());
    for actorless in [false, true] {
        let root = Root::new(); let config = configured(&root);
        let path = fixture::write(&root, b"allow"); let loaded = recipe::load(&path, &config, true).unwrap();
        let peers = fixture::peers(&root, &config); let actor = actor_profile(&root, &config, 1);
        let settings = settings(&root, 0); let store = config.store.clone();
        let socket = root.0.join("peers/precheck.sock"); fs::write(&socket, b"unrelated path").unwrap();
        assert!(run_with_actor_precheck(config, loaded, &peers, false, None, None, None,
            if actorless { None } else { Some(&actor) }, Some(&settings),
            || panic!("invalid disclosure profile cannot reach time")).is_err());
        assert!(!store.exists()); assert_eq!(fs::read(socket).unwrap(), b"unrelated path");
    }
}

#[test]
fn stale_disclosure_epoch_and_unsupported_cache_profile_refuse_before_source_or_clock() {
    let root = Root::new();
    let (mut config, _, _, mut driver, _) = generated(&root, 512);
    let peers = fixture::peers(&root, &config); let actor = actor_profile(&root, &config, 1);
    let wrong = settings(&root, 1);
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    let reads = config.source.status().read_attempts;
    let error = precheck::Service::start_learned(&wrong, &actor, &peers, &mut config,
        &mut driver, &deadline(), &mut || panic!("wrong epoch precedes time and source"),
        LearnedTextRelease::Message).err().unwrap();
    assert!(error.contains("policy epoch"), "{error}");
    let cached = precheck::Settings::decode(format!(r#"{{"schema":"fa.actor-precheck-service/2","policy_disclosure":"exact_predicates","socket":"{}/peers/precheck.sock","expected_policy_epoch":0,"disclosure_id":81,"query_limit":8,"exchange_limit":8,"candidate_limit":8,"connection_limit":8,"runtime_ms":60000,"cache_entries":8,"cache_bytes":65536}}"#,
        root.0.display()).as_bytes()).unwrap();
    let error = precheck::Service::start_learned(&cached, &actor, &peers, &mut config,
        &mut driver, &deadline(), &mut || panic!("unsupported profile precedes time and source"),
        LearnedTextRelease::Message).err().unwrap();
    assert!(error.contains("v1"), "{error}");
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    assert_eq!(config.source.status().read_attempts, reads);
    assert!(!root.0.join("peers/precheck.sock").exists());
    assert_eq!(cleanup(driver, 1000, 1), 0);
}
