//! Ordinary Tower recovery services the original authenticated Stop before
//! reconciliation, live source admission or actor custody. Synthetic helpers
//! establish the published history; they do not qualify evaluator quality.
use super::*;
use crate::recover_stop::guarded::SubmitProfile;
use crate::workflow::continuation::guarded::mediated::profile::Selection as RecoverySelection;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::{
        FileGuardSet, FileRecoveryRequirements,
        mediated::FileMediatedRequirements,
        recovery::{FileGuardedRecoveryStatus, FilePublicationRecoveryRequirements},
    },
    reviewer::{
        client::ReviewerExpectation,
        peer::control::{
            StopBinding, StopClient, StopClientPhase, StopClientProgress, StopControlReceipt,
        },
    },
    shutdown::recovery::FileAdmissionStoppedHistory,
};
use std::{
    net::Shutdown,
    sync::{Arc, atomic::AtomicBool},
    thread::{self, JoinHandle},
};

const OPERATION: u64 = 8;

struct Case {
    root: Root,
    config: Config,
    peers: PeerProfile,
    saved: TopologyCheckpoint,
    checked: bool,
}

fn prepared(checked: bool) -> Case {
    let root = Root::new();
    let (config, peers, saved) = seed(&root, checked, 32);
    actor_profile(&root, OPERATION);
    // Recovery Stop needs historical configuration and retained custody only.
    // Neither a new source, current evidence nor publication capture is usable.
    for name in [
        "topology.json",
        "inventory.json",
        "evidence.json",
        "original.bin",
        "current.bin",
    ] {
        let path = root.path(name);
        if path.exists() {
            fs::remove_file(path).unwrap();
        }
    }
    fs::write(
        config.store.join("delivery.pending"),
        b"retain original recovery staging",
    )
    .unwrap();
    Case {
        root,
        config,
        peers,
        saved,
        checked,
    }
}

fn files(case: &Case) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&case.config.store)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

// The original clock-free native consumer supplies both receipt and exact byte
// oracle. Restore only this test fixture's known preimage before the live case;
// no event, receipt or source-work value is fabricated by the network adapter.
fn oracle(case: &Case) -> (FileAdmissionStoppedHistory, Vec<u8>) {
    let original = files(case);
    let profile = SubmitProfile::read(&case.root.path("guarded.json")).unwrap();
    let custody = profile.admit(&case.config).unwrap();
    let topology = RecoverySelection::read(&case.root.path("continuation.json")).unwrap();
    let history = topology.history(&case.config, &mut || Ok(())).unwrap();
    let expected = FileMediatedRequirements {
        oversight: FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: None,
                decoder: None,
                decoder_stop: None,
                source: Some(case.config.source_policy),
                identity: None,
                campaigns: None,
                credential: None,
            },
            effective_policy: case.config.profile.delivery.policy.clone(),
            credential_epoch: None,
            minimum: history
                .minimum(custody.anchor(), profile.minimum())
                .unwrap(),
        },
        topology: history.requirement,
        prediction: None,
        evaluation: None,
    };
    let validation = if case.checked {
        Some(
            PublicationProfile::read(&case.root.path("witness.json"))
                .unwrap()
                .simple_limits()
                .unwrap(),
        )
    } else {
        None
    };
    let mut recovery = FileOversight::begin_open_mediated_guarded_anchored(
        &case.config.store,
        case.config.profile.clone(),
        &expected,
        custody.anchor(),
    )
    .unwrap()
    .require_recovery_reserve(RecoveryReserve::terminal())
    .unwrap()
    .require_publication_profile(FilePublicationRecoveryRequirements {
        validation,
        feed: None,
        joint: None,
    })
    .unwrap();
    while recovery.progress().status == FileGuardedRecoveryStatus::Replaying {
        recovery
            .advance(recovery.progress().replayed_events, 1)
            .unwrap();
    }
    assert_eq!(recovery.progress().status, FileGuardedRecoveryStatus::Ready);
    let request = recovery.stop_request(OPERATION).unwrap();
    let stopped = recovery.finish_admission_stop(request).unwrap();
    assert_eq!(stopped.appended_events(), 2);
    let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
    for (path, bytes) in &original {
        fs::write(path, bytes).unwrap();
    }
    assert_eq!(files(case), original);
    (stopped, canonical)
}

