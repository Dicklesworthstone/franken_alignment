//! Independently retained native cuts around actual publication and recovery.
//! Synthetic same-process helper/peer fixtures do not prove hostile containment.
use super::*;
use crate::workflow::actor_service::learned::anchors as archive;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::FileHistoryAnchor;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::{
    StopClientProgress, StopControlReceipt,
};
use std::path::{Path, PathBuf};

fn selection(
    config: &Config,
    input: Option<&Path>,
    output: Option<&Path>,
    open: bool,
) -> anchors::Selection {
    let mut args = vec![
        if open { "serve-open".to_owned() } else { "serve-create".to_owned() },
        "config".into(), "actor".into(), "reviewer".into(),
        "--native-text".into(), "recipe".into(),
    ];
    if let Some(path) = input {
        args.extend(["--history-anchor".into(), path.display().to_string()]);
    }
    if let Some(path) = output {
        args.extend(["--retain-history-anchor".into(), path.display().to_string()]);
    }
    let (_, options) = anchors::take_options(&args).unwrap();
    options.admit(config, open).unwrap().unwrap()
}

fn publish_retained(root: &Root, output_path: &Path, race: bool) -> (Vec<u8>, Result<(), String>) {
    let config = configured(root);
    let native = inputs(root, &config);
    let (actor, reviewer) = profiles(root, &config);
    let history = selection(&config, None, Some(output_path), false);
    let store = config.store.clone();
    let profile = config.profile.clone();
    let (sender, receiver) = mpsc::channel();
    let client = actor_thread(actor.clone(), receiver);
    let human = reviewing(reviewer.clone(), store.clone(), profile.clone(), ReviewDecision::Approve);
    let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    let mut raced = false;
    let result = serve_lifecycle_with_history(
        config, (&actor, &reviewer), native,
        (false, None, None, finish::Intent::Existing),
        (None, None, Some(&history)),
        || {
            if race && !raced && FileOversight::read_publication(&store, &profile)
                .is_ok_and(|snapshot| snapshot.executions == 1)
            {
                std::fs::write(output_path, b"independent archive wins the race").unwrap();
                raced = true;
            }
            ElapsedTick(1000)
        },
        &mut output,
    );
    human.join().unwrap();
    assert!(matches!(client.join().unwrap().result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    if race { assert!(raced, "race must follow an acknowledged real publication"); }
    (output.bytes, result)
}

#[test]
fn native_history_create_and_expired_receipt_keep_one_publication_and_successor_archives() {
    let root = Root::new();
    let first = root.0.join("first.anchor");
    let second = root.0.join("receipt.anchor");
    let (original, result) = publish_retained(&root, &first, false);
    result.unwrap();
    let first_bytes = std::fs::read(&first).unwrap();
    let prior = archive::load(&first).unwrap();
    let mut config = configured(&root);
    let native = inputs(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let history = selection(&config, Some(&first), Some(&second), true);
    let store = config.store.clone();
    let profile = config.profile.clone();
    let decoder = native.decoder.clone();
    let tokenizer = native.tokenizer.clone();
    let before = FileOversight::read_decoder_text_progress(
        &store, &profile, &decoder, &tokenizer, 7,
    ).unwrap();
    assert_eq!(prior.revision(), before.publication.revision);
    // A historical receipt has no live source or helper dependency and never
    // reissues its effect, even after the original request deadline.
    config.programs.clear();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (sender, receiver) = mpsc::channel();
    let client = actor_thread(actor.clone(), receiver);
    let mut output = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
    serve_lifecycle_with_history(
        config, (&actor, &reviewer), native,
        (true, None, None, finish::Intent::Existing),
        (None, None, Some(&history)), || ElapsedTick(20_000), &mut output,
    ).unwrap();
    assert!(matches!(client.join().unwrap().result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let after = FileOversight::read_decoder_text_progress(
        &store, &profile, &decoder, &tokenizer, 7,
    ).unwrap();
    assert_eq!(output.bytes, original);
    assert_eq!(after.publication.executions, 1);
    assert_eq!(after.publication.payload, before.publication.payload);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(archive::load(&second).unwrap().revision(), after.publication.revision);
    assert!(archive::load(&second).unwrap().revision() > prior.revision());
    assert_eq!(std::fs::read(&first).unwrap(), first_bytes);
    assert!(!reviewer.socket(91).exists());
    assert!(!crate::workflow::control::socket_path(&reviewer, 91).exists());
}

#[test]
fn native_history_output_race_preserves_real_execution_and_never_overwrites_or_retries() {
    let root = Root::new();
    let output = root.0.join("raced.anchor");
    let (_, result) = publish_retained(&root, &output, true);
    let error = result.unwrap_err();
    assert!(error.contains("native history retention failed"), "{error}");
    assert!(error.contains("no effect retry"), "{error}");
    assert_eq!(std::fs::read(&output).unwrap(), b"independent archive wins the race");
    let config = configured(&root);
    let snapshot = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(snapshot.executions, 1);
    assert_eq!(snapshot.payload, b"A");
    assert!(snapshot.control.ledger.charged > 0);
    assert!(snapshot.stop.is_none(), "retention failure must not issue a new Stop");
}

fn seeded(root: &Root, steps: u64) -> (Config, Inputs, FileHistoryAnchor) {
    let mut config = configured(root);
    let native = inputs(root, &config);
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let (mut host, _) = bootstrap::prepare(&config, &native, selected, false).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        native.generation, numerical.actor_revision, numerical.position, native.request.clone(),
    ).unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    for step in 0..steps {
        host.advance_decoder_generation(host.revision(), native.generation, step).unwrap();
    }
    let anchor = host.history_anchor().unwrap();
    drop(host);
    config.programs = native_programs_at_epoch(root, 1);
    (config, native, anchor)
}

#[test]
fn native_history_startup_refusal_retains_the_original_fence_without_new_work() {
    let root = Root::new();
    let (config, mut native, prior) = seeded(&root, 1);
    let input = root.0.join("before.anchor");
    let output = root.0.join("refused.anchor");
    archive::export_acknowledged(&prior, &input).unwrap();
    let history = selection(&config, Some(&input), Some(&output), true);
    let (actor, reviewer) = profiles(&root, &config);
    let store = config.store.clone();
    let profile = config.profile.clone();
    let decoder = native.decoder.clone();
    let tokenizer = native.tokenizer.clone();
    let before = FileOversight::read_decoder_text_progress(
        &store, &profile, &decoder, &tokenizer, 7,
    ).unwrap();
    // Recipe selection fails only after the exact owner has recovered and
    // acknowledged its original Fence. That successor must remain retainable.
    native.request.prompt = b"changed original prompt".to_vec();
    let mut document = Vec::new();
    let error = serve_lifecycle_with_history(
        config, (&actor, &reviewer), native,
        (true, None, None, finish::Intent::Existing),
        (None, None, Some(&history)), || ElapsedTick(1001), &mut document,
    ).unwrap_err();
    assert!(!error.contains("history retention failed"), "{error}");
    assert!(document.is_empty());
    let saved = archive::load(&output).unwrap();
    let after = FileOversight::read_decoder_text_progress(
        &store, &profile, &decoder, &tokenizer, 7,
    ).unwrap();
    assert_eq!(saved.revision(), prior.revision() + 1);
    assert_eq!(saved.revision(), after.publication.revision);
    assert_eq!(after.text.generation_revision(), 1);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.publication.executions, 0);
    assert!(after.publication.stop.is_none());
    assert!(!reviewer.socket(91).exists());
}

#[test]
fn native_history_rejects_rollback_before_replay_fence_or_archive_output() {
    let root = Root::new();
    let (config, native, prior) = seeded(&root, 1);
    let old = std::fs::read(config.store.join("delivery.bin")).unwrap();
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let (host, _) = FileOversight::open_generated_text_stream_from_source_anchored(
        &config.store, config.profile.clone(), &native.decoder, &native.tokenizer, selected, &prior,
    ).unwrap();
    let newer = host.history_anchor_after(&prior).unwrap();
    drop(host);
    let input = root.0.join("newer.anchor");
    let output = root.0.join("refused.anchor");
    archive::export_acknowledged(&newer, &input).unwrap();
    std::fs::write(config.store.join("delivery.bin"), &old).unwrap();
    let history = selection(&config, Some(&input), Some(&output), true);
    let (actor, reviewer) = profiles(&root, &config);
    let store = config.store.clone();
    let mut calls = 0;
    let mut document = Vec::new();
    let error = serve_lifecycle_with_history(
        config, (&actor, &reviewer), native,
        (true, None, None, finish::Intent::Existing),
        (None, None, Some(&history)),
        || { calls += 1; ElapsedTick(1001) }, &mut document,
    ).unwrap_err();
    assert!(error.contains("Stale"), "{error}");
    assert_eq!(calls, 2, "no historical event checkpoint may run after a bad anchor");
    assert_eq!(std::fs::read(store.join("delivery.bin")).unwrap(), old);
    assert!(!output.exists());
    assert!(document.is_empty());
}

fn stopping(reviewer: &PeerProfile, operation: u64) -> std::thread::JoinHandle<StopControlReceipt> {
    let mut client = reviewer.connect_stop_client(operation).unwrap();
    std::thread::spawn(move || {
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => pause(1),
            }
        }
    })
}

#[test]
fn native_history_retains_authenticated_stop_even_after_acknowledgment_deadline_failure() {
    for late in [false, true] {
        let root = Root::new();
        let (mut config, native, prior) = seeded(&root, 1);
        let input = root.0.join("before.anchor");
        let output = root.0.join("stopped.anchor");
        archive::export_acknowledged(&prior, &input).unwrap();
        let history = selection(&config, Some(&input), Some(&output), true);
        let (_, reviewer) = profiles(&root, &config);
        let mut control = Control::new(&config, 91, Some(&reviewer)).unwrap();
        let selected = bootstrap::selection(&config, &native, None).unwrap();
        let store = config.store.clone();
        let profile = config.profile.clone();
        config.programs.clear();
        std::fs::remove_file(root.0.join("evidence.json")).unwrap();
        let client = stopping(&reviewer, 91);
        let deadline = Deadline {
            logical: ElapsedTick(10_000),
            started: Instant::now(), wall: Duration::from_secs(10),
        };
        let error = bootstrap::open_with_history(
            &config, &native, selected, Some(&history), Some(&mut control), &deadline,
            &mut || {
                let stopped = FileOversight::read_publication(&store, &profile)
                    .is_ok_and(|snapshot| snapshot.stop.is_some());
                ElapsedTick(if late && stopped { 20_000 } else { 1001 })
            },
        ).err().expect("terminal recovery may not issue live custody");
        assert!(error.starts_with("native admission Stop acknowledged"), "{error}");
        if late { assert!(error.contains("after native recovery Stop application"), "{error}"); }
        assert!(client.join().unwrap().acknowledged());
        let after = FileOversight::read_publication(&store, &profile).unwrap();
        let retained = archive::load(&output).unwrap();
        assert_eq!(after.revision, prior.revision() + 2);
        assert_eq!(retained.revision(), after.revision);
        assert_eq!(after.executions, 0);
        assert_eq!(after.control.ledger.charged, 0);
        assert_eq!(after.stop.unwrap().request().operation, 91);
        assert!(control.recovery_pending_peer().is_err());
        assert_eq!(archive::load(&output).unwrap(), retained);
    }
}

#[test]
fn native_history_closed_grammar_and_canonical_endpoint_aliases_refuse_before_startup() {
    let valid = ["serve-open", "config", "actor", "reviewer", "--history-anchor", "prior",
        "--native-text", "recipe", "--retain-history-anchor", "next",
        "--after-message", "91"].map(str::to_owned);
    let (remaining, _) = anchors::take_options(&valid).unwrap();
    assert_eq!(remaining, ["serve-open", "config", "actor", "reviewer",
        "--native-text", "recipe", "--after-message", "91"].map(str::to_owned));
    for suffix in [
        vec!["--history-anchor", "prior"],
        vec!["--history-anchor", "prior", "--history-anchor", "other"],
        vec!["--retain-history-anchor", "next", "--resume"],
        vec!["--retain-history-anchor", "next", "--unexpected", "yes"],
        vec!["--retain-history-anchor", "../outside"],
    ] {
        let mut args = vec!["serve-create", "config", "actor", "reviewer", "--native-text", "recipe"];
        args.extend(suffix);
        assert!(anchors::take_options(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
    }
    let root = Root::new();
    let config = configured(&root);
    let (actor, reviewer) = profiles(&root, &config);
    let permitted = selection(&config, None, Some(&root.0.join("new.anchor")), false);
    permitted.check(&config, &[(&actor, None)], &reviewer, None, false).unwrap();
    std::os::unix::fs::symlink(&root.0, root.0.join("alias")).unwrap();
    let endpoints: Vec<PathBuf> = vec![
        actor.socket.clone(), reviewer.socket(actor.request),
        crate::workflow::control::socket_path(&reviewer, actor.request),
        root.0.join("alias").join(actor.socket.file_name().unwrap()),
    ];
    for endpoint in endpoints {
        let selected = selection(&config, None, Some(&endpoint), false);
        assert!(selected.check(&config, &[(&actor, None)], &reviewer, None, false).is_err());
    }
    assert!(!config.store.exists());
    assert!(!actor.socket.exists());
    assert!(!root.0.join("new.anchor").exists());
}

#[test]
fn native_history_faulted_owner_never_exports_its_pre_failure_capture() {
    let root = Root::new();
    let config = configured(&root);
    let native = inputs(&root, &config);
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let (mut host, _) = bootstrap::prepare(&config, &native, selected, false).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let output = root.0.join("faulted.anchor");
    let history = selection(&config, None, Some(&output), false);
    let prior = history.capture(&host).unwrap().unwrap();
    let previous = root.0.join("previous.anchor");
    archive::export_acknowledged(&prior, &previous).unwrap();
    let retained_bytes = std::fs::read(&previous).unwrap();
    let canonical = std::fs::read(config.store.join("delivery.bin")).unwrap();
    let pending = config.store.join("delivery.pending");
    std::fs::write(&pending, b"retained ambiguous staging").unwrap();
    // The original writer refuses occupied staging and poisons the live owner.
    // A cached pre-work capture may never be published as this failed attempt.
    assert!(matches!(
        host.observe_time(host.revision(), ElapsedTick(1001)),
        Err(JournalError::Io(_))
    ));
    let error = history.retain(&host, Ok(())).unwrap_err();
    assert!(error.contains("Unavailable"), "{error}");
    assert!(error.contains("history retention failed"), "{error}");
    assert!(!output.exists());
    assert_eq!(std::fs::read(&previous).unwrap(), retained_bytes);
    assert_eq!(std::fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(std::fs::read(&pending).unwrap(), b"retained ambiguous staging");
}
