//! Original stop and checked-publication boundaries around qualified live intake.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
    PeerPolicy, VerifiedReviewerSocket,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
    StopClientProgress, StopControlReceipt,
};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::sync::{Arc, mpsc};

fn stop_client(profile: &PeerProfile, operation: u64) -> StopControlReceipt {
    let path = crate::workflow::control::socket_path(profile, operation);
    wait(&path);
    let (local, _) = UnixStream::pair().unwrap();
    let id = PeerCredentials::observe(&local).unwrap();
    let socket = VerifiedReviewerSocket::verify(UnixStream::connect(path).unwrap(),
        PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap()).unwrap();
    let mut client = socket.into_stop_client(profile.expected, operation).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        match client.step().unwrap() {
            StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
            StopClientProgress::Complete => return client.receipt().unwrap(),
            _ => pause(1),
        }
    }
}

#[test]
fn independent_stop_preempts_absent_or_partial_actor_without_current_evidence() {
    for fragmented in [false, true] {
        let root = Directory::new(); seed(&root, None);
        let (mut config, actor_profile, peer, _, capsule, qualification) = prepared(&root, 7, 100, 2);
        let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        config.programs.clear();
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        let polls = Arc::new(AtomicU64::new(0));
        let peer_polls = Arc::clone(&polls);
        let actor_policy = actor_profile.clone();
        let stop_profile = peer.clone();
        let controller = thread::spawn(move || {
            wait(&crate::workflow::control::socket_path(&stop_profile, 7));
            let stalled = if fragmented {
                let mut socket = UnixStream::connect(&actor_policy.socket).unwrap();
                actor_policy.supervisor.verify(&socket).unwrap();
                // An incomplete frame header cannot reach the actor command decoder.
                socket.write_all(&[0]).unwrap();
                peer_polls.store(1, Ordering::Release);
                let started = Instant::now();
                while peer_polls.load(Ordering::Acquire) < 4 {
                    assert!(started.elapsed() < Duration::from_secs(5));
                    pause(1);
                }
                Some(socket)
            } else { None };
            let receipt = stop_client(&stop_profile, 7);
            drop(stalled);
            receipt
        });
        serve_with_credibility(config, &actor_profile, &peer, None, true, Some(&capsule), || {
            if polls.load(Ordering::Acquire) != 0 { polls.fetch_add(1, Ordering::AcqRel); }
            ElapsedTick(1001)
        }).unwrap();
        let receipt = controller.join().unwrap();
        assert!(receipt.acknowledged());
        assert!(receipt.drained());
        let config = configured(&root, 0);
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(after.stop.as_ref().unwrap().request().operation, 7);
        assert!(after.control.suspended);
        assert_eq!(after.control.ledger.stages, before.control.ledger.stages);
        assert_eq!(after.executions, 1);
        assert_eq!(after.payload, b"seed");
        assert_eq!(after.control.ledger.charged, 4);
        assert_eq!(after.control.ledger.reserved, 0);
        assert!(after.control.ledger.epoch > qualification.expected_epoch + 1);
        assert!(!actor_profile.socket.exists());
        assert!(!peer.socket(7).exists());
        assert!(!crate::workflow::control::socket_path(&peer, 7).exists());
    }
}

#[test]
fn same_stop_listener_survives_intake_and_interrupts_the_wait_for_human_review() {
    let root = Directory::new(); seed(&root, None);
    let (config, actor_profile, peer, document, capsule, _) = prepared(&root, 7, 100, 2);
    let stop_path = crate::workflow::control::socket_path(&peer, 7);
    let actor_policy = actor_profile.clone();
    let early_path = stop_path.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    let submitted = thread::spawn(move || {
        wait(&early_path);
        let metadata = fs::symlink_metadata(&early_path).unwrap();
        sender.send((metadata.dev(), metadata.ino())).unwrap();
        let mut out = Vec::new();
        let result = client::submit(&actor_policy, &document, &mut out);
        let response = decode_response(out.strip_suffix(b"\n").expect("original stop result")).unwrap();
        (result, response)
    });
    let stop_profile = peer.clone();
    let controller = thread::spawn(move || {
        let original = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        // No approving human connects. The original stop role remains separate.
        wait(&stop_profile.socket(7));
        let current = fs::symlink_metadata(&stop_path).unwrap();
        assert_eq!((current.dev(), current.ino()), original);
        stop_client(&stop_profile, 7)
    });
    serve_with_credibility(config, &actor_profile, &peer, None, true,
        Some(&capsule), || ElapsedTick(1001)).unwrap();
    let receipt = controller.join().unwrap();
    assert!(receipt.acknowledged()); assert!(receipt.drained());
    assert!(!executed(&submitted.join().unwrap().1));
    let config = configured(&root, 0);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert!(after.stop.is_some());
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, 4);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.available, config.profile.delivery.total - 4);
    assert!(!actor_profile.socket.exists());
    assert!(!peer.socket(7).exists());
    assert!(!crate::workflow::control::socket_path(&peer, 7).exists());
}

