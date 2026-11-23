//! Real scheduled publication accumulates native evaluation on the original
//! owner; a read operator sees it without opening the actor's source.
use super::*;
use crate::recover_stop::guarded::SubmitProfile;
use crate::workflow::actor_service::{andon as service, andon_settings::Settings};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor,
    anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    evaluation::andon::FileNativeAndonSnapshot,
};
use std::io::Cursor;

fn settings() -> Settings {
    Settings::decode(include_bytes!(
        "../../../../../../fixtures/native_andon_operator.json"
    ))
    .unwrap()
}

fn floor(config: &Config) -> FileRecoveryFloor {
    let view = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    FileRecoveryFloor {
        journal_revision: view.revision,
        control_sequence: view.control.sequence,
        authority_epoch: view.control.ledger.epoch,
    }
}

fn inspect(config: &Config, settings: &Settings, path: &Path) -> FileNativeAndonSnapshot {
    let anchor = FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap();
    FileOversight::read_andon_guarded(
        &config.store,
        &config.profile,
        &settings.requirements(config, floor(config)).unwrap(),
        &settings.protocol,
        &anchor,
    )
    .unwrap()
}

fn recovery(config: &Config, input: &Path, output: Option<&Path>) -> SubmitProfile {
    let floor = floor(config);
    let output = output
        .map(|path| format!(",\"retain_history_anchor\":{path:?}"))
        .unwrap_or_default();
    SubmitProfile::decode(format!(r#"{{"schema_version":"fa.guarded-submit/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":{:?}{output}}}"#,
        floor.journal_revision, floor.control_sequence, floor.authority_epoch, input).as_bytes()).unwrap()
}