#[derive(Debug)]
struct ClientResult {
    binding: Option<StopBinding>,
    receipt: Option<StopControlReceipt>,
    outcome_unknown: bool,
    failure: Option<String>,
}

fn client(mut client: StopClient<UnixStream>, lose_reply: bool) -> JoinHandle<ClientResult> {
    thread::spawn(move || {
        let started = Instant::now();
        let failure = loop {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "ordinary recovery Stop client stalled"
            );
            match client.step() {
                Ok(StopClientProgress::NeedsDecision) => client.request_stop().unwrap(),
                Ok(StopClientProgress::Complete) => break None,
                Ok(_) => {}
                Err(error) => break Some(format!("{error:?}")),
            }
            if lose_reply && client.phase() == StopClientPhase::AwaitingReceipt {
                break None;
            }
            pause(1);
        };
        ClientResult {
            binding: client.binding(),
            receipt: client.receipt(),
            outcome_unknown: client.outcome_unknown(),
            failure,
        }
    })
}

fn run(
    case: &Case,
    args: &[String],
    lose_reply: bool,
    mut time: impl FnMut() -> ElapsedTick,
) -> (String, ClientResult) {
    let mut calls = 0;
    let mut peer = None;
    let result = command_with_clock(args, None, || {
        calls += 1;
        if calls == 2 {
            // The first clock establishes the workflow budget. The next safe
            // checkpoint follows listener creation, before original replay.
            peer = Some(client(
                case.peers.connect_stop_client(OPERATION).unwrap(),
                lose_reply,
            ));
        }
        time()
    });
    let result = result.expect_err("an explicit recovery Stop cannot return live custody");
    (
        result,
        peer.expect("original recovery admission checkpoint")
            .join()
            .unwrap(),
    )
}

fn retained_anchor(case: &Case) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(case.root.path("second.anchor")).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap()
}

fn assert_original_stop(case: &Case, stopped: &FileAdmissionStoppedHistory, canonical: &[u8]) {
    assert_eq!(
        fs::read(case.config.store.join("delivery.bin")).unwrap(),
        canonical
    );
    let state = FileOversight::read_publication(&case.config.store, &case.config.profile).unwrap();
    assert_eq!(&state, stopped.snapshot());
    assert_eq!(state.revision, case.saved.history_anchor().revision() + 2);
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"network");
    assert_eq!(
        (state.control.ledger.charged, state.control.ledger.reserved),
        (7, 0)
    );
    assert_eq!(state.control.ledger.stages.len(), 1);
    assert_eq!(state.stop.unwrap().request().operation, OPERATION);
    assert!(!case.config.store.join("delivery.pending").exists());
    assert_eq!(
        read_checkpoint(&case.root, "first-checkpoint.bin"),
        case.saved
    );
    for name in [
        "topology.json",
        "inventory.json",
        "evidence.json",
        "original.bin",
        "current.bin",
    ] {
        assert!(!case.root.path(name).exists(), "no fresh input: {name}");
    }
    assert!(!case.root.path("actors/actor.sock").exists());
    assert!(!case.peers.socket(OPERATION).exists());
    assert!(!crate::workflow::control::socket_path(&case.peers, OPERATION).exists());
}

fn assert_source_retained(case: &Case, stopped: &FileAdmissionStoppedHistory) {
    let expected = case
        .saved
        .after_admission_stop(
            &case.config.profile,
            &fs::canonicalize(&case.config.store).unwrap(),
            stopped,
        )
        .unwrap();
    let retained = read_checkpoint(&case.root, "second-checkpoint.bin");
    assert_eq!(retained, expected);
    assert_eq!(retained.work(), case.saved.work());
    assert_eq!(retained.last_operation(), case.saved.last_operation());
    assert!(!retained.available());
    assert_eq!(retained.history_anchor(), stopped.history_anchor());
}