#[test]
fn recovered_qualification_needs_a_fresh_operation_not_omission_or_historical_retry() {
    let root = Directory::new();
    let (_, old_capsule) = qualified(&root);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    for capsule in [None, Some(old_capsule.as_path())] {
        let mut config = configured(&root, 0); config.programs.clear();
        let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        let (actor_profile, peer) = profiles(&root, &config, 8);
        assert!(serve_with_credibility(config, &actor_profile, &peer, None, true,
            capsule, || ElapsedTick(1002)).is_err());
        let config = configured(&root, 0);
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(after.control.sequence, before.control.sequence);
        assert_eq!(after.control.ledger.stages, before.control.ledger.stages);
        assert_eq!(after.control.ledger.charged, 11);
        assert_eq!(after.executions, 2);
        assert!(!peer.socket(8).exists());
        assert!(!actor_profile.socket.exists());
    }
    let (config, actor_profile, peer, document, capsule, _) = prepared(&root, 8, 101, 3);
    evidence(&root, &config);
    let submitted = actor(actor_profile.clone(), document);
    let human = reviewer(peer.clone(), 8);
    serve_with_credibility(config, &actor_profile, &peer, None, true,
        Some(&capsule), || ElapsedTick(1003)).unwrap();
    human.join().unwrap();
    let (status, response) = submitted.join().unwrap();
    assert!(status.is_ok(), "{status:?}"); assert!(executed(&response));
    let config = configured(&root, 0);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 3);
    assert_eq!(after.control.ledger.charged, 18);
    assert_eq!(after.control.ledger.reserved, 0);
}

#[test]
fn qualified_checked_live_publication_still_seals_late_whole_input_drift() {
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{
        FilePublicationProducer, PublicationProducerProfile,
    };
    use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
    for changed in [false, true] {
        let root = Directory::new(); let config = configured(&root, 0);
        let actual = |epoch| ActualHelperInput::new(b"Q".to_vec(), InputProfileBinding {
            profile_id: 1, profile_bytes: vec![], tokenizer_epoch: 1, policy_epoch: 0, model_epoch: epoch,
        }, vec![SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question }], vec![]).unwrap();
        let producer_profile = PublicationProducerProfile { source: 91, scope: config.profile.delivery.scope,
            feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 };
        let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), producer_profile,
            FilePublicationInputs::new(None, Some(actual(1))), ElapsedTick(1000)).unwrap();
        let json = format!(r#"{{"schema":"fa.supervised-whole-input/1","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,
            root.0.display(), scope(&config));
        let publication = PublicationProfile::decode(json.as_bytes()).unwrap();
        seed(&root, Some(&publication));
        let (config, actor_profile, peer, document, capsule, _) = prepared(&root, 7, 100, 2);
        let store = config.store.clone(); let bootstrap = config.profile.clone();
        let submitted = actor(actor_profile.clone(), document);
        let human = reviewer(peer.clone(), 7);
        let mut advanced = false;
        let result = serve_with_credibility(config, &actor_profile, &peer, Some(&publication), true,
            Some(&capsule), || {
                if !advanced && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                    && disk.control.ledger.charged > 4 && disk.executions == 1 {
                    producer.publish(1, FilePublicationInputs::new(None, Some(actual(if changed { 2 } else { 1 }))),
                        ElapsedTick(1001)).unwrap();
                    advanced = true;
                }
                ElapsedTick(1001)
            });
        if !changed { assert!(result.is_ok(), "{result:?}"); }
        human.join().unwrap();
        let (_, response) = submitted.join().unwrap();
        assert!(advanced);
        assert_eq!(executed(&response), !changed);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, if changed { 1 } else { 2 });
        assert_eq!(disk.control.ledger.charged, if changed { 4 } else { 11 });
        assert_eq!(disk.payload, if changed { b"seed".as_slice() } else { b"network".as_slice() });
        assert_eq!(disk.control.ledger.reserved, 0);
    }
}
