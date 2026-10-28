//! Real learned topology publication followed by paired-custody receipt serving.
//! Synthetic fixtures exercise the original native engine and Unix peer loop.
use super::*;
use super::super::mediated_topology as topology_fixture;
use super::super::super::topology as source_custody;
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::{
    MAX_TOPOLOGY_CHECKPOINT_BYTES, TopologyCheckpoint,
};
use std::io::Cursor;

pub(super) struct TopologyCase {
    pub(super) case: Case,
    pub(super) original: TopologyCheckpoint,
    pub(super) profile: PathBuf,
}

fn historical(root: &Root, source: &Path, operation: u64, name: &str) -> PathBuf {
    let path = root.0.join(format!("{name}.json"));
    fs::write(
        &path,
        format!(
            r#"{{"schema_version":"fa.mediated-submit/2",
        "initial":{{"topology_path":"{}","inventory_path":"{}"}},
        "next":{{"topology_path":"{}","inventory_path":"{}"}},
        "operation":{operation},"source_checkpoint":"{}"}}"#,
            root.0.join("initial.json").display(),
            root.0.join("initial-inventory.json").display(),
            root.0.join("missing-next.json").display(),
            root.0.join("missing-next-inventory.json").display(),
            source.display()
        ),
    )
    .unwrap();
    path
}

