//! One native operator grant survives a finite schedule's actual publications.
//! Synthetic helper verdicts exercise control behavior, not model safety evidence.

use super::operator::{
    explain_document, known_operator, reason, settings_document, situation_document,
    situation_summary, store_files,
};
use super::series::{document, executed, finish, human_for, poll, run_pair, stop_request};
use super::*;
use crate::workflow::actor_service::operator::Settings;
use crate::workflow::actor_service::series::{self as service, Options};
use std::sync::mpsc;

fn connect_operator(settings: &Settings) -> UnixStream {
    wait(&settings.socket);
    let socket = UnixStream::connect(&settings.socket).unwrap();
    settings.supervisor.verify(&socket).unwrap();
    socket
}

fn counts(reply: &Json, expected: &[(&str, &str)]) {
    let summary = situation_summary(reply);
    for &(name, value) in expected {
        assert_eq!(field(summary, name).as_str(), Some(value), "{name}");
    }
}

fn confirmed_explanation<'a>(reply: &'a Json, request: &str) -> &'a Json {
    let observed = known_operator(reply, "explanation");
    let native = field(field(reply, "result"), "value");
    assert_eq!(
        field(native, "scope").as_str(),
        Some("historical_exact_policy_admission")
    );
    assert_eq!(field(native, "effect_authority").as_bool(), Some(false));
    assert_eq!(
        field(native, "current_eligibility").as_str(),
        Some("not_observed")
    );
    let value = field(observed, "value");
    assert_eq!(field(value, "initial_state").as_str(), Some("reviewing"));
    assert_eq!(field(value, "ending_state").as_str(), Some("confirmed"));
    assert_eq!(field(value, "result").as_str(), Some("satisfied"));
    assert_eq!(
        field(value, "witness_values_disclosed").as_bool(),
        Some(false)
    );
    let basis = field(observed, "basis");
    assert_eq!(field(basis, "request").as_str(), Some(request));
    basis
}