#[test]
fn plain_checked_and_scheduled_recovery_stop_match_original_native_bytes_before_live_admission() {
    for (checked, sequence) in [(false, false), (true, false), (true, true)] {
        let case = prepared(checked);
        let (stopped, canonical) = oracle(&case);
        let mut args = arguments(&case.root, checked, true, false);
        if sequence {
            args.extend(["--requests".into(), "8,9".into()]);
        }
        let (terminal, client) = run(&case, &args, false, || ElapsedTick(1001));
        assert!(
            terminal.contains("native admission Stop acknowledged"),
            "{terminal}"
        );
        assert!(client.failure.is_none(), "{client:?}");
        let receipt = client.receipt.unwrap();
        assert!(receipt.acknowledged());
        assert_eq!(Some(receipt.binding), client.binding);
        assert_eq!(receipt.binding.operation, OPERATION);
        assert_ne!(receipt.binding.session, [0; 32]);
        assert!(!client.outcome_unknown);
        assert_original_stop(&case, &stopped, &canonical);
        assert_eq!(&retained_anchor(&case), stopped.history_anchor());
        assert_source_retained(&case, &stopped);
        assert!(!crate::workflow::control::socket_path(&case.peers, 9).exists());
    }
}

#[derive(Clone, Copy, Debug)]
enum Attack {
    Silent,
    Nonce,
    Operation,
    Truncated,
}

fn malformed(
    mut stream: UnixStream,
    expected: ReviewerExpectation,
    attack: Attack,
    offered: Arc<AtomicBool>,
) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut offer = [0; 104];
        stream.read_exact(&mut offer).unwrap();
        assert_eq!(&offer[..8], b"FASTOFF1");
        // The malformed test client checks the independently selected audience
        // before corrupting only one original request component.
        let scope = expected.scope;
        for (index, word) in [
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            expected.clock_domain,
            expected.reviewer,
            OPERATION,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(&offer[8 + index * 8..16 + index * 8], &word.to_be_bytes());
        }
        assert_ne!(&offer[72..], &[0; 32]);
        offered.store(true, Ordering::SeqCst);
        if !matches!(attack, Attack::Silent) {
            let mut request = offer;
            request[..8].copy_from_slice(b"FASTREQ1");
            match attack {
                Attack::Nonce => request[103] ^= 1,
                Attack::Operation => request[71] ^= 1,
                Attack::Silent | Attack::Truncated => {}
            }
            let count = if matches!(attack, Attack::Truncated) {
                103
            } else {
                104
            };
            stream.write_all(&request[..count]).unwrap();
            stream.shutdown(Shutdown::Write).unwrap();
        }
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).unwrap();
        reply
    })
}

#[test]
fn silent_and_inexact_authenticated_requests_preserve_original_journal_and_staging() {
    let case = prepared(true);
    let original = files(&case);
    let actor = Profile::read(&case.root.path("actor.json")).unwrap();
    for attack in [
        Attack::Silent,
        Attack::Nonce,
        Attack::Operation,
        Attack::Truncated,
    ] {
        let offered = Arc::new(AtomicBool::new(false));
        let mut calls = 0;
        let mut peer = None;
        let error = command_with_clock(&arguments(&case.root, true, true, false), None, || {
            calls += 1;
            if calls == 2 {
                let stream = UnixStream::connect(crate::workflow::control::socket_path(
                    &case.peers,
                    OPERATION,
                ))
                .unwrap();
                actor.supervisor.verify(&stream).unwrap();
                peer = Some(malformed(
                    stream,
                    case.peers.expected,
                    attack,
                    Arc::clone(&offered),
                ));
            }
            if matches!(attack, Attack::Silent) && offered.load(Ordering::SeqCst) {
                ElapsedTick(1_000_000)
            } else {
                ElapsedTick(1001)
            }
        })
        .unwrap_err();
        let expected = match attack {
            Attack::Silent => "deadline",
            Attack::Truncated => "UnexpectedEof",
            _ => "Binding",
        };
        assert!(error.contains(expected), "{attack:?}: {error}");
        assert!(
            peer.unwrap().join().unwrap().is_empty(),
            "no native receipt before an exact request"
        );
        assert_eq!(files(&case), original);
        assert!(!case.root.path("second.anchor").exists());
        assert!(!case.root.path("second-checkpoint.bin").exists());
        assert!(!crate::workflow::control::socket_path(&case.peers, OPERATION).exists());
    }
    // Same checked history, operation and peer identity; only the complete exact
    // request changes, and now the original admission consumer acknowledges it.
    let (stopped, canonical) = oracle(&case);
    let (terminal, client) = run(
        &case,
        &arguments(&case.root, true, true, false),
        false,
        || ElapsedTick(1001),
    );
    assert!(
        terminal.contains("native admission Stop acknowledged"),
        "{terminal}"
    );
    assert!(client.receipt.unwrap().acknowledged());
    assert_original_stop(&case, &stopped, &canonical);
}

