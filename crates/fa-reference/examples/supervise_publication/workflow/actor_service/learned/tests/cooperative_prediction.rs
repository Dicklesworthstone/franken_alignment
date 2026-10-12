//! Real loaded /5 forecasts and the production quantum entry point. Cold replay
//! must yield before a draw, not merely report that an already sampled token was
//! withheld. Model weights and probabilities are explicit deterministic controls.
use super::*;
use super::continuation as native;
use super::super::prediction;

fn configured_forecast(root: &Root, owned: bool) -> (Config, Loaded) {
    let config = configured(root);
    let path = native::write(root);
    super::forecast::install(root, &path, owned, 60_000);
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    (config, loaded)
}
fn start(config: &Config, loaded: &Loaded)
    -> (FileHumanReviewer, prediction::Session, FileSupervisedDriver, Wire) {
    let (host, reviewer, observer) = recovery::create_selected(config, loaded).unwrap();
    let session = prediction::Session::new(loaded.request, observer.unwrap());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    (reviewer, session, FileSupervisedDriver::new(supervisor), ActorWire::new(port))
}

fn prompt(driver: &mut FileSupervisedDriver, config: &mut Config, deadline: &Deadline) {
    // Run only the original complete two-token prompt. All sampled output is
    // left to the already existing independently required forecast boundary.
    for _ in 0..16 {
        let current = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        if current.status == GenerationStatus::Generating {
            assert_eq!(current.position, 2); assert_eq!(current.sampled_draws, 0);
            return;
        }
        assert!(current.status.is_active());
        refresh(driver, config, deadline, &mut || ElapsedTick(1000)).unwrap();
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.advance_learned_generation(revision, current.actor_revision, current.position).unwrap().unwrap();
    }
    panic!("the original finite fixture prompt did not complete");
}

fn cold_boundary(driver: &mut FileSupervisedDriver, config: &mut Config,
    deadline: &Deadline, prediction: &mut prediction::Session) {
    prompt(driver, config, deadline);
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    prediction.begin_message(&mut host).unwrap();
    let start = host.revision();
    for _ in 0..prediction::MAX_REPLAY_EVENTS_PER_QUANTUM * 4 {
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1000)).unwrap();
    }
    assert_eq!(host.revision() - start, (prediction::MAX_REPLAY_EVENTS_PER_QUANTUM * 4) as u64);
    host.clear_learned_replay_cache();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let forecast = host.action_consistency_deadline().unwrap();
    let bytes = fs::read(config.store.join("delivery.bin")).unwrap(); let revision = host.revision();
    // The SAME non-draining method called by generate_selected, not the
    // test-only one-sample helper. A long cold history cannot fit this quantum.
    prediction.advance_quantum(&mut host).unwrap();
    assert_eq!(host.revision(), revision);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.learned_generation_inspection().unwrap().pending.is_none());
    assert_eq!(host.action_consistency_deadline().unwrap(), forecast);
    assert_eq!(numerical.sampled_draws, 0);
}

#[test]
fn cold_forecast_replay_yields_then_publishes_original_output_with_native_and_human_review() {
    for owned in [false, true] {
        let root = Root::new(); let (mut config, loaded) = configured_forecast(&root, owned);
        let (reviewer, mut prediction, mut driver, mut wire) = start(&config, &loaded);
        let deadline = deadline(); let mut control = Control::new(&config, loaded.request, None).unwrap();
        cold_boundary(&mut driver, &mut config, &deadline, &mut prediction);
        let forecast = driver.supervisor().host().unwrap().action_consistency_deadline().unwrap().unwrap();
        assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000), Some(&mut prediction)).unwrap());
        {
            let host = driver.supervisor().host().unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.position, 4); assert_eq!(numerical.sampled_draws, 2);
            assert_eq!(host.learned_text_message(loaded.evidence).unwrap().bytes(), b"A");
            assert_eq!(host.action_consistency_deadline().unwrap(), Some(forecast));
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert_eq!(host.inspect().executions, 0);
        }
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline,
            &mut || ElapsedTick(1000)).unwrap();
        assert_eq!(driver.supervisor().host().unwrap().action_consistency_snapshot().unwrap().evidence.samples(), 1);
        let (mut driver, history) = native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { .. }));
        let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
        native::publish(&mut driver, &mut config, &human);
        assert!(executed(&mut wire, loaded.request));
        let host = driver.supervisor().host().unwrap();
        let stream = host.stream_snapshot().unwrap();
        assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A"]);
        assert_eq!(stream.confirmed, stream.published);
        assert_eq!(stream.publication.executions, 1);
        assert_eq!(stream.publication.control.ledger.charged,
            host.request_action(loaded.request).unwrap().spec().units);
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
    }
}

