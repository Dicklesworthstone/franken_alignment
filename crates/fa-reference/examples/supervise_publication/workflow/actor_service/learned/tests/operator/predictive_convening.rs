//! Required pre-output evidence survives an actual held-review CLI continuation.
//! Synthetic native models and neutral forecasts test protocol, not calibration.
use super::reserved_convening as retained;
use super::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::consistency::ConsistencyEventDomain;
use std::sync::atomic::AtomicU64;

fn predictive(root: &Root, verdict: &[u8], owned: bool, funded: usize) -> PathBuf {
    let path = retained::recipe_with_successors(root, verdict, funded);
    let forecast = super::super::forecast::install(root, &path, owned, 60_000);
    let document = fs::read_to_string(&forecast).unwrap();
    fs::write(
        &forecast,
        document.replace("\"max_predictions\":8", "\"max_predictions\":1"),
    )
    .unwrap();
    path
}

fn assert_one_original_prediction(host: &FileOversight, loaded: &Loaded, owned: bool) {
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 4);
    assert_eq!(numerical.sampled_draws, 2);
    assert_eq!(numerical.work.sampling_attempts, 2);
    let snapshot = host.action_consistency_snapshot().unwrap();
    assert_eq!(snapshot.evidence.samples(), 1);
    assert_eq!(snapshot.pending_attempt, None);
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    assert_eq!(host.action_consistency_deadline().unwrap(), None);
    let (frame, event, domain, sample) = if owned {
        let observation = host.learned_action_consistency_observation(1).unwrap();
        let report = host.learned_action_consistency_report(1).unwrap();
        let work = host.learned_action_consistency_snapshot().unwrap();
        assert!(report.prediction().is_ok());
        assert!(report.work().probe_coordinates > 0);
        // One original report accounts for the entire predictor lifetime. A
        // held round neither refits the predictor nor supplies a fresh budget.
        assert_eq!(work.work, report.work());
        assert!(work.retained_source_bytes > 0);
        assert!(!work.has_unreported_work);
        (
            observation.prediction().observation().frame(),
            observation.event(),
            observation.event_domain(),
            observation.sample(),
        )
    } else {
        let observation = host.action_consistency_observation(1).unwrap();
        (
            observation.prediction().observation().frame(),
            observation.event(),
            observation.event_domain(),
            observation.sample(),
        )
    };
    assert_eq!((frame.stream, frame.sequence, frame.position), (5, 2, 1));
    assert!(event);
    assert_eq!(sample, 1);
    assert_eq!(
        domain,
        ConsistencyEventDomain::StreamMessagePrefix(loaded.stream)
    );
}

fn assert_round(host: &FileOversight, round: u64, expected: Consequence) {
    let audited = host.review_replay(round).unwrap();
    assert_eq!(
        audited
            .application()
            .as_ref()
            .unwrap()
            .policy
            .control
            .decision
            .consequence,
        expected
    );
}

