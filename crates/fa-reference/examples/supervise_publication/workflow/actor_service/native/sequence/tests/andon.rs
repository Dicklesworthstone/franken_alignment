//! Native generation, independent labels and live Ledger disclosure share one
//! original owner. A separate process supplies the evaluator's kernel identity;
//! synthetic model weights and verdicts establish composition, not model quality.
use super::*;
use crate::workflow::actor_service::{andon_settings, evaluator};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::Verb;
use operator_tests::{connect, exchange, field, held_reviews, request};
use std::{
    fs,
    io::{BufRead, Cursor, Read},
    os::unix::fs::MetadataExt,
    path::Path,
    process::{Child, ChildStdin, Command as Process, Stdio},
    sync::mpsc,
};

const PEER_ROOT: &str = "FA_NATIVE_SEQUENCE_EVALUATOR_ROOT";
const PEER_TEST: &str = "workflow::actor_service::native::sequence::tests::andon_tests::native_resident_evaluator_process";

#[path = "andon/continuation.rs"]
mod continuation;
#[path = "andon/held_case.rs"]
mod held_case;
#[path = "andon/joint.rs"]
mod joint;
#[path = "andon/precheck.rs"]
mod precheck;
#[path = "andon/recovery.rs"]
mod recovered;
#[path = "andon/stop_failure.rs"]
mod stop_failure;

fn parse(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, strict_json::Limits::default()).unwrap()
}

pub(super) fn settings() -> andon_settings::Settings {
    andon_settings::Settings::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/native_andon_operator.json"
    )))
    .unwrap()
}

pub(super) fn peer_profile(root: &Root, actor: &Profile, pid: u32, candidates: u32) -> String {
    let scope = actor.scope;
    let server = actor.supervisor;
    format!(
        r#"{{"schema_version":"fa.native-evaluator-peer/1","socket":"{}/evaluator.sock",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "deployment":101,"evaluator":202,
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "peer":{{"uid":{},"gid":{},"pid":{pid}}},
        "candidate_limit":{candidates},"runtime_ms":15000,"poll_ms":1}}"#,
        root.0.display(),
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        server.uid(),
        server.gid(),
        server.pid().unwrap(),
        server.uid(),
        server.gid(),
    )
}

pub(super) fn history(
    root: &Root,
    config: &Config,
    entries: &[Entry],
    reviewer: &PeerProfile,
    operator: Option<&operator::Selection>,
) -> (anchors::Selection, PathBuf) {
    let output = root.0.join("original.anchor");
    let arguments = [
        "serve-native-generated".to_owned(),
        "config".to_owned(),
        "sequence".to_owned(),
        "reviewer".to_owned(),
        "--retain-history-anchor".to_owned(),
        output.display().to_string(),
    ];
    let (_, options) = anchors::take_options(&arguments).unwrap();
    let selected = options
        .admit(
            config,
            &entries
                .iter()
                .map(|entry| (&entry.actor, entry.precheck.as_ref()))
                .collect::<Vec<_>>(),
            reviewer,
            operator,
            None,
            None,
        )
        .unwrap();
    (selected, output)
}

fn ledger_operator(root: &Root, actor: &Profile) -> operator::Selection {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let selected = crate::workflow::actor_service::operator::Settings::decode(
        format!(
            r#"{{"schema":"fa.tower-operator-service/1",
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "socket":"{}/operator.sock",
        "supervisor":{{"uid":{},"gid":{},"pid":{}}},
        "operator":{{"uid":{},"gid":{},"pid":{}}},
        "candidate_limit":2,"connection_limit":2,"exchange_limit":16,
        "runtime_ms":15000,"poll_ms":1,"inspection_queries":0,
        "explanation_queries":0,"explanation_replay_events":0,"explanation_disclosure":"none",
        "governance_ledger":{{"queries":6,"disclosure":"recorded_governance_accounting"}}}}"#,
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            root.0.display(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
            identity.uid(),
            identity.gid(),
            identity.pid().unwrap(),
        )
        .as_bytes(),
    )
    .unwrap();
    operator::Selection::new(selected, 41).unwrap()
}

