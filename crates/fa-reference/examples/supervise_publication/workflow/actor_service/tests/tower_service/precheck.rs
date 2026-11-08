//! The existing sidecar and Tower ingress share one original supervised owner.
//! Synthetic helper fixtures test control boundaries, not detector quality.

use super::*;
use crate::workflow::actor_service::precheck::Settings;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{
        PrecheckRequest, PrecheckWireError, PrecheckWireResponse, client::PrecheckPeerClient,
        decode_precheck_response, encode_precheck_request,
    },
};

fn settings_document(socket: &Path, version: u8, queries: usize) -> String {
    let (disclosure, extra) = match version {
        1 => ("exact_predicates", ""),
        2 => (
            "exact_predicates",
            ",\"cache_entries\":4,\"cache_bytes\":262144",
        ),
        3 => (
            "full",
            concat!(
                ",\"cache_entries\":4,\"cache_bytes\":262144,",
                "\"leakage\":{\"policy_structure\":\"disclosed\",",
                "\"policy_literals\":\"disclosed\",\"source_values\":\"withheld\",",
                "\"helper_feedback\":\"withheld\",\"consequences\":\"withheld\",",
                "\"timing\":\"observable\",\"query_usage\":\"observable\"}"
            ),
        ),
        _ => panic!("unsupported fixture profile"),
    };
    format!(
        r#"{{"schema":"fa.actor-precheck-service/{version}","policy_disclosure":"{disclosure}","socket":"{}","expected_policy_epoch":0,"disclosure_id":81,"query_limit":{queries},"exchange_limit":8,"candidate_limit":8,"connection_limit":4,"runtime_ms":15000{extra}}}"#,
        socket.display()
    )
}

fn settings(root: &Directory) -> Settings {
    Settings::decode(settings_document(&root.0.join("actors/precheck.sock"), 1, 4).as_bytes())
        .unwrap()
}

fn query(profile: &Profile, socket: &Path, proposal: ActorProposal) -> PrecheckWireResponse {
    wait(socket);
    let mut client = PrecheckPeerClient::connect(
        UnixStream::connect(socket).unwrap(),
        profile.supervisor,
        PrecheckRequest {
            query: 9001,
            proposal,
        },
        0,
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "precheck reply timed out"
        );
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() {
            return response;
        }
        assert!(
            !report.status.closed(),
            "closed without its original response"
        );
        pause(1);
    }
}

