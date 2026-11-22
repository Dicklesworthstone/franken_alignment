//! Actual learned command and original authenticated operator/human/Stop peers.
//! Fixed synthetic weights establish custody and protocol behavior only.
use super::*;
use fa_reference::action::Scope;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use fa_reference::action::consequence::oversight::sidecar::MAX_SIDECAR_REFINEMENT_ROUNDS;
use fa_reference::tower::ReadArguments;
use std::sync::atomic::AtomicU64;

fn recipe_with_successors(root: &Root, verdict: &[u8], operations: usize) -> PathBuf {
    let path = fixture::write(root, verdict);
    let total = operations + 1;
    let original = fs::read_to_string(&path).unwrap();
    let prepared = original
        .replace(
            "\"evaluations\":2",
            &format!("\"evaluations\":{}", 2 * total),
        )
        .replace(
            &format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 2),
            &format!(
                "\"scalar_products\":{}",
                MAX_DECODER_PRODUCTS * 2 * total as u64
            ),
        )
        .replace(
            "\"sampling_entries\":8192",
            &format!("\"sampling_entries\":{}", 8192 * total),
        );
    fs::write(&path, prepared).unwrap();
    path
}

fn selected(
    root: &Root,
    actor: &Profile,
    operations: usize,
) -> (PathBuf, live_operator::Selection) {
    let (path, _) = selection(root, actor);
    let document = fs::read_to_string(&path).unwrap()
        .replace("\"exchange_limit\":16", "\"exchange_limit\":1024")
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replacen('{', &format!("{{\"convening\":{{\"queries\":1024,\"operations\":{operations},\"authority\":\"convene_held_review\"}},"), 1);
    fs::write(&path, document).unwrap();
    let selected = live_operator::Selection::read(&path, GENERATION).unwrap();
    (path, selected)
}

fn exchange(socket: &mut UnixStream, command: &str) -> Json {
    socket.write_all(command.as_bytes()).unwrap();
    socket.write_all(b"\n").unwrap();
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() <= 1024 * 1024);
        if byte[0] == b'\n' {
            break;
        }
    }
    strict_json::parse(&bytes, KNOWLEDGE_JSON_LIMITS).unwrap()
}

fn value<'a>(reply: &'a Json, kind: &str) -> Option<&'a Json> {
    let result = reply.get("result")?;
    if result.get("kind")?.as_str()? != kind {
        return None;
    }
    let knowledge = result.get("value")?.get("knowledge")?;
    if knowledge.get("state")?.as_str()? != "known" {
        return None;
    }
    knowledge.get("value")
}

fn next(scope: Scope, round: u64) -> String {
    let address = Address::parse(&format!(
        "fa://{}/attempt/request-{}-{}-{}-{}-1@{GENERATION}",
        scope.tenant, scope.run, scope.principal, scope.branch, scope.authority
    ))
    .unwrap();
    let mut request = TowerRequest::new(format!("next-{round}"), address, Verb::Next);
    request.arguments = ReadArguments::ConveneNext { round };
    encode_request(&request).unwrap()
}

