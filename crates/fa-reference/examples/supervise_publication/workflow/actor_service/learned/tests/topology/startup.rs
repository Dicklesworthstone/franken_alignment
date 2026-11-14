//! A failed live startup must leave a usable original history/source pair.
//! Corrected invocations still execute real imported inference and fresh native
//! and human reviews; retained custody itself never authorizes publication.
use super::*;
use super::super::super::{operator as live_operator, run_with_actor_services};
use crate::workflow::actor_service::Profile;

#[path = "startup/resident.rs"]
mod resident;
#[path = "startup/interruptions.rs"]
mod interruptions;

#[derive(Clone)]
struct Archives {
    history: PathBuf,
    source: PathBuf,
}

impl Archives {
    fn named(root: &Root, name: &str) -> Self {
        Self {
            history: root.0.join(format!("{name}.anchor")),
            source: root.0.join(format!("{name}.source")),
        }
    }

    fn first(root: &Root) -> Self {
        Self {
            history: root.0.join("history-1.bin"),
            source: root.0.join("source-1.bin"),
        }
    }

    fn read(&self) -> TopologyCheckpoint {
        let saved = read_source(&self.source);
        assert_eq!(saved.history_anchor(), &anchors::load(&self.history).unwrap());
        saved
    }
}

fn read_source(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES,
    )
    .unwrap()
}

fn first(root: &Root, mode: Option<bool>) -> (Archives, FileDeliverySnapshot) {
    let (config, path, loaded) = selected(root, mode, false);
    run_peer(root, config, loaded, None, None);
    let config = configured(root);
    let loaded = recipe::load(&path, &config, false).unwrap();
    let state = observed(&config, &loaded);
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"A");
    assert!(state.stop.is_none());
    let input = Archives::first(root);
    assert_eq!(input.read().history_anchor().revision(), state.revision);
    (input, state)
}

fn write_next(root: &Root, generation: u64) {
    fs::write(
        root.0.join(format!("startup-live-{generation}.json")),
        graph(generation, false),
    )
    .unwrap();
    fs::write(
        root.0.join(format!("startup-inventory-{generation}.json")),
        INVENTORY,
    )
    .unwrap();
}

fn recovery_profile(
    root: &Root,
    input: &Archives,
    output: Option<&Archives>,
    generation: u64,
) -> PathBuf {
    let saved = input.read();
    let initial = pair(root, "initial.json", "initial-inventory.json");
    let next = pair(
        root,
        &format!("startup-live-{generation}.json"),
        &format!("startup-inventory-{generation}.json"),
    );
    let retention = output.map_or_else(String::new, |output| {
        format!(r#", "retain_checkpoint":"{}""#, output.source.display())
    });
    let path = root.0.join("startup-recovery.json");
    fs::write(&path, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{initial},"next":{next},"operation":{},"source_checkpoint":"{}"{retention}}}"#,
        saved.last_operation() + 1, input.source.display())).unwrap();
    path
}

fn load_recovery(
    root: &Root,
    config: &Config,
    request: u64,
    input: &Archives,
    output: &Archives,
    generation: u64,
) -> (PathBuf, Loaded) {
    let (path, mut loaded) = native::next(root, config, request);
    loaded.recovery_anchor = Some(anchors::load(&input.history).unwrap());
    loaded.anchor_output = Some(output.history.clone());
    let profile = recovery_profile(root, input, Some(output), generation);
    custody::admit(config, &mut loaded, Some(&profile), custody::Mode::Live).unwrap();
    (path, loaded)
}

fn park_live_inputs(root: &Root) -> Vec<(PathBuf, Vec<u8>)> {
    [
        "evidence.json",
        "native-roster.json",
        "helper-model.json",
        "helper-weights.safetensors",
        "helper-monitor.json",
        "helper-tokenizer.bbpe",
        "helper-sampling.json",
    ]
    .into_iter()
    .map(|name| {
        let path = root.0.join(name);
        let bytes = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        (path, bytes)
    })
    .collect()
}

fn restore_live_inputs(inputs: Vec<(PathBuf, Vec<u8>)>) {
    for (path, bytes) in inputs {
        fs::write(path, bytes).unwrap();
    }
}

