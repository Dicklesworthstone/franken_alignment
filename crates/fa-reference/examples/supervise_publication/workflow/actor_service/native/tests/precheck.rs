//! Real native generation and authenticated prechecks followed by original
//! process-helper/human publication. Synthetic models establish protocol only.
use super::*;
use crate::workflow::actor_service::precheck::Settings;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::StopClientProgress;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{
        PrecheckRequest, PrecheckWireError, PrecheckWireResponse, client::PrecheckPeerClient,
    },
};
use fa_reference::action::consequence::oversight::{
    actor::ActorProposal,
    actor_transport::DriveBudget,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot},
};
use std::{collections::BTreeMap, fs, path::Path};

fn selected(root: &Root, epoch: u64) -> exact_precheck::Selection {
    exact_precheck::Selection::new(
        Settings::decode(disclosure_document(root, epoch).as_bytes()).unwrap(),
    )
    .unwrap()
}

fn disclosure_document(root: &Root, epoch: u64) -> String {
    format!(
        r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}/precheck.sock","expected_policy_epoch":{epoch},"disclosure_id":81,"query_limit":2,"exchange_limit":8,"candidate_limit":8,"connection_limit":8,"runtime_ms":15000}}"#,
        root.0.display(),
    )
}

fn query(
    actor: &Profile,
    socket: &Path,
    proposal: ActorProposal,
    label: u64,
) -> PrecheckWireResponse {
    let epoch = proposal.expected_policy_epoch;
    let mut client = PrecheckPeerClient::connect(
        UnixStream::connect(socket).unwrap(),
        actor.supervisor,
        PrecheckRequest {
            query: label,
            proposal,
        },
        epoch,
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "native precheck timed out"
        );
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() {
            return response;
        }
        assert!(
            !report.status.closed(),
            "precheck closed without its response"
        );
        pause(1);
    }
}

fn passed(response: PrecheckWireResponse, label: u64, epoch: u64) {
    assert_eq!(response.query, Some(label));
    let Ok(Knowledge::Known { value, basis }) = response.result else {
        panic!("expected exact original native precheck: {response:?}");
    };
    assert_eq!(value, FilePrecheckOutcome::PrecheckPassed);
    assert_eq!(
        (basis.query, basis.disclosure, basis.policy_epoch),
        (label, 81, epoch)
    );
}

#[derive(Clone, Copy)]
enum Behavior {
    Approve,
    Reject,
    ChangeSource,
    Truncate,
}

