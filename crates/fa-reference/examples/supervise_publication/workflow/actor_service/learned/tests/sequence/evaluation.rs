//! Original learned Message/Message/Finish and independent late case labels.
//! Real numerical/native/human/socket paths exercise lifecycle composition;
//! fixture weights and authored tests do not establish detector qualification.
use super::super::super::{anchors as archive, evaluation as selected_evaluation};
use super::super::evaluation_tests::{
    accounting_file, field, number, peer_file, read_json, wait_for,
};
use super::*;
use crate::workflow::actor_service::evaluator;
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json;
use std::{
    io::{self, Read},
    process::{Child, Command as ProcessCommand, Stdio},
};

#[path = "evaluation/transport.rs"]
mod transport;

const CHILD: &str =
    "workflow::actor_service::learned::tests::resident_sequence::evaluation::evaluator_process";
const CHILD_ROOT: &str = "FA_RESIDENT_LEARNED_EVALUATOR_ROOT";
const CHILD_MODE: &str = "FA_RESIDENT_LEARNED_EVALUATOR_MODE";

struct EvaluatorChild(Child);
impl EvaluatorChild {
    fn start(root: &Root, mode: &str) -> Self {
        Self(
            ProcessCommand::new(std::env::current_exe().unwrap())
                .args(["--exact", CHILD, "--nocapture"])
                .env(CHILD_ROOT, &root.0)
                .env(CHILD_MODE, mode)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        )
    }

    fn finish(&mut self) {
        let started = Instant::now();
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "independent resident evaluator failed: {status}"
                );
                return;
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "resident evaluator did not finish"
            );
            crate::workflow::pause(1);
        }
    }
}
impl Drop for EvaluatorChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn arguments(root: &Root, accounting: &Path, peer: &Path) -> Vec<String> {
    vec![
        "serve-learned-generated".into(),
        "config".into(),
        "sequence".into(),
        "reviewer".into(),
        "--andon".into(),
        accounting.display().to_string(),
        "--evaluator-profile".into(),
        peer.display().to_string(),
        "--retain-history-anchor".into(),
        root.0.join("resident.anchor").display().to_string(),
    ]
}

fn select_evaluation(
    root: &Root,
    config: &Config,
    entries: &mut [resident::Entry],
    pid: u32,
    candidates: u32,
) -> selected_evaluation::Selection {
    let accounting = accounting_file(root);
    let peer = peer_file(root, &entries[0].actor, pid);
    let text = fs::read_to_string(&peer).unwrap();
    assert!(text.contains("\"candidate_limit\":2"));
    fs::write(
        &peer,
        text.replace(
            "\"candidate_limit\":2",
            &format!("\"candidate_limit\":{candidates}"),
        )
        .replace("\"runtime_ms\":15000", "\"runtime_ms\":45000"),
    )
    .unwrap();
    let (remaining, selection) =
        selected_evaluation::take_sequence_options(&arguments(root, &accounting, &peer), None)
            .unwrap();
    let selection = selection.unwrap().read().unwrap();
    let (_, history) = archive::take_options(&remaining).unwrap();
    history
        .admit(config, &mut entries[0].loaded, false)
        .unwrap();
    selection
}