fn assert_unchanged_effects(
    before: &FileDeliverySnapshot,
    after: &FileDeliverySnapshot,
    events: u64,
    revoked_epochs: u64,
) {
    assert_eq!(after.revision, before.revision + events);
    // The available graph is withdrawn before generic recovery revokes again;
    // an acknowledged source rebind is a further original revocation.
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + revoked_epochs);
    assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
    assert_eq!(after.control.ledger.available, before.control.ledger.available);
    assert_eq!(after.control.ledger.reserved, before.control.ledger.reserved);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.stages, before.control.ledger.stages);
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.target, before.target);
    assert!(after.stop.is_none());
}

fn assert_retained_cut(
    output: &Archives,
    original: &TopologyCheckpoint,
    state: &FileDeliverySnapshot,
    rebound: bool,
) -> TopologyCheckpoint {
    let saved = output.read();
    assert_eq!(saved.minimum().journal_revision, state.revision);
    assert_eq!(saved.minimum().control_sequence, state.control.sequence);
    assert_eq!(saved.minimum().authority_epoch, state.control.ledger.epoch);
    assert_eq!(saved.work(), original.work(), "startup cannot renew or spend source work");
    assert_eq!(saved.last_operation(), original.last_operation() + if rebound { 1 } else { 0 });
    assert_eq!(saved.available(), rebound);
    if rebound {
        assert_eq!(saved.expected().graph().spec().generation,
            original.expected().graph().spec().generation + 1);
        assert_ne!(saved.topology_bytes(), original.topology_bytes());
    } else {
        assert_eq!(saved.expected(), original.expected());
        assert_eq!(saved.topology_bytes(), original.topology_bytes());
        assert_eq!(saved.inventory_bytes(), original.inventory_bytes());
    }
    saved
}

fn operator(root: &Root, actor: &Profile) -> live_operator::Selection {
    let scope = actor.scope;
    let identity = actor.supervisor;
    let path = root.0.join("startup-operator.json");
    fs::write(&path, format!(r#"{{"schema":"fa.tower-operator-service/1","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}/peers/startup-operator.sock","supervisor":{{"uid":{},"gid":{},"pid":{}}},"operator":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":1,"connection_limit":1,"exchange_limit":2,"runtime_ms":60000,"poll_ms":1,"inspection_queries":1,"explanation_queries":0,"explanation_replay_events":0,"explanation_disclosure":"none"}}"#,
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        root.0.display(), identity.uid(), identity.gid(), identity.pid().unwrap(),
        identity.uid(), identity.gid(), identity.pid().unwrap())).unwrap();
    live_operator::Selection::read(&path, 91).unwrap()
}

fn assert_finished(root: &Root, recipe_path: &Path, final_pair: &Archives) {
    let config = configured(root);
    let mut loaded = recipe::load(recipe_path, &config, false).unwrap();
    let state = observed(&config, &loaded);
    assert_eq!((state.executions, state.payload.as_slice()), (3, b"AA".as_slice()));
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(final_pair.read().history_anchor().revision(), state.revision);
    loaded.recovery_anchor = Some(anchors::load(&final_pair.history).unwrap());
    let profile = recovery_profile(root, final_pair, None, 99);
    custody::admit(&config, &mut loaded, Some(&profile), custody::Mode::Historical).unwrap();
    let (host, _, _, _) = recovery::open_selected(&config, &loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (6, 4));
    assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert!(stream.confirmed.finished());
    assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.pending, None);
    let charge: u64 = (1..=3).map(|request| host.request_action(request).unwrap().spec().units).sum();
    assert_eq!(state.control.ledger.charged, charge);
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}

#[derive(Clone, Copy)]
enum Failure {
    MissingNextSource,
    WrongPredecessor,
    OperatorCollision,
}

#[test]
fn missing_next_topology_retains_the_fence_pair_for_corrected_learned_message_and_finish() {
    corrected_single(Failure::MissingNextSource);
}

#[test]
fn wrong_predecessor_retains_the_rebound_pair_for_corrected_learned_message_and_finish() {
    corrected_single(Failure::WrongPredecessor);
}

