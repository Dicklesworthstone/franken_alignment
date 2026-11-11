//! Build a real original Unknown through helper processes and an independent
//! human key, then retain that exact owner for the scheduled service.
use super::*;
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::FileHumanReviewer;
use fa_reference::action::consequence::delivery::persistent::observed::publication::{
    capture::{FileCaptureIdentity, FilePublicationCapture},
    witnesses::{FilePublicationInputs, FileWitnessInput},
};
use fa_reference::product_frontier::{
    FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};

pub(super) struct Prepared {
    pub config: Config,
    pub actor: Profile,
    pub reviewer: PeerProfile,
    pub publication: Option<PublicationProfile>,
    pub settings: Settings,
    pub port: Port,
    pub driver: FileSupervisedDriver,
    pub human: FileHumanReviewer,
}

fn checked(root: &Directory) -> PublicationProfile {
    PublicationProfile::decode(format!(
        r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}/original.bin","current":"{}/current.bin","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":7,"role":"subject"}}]}}"#,
        root.0.display(), root.0.display(),
    ).as_bytes()).unwrap()
}

fn write_capture(root: &Directory, action: &FrozenAction, attempt: u64) {
    let key = ProjectionKey {
        source: 40, branch: action.spec().scope.branch, projection: 7, source_epoch: 1,
    };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(
        1, 1, 20,
        AdapterDomainInput::new(
            DomainProjection::new(40, 1, key), DomainClosure::Closed(close),
        ),
        vec![SnapshotEntry::new(7, 1, b"ok".to_vec()).unwrap()],
        &frontiers,
    ).unwrap();
    let packet = FilePublicationCapture::new(
        attempt, FileCaptureIdentity { source: 91, generation: 1 }, action,
        FilePublicationInputs::new(Some(input), None),
    ).unwrap().to_bytes().unwrap();
    fs::write(root.0.join("original.bin"), &packet).unwrap();
    fs::write(root.0.join("current.bin"), packet).unwrap();
}

pub(super) fn second_capture(root: &Directory, config: &Config) {
    // This separate owner supplies only a fixture capture. The scheduled native
    // owner still constructs and checks the second action, reviews and keys.
    let (mut host, _) = FileOversight::create(
        root.0.join("capture-owner"), config.profile.clone(),
    ).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1002)).unwrap();
    let action = host.propose(host.revision(), 2, ActionSpec {
        version: VERSION, scope: config.profile.delivery.scope,
        target: Some(config.profile.delivery.target), payload: b"second".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100000), units: 7,
    }, Snapshot {
        semantic_epoch: 1, complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]),
    }).unwrap();
    write_capture(root, &action, 2);
}

