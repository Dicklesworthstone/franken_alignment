//! Finite producer catch-up through the real learned operator. The capture
//! producer is independent of the original native/process helper inputs.
use super::*;
use fa_reference::Snapshot;
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::{
    stream::StreamView,
    publication_gate::changes::{PublicationChange, PublicationInputCut,
        freshness::PublicationHeartbeat},
    persistent::observed::{
        publication::capture::{FileCaptureIdentity, FilePublicationCapture,
            heartbeat::feed::PublicationFeedBatch},
        reviewer::peer::control::{StopClientProgress, StopControlReceipt},
    },
};
use fa_reference::action::consequence::oversight::actor_wire::{WireResponse,
    client::{ActorClientState, ClientProgress, ClientSessionLimits, drive::ClientDriveBudget}};
use fa_reference::witness::refinement::index::routing::WitnessChange;
use std::{collections::BTreeMap, os::unix::net::UnixStream,
    sync::{Arc, atomic::{AtomicBool, Ordering}}, thread};

#[derive(Clone, Copy, Debug)]
enum Review { Native, Process }

fn replace(root: &Root, name: &str, bytes: &[u8]) {
    let pending = root.0.join(format!("{name}.next"));
    fs::write(&pending, bytes).unwrap(); fs::rename(pending, root.0.join(name)).unwrap();
}

fn feed(root: &Root, through: u64) {
    let batch = PublicationFeedBatch::new(PublicationHeartbeat { source: 41,
        clock_domain: CLOCK_DOMAIN, generation: through + 1, through, produced_at: ElapsedTick(1000) },
        0, (1..=through).map(|sequence| PublicationChange { source: 41, sequence,
            change: WitnessChange::All }).collect()).unwrap();
    replace(root, "feed.bin", &batch.to_bytes().unwrap());
}

fn capture(action: &FrozenAction, generation: u64, through: u64, keys: &[u64]) -> Vec<u8> {
    FilePublicationCapture::new_at_cut(1, FileCaptureIdentity { source: 91, generation }, action,
        structured(action.spec().scope, generation, keys), PublicationInputCut { source: 41, through })
        .unwrap().to_bytes().unwrap()
}

fn setup(root: &Root, mode: Review, retries: u64, actor: bool, lag: bool)
    -> (Config, PathBuf, Loaded, FrozenAction)
{
    let config = configured(root);
    let path = match mode {
        Review::Native => native::write(root),
        Review::Process => {
            let path = fixture::write(root, b"allow");
            fs::write(&path, super::super::processes::process_recipe(root, "allow")).unwrap(); path
        },
    };
    let selected = recipe_tests::captures(root, 4)
        .replace("\"max_retries\":3", &format!("\"max_retries\":{retries}"))
        .replace(r#"{"kind":"absent_key","key":1}"#,
            r#"{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1},{"kind":"empty_range","start":6,"end":9}"#);
    fs::write(root.0.join("publication.json"), selected).unwrap();
    fs::write(&path, envelope(root, &fs::read_to_string(&path).unwrap())).unwrap();
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    // The source fixture independently derives the expected first release using
    // the original policy and frame encoders. Its bytes must later equal the
    // action actually offered by the learned owner; it supplies no live key.
    let payload = StreamView::empty(loaded.stream).encode_message("A").unwrap();
    let (mut producer, _) = FileOversight::create(root.0.join("capture-owner"), config.profile.clone()).unwrap();
    producer.observe_time(producer.revision(), ElapsedTick(1000)).unwrap();
    let action = producer.propose(producer.revision(), 1, ActionSpec { version: VERSION,
        scope: config.profile.delivery.scope, target: Some(config.profile.delivery.target),
        units: payload.len() as u64, payload, required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(if actor { 50_000 } else { 61_000 }) },
        Snapshot { semantic_epoch: 1, complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]) }).unwrap();
    drop(producer);
    let original = capture(&action, 1, 0, &[0, 2, 4]);
    replace(root, "original.bin", &original); replace(root, "current.bin", &original);
    feed(root, u64::from(lag));
    (config, path, loaded, action)
}