#[test]
fn queued_stop_cannot_relax_source_identity_history_floors_or_checked_publication() {
    let case = prepared(true);
    let original = files(&case);
    let configuration = fs::read_to_string(case.root.path("config.json")).unwrap();
    let guarded = fs::read_to_string(case.root.path("guarded.json")).unwrap();
    let floor = case.saved.minimum();
    for changed in 0..5 {
        let mut args = arguments(&case.root, true, true, false);
        if changed == 0 {
            let altered =
                configuration.replace("\"capture_generation\": 1", "\"capture_generation\": 2");
            assert_ne!(altered, configuration);
            fs::write(case.root.path("config.json"), altered).unwrap();
        } else if changed < 4 {
            let (field, value) = match changed {
                1 => ("journal_revision", floor.journal_revision),
                2 => ("control_sequence", floor.control_sequence),
                _ => ("authority_epoch", floor.authority_epoch),
            };
            let altered = guarded.replace(
                &format!("\"{field}\":{value}"),
                &format!("\"{field}\":{}", value + 1),
            );
            assert_ne!(altered, guarded);
            fs::write(case.root.path("guarded.json"), altered).unwrap();
        } else {
            args = arguments(&case.root, false, true, false);
        }
        let (error, client) = run(&case, &args, false, || ElapsedTick(1001));
        assert!(
            !error.contains("native admission Stop acknowledged"),
            "{changed}: {error}"
        );
        assert!(
            client.binding.is_none(),
            "complete independent guard checks precede any offer: {changed}"
        );
        assert!(client.receipt.is_none());
        assert!(!client.outcome_unknown);
        assert!(client.failure.is_some());
        assert_eq!(files(&case), original);
        assert!(!case.root.path("second.anchor").exists());
        assert!(!case.root.path("second-checkpoint.bin").exists());
        fs::write(case.root.path("config.json"), &configuration).unwrap();
        fs::write(case.root.path("guarded.json"), &guarded).unwrap();
    }
    let (stopped, canonical) = oracle(&case);
    let (_, client) = run(
        &case,
        &arguments(&case.root, true, true, false),
        false,
        || ElapsedTick(1001),
    );
    assert!(client.receipt.unwrap().acknowledged());
    assert_original_stop(&case, &stopped, &canonical);
}

#[test]
fn lost_reply_and_post_ack_deadline_preserve_each_independent_custody_export() {
    for block_history in [true, false] {
        let case = prepared(true);
        let (stopped, canonical) = oracle(&case);
        let blocked = case.root.path(if block_history {
            "second.anchor"
        } else {
            "second-checkpoint.bin"
        });
        let mut expired = false;
        let (terminal, client) = run(
            &case,
            &arguments(&case.root, true, true, false),
            true,
            || {
                if fs::read(case.config.store.join("delivery.bin")).unwrap() == canonical {
                    assert!(
                        !expired,
                        "the acknowledged result must not restart the workflow clock"
                    );
                    expired = true;
                    fs::write(
                        &blocked,
                        b"independently occupied after native acknowledgment",
                    )
                    .unwrap();
                    ElapsedTick(1_000_000)
                } else {
                    ElapsedTick(1001)
                }
            },
        );
        assert!(expired);
        assert!(client.binding.is_some());
        assert!(client.receipt.is_none());
        assert!(client.outcome_unknown);
        assert!(
            terminal.contains("native admission Stop acknowledged"),
            "{terminal}"
        );
        assert!(
            terminal.contains("after native recovery Stop application"),
            "{terminal}"
        );
        assert!(terminal.contains("retention"), "{terminal}");
        assert_original_stop(&case, &stopped, &canonical);
        assert_eq!(
            fs::read(blocked).unwrap(),
            b"independently occupied after native acknowledgment"
        );
        if block_history {
            assert_source_retained(&case, &stopped);
        } else {
            assert_eq!(&retained_anchor(&case), stopped.history_anchor());
        }
    }
}