#[test]
fn missing_evidence_or_workflow_deadline_between_replay_quanta_prevents_the_first_draw() {
    for owned in [false, true] {
        // Two causal failures and a near-identical successful generation control.
        for failure in 0..3 {
            let root = Root::new(); let (mut config, loaded) = configured_forecast(&root, owned);
            let (reviewer, mut prediction, mut driver, _) = start(&config, &loaded);
            let deadline = deadline(); let mut control = Control::new(&config, loaded.request, None).unwrap();
            cold_boundary(&mut driver, &mut config, &deadline, &mut prediction);
            let host = driver.supervisor().host().unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let forecast = host.action_consistency_deadline().unwrap();
            let likelihood = host.action_consistency_snapshot().unwrap().evidence;
            let owned_work = owned.then(|| host.learned_action_consistency_snapshot().unwrap());
            drop(host);
            if failure == 1 { fs::remove_file(root.0.join("evidence.json")).unwrap(); }
            let generated = generate_selected(&mut driver, &reviewer, &mut config, &mut control,
                &deadline, &mut || ElapsedTick(if failure == 2 { 61_001 } else { 1000 }),
                Some(&mut prediction));
            let host = driver.supervisor().host().unwrap();
            if failure == 0 {
                assert!(generated.unwrap());
                assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
            } else {
                assert!(generated.is_err());
                assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
                assert!(host.learned_generation_inspection().unwrap().pending.is_none());
                if let Some(work) = owned_work {
                    let current = host.learned_action_consistency_snapshot().unwrap();
                    assert_eq!(current.work, work.work);
                    assert_eq!(current.retained_source_bytes, work.retained_source_bytes);
                }
            }
            assert_eq!(host.action_consistency_deadline().unwrap(), forecast);
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence, likelihood);
            assert_eq!(host.retained_requests(), 0); assert_eq!(host.inspect().executions, 0);
            drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
        }
    }
}

#[test]
fn the_operator_yields_after_intent_and_observes_forecast_expiry_before_completion() {
    for owned in [false, true] {
        for expired in [false, true] {
            let root = Root::new(); let (mut config, loaded) = configured_forecast(&root, owned);
            let (reviewer, mut prediction, mut driver, _) = start(&config, &loaded);
            let deadline = deadline(); let mut control = Control::new(&config, loaded.request, None).unwrap();
            cold_boundary(&mut driver, &mut config, &deadline, &mut prediction);
            let mut acknowledged = false;
            for _ in 0..1000 {
                let mut host = driver.supervisor_mut().host_mut().unwrap();
                prediction.advance_quantum(&mut host).unwrap();
                assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 0);
                if host.learned_generation_inspection().unwrap().pending.is_some() {
                    acknowledged = true; break;
                }
            }
            assert!(acknowledged, "cold finite history must reach the separate durable intent");
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let pending = host.learned_generation_inspection().unwrap().pending;
            let forecast = host.action_consistency_deadline().unwrap().unwrap();
            let revision = host.revision();
            if expired { host.observe_time(revision, forecast.expires_at).unwrap(); }
            drop(host);
            let generated = generate_selected(&mut driver, &reviewer, &mut config, &mut control,
                &deadline, &mut || ElapsedTick(1000), Some(&mut prediction));
            let host = driver.supervisor().host().unwrap();
            if expired {
                // The original coverage gate refuses before it can accept a
                // fresh source or the deliberately older clock callback above.
                assert!(generated.is_err());
                assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
                assert_eq!(host.learned_generation_inspection().unwrap().pending, pending);
                assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
            } else {
                assert!(generated.unwrap());
                assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
            }
            assert_eq!(host.action_consistency_deadline().unwrap(), Some(forecast));
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
            assert_eq!(host.inspect().executions, 0);
            drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn an_authenticated_stop_between_replay_quanta_drains_without_sampling_or_publishing() {
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket, control::StopClientProgress,
    };
    use std::os::unix::net::UnixStream;
    for owned in [false, true] {
        let root = Root::new(); let (mut config, loaded) = configured_forecast(&root, owned);
        let peers = fixture::peers(&root, &config);
        // Match production order: claim the actual checked stop endpoint before
        // creating the original durable owner. No socket failure is skipped.
        let mut control = Control::new(&config, loaded.request, Some(&peers)).unwrap();
        let (reviewer, mut prediction, mut driver, _) = start(&config, &loaded);
        let deadline = deadline();
        cold_boundary(&mut driver, &mut config, &deadline, &mut prediction);
        let host = driver.supervisor().host().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let forecast = host.action_consistency_deadline().unwrap();
        let likelihood = host.action_consistency_snapshot().unwrap().evidence;
        let owned_work = owned.then(|| host.learned_action_consistency_snapshot().unwrap());
        drop(host);
        let (socket, _other) = UnixStream::pair().unwrap();
        let credentials = PeerCredentials::observe(&socket).unwrap();
        let path = crate::workflow::control::socket_path(&peers, loaded.request);
        // Connect before the next control checkpoint, avoiding a race in which
        // numerical work could run before the client reached the accept queue.
        let socket = UnixStream::connect(path).unwrap();
        let policy = PeerPolicy::new(credentials.uid(), credentials.gid(), Some(credentials.pid())).unwrap();
        let request = loaded.request;
        let controller = std::thread::spawn(move || {
            let socket = VerifiedReviewerSocket::verify(socket, policy).unwrap();
            let mut client = socket.into_stop_client(peers.expected, request).unwrap();
            let started = Instant::now();
            loop {
                assert!(started.elapsed() < Duration::from_secs(10), "checked stop exchange stalled");
                match client.step().unwrap() {
                    StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                    StopClientProgress::Complete => return client.receipt().unwrap(),
                    _ => std::thread::sleep(Duration::from_millis(1)),
                }
            }
        });
        let generated = generate_selected(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000), Some(&mut prediction));
        let receipt = controller.join().unwrap();
        assert!(!generated.unwrap()); assert!(receipt.acknowledged()); assert!(receipt.drained());
        let host = driver.supervisor().host().unwrap();
        assert!(host.inspect().stop.is_some()); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.action_consistency_deadline().unwrap(), forecast);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, likelihood);
        if let Some(work) = owned_work {
            let current = host.learned_action_consistency_snapshot().unwrap();
            assert_eq!(current.work, work.work);
            assert_eq!(current.retained_source_bytes, work.retained_source_bytes);
        }
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}
