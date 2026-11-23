//! Real fixed subprocess cohorts, authenticated operator continuation and the
//! original actor/human owners. Ballots and predictors are protocol fixtures.
use super::reserved_convening as retained;
use super::*;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::oversight::{
    consistency::ConsistencyEventDomain,
    helper_client::{ClientPhase, HelperClient},
    helper_workers::HelperLimits,
};
use fa_reference::round::Verdict;
use std::sync::atomic::AtomicU64;

const FIRST: u64 = 701;
const SECOND: u64 = 702;

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Admit every program and all starts before the first Begin. There is no
/// replacement roster, reserved native evaluator or later program-file read.
pub(super) fn fixed_recipe(root: &Root, predictor: Option<bool>, count: usize) -> PathBuf {
    let path = fixture::write(root, b"allow");
    let forecast = predictor.map(|owned| {
        let forecast = super::super::forecast::install(root, &path, owned, 60_000);
        let bytes = fs::read_to_string(&forecast).unwrap();
        fs::write(&forecast, bytes.replace("\"max_predictions\":8", "\"max_predictions\":1"))
            .unwrap();
        forecast
    });
    let executable = quote(std::env::current_exe().unwrap().to_str().unwrap());
    let directory = quote(root.0.to_str().unwrap());
    let test = quote(&format!(
        "{}::process_child",
        module_path!().split_once("::").unwrap().1
    ));
    let rounds = (0..count).map(|index| {
        let id = FIRST + index as u64;
        let members = ["alpha", "beta"].into_iter().map(|name| {
            // One weight-5 Hold keeps the request held. Two Holds reach the
            // independently configured run-suspension threshold instead.
            let mode = if index == 0 && name == "alpha" { "hold" } else { "allow" };
            let trace = quote(root.0.join(format!("{id}-{name}.sidecar")).to_str().unwrap());
            format!(r#""{name}":{{"executable":{executable},"directory":{directory},"arguments":["--exact",{test},"--nocapture"],"environment":{{"FA_FIXED_PROCESS_MODE":"{mode}","FA_FIXED_PROCESS_MEMBER":"{name}","FA_FIXED_PROCESS_TRACE":{trace}}}}}"#)
        }).collect::<Vec<_>>().join(",");
        format!(r#"{{"id":{id},"members":{{{members}}}}}"#)
    }).collect::<Vec<_>>().join(",");
    let forecast = forecast.as_ref().map_or_else(|| "null".to_owned(),
        |path| quote(path.to_str().unwrap()));
    let priority = if count == 1 { "[]" } else {
        r#"[{"layer":1,"side":"key","position":3,"head":0}]"#
    };
    let raw = fixture::recipe(root)
        .replace("fa.learned-publication/1", "fa.learned-publication/6")
        .replace(
            &format!(r#""native_roster":"{}/native-roster.json""#, root.0.display()),
            &format!(r#""forecast":{forecast}"#),
        )
        .replace(r#""budget":{"rounds":1"#,
            &format!(r#""priority":{priority},"budget":{{"rounds":{count}"#));
    let (prefix, _) = raw.split_once(r#","native_review":"#).unwrap();
    fs::write(&path, format!(r#"{prefix},"process_review":{{"rounds":[{rounds}],"polls":4096,"input_bytes":{},"salt_bytes":32,"starts":{}}}}}"#,
        HelperLimits::default().input_bytes, count * 2)).unwrap();
    path
}

#[test]
fn process_child() {
    let Ok(mode) = std::env::var("FA_FIXED_PROCESS_MODE") else { return; };
    let name = std::env::var("FA_FIXED_PROCESS_MEMBER").unwrap();
    let trace = std::env::var_os("FA_FIXED_PROCESS_TRACE").unwrap();
    let config = Config::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"
    ))).unwrap();
    let profile = config.profile.committee.members()[&name].profile_at(0);
    let mut client = HelperClient::<UnixStream>::from_process_stdin(profile).unwrap();
    let started = Instant::now();
    let mut answered = false;
    while started.elapsed() < Duration::from_secs(15) {
        if client.step().is_err() { return; }
        if client.phase() == ClientPhase::NeedsInference && !answered {
            let input = client.input().unwrap().actual_input();
            let sidecar = input.part_bytes(1).unwrap();
            assert!(sidecar.starts_with(b"FASIDE\0\x01"));
            fs::write(&trace, sidecar).unwrap();
            client.respond(
                if mode == "hold" { Verdict::Hold } else { Verdict::Allow },
                if name == "alpha" { &[19; 32] } else { &[23; 32] },
            ).unwrap();
            answered = true;
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        thread::sleep(Duration::from_millis(1));
    }
    panic!("fixed learned process helper did not finish");
}

fn assert_round(host: &FileOversight, round: u64, expected: Consequence) {
    let audited = host.review_replay(round).unwrap();
    assert_eq!(audited.application().as_ref().unwrap()
        .policy.control.decision.consequence, expected);
}

fn assert_original_actor_and_prediction(host: &FileOversight, loaded: &Loaded, predictor: Option<bool>) {
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws,
        numerical.work.sampling_attempts), (4, 2, 2));
    let Some(owned) = predictor else { return; };
    let snapshot = host.action_consistency_snapshot().unwrap();
    assert_eq!(snapshot.evidence.samples(), 1);
    assert!(!snapshot.coverage_lost && !snapshot.evidence.crossed());
    assert_eq!(snapshot.pending_attempt, None);
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    assert_eq!(host.action_consistency_deadline().unwrap(), None);
    let (frame, event, domain, sample) = if owned {
        let observation = host.learned_action_consistency_observation(1).unwrap();
        let report = host.learned_action_consistency_report(1).unwrap();
        let work = host.learned_action_consistency_snapshot().unwrap();
        assert!(report.prediction().is_ok());
        assert!(report.work().probe_coordinates > 0);
        assert_eq!(work.work, report.work());
        assert!(work.retained_source_bytes > 0);
        assert!(!work.has_unreported_work);
        (observation.prediction().observation().frame(), observation.event(),
            observation.event_domain(), observation.sample())
    } else {
        let observation = host.action_consistency_observation(1).unwrap();
        (observation.prediction().observation().frame(), observation.event(),
            observation.event_domain(), observation.sample())
    };
    assert_eq!((frame.stream, frame.sequence, frame.position), (5, 2, 1));
    assert!(event);
    assert_eq!(sample, 1);
    assert_eq!(domain, ConsistencyEventDomain::StreamMessagePrefix(loaded.stream));
}

#[test]
fn learned_command_convenes_fixed_process_successor_and_requires_fresh_human_approval() {
    for predictor in [None, Some(false), Some(true)] {
        for choice in [ReviewDecision::Approve, ReviewDecision::Reject] {
            let root = Root::new();
            let config = configured(&root);
            let path = fixed_recipe(&root, predictor, 2);
            evidence(&root, &config, 1, true, false);
            let peers = fixture::peers(&root, &config);
            let profile = actor_profile(&root, &config, 1);
            let (operator_path, selected) = retained::selected(&root, &profile, 1);
            let args = retained::arguments(&root, &config, &path, &profile, &operator_path);
            let original_recipe = fs::read(&path).unwrap();
            let changed_recipe = path.clone();
            let forecast = root.0.join("forecast.json");
            let original_forecast = predictor.map(|_| fs::read(&forecast).unwrap());
            let changed_forecast = forecast.clone();
            let permit_human = Arc::new(AtomicBool::new(false));
            let after_begin = Arc::clone(&permit_human);
            let human_socket = peers.socket(1);
            let observer = thread::spawn(move || {
                let mut socket = connect(&selected);
                let plan = retained::held(&mut socket, selected.settings.scope, SECOND);
                assert!(!human_socket.exists(), "Hold cannot acquire a human publication key");
                assert_eq!(field(&plan, &["result", "value", "knowledge", "value",
                    "action", "arguments", "expected", "held_round"]).as_str(), Some("701"));
                // Successors use the original admitted schedule and settled
                // predictor even if their external descriptors are replaced.
                fs::write(changed_recipe, b"unusable replacement process recipe").unwrap();
                if predictor.is_some() {
                    fs::write(changed_forecast, b"unusable replacement predictor").unwrap();
                }
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
                    peer = Some(actor(profile.clone(), LearnedTextRelease::Message,
                        ElapsedTick(50_000), Behavior::Observe));
                }
                if reviewer.is_none() && permit_human.load(Ordering::Acquire)
                    && peers.socket(1).exists()
                {
                    reviewer = Some(human(peers.clone(), 1, choice));
                }
                ElapsedTick(1000)
            });
            let approved = choice == ReviewDecision::Approve;
            assert_eq!(result.is_ok(), approved, "predictor={predictor:?}: {result:?}");
            observer.join().unwrap();
            let response = peer.expect("original actor endpoint").join().unwrap();
            let packet = reviewer.expect("fresh human approval after process Continue")
                .join().unwrap();
            assert_eq!(packet.action().spec().deadline, ElapsedTick(50_000));
            assert_eq!(matches!(response.result,
                Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), approved);
            fs::write(&path, original_recipe).unwrap();
            if let Some(bytes) = original_forecast { fs::write(forecast, bytes).unwrap(); }
            let loaded = recipe::load(&path, &config, false).unwrap();
            assert!(loaded.native.is_none() && loaded.process.is_some());
            let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
            let state = host.inspect();
            assert_eq!(state.executions, u64::from(approved));
            assert_eq!(state.payload, if approved { b"A".to_vec() } else { Vec::new() });
            assert_eq!(state.control.ledger.charged,
                if approved { packet.action().spec().units } else { 0 });
            assert_eq!(state.control.ledger.reserved, 0);
            assert_eq!(host.request_action(1).unwrap(), packet.action());
            assert_round(&host, FIRST, Consequence::HoldEffect);
            assert_round(&host, SECOND, Consequence::Continue);
            assert!(host.review_replay(SECOND + 1).is_err());
            assert_original_actor_and_prediction(&host, &loaded, predictor);
            for name in ["alpha", "beta"] {
                assert_eq!(
                    fs::read(root.0.join(format!("{FIRST}-{name}.sidecar"))).unwrap(),
                    fs::read(root.0.join(format!("{SECOND}-{name}.sidecar"))).unwrap(),
                    "successor must preserve the original owned sidecar"
                );
            }
            assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        }
    }
}

#[test]
fn held_process_service_preserves_source_stop_and_original_deadline() {
    for mode in ["source", "stop", "deadline", "window"] {
        let root = Root::new();
        let config = configured(&root);
        let path = fixed_recipe(&root, None, 2);
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let generation = loaded.generation.clone();
        let peers = fixture::peers(&root, &config);
        let profile = actor_profile(&root, &config, 1);
        let (operator_path, selected) = retained::selected(&root, &profile, 1);
        let remote = live_operator::Selection::read(&operator_path, GENERATION).unwrap();
        let stopping = peers.clone();
        let source = root.0.join("evidence.json");
        let fixed_commit = 1000 + config.timing.reveal_ms + config.timing.commit_ms;
        let human_socket = peers.socket(1);
        let time = Arc::new(AtomicU64::new(1000));
        let other_time = Arc::clone(&time);
        let observer = thread::spawn(move || {
            let mut socket = connect(&remote);
            let plan = retained::held(&mut socket, remote.settings.scope, SECOND);
            assert!(!human_socket.exists());
            // A restrictive peer remains usable without live policy input.
            fs::remove_file(source).unwrap();
            match mode {
                "source" => {
                    let command = retained::action(remote.settings.scope, &plan);
                    retained::wait_outcome(&mut socket, &command, "failed");
                }
                "deadline" => { other_time.store(50_000, Ordering::SeqCst); }
                "window" => { other_time.store(fixed_commit, Ordering::SeqCst); }
                "stop" => {
                    // A partial operator frame must not monopolize the held
                    // service or prevent the independent Stop peer.
                    socket.write_all(b"{\"request_id\":").unwrap();
                    let mut stop = stopping.connect_stop_client(1).unwrap();
                    let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(25));
                        match stop.step().unwrap() {
                            StopClientProgress::NeedsDecision => stop.request_stop().unwrap(),
                            StopClientProgress::Complete => break,
                            _ => thread::sleep(Duration::from_millis(1)),
                        }
                    }
                    assert!(stop.receipt().is_some());
                }
                _ => unreachable!(),
            }
        });
        let mut peer = None;
        let result = run_with_actor_services(
            configured(&root), loaded, &peers, false, None, None, None,
            Some(&profile), None, Some(&selected), || {
                if peer.is_none() && profile.socket.exists() {
                    peer = Some(actor(profile.clone(), LearnedTextRelease::Message,
                        ElapsedTick(50_000), Behavior::Observe));
                }
                ElapsedTick(time.load(Ordering::SeqCst))
            },
        ).unwrap();
        observer.join().unwrap();
        assert_eq!(result.failure.is_some(), matches!(mode, "source" | "deadline"),
            "mode={mode}: {:?}", result.failure);
        assert_eq!(result.cleanup_pending, 0);
        assert!(!matches!(peer.unwrap().join().unwrap().result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let held = FileOversight::read_review_replay_with_learned_generation(
            &config.store, &config.profile, &generation, FIRST,
        ).unwrap();
        assert_eq!(held.application().as_ref().unwrap()
            .policy.control.decision.consequence, Consequence::HoldEffect);
        assert!(FileOversight::read_review_replay_with_learned_generation(
            &config.store, &config.profile, &generation, SECOND,
        ).is_err());
        for name in ["alpha", "beta"] {
            assert!(!root.0.join(format!("{SECOND}-{name}.sidecar")).exists());
        }
        let state = FileOversight::read_publication_with_learned_generation(
            &config.store, &config.profile, &generation,
        ).unwrap();
        assert_eq!(state.stop.is_some(), mode != "window");
        assert_eq!((state.executions, state.control.ledger.charged,
            state.control.ledger.reserved), (0, 0, 0));
        assert!(!peers.socket(1).exists());
    }
}

#[test]
fn process_hold_without_selected_grant_or_fixed_successor_never_waits_or_launches_again() {
    for selected_grant in [false, true] {
        let root = Root::new();
        let config = configured(&root);
        let path = fixed_recipe(&root, None, if selected_grant { 1 } else { 2 });
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let generation = loaded.generation.clone();
        let peers = fixture::peers(&root, &config);
        let profile = actor_profile(&root, &config, 1);
        let (_, selected) = retained::selected(&root, &profile, 1);
        let mut peer = None;
        let result = run_with_actor_services(
            config, loaded, &peers, false, None, None, None, Some(&profile), None,
            selected_grant.then_some(&selected), || {
                if peer.is_none() && profile.socket.exists() {
                    peer = Some(actor(profile.clone(), LearnedTextRelease::Message,
                        ElapsedTick(50_000), Behavior::Observe));
                }
                ElapsedTick(1000)
            },
        ).unwrap();
        assert_eq!(result.cleanup_pending, 0);
        assert!(!matches!(peer.unwrap().join().unwrap().result,
            Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let config = configured(&root);
        let held = FileOversight::read_review_replay_with_learned_generation(
            &config.store, &config.profile, &generation, FIRST,
        ).unwrap();
        assert_eq!(held.application().as_ref().unwrap()
            .policy.control.decision.consequence, Consequence::HoldEffect);
        assert!(FileOversight::read_review_replay_with_learned_generation(
            &config.store, &config.profile, &generation, SECOND,
        ).is_err());
        for name in ["alpha", "beta"] {
            assert!(!root.0.join(format!("{SECOND}-{name}.sidecar")).exists());
        }
        let state = FileOversight::read_publication_with_learned_generation(
            &config.store, &config.profile, &generation,
        ).unwrap();
        assert_eq!((state.executions, state.control.ledger.charged,
            state.control.ledger.reserved), (0, 0, 0));
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