pub(super) fn prepare(root: &Directory, with_checked: bool) -> Prepared {
    let mut config = configured(root);
    // The first dispatch's endpoint retention really expires before Unknown.
    // Later Stop cannot invent nonexecution or refund this original liability.
    config.profile.delivery.retention_ticks = 1;
    evidence(root, &config);
    let (mut actor, reviewer) = profiles(root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    actor.reply_ms = 1000;
    let publication = with_checked.then(|| checked(root));
    let (mut host, human) = prepare_host(&config, publication.as_ref(), false).unwrap();
    let captured = host.refresh_file_source(
        host.revision(), &mut config.source, ElapsedTick(1000),
    ).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let (port, mut driver) = host.into_supervised_driver();
    let revision = driver.supervisor().host().unwrap().revision();
    driver.supervisor_mut().set_snapshot(
        revision, Some(captured.snapshot().clone()),
    ).unwrap();
    let mut wire = ActorWire::new(port.clone());
    let submitted = wire.exchange(&encode_command(&Command::Submit {
        request: 7,
        proposal: ActorProposal {
            target: config.profile.delivery.target, payload: b"first".to_vec(),
            units: 7, deadline: ElapsedTick(100000), expected_policy_epoch: 0,
        },
    }).unwrap());
    assert!(matches!(submitted.result, Ok(Knowledge::Pending { .. })));
    if with_checked {
        let host = driver.supervisor().host().unwrap();
        write_capture(root, host.request_action(7).unwrap(), 1);
    }
    let programs = config.programs.clone();
    let first_human = human_for(reviewer.clone(), 7);
    let deadline = Deadline {
        logical: ElapsedTick(16000), started: Instant::now(),
        wall: Duration::from_millis(config.timing.runtime_ms),
    };
    let mut interrupted = false;
    let mut before_send = |driver: &mut FileSupervisedDriver| -> Result<bool, String> {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        if let FileRequestDisposition::Admitted {
            attempt, stage: ActionState::Dispatching,
        } = host.request_status(7).map_err(debug)?.disposition {
            assert!(!interrupted);
            interrupted = true;
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(1002)).map_err(debug)?;
            let revision = host.revision();
            host.reconcile(revision, attempt).map_err(debug)?;
            assert_eq!(host.inspect().control.ledger.stages[&attempt], ActionState::Unknown);
            return Ok(true);
        }
        Ok(false)
    };
    execute_serviced(
        &mut driver, &human, &mut config, 7, &deadline,
        ExecuteServices {
            peers: Some(&reviewer), publication: publication.as_ref(),
            ingress: &mut before_send, stop: None,
        },
        &mut || ElapsedTick(1000),
    ).unwrap();
    first_human.join().unwrap();
    assert!(interrupted);
    config.programs = programs;
    let before = driver.supervisor().host().unwrap().inspect();
    assert_eq!((before.executions, before.control.ledger.charged), (0, 7));
    assert_eq!(before.control.ledger.reserved, 0);
    assert!(before.stop.is_none());
    drop(wire);
    // Explicit same-owner release withdraws only the local sendable job. The
    // original authority, intake snapshot, port and child duty are retained.
    let driver = driver.release().into_driver().unwrap();
    assert_eq!(driver.supervisor().host().unwrap().inspect(), before);
    let text = settings_document(root, &config, 0, 0).replacen(
        '{',
        &format!(r#"{{"abandonment":{{"queries":2,"operations":1,"max_replay_events":{MAX_ABANDONMENT_REPLAY_EVENTS},"authority":"abandon_unknown_requests"}},"#),
        1,
    );
    let mut settings = Settings::decode(text.as_bytes()).unwrap();
    settings.connection_limit = 2;
    settings.candidate_limit = 2;
    Prepared { config, actor, reviewer, publication, settings, port, driver, human }
}

pub(super) fn serve<F>(prepared: Prepared, time: F) -> Result<(), String>
where F: FnMut() -> ElapsedTick,
{
    let Prepared { config, actor, reviewer, publication, settings, port, driver, human } = prepared;
    service::validate(&[7, 8], &actor, &reviewer)?;
    actor.check_host(&config)?;
    reviewer.check_host(&config)?;
    let paths = [
        reviewer.socket(7), reviewer.socket(8),
        crate::workflow::control::socket_path(&reviewer, 7),
        crate::workflow::control::socket_path(&reviewer, 8),
    ];
    let paths: Vec<_> = paths.iter().map(|path| path.as_path()).collect();
    settings.check_host(&config, &actor, &reviewer, &paths)?;
    let socket = BoundSocket::bind(&actor.socket, None)?;
    actor.secure_socket()?;
    let deadline = Deadline {
        logical: ElapsedTick(1002 + config.timing.runtime_ms), started: Instant::now(),
        wall: Duration::from_millis(config.timing.runtime_ms),
    };
    service::serve_ready_driver(
        config, &actor, &reviewer, publication.as_ref(),
        Options { requests: &[7, 8], open: false, credibility: None },
        Some(GENERATION), Some(&settings), socket, port, driver, human, None,
        OwnerStart::Ordinary, None, deadline, time, |_, _, _| Ok(()),
    )
}
