use super::*;
pub(super) mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{ReviewDecision, ReviewPacket};
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::oversight::policy_state::StateFreshness;
use std::fs;
use std::io;
use std::thread;

fn review(peers: PeerProfile, decision: ReviewDecision) -> thread::JoinHandle<ReviewPacket> {
    thread::spawn(move || {
        let started = Instant::now();
        while !peers.socket(1).exists() {
            assert!(started.elapsed() < Duration::from_secs(10), "no native human offer");
            thread::sleep(Duration::from_millis(1));
        }
        let mut client = peers.connect_client(1).unwrap(); let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    packet = Some(client.packet().unwrap().clone()); client.respond(decision).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn executed(result: &RunResult) -> bool {
    matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}

#[test]
fn generated_publication_computes_original_text_then_requires_independent_human_approval() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new(); let config = configured(&root); let peers = peers(&root, &config);
        let source = write_recipe(&root); let loaded = recipe::load(&source, &config).unwrap();
        let decoder = loaded.decoder.clone(); let tokenizer = loaded.tokenizer.clone();
        let store = config.store.clone(); let profile = config.profile.clone();
        let human = review(peers.clone(), decision);
        let result = create(config, loaded, &peers, || ElapsedTick(1000)).unwrap();
        let packet = human.join().unwrap();
        assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        assert_eq!(packet.views().len(), 2);
        assert_eq!(executed(&result), decision == ReviewDecision::Approve);
        let disk = FileOversight::read_publication(&store, &profile).unwrap();
        let native = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
        assert_eq!(native.text.bytes().unwrap(), b"A");
        assert_eq!(native.text.finish(), Some(Ok(GenerationFinish::StopToken)));
        assert_eq!(native.text.generation_revision(), 4);
        assert_eq!(native.text.numerical().receipt().unwrap().result().unwrap().work().attempted_samples, 2);
        assert_eq!(disk.control.ledger.reserved, 0);
        if decision == ReviewDecision::Approve {
            assert_eq!(disk.executions, 1);
            assert_eq!(ReleaseFrame::decode(&disk.payload).unwrap().message(), Some("A"));
            assert_eq!(disk.control.ledger.charged, packet.action().spec().payload.len() as u64);
        } else {
            assert_eq!(disk.executions, 0); assert!(disk.payload.is_empty()); assert_eq!(disk.control.ledger.charged, 0);
        }
    }
}

#[test]
fn generated_publication_hold_and_token_limit_never_reach_helpers_or_disclose_partial_text() {
    for held in [true, false] {
        let root = Root::new(); let mut config = configured(&root); let peers = peers(&root, &config);
        // No runnable helpers: any accidental launch must fail, not bless output.
        config.programs.clear();
        let mut loaded = loaded(held); if !held { loaded.text.max_new_tokens = 1; }
        let decoder = loaded.decoder.clone(); let tokenizer = loaded.tokenizer.clone();
        let store = config.store.clone(); let profile = config.profile.clone();
        let result = create(config, loaded, &peers, || ElapsedTick(1000)).unwrap();
        assert!(result.failure.as_deref().unwrap().contains("native generation did not complete"));
        assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
        let mut output = Vec::new(); assert!(emit(result, &mut output).is_err());
        assert!(!output.windows(b"payload".len()).any(|part| part == b"payload"));
        let disk = FileOversight::read_publication(&store, &profile).unwrap();
        assert_eq!(disk.executions, 0); assert!(disk.payload.is_empty());
        let native = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
        assert_eq!(native.text.finish(), Some(Ok(if held { GenerationFinish::Held } else { GenerationFinish::TokenLimit })));
        if !held { assert_eq!(native.text.bytes().unwrap(), b"A"); }
        assert!(!peers.socket(1).exists());
    }
}

#[test]
fn generated_publication_missing_source_does_not_begin_native_work() {
    let root = Root::new(); let mut config = configured(&root); let peers = peers(&root, &config);
    config.programs.clear(); fs::remove_file(root.0.join("evidence.json")).unwrap();
    let loaded = loaded(false); let decoder = loaded.decoder.clone(); let tokenizer = loaded.tokenizer.clone();
    let store = config.store.clone(); let profile = config.profile.clone();
    let result = create(config, loaded, &peers, || ElapsedTick(1000)).unwrap();
    assert!(result.failure.is_some()); assert!(!executed(&result));
    let disk = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(disk.executions, 0); assert!(disk.stop.is_some());
    assert!(FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).is_err());
}