fn run_pair(root: &Directory) -> PathBuf {
    let config = configured(root);
    evidence(root, &config);
    let source = config.source.path().to_path_buf();
    let source_bytes = fs::read(&source).unwrap();
    fs::remove_file(&source).unwrap();
    let (mut actor, reviewer) = profiles(root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    let operator = crate::workflow::actor_service::operator::Settings::decode(
        super::operator::settings_document(root, &config, 0, 0)
            .replacen('{', "{\"governance_ledger\":{\"queries\":2,\"disclosure\":\"recorded_governance_accounting\"},", 1)
            .as_bytes(),
    ).unwrap();
    let ledger = super::operator::situation_document(&config)
        .replace("\"verb\":\"situation\"", "\"verb\":\"ledger\"");
    let first = super::series::document(&config, 7, 1, "6669727374");
    let second = super::series::document(&config, 8, 2, "7365636f6e64");
    let first_poll = super::series::poll(&config, 7);
    let second_poll = super::series::poll(&config, 8);
    let operator_path = operator.socket.clone();
    let actor_peer = actor.clone();
    let next_stop = crate::workflow::control::socket_path(&reviewer, 8);
    let peer = std::thread::spawn(move || {
        wait(&operator_path);
        let mut observer = UnixStream::connect(&operator_path).unwrap();
        let initial = raw_exchange(&mut observer, &ledger);
        let known = super::operator::known_operator(&initial, "governance-ledger");
        let accounting = field(field(known, "value"), "andon");
        assert_eq!(field(accounting, "state").as_str(), Some("known"));
        let credibility = field(field(known, "value"), "credibility");
        assert_eq!(
            field(field(credibility, "value"), "retained_cases").as_str(),
            Some("0")
        );
        assert!(
            !source.exists(),
            "ledger inspection must not bootstrap source"
        );
        fs::write(&source, source_bytes).unwrap();
        let mut socket = connect(&actor_peer);
        assert!(super::series::executed(&super::series::finish(
            &mut socket,
            &first,
            &first_poll
        )));
        wait(&next_stop);
        let pending = raw_exchange(&mut observer, &ledger);
        let known = super::operator::known_operator(&pending, "governance-ledger");
        let credibility = field(field(known, "value"), "credibility");
        assert_eq!(
            field(field(credibility, "value"), "retained_cases").as_str(),
            Some("1")
        );
        assert_eq!(
            field(field(credibility, "value"), "pending_cases").as_str(),
            Some("1")
        );
        assert!(super::series::executed(&super::series::finish(
            &mut socket,
            &second,
            &second_poll
        )));
    });
    let first_human = super::series::human_for(reviewer.clone(), 7);
    let second_human = super::series::human_for(reviewer.clone(), 8);
    let settings = settings();
    let output = root.0.join("created.anchor");
    service::serve(
        config,
        &actor,
        &reviewer,
        None,
        service::Selection {
            settings: &settings,
            mode: service::Mode::Create(&output),
            requests: &[7, 8],
            generation: GENERATION,
            operator: Some(&operator),
            evaluator: None,
            prechecks: None,
            inputs: &[],
        },
        || ElapsedTick(1000),
    )
    .unwrap();
    peer.join().unwrap();
    first_human.join().unwrap();
    second_human.join().unwrap();
    assert!(!actor.socket.exists());
    assert!(!operator.socket.exists());
    output
}

#[test]
fn live_andon_schedule_provisions_native_ledger_and_retains_two_original_cases() {
    let root = Directory::new();
    let output = run_pair(&root);
    let config = configured(&root);
    let state = inspect(&config, &settings(), &output);
    assert_eq!(state.credibility.journal.executions, 2);
    assert_eq!(state.credibility.journal.payload, b"second");
    assert_eq!(state.credibility.journal.control.ledger.charged, 14);
    assert_eq!(state.credibility.report.retained_cases, 2);
    assert_eq!(state.credibility.report.pending_cases, 2);
    assert_eq!(state.accounting.windows()[&1].false_stops, 0);
    assert!(state.receipts.is_empty());
}

#[test]
fn anchored_andon_recovery_keeps_cases_and_serves_exact_history_without_sources_or_helpers() {
    let root = Directory::new();
    let original = run_pair(&root);
    let mut config = configured(&root);
    let settings = settings();
    let before = inspect(&config, &settings, &original);
    config.programs.clear();
    fs::remove_file(config.source.path()).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let output = root.0.join("recovered.anchor");
    let recovery = recovery(&config, &original, Some(&output));
    let first = super::series::document(&config, 7, 1, "6669727374");
    let second = super::series::document(&config, 8, 2, "7365636f6e64");
    let actor_peer = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&actor_peer);
        assert!(super::series::executed(&raw_exchange(&mut socket, &first)));
        assert!(super::series::executed(&raw_exchange(&mut socket, &second)));
    });
    service::serve(
        config,
        &actor,
        &reviewer,
        None,
        service::Selection {
            settings: &settings,
            mode: service::Mode::Existing(&recovery),
            requests: &[7, 8],
            generation: GENERATION,
            operator: None,
            evaluator: None,
            prechecks: None,
            inputs: &[],
        },
        || ElapsedTick(200000),
    )
    .unwrap();
    peer.join().unwrap();
    let config = configured(&root);
    let after = inspect(&config, &settings, &output);
    assert_eq!(after.credibility.report, before.credibility.report);
    assert_eq!(after.accounting, before.accounting);
    assert_eq!(after.receipts, before.receipts);
    assert_eq!(after.credibility.journal.executions, 2);
    assert_eq!(after.credibility.journal.control.ledger.charged, 14);
    assert!(
        after.credibility.journal.control.ledger.epoch
            > before.credibility.journal.control.ledger.epoch
    );
    assert!(!config.source.path().exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn andon_custody_and_incompatible_cli_selections_refuse_before_any_profile_read() {
    let base = [
        "serve-create",
        "missing-config",
        "missing-actor",
        "missing-reviewer",
        "--tower-generation",
        "11",
        "--andon",
        "missing-andon",
    ];
    for extra in [
        vec![],
        vec![
            "--retain-history-anchor",
            "/new.anchor",
            "--topology-profile",
            "missing-topology",
        ],
        vec![
            "--retain-history-anchor",
            "/new.anchor",
            "--precheck-profile",
            "missing-precheck",
        ],
        vec![
            "--retain-history-anchor",
            "/new.anchor",
            "--governor-profile",
            "missing-governor",
        ],
        vec![
            "--retain-history-anchor",
            "/new.anchor",
            "--andon",
            "duplicate",
        ],
    ] {
        let args = base
            .into_iter()
            .chain(extra)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let error = crate::workflow::actor_service::tower::command_with_clock(&args, None, || {
            panic!("clock before CLI admission")
        })
        .unwrap_err();
        assert!(!error.contains("No such file"), "{error}");
    }
    let args = base
        .into_iter()
        .chain(["--retain-history-anchor", "/new.anchor"])
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let error = crate::workflow::actor_service::tower::command_with_clock(
        &args,
        Some(Path::new("missing-activation")),
        || panic!("clock before CLI admission"),
    )
    .unwrap_err();
    assert!(error.contains("credibility activation"));
}

#[test]
fn future_stop_or_missing_source_cannot_be_selected_as_the_creation_anchor() {
    for source_alias in [false, true] {
        let root = Directory::new();
        let config = configured(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let output = if source_alias {
            config.source.path().to_path_buf()
        } else {
            crate::workflow::control::socket_path(&reviewer, 8)
        };
        let error = service::serve(
            config,
            &actor,
            &reviewer,
            None,
            service::Selection {
                settings: &settings(),
                mode: service::Mode::Create(&output),
                requests: &[7, 8],
                generation: GENERATION,
                operator: None,
                evaluator: None,
                prechecks: None,
                inputs: &[],
            },
            || panic!("clock before custody admission"),
        )
        .unwrap_err();
        assert!(error.contains("distinct"), "{error}");
        assert!(!root.0.join("store").exists());
        assert!(!actor.socket.exists());
        assert!(!output.exists());
    }
}

#[test]
fn recovery_requires_successor_custody_and_refuses_a_changed_original_andon_contract() {
    let root = Directory::new();
    let original = run_pair(&root);
    let config = configured(&root);
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let without_output = recovery(&config, &original, None);
    let error = service::serve(
        config,
        &actor,
        &reviewer,
        None,
        service::Selection {
            settings: &settings(),
            mode: service::Mode::Existing(&without_output),
            requests: &[7, 8],
            generation: GENERATION,
            operator: None,
            evaluator: None,
            prechecks: None,
            inputs: &[],
        },
        || panic!("clock before successor custody"),
    )
    .unwrap_err();
    assert!(error.contains("successor history anchor"));
    let config = configured(&root);
    let output = root.0.join("changed.anchor");
    let recovery = recovery(&config, &original, Some(&output));
    let changed = Settings::decode(
        std::str::from_utf8(include_bytes!(
            "../../../../../../fixtures/native_andon_operator.json"
        ))
        .unwrap()
        .replace("\"cost_per_stop\": 7", "\"cost_per_stop\": 8")
        .as_bytes(),
    )
    .unwrap();
    assert!(
        service::serve(
            config,
            &actor,
            &reviewer,
            None,
            service::Selection {
                settings: &changed,
                mode: service::Mode::Existing(&recovery),
                requests: &[7, 8],
                generation: GENERATION,
                operator: None,
                evaluator: None,
                prechecks: None,
                inputs: &[],
            },
            || ElapsedTick(200000),
        )
        .is_err()
    );
    assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), before);
    assert!(!output.exists());
    assert!(!actor.socket.exists());
}