#[test]
fn late_operator_collision_retains_the_rebound_pair_for_corrected_learned_message_and_finish() {
    corrected_single(Failure::OperatorCollision);
}

fn corrected_single(failure: Failure) {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (input, before) = first(&root, mode);
        let original = input.read();
        let original_history = fs::read(&input.history).unwrap();
        let original_source = fs::read(&input.source).unwrap();
        let rebound = !matches!(failure, Failure::MissingNextSource);
        if rebound { write_next(&root, 2); }
        let output = Archives::named(&root, "failed-startup");
        let config = configured(&root);
        let (recipe_path, loaded) = load_recovery(&root, &config, 2, &input, &output, 2);
        let reader = recipe::load(&recipe_path, &config, false).unwrap();
        let actor = peers::actor_profile(&root, &config, 2);
        let reviewers = fixture::peers(&root, &config);
        let selected_operator = matches!(failure, Failure::OperatorCollision).then(|| operator(&root, &actor));
        let parked = park_live_inputs(&root);
        let mut calls = 0;
        let result = run_with_actor_services(config, loaded, &reviewers, true, None,
            Some(if matches!(failure, Failure::WrongPredecessor) { 999 } else { 1 }),
            None, Some(&actor), None, selected_operator.as_ref(), || {
                if calls == 0 {
                    if let Some(operator) = &selected_operator {
                        fs::write(&operator.settings.socket, b"independent operator path").unwrap();
                    }
                }
                calls += 1;
                ElapsedTick(1000)
            });
        let error = result.err().expect("selected startup boundary must refuse");
        match failure {
            Failure::MissingNextSource => assert!(error.contains("NotFound"), "{error}"),
            Failure::WrongPredecessor => assert!(error.contains("Missing"), "{error}"),
            Failure::OperatorCollision => {},
        }
        let config = configured(&root);
        let after = observed(&config, &reader);
        assert_unchanged_effects(&before, &after,
            if rebound { 2 } else { 1 }, if rebound { 3 } else { 2 });
        assert_retained_cut(&output, &original, &after, rebound);
        assert_eq!(fs::read(&input.history).unwrap(), original_history);
        assert_eq!(fs::read(&input.source).unwrap(), original_source);
        for (path, _) in &parked { assert!(!path.exists()); }
        if let Some(operator) = selected_operator {
            assert_eq!(fs::read(&operator.settings.socket).unwrap(), b"independent operator path");
            fs::remove_file(operator.settings.socket).unwrap();
        }
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        restore_live_inputs(parked);

        // The old exact source cut still refuses: a retained successor must be
        // selected explicitly, not rediscovered or replaced by a weaker open.
        let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
        let stale_output = Archives::named(&root, "stale-startup");
        let (_, stale) = load_recovery(&root, &config, 2, &input, &stale_output, 2);
        let stale = run_with_actor_services(config, stale, &reviewers, true, None, Some(1),
            None, Some(&actor), None, None, || ElapsedTick(1000));
        assert!(stale.is_err());
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
        assert!(!stale_output.history.exists() && !stale_output.source.exists());

        let next_generation = if rebound { 3 } else { 2 };
        write_next(&root, next_generation);
        let message_pair = Archives::named(&root, "corrected-message");
        let config = configured(&root);
        let (_, loaded) = load_recovery(&root, &config, 2, &output, &message_pair, next_generation);
        run_peer(&root, config, loaded, Some(1), None);
        assert!(message_pair.read().work().checks >= original.work().checks + 2);
        write_next(&root, next_generation + 1);
        let final_pair = Archives::named(&root, "corrected-finish");
        let config = configured(&root);
        let (last, loaded) = load_recovery(&root, &config, 3, &message_pair, &final_pair, next_generation + 1);
        run_peer(&root, config, loaded, None, Some(2));
        assert_finished(&root, &last, &final_pair);
        assert_eq!(fs::read(&input.history).unwrap(), original_history);
        assert_eq!(fs::read(&input.source).unwrap(), original_source);
    }
}