fn label(root: &Path, round: u64, tag: &str) {
    let profile = root.join("evaluator.json");
    let mut observation = Vec::new();
    evaluator::client::command(
        &[
            "evaluator-case".into(),
            profile.display().to_string(),
            round.to_string(),
        ],
        &mut observation,
    )
    .unwrap();
    let case = strict_json::parse(&observation, KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(
        field(&case, &["knowledge", "state"]).as_str(),
        Some("known")
    );
    assert_eq!(number(&case, &["knowledge", "value", "round"]), round);
    let evidence_root = field(&case, &["knowledge", "value", "case_evidence_root_hex"])
        .as_str()
        .unwrap();
    let assessment = format!(
        r#"{{"schema_version":"fa.native-assessment/1","round":{round},"attempt":{},"policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{evidence_root}","expected_evaluation_revision":{},"origin":{},"evidence_id_hex":"{}","truth":"benign"}}"#,
        number(&case, &["knowledge", "value", "attempt"]),
        number(&case, &["knowledge", "value", "policy_generation"]),
        number(&case, &["knowledge", "value", "congress_generation"]),
        number(&case, &["knowledge", "basis", "evaluation_revision"]),
        9000 + round,
        format!("{round:02x}").repeat(32)
    );
    let path = root.join(format!("assessment-{tag}.json"));
    fs::write(&path, assessment).unwrap();
    let mut receipt = Vec::new();
    evaluator::client::command(
        &[
            "evaluator-assess".into(),
            profile.display().to_string(),
            path.display().to_string(),
        ],
        &mut receipt,
    )
    .unwrap();
    let applied = strict_json::parse(&receipt, KNOWLEDGE_JSON_LIMITS).unwrap();
    assert_eq!(field(&applied, &["status"]).as_str(), Some("applied"));
    for coordinate in ["control_sequence", "authority_epoch"] {
        assert_eq!(
            field(&applied, &["before", coordinate]),
            field(&applied, &["after", coordinate])
        );
    }
    assert_eq!(
        number(&applied, &["after", "evaluation_revision"]),
        number(&applied, &["before", "evaluation_revision"]) + 1
    );
    assert_eq!(
        number(&applied, &["after", "journal_revision"]),
        number(&applied, &["before", "journal_revision"]) + 2
    );
    fs::write(root.join(format!("case-{tag}.json")), observation).unwrap();
    fs::write(root.join(format!("receipt-{tag}.json")), receipt).unwrap();
}

#[test]
fn evaluator_process() {
    let Ok(root) = std::env::var(CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    let mode = std::env::var(CHILD_MODE).unwrap();
    let first = mode == "exhausted";
    let early = matches!(mode.as_str(), "transport" | "early-hold");
    wait_for(&root.join(if first || early {
        "actor-1.finished"
    } else {
        "actor-3.finished"
    }));
    let settings = evaluator::Settings::read(&root.join("evaluator.json")).unwrap();
    assert_eq!(settings.evaluator.pid, Some(std::process::id()));
    assert_ne!(settings.supervisor.pid, Some(std::process::id()));
    if first {
        // Discovery does not race a moving evaluation predecessor. Both reads
        // spend the same original candidate budget even if later reviews apply
        // before the second read; no assessment is silently retried.
        for _ in 0..2 {
            let mut observation = Vec::new();
            evaluator::client::command(
                &[
                    "evaluator-case".into(),
                    root.join("evaluator.json").display().to_string(),
                    "1".into(),
                ],
                &mut observation,
            )
            .unwrap();
            let case = strict_json::parse(&observation, KNOWLEDGE_JSON_LIMITS).unwrap();
            assert_eq!(
                field(&case, &["knowledge", "state"]).as_str(),
                Some("known")
            );
        }
        wait_for(&root.join("actor-3.finished"));
        let mut output = Vec::new();
        assert!(
            evaluator::client::command(
                &[
                    "evaluator-case".into(),
                    root.join("evaluator.json").display().to_string(),
                    "33".into()
                ],
                &mut output
            )
            .is_err(),
            "message handoff renewed exhausted evaluator candidates"
        );
        fs::write(
            root.join("budget.exhausted"),
            b"original candidates stay spent",
        )
        .unwrap();
    } else {
        // A late label cannot require generation/source/helper inputs again.
        for file in [
            "evidence.json",
            "model.json",
            "weights.safetensors",
            "native-roster.json",
        ] {
            fs::remove_file(root.join(file)).unwrap();
        }
        if matches!(mode.as_str(), "partial" | "deadline") {
            let mut socket = UnixStream::connect(settings.socket_path()).unwrap();
            settings
                .supervisor
                .policy()
                .unwrap()
                .verify(&socket)
                .unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(15)))
                .unwrap();
            socket.write_all(b"{\"schema_version\":").unwrap();
            fs::write(
                root.join("partial.ready"),
                b"authenticated incomplete frame",
            )
            .unwrap();
            let mut byte = [0_u8; 1];
            match socket.read(&mut byte) {
                Ok(0) => {}
                Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
                result => panic!("Stop left original resident evaluator intake live: {result:?}"),
            }
            fs::write(root.join("partial.closed"), b"Stop retired intake").unwrap();
        } else {
            if mode == "both" {
                label(&root, 1, "first");
            }
            label(&root, if early { 1 } else { 33 }, "final");
        }
    }
    wait_for(&root.join("service.finished"));
    assert!(UnixStream::connect(settings.socket_path()).is_err());
}