fn ledger(reply: &Json, retained: u64, pending: u64, labels: u64) {
    assert_eq!(
        field(reply, &["result", "kind"]).as_str(),
        Some("governance-ledger")
    );
    let known = field(reply, &["result", "value", "knowledge"]);
    assert_eq!(field(known, &["state"]).as_str(), Some("known"));
    assert_eq!(
        field(known, &["basis", "origin"]).as_str(),
        Some("acknowledged_owner")
    );
    let value = field(known, &["value"]);
    assert_eq!(
        field(value, &["credibility", "state"]).as_str(),
        Some("known")
    );
    for (name, count) in [
        ("retained_cases", retained),
        ("pending_cases", pending),
        ("benign_origins", labels),
        ("lifetime_false_stops", 0),
    ] {
        assert_eq!(
            field(value, &["credibility", "value", name])
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap(),
            count,
            "{name}: {reply:?}"
        );
    }
    assert_eq!(field(value, &["andon", "state"]).as_str(), Some("known"));
    let windows = field(value, &["andon", "value", "windows"])
        .as_array()
        .unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(field(&windows[0], &["total_cost"]).as_str(), Some("0"));
}

pub(super) struct EvaluatorProcess {
    child: Option<Child>,
    input: Option<ChildStdin>,
    root: PathBuf,
    profile: PathBuf,
    pub(super) selected: evaluator::Settings,
    serial: u64,
}

impl EvaluatorProcess {
    pub(super) fn start(root: &Root, actor: &Profile, candidates: u32) -> Self {
        let mut child = Process::new(std::env::current_exe().unwrap())
            .args(["--exact", PEER_TEST, "--nocapture", "--test-threads=1"])
            .env(PEER_ROOT, &root.0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        assert_ne!(Some(child.id()), actor.actor.pid());
        let bytes = peer_profile(root, actor, child.id(), candidates);
        let profile = root.0.join("evaluator.json");
        fs::write(&profile, bytes.as_bytes()).unwrap();
        Self {
            input: child.stdin.take(),
            child: Some(child),
            root: root.0.clone(),
            profile,
            selected: evaluator::Settings::decode(bytes.as_bytes()).unwrap(),
            serial: 0,
        }
    }

    pub(super) fn operation(&mut self, mode: &str, round: u64) -> (bool, Vec<u8>) {
        self.begin(mode, round);
        self.completion()
    }

    fn begin(&mut self, mode: &str, round: u64) {
        self.serial += 1;
        writeln!(
            self.input.as_mut().unwrap(),
            "{} {mode} {round}",
            self.serial
        )
        .unwrap();
        self.input.as_mut().unwrap().flush().unwrap();
    }

    fn queue(&mut self, wire: &[u8]) {
        fs::write(self.root.join("queued-assessment.wire"), wire).unwrap();
        self.begin("queued", 101);
        let queued = self.root.join(format!("evaluator-{}.queued", self.serial));
        self.wait_for(&queued);
    }

    fn wait_for(&mut self, path: &Path) {
        let started = Instant::now();
        while !path.exists() {
            assert!(started.elapsed() < Duration::from_secs(20));
            assert!(self.child.as_mut().unwrap().try_wait().unwrap().is_none());
            pause(1);
        }
    }

    fn completion(&mut self) -> (bool, Vec<u8>) {
        let done = self.root.join(format!("evaluator-{}.done", self.serial));
        self.wait_for(&done);
        let result = fs::read_to_string(done).unwrap();
        let bytes = fs::read(self.root.join(format!("evaluator-{}.reply", self.serial))).unwrap();
        (result == "ok", bytes)
    }

    pub(super) fn finish(mut self) {
        drop(self.input.take());
        let result = self.child.take().unwrap().wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            String::from_utf8_lossy(&result.stdout).contains("1 passed"),
            "the evaluator helper must actually execute its selected test entry"
        );
    }
}

impl Drop for EvaluatorProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            drop(self.input.take());
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

fn send(profile: &Path, verb: &str, argument: String, output: &mut Vec<u8>) -> Result<(), String> {
    crate::workflow::actor_service::evaluator_command(
        &[verb.to_owned(), profile.display().to_string(), argument],
        output,
    )
}