#[test]
fn generated_publication_post_read_expiry_prevents_the_first_token() {
    for expired in [false, true] {
        let root = Root::new(); let mut config = configured(&root); let peers = peers(&root, &config);
        config.source_policy.freshness = StateFreshness::new(10).unwrap();
        let human = (!expired).then(|| review(peers.clone(), ReviewDecision::Approve));
        if expired { config.programs.clear(); }
        let loaded = loaded(false); let decoder = loaded.decoder.clone(); let tokenizer = loaded.tokenizer.clone();
        let store = config.store.clone(); let profile = config.profile.clone();
        let mut calls = 0;
        let result = create(config, loaded, &peers, || {
            calls += 1; ElapsedTick(if calls >= 6 { if expired { 1010 } else { 1009 } } else { 1000 })
        }).unwrap();
        if let Some(human) = human { human.join().unwrap(); }
        assert_eq!(executed(&result), !expired);
        let native = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
        if expired {
            assert!(result.failure.is_some());
            assert_eq!(native.numerical.numerical.position, 0);
            assert!(native.text.bytes().unwrap().is_empty());
            assert_eq!(native.publication.executions, 0);
        } else {
            assert!(result.failure.is_none());
            assert_eq!(native.numerical.numerical.position, 4);
            assert_eq!(native.publication.executions, 1);
        }
    }
}

#[test]
fn generated_publication_refuses_content_stop_and_existing_store_before_any_publication() {
    let root = Root::new(); let config = configured(&root); let peers = peers(&root, &config);
    let mut invalid = loaded(false); invalid.text.stop_tokens = vec![65];
    assert!(create(config, invalid, &peers, || panic!("invalid control sampled time")).is_err());
    assert!(!root.0.join("store").exists());
    fs::create_dir(root.0.join("store")).unwrap();
    fs::write(root.0.join("store/retained"), b"unrelated bytes").unwrap();
    let config = configured(&root);
    assert!(create(config, loaded(false), &peers, || ElapsedTick(1000)).is_err());
    assert_eq!(fs::read(root.0.join("store/retained")).unwrap(), b"unrelated bytes");
}

#[test]
fn generated_publication_output_failure_cannot_resend_committed_effect() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { panic!("failed write must not flush") }
    }
    let root = Root::new(); let config = configured(&root); let peers = peers(&root, &config);
    let store = config.store.clone(); let profile = config.profile.clone(); let human = review(peers.clone(), ReviewDecision::Approve);
    let result = create(config, loaded(false), &peers, || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(executed(&result));
    let before = fs::read(store.join("delivery.bin")).unwrap();
    assert!(emit(result, &mut Broken).unwrap_err().contains("no effect retry"));
    assert_eq!(fs::read(store.join("delivery.bin")).unwrap(), before);
    assert_eq!(FileOversight::read_publication(&store, &profile).unwrap().executions, 1);
}

#[test]
fn generated_publication_independent_stop_preempts_native_tokens_and_helper_launch() {
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
        VerifiedReviewerSocket, PeerPolicy, PeerCredentials,
    };
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::StopClientProgress;
    use std::os::unix::net::UnixStream;
    let root = Root::new(); let mut config = configured(&root); let peers = peers(&root, &config);
    config.programs.clear();
    let store = config.store.clone(); let profile = config.profile.clone();
    let endpoint = super::super::super::control::socket_path(&peers, 1);
    let endpoint_for_client = endpoint.clone(); let client_profile = peers.clone();
    let (sent, received) = std::sync::mpsc::channel();
    let controller = thread::spawn(move || {
        let start = Instant::now();
        while !endpoint_for_client.exists() {
            assert!(start.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(1));
        }
        let (probe, _other) = UnixStream::pair().unwrap();
        let observed = PeerCredentials::observe(&probe).unwrap();
        let policy = PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap();
        let socket = VerifiedReviewerSocket::verify(UnixStream::connect(endpoint_for_client).unwrap(), policy).unwrap();
        sent.send(()).unwrap();
        let mut client = socket.into_stop_client(client_profile.expected, 1).unwrap();
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    });
    let mut synchronized = false;
    let result = create(config, loaded(false), &peers, || {
        if !synchronized && endpoint.exists() {
            received.recv_timeout(Duration::from_secs(10)).unwrap(); synchronized = true;
        }
        ElapsedTick(1000)
    }).unwrap();
    let stopped = controller.join().unwrap();
    assert!(stopped.acknowledged()); assert!(stopped.drained());
    assert!(result.failure.is_none(), "{:?}", result.failure); assert!(!executed(&result));
    let disk = FileOversight::read_publication(&store, &profile).unwrap();
    assert!(disk.stop.is_some()); assert_eq!(disk.executions, 0); assert!(disk.payload.is_empty());
    assert_eq!(disk.control.ledger.charged, 0); assert_eq!(disk.control.ledger.reserved, 0);
    assert!(!peers.socket(1).exists());
}

#[test]
fn generated_publication_command_is_routed_without_a_legacy_or_actor_mode_fallback() {
    assert_eq!(crate::command(vec!["create-generated".into()]).unwrap_err(), USAGE);
    assert!(command(&["create-generated", "config", "recipe", "reviewer", "--peers"]
        .map(str::to_owned), None).is_err());
    assert!(command(&["create-generated", "config", "recipe", "reviewer"]
        .map(str::to_owned), Some(Path::new("qualification"))).is_err());
}
