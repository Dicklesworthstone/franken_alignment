//! Actual actor transport, child helpers, independent reviewer sockets and files.
//! Verdicts are synthetic test data, not evidence of a model's detection quality.
use super::*;
use crate::workflow::actor_service::series::{self, Options};
use fa_reference::action::consequence::oversight::actor_wire::WireError;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
    PeerPolicy as ReviewerPeerPolicy, VerifiedReviewerSocket,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
    StopClientProgress, StopControlReceipt,
};

fn command(c: &Config, request: u64, version: u64, payload: &[u8], units: u64) -> Command {
    let mut target = c.profile.delivery.target;
    target.expected_version = version;
    Command::Submit { request, proposal: ActorProposal { target, payload: payload.to_vec(),
        units, deadline: ElapsedTick(100000), expected_policy_epoch: 0 } }
}
fn human_for(profile: PeerProfile, request: u64) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        wait(&profile.socket(request));
        let mut client = profile.connect_client(request).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(ReviewDecision::Approve).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    })
}
fn finish(stream: UnixStream, command: Command, budget: &mut ClientIoBudget) -> (UnixStream, WireResponse) {
    let request = match &command { Command::Submit { request, .. } => *request, _ => panic!("submit fixture") };
    let (mut stream, mut response) = exchange(stream, command, budget);
    let started = Instant::now();
    while matches!(response.result, Ok(Knowledge::Pending { .. })) {
        assert!(started.elapsed() < Duration::from_secs(10));
        pause(2);
        (stream, response) = exchange(stream, Command::Poll { request }, budget);
    }
    (stream, response)
}
fn executed(response: &WireResponse) -> bool {
    matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn stop_client(profile: &PeerProfile, operation: u64) -> StopControlReceipt {
    let path = crate::workflow::control::socket_path(profile, operation);
    wait(&path);
    let id = credentials();
    let socket = VerifiedReviewerSocket::verify(UnixStream::connect(path).unwrap(),
        ReviewerPeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap()).unwrap();
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
fn run_pair(root: &Directory) {
    let c = configured(root); evidence(root, &c);
    let (mut actor, reviewer) = profiles(root, &c);
    // Both effects and their final retries must use ONE authenticated connection.
    actor.connections = 1; actor.candidates = 1;
    let version = c.profile.delivery.target.expected_version;
    let first = command(&c, 7, version, b"first", 7);
    let second = command(&c, 8, version + 1, b"second", 7);
    let retry = command(&c, 7, version, b"first", 7);
    let conflict = command(&c, 7, version, b"changed", 7);
    let p = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let (stream, one) = finish(connect(&p), first, &mut budget); assert!(executed(&one), "{one:?}");
        let (stream, two) = finish(stream, second, &mut budget); assert!(executed(&two), "{two:?}");
        let (stream, old) = exchange(stream, retry, &mut budget); assert!(executed(&old), "{old:?}");
        let (_, changed) = exchange(stream, conflict, &mut budget);
        assert_eq!(changed.result, Err(WireError::IdempotencyConflict));
    });
    let first_human = human_for(reviewer.clone(), 7);
    let second_human = human_for(reviewer.clone(), 8);
    series::serve(c, &actor, &reviewer, None,
        Options { requests: &[7, 8], open: false, credibility: None }, || ElapsedTick(1000)).unwrap();
    peer.join().unwrap(); first_human.join().unwrap(); second_human.join().unwrap();
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists()); assert!(!reviewer.socket(8).exists());
}

#[test]
fn two_publications_use_one_authority_one_connection_and_fresh_two_key_reviews() {
    let root = Directory::new(); run_pair(&root);
    let c = configured(&root);
    let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 2);
    assert_eq!(disk.payload, b"second");
    assert_eq!(disk.target.expected_version, c.profile.delivery.target.expected_version + 2);
    assert_eq!(disk.control.ledger.charged, 14);
    assert_eq!(disk.control.ledger.available, c.profile.delivery.total - 14);
    assert_eq!(disk.control.ledger.reserved, 0);
    assert_eq!(disk.control.ledger.stages.len(), 2);
    assert_eq!(disk.control.ledger.epoch, 0); // No dispatcher recovery between effects.
    assert!(disk.stop.is_none());
}