/// Entry point for an actual separate kernel peer. It performs only public
/// command transport; no native owner, fixture host or evaluator role is shared.
#[test]
fn native_resident_evaluator_process() {
    let Some(root) = std::env::var_os(PEER_ROOT).map(PathBuf::from) else {
        return;
    };
    let profile = root.join("evaluator.json");
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let words = line.split_whitespace().collect::<Vec<_>>();
        assert_eq!(words.len(), 3);
        let serial = words[0].parse::<u64>().unwrap();
        let mode = words[1];
        let round = words[2].parse::<u64>().unwrap();
        let mut response = Vec::new();
        let result = (|| -> Result<(), String> {
            if mode == "queued" {
                // Seal a real assessment before the caller enters its original
                // control checkpoint. The marker records socket I/O, not a
                // claimed native label or an invented successful response.
                let selected = evaluator::Settings::read(&profile)?;
                let mut socket = UnixStream::connect(selected.socket_path()).map_err(debug)?;
                selected
                    .supervisor
                    .policy()?
                    .verify(&socket)
                    .map_err(debug)?;
                socket
                    .set_read_timeout(Some(Duration::from_secs(20)))
                    .map_err(debug)?;
                socket
                    .set_write_timeout(Some(Duration::from_secs(20)))
                    .map_err(debug)?;
                socket
                    .write_all(&fs::read(root.join("queued-assessment.wire")).map_err(debug)?)
                    .map_err(debug)?;
                socket.shutdown(std::net::Shutdown::Write).map_err(debug)?;
                fs::write(root.join(format!("evaluator-{serial}.queued")), b"sealed")
                    .map_err(debug)?;
                socket.read_to_end(&mut response).map_err(debug)?;
                if response.is_empty() {
                    return Err("original evaluator closed without an assessment receipt".into());
                }
                return match field(&parse(&response), &["status"]).as_str() {
                    Some("applied" | "already_applied") => Ok(()),
                    _ => Err("original evaluator did not acknowledge this assessment".into()),
                };
            }
            let selected = if mode == "wrong-audience" {
                let path = root.join("wrong-evaluator.json");
                fs::write(
                    &path,
                    fs::read_to_string(&profile)
                        .unwrap()
                        .replace("\"evaluator\":202", "\"evaluator\":203"),
                )
                .unwrap();
                path
            } else {
                profile.clone()
            };
            send(
                &selected,
                "evaluator-case",
                round.to_string(),
                &mut response,
            )?;
            if mode != "label" {
                return Ok(());
            }
            let case = parse(&response);
            assert_eq!(field(&case, &["effect_authority"]).as_bool(), Some(false));
            assert_eq!(
                field(&case, &["knowledge", "state"]).as_str(),
                Some("known")
            );
            let value = field(&case, &["knowledge", "value"]);
            let number = |name| {
                field(value, &[name])
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
            };
            let revision = field(&case, &["knowledge", "basis", "evaluation_revision"])
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();
            assert_eq!(number("round"), round);
            let assessment = format!(
                r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},"policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{}","expected_evaluation_revision":{revision},"origin":{round},"evidence_id_hex":"{}","truth":"benign"}}"#,
                number("round"),
                number("attempt"),
                number("policy_generation"),
                number("congress_generation"),
                field(value, &["case_evidence_root_hex"]).as_str().unwrap(),
                "13".repeat(32),
            );
            let path = root.join(format!("assessment-{serial}.json"));
            fs::write(&path, assessment).unwrap();
            fs::write(root.join(format!("case-{serial}.json")), &response).unwrap();
            response.clear();
            send(
                &profile,
                "evaluator-assess",
                path.display().to_string(),
                &mut response,
            )
        })();
        fs::write(root.join(format!("evaluator-{serial}.reply")), &response).unwrap();
        let pending = root.join(format!("evaluator-{serial}.pending"));
        fs::write(
            &pending,
            match result {
                Ok(()) => "ok".to_owned(),
                Err(error) => format!("error: {error}"),
            },
        )
        .unwrap();
        fs::rename(pending, root.join(format!("evaluator-{serial}.done"))).unwrap();
    }
}

pub(super) fn retained(path: &Path, reader: &Reader) {
    let anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    assert_eq!(anchor.revision(), reader.publication().revision);
}