#[test]
fn operator_retains_original_admission_and_finite_grants_across_two_publications() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.connections = 1;
    actor.candidates = 1;
    actor.reply_ms = 1000;
    let mut settings =
        Settings::decode(settings_document(&root, &config, 3, 3).as_bytes()).unwrap();
    settings.connection_limit = 2;
    settings.candidate_limit = 2;
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64");
    let first_poll = poll(&config, 7);
    let second_poll = poll(&config, 8);
    let situation = situation_document(&config);
    let explanation = explain_document(&config);
    let store = config.store.clone();
    let source = root.0.join("evidence.json");
    let next_control = crate::workflow::control::socket_path(&reviewer, 8);
    let profile = actor.clone();
    let operator_profile = settings.clone();
    let (waiting_send, waiting_receive) = mpsc::channel();
    let (approve_send, approve_receive) = mpsc::channel();
    let human_profile = reviewer.clone();
    let second_human = std::thread::spawn(move || {
        let mut client = human_profile
            .connect_client_waiting(8, Duration::from_secs(10))
            .unwrap();
        let started = Instant::now();
        let mut offered = false;
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    assert!(!offered);
                    offered = true;
                    waiting_send.send(()).unwrap();
                    approve_receive
                        .recv_timeout(Duration::from_secs(10))
                        .unwrap();
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return,
                _ => pause(1),
            }
        }
    });
    let first_human = human_for(reviewer.clone(), 7);
    let peer = std::thread::spawn(move || {
        // One unit from each read grant is spent before any admission. The same
        // operator connection must survive helper reaping and the handoff.
        let mut operator = connect_operator(&operator_profile);
        let initial = store_files(&store);
        counts(
            &raw_exchange(&mut operator, &situation),
            &[("attempts", "0")],
        );
        let missing = raw_exchange(&mut operator, &explanation);
        assert_eq!(
            field(field(&missing, "result"), "kind").as_str(),
            Some("explanation")
        );
        assert_eq!(
            field(
                field(field(field(&missing, "result"), "value"), "knowledge"),
                "state"
            )
            .as_str(),
            Some("unknown")
        );
        assert_eq!(store_files(&store), initial);

        let mut actor = connect(&profile);
        assert!(executed(&finish(&mut actor, &first, &first_poll)));
        // This endpoint is installed only after the first helper children have
        // been confirmed reaped by the original finite schedule.
        wait(&next_control);
        let admitted = raw_exchange(&mut actor, &second);
        assert_eq!(
            field(knowledge(&admitted), "state").as_str(),
            Some("pending")
        );
        waiting_receive
            .recv_timeout(Duration::from_secs(10))
            .unwrap();
        let source_bytes = fs::read(&source).unwrap();
        fs::remove_file(&source).unwrap();

        counts(
            &raw_exchange(&mut operator, &situation),
            &[
                ("executions", "1"),
                ("charged_units", "7"),
                ("reserved_units", "0"),
                ("retained_requests", "2"),
                ("attempts", "2"),
                ("pending_attempts", "1"),
                ("terminal_attempts", "1"),
            ],
        );
        let held_explanation = raw_exchange(&mut operator, &explanation);
        let original_basis = confirmed_explanation(&held_explanation, "7");
        assert_eq!(field(original_basis, "attempt").as_str(), Some("1"));
        assert!(executed(&raw_exchange(&mut actor, &first_poll)));
        assert!(!source.exists());
        // Human offer polling does not drive a source refresh. Restore the
        // exact evidence before approval: authorization, dispatch and first
        // publication each require their original fresh file observations.
        fs::write(&source, source_bytes).unwrap();
        approve_send.send(()).unwrap();
        assert!(executed(&finish(&mut actor, &second, &second_poll)));

        fs::remove_file(&source).unwrap();
        let settled = store_files(&store);
        counts(
            &raw_exchange(&mut operator, &situation),
            &[
                ("executions", "2"),
                ("charged_units", "14"),
                ("reserved_units", "0"),
                ("attempts", "2"),
                ("pending_attempts", "0"),
                ("terminal_attempts", "2"),
            ],
        );
        let terminal_explanation = raw_exchange(&mut operator, &explanation);
        let ending_basis = confirmed_explanation(&terminal_explanation, "7");
        for key in [
            "scope",
            "request",
            "attempt",
            "admission_revision",
            "admission_control_sequence",
            "admitted_at",
        ] {
            assert_eq!(
                field(ending_basis, key),
                field(original_basis, key),
                "{key}"
            );
        }
        assert_eq!(
            field(
                known_operator(&terminal_explanation, "explanation"),
                "value"
            ),
            field(known_operator(&held_explanation, "explanation"), "value")
        );
        let before = field(original_basis, "journal_revision")
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let after = field(ending_basis, "journal_revision")
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert!(
            after > before,
            "the original admission must carry the later owner cut"
        );

        // Handoff, terminal grace and reconnect do not renew either native
        // grant. Exhaustion still leaves the successful effects untouched.
        drop(operator);
        let mut exhausted = connect_operator(&operator_profile);
        reason(&raw_exchange(&mut exhausted, &situation), "BudgetExhausted");
        reason(
            &raw_exchange(&mut exhausted, &explanation),
            "BudgetExhausted",
        );
        assert_eq!(store_files(&store), settled);
        assert!(!source.exists());
    });
    service::serve_tower(
        config,
        &actor,
        &reviewer,
        None,
        Options {
            requests: &[7, 8],
            open: false,
            credibility: None,
        },
        GENERATION,
        Some(&settings),
        || ElapsedTick(1000),
    )
    .unwrap();
    peer.join().unwrap();
    first_human.join().unwrap();
    second_human.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 2);
    assert_eq!(disk.payload, b"second");
    assert_eq!(
        disk.target.expected_version,
        config.profile.delivery.target.expected_version + 2
    );
    assert_eq!(
        (disk.control.ledger.charged, disk.control.ledger.reserved),
        (14, 0)
    );
    assert_eq!(
        disk.control.ledger.available,
        config.profile.delivery.total - 14
    );
    assert_eq!(disk.control.ledger.stages.len(), 2);
    assert!(disk.stop.is_none());
    assert!(!actor.socket.exists());
    assert!(!settings.socket.exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn stop_drains_partial_second_actor_and_operator_frames_without_refunding_the_first_effect() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let settings = Settings::decode(settings_document(&root, &config, 2, 0).as_bytes()).unwrap();
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64");
    let first_poll = poll(&config, 7);
    let situation = situation_document(&config);
    let source = root.0.join("evidence.json");
    let profile = actor.clone();
    let operator_profile = settings.clone();
    let control = reviewer.clone();
    let peer = std::thread::spawn(move || {
        let mut operator = connect_operator(&operator_profile);
        counts(
            &raw_exchange(&mut operator, &situation),
            &[("attempts", "0")],
        );
        let mut actor = connect(&profile);
        assert!(executed(&finish(&mut actor, &first, &first_poll)));
        wait(&crate::workflow::control::socket_path(&control, 8));
        counts(
            &raw_exchange(&mut operator, &situation),
            &[
                ("executions", "1"),
                ("charged_units", "7"),
                ("attempts", "1"),
            ],
        );
        fs::remove_file(&source).unwrap();
        operator
            .write_all(&situation.as_bytes()[..situation.len() / 2])
            .unwrap();
        actor
            .write_all(&second.as_bytes()[..second.len() / 2])
            .unwrap();
        stop_request(&control, 8);
    });
    let human = human_for(reviewer.clone(), 7);
    service::serve_tower(
        config,
        &actor,
        &reviewer,
        None,
        Options {
            requests: &[7, 8],
            open: false,
            credibility: None,
        },
        GENERATION,
        Some(&settings),
        || ElapsedTick(1000),
    )
    .unwrap();
    peer.join().unwrap();
    human.join().unwrap();
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.payload, b"first");
    assert_eq!(
        (disk.control.ledger.charged, disk.control.ledger.reserved),
        (7, 0)
    );
    assert_eq!(
        disk.control.ledger.available,
        config.profile.delivery.total - 7
    );
    assert_eq!(disk.control.ledger.stages.len(), 1);
    assert_eq!(disk.stop.unwrap().request().operation, 8);
    assert!(disk.control.suspended);
    assert!(!root.0.join("evidence.json").exists());
    assert!(!actor.socket.exists());
    assert!(!settings.socket.exists());
    assert!(!reviewer.socket(8).exists());
}