#[test]
fn pending_recovery_stop_keeps_native_accounting_without_live_fence_time_or_source_work() {
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::StopClientProgress;

    let root = Directory::new();
    let original = run_pair(&root);
    let mut config = configured(&root);
    let settings = settings();
    let before = inspect(&config, &settings, &original);
    config.programs.clear();
    fs::remove_file(config.source.path()).unwrap();
    let (actor, reviewer) = profiles(&root, &config);
    let output = root.0.join("stopped.anchor");
    let recovery = recovery(&config, &original, Some(&output));
    let mut calls = 0;
    let mut peer = None;
    let result = service::serve(
        config,
        &actor,
        &reviewer,
        None,
        service::Selection {
            settings: &settings,
            mode: service::Mode::Existing(&recovery),
            requests: &[7, 8],
            generation: GENERATION,
            operator: None,
            evaluator: None,
            prechecks: None,
            inputs: &[],
        },
        || {
            calls += 1;
            if calls == 2 {
                // Budget creation precedes listeners. This next safe checkpoint
                // follows their creation but precedes any historical replay.
                let mut client = reviewer.connect_stop_client(7).unwrap();
                peer = Some(std::thread::spawn(move || {
                    let started = Instant::now();
                    loop {
                        assert!(started.elapsed() < Duration::from_secs(10));
                        match client.step().unwrap() {
                            StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                            StopClientProgress::Complete => return client.receipt().unwrap(),
                            _ => pause(1),
                        }
                    }
                }));
            }
            ElapsedTick(200000)
        },
    );
    let error = result.expect_err("recovery Stop cannot return live custody");
    assert!(
        error.contains("native admission Stop acknowledged"),
        "{error}"
    );
    peer.expect("original recovery checkpoint").join().unwrap();
    let config = configured(&root);
    let after = inspect(&config, &settings, &output);
    let original = &before.credibility.journal;
    let stopped = &after.credibility.journal;
    assert_eq!(stopped.revision, original.revision + 2);
    assert_eq!(
        stopped.control.ledger.elapsed,
        original.control.ledger.elapsed
    );
    assert_eq!(stopped.executions, original.executions);
    assert_eq!(stopped.payload, original.payload);
    assert_eq!(
        stopped.control.ledger.charged,
        original.control.ledger.charged
    );
    assert_eq!(stopped.stop.as_ref().unwrap().request().operation, 7);
    assert_eq!(after.credibility.report, before.credibility.report);
    assert_eq!(after.accounting, before.accounting);
    assert_eq!(after.receipts, before.receipts);
    assert!(!config.source.path().exists());
    assert!(!actor.socket.exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 7).exists());
}