#[test]
fn resident_native_evaluation_keeps_original_cases_and_candidate_limits_across_finish() {
    // Two rejected peers + two case/label pairs consume six original candidates.
    // A larger original allowance permits Finish's pair; a handoff cannot add it.
    for candidates in [6, 8] {
        let root = Root::new();
        let (config, entries, reviewer) = selected(&root, true);
        let reader = Reader::new(&config, &entries[0]);
        let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, candidates);
        let evaluator = peer.selected.clone();
        let observer = ledger_operator(&root, &entries[0].actor);
        let (history, anchor) = history(&root, &config, &entries, &reviewer, Some(&observer));
        let observer_settings = observer.settings.clone();
        let actor = entries[0].actor.clone();
        let store = reader.store.clone();
        let scope = reader.profile.clone();
        let (offered, offers) = mpsc::channel();
        let (release, decisions) = mpsc::channel();
        let human = held_reviews(reviewer.clone(), REQUESTS.to_vec(), offered, decisions);
        let independent = thread::spawn(move || {
            let mut socket = connect(&observer_settings);
            let mut original_inode = None;
            for (index, request_id) in REQUESTS.into_iter().enumerate() {
                let (offered, _) = offers.recv_timeout(Duration::from_secs(20)).unwrap();
                assert_eq!(offered, request_id);
                if index == 0 {
                    // The operator starts first. A real human offer proves
                    // the independent evaluator startup has also completed.
                    original_inode = Some(fs::metadata(peer.selected.socket_path()).unwrap().ino());
                }
                assert_eq!(
                    FileOversight::read_publication(&store, &scope)
                        .unwrap()
                        .executions,
                    index as u64,
                    "neither a label nor Ledger observation is the missing human key"
                );
                let before = exchange(
                    &mut socket,
                    &request(&actor, &format!("before-{index}"), Verb::Ledger),
                    observer_settings.response_bytes,
                );
                ledger(&before, index as u64 + 1, 1, index as u64);
                if index == 0 {
                    let mut untrusted = Vec::new();
                    assert!(
                        send(
                            &peer.profile,
                            "evaluator-case",
                            request_id.to_string(),
                            &mut untrusted
                        )
                        .is_err()
                    );
                    assert!(
                        untrusted.is_empty(),
                        "the actor's kernel identity must not receive a case"
                    );
                    let (accepted, wrong) = peer.operation("wrong-audience", request_id);
                    assert!(
                        !accepted && wrong.is_empty(),
                        "a different evaluator audience must not receive a case"
                    );
                }
                if index < 2 || candidates == 8 {
                    assert_eq!(
                        fs::metadata(peer.selected.socket_path()).unwrap().ino(),
                        original_inode.unwrap(),
                        "every entry must keep the original evaluator listener and role"
                    );
                    let (accepted, receipt) = peer.operation("label", request_id);
                    assert!(
                        accepted,
                        "actual independent case/label transport failed: {}",
                        String::from_utf8_lossy(&receipt)
                    );
                    assert_eq!(
                        field(&parse(&receipt), &["status"]).as_str(),
                        Some("applied")
                    );
                } else {
                    let started = Instant::now();
                    while peer.selected.socket_path().exists() {
                        assert!(started.elapsed() < Duration::from_secs(20));
                        pause(1);
                    }
                    let (accepted, receipt) = peer.operation("read", request_id);
                    assert!(
                        !accepted && receipt.is_empty(),
                        "a third entry cannot replenish the exhausted original evaluator"
                    );
                }
                let labeled = index < 2 || candidates == 8;
                let after = exchange(
                    &mut socket,
                    &request(&actor, &format!("after-{index}"), Verb::Ledger),
                    observer_settings.response_bytes,
                );
                ledger(
                    &after,
                    index as u64 + 1,
                    u64::from(!labeled),
                    index as u64 + u64::from(labeled),
                );
                assert_eq!(
                    FileOversight::read_publication(&store, &scope)
                        .unwrap()
                        .executions,
                    index as u64
                );
                release.send(Some(ReviewDecision::Approve)).unwrap();
            }
            peer.finish();
        });
        let mut output = Output::new(&entries);
        let profiles = output.profiles.clone();
        let result = run_with_andon(
            config,
            entries,
            &reviewer,
            None,
            Some(&observer),
            &history,
            &settings(),
            &evaluator,
            crate::workflow::clock,
            &mut output,
        );
        assert!(result.is_ok(), "{result:?}");
        independent.join().unwrap();
        let packets = human.join().unwrap();
        let replies = output.join();
        assert_eq!(packets.len(), 3);
        assert_eq!(replies.len(), 3);
        assert!(replies.iter().all(executed));
        let mut sessions = BTreeSet::new();
        for (index, packet) in packets.iter().enumerate() {
            assert!(sessions.insert(packet.binding().session));
            assert_eq!(packet.views().len(), 2);
            let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
            assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
            assert_eq!(frame.is_finish(), index == 2);
        }
        let stream = reader.stream();
        assert_eq!(stream.publication.executions, 3);
        assert_eq!(stream.publication.payload, b"AA");
        assert_eq!(stream.publication.control.ledger.epoch, 0);
        assert_eq!(stream.publication.control.ledger.reserved, 0);
        assert_eq!(
            stream.publication.control.ledger.charged,
            packets
                .iter()
                .map(|packet| packet.action().spec().units)
                .sum::<u64>()
        );
        assert!(stream.publication.stop.is_none());
        assert_eq!(stream.confirmed, stream.published);
        assert!(stream.confirmed.finished());
        let progress = reader.progress(8);
        assert_eq!(progress.numerical.numerical.position, 8);
        assert_eq!(progress.numerical.numerical.sampled_draws, 4);
        reader.no_generation(9);
        retained(&anchor, &reader);
        assert!(!evaluator.socket_path().exists());
        assert!(!observer.settings.socket.exists());
        closed(&profiles, &reviewer);
    }
}