pub(super) fn published(mode: Option<bool>) -> TopologyCase {
    let root = Root::new();
    let (mut config, path, loaded) = topology_fixture::selected(&root, mode, false);
    let deadline = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let source_custody::Started {
        custody: (host, reviewer, observer, _identity),
        source,
        output,
    } = source_custody::start(
        &config,
        &loaded,
        false,
        &mut control,
        &deadline,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let mut prediction = observer.map(|role| prediction::Session::new(loaded.request, role));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    source_custody::attach(&mut driver, source).unwrap();
    source_custody::certify(&mut driver, &deadline, &mut || ElapsedTick(1000)).unwrap();
    assert!(
        generate_selected(
            &mut driver,
            &reviewer,
            &mut config,
            &mut control,
            &deadline,
            &mut || ElapsedTick(1000),
            prediction.as_mut()
        )
        .unwrap()
    );
    submit(
        &mut driver,
        &mut wire,
        &mut config,
        &loaded,
        &deadline,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    let (mut driver, history) =
        native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
    let human = native::approve(&mut driver, &mut config, &reviewer, loaded.request);
    assert!(matches!(
        driver
            .step_computed_from_policy_file(&mut config.source, || ElapsedTick(1000), Some(&human))
            .result
            .unwrap(),
        FileDriverEvent::Dispatched { .. }
    ));
    assert!(matches!(
        driver
            .step_computed_from_policy_file(&mut config.source, || ElapsedTick(1000), None)
            .result
            .unwrap(),
        FileDriverEvent::PublicationChecked { .. }
    ));
    let outcome = response(&mut wire, loaded.request).result.unwrap();
    assert!(matches!(outcome, Knowledge::Unknown { .. }));
    let before = driver.supervisor().host().unwrap().inspect();
    assert_eq!(before.executions, 1);
    assert!(source_custody::retain(output.as_ref(), &driver, None).is_none());
    assert!(history_anchors::retain(&loaded, &driver, None).is_none());
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(wire);
    drop(prediction);
    drop(history);
    let anchor = root.0.join("history-1.bin");
    let original = topology_fixture::checkpoint(&root, 1);
    assert_eq!(
        original.history_anchor(),
        &history_anchors::load(&anchor).unwrap()
    );
    assert_eq!(original.history_anchor().revision(), before.revision);
    let profile = historical(
        &root,
        &root.0.join("source-1.bin"),
        original.last_operation() + 1,
        "receipt-service-topology",
    );
    let mut loaded = recipe::load(&path, &config, false).unwrap();
    assert!(loaded.native.is_none());
    loaded.recovery_anchor = Some(history_anchors::load(&anchor).unwrap());
    loaded.floor.journal_revision = before.revision;
    loaded.floor.control_sequence = before.control.sequence;
    loaded.floor.authority_epoch = before.control.ledger.epoch;
    source_custody::admit(
        &config,
        &mut loaded,
        Some(&profile),
        source_custody::Mode::Historical,
    )
    .unwrap();
    for name in [
        "live-1.json",
        "inventory-1.json",
        "evidence.json",
        "native-roster.json",
        "helper-model.json",
        "helper-weights.safetensors",
        "helper-monitor.json",
        "helper-tokenizer.bbpe",
        "helper-sampling.json",
    ] {
        fs::remove_file(root.0.join(name)).unwrap();
    }
    fs::write(
        config.store.join("delivery.pending"),
        b"unconfirmed receipt-service staging",
    )
    .unwrap();
    let (socket, _) = UnixStream::pair().unwrap();
    let credentials = PeerCredentials::observe(&socket).unwrap();
    let peer = PeerPolicy::new(
        credentials.uid(),
        credentials.gid(),
        Some(credentials.pid()),
    )
    .unwrap();
    let actor = Profile {
        request: loaded.request,
        scope: config.profile.delivery.scope,
        socket: root.0.join("peers/receipt.sock"),
        supervisor: peer,
        actor: peer,
        candidates: 4,
        connections: 4,
        exchanges: 32,
        runtime_ms: 100,
        poll_ms: 1,
        reply_ms: 10,
    };
    let case = Case {
        root,
        config,
        loaded,
        recipe: path,
        anchor,
        actor,
        before,
        outcome,
        after: None,
    };
    assert_eq!(case.inspect(), case.before);
    TopologyCase {
        case,
        original,
        profile,
    }
}

pub(super) fn args(
    setup: &TopologyCase,
    input: &Path,
    profile: &Path,
    outputs: Option<(&Path, &Path)>,
    mode: Mode,
) -> Vec<String> {
    let case = &setup.case;
    let config = config_file(case);
    let actor = actor_file(case);
    let mut args = vec![
        "create-learned-generated".into(),
        config.display().to_string(),
        case.recipe.display().to_string(),
        "--resume".into(),
        "--serve-receipts".into(),
        actor.display().to_string(),
        "--history-anchor".into(),
        input.display().to_string(),
        "--topology-profile".into(),
        profile.display().to_string(),
    ];
    if let Some((history, source)) = outputs {
        args.extend([
            "--retain-history-anchor".into(),
            history.display().to_string(),
            "--retain-topology-checkpoint".into(),
            source.display().to_string(),
        ]);
    }
    args.push(
        if mode == Mode::Passive {
            "--receipts-only"
        } else {
            "--reconcile-receipts"
        }
        .into(),
    );
    args
}

pub(super) fn source(path: &Path) -> TopologyCheckpoint {
    let bytes = fs::read(path).unwrap();
    TopologyCheckpoint::read_trusted(&mut Cursor::new(bytes), MAX_TOPOLOGY_CHECKPOINT_BYTES)
        .unwrap()
}

pub(super) fn same_source(original: &TopologyCheckpoint, current: &TopologyCheckpoint, revision: u64) {
    assert_eq!(current.history_anchor().revision(), revision);
    assert_eq!(current.topology_bytes(), original.topology_bytes());
    assert_eq!(current.inventory_bytes(), original.inventory_bytes());
    assert_eq!(current.expected(), original.expected());
    assert_eq!(current.work(), original.work());
    assert_eq!(current.last_operation(), original.last_operation());
    assert!(!current.available());
    assert!(current.minimum().control_sequence >= original.minimum().control_sequence);
    assert!(current.minimum().authority_epoch >= original.minimum().authority_epoch);
}

pub(super) fn pair(setup: &TopologyCase, history: &Path, source_path: &Path, revision: u64) {
    let current = source(source_path);
    same_source(&setup.original, &current, revision);
    assert_eq!(
        current.history_anchor(),
        &history_anchors::load(history).unwrap()
    );
}

pub(super) fn serve_command(
    setup: &TopologyCase,
    args: &[String],
    mode: Mode,
    history: &Path,
    source: &Path,
    revision: u64,
) -> Vec<WireResponse> {
    let case = &setup.case;
    let done = Cell::new(false);
    let waits = Cell::new(0);
    let clocks = Cell::new(0);
    let mut client = None;
    let mut replies = Vec::new();
    let report = service::command_with(
        args,
        None,
        || {
            assert_eq!(mode, Mode::Reconcile);
            clocks.set(clocks.get() + 1);
            ElapsedTick(1001)
        },
        &mut || {
            if done.get() {
                Duration::from_millis(100)
            } else {
                Duration::ZERO
            }
        },
        &mut |_| {
            waits.set(waits.get() + 1);
            assert!(waits.get() < 128);
            if waits.get() == 1 {
                // Both independently retained archives precede the first actor exchange.
                pair(setup, history, source, revision);
                drop(
                    FileOversight::begin_open_with_learned_generation(
                        &case.config.store,
                        case.config.profile.clone(),
                        &case.loaded.generation,
                    )
                    .unwrap(),
                );
            }
            exchange(case, &mut client, &mut replies, &done);
        },
    )
    .unwrap();
    assert_eq!(clocks.get(), usize::from(mode == Mode::Reconcile));
    assert_eq!(report.frames, 3);
    assert_eq!(report.candidates, 2);
    assert_eq!(replies[1].result, Err(WireError::Withheld));
    assert_eq!(replies[0].result, replies[2].result);
    assert!(!case.actor.socket.exists());
    replies
}

#[test]
fn topology_receipt_service_retains_paired_custody_before_exchange_in_every_learned_mode() {
    for mode in [None, Some(false), Some(true)] {
        let setup = published(mode);
        let case = &setup.case;
        let old_history = fs::read(&case.anchor).unwrap();
        let old_source = fs::read(case.root.0.join("source-1.bin")).unwrap();
        let history = case.root.0.join("service-current.anchor");
        let source = case.root.0.join("service-current.checkpoint");
        let command = args(
            &setup,
            &case.anchor,
            &setup.profile,
            Some((&history, &source)),
            Mode::Reconcile,
        );
        let revision = case.before.revision + 3;
        let replies = serve_command(
            &setup,
            &command,
            Mode::Reconcile,
            &history,
            &source,
            revision,
        );
        assert!(matches!(
            replies[0].result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            })
        ));
        let after = case.inspect();
        assert_eq!(after.revision, revision);
        assert_eq!(after.executions, case.before.executions);
        assert_eq!(after.payload, case.before.payload);
        assert_eq!(
            after.control.ledger.charged,
            case.before.control.ledger.charged
        );
        assert!(!case.config.store.join("delivery.pending").exists());
        let completed = case.files();
        let passive_profile = historical(
            &case.root,
            &source,
            setup.original.last_operation() + 1,
            "retained-service-topology",
        );
        let passive = args(&setup, &history, &passive_profile, None, Mode::Passive);
        let replies = serve_command(&setup, &passive, Mode::Passive, &history, &source, revision);
        assert!(matches!(
            replies[0].result,
            Ok(Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            })
        ));
        assert_eq!(case.files(), completed);
        assert_eq!(case.inspect(), after);
        assert_eq!(fs::read(&case.anchor).unwrap(), old_history);
        assert_eq!(
            fs::read(case.root.0.join("source-1.bin")).unwrap(),
            old_source
        );
        assert!(!case.root.0.join("live-1.json").exists());
        assert!(!case.root.0.join("inventory-1.json").exists());
        assert!(!case.root.0.join("native-roster.json").exists());
    }
}

