//! Original imported inference, authenticated actor/human peers and native cuts.
//! Synthetic weights and independently written graphs exercise enforcement and
//! custody; they do not establish deployment inventory or detector quality.
use super::*;
use super::continuation as native;
use super::peer_intake as peers;
use super::super::{anchors, prediction, topology as custody};
use fa_reference::action::consequence::delivery::{EndpointOutcome,
    persistent::{FileDeliverySnapshot, observed::{
        driver::topology::checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES},
        mediation::source::TOPOLOGY_REFRESH_READ_BYTES,
        reviewer::wire::ReviewDecision,
    }}};
use fa_reference::action::consequence::mediation::MAX_CHECK_EDGE_VISITS;
use std::{io::Cursor, path::{Path, PathBuf}};

const INVENTORY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"));

fn graph(generation: u64, bypass: bool) -> String {
    let extra = if bypass { r#",{"id":3,"from":1,"to":4,"channel":"dispatch","route":"publication","provenance":103}"# } else { "" };
    let reads = 32 * TOPOLOGY_REFRESH_READ_BYTES;
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
        "complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
            {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}{extra}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":32,"read_bytes":{reads}}}}}"#)
}

fn pair(root: &Root, topology: &str, inventory: &str) -> String {
    format!(r#"{{"topology_path":"{}","inventory_path":"{}"}}"#,
        root.0.join(topology).display(), root.0.join(inventory).display())
}

fn selected(root: &Root, mode: Option<bool>, bypass: bool) -> (Config, PathBuf, Loaded) {
    let config = configured(root); let path = native::write(root);
    if let Some(owned) = mode { super::forecast::install(root, &path, owned, 60_000); }
    evidence(root, &config, 1, true, false);
    for name in ["initial.json", "live-1.json"] { fs::write(root.0.join(name), graph(1, bypass)).unwrap(); }
    for name in ["initial-inventory.json", "inventory-1.json"] { fs::write(root.0.join(name), INVENTORY).unwrap(); }
    let profile = root.0.join("create-topology.json");
    fs::write(&profile, format!(r#"{{"schema_version":"fa.mediated-create/2","topology_path":"{}","inventory_path":"{}","retain_checkpoint":"{}"}}"#,
        root.0.join("live-1.json").display(), root.0.join("inventory-1.json").display(),
        root.0.join("source-1.bin").display())).unwrap();
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    loaded.anchor_output = Some(root.0.join("history-1.bin"));
    custody::admit(&config, &mut loaded, Some(&profile), custody::Mode::Create).unwrap();
    (config, path, loaded)
}

fn checkpoint(root: &Root, generation: u64) -> TopologyCheckpoint {
    let bytes = fs::read(root.0.join(format!("source-{generation}.bin"))).unwrap();
    TopologyCheckpoint::read_trusted(&mut Cursor::new(bytes), MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}

fn observed(config: &Config, loaded: &Loaded) -> FileDeliverySnapshot {
    if let Some(predictor) = loaded.generation.required_owned_pre_output_forecast() {
        FileOversight::read_publication_with_owned_learned_consistency(&config.store,
            &config.profile, &loaded.generation, predictor).unwrap().publication
    } else {
        FileOversight::read_publication_with_learned_generation(&config.store,
            &config.profile, &loaded.generation).unwrap()
    }
}

fn run_peer(root: &Root, config: Config, loaded: Loaded,
    after_message: Option<u64>, finishing: Option<u64>) {
    let reviewer_profile = fixture::peers(root, &config);
    let profile = peers::actor_profile(root, &config, loaded.request);
    let request = loaded.request; let mut actor = None; let mut human = None;
    let release = if finishing.is_some() { LearnedTextRelease::Finish } else { LearnedTextRelease::Message };
    let result = run_with_actor(config, loaded, &reviewer_profile,
        after_message.is_some() || finishing.is_some(), finishing, after_message,
        None, Some(&profile), || {
            if actor.is_none() && profile.socket.exists() {
                actor = Some(peers::actor(profile.clone(), release, ElapsedTick(50_000), peers::Behavior::Observe));
            }
            if human.is_none() && reviewer_profile.socket(request).exists() {
                human = Some(peers::human(reviewer_profile.clone(), request, ReviewDecision::Approve));
            }
            ElapsedTick(1000)
        }).unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    let response = actor.expect("original actor endpoint bound").join().unwrap();
    assert_eq!(response.result, result.response.result);
    assert!(matches!(response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let packet = human.expect("independent human approval required").join().unwrap();
    assert_eq!(packet.action().spec().deadline, ElapsedTick(50_000));
    assert!(!profile.socket.exists()); assert!(!reviewer_profile.socket(request).exists());
}

fn archived_selection(root: &Root, saved: &TopologyCheckpoint, generation: u64,
    next: Option<u64>) -> PathBuf {
    let initial = pair(root, "initial.json", "initial-inventory.json");
    let following = next.map_or_else(|| pair(root, "missing-next.json", "missing-next-inventory.json"),
        |next| pair(root, &format!("live-{next}.json"), &format!("inventory-{next}.json")));
    let output = next.map_or(String::new(), |next|
        format!(r#", "retain_checkpoint":"{}""#, root.0.join(format!("source-{next}.bin")).display()));
    let path = root.0.join(format!("recover-{generation}-{}.json", next.unwrap_or(0)));
    fs::write(&path, format!(r#"{{"schema_version":"fa.mediated-submit/2","initial":{initial},"next":{following},"operation":{},"source_checkpoint":"{}"{output}}}"#,
        saved.last_operation() + 1, root.0.join(format!("source-{generation}.bin")).display())).unwrap();
    path
}

#[test]
fn authenticated_learned_messages_and_finish_keep_topology_custody_across_all_predictor_recoveries() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let (config, mut path, loaded) = selected(&root, mode, false);
        run_peer(&root, config, loaded, None, None);
        let config = configured(&root); let loaded = recipe::load(&path, &config, false).unwrap();
        let first = observed(&config, &loaded);
        assert_eq!(first.executions, 1); assert_eq!(first.payload, b"A");
        let mut saved = checkpoint(&root, 1);
        assert!(saved.available()); assert!(saved.work().checks >= 2);
        assert_eq!(saved.work().reserved_read_bytes, saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES);
        assert_eq!(saved.history_anchor(), &anchors::load(&root.0.join("history-1.bin")).unwrap());
        for generation in [2, 3] {
            let config = configured(&root);
            let (next_path, mut loaded) = native::next(&root, &config, generation); path = next_path;
            loaded.recovery_anchor = Some(anchors::load(&root.0.join(format!("history-{}.bin", generation - 1))).unwrap());
            loaded.anchor_output = Some(root.0.join(format!("history-{generation}.bin")));
            fs::write(root.0.join(format!("live-{generation}.json")), graph(generation, false)).unwrap();
            fs::write(root.0.join(format!("inventory-{generation}.json")), INVENTORY).unwrap();
            let profile = archived_selection(&root, &saved, generation - 1, Some(generation));
            custody::admit(&config, &mut loaded, Some(&profile), custody::Mode::Live).unwrap();
            run_peer(&root, config, loaded, (generation == 2).then_some(1), (generation == 3).then_some(2));
            let current = checkpoint(&root, generation);
            assert!(current.work().checks >= saved.work().checks + 2);
            assert!(current.last_operation() > saved.last_operation());
            assert_eq!(current.history_anchor(), &anchors::load(&root.0.join(format!("history-{generation}.bin"))).unwrap());
            saved = current;
        }
        let config = configured(&root); let mut loaded = recipe::load(&path, &config, false).unwrap();
        let state = observed(&config, &loaded);
        assert_eq!(state.executions, 3); assert_eq!(state.payload, b"AA");
        assert_eq!(state.control.ledger.available + state.control.ledger.reserved + state.control.ledger.charged,
            config.profile.delivery.total);
        loaded.recovery_anchor = Some(anchors::load(&root.0.join("history-3.bin")).unwrap());
        let profile = archived_selection(&root, &saved, 3, None);
        for generation in [1, 2, 3] {
            fs::remove_file(root.0.join(format!("live-{generation}.json"))).unwrap();
            fs::remove_file(root.0.join(format!("inventory-{generation}.json"))).unwrap();
        }
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        fs::remove_file(root.0.join("native-roster.json")).unwrap();
        custody::admit(&config, &mut loaded, Some(&profile), custody::Mode::Historical).unwrap();
        let canonical = config.store.join("delivery.bin"); let before = fs::read(&canonical).unwrap();
        let pending = config.store.join("delivery.pending"); fs::write(&pending, b"keep staged evidence").unwrap();
        let receipt = recovery::readonly::read(&config, &loaded, Some(2), &mut || Duration::ZERO).unwrap();
        assert!(matches!(receipt.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        assert_eq!(fs::read(&canonical).unwrap(), before); assert_eq!(fs::read(&pending).unwrap(), b"keep staged evidence");
        let stopped = recovery::stop_selected_with_elapsed(&config, &loaded, 801,
            || ElapsedTick(1001), &mut || Duration::ZERO).unwrap();
        assert!(stopped.progress().drained()); assert_eq!(stopped.snapshot().executions, 3);
        assert_eq!(stopped.snapshot().control.ledger.charged, state.control.ledger.charged);
        let stopped_bytes = fs::read(&canonical).unwrap();
        // The older source checkpoint cannot silently describe a successor.
        assert!(recovery::stop_selected_with_elapsed(&config, &loaded, 801,
            || panic!("old checkpoint must refuse before clock"), &mut || Duration::ZERO).is_err());
        assert_eq!(fs::read(&canonical).unwrap(), stopped_bytes);
        // Explicit independent current availability must include the Stop's
        // withdrawal. Work is retained; no live source or new cut is required.
        fs::write(root.0.join("archived-current.json"), saved.topology_bytes()).unwrap();
        fs::write(root.0.join("archived-current-inventory.json"), saved.inventory_bytes()).unwrap();
        let current = pair(&root, "archived-current.json", "archived-current-inventory.json");
        let initial = pair(&root, "initial.json", "initial-inventory.json");
        let next = pair(&root, "missing-next.json", "missing-next-inventory.json");
        for available in [true, false] {
            let manual = root.0.join("stopped-topology.json");
            fs::write(&manual, format!(r#"{{"schema_version":"fa.mediated-submit/1","initial":{initial},"current":{current},"next":{next},"available":{available},"operation":{},"source":{{"checks":{},"reserved_read_bytes":{},"last_operation":{}}}}}"#,
                saved.last_operation() + 1, saved.work().checks, saved.work().reserved_read_bytes, saved.last_operation())).unwrap();
            loaded.topology = None;
            custody::admit(&config, &mut loaded, Some(&manual), custody::Mode::Historical).unwrap();
            let retry = recovery::stop_selected_with_elapsed(&config, &loaded, 801,
                || panic!("exact drained Stop reads no clock"), &mut || Duration::ZERO);
            assert_eq!(retry.is_ok(), !available);
            assert_eq!(fs::read(&canonical).unwrap(), stopped_bytes);
        }
    }
}

#[test]
fn final_native_source_loss_blocks_learned_effect_after_both_keys_without_refunding_dispatch() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let (mut config, _, loaded) = selected(&root, mode, false);
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        let custody::Started { custody: (host, reviewer, observer), source, .. } =
            custody::start(&config, &loaded, false, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap();
        let mut prediction = observer.map(|observer| prediction::Session::new(1, observer));
        let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
        custody::attach(&mut driver, source).unwrap();
        custody::certify(&mut driver, &deadline, &mut || ElapsedTick(1000)).unwrap();
        assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
            &deadline, &mut || ElapsedTick(1000), prediction.as_mut()).unwrap());
        submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
        let (mut driver, history) = native::review_current(&mut config, &loaded, &reviewer, driver, &deadline);
        let human = native::approve(&mut driver, &mut config, &reviewer, 1);
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
        let charged = driver.supervisor().host().unwrap().inspect().control.ledger.charged;
        assert!(charged > 0); let work = driver.topology().unwrap().source().work();
        fs::remove_file(root.0.join("inventory-1.json")).unwrap();
        assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
                if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
        assert_eq!(driver.topology().unwrap().source().work().checks, work.checks + 1);
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 0); assert!(host.inspect().payload.is_empty());
        assert_eq!(host.inspect().control.ledger.charged, charged);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
    }
}

#[test]
fn initial_native_bypass_refuses_before_any_learned_inference_or_effect() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let (config, _, loaded) = selected(&root, mode, true);
        let deadline = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
        let custody::Started { custody: (host, _, _), source, .. } =
            custody::start(&config, &loaded, false, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap();
        let (_, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let mut driver = FileSupervisedDriver::new(supervisor);
        custody::attach(&mut driver, source).unwrap();
        let before = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        assert!(custody::certify(&mut driver, &deadline, &mut || ElapsedTick(1000)).is_err());
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 0); assert!(host.inspect().control.ledger.stages.is_empty());
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical, before); assert_eq!(numerical.position, 0); assert_eq!(numerical.sampled_draws, 0);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

#[test]
fn topology_options_preserve_original_learned_modes_and_refuse_incomplete_paths() {
    let original = ["create-learned-generated", "/config", "/recipe", "--resume", "--receipts-only"].map(str::to_owned);
    let mut selected = original.to_vec(); selected.extend(["--topology-profile".into(), "/topology".into()]);
    let (remaining, path) = custody::take_option(&selected).unwrap();
    assert_eq!(remaining, original); assert_eq!(path.as_deref(), Some(Path::new("/topology")));
    selected.pop(); assert!(custody::take_option(&selected).is_err());
    selected.push("--history-anchor".into()); assert!(custody::take_option(&selected).is_err());
    selected.pop(); selected.push("/topology".into());
    selected.extend(["--topology-profile".into(), "/another".into()]);
    assert!(custody::take_option(&selected).is_err());
}