mod checked {
    use super::*;
    use fa_reference::ReadWitness;
    use fa_reference::action::consequence::delivery::persistent::observed::publication::{
        capture::{FileCaptureIdentity, FilePublicationCapture},
        witnesses::{FilePublicationInputs, FileWitnessInput},
    };
    use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
    use fa_reference::product_frontier::{
        FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
    };
    use fa_reference::witness::{
        AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry,
    };

    fn capture(config: &Config, attempt: u64, payload: &[u8], value: &[u8]) -> Vec<u8> {
        let mut target = config.profile.delivery.target;
        target.expected_version = attempt;
        let action = FrozenAction::freeze(ActionSpec {
            version: VERSION,
            scope: config.profile.delivery.scope,
            target: Some(target),
            payload: payload.to_vec(),
            required_witnesses: vec![ReadWitness::Exact {
                key: 7,
                value: Some(b"ok".to_vec()),
            }],
            policy_epoch: 0,
            deadline: ElapsedTick(100000),
            units: 7,
        })
        .unwrap();
        let key = ProjectionKey {
            source: 40,
            branch: action.spec().scope.branch,
            projection: 7,
            source_epoch: 1,
        };
        let close = TrustedClosingMarker {
            key,
            final_sequence: 1,
            marker_generation: 1,
        };
        let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
        frontiers
            .accept(key, FrontierStage::Authenticated, 1)
            .unwrap();
        frontiers.record_close(close).unwrap();
        let input = FileWitnessInput::new(
            attempt,
            attempt,
            20,
            AdapterDomainInput::new(
                DomainProjection::new(40, 1, key),
                DomainClosure::Closed(close),
            ),
            vec![SnapshotEntry::new(0, 1, value.to_vec()).unwrap()],
            &frontiers,
        )
        .unwrap();
        FilePublicationCapture::new(
            attempt,
            FileCaptureIdentity {
                source: 91,
                generation: attempt,
            },
            &action,
            FilePublicationInputs::new(Some(input), None),
        )
        .unwrap()
        .to_bytes()
        .unwrap()
    }

    fn write_capture(path: &Path, bytes: &[u8]) {
        let pending = path.with_extension("next");
        fs::write(&pending, bytes).unwrap();
        fs::rename(pending, path).unwrap();
    }