#[test]
fn post_fence_archive_races_preserve_the_other_custody_and_the_original_startup_failure() {
    for mode in [None, Some(false), Some(true)] {
        for race_source in [false, true] {
            let root = Root::new();
            let (input, before) = first(&root, mode);
            let original = input.read();
            let output = Archives::named(&root, "raced-startup");
            let config = configured(&root);
            let (path, loaded) = load_recovery(&root, &config, 2, &input, &output, 2);
            let reader = recipe::load(&path, &config, false).unwrap();
            let reviewers = fixture::peers(&root, &config);
            let actor = peers::actor_profile(&root, &config, 2);
            let canonical = config.store.join("delivery.bin");
            let bytes = fs::read(&canonical).unwrap();
            let raced = if race_source { &output.source } else { &output.history };
            let mut collision = false;
            let result = run_with_actor_services(config, loaded, &reviewers, true, None, Some(1),
                None, Some(&actor), None, None, || {
                    // Preflight succeeded before the exact one-Fence successor.
                    // The next-source read checkpoint supplies this causal race.
                    if !collision && fs::read(&canonical).unwrap() != bytes {
                        fs::write(raced, b"independent archive wins").unwrap();
                        collision = true;
                    }
                    ElapsedTick(1000)
                });
            let error = result.err().expect("missing next source must refuse");
            assert!(collision);
            assert!(error.contains("NotFound"), "{error}");
            assert!(error.contains("retention failed"), "{error}");
            assert_eq!(fs::read(raced).unwrap(), b"independent archive wins");
            let after = observed(&configured(&root), &reader);
            assert_unchanged_effects(&before, &after, 1, 2);
            if race_source {
                assert_eq!(anchors::load(&output.history).unwrap().revision(), after.revision);
            } else {
                let saved = read_source(&output.source);
                assert_eq!(saved.history_anchor().revision(), after.revision);
                assert_eq!(saved.work(), original.work());
                assert_eq!(saved.last_operation(), original.last_operation());
                assert_eq!(saved.expected(), original.expected());
                assert!(!saved.available());
            }
            assert_eq!(input.read(), original);
            assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        }
    }
}

#[test]
fn invalid_source_retention_destinations_refuse_before_the_fence_or_staging_cleanup() {
    let root = Root::new();
    let (input, before) = first(&root, None);
    let config = configured(&root);
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    let staging = config.store.join("delivery.pending");
    fs::write(&staging, b"preexisting unacknowledged evidence").unwrap();
    write_next(&root, 2);
    let alias = root.0.join("custody-alias");
    std::os::unix::fs::symlink(&root.0, &alias).unwrap();
    for choice in 0..4 {
        let mut output = Archives::named(&root, &format!("invalid-{choice}"));
        let expected = match choice {
            0 => {
                fs::write(&output.source, b"independent existing checkpoint").unwrap();
                "new file"
            }
            1 => {
                output.source = output.history.clone();
                "distinct"
            }
            2 => {
                output.source = alias.join(output.history.file_name().unwrap());
                "distinct"
            }
            3 => {
                output.source = config.store.join("forbidden.source");
                "outside"
            }
            _ => unreachable!(),
        };
        let config = configured(&root);
        let (path, loaded) = load_recovery(&root, &config, 2, &input, &output, 2);
        let reader = recipe::load(&path, &config, false).unwrap();
        let actor = peers::actor_profile(&root, &config, 2);
        let reviewers = fixture::peers(&root, &config);
        let result = run_with_actor_services(config, loaded, &reviewers, true, None, Some(1),
            None, Some(&actor), None, None, || ElapsedTick(1000));
        let error = result.err().expect("unsafe source custody cannot consume recovery");
        assert!(error.contains(expected), "{error}");
        assert_eq!(fs::read(root.0.join("store/delivery.bin")).unwrap(), canonical);
        assert_eq!(fs::read(&staging).unwrap(), b"preexisting unacknowledged evidence");
        assert_eq!(observed(&configured(&root), &reader), before);
        assert!(!output.history.exists());
        if choice == 0 {
            assert_eq!(fs::read(&output.source).unwrap(), b"independent existing checkpoint");
        } else {
            assert!(!output.source.exists());
        }
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}