fn run(behavior: Behavior) {
    let root = Root::new();
    let config = configured(&root);
    let (actor, reviewer) = profiles(&root, &config);
    let native = inputs(&root, &config);
    let selection = selected(&root, 0);
    let store = config.store.clone();
    let profile = config.profile.clone();
    let decoder = native.decoder.clone();
    let tokenizer = native.tokenizer.clone();
    let socket = selection.settings.socket_path().to_owned();
    let (source_sender, source_receiver) = mpsc::channel::<Vec<u8>>();
    let (actor_sender, actor_receiver) = mpsc::channel();
    let client = actor_thread(actor.clone(), actor_receiver);
    let human = if matches!(behavior, Behavior::ChangeSource) {
        None
    } else {
        Some(reviewing(
            reviewer.clone(),
            store.clone(),
            profile.clone(),
            if matches!(behavior, Behavior::Reject) {
                ReviewDecision::Reject
            } else {
                ReviewDecision::Approve
            },
        ))
    };
    let observing_actor = actor.clone();
    let journal = store.join("delivery.bin");
    let review_socket = reviewer.socket(actor.request);
    let source = root.0.join("evidence.json");
    let denied = EvidenceSnapshot::new(
        EvidenceIdentity {
            scope: profile.delivery.scope,
            source: 51,
            generation: 2,
        },
        fa_reference::Snapshot {
            semantic_epoch: 2,
            complete: true,
            values: BTreeMap::from([(7, b"blocked".to_vec())]),
        },
        BTreeMap::from([
            ("alpha".to_owned(), b"review this fixture: alpha".to_vec()),
            ("beta".to_owned(), b"review this fixture: beta".to_vec()),
        ]),
    )
    .unwrap()
    .encode();
    let observer_socket = socket.clone();
    let observer = std::thread::spawn(move || {
        let document = source_receiver
            .recv_timeout(Duration::from_secs(20))
            .unwrap();
        let Command::Submit { proposal, .. } = decode_command(&document).unwrap() else {
            panic!("native output must remain a source-only Submit reference");
        };
        assert_eq!(
            proposal.payload.len(),
            FileGeneratedTextActorPort::INTENT_BYTES
        );
        assert!(!review_socket.exists());
        let before = fs::read(&journal).unwrap();
        if matches!(behavior, Behavior::Truncate) {
            let mut broken = UnixStream::connect(&observer_socket).unwrap();
            broken.write_all(b"{").unwrap();
            broken.shutdown(std::net::Shutdown::Write).unwrap();
            let started = Instant::now();
            while observer_socket.exists() {
                assert!(started.elapsed() < Duration::from_secs(20));
                pause(1);
            }
        } else {
            passed(
                query(&observing_actor, &observer_socket, proposal.clone(), 1),
                1,
                0,
            );
            passed(
                query(&observing_actor, &observer_socket, proposal.clone(), 2),
                2,
                0,
            );
            assert_eq!(
                query(&observing_actor, &observer_socket, proposal, 3).result,
                Err(PrecheckWireError::Capacity),
                "a reconnect must not replenish queries"
            );
        }
        assert_eq!(
            fs::read(&journal).unwrap(),
            before,
            "precheck changed source, model, request or journal"
        );
        assert!(!review_socket.exists(), "precheck launched human review");
        if matches!(behavior, Behavior::ChangeSource) {
            fs::write(source, denied).unwrap();
        }
        actor_sender.send(document).unwrap();
    });
    let mut output = ReferenceOutput {
        bytes: Vec::new(),
        sender: Some(source_sender),
    };
    let result = serve_lifecycle_with_precheck(
        config,
        &actor,
        &reviewer,
        native,
        (false, None, None, finish::Intent::Existing),
        None,
        Some(&selection),
        || ElapsedTick(1000),
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    observer.join().unwrap();
    let response = client.join().unwrap();
    let packet = human.map(|human| human.join().unwrap());
    let image =
        FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7)
            .unwrap();
    assert_eq!(image.text.generation_revision(), 4);
    assert_eq!(image.numerical.numerical.position, 4);
    assert_eq!(image.text.bytes().unwrap(), b"A");
    assert_eq!(image.publication.control.ledger.reserved, 0);
    assert!(!socket.exists());
    assert!(!actor.socket.exists());
    match behavior {
        Behavior::Approve | Behavior::Truncate => {
            assert!(matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
            let packet = packet.unwrap();
            let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
            assert_eq!(frame.message(), Some("A"));
            assert_eq!(image.publication.executions, 1);
            assert_eq!(
                image.publication.control.ledger.charged,
                packet.action().spec().units
            );
        }
        Behavior::Reject => {
            assert!(!matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
            assert!(packet.is_some());
            assert_eq!(image.publication.executions, 0);
        }
        Behavior::ChangeSource => {
            assert!(matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Denied,
                    ..
                })
            ));
            assert!(packet.is_none());
            assert_eq!(
                (
                    image.publication.executions,
                    image.publication.control.ledger.charged
                ),
                (0, 0)
            );
        }
    }
}

#[test]
fn generated_precheck_uses_one_query_grant_and_still_requires_independent_human_approval() {
    run(Behavior::Approve);
    run(Behavior::Reject);
}

#[test]
fn generated_precheck_cannot_replace_fresh_source_and_malformed_sidecar_cannot_stop_publication() {
    run(Behavior::ChangeSource);
    run(Behavior::Truncate);
}