fn poll(peers: &mut Peers, root: &Root, decisions: [Option<ReviewDecision>; 3]) {
    for (index, decision) in decisions.into_iter().enumerate() {
        if peers.profiles[index].socket.exists() {
            peers.seen_actor[index] = true;
            if peers.actors[index].is_none() {
                let due = plus(crate::workflow::clock(), 55_000).unwrap();
                let peer = peers::actor(
                    peers.profiles[index].clone(),
                    peers.release[index],
                    due,
                    peers::Behavior::Observe,
                );
                let marker = root
                    .0
                    .join(format!("actor-{}.finished", peers.profiles[index].request));
                peers.actors[index] = Some(thread::spawn(move || {
                    let response = peer.join().unwrap();
                    fs::write(marker, b"original terminal actor outcome").unwrap();
                    response
                }));
            }
        }
        if peers
            .reviewer
            .socket(peers.profiles[index].request)
            .exists()
        {
            peers.seen_human[index] = true;
            if peers.humans[index].is_none() {
                if let Some(decision) = decision {
                    peers.humans[index] = Some(peers::human(
                        peers.reviewer.clone(),
                        peers.profiles[index].request,
                        decision,
                    ));
                }
            }
        }
    }
}

fn run_sequence(hold: bool, approve: bool, mode: &str, candidates: u32) {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    let hold_assets = hold.then(Root::new);
    if let Some(held) = &hold_assets {
        fixture::write(held, b"hold");
        native::provision_response(held, b"hold");
        let document = fs::read_to_string(&paths[2]).unwrap();
        fs::write(
            &paths[2],
            document.replace(
                root.0.join("native-roster.json").to_str().unwrap(),
                held.0.join("native-roster.json").to_str().unwrap(),
            ),
        )
        .unwrap();
        entries[2].loaded = recipe::load(&paths[2], &config, true).unwrap();
        assert_eq!(entries[0].loaded.generation, entries[2].loaded.generation);
    }
    let reader = Reader::new(&root, &paths[0]);
    let mut child = EvaluatorChild::start(&root, mode);
    let selection = select_evaluation(&root, &config, &mut entries, child.0.id(), candidates);
    let mut peers = Peers::new(&root, &config, &entries);
    let reviewer = peers.reviewer.clone();
    let mut stop_peer = None;
    let mut original_socket = None;
    let result =
        resident::run_with_evaluation(config, entries, &reviewer, None, &selection, || {
            use std::os::unix::fs::MetadataExt;
            if let Ok(metadata) = fs::symlink_metadata(selection.peer.socket_path()) {
                let identity = (metadata.dev(), metadata.ino());
                match original_socket {
                    Some(first) => assert_eq!(first, identity),
                    None => original_socket = Some(identity),
                }
            }
            poll(
                &mut peers,
                &root,
                [
                    Some(ReviewDecision::Approve),
                    Some(ReviewDecision::Approve),
                    (!hold).then_some(if approve {
                        ReviewDecision::Approve
                    } else {
                        ReviewDecision::Reject
                    }),
                ],
            );
            if mode == "partial" && root.0.join("partial.ready").exists() && stop_peer.is_none() {
                let reviewer = reviewer.clone();
                stop_peer = Some(thread::spawn(move || {
                    let mut client = reviewer.connect_stop_client(1).unwrap();
                    let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(15));
                        match client.step().unwrap() {
                            StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                            StopClientProgress::Complete => return client.receipt().unwrap(),
                            _ => crate::workflow::pause(1),
                        }
                    }
                }));
            }
            let now = crate::workflow::clock();
            if mode == "deadline" && root.0.join("partial.ready").exists() {
                plus(now, 60_001).unwrap()
            } else {
                now
            }
        })
        .unwrap();
    fs::write(
        root.0.join("service.finished"),
        b"same original resident owner ended",
    )
    .unwrap();
    child.finish();
    assert!(original_socket.is_some());
    assert_eq!(result.cleanup_pending, 0);
    assert_eq!(result.responses.len(), 3);
    let executed = !hold && approve;
    assert_eq!(result.failure.is_none(), executed, "{:?}", result.failure);
    assert_eq!(peers.seen_actor, [true; 3]);
    assert_eq!(peers.seen_human, [true, true, !hold]);
    let (responses, packets) = peers.join();
    for index in 0..3 {
        assert_eq!(
            responses[index].as_ref().unwrap().result,
            result.responses[index].result
        );
        assert_eq!(
            confirmed(responses[index].as_ref().unwrap()),
            index < 2 || executed
        );
    }
    let mut sessions = BTreeSet::new();
    for packet in packets.iter().flatten() {
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.views().len(), 2);
    }
    let snapshot = reader.snapshot();
    let stream = FileOversight::read_stream_publication_with_learned_generation(
        &reader.config.store,
        &reader.config.profile,
        &reader.loaded.generation,
    )
    .unwrap();
    assert_eq!(snapshot.executions, if executed { 3 } else { 2 });
    assert_eq!(snapshot.payload, b"AA");
    assert_eq!(snapshot.control.ledger.reserved, 0);
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert_eq!(stream.confirmed.finished(), executed);
    let anchor = archive::load(&root.0.join("resident.anchor")).unwrap();
    assert_eq!(anchor.revision(), snapshot.revision);
    if matches!(mode, "partial" | "deadline") {
        if mode == "partial" {
            let receipt: StopControlReceipt = stop_peer.unwrap().join().unwrap();
            assert!(receipt.acknowledged() && receipt.drained());
            assert!(snapshot.stop.is_some());
        } else {
            assert!(snapshot.stop.is_none());
            assert_eq!(snapshot.control.ledger.epoch, 0);
        }
        assert!(root.0.join("partial.closed").exists());
        assert!(!root.0.join("receipt-final.json").exists());
    } else {
        assert!(snapshot.stop.is_none());
        assert_eq!(snapshot.control.ledger.epoch, 0);
        if mode == "exhausted" {
            assert!(root.0.join("budget.exhausted").exists());
            assert!(!root.0.join("receipt-final.json").exists());
        } else {
            let receipt = read_json(&root.0.join("receipt-final.json"));
            assert!(anchor.revision() >= number(&receipt, &["after", "journal_revision"]));
            if mode == "both" {
                let first = read_json(&root.0.join("receipt-first.json"));
                assert!(
                    number(&receipt, &["after", "evaluation_revision"])
                        > number(&first, &["after", "evaluation_revision"])
                );
            }
        }
    }
    assert!(!selection.peer.socket_path().exists());
    assert!(!socket_path(&reviewer, 1).exists());
    quiescent(&root);
}