fn held(socket: &mut UnixStream, scope: Scope, round: u64) -> Json {
    let command = next(scope, round);
    let started = Instant::now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "original learned Hold never became available"
        );
        let reply = exchange(socket, &command);
        if let Some(plan) = value(&reply, "convene-next") {
            if field(plan, &["can_convene"]).as_bool() == Some(true) {
                return reply;
            }
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn number(value: &Json) -> u64 {
    value.as_str().unwrap().parse().unwrap()
}

fn action(scope: Scope, plan: &Json) -> String {
    let action = field(value(plan, "convene-next").unwrap(), &["action"]);
    let args = field(action, &["arguments"]);
    let expected = field(args, &["expected"]);
    let mut request = TowerRequest::new(
        field(action, &["operation_id"]).as_str().unwrap(),
        Address::parse(field(action, &["address"]).as_str().unwrap()).unwrap(),
        Verb::Convene,
    );
    request.idempotency_key = Some(
        field(action, &["idempotency_key"])
            .as_str()
            .unwrap()
            .to_owned(),
    );
    request.arguments = ReadArguments::Convene {
        round: number(field(args, &["round"])),
        expected: FileConveneBasis {
            scope,
            request: number(field(expected, &["request"])),
            request_generation: number(field(expected, &["request_generation"])),
            attempt: number(field(expected, &["attempt"])),
            held_round: number(field(expected, &["held_round"])),
            journal_revision: number(field(expected, &["journal_revision"])),
            control_sequence: number(field(expected, &["control_sequence"])),
            authority_epoch: number(field(expected, &["authority_epoch"])),
            input_revision: number(field(expected, &["input_revision"])),
            recorded_at: expected
                .get("recorded_at")
                .filter(|value| !value.is_null())
                .map(|value| ElapsedTick(number(value))),
            deadline: ElapsedTick(number(field(expected, &["deadline"]))),
        },
    };
    encode_request(&request).unwrap()
}

fn wait_outcome(socket: &mut UnixStream, command: &str, expected: &str) -> Json {
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(25));
        let reply = exchange(socket, command);
        let outcome = value(&reply, "convene").unwrap();
        if field(outcome, &["outcome"]).as_str() == Some(expected) {
            return reply;
        }
        assert_eq!(field(outcome, &["outcome"]).as_str(), Some("pending"));
        thread::sleep(Duration::from_millis(1));
    }
}

fn arguments(
    root: &Root,
    config: &Config,
    recipe: &Path,
    actor: &Profile,
    operator: &Path,
) -> Vec<String> {
    let mut args = super::super::mediated_topology::command_inputs(root, config, recipe, actor);
    args.extend([
        "--operator-profile".into(),
        operator.display().to_string(),
        "--tower-generation".into(),
        GENERATION.to_string(),
    ]);
    args
}