#[test]
fn native_evaluation_stop_and_original_deadline_preserve_the_confirmed_prefix() {
    for expire in [false, true] {
        let root = Root::new();
        let (config, entries, reviewer) = selected(&root, true);
        let reader = Reader::new(&config, &entries[0]);
        let mut peer = EvaluatorProcess::start(&root, &entries[0].actor, 8);
        let evaluator = peer.selected.clone();
        let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
        let mut output = Output::new(&entries);
        let profiles = output.profiles.clone();
        let mut humans = Humans::new(
            &reviewer,
            &reader,
            [Some(ReviewDecision::Approve), None, None],
        );
        let start = crate::workflow::clock();
        let mut expired = false;
        let mut controller = None;
        let result = run_with_andon(
            config,
            entries,
            &reviewer,
            None,
            None,
            &history,
            &settings(),
            &evaluator,
            || {
                humans.poll();
                if expire
                    && !expired
                    && !profiles[0].socket.exists()
                    && reader.store.join("delivery.bin").exists()
                    && reader.publication().executions == 1
                {
                    // The first reply window and actor connection have
                    // retired. Expire at handoff before any second reference
                    // is emitted, so transport loss cannot masquerade as a
                    // required terminal actor response.
                    assert!(evaluator.socket_path().exists());
                    expired = true;
                }
                if !expire && reviewer.socket(REQUESTS[1]).exists() && controller.is_none() {
                    assert_eq!(reader.publication().executions, 1);
                    assert!(evaluator.socket_path().exists());
                    assert!(crate::workflow::control::socket_path(&reviewer, REQUESTS[0]).exists());
                    assert!(
                        !crate::workflow::control::socket_path(&reviewer, REQUESTS[1]).exists()
                    );
                    controller = Some(stopping(reviewer.clone()));
                }
                if expired {
                    ElapsedTick(start.0 + 60_000)
                } else {
                    crate::workflow::clock()
                }
            },
            &mut output,
        );
        assert!(result.is_err());
        if expire {
            assert!(expired);
        } else {
            let stop = controller
                .expect("the second native human boundary must be reached")
                .join()
                .unwrap();
            assert!(stop.acknowledged() && stop.drained());
            assert_eq!(stop.binding.operation, REQUESTS[0]);
        }
        let packets = humans.join();
        let replies = output.join();
        assert_eq!(replies.len(), if expire { 1 } else { 2 });
        assert!(executed(&replies[0]));
        if expire {
            assert!(packets[1..].iter().all(Option::is_none));
            reader.no_generation(8);
        } else {
            assert!(!executed(&replies[1]));
        }
        let stream = reader.stream();
        assert_eq!(stream.publication.executions, 1);
        assert_eq!(stream.publication.payload, b"A");
        assert_eq!(stream.publication.control.ledger.reserved, 0);
        assert_eq!(
            stream.publication.control.ledger.charged,
            packets[0].as_ref().unwrap().action().spec().units
        );
        assert!(stream.publication.stop.is_some());
        assert!(!stream.confirmed.finished());
        reader.no_generation(9);
        retained(&anchor, &reader);
        assert!(!evaluator.socket_path().exists());
        let (accepted, bytes) = peer.operation("read", REQUESTS[0]);
        assert!(
            !accepted && bytes.is_empty(),
            "terminal cleanup cannot leave evaluator intake alive"
        );
        peer.finish();
        closed(&profiles, &reviewer);
    }
}