#[test]
fn topology_receipt_service_exports_independently_after_races_and_serves_no_unretained_result() {
    for (block_history, block_source) in [(true, false), (false, true), (true, true)] {
        let setup = published(Some(true));
        let case = &setup.case;
        let history = case.root.0.join("raced-service.anchor");
        let source_path = case.root.0.join("raced-service.checkpoint");
        let command = args(
            &setup,
            &case.anchor,
            &setup.profile,
            Some((&history, &source_path)),
            Mode::Reconcile,
        );
        let mut clocks = 0;
        let error = service::command_with(
            &command,
            None,
            || {
                clocks += 1;
                if block_history {
                    fs::write(&history, b"existing history output").unwrap();
                }
                if block_source {
                    fs::write(&source_path, b"existing source output").unwrap();
                }
                ElapsedTick(1001)
            },
            &mut || Duration::ZERO,
            &mut |_| panic!("a failed custody export cannot expose a receipt port"),
        )
        .unwrap_err();
        assert_eq!(clocks, 1);
        assert!(error.contains("reconciliation acknowledged"), "{error}");
        assert!(error.contains("custody retention failed"), "{error}");
        assert!(error.contains("no effect retry was sent"), "{error}");
        let revision = case.before.revision + 3;
        if block_history {
            assert_eq!(fs::read(&history).unwrap(), b"existing history output");
        } else {
            assert_eq!(
                history_anchors::load(&history).unwrap().revision(),
                revision
            );
        }
        if block_source {
            assert_eq!(fs::read(&source_path).unwrap(), b"existing source output");
        } else {
            same_source(&setup.original, &source(&source_path), revision);
        }
        let after = case.inspect();
        assert_eq!(after.revision, revision);
        assert_eq!(after.executions, 1);
        assert_eq!(
            after.control.ledger.charged,
            case.before.control.ledger.charged
        );
        assert!(!case.actor.socket.exists());
        assert_eq!(source(&case.root.0.join("source-1.bin")), setup.original);
    }
}