#[test]
fn operator_collisions_with_later_review_and_stop_sockets_refuse_before_time_or_store() {
    for stop_collision in [false, true] {
        let root = Directory::new();
        let config = configured(&root);
        let store = config.store.clone();
        let (actor, reviewer) = profiles(&root, &config);
        let mut settings =
            Settings::decode(settings_document(&root, &config, 1, 0).as_bytes()).unwrap();
        settings.socket = if stop_collision {
            crate::workflow::control::socket_path(&reviewer, 8)
        } else {
            reviewer.socket(8)
        };
        let result = service::serve_tower(
            config,
            &actor,
            &reviewer,
            None,
            Options {
                requests: &[7, 8],
                open: false,
                credibility: None,
            },
            GENERATION,
            Some(&settings),
            || panic!("later scheduled socket collision entered the clock or source boundary"),
        );
        assert!(result.is_err());
        assert!(!store.exists());
        assert!(!root.0.join("evidence.json").exists());
        assert!(!actor.socket.exists());
        assert!(!settings.socket.exists());
        for request in [7, 8] {
            assert!(!reviewer.socket(request).exists());
            assert!(!crate::workflow::control::socket_path(&reviewer, request).exists());
        }
    }
}

#[test]
fn recorded_schedule_serves_original_operator_history_without_source_helpers_or_effect_replay() {
    let root = Directory::new();
    run_pair(&root);
    let mut config = configured(&root);
    config.programs.clear();
    let (mut actor, reviewer) = profiles(&root, &config);
    actor.reply_ms = 1000;
    let settings = Settings::decode(settings_document(&root, &config, 2, 2).as_bytes()).unwrap();
    let first = document(&config, 7, 1, "6669727374");
    let second = document(&config, 8, 2, "7365636f6e64");
    let situation = situation_document(&config);
    let first_explanation = explain_document(&config);
    let second_explanation = first_explanation.replace("-7@", "-8@");
    let source = root.0.join("evidence.json");
    let missing = root.0.join("must-not-read-activation.capsule");
    let store = config.store.clone();
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let profile = actor.clone();
    let operator_profile = settings.clone();
    let review_paths = [reviewer.socket(7), reviewer.socket(8)];
    let peer = std::thread::spawn(move || {
        let mut operator = connect_operator(&operator_profile);
        // A historical schedule must service the operator even while waiting
        // for its first actor frame. Operator traffic is not an actor command.
        counts(
            &raw_exchange(&mut operator, &situation),
            &[
                ("executions", "2"),
                ("charged_units", "14"),
                ("terminal_attempts", "2"),
            ],
        );
        pause(profile.reply_ms + 25);
        let mut actor = connect(&profile);
        assert!(executed(&raw_exchange(&mut actor, &first)));
        assert!(executed(&raw_exchange(&mut actor, &second)));
        // Opening has already performed its original recovery work. The two
        // historical admissions and operator reads cannot write this image.
        let recovered = store_files(&store);
        let first = raw_exchange(&mut operator, &first_explanation);
        let first_basis = confirmed_explanation(&first, "7");
        let second = raw_exchange(&mut operator, &second_explanation);
        let second_basis = confirmed_explanation(&second, "8");
        assert_ne!(
            field(first_basis, "attempt"),
            field(second_basis, "attempt")
        );
        let first_revision = field(first_basis, "admission_revision")
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let second_revision = field(second_basis, "admission_revision")
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert!(first_revision < second_revision);
        counts(
            &raw_exchange(&mut operator, &situation),
            &[
                ("executions", "2"),
                ("charged_units", "14"),
                ("reserved_units", "0"),
                ("attempts", "2"),
                ("pending_attempts", "0"),
                ("terminal_attempts", "2"),
            ],
        );
        reason(&raw_exchange(&mut operator, &situation), "BudgetExhausted");
        reason(
            &raw_exchange(&mut operator, &first_explanation),
            "BudgetExhausted",
        );
        assert_eq!(store_files(&store), recovered);
        assert!(!source.exists());
        assert!(review_paths.iter().all(|path| !path.exists()));
    });
    service::serve_tower(
        config,
        &actor,
        &reviewer,
        None,
        Options {
            requests: &[7, 8],
            open: true,
            credibility: Some(&missing),
        },
        GENERATION,
        Some(&settings),
        || ElapsedTick(200000),
    )
    .unwrap();
    peer.join().unwrap();
    let config = configured(&root);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.target, before.target);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.stages.len(), 2);
    assert!(after.stop.is_none());
    assert!(!missing.exists());
    assert!(!actor.socket.exists());
    assert!(!settings.socket.exists());
    assert!(!reviewer.socket(7).exists());
    assert!(!reviewer.socket(8).exists());
}