#[test]
fn original_learned_command_convenes_fresh_native_review_and_requires_its_human_key() {
    for choice in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let config = configured(&root);
        let path = recipe_with_successors(&root, b"allow", 1);
        evidence(&root, &config, 1, true, false);
        let generation = recipe::load(&path, &config, false).unwrap().generation;
        let peers = fixture::peers(&root, &config);
        let profile = actor_profile(&root, &config, 1);
        let (operator_path, selected) = selected(&root, &profile, 1);
        let args = arguments(&root, &config, &path, &profile, &operator_path);
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
        let after_successor = Arc::clone(&permit_human);
        let helper_file = root.0.join("helper-weights.safetensors");
        let original_helper = fs::read(&helper_file).unwrap();
        let changed_file = helper_file.clone();
        let observer = thread::spawn(move || {
            let mut socket = connect(&selected);
            let plan = held(&mut socket, selected.settings.scope, 501);
            held_time.store(true, Ordering::Release);
            // Replacing the on-disk helper after Hold cannot retune any reserved
            // evaluator. Every actual owner was already constructed before Begin.
            fs::write(&changed_file, b"unusable replacement helper bytes").unwrap();
            let command = action(selected.settings.scope, &plan);
            let opened = wait_outcome(&mut socket, &command, "opened");
            for _ in 0..3 {
                assert_eq!(exchange(&mut socket, &command), opened);
            }
            after_successor.store(true, Ordering::Release);
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
            // Advancing on every original callback expires the first short
            // congress regardless of peer scheduling. After its actual Hold,
            // the observer freezes time before selecting the fresh successor.
            ElapsedTick(if freeze_time.load(Ordering::Acquire) {
                time.load(Ordering::SeqCst)
            } else {
                time.fetch_add(1, Ordering::SeqCst)
            })
        });
        let approved = choice == ReviewDecision::Approve;
        assert_eq!(result.is_ok(), approved, "{result:?}");
        observer.join().unwrap();
        let response = peer.unwrap().join().unwrap();
        let packet = reviewer
            .expect("fresh native Continue still needs independent human custody")
            .join()
            .unwrap();
        assert_eq!(packet.action().spec().deadline, ElapsedTick(50_000));
        let state = FileOversight::read_publication_with_learned_generation(
            &config.store,
            &config.profile,
            &generation,
        )
        .unwrap();
        assert_eq!(state.executions, u64::from(approved));
        assert_eq!(
            state.payload,
            if approved { b"A".to_vec() } else { Vec::new() }
        );
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
        assert_eq!(state.control.ledger.reserved, 0);
        for (round, expected) in [(1, Consequence::HoldEffect), (501, Consequence::Continue)] {
            let audited = FileOversight::read_review_replay_with_learned_generation(
                &config.store,
                &config.profile,
                &generation,
                round,
            )
            .unwrap();
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
        fs::write(&helper_file, original_helper).unwrap();
        let loaded = recipe::load(&path, &config, false).unwrap();
        let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(
            host.learned_generation_inspection()
                .unwrap()
                .numerical
                .sampled_draws,
            2
        );
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn learned_reserved_capacity_preserves_admission_and_respects_original_work_and_grants() {
    for mode in [
        "native", "process", "raw", "owned", "unfunded", "limited", "rounds",
    ] {
        let root = Root::new();
        let config = configured(&root);
        let funded = if mode == "rounds" {
            MAX_SIDECAR_REFINEMENT_ROUNDS - 1
        } else {
            1
        };
        let path = recipe_with_successors(&root, b"allow", funded);
        if mode == "process" {
            fs::write(
                &path,
                super::super::processes::process_recipe(&root, "allow"),
            )
            .unwrap();
        } else if mode == "raw" || mode == "owned" {
            super::super::forecast::install(&root, &path, mode == "owned", 60_000);
        } else if mode == "unfunded" {
            fs::write(&path, fixture::recipe(&root)).unwrap();
        }
        let peers = fixture::peers(&root, &config);
        let profile = actor_profile(&root, &config, 1);
        let operations = match mode {
            "rounds" => MAX_SIDECAR_REFINEMENT_ROUNDS,
            "limited" => 3,
            _ => 1,
        };
        let (_, selected) = selected(&root, &profile, operations);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let admission = selected.check(&config, &loaded, &profile, &peers, None);
        let supported = !matches!(mode, "process" | "raw" | "owned");
        assert_eq!(admission.is_ok(), supported, "mode={mode}: {admission:?}");
        if supported {
            let expected = match mode {
                "unfunded" => 0,
                "rounds" => MAX_SIDECAR_REFINEMENT_ROUNDS - 1,
                _ => 1,
            };
            assert_eq!(
                review::reserved_successors(&loaded, operations),
                expected,
                "{mode}"
            );
            assert_eq!(review::reserved_successors(&loaded, 0), 0);
        }
        assert!(!config.store.exists());
        assert!(!profile.socket.exists());
        assert!(!selected.settings.socket.exists());
    }
}

#[test]
fn learned_reserved_capacity_never_refills_work_or_resamples_after_hold() {
    for (funded, operations) in [(0, 1), (1, 1), (1, 3)] {
        let root = Root::new();
        let config = configured(&root);
        let path = recipe_with_successors(&root, b"hold", funded);
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let generation = loaded.generation.clone();
        let peers = fixture::peers(&root, &config);
        let profile = actor_profile(&root, &config, 1);
        let (_, selected) = selected(&root, &profile, operations);
        let remote =
            live_operator::Selection::read(&root.0.join("operator.json"), GENERATION).unwrap();
        let observer = (funded > 0).then(|| {
            thread::spawn(move || {
                let mut socket = connect(&remote);
                let plan = held(&mut socket, remote.settings.scope, 501);
                let command = action(remote.settings.scope, &plan);
                wait_outcome(&mut socket, &command, "opened");
            })
        });
        let mut peer = None;
        let result = run_with_actor_services(
            configured(&root),
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
                if peer.is_none() && profile.socket.exists() {
                    peer = Some(actor(
                        profile.clone(),
                        LearnedTextRelease::Message,
                        ElapsedTick(50_000),
                        Behavior::Observe,
                    ));
                }
                ElapsedTick(1000)
            },
        )
        .unwrap();
        if let Some(observer) = observer {
            observer.join().unwrap();
        }
        assert!(result.failure.is_none(), "{:?}", result.failure);
        assert!(!matches!(
            peer.unwrap().join().unwrap().result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            })
        ));
        assert!(!peers.socket(1).exists());
        let state = FileOversight::read_publication_with_learned_generation(
            &config.store,
            &config.profile,
            &generation,
        )
        .unwrap();
        assert_eq!(
            (
                state.executions,
                state.control.ledger.charged,
                state.control.ledger.reserved
            ),
            (0, 0, 0)
        );
        for round in [1, 501].into_iter().take(funded + 1) {
            let audited = FileOversight::read_review_replay_with_learned_generation(
                &config.store,
                &config.profile,
                &generation,
                round,
            )
            .unwrap();
            assert_eq!(
                audited
                    .application()
                    .as_ref()
                    .unwrap()
                    .policy
                    .control
                    .decision
                    .consequence,
                Consequence::HoldEffect
            );
        }
        assert!(
            FileOversight::read_review_replay_with_learned_generation(
                &config.store,
                &config.profile,
                &generation,
                if funded == 0 { 501 } else { 502 }
            )
            .is_err()
        );
        let loaded = recipe::load(&path, &config, false).unwrap();
        let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(
            host.learned_generation_inspection()
                .unwrap()
                .numerical
                .sampled_draws,
            2
        );
    }
}