#[test]
fn original_evaluator_follows_messages_and_finish_without_replacing_its_socket_or_role() {
    run_sequence(false, true, "both", 4);
}

#[test]
fn final_resident_hold_and_human_rejection_remain_independently_assessable() {
    run_sequence(true, false, "final", 2);
    run_sequence(false, false, "final", 2);
}

#[test]
fn exhausted_evaluator_candidates_do_not_renew_at_a_learned_message_boundary() {
    run_sequence(false, true, "exhausted", 2);
}

#[test]
fn original_stop_preempts_a_partial_evaluator_frame_after_the_final_resident_hold() {
    run_sequence(true, false, "partial", 2);
}

#[test]
fn final_request_deadline_retires_pending_evaluation_without_a_fresh_lifetime() {
    run_sequence(true, false, "deadline", 2);
}

#[test]
fn actual_evaluated_resident_command_keeps_the_original_case_history_through_finish() {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    let reader = Reader::new(&root, &paths[0]);
    let mut child = EvaluatorChild::start(&root, "both");
    let selection = select_evaluation(&root, &config, &mut entries, child.0.id(), 4);
    let mut rows = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let inputs = super::super::mediated_topology::command_inputs(
            &root,
            &config,
            &paths[index],
            &entry.actor,
        );
        let actor = root
            .0
            .join(format!("scheduled-actor-{}.json", entry.loaded.request));
        fs::rename(&inputs[5], &actor).unwrap();
        rows.push(format!(
            r#"{{"recipe":{},"actor_profile":{},"release":"{}"}}"#,
            quote(paths[index].to_str().unwrap()),
            quote(actor.to_str().unwrap()),
            if entry.release == LearnedTextRelease::Finish {
                "finish"
            } else {
                "message"
            }
        ));
    }
    let manifest = root.0.join("sequence.json");
    fs::write(
        &manifest,
        format!(
            r#"{{"schema":"fa.learned-publication-sequence/1","entries":[{}]}}"#,
            rows.join(",")
        ),
    )
    .unwrap();
    let mut args = arguments(
        &root,
        &root.0.join("andon.json"),
        &root.0.join("evaluator.json"),
    );
    args[1] = root.0.join("command-host.json").display().to_string();
    args[2] = manifest.display().to_string();
    args[3] = root.0.join("command-reviewer.json").display().to_string();
    let mut peers = Peers::new(&root, &config, &entries);
    let outcome = resident::command_with_clock(&args, None, || {
        poll(&mut peers, &root, [Some(ReviewDecision::Approve); 3]);
        crate::workflow::clock()
    });
    fs::write(
        root.0.join("service.finished"),
        b"original argv workflow ended",
    )
    .unwrap();
    child.finish();
    assert!(outcome.is_ok(), "{outcome:?}");
    let (responses, packets) = peers.join();
    assert!(
        responses
            .iter()
            .all(|response| confirmed(response.as_ref().unwrap()))
    );
    assert!(packets.iter().all(Option::is_some));
    let snapshot = reader.snapshot();
    assert_eq!(snapshot.executions, 3);
    assert_eq!(snapshot.payload, b"AA");
    assert_eq!(snapshot.control.ledger.epoch, 0);
    let anchor = archive::load(&root.0.join("resident.anchor")).unwrap();
    let receipt = read_json(&root.0.join("receipt-final.json"));
    assert_eq!(snapshot.revision, anchor.revision());
    assert!(anchor.revision() >= number(&receipt, &["after", "journal_revision"]));
    assert!(!selection.peer.socket_path().exists());
    quiescent(&root);
}