fn captures(path: &Path) -> usize {
    // Binding writes fields rather than a packet. Only acknowledged subsequent
    // captures carry this original v2 packet header; these fixed payloads and
    // structured values contain no occurrence of it. This avoids replaying the
    // whole numerical journal in every clock callback.
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return 0,
        Err(error) => panic!("capture journal inspection failed: {error}"),
    };
    bytes.windows(8).filter(|window| *window == b"FAPCAP02").count()
}

#[derive(Default)]
struct WaitBoundary { before_human: Option<usize>, observed: bool }
impl WaitBoundary {
    fn reached(&mut self, human_started: bool, canonical: &Path) -> bool {
        if !human_started { return false; }
        let now = captures(canonical);
        let Some(before) = self.before_human else {
            assert!(now > 0, "lag must have been observed during actual helper review");
            self.before_human = Some(now); return false;
        };
        if now > before { self.observed = true; }
        self.observed
    }
}

fn unchanged_generation(root: &Root, path: &Path) {
    let config = configured(root); let loaded = recipe::load(path, &config, false).unwrap();
    let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 4); assert_eq!(numerical.sampled_draws, 2);
}

#[test]
fn learned_wait_keeps_native_and_process_reviews_and_both_keys_across_real_catch_up() {
    for mode in [Review::Native, Review::Process] {
        let root = Root::new();
        let (config, path, loaded, action) = setup(&root, mode, 1, true, true);
        let reader = Reader::new(&config, &loaded); let canonical = config.store.join("delivery.bin");
        let mut boundary = WaitBoundary::default(); let mut caught_up = false;
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
            &mut |human| {
                if !caught_up && boundary.reached(human, &canonical) {
                    let pending = reader.snapshot();
                    assert_eq!(pending.executions, 0); assert_eq!(pending.control.ledger.charged, 0);
                    replace(&root, "current.bin", &capture(&action, 2, 1, &[0, 2, 4, 99]));
                    caught_up = true;
                }
                ElapsedTick(1000)
            });
        assert!(boundary.observed && caught_up);
        assert!(result.failure.is_none(), "{mode:?}: {:?}", result.failure);
        assert!(confirmed(&result)); let packet = packet.unwrap();
        assert_eq!(packet.action(), &action); assert_eq!(packet.views().len(), 2);
        assert!(boundary.before_human.unwrap() > 1,
            "helper acquisitions may not consume the one final catch-up retry");
        if matches!(mode, Review::Process) {
            for name in ["alpha", "beta"] {
                let input = fs::read(root.0.join(format!("{name}.input"))).unwrap();
                assert!(input.windows(8).any(|window| window == b"FASIDE\0\x01"));
            }
        }
        let state = reader.snapshot(); assert_eq!(state.executions, 1); assert_eq!(state.payload, b"A");
        assert_eq!(state.control.ledger.charged, action.spec().units);
        assert_eq!(state.control.ledger.reserved, 0); assert_eq!(state.control.ledger.stages.len(), 1);
        unchanged_generation(&root, &path);
    }
}

#[test]
fn learned_wait_catch_up_cannot_rebase_a_review_over_a_new_empty_range_member() {
    for phantom in [false, true] {
        let root = Root::new();
        let (config, path, loaded, action) = setup(&root, Review::Native, 1, true, true);
        let reader = Reader::new(&config, &loaded); let canonical = config.store.join("delivery.bin");
        let mut boundary = WaitBoundary::default(); let mut caught_up = false;
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
            &mut |human| {
                if !caught_up && boundary.reached(human, &canonical) {
                    let keys: &[u64] = if phantom { &[0, 2, 4, 7] } else { &[0, 2, 4, 99] };
                    replace(&root, "current.bin", &capture(&action, 2, 1, keys)); caught_up = true;
                }
                ElapsedTick(1000)
            });
        assert!(boundary.observed && caught_up); assert_eq!(packet.unwrap().action(), &action);
        assert_eq!(confirmed(&result), !phantom, "{:?}", result.failure);
        let state = reader.snapshot(); assert_eq!(state.executions, u64::from(!phantom));
        assert_eq!(state.payload, if phantom { Vec::new() } else { b"A".to_vec() });
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.charged, if phantom { 0 } else { action.spec().units });
        if phantom { assert!(result.failure.as_ref().unwrap().contains("Stale"), "{:?}", result.failure); }
        else { assert!(result.failure.is_none(), "{:?}", result.failure); }
        unchanged_generation(&root, &path);
    }
}