#[test]
fn topology_receipt_service_retains_acknowledgment_before_late_deadline_or_peer_failure() {
    for peer_failure in [false, true] {
        let setup = published(Some(false));
        let case = &setup.case;
        let history = case.root.0.join("interrupted-service.anchor");
        let source = case.root.0.join("interrupted-service.checkpoint");
        let command = args(
            &setup,
            &case.anchor,
            &setup.profile,
            Some((&history, &source)),
            Mode::Reconcile,
        );
        if peer_failure {
            let actor_path = Path::new(&command[5]);
            let original = fs::read_to_string(actor_path).unwrap();
            let actor = case.actor.actor;
            let old = format!(
                "\"actor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}}",
                actor.uid(),
                actor.gid(),
                actor.pid().unwrap()
            );
            let new = format!(
                "\"actor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}}",
                if actor.uid() == 0 { 1 } else { 0 },
                actor.gid(),
                actor.pid().unwrap()
            );
            assert!(original.contains(&old));
            fs::write(
                actor_path,
                original
                    .replace(&old, &new)
                    .replace("\"candidate_limit\":4", "\"candidate_limit\":1")
                    .replace("\"connection_limit\":4", "\"connection_limit\":1"),
            )
            .unwrap();
        }
        let acknowledged = Cell::new(false);
        let clocks = Cell::new(0);
        let mut peer = None;
        let error = service::command_with(
            &command,
            None,
            || {
                clocks.set(clocks.get() + 1);
                acknowledged.set(true);
                ElapsedTick(1001)
            },
            &mut || {
                if acknowledged.get() && !peer_failure {
                    Duration::from_millis(100)
                } else {
                    Duration::ZERO
                }
            },
            &mut |_| {
                assert!(peer_failure);
                assert!(peer.is_none());
                pair(&setup, &history, &source, case.before.revision + 3);
                let socket = UnixStream::connect(&case.actor.socket).unwrap();
                socket.set_nonblocking(true).unwrap();
                peer = Some(socket);
            },
        )
        .unwrap_err();
        assert_eq!(clocks.get(), 1);
        assert!(error.contains("reconciliation acknowledged"), "{error}");
        assert!(error.contains("no effect retry was sent"), "{error}");
        if peer_failure {
            assert!(error.contains("CredentialsRejected"), "{error}");
            assert_eq!(peer.as_mut().unwrap().read(&mut [0; 1]).unwrap(), 0);
        } else {
            assert!(error.contains("startup deadline reached"), "{error}");
        }
        pair(&setup, &history, &source, case.before.revision + 3);
        let after = case.inspect();
        assert_eq!(after.revision, case.before.revision + 3);
        assert_eq!(after.executions, 1);
        assert_eq!(
            after.control.ledger.charged,
            case.before.control.ledger.charged
        );
        assert!(!case.actor.socket.exists());
    }
}

#[test]
fn topology_receipt_service_refuses_invalid_output_selection_and_socket_collision_before_clock() {
    let setup = published(None);
    let case = &setup.case;
    let before = case.files();
    let history = case.root.0.join("selected-service.anchor");
    let source = case.root.0.join("selected-service.checkpoint");
    let good = args(
        &setup,
        &case.anchor,
        &setup.profile,
        Some((&history, &source)),
        Mode::Reconcile,
    );
    let mut bad = good.clone();
    *bad.last_mut().unwrap() = "--receipts-only".into();
    let mut duplicate = good.clone();
    let end = duplicate.len() - 1;
    duplicate.splice(
        end..end,
        [
            "--retain-topology-checkpoint".into(),
            case.root
                .0
                .join("duplicate.checkpoint")
                .display()
                .to_string(),
        ],
    );
    let mut same = good.clone();
    let at = same
        .iter()
        .position(|arg| arg == "--retain-topology-checkpoint")
        .unwrap();
    same[at + 1] = history.display().to_string();
    let mut missing = good.clone();
    let at = missing
        .iter()
        .position(|arg| arg == "--retain-history-anchor")
        .unwrap();
    missing.drain(at..at + 2);
    for invalid in [bad, duplicate, same, missing] {
        assert!(
            service::command_with(
                &invalid,
                None,
                || panic!("invalid custody cannot read time"),
                &mut || Duration::ZERO,
                &mut |_| panic!("invalid custody cannot serve")
            )
            .is_err()
        );
        assert_eq!(case.files(), before);
        assert!(!case.actor.socket.exists());
        assert!(!history.exists());
        assert!(!source.exists());
    }
    fs::write(&case.actor.socket, b"occupied service endpoint").unwrap();
    assert!(
        service::command_with(
            &good,
            None,
            || panic!("occupied socket cannot spend a Fence"),
            &mut || Duration::ZERO,
            &mut |_| panic!("occupied socket cannot serve")
        )
        .is_err()
    );
    assert_eq!(
        fs::read(&case.actor.socket).unwrap(),
        b"occupied service endpoint"
    );
    assert_eq!(case.files(), before);
    assert!(!history.exists());
    assert!(!source.exists());
    fs::remove_file(&case.actor.socket).unwrap();
    let replies = serve_command(
        &setup,
        &good,
        Mode::Reconcile,
        &history,
        &source,
        case.before.revision + 3,
    );
    assert!(matches!(
        replies[0].result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    ));
}