#[test]
fn native_evaluation_rejects_foreign_roles_and_future_endpoint_aliases_before_create() {
    for case in [
        "scope",
        "supervisor",
        "actor",
        "audience",
        "future_actor",
        "anchor",
        "clock",
    ] {
        let root = Root::new();
        let (mut config, entries, reviewer) = selected(&root, true);
        let actor = &entries[0].actor;
        let mut evaluator = evaluator::Settings::decode(
            peer_profile(
                &root,
                actor,
                actor.supervisor.pid().unwrap().checked_add(1).unwrap(),
                8,
            )
            .as_bytes(),
        )
        .unwrap();
        let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
        match case {
            "scope" => evaluator.scope.branch += 1,
            "supervisor" => evaluator.supervisor.pid = evaluator.evaluator.pid,
            "actor" => evaluator.evaluator.pid = actor.actor.pid(),
            "audience" => evaluator.evaluator_id += 1,
            "future_actor" => evaluator.socket = entries[2].actor.socket.clone(),
            "anchor" => evaluator.socket = anchor.clone(),
            "clock" => config.profile.delivery.clock_domain += 1,
            _ => unreachable!(),
        }
        let store = config.store.clone();
        let profiles = entries
            .iter()
            .map(|entry| entry.actor.clone())
            .collect::<Vec<_>>();
        let mut clocks = 0;
        let mut output = Vec::new();
        let result = run_with_andon(
            config,
            entries,
            &reviewer,
            None,
            None,
            &history,
            &settings(),
            &evaluator,
            || {
                clocks += 1;
                crate::workflow::clock()
            },
            &mut output,
        );
        assert!(result.is_err(), "accepted {case}");
        assert_eq!(clocks, 0, "{case}");
        assert!(output.is_empty());
        assert!(!store.exists() && !anchor.exists());
        assert!(!evaluator.socket_path().exists());
        closed(&profiles, &reviewer);
    }
}

#[test]
fn new_second_frame_witness_conflict_retains_first_accounted_publication() {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, true);
    let reader = Reader::new(&config, &entries[0]);
    let (publication, mut producer) = checked::publication(&root, &reader);
    let evaluator = evaluator::Settings::decode(
        peer_profile(&root, &entries[0].actor, std::process::id().checked_add(100_000).unwrap(), 1).as_bytes(),
    ).unwrap();
    let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
    let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
    let mut output = Output::new(&entries);
    let mut first_charge = None;
    let mut changed = false;
    let result = run_with_andon(
        config, entries, &reviewer, Some(&publication), None, &history, &settings(), &evaluator,
        || {
            humans.poll();
            if !changed
                && let Ok(image) = FileOversight::read_publication(&reader.store, &reader.profile)
                && image.executions == 1
                && image.control.ledger.charged > *first_charge.get_or_insert(image.control.ledger.charged)
            {
                // After both human approvals, invalidate the second frame's
                // reviewed absence. The same evaluator is still resident.
                assert!(evaluator.socket_path().exists());
                producer.publish(1, checked::witness_inputs(&reader, 2, &[0, 1]), ElapsedTick(1001)).unwrap();
                changed = true;
            }
            ElapsedTick(if changed { 1001 } else { 1000 })
        },
        &mut output,
    );
    assert!(changed && result.is_err());
    assert_eq!(output.documents.len(), 2);
    assert!(!humans.seen[2]);
    let packets = humans.join();
    let replies = output.join();
    assert!(executed(&replies[0]) && !executed(&replies[1]));
    let stream = reader.stream();
    assert_eq!(stream.publication.executions, 1);
    assert_eq!(stream.publication.payload, b"A");
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(stream.publication.control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
    assert_eq!(stream.confirmed, stream.published);
    assert!(!stream.confirmed.finished());
    reader.no_generation(9);
    retained(&anchor, &reader);
    assert!(!evaluator.socket_path().exists());
}