#[test]
fn generated_precheck_stop_remains_live_while_a_query_frame_is_incomplete() {
    let root = Root::new();
    let config = configured(&root);
    let native = inputs(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let selected = selected(&root, 0);
    let socket = selected.settings.socket_path().to_owned();
    let store = config.store.clone();
    let bootstrap = config.profile.clone();
    let peer = reviewer.clone();
    let request = actor.request;
    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    let endpoint = socket.clone();
    let stop = std::thread::spawn(move || {
        receiver.recv_timeout(Duration::from_secs(20)).unwrap();
        let mut partial = UnixStream::connect(&endpoint).unwrap();
        partial.write_all(b"{").unwrap();
        let mut client = peer.connect_stop_client(request).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(20));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => pause(1),
            }
        }
    });
    let mut output = ReferenceOutput {
        bytes: Vec::new(),
        sender: Some(sender),
    };
    serve_lifecycle_with_precheck(
        config,
        &actor,
        &reviewer,
        native,
        (false, None, None, finish::Intent::Existing),
        None,
        Some(&selected),
        || ElapsedTick(1000),
        &mut output,
    )
    .unwrap();
    stop.join().unwrap();
    let image = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert!(image.stop.is_some());
    assert_eq!(image.executions, 0);
    assert!(image.control.ledger.stages.is_empty());
    assert_eq!(
        (image.control.ledger.charged, image.control.ledger.reserved),
        (0, 0)
    );
    assert!(!socket.exists());
    assert!(!reviewer.socket(actor.request).exists());
}