#[test]
fn predictive_command_convenes_original_held_review_and_still_requires_fresh_human_custody() {
    for owned in [false, true] {
        for choice in [ReviewDecision::Approve, ReviewDecision::Reject] {
            let root = Root::new();
            let config = configured(&root);
            let path = predictive(&root, b"allow", owned, 1);
            evidence(&root, &config, 1, true, false);
            let peers = fixture::peers(&root, &config);
            let profile = actor_profile(&root, &config, 1);
            let (operator_path, selected) = retained::selected(&root, &profile, 1);
            let args = retained::arguments(&root, &config, &path, &profile, &operator_path);
            let host_path = root.0.join("command-host.json");
            let host_document = fs::read_to_string(&host_path)
                .unwrap()
                .replace("\"commit_ms\": 5000", "\"commit_ms\": 1")
                .replace("\"reveal_ms\": 10000", "\"reveal_ms\": 2");
            fs::write(host_path, host_document).unwrap();
            let time = Arc::new(AtomicU64::new(1000));
            let freeze_time = Arc::new(AtomicBool::new(false));
            let held_time = Arc::clone(&freeze_time);
            let permit_human = Arc::new(AtomicBool::new(false));
            let after_begin = Arc::clone(&permit_human);
            let helper_file = root.0.join("helper-weights.safetensors");
            let forecast_file = root.0.join("forecast.json");
            let original_helper = fs::read(&helper_file).unwrap();
            let original_forecast = fs::read(&forecast_file).unwrap();
            let changed_helper = helper_file.clone();
            let changed_forecast = forecast_file.clone();
            let human_socket = peers.socket(1);
            let observer = thread::spawn(move || {
                let mut socket = connect(&selected);
                let plan = retained::held(&mut socket, selected.settings.scope, 501);
                held_time.store(true, Ordering::Release);
                assert!(!human_socket.exists());
                assert_eq!(
                    field(
                        &plan,
                        &[
                            "result",
                            "value",
                            "knowledge",
                            "value",
                            "action",
                            "arguments",
                            "expected",
                            "held_round"
                        ]
                    )
                    .as_str(),
                    Some("1")
                );
                // These immutable inputs were already admitted before the
                // first Begin. A successor uses the retained owners and the
                // settled forecast, even if their source files are replaced.
                fs::write(changed_helper, b"unusable replacement helper").unwrap();
                fs::write(changed_forecast, b"unusable replacement predictor").unwrap();
                let command = retained::action(selected.settings.scope, &plan);
                let opened = retained::wait_outcome(&mut socket, &command, "opened");
                for _ in 0..3 {
                    assert_eq!(retained::exchange(&mut socket, &command), opened);
                }
                after_begin.store(true, Ordering::Release);
            });
            let mut peer = None;
            let mut reviewer = None;
            let result = command_with_clock(&args, None, || {
                if peer.is_none() && profile.socket.exists() {
                    peer = Some(actor(
                        profile.clone(),
                        LearnedTextRelease::Message,
                        ElapsedTick(50_000),
                        Behavior::Observe,
                    ));
                }
                if reviewer.is_none()
                    && permit_human.load(Ordering::Acquire)
                    && peers.socket(1).exists()
                {
                    reviewer = Some(human(peers.clone(), 1, choice));
                }
                // The first short native round really expires. Only the
                // operator's acknowledged Hold freezes time for its fresh
                // reserved successor, retaining the original action deadline.
                ElapsedTick(if freeze_time.load(Ordering::Acquire) {
                    time.load(Ordering::SeqCst)
                } else {
                    time.fetch_add(1, Ordering::SeqCst)
                })
            });
            let approved = choice == ReviewDecision::Approve;
            assert_eq!(result.is_ok(), approved, "owned={owned}: {result:?}");
            observer.join().unwrap();
            let response = peer.expect("original actor transport").join().unwrap();
            let packet = reviewer
                .expect("fresh independent human review")
                .join()
                .unwrap();
            assert_eq!(packet.action().spec().deadline, ElapsedTick(50_000));
            assert_eq!(
                matches!(
                    response.result,
                    Ok(Knowledge::Known {
                        value: ActorOutcome::Executed,
                        ..
                    })
                ),
                approved
            );
            fs::write(helper_file, original_helper).unwrap();
            fs::write(forecast_file, original_forecast).unwrap();
            let loaded = recipe::load(&path, &config, false).unwrap();
            let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
            let state = host.inspect();
            assert_eq!(state.executions, u64::from(approved));
            assert_eq!(
                state.payload,
                if approved { b"A".to_vec() } else { Vec::new() }
            );
            assert_eq!(
                state.control.ledger.charged,
                if approved {
                    packet.action().spec().units
                } else {
                    0
                }
            );
            assert_eq!(state.control.ledger.reserved, 0);
            assert_eq!(host.request_action(1).unwrap(), packet.action());
            assert_round(&host, 1, Consequence::HoldEffect);
            assert_round(&host, 501, Consequence::Continue);
            assert!(host.review_replay(502).is_err());
            assert_one_original_prediction(&host, &loaded, owned);
            let prediction = host.action_consistency_snapshot().unwrap();
            assert!(!prediction.coverage_lost && !prediction.evidence.crossed());
            assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        }
    }
}