#[test]
fn evaluated_resident_witness_paths_cannot_consume_a_later_role_or_authority_file() {
    for role in ["later actor", "later human", "later Stop", "store", "source", "archive"] {
        let root = Root::new();
        let (config, entries, reviewer) = selected(&root, true);
        let actor = &entries[0].actor;
        let reader = Reader::new(&config, &entries[0]);
        let last = entries.last().unwrap();
        let evaluator = evaluator::Settings::decode(
            peer_profile(&root, actor, actor.supervisor.pid().unwrap().checked_add(1).unwrap(), 2).as_bytes(),
        ).unwrap();
        let (history, anchor) = history(&root, &config, &entries, &reviewer, None);
        let collision = match role {
            "later actor" => last.actor.socket.join("producer"),
            "later human" => reviewer.socket(last.actor.request),
            "later Stop" => crate::workflow::control::socket_path(&reviewer, last.actor.request),
            "store" => config.store.join("delivery.bin"),
            "source" => config.source.path().to_path_buf(),
            "archive" => anchor.join("producer"),
            _ => unreachable!(),
        };
        let publication = PublicationProfile::decode(
            checked::witness_document(&collision, &reader, true, 120_000).as_bytes(),
        ).unwrap();
        let store = config.store.clone();
        let source = fs::read(config.source.path()).unwrap();
        let source_path = config.source.path().to_path_buf();
        let mut output = Vec::new();
        assert!(run_with_andon(
            config, entries, &reviewer, Some(&publication), None, &history, &settings(),
            &evaluator, || panic!("witness alias reached startup"), &mut output,
        ).is_err(), "admitted {role} alias");
        assert!(output.is_empty());
        assert!(!store.exists() && !anchor.exists());
        assert_eq!(fs::read(source_path).unwrap(), source);
        assert!(!evaluator.socket_path().exists());
    }
}

#[test]
fn native_evaluation_unsupported_modes_refuse_before_reading_profiles() {
    let base = [
        "serve-native-generated",
        "missing-config",
        "missing-sequence",
        "missing-reviewer",
        "--andon",
        "missing-andon",
        "--evaluator-profile",
        "missing-evaluator",
        "--retain-history-anchor",
        "/must-not-create.anchor",
    ];
    let variants = [
        vec!["--after-message", "91"],
        vec!["--history-anchor", "missing-anchor"],
        vec!["--governor-profile", "missing-governor"],
        vec!["--topology-profile", "missing-topology"],
        vec!["--precheck-profile", "missing-precheck"],
        vec!["--precheck-schedule", "missing-schedule"],
        vec!["--andon", "duplicate"],
        vec!["--operator-profile", "missing-operator"],
    ];
    for extra in variants {
        let args = base
            .into_iter()
            .chain(extra)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut output = Vec::new();
        let error = profile::command_with_clock(
            &args,
            None,
            || panic!("invalid selection observed time"),
            &mut output,
        )
        .unwrap_err();
        assert_eq!(error, andon::INVALID);
        assert!(output.is_empty());
    }
    for missing in ["--andon", "--evaluator-profile", "--retain-history-anchor"] {
        let args = base[..4]
            .iter()
            .copied()
            .chain(
                base[4..]
                    .chunks_exact(2)
                    .filter(|pair| pair[0] != missing)
                    .flatten()
                    .copied(),
            )
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            profile::command_with_clock(
                &args,
                None,
                || panic!("incomplete selection observed time"),
                &mut Vec::new()
            )
            .unwrap_err(),
            andon::INVALID
        );
    }
    assert_eq!(
        profile::command_with_clock(
            &base.map(str::to_owned),
            Some(Path::new("missing-activation")),
            || panic!("unsupported activation observed time"),
            &mut Vec::new()
        )
        .unwrap_err(),
        andon::INVALID
    );
}