fn passed(response: PrecheckWireResponse) {
    assert_eq!(response.query, Some(9001));
    let Ok(Knowledge::Known { value, basis }) = response.result else {
        panic!("expected original exact-precheck result");
    };
    assert_eq!(value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!(
        (basis.query, basis.disclosure, basis.policy_epoch),
        (9001, 81, 0)
    );
    assert_eq!(basis.observed_at, ElapsedTick(1000));
}

fn candidate(config: &Config) -> ActorProposal {
    let Command::Submit { proposal, .. } = proposal(config) else {
        unreachable!();
    };
    proposal
}

fn store_files(store: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(store)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn serve_with_precheck(
    config: Config,
    actor: &Profile,
    reviewer: &PeerProfile,
    settings: &Settings,
) -> Result<(), String> {
    serve_selected_ingress(
        config,
        actor,
        reviewer,
        None,
        false,
        None,
        None,
        Some(settings),
        Some(GENERATION),
        None,
        || ElapsedTick(1000),
    )
}

#[test]
fn actor_precheck_then_tower_publication_preserves_all_three_profiles_and_shared_query_work() {
    for version in 1..=3 {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let tower_args = prepared_cli(&root, &config);
        let precheck_socket = root.0.join("actors/precheck.sock");
        let profile_path = root.0.join("precheck-profile.json");
        let request_path = root.0.join("precheck-request.json");
        let profile = settings_document(&precheck_socket, version, 1);
        let settings = Settings::decode(profile.as_bytes()).unwrap();
        fs::write(&profile_path, profile).unwrap();
        fs::write(
            &request_path,
            encode_precheck_request(&PrecheckRequest {
                query: 9001,
                proposal: candidate(&config),
            })
            .unwrap(),
        )
        .unwrap();
        let precheck_args = vec![
            "actor-precheck".to_owned(),
            tower_args[1].clone(),
            profile_path.to_str().unwrap().to_owned(),
            request_path.to_str().unwrap().to_owned(),
        ];
        let store = config.store.clone();
        let selected_socket = precheck_socket.clone();
        let review_socket = reviewer.socket(7);
        let peer = std::thread::spawn(move || {
            wait(&selected_socket);
            let before = store_files(&store);
            let mut output = Vec::new();
            crate::command_with_output(precheck_args.clone(), &mut output).unwrap();
            passed(decode_precheck_response(output.strip_suffix(b"\n").unwrap(), 9001, 0).unwrap());
            assert_eq!(
                store_files(&store),
                before,
                "query mutated the original owner"
            );
            assert!(
                !review_socket.exists(),
                "precheck launched helper/human review"
            );

            // An explicit reconnect and repeated query label spend the SAME
            // grant; neither the Tower session nor native witness cache renews it.
            output.clear();
            assert!(crate::command_with_output(precheck_args, &mut output).is_err());
            let exhausted =
                decode_precheck_response(output.strip_suffix(b"\n").unwrap(), 9001, 0).unwrap();
            assert_eq!(exhausted.result, Err(PrecheckWireError::Capacity));
            assert_eq!(store_files(&store), before);
            assert!(!review_socket.exists());

            let mut output = Vec::new();
            crate::command_with_output(tower_args, &mut output).unwrap();
            output
        });
        let human = human(reviewer.clone());
        serve_with_precheck(config, &actor, &reviewer, &settings).unwrap();
        human.join().unwrap();
        let output = peer.join().unwrap();
        let replies: Vec<_> = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(replies.len(), 2);
        assert_eq!(
            field(knowledge(&parse(replies[0])), "state").as_str(),
            Some("pending")
        );
        let config = configured(&root);
        let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(disk.executions, 1);
        assert_eq!(disk.payload, b"network");
        assert_eq!(
            (disk.control.ledger.charged, disk.control.ledger.reserved),
            (7, 0)
        );
        assert_eq!(disk.control.ledger.stages.len(), 1);
        assert!(disk.stop.is_none());
        assert!(!precheck_socket.exists());
        assert!(!actor.socket.exists());
    }
}

#[test]
fn tower_submit_reacquires_changed_source_after_a_real_passing_precheck() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root);
    let precheck_socket = root.0.join("actors/precheck.sock");
    let source_path = root.0.join("evidence.json");
    let changed = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 2,
            scope: config.profile.delivery.scope,
        },
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(7, b"blocked".to_vec())]),
        },
        ["alpha", "beta"]
            .into_iter()
            .map(|member| {
                (
                    member.to_owned(),
                    format!("review this fixture: {member}").into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap()
    .encode();
    let candidate = candidate(&config);
    let document = proposal_document(&config, 7);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        passed(query(&profile, &precheck_socket, candidate));
        fs::write(source_path, changed).unwrap();
        raw_exchange(&mut connect(&profile), &document)
    });
    serve_with_precheck(config, &actor, &reviewer, &settings).unwrap();
    let response = peer.join().unwrap();
    assert_eq!(field(knowledge(&response), "state").as_str(), Some("known"));
    assert_eq!(
        field(knowledge(&response), "value").as_str(),
        Some("denied")
    );
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 0);
    assert_eq!(
        (disk.control.ledger.charged, disk.control.ledger.reserved),
        (0, 0)
    );
    assert!(disk.stop.is_none());
    assert!(!reviewer.socket(7).exists());
    assert!(!root.0.join("actors/precheck.sock").exists());
}