#[test]
fn learned_wait_retains_one_finite_retry_budget_even_when_the_clock_does_not_advance() {
    for retries in [1, 2] {
        let root = Root::new();
        let (config, path, loaded, action) = setup(&root, Review::Process, retries, true, true);
        let reader = Reader::new(&config, &loaded); let canonical = config.store.join("delivery.bin");
        let mut boundary = WaitBoundary::default();
        let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
            &mut |human| { boundary.reached(human, &canonical); ElapsedTick(1000) });
        assert!(boundary.observed); assert_eq!(packet.unwrap().action(), &action);
        assert!(!confirmed(&result));
        assert!(result.failure.as_ref().unwrap().contains("producer catch-up retry budget exhausted"),
            "{:?}", result.failure);
        assert_eq!(captures(&canonical) - boundary.before_human.unwrap(), retries as usize + 1);
        let state = reader.snapshot(); assert!(state.stop.is_some()); assert_eq!(state.executions, 0);
        assert!(state.payload.is_empty()); assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.charged, 0); unchanged_generation(&root, &path);
    }
}

fn request_stop(profile: &PeerProfile) -> thread::JoinHandle<StopControlReceipt> {
    // Connect synchronously at the acknowledged wait boundary, so the next
    // original LiveControl checkpoint sees this authenticated waiting peer.
    let mut client = profile.connect_stop_client(1).unwrap();
    thread::spawn(move || {
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(25), "waiting Stop did not complete");
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}

#[test]
fn authenticated_stop_is_serviced_after_learned_wait_without_waiting_for_retry_exhaustion() {
    let root = Root::new();
    let (config, path, loaded, action) = setup(&root, Review::Process, 64, true, true);
    let reader = Reader::new(&config, &loaded); let canonical = config.store.join("delivery.bin");
    let peer = fixture::peers(&root, &config);
    let mut boundary = WaitBoundary::default(); let mut stopping = None;
    let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
        &mut |human| {
            if stopping.is_none() && boundary.reached(human, &canonical) {
                stopping = Some(request_stop(&peer));
            }
            ElapsedTick(1000)
        });
    let receipt = stopping.expect("Stop was connected after durable catch-up deferral").join().unwrap();
    assert!(receipt.acknowledged() && receipt.drained()); assert!(boundary.observed);
    assert_eq!(packet.unwrap().action(), &action); assert!(!confirmed(&result));
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert!(captures(&canonical) - boundary.before_human.unwrap() < 64);
    let state = reader.snapshot(); assert!(state.stop.is_some()); assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 0); assert_eq!(state.control.ledger.reserved, 0);
    unchanged_generation(&root, &path);
}

fn cancellable_actor(profile: crate::workflow::actor_service::Profile, ready: Arc<AtomicBool>)
    -> thread::JoinHandle<WireResponse>
{
    thread::spawn(move || {
        let started = Instant::now();
        let socket = UnixStream::connect(&profile.socket).unwrap(); profile.supervisor.verify(&socket).unwrap();
        let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap()
            .connect_unix(socket).unwrap();
        let proposal = FileLearnedTextStreamActorPort::encode_release(profile.request,
            LearnedTextRelease::Message, ElapsedTick(50_000)).unwrap();
        client.submit(profile.request, &proposal).unwrap();
        let mut cancelled = false;
        loop {
            assert!(started.elapsed() < Duration::from_secs(25), "waiting actor cancellation did not complete");
            if let ClientProgress::Response(response) = client.drive(ClientDriveBudget::default()).unwrap().progress {
                if matches!(response.result, Ok(Knowledge::Known { .. })) {
                    assert!(cancelled, "request terminated before the wait-triggered cancellation"); return response;
                }
                assert!(matches!(response.result, Ok(Knowledge::Pending { .. })), "{response:?}");
                if ready.load(Ordering::Acquire) && !cancelled {
                    client.cancel(profile.request).unwrap(); cancelled = true;
                } else { client.poll(profile.request).unwrap(); }
            }
            thread::sleep(Duration::from_millis(1));
        }
    })
}