#[test]
fn generated_precheck_admits_native_options_in_any_order_and_preserves_incompatible_modes() {
    for checked in [false, true] {
        let mut base: Vec<_> = [
            if checked {
                "serve-open-checked"
            } else {
                "serve-open"
            },
            "/config",
            "/actor",
            "/reviewer",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        if checked {
            base.push("/witness".into());
        }
        for options in [
            [
                "--precheck-profile",
                "/precheck",
                "--native-text",
                "/recipe",
                "--after-message",
                "90",
            ],
            [
                "--finish-after-message",
                "90",
                "--native-text",
                "/recipe",
                "--precheck-profile",
                "/precheck",
            ],
        ] {
            let mut args = base.clone();
            args.extend(options.into_iter().map(str::to_owned));
            let (normalized, profile) = exact_precheck::take_option(&args).unwrap();
            assert_eq!(profile, Some(std::path::PathBuf::from("/precheck")));
            assert_eq!(normalized.last().map(String::as_str), Some("90"));
            assert!(matches!(
                normalized[normalized.len() - 2].as_str(),
                "--after-message" | "--finish-after-message"
            ));
        }
        let mut args = base;
        args.extend(
            [
                "--native-text",
                "/recipe",
                "--precheck-profile",
                "/precheck",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        for forbidden in [
            vec!["--requests", "91,92"],
            vec!["--guarded", "/guard"],
            vec!["--topology-profile", "/topology"],
            vec!["--precheck-profile", "/again"],
            vec!["--operator-profile", "/operator"],
            vec!["--tower-generation", "1"],
            vec!["--after-message", "90", "--finish-after-message", "90"],
        ] {
            let mut changed = args.clone();
            changed.extend(forbidden.into_iter().map(str::to_owned));
            assert!(
                exact_precheck::take_option(&changed).is_err(),
                "{changed:?}"
            );
        }
    }
}

#[test]
fn generated_precheck_path_alias_is_rejected_before_clock_or_durable_creation() {
    let root = Root::new();
    let config = configured(&root);
    let native = inputs(&root, &config);
    let (mut actor, reviewer) = profiles(&root, &config);
    let peers = root.0.join("peers");
    fs::create_dir(&peers).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&peers, fs::Permissions::from_mode(0o700)).unwrap();
    actor.socket = peers.join("actor.sock");
    let alias = root.0.join("alias");
    std::os::unix::fs::symlink(&root.0, &alias).unwrap();
    let settings = Settings::decode(format!(
        r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}/peers/actor.sock","expected_policy_epoch":0,"disclosure_id":81,"query_limit":1,"exchange_limit":1,"candidate_limit":1,"connection_limit":1,"runtime_ms":15000}}"#,
        alias.display(),
    ).as_bytes()).unwrap();
    let selected = exact_precheck::Selection::new(settings).unwrap();
    let store = config.store.clone();
    let error = serve_lifecycle_with_precheck(
        config,
        &actor,
        &reviewer,
        native,
        (false, None, None, finish::Intent::Existing),
        None,
        Some(&selected),
        || panic!("aliases must be checked before startup"),
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(error.contains("conflicts"), "{error}");
    assert!(!store.exists());
    assert!(!actor.socket.exists());
}

#[test]
fn impossible_generated_create_epoch_preserves_the_store_before_clock_or_source_work() {
    let root = Root::new();
    let mut config = configured(&root);
    let native = inputs(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let chosen = selected(&root, 1);
    let store = config.store.clone();
    // An unavailable source must not mask this independently knowable error.
    // Actual source capture and numerical work belong after this preflight.
    config.source =
        fa_reference::action::consequence::oversight::evidence_source::FileEvidenceSource::new(
            root.0.join("unread-source.json"),
            51,
            config.profile.delivery.scope,
            1_048_576,
        )
        .unwrap();
    let mut output = Vec::new();
    let error = serve_lifecycle_with_precheck(
        config,
        &actor,
        &reviewer,
        native,
        (false, None, None, finish::Intent::Existing),
        None,
        Some(&chosen),
        || panic!("an impossible initial epoch must fail before the first clock observation"),
        &mut output,
    )
    .unwrap_err();
    assert_eq!(
        error,
        "native precheck creation requires initial policy epoch zero"
    );
    assert!(output.is_empty());
    assert!(!store.exists());
    assert!(!actor.socket.exists());
    assert!(!reviewer.socket(actor.request).exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, actor.request).exists());
    assert!(!chosen.settings.socket_path().exists());
}

#[test]
fn native_precheck_cli_rejects_impossible_creation_before_recipe_reads_and_preserves_open_epochs() {
    let root = Root::new();
    let config = configured(&root);
    let (actor, reviewer) = profiles(&root, &config);
    let config_path = root.0.join("config.json");
    let actor_path = root.0.join("actor.json");
    let reviewer_path = root.0.join("reviewer.json");
    let precheck_path = root.0.join("precheck.json");
    let missing_recipe = root.0.join("missing-recipe.json");
    let source = root.0.join("unread-source.json");
    let config_document = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    ))
    .replace(
        "/operator/publication-store",
        config.store.to_str().unwrap(),
    )
    .replace("/operator/evidence.json", source.to_str().unwrap())
    .replace(
        r#""initial_payload_hex": "696e697469616c""#,
        r#""initial_payload_hex": """#,
    );
    fs::write(&config_path, config_document).unwrap();
    let scope = config.profile.delivery.scope;
    let scope = format!(
        r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority
    );
    let identity = format!(
        r#"{{"uid":{},"gid":{},"pid":{}}}"#,
        actor.supervisor.uid(),
        actor.supervisor.gid(),
        actor.supervisor.pid().unwrap()
    );
    fs::write(&actor_path, format!(
        r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":91,"scope":{scope},"socket":"{}","supervisor":{identity},"actor":{identity},"candidate_limit":4,"connection_limit":4,"exchange_limit":4096,"runtime_ms":15000,"poll_ms":1,"reply_ms":20}}"#,
        actor.socket.display(),
    )).unwrap();
    fs::write(&reviewer_path, format!(
        r#"{{"version":1,"clock":"unix_milliseconds","scope":{scope},"reviewer_id":77,"socket_directory":"{}","supervisor":{identity},"reviewer":{identity},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#,
        root.0.display(),
    )).unwrap();
    let recipe_error = crate::config::read_regular(&missing_recipe, 16_384).unwrap_err();
    for (mode, epoch, impossible) in [
        ("serve-create", 1, true),
        ("serve-create", 0, false),
        ("serve-open", 1, false),
    ] {
        fs::write(&precheck_path, disclosure_document(&root, epoch)).unwrap();
        let args = vec![
            mode.to_owned(),
            config_path.display().to_string(),
            actor_path.display().to_string(),
            reviewer_path.display().to_string(),
            "--native-text".into(),
            missing_recipe.display().to_string(),
            "--precheck-profile".into(),
            precheck_path.display().to_string(),
        ];
        let error = command(&args, None).unwrap_err();
        if impossible {
            assert_eq!(
                error,
                "native precheck creation requires initial policy epoch zero"
            );
        } else {
            // Valid creation and explicit recovery must pass this preflight
            // and reach the same original recipe reader. Recovery later binds
            // the selected epoch to the actual post-Fence owner, not zero.
            assert_eq!(error, recipe_error, "{mode} epoch {epoch}");
        }
        assert!(!config.store.exists());
        assert!(!source.exists());
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(actor.request).exists());
        assert!(!root.0.join("precheck.sock").exists());
    }
}