#[test]
fn cancellation_during_first_human_wait_does_not_prevent_second_publication() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let (actor, reviewer) = profiles(&root, &c);
    let first = command(&c, 7, 1, b"cancel-me", 7);
    let second = command(&c, 8, 1, b"permitted", 7); // No first effect, so no version increment.
    let wait_for_human = reviewer.socket(7); let p = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let (stream, response) = exchange(connect(&p), first, &mut budget);
        assert!(matches!(response.result, Ok(Knowledge::Pending { .. })));
        wait(&wait_for_human);
        let (stream, response) = exchange(stream, Command::Cancel { request: 7 }, &mut budget);
        assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        let (_, response) = finish(stream, second, &mut budget);
        assert!(executed(&response), "{response:?}");
    });
    let human = human_for(reviewer.clone(), 8);
    series::serve(c, &actor, &reviewer, None,
        Options { requests: &[7, 8], open: false, credibility: None }, || ElapsedTick(1000)).unwrap();
    peer.join().unwrap(); human.join().unwrap();
    let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.payload, b"permitted");
    assert_eq!(disk.control.ledger.charged, 7); assert_eq!(disk.control.ledger.reserved, 0);
    assert!(disk.control.ledger.stages.values().any(|s| *s == ActionState::Cancelled));
    assert!(disk.stop.is_none());
}

#[test]
fn later_requests_cannot_refill_the_original_cumulative_spending_budget() {
    let root = Directory::new(); let mut c = configured(&root); c.profile.delivery.total = 10;
    evidence(&root, &c); let (actor, reviewer) = profiles(&root, &c);
    let first = command(&c, 7, 1, b"first", 7);
    let second = command(&c, 8, 2, b"over-budget", 7);
    let p = actor.clone(); let profile = c.profile.clone(); let path = c.store.clone();
    let peer = std::thread::spawn(move || {
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let (stream, response) = finish(connect(&p), first, &mut budget); assert!(executed(&response));
        let (_, response) = finish(stream, second, &mut budget); assert!(!executed(&response));
    });
    let human = human_for(reviewer.clone(), 7);
    series::serve(c, &actor, &reviewer, None,
        Options { requests: &[7, 8], open: false, credibility: None }, || ElapsedTick(1000)).unwrap();
    peer.join().unwrap(); human.join().unwrap();
    let disk = FileOversight::read_publication(&path, &profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.payload, b"first");
    assert_eq!(disk.control.ledger.charged, 7); assert_eq!(disk.control.ledger.available, 3);
    assert_eq!(disk.control.ledger.reserved, 0); assert!(!reviewer.socket(8).exists());
    let (host, _) = FileOversight::open(&path, profile).unwrap();
    assert!(matches!(host.request_status(8).unwrap().disposition, FileRequestDisposition::NotAdmitted(_)));
}

#[test]
fn complete_historical_schedule_retries_without_source_helpers_humans_or_activation_file() {
    let root = Directory::new(); run_pair(&root);
    let mut c = configured(&root); let (actor, reviewer) = profiles(&root, &c);
    let first = command(&c, 7, 1, b"first", 7); let second = command(&c, 8, 2, b"second", 7);
    let before = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap(); c.programs.clear();
    let missing = root.0.join("must-not-be-read.capsule");
    let p = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let (stream, one) = exchange(connect(&p), first, &mut budget); assert!(executed(&one));
        let (_, two) = exchange(stream, second, &mut budget); assert!(executed(&two));
    });
    // Original actor deadlines have passed. Exact historical retries are not new
    // execution and cannot be forced through current-source qualification.
    series::serve(c, &actor, &reviewer, None,
        Options { requests: &[7, 8], open: true, credibility: Some(&missing) }, || ElapsedTick(200000)).unwrap();
    peer.join().unwrap();
    let c = configured(&root); let after = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(after.executions, before.executions); assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert!(!reviewer.socket(7).exists()); assert!(!reviewer.socket(8).exists());
    assert!(!missing.exists());
}

#[test]
fn all_schedule_and_future_socket_bindings_are_checked_before_creating_the_store() {
    let root = Directory::new();
    for requests in [vec![], vec![8], vec![7, 0], vec![7, 7],
        std::iter::once(7).chain(100..164).collect()] {
        let c = configured(&root); let (actor, reviewer) = profiles(&root, &c);
        assert!(series::serve(c, &actor, &reviewer, None,
            Options { requests: &requests, open: false, credibility: None }, || panic!("invalid schedule observed time")).is_err());
        assert!(!root.0.join("store").exists()); assert!(!actor.socket.exists());
    }
    let c = configured(&root); let (mut actor, reviewer) = profiles(&root, &c);
    actor.socket = reviewer.socket(8);
    assert!(series::serve(c, &actor, &reviewer, None,
        Options { requests: &[7, 8], open: false, credibility: None }, || panic!("socket collision observed time")).is_err());
    assert!(!root.0.join("store").exists()); assert!(!actor.socket.exists());
}