#[test]
fn predictive_held_command_preserves_source_stop_and_original_reserved_capacity() {
    for owned in [false, true] {
        for mode in ["source", "stop", "unfunded", "limited"] {
            let root = Root::new();
            let config = configured(&root);
            let funded = usize::from(mode != "unfunded");
            let path = predictive(&root, b"hold", owned, funded);
            evidence(&root, &config, 1, true, false);
            let original_source = fs::read(root.0.join("evidence.json")).unwrap();
            let peers = fixture::peers(&root, &config);
            let profile = actor_profile(&root, &config, 1);
            let (operator_path, selected) =
                retained::selected(&root, &profile, if mode == "limited" { 3 } else { 1 });
            let args = retained::arguments(&root, &config, &path, &profile, &operator_path);
            let stopping = peers.clone();
            let source_file = root.0.join("evidence.json");
            let observer = (funded > 0).then(|| {
                thread::spawn(move || {
                    let mut socket = connect(&selected);
                    let plan = retained::held(&mut socket, selected.settings.scope, 501);
                    assert!(!stopping.socket(1).exists());
                    match mode {
                        "source" => {
                            fs::remove_file(source_file).unwrap();
                            let command = retained::action(selected.settings.scope, &plan);
                            retained::wait_outcome(&mut socket, &command, "failed");
                        }
                        "stop" => {
                            // The independent Stop transport remains serviceable
                            // while the operator leaves a request incomplete.
                            socket.write_all(b"{\"request_id\":").unwrap();
                            let mut stop = stopping.connect_stop_client(1).unwrap();
                            let started = Instant::now();
                            loop {
                                assert!(started.elapsed() < Duration::from_secs(25));
                                match stop.step().unwrap() {
                                    StopClientProgress::NeedsDecision => {
                                        stop.request_stop().unwrap()
                                    }
                                    StopClientProgress::Complete => break,
                                    _ => thread::sleep(Duration::from_millis(1)),
                                }
                            }
                            assert!(stop.receipt().is_some());
                        }
                        "limited" => {
                            let command = retained::action(selected.settings.scope, &plan);
                            retained::wait_outcome(&mut socket, &command, "opened");
                        }
                        _ => unreachable!(),
                    }
                })
            });
            let mut peer = None;
            let result = command_with_clock(&args, None, || {
                if peer.is_none() && profile.socket.exists() {
                    peer = Some(actor(
                        profile.clone(),
                        LearnedTextRelease::Message,
                        ElapsedTick(50_000),
                        Behavior::Observe,
                    ));
                }
                ElapsedTick(1000)
            });
            assert!(result.is_err(), "owned={owned}, mode={mode}");
            if let Some(observer) = observer {
                observer.join().unwrap();
            }
            let response = peer.expect("original actor transport").join().unwrap();
            assert!(!matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
            assert!(!peers.socket(1).exists());
            // Restoring the original bytes permits subsequent guarded recovery
            // inspection; it cannot repeat the failed command or replace its receipt.
            fs::write(root.0.join("evidence.json"), original_source).unwrap();
            let loaded = recipe::load(&path, &config, false).unwrap();
            let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
            let state = host.inspect();
            assert_eq!(
                (
                    state.executions,
                    state.control.ledger.charged,
                    state.control.ledger.reserved
                ),
                (0, 0, 0)
            );
            assert!(state.payload.is_empty());
            if mode == "source" || mode == "stop" {
                assert!(state.stop.is_some());
            }
            assert_round(&host, 1, Consequence::HoldEffect);
            if mode == "limited" {
                assert_round(&host, 501, Consequence::HoldEffect);
            } else {
                assert!(host.review_replay(501).is_err());
            }
            // Three operator grants cannot create a second successor when
            // only one original native cohort was prepaid before Begin.
            assert!(host.review_replay(502).is_err());
            assert_one_original_prediction(&host, &loaded, owned);
            assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        }
    }
}