#[test]
fn evaluated_resident_grammar_and_future_custody_refuse_before_owner_or_clock_work() {
    let root = Root::new();
    let valid = arguments(&root, Path::new("missing-andon"), Path::new("missing-peer"));
    assert!(selected_evaluation::take_sequence_options(&valid, None).is_ok());
    for option in [
        "--continue-generation",
        "--after-message",
        "--governor-profile",
        "--topology-profile",
        "--campaign-profile",
        "--history-anchor",
        "--checkpoint-before-generation",
        "--actor-profile",
    ] {
        let mut args = valid.clone();
        args.extend([option.into(), "unsupported".into()]);
        let error =
            resident::command_with_clock(&args, None, || panic!("unsupported mode sampled time"));
        assert!(error.is_err(), "accepted {option}");
        assert!(!root.0.join("store").exists());
    }
    for index in [4, 6, 8] {
        let mut args = valid.clone();
        args.drain(index..index + 2);
        assert!(selected_evaluation::take_sequence_options(&args, None).is_err());
    }
    assert!(
        selected_evaluation::take_sequence_options(&valid, Some(Path::new("capsule"))).is_err()
    );
    for overlap in 0..4 {
        let root = Root::new();
        let (config, _, mut entries) = selected(&root, None, None);
        let reviewer = fixture::peers(&root, &config);
        // A distinct, provisioned PID is sufficient for read-only admission;
        // no service or subprocess may be started on these rejected paths.
        let selection = select_evaluation(
            &root,
            &config,
            &mut entries,
            std::process::id().checked_add(100_000).unwrap(),
            2,
        );
        match overlap {
            0 => entries[2].actor.socket = selection.peer.socket_path().to_path_buf(),
            1 => entries[0].loaded.anchor_output = Some(entries[2].actor.socket.clone()),
            2 => entries[0].loaded.anchor_output = Some(selection.peer.socket_path().to_path_buf()),
            3 => entries[2].actor.actor = selection.peer.evaluator.policy().unwrap(),
            _ => unreachable!(),
        }
        assert!(
            resident::run_with_evaluation(config, entries, &reviewer, None, &selection, || panic!(
                "invalid future custody sampled time"
            ))
            .is_err()
        );
        assert!(!root.0.join("store").exists());
        assert!(!selection.peer.socket_path().exists());
    }
}