#[test]
fn independent_stop_remains_live_while_waiting_for_the_second_actor_request() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let (actor, reviewer) = profiles(&root, &c); let first = command(&c, 7, 1, b"first", 7);
    let p = actor.clone(); let stop_profile = reviewer.clone();
    let peer = std::thread::spawn(move || {
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let (stream, response) = finish(connect(&p), first, &mut budget); assert!(executed(&response));
        let receipt = stop_client(&stop_profile, 8);
        assert!(receipt.acknowledged()); assert!(receipt.drained());
        drop(stream);
    });
    let human = human_for(reviewer.clone(), 7);
    series::serve(c, &actor, &reviewer, None,
        Options { requests: &[7, 8], open: false, credibility: None }, || ElapsedTick(1000)).unwrap();
    peer.join().unwrap(); human.join().unwrap();
    let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.charged, 7);
    assert_eq!(disk.stop.unwrap().request().operation, 8); assert!(disk.control.suspended);
    assert_eq!(disk.control.ledger.stages.len(), 1); assert_eq!(disk.control.ledger.reserved, 0);
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn second_source_failure_neither_reuses_first_capture_nor_refunds_first_publication() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let (actor, reviewer) = profiles(&root, &c); let first = command(&c, 7, 1, b"first", 7);
    let second = command(&c, 8, 2, b"needs-source", 7);
    let p = actor.clone(); let source = root.0.join("evidence.json"); let stop_profile = reviewer.clone();
    let peer = std::thread::spawn(move || {
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let (stream, response) = finish(connect(&p), first, &mut budget); assert!(executed(&response));
        fs::remove_file(source).unwrap();
        let (stream, response) = exchange(stream, second, &mut budget);
        assert_eq!(response.result, Err(WireError::Unavailable));
        assert!(stop_client(&stop_profile, 8).acknowledged());
        drop(stream);
    });
    let human = human_for(reviewer.clone(), 7);
    series::serve(c, &actor, &reviewer, None,
        Options { requests: &[7, 8], open: false, credibility: None }, || ElapsedTick(1000)).unwrap();
    peer.join().unwrap(); human.join().unwrap();
    let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.payload, b"first");
    assert_eq!(disk.control.ledger.charged, 7); assert_eq!(disk.control.ledger.reserved, 0);
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn checked_second_publication_revalidates_its_own_input_without_refunding_the_first() {
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{
        FilePublicationProducer, PublicationProducerProfile,
    };
    use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
    for changed in [false, true] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let (actor, reviewer) = profiles(&root, &c);
        let actual = |epoch| ActualHelperInput::new(b"Q".to_vec(), InputProfileBinding {
            profile_id: 1, profile_bytes: vec![], tokenizer_epoch: 1, policy_epoch: 0, model_epoch: epoch,
        }, vec![SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question }], vec![]).unwrap();
        let producer_profile = PublicationProducerProfile { source: 91, scope: c.profile.delivery.scope,
            feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 };
        let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), producer_profile,
            FilePublicationInputs::new(None, Some(actual(1))), ElapsedTick(1000)).unwrap();
        let text = format!(r#"{{"schema":"fa.supervised-whole-input/1","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,
            root.0.display(), scope(&c));
        let checked = PublicationProfile::decode(text.as_bytes()).unwrap();
        let first = command(&c, 7, 1, b"first", 7); let second = command(&c, 8, 2, b"second", 7);
        let store = c.store.clone(); let bootstrap = c.profile.clone(); let p = actor.clone();
        let peer = std::thread::spawn(move || {
            let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
            let (stream, response) = finish(connect(&p), first, &mut budget); assert!(executed(&response));
            let (_, response) = finish(stream, second, &mut budget);
            assert_eq!(executed(&response), !changed, "{response:?}");
        });
        let first_human = human_for(reviewer.clone(), 7); let second_human = human_for(reviewer.clone(), 8);
        let mut advanced = false;
        series::serve(c, &actor, &reviewer, Some(&checked),
            Options { requests: &[7, 8], open: false, credibility: None }, || {
                // Observe the real durable dispatch, not a timer or test-local
                // phase flag. The first effect already occurred and stays spent.
                if !advanced && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                    && disk.executions == 1 && disk.control.ledger.charged == 14 {
                    producer.publish(1, FilePublicationInputs::new(None, Some(actual(if changed { 2 } else { 1 }))),
                        ElapsedTick(1000)).unwrap();
                    advanced = true;
                }
                ElapsedTick(1000)
            }).unwrap();
        peer.join().unwrap(); first_human.join().unwrap(); second_human.join().unwrap();
        assert!(advanced);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, if changed { 1 } else { 2 });
        assert_eq!(disk.payload.as_slice(), if changed { b"first".as_slice() } else { b"second".as_slice() });
        assert_eq!(disk.control.ledger.charged, if changed { 7 } else { 14 });
        assert_eq!(disk.control.ledger.reserved, 0);
    }
}