#[test]
fn held_learned_wait_preserves_fresh_source_stop_and_deadline_boundaries() {
    for mode in ["source", "stop", "deadline"] {
        let root = Root::new();
        let config = configured(&root);
        let path = recipe_with_successors(&root, b"hold", 1);
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&path, &config, true).unwrap();
        let generation = loaded.generation.clone();
        let peers = fixture::peers(&root, &config);
        let profile = actor_profile(&root, &config, 1);
        let (_, selected) = selected(&root, &profile, 1);
        let remote =
            live_operator::Selection::read(&root.0.join("operator.json"), GENERATION).unwrap();
        let stopping = peers.clone();
        let source = root.0.join("evidence.json");
        let time = Arc::new(AtomicU64::new(1000));
        let other_time = Arc::clone(&time);
        let observer = thread::spawn(move || {
            let mut socket = connect(&remote);
            let plan = held(&mut socket, remote.settings.scope, 501);
            fs::remove_file(source).unwrap();
            match mode {
                "source" => {
                    let command = action(remote.settings.scope, &plan);
                    wait_outcome(&mut socket, &command, "failed");
                }
                "deadline" => {
                    other_time.store(50_000, Ordering::SeqCst);
                }
                "stop" => {
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
            configured(&root),
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
                if peer.is_none() && profile.socket.exists() {
                    peer = Some(actor(
                        profile.clone(),
                        LearnedTextRelease::Message,
                        ElapsedTick(50_000),
                        Behavior::Observe,
                    ));
                }
                ElapsedTick(time.load(Ordering::SeqCst))
            },
        )
        .unwrap();
        observer.join().unwrap();
        assert_eq!(
            result.failure.is_some(),
            mode != "stop",
            "mode={mode}: {:?}",
            result.failure
        );
        assert!(!matches!(
            peer.unwrap().join().unwrap().result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            })
        ));
        assert!(
            FileOversight::read_review_replay_with_learned_generation(
                &config.store,
                &config.profile,
                &generation,
                501
            )
            .is_err(),
            "no successor completes after source loss, Stop or the original deadline"
        );
        let state = FileOversight::read_publication_with_learned_generation(
            &config.store,
            &config.profile,
            &generation,
        )
        .unwrap();
        assert!(state.stop.is_some());
        assert_eq!(
            (
                state.executions,
                state.control.ledger.charged,
                state.control.ledger.reserved
            ),
            (0, 0, 0)
        );
        assert!(!peers.socket(1).exists());
    }
}