#[test]
fn actor_cancellation_during_learned_wait_is_local_and_preserves_paid_generation() {
    let root = Root::new();
    let (config, path, loaded, action) = setup(&root, Review::Process, 64, true, true);
    let reader = Reader::new(&config, &loaded); let canonical = config.store.join("delivery.bin");
    let reviewer = fixture::peers(&root, &config); let actor = peers::actor_profile(&root, &config, 1);
    let ready = Arc::new(AtomicBool::new(false)); let mut boundary = WaitBoundary::default();
    let mut submitted = None; let mut approved = None;
    let result = run_with_actor(config, loaded, &reviewer, false, None, None, None, Some(&actor), || {
        if submitted.is_none() && actor.socket.exists() {
            submitted = Some(cancellable_actor(actor.clone(), Arc::clone(&ready)));
        }
        if approved.is_none() && reviewer.socket(1).exists() {
            approved = Some(peers::human(reviewer.clone(), 1, ReviewDecision::Approve));
        }
        if boundary.reached(approved.is_some(), &canonical) { ready.store(true, Ordering::Release); }
        ElapsedTick(1000)
    }).unwrap();
    let response = submitted.unwrap().join().unwrap(); let packet = approved.unwrap().join().unwrap();
    assert!(boundary.observed); assert_eq!(packet.action(), &action);
    assert_eq!(response.result, result.response.result);
    assert!(matches!(response.result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert!(captures(&canonical) - boundary.before_human.unwrap() < 64);
    let state = reader.snapshot(); assert!(state.stop.is_none()); assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 0); assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    unchanged_generation(&root, &path);
}

fn submit_pending(profile: crate::workflow::actor_service::Profile) -> thread::JoinHandle<WireResponse> {
    thread::spawn(move || {
        let started = Instant::now();
        let socket = UnixStream::connect(&profile.socket).unwrap(); profile.supervisor.verify(&socket).unwrap();
        let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap()
            .connect_unix(socket).unwrap();
        let proposal = FileLearnedTextStreamActorPort::encode_release(profile.request,
            LearnedTextRelease::Message, ElapsedTick(50_000)).unwrap();
        client.submit(profile.request, &proposal).unwrap();
        loop {
            assert!(started.elapsed() < Duration::from_secs(25), "original actor admission did not complete");
            if let ClientProgress::Response(response) = client.drive(ClientDriveBudget::default()).unwrap().progress {
                assert_eq!(response.request, Some(profile.request));
                assert!(matches!(response.result, Ok(Knowledge::Pending { .. })), "{response:?}");
                // End this authenticated exchange inside its accepted lifetime.
                // Expiry does not promise a fresh terminal transport window;
                // the authoritative terminal receipt is recovered below.
                return response;
            }
            thread::sleep(Duration::from_millis(1));
        }
    })
}

#[test]
fn learned_wait_does_not_extend_the_accepted_actor_deadline() {
    let root = Root::new();
    let (config, path, loaded, action) = setup(&root, Review::Process, 64, true, true);
    let reader = Reader::new(&config, &loaded); let canonical = config.store.join("delivery.bin");
    let reviewer = fixture::peers(&root, &config); let actor = peers::actor_profile(&root, &config, 1);
    let mut submitted = None; let mut approved = None;
    let mut boundary = WaitBoundary::default(); let mut expire_next = false;
    let result = run_with_actor(config, loaded, &reviewer, false, None, None, None, Some(&actor), || {
        if submitted.is_none() && actor.socket.exists() { submitted = Some(submit_pending(actor.clone())); }
        if approved.is_none() && reviewer.socket(1).exists() {
            approved = Some(peers::human(reviewer.clone(), 1, ReviewDecision::Approve));
        }
        if expire_next { return ElapsedTick(50_000); }
        if boundary.reached(approved.is_some(), &canonical) {
            // This is the clock immediately after persisting the final Deferred
            // capture. Keep its original validation live so Incomplete becomes
            // one acknowledged wait. Expire at the NEXT outer deadline check.
            expire_next = true;
        }
        ElapsedTick(1000)
    }).unwrap();
    let pending = submitted.unwrap().join().unwrap(); let packet = approved.unwrap().join().unwrap();
    assert!(matches!(pending.result, Ok(Knowledge::Pending { .. })));
    assert!(boundary.observed && expire_next); assert_eq!(packet.action(), &action);
    assert_eq!(action.spec().deadline, ElapsedTick(50_000));
    assert_eq!(result.failure.as_deref(), Some("one-shot workflow deadline reached"));
    assert!(matches!(result.response.result,
        Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert_eq!(captures(&canonical) - boundary.before_human.unwrap(), 1);
    assert_eq!(result.cleanup_pending, 0);
    let state = reader.snapshot(); assert!(state.stop.is_some()); assert_eq!(state.executions, 0);
    assert!(state.payload.is_empty());
    assert_eq!(state.control.ledger.charged, 0); assert_eq!(state.control.ledger.reserved, 0);
    let journal = fs::read(&canonical).unwrap();
    for name in ["original.bin", "current.bin", "feed.bin", "evidence.json"] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::remove_dir(root.0.join("peers")).unwrap();
    let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
    assert!(loaded.native.is_none()); let reads = config.source.status().read_attempts;
    let receipt = recovery::readonly::read(&config, &loaded, None, &mut || Duration::ZERO).unwrap();
    assert_eq!(receipt.result, result.response.result);
    assert_eq!(config.source.status().read_attempts, reads); assert_eq!(fs::read(&canonical).unwrap(), journal);
    assert_eq!(reader.snapshot(), state); assert!(!root.0.join("peers").exists());
    unchanged_generation(&root, &path);
}

#[test]
fn learned_wait_never_reopens_a_catch_up_window_after_actual_dispatch() {
    let root = Root::new();
    let (config, path, loaded, action) = setup(&root, Review::Process, 4, false, false);
    let reader = Reader::new(&config, &loaded); let reviewer = fixture::peers(&root, &config);
    let mut human = None; let mut changed = false;
    let result = run(config, loaded, &reviewer, false, || {
        if human.is_none() && reviewer.socket(1).exists() {
            human = Some(peers::human(reviewer.clone(), 1, ReviewDecision::Approve));
        }
        if human.is_some() && !changed {
            let state = reader.snapshot();
            if state.control.ledger.charged != 0 && state.executions == 0 {
                feed(&root, 1); changed = true;
            }
        }
        ElapsedTick(1000)
    }).unwrap();
    assert!(changed); assert_eq!(human.unwrap().join().unwrap().action(), &action);
    assert!(!confirmed(&result)); assert_eq!(result.cleanup_pending, 0);
    assert!(!result.failure.as_ref().unwrap().contains("retry budget exhausted"));
    let state = reader.snapshot(); assert_eq!(state.executions, 0); assert!(state.payload.is_empty());
    assert_eq!(state.control.ledger.charged, action.spec().units);
    assert_eq!(state.control.ledger.reserved, 0); unchanged_generation(&root, &path);
}

#[test]
fn completed_learned_wait_receipts_recover_without_any_live_capture_feed_or_helper() {
    let root = Root::new();
    let (config, path, loaded, action) = setup(&root, Review::Native, 1, true, true);
    let reader = Reader::new(&config, &loaded); let canonical = config.store.join("delivery.bin");
    let mut boundary = WaitBoundary::default(); let mut caught_up = false;
    let (result, packet) = service(&root, config, loaded, None, None, ReviewDecision::Approve,
        &mut |human| {
            if !caught_up && boundary.reached(human, &canonical) {
                replace(&root, "current.bin", &capture(&action, 2, 1, &[0, 2, 4])); caught_up = true;
            }
            ElapsedTick(1000)
        });
    assert!(boundary.observed && caught_up); assert!(confirmed(&result)); assert!(result.failure.is_none());
    assert_eq!(packet.unwrap().action(), &action); let before = reader.snapshot();
    let journal = fs::read(&canonical).unwrap();
    for name in ["original.bin", "current.bin", "feed.bin", "evidence.json", "native-roster.json",
        "helper-model.json", "helper-weights.safetensors", "helper-monitor.json",
        "helper-tokenizer.bbpe", "helper-sampling.json"] { fs::remove_file(root.0.join(name)).unwrap(); }
    fs::remove_dir(root.0.join("peers")).unwrap();
    let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
    assert!(loaded.native.is_none()); let reads = config.source.status().read_attempts;
    let receipt = recovery::readonly::read(&config, &loaded, None, &mut || Duration::ZERO).unwrap();
    assert_eq!(receipt.result, result.response.result);
    assert_eq!(config.source.status().read_attempts, reads); assert_eq!(fs::read(&canonical).unwrap(), journal);
    assert_eq!(reader.snapshot(), before); assert!(!root.0.join("peers").exists());
}