#[test]
fn checked_tower_precheck_preserves_final_whole_input_validation() {
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
        FilePublicationInputs,
        producer::{FilePublicationProducer, PublicationProducerProfile},
    };
    use fa_reference::full_input::{
        ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart,
    };
    for changed in [false, true] {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let settings = settings(&root);
        let precheck_socket = root.0.join("actors/precheck.sock");
        let actual = |epoch| {
            ActualHelperInput::new(
                b"Q".to_vec(),
                InputProfileBinding {
                    profile_id: 1,
                    profile_bytes: vec![],
                    tokenizer_epoch: 1,
                    policy_epoch: 0,
                    model_epoch: epoch,
                },
                vec![SubmittedPart {
                    span: ByteSpan { start: 0, end: 1 },
                    kind: PartKind::Question,
                }],
                vec![],
            )
            .unwrap()
        };
        let (mut producer, _) = FilePublicationProducer::create(
            root.0.join("producer"),
            PublicationProducerProfile {
                source: 91,
                scope: config.profile.delivery.scope,
                feed: 41,
                clock_domain: CLOCK_DOMAIN,
                after: 0,
            },
            FilePublicationInputs::new(None, Some(actual(1))),
            ElapsedTick(1000),
        )
        .unwrap();
        let publication = PublicationProfile::decode(format!(
            r#"{{"schema":"fa.supervised-whole-input/1","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,
            root.0.display(), scope(&config)
        ).as_bytes()).unwrap();
        let candidate = candidate(&config);
        let document = proposal_document(&config, 7);
        let profile = actor.clone();
        let peer = std::thread::spawn(move || {
            passed(query(&profile, &precheck_socket, candidate));
            raw_exchange(&mut connect(&profile), &document)
        });
        let human = human(reviewer.clone());
        let store = config.store.clone();
        let bootstrap = config.profile.clone();
        let mut advanced = false;
        let result = serve_selected_ingress(
            config,
            &actor,
            &reviewer,
            Some(&publication),
            false,
            None,
            None,
            Some(&settings),
            Some(GENERATION),
        None,
            || {
                if !advanced
                    && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                    && disk.control.ledger.charged != 0
                    && disk.executions == 0
                {
                    producer
                        .publish(
                            1,
                            FilePublicationInputs::new(
                                None,
                                Some(actual(if changed { 2 } else { 1 })),
                            ),
                            ElapsedTick(1000),
                        )
                        .unwrap();
                    advanced = true;
                }
                ElapsedTick(1000)
            },
        );
        if !changed {
            result.unwrap();
        }
        human.join().unwrap();
        let response = peer.join().unwrap();
        assert_eq!(
            field(knowledge(&response), "state").as_str(),
            Some("pending")
        );
        assert!(
            advanced,
            "the original final-publication boundary was not reached"
        );
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(!changed));
        assert_eq!(disk.control.ledger.charged, if changed { 0 } else { 7 });
        assert_eq!(disk.control.ledger.reserved, 0);
        assert!(!root.0.join("actors/precheck.sock").exists());
    }
}

#[test]
fn a_truncated_sidecar_frame_does_not_stop_or_replace_tower_publication() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root);
    let precheck_socket = root.0.join("actors/precheck.sock");
    let document = proposal_document(&config, 7);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        wait(&precheck_socket);
        let mut broken = UnixStream::connect(&precheck_socket).unwrap();
        broken.write_all(b"{").unwrap();
        broken.shutdown(std::net::Shutdown::Write).unwrap();
        let started = Instant::now();
        while precheck_socket.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "sidecar did not retire"
            );
            pause(1);
        }
        assert!(UnixStream::connect(&precheck_socket).is_err());
        raw_exchange(&mut connect(&profile), &document)
    });
    let human = human(reviewer.clone());
    serve_with_precheck(config, &actor, &reviewer, &settings).unwrap();
    human.join().unwrap();
    let response = peer.join().unwrap();
    assert_eq!(
        field(knowledge(&response), "state").as_str(),
        Some("pending")
    );
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.control.ledger.charged, 7);
    assert!(
        disk.stop.is_none(),
        "read-only sidecar failure invoked Stop"
    );
}