    #[test]
    fn accounted_checked_schedule_rechecks_each_effect_after_independent_human_approval() {
        for changed in [false, true] {
            let root = Directory::new();
            let config = configured(&root);
            evidence(&root, &config);
            let (actor, reviewer) = profiles(&root, &config);
            let original = root.0.join("original.bin");
            let current = root.0.join("current.bin");
            let publication = PublicationProfile::decode(format!(
                r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":{:?},"current":{:?},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}}]}}"#,
                original, current,
            ).as_bytes()).unwrap();
            let first_capture = capture(&config, 1, b"first", b"original");
            write_capture(&original, &first_capture);
            write_capture(&current, &first_capture);
            let next_capture = capture(&config, 2, b"second", b"original");
            let changed_capture = capture(&config, 2, b"second", b"changed");
            let first = super::super::series::document(&config, 7, 1, "6669727374");
            let second = super::super::series::document(&config, 8, 2, "7365636f6e64");
            let first_poll = super::super::series::poll(&config, 7);
            let second_poll = super::super::series::poll(&config, 8);
            let actor_peer = actor.clone();
            let next_stop = crate::workflow::control::socket_path(&reviewer, 8);
            let next_current = current.clone();
            let peer = std::thread::spawn(move || {
                let mut socket = connect(&actor_peer);
                assert!(super::super::series::executed(
                    &super::super::series::finish(&mut socket, &first, &first_poll,)
                ));
                wait(&next_stop);
                write_capture(&original, &next_capture);
                write_capture(&next_current, &next_capture);
                let reply = super::super::series::finish(&mut socket, &second, &second_poll);
                assert_eq!(
                    super::super::series::executed(&reply),
                    !changed,
                    "{reply:?}"
                );
            });
            let first_human = super::super::series::human_for(reviewer.clone(), 7);
            let second_reviewer = reviewer.clone();
            let second_human = std::thread::spawn(move || {
                let mut client = second_reviewer
                    .connect_client_waiting(8, Duration::from_secs(10))
                    .unwrap();
                let started = Instant::now();
                loop {
                    assert!(started.elapsed() < Duration::from_secs(10));
                    match client.step().unwrap() {
                        ReviewClientProgress::NeedsDecision => {
                            if changed {
                                write_capture(&current, &changed_capture);
                            }
                            client.respond(ReviewDecision::Approve).unwrap();
                        }
                        ReviewClientProgress::Complete => break,
                        _ => pause(1),
                    }
                }
            });
            let settings = settings();
            let output = root.0.join("checked.anchor");
            service::serve(
                config,
                &actor,
                &reviewer,
                Some(&publication),
                service::Selection {
                    settings: &settings,
                    mode: service::Mode::Create(&output),
                    requests: &[7, 8],
                    generation: GENERATION,
                    operator: None,
                    evaluator: None,
                    prechecks: None,
                    inputs: &[],
                },
                || ElapsedTick(1000),
            )
            .unwrap();
            peer.join().unwrap();
            first_human.join().unwrap();
            second_human.join().unwrap();
            let config = configured(&root);
            let anchor = FileHistoryAnchor::read_trusted(
                &mut Cursor::new(fs::read(output).unwrap()),
                MAX_HISTORY_ANCHOR_BYTES,
            )
            .unwrap();
            let expected = settings
                .requirements(&config, floor(&config))
                .unwrap()
                .require_publication(publication.simple_limits().unwrap())
                .unwrap();
            let state = FileOversight::read_andon_guarded(
                &config.store,
                &config.profile,
                &expected,
                &settings.protocol,
                &anchor,
            )
            .unwrap();
            assert_eq!(
                state.credibility.journal.executions,
                if changed { 1 } else { 2 }
            );
            assert_eq!(
                state.credibility.journal.control.ledger.charged,
                if changed { 7 } else { 14 }
            );
            assert_eq!(state.credibility.journal.control.ledger.reserved, 0);
            assert_eq!(state.credibility.report.retained_cases, 2);
            assert_eq!(state.credibility.report.pending_cases, 2);
            assert_eq!(state.accounting.windows()[&1].false_stops, 0);
        }
    }
}