#[test]
fn independent_stop_drains_both_partial_ingresses_without_admitting_a_request() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = settings(&root);
    let precheck_socket = root.0.join("actors/precheck.sock");
    let profile = actor.clone();
    let stop_profile = reviewer.clone();
    let peer = std::thread::spawn(move || {
        wait(&precheck_socket);
        let mut precheck = UnixStream::connect(&precheck_socket).unwrap();
        precheck.write_all(b"{\"query\":").unwrap();
        let mut tower = connect(&profile);
        tower.set_nonblocking(false).unwrap();
        tower
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        tower.write_all(b"{\"schema_version\":").unwrap();
        let mut output = Vec::new();
        crate::peers::stop::request(&stop_profile, 7, &mut output).unwrap();
        let receipt = parse(&output);
        assert_eq!(field(&receipt, "status").as_str(), Some("stopped_drained"));
    });
    serve_with_precheck(config, &actor, &reviewer, &settings).unwrap();
    peer.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert!(disk.stop.is_some());
    assert!(disk.control.ledger.stages.is_empty());
    assert_eq!(disk.executions, 0);
    assert_eq!(
        (disk.control.ledger.charged, disk.control.ledger.reserved),
        (0, 0)
    );
    assert!(!actor.socket.exists());
    assert!(!root.0.join("actors/precheck.sock").exists());
    assert!(!reviewer.socket(7).exists());
}

#[test]
fn tower_sidecar_socket_preflight_precedes_clocks_source_and_store_creation() {
    for dimension in 0..4 {
        let root = Directory::new();
        let config = configured(&root);
        let store = config.store.clone();
        let (actor, reviewer) = profiles(&root, &config);
        let path = match dimension {
            0 => actor.socket.clone(),
            1 => reviewer.socket(7),
            2 => crate::workflow::control::socket_path(&reviewer, 7),
            _ => root.0.join("actors/precheck.sock"),
        };
        if dimension == 3 {
            fs::write(&path, b"preexisting path must survive").unwrap();
        }
        let settings = Settings::decode(settings_document(&path, 1, 4).as_bytes()).unwrap();
        let result = serve_selected_ingress(
            config,
            &actor,
            &reviewer,
            None,
            false,
            None,
            None,
            Some(&settings),
            Some(GENERATION),
        None,
            || panic!("preflight entered the clock/source boundary"),
        );
        assert!(result.is_err());
        assert!(!store.exists());
        assert!(!actor.socket.exists());
        if dimension == 3 {
            assert_eq!(fs::read(path).unwrap(), b"preexisting path must survive");
        }
    }
}

#[test]
fn recorded_tower_requests_cannot_acquire_a_new_precheck_disclosure() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let document = proposal_document(&config, 7);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || raw_exchange(&mut connect(&profile), &document));
    let human = human(reviewer.clone());
    serve_tower(config, &actor, &reviewer, false, 1000).unwrap();
    peer.join().unwrap();
    human.join().unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut config = configured(&root);
    config.programs.clear();
    let settings = settings(&root);
    let result = serve_selected_ingress(
        config,
        &actor,
        &reviewer,
        None,
        true,
        None,
        None,
        Some(&settings),
        Some(GENERATION),
        None,
        || ElapsedTick(200000),
    );
    assert!(
        result
            .unwrap_err()
            .contains("recorded requests are observation-only")
    );
    assert!(!root.0.join("actors/precheck.sock").exists());
    assert!(!root.0.join("evidence.json").exists());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(7).exists());
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.control.ledger.charged, 7);
}
