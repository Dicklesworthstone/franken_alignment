//! Actual create handler, native helper processes, human socket and original sink.
//! Controlled ballots are not model accuracy or operating-system containment proof.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    mediated::{FileMediatedRequirements, FileTopologyRequirement}};
use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::TOPOLOGY_REFRESH_READ_BYTES;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress}, wire::ReviewDecision};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::encode_command;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::Snapshot;
use std::{collections::BTreeMap, fs, io, os::unix::net::UnixStream, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};

const TEMPLATE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"));
const INVENTORY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-topology-create-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn path(&self, name: &str) -> PathBuf { self.0.join(name) }
    fn bytes(&self) -> Vec<u8> { fs::read(self.path("store/delivery.bin")).unwrap() }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("topology create cleanup: {error}"); }
    }
}
fn configuration(root: &Root) -> Config {
    let exe = std::env::current_exe().unwrap().display().to_string();
    let mut text = TEMPLATE.replace("/operator/publication-store", root.path("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.path("evidence.json").to_str().unwrap())
        .replace("\"directory\": \"/operator/helpers\"", &format!("\"directory\": {:?}", root.0.display().to_string()))
        .replace("\"arguments\": []", "\"arguments\": [\"--exact\",\"tests::synthetic_helper_process\",\"--nocapture\"]")
        .replace("\"poll_ms\": 10", "\"poll_ms\": 1")
        .replace("\"runtime_ms\": 120000", "\"runtime_ms\": 15000");
    for member in ["alpha", "beta"] {
        text = text.replace(&format!("\"executable\": \"/operator/helpers/{member}\""),
            &format!("\"executable\": {exe:?}"));
        text = text.replacen("\"environment\": {}",
            &format!("\"environment\": {{\"FA_EXAMPLE_HELPER_MEMBER\":\"{member}\"}}"), 1);
    }
    fs::write(root.path("config.json"), &text).unwrap();
    Config::decode(text.as_bytes()).unwrap()
}
fn evidence(root: &Root, config: &Config) {
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
        scope: config.profile.delivery.scope }, Snapshot { semantic_epoch: 1, complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]) }, ["alpha", "beta"].into_iter()
        .map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    fs::write(root.path("evidence.json"), data.encode()).unwrap();
}
fn graph_text(bypass: bool) -> String {
    let extra = if bypass { r#",{"id":3,"from":1,"to":4,"channel":"dispatch","route":"publication","provenance":103}"# } else { "" };
    let reads = 32 * TOPOLOGY_REFRESH_READ_BYTES;
    format!(r#"{{"schema":"fa.authority-topology/1","generation":1,"inventory_generation":1,
        "complete_inventory_generation":1,
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
            {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}{extra}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":32,"read_bytes":{reads}}}}}"#)
}
fn selection(root: &Root, graph: &str) -> topology::Selection {
    fs::write(root.path("topology.json"), graph).unwrap();
    fs::write(root.path("inventory.json"), INVENTORY).unwrap();
    let text = format!(r#"{{"schema_version":"fa.mediated-create/1","topology_path":"{}","inventory_path":"{}"}}"#,
        root.path("topology.json").display(), root.path("inventory.json").display());
    fs::write(root.path("topology-profile.json"), &text).unwrap();
    topology::Selection::decode(text.as_bytes()).unwrap()
}
fn proposal(config: &Config, deadline: ElapsedTick) -> Vec<u8> {
    encode_command(&Command::Submit { request: 1, proposal: ActorProposal { target: config.profile.delivery.target,
        payload: b"once".to_vec(), units: 4, deadline, expected_policy_epoch: 0 } }).unwrap()
}
fn reviewer(config: &Config, decision: ReviewDecision) -> std::thread::JoinHandle<()> {
    let path = config.socket(1);
    let expected = ReviewerExpectation { reviewer: config.profile.human.reviewer_id,
        scope: config.profile.delivery.scope, clock_domain: crate::config::CLOCK_DOMAIN };
    std::thread::spawn(move || {
        let start = Instant::now();
        let stream = loop {
            match UnixStream::connect(&path) {
                Ok(stream) => break stream,
                Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(start.elapsed() < Duration::from_secs(10), "reviewer offer absent: {error}");
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("reviewer connection: {error}"),
            }
        };
        let mut client = ReviewerClient::from_unix(stream, expected).unwrap();
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(decision).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn requirements(config: &Config, available: bool) -> FileMediatedRequirements {
    let graph = LoadedAuthorityTopology::from_json_bytes(graph_text(false).as_bytes(), INVENTORY).unwrap().graph().clone();
    FileMediatedRequirements { oversight: FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: Some(config.source_policy),
            identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 },
    }, topology: FileTopologyRequirement { initial: graph.clone(), current: graph, available },
        prediction: None, evaluation: None }
}

#[test]
fn actual_create_and_offline_stop_keep_the_original_publication_and_independent_history() {
    let root = Root::new(); let config = configuration(&root); evidence(&root, &config);
    let _selected = selection(&root, &graph_text(false));
    let doc = proposal(&config, plus(crate::workflow::clock(), 30000).unwrap());
    fs::write(root.path("submit.json"), doc).unwrap();
    let human = reviewer(&config, ReviewDecision::Approve);
    let args = vec!["create".into(), root.path("config.json").display().to_string(),
        root.path("submit.json").display().to_string(), "--retain-history-anchor".into(),
        root.path("after.anchor").display().to_string(), "--topology-profile".into(),
        root.path("topology-profile.json").display().to_string()];
    let result = crate::command(args);
    human.join().unwrap(); result.unwrap();
    let state = FileOversight::read_mediation(&config.store, &config.profile, &requirements(&config, true)).unwrap();
    assert_eq!(state.journal.executions, 1); assert_eq!(state.journal.payload, b"once");
    assert_eq!(state.journal.control.ledger.charged, 4);
    assert!(matches!(state.topology.last_check, Some(Ok(CutCheck::Verified(_)))));
    assert!(state.topology.accepted.is_some());
    assert!(root.path("after.anchor").is_file());
    // Exercise the operator handoff, not just a library-only bootstrap: the
    // successful create archive becomes independent input to offline stop.
    fs::remove_file(root.path("evidence.json")).unwrap();
    let before_archive = fs::read(root.path("after.anchor")).unwrap();
    let stop_profile = format!(r#"{{"schema_version":"fa.guarded-stop/2","minimum":{{
        "journal_revision":{},"control_sequence":{},"authority_epoch":{}}},
        "history_anchor":"{}","retain_history_anchor":"{}","topology":{{
        "initial_topology":"{}","initial_inventory":"{}",
        "current_topology":"{}","current_inventory":"{}","available":true}}}}"#,
        state.journal.revision, state.journal.control.sequence, state.journal.control.ledger.epoch,
        root.path("after.anchor").display(), root.path("stopped.anchor").display(),
        root.path("topology.json").display(), root.path("inventory.json").display(),
        root.path("topology.json").display(), root.path("inventory.json").display());
    let stop = crate::recover_stop::guarded::Profile::decode(stop_profile.as_bytes()).unwrap();
    let mut offline = configuration(&root); offline.programs.clear();
    crate::recover_stop::guarded::run(&offline, &stop, 900, crate::workflow::clock, &mut Vec::new()).unwrap();
    let stopped = FileOversight::read_mediation(&config.store, &config.profile, &requirements(&config, false)).unwrap();
    assert!(stopped.journal.stop.is_some()); assert_eq!(stopped.journal.executions, 1);
    assert_eq!(stopped.journal.payload, b"once"); assert_eq!(stopped.journal.control.ledger.charged, 4);
    assert_eq!(stopped.journal.revision, state.journal.revision + 3);
    assert_eq!(fs::read(root.path("after.anchor")).unwrap(), before_archive);
    assert!(root.path("stopped.anchor").is_file());
    let before = root.bytes();
    let retry = stop_profile.replace("\"available\":true", "\"available\":false")
        .replace("stopped.anchor", "retried.anchor");
    let retry = crate::recover_stop::guarded::Profile::decode(retry.as_bytes()).unwrap();
    crate::recover_stop::guarded::run(&offline, &retry, 900, || panic!("drained stop retry clock"), &mut Vec::new()).unwrap();
    assert_eq!(root.bytes(), before);
    assert_eq!(fs::read(root.path("stopped.anchor")).unwrap(), fs::read(root.path("retried.anchor")).unwrap());
}

#[test]
fn missing_inventory_after_dispatch_cannot_bypass_the_required_final_publication_check() {
    let root = Root::new(); let config = configuration(&root); evidence(&root, &config);
    let selected = selection(&root, &graph_text(false)); let doc = proposal(&config, ElapsedTick(100000));
    let store = config.store.clone(); let profile = config.profile.clone(); let output = root.path("after.anchor");
    let human = reviewer(&config, ReviewDecision::Approve); let mut removed = false;
    let result = run_selected(config, Creation { document: &doc, output: &output, peers: None, publication: None },
        Some(selected), || {
            if !removed && let Ok(state) = FileOversight::read_publication(&store, &profile)
                && state.control.ledger.charged != 0 && state.executions == 0 {
                fs::remove_file(root.path("inventory.json")).unwrap(); removed = true;
            }
            ElapsedTick(1000)
        });
    human.join().unwrap(); let result = result.unwrap(); assert!(removed);
    assert!(!matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(state.executions, 0); assert_eq!(state.payload, b"initial");
    assert_eq!(state.control.ledger.available + state.control.ledger.reserved + state.control.ledger.charged,
        profile.delivery.total);
    assert!(output.is_file()); assert_eq!(result.cleanup_pending, 0);
}

#[test]
fn initial_bypass_refuses_before_any_actor_admission_and_retains_the_native_cut() {
    let root = Root::new(); let mut config = configuration(&root); evidence(&root, &config); config.programs.clear();
    let selected = selection(&root, &graph_text(true)); let doc = proposal(&config, ElapsedTick(100000));
    let profile = config.profile.clone(); let store = config.store.clone(); let output = root.path("after.anchor");
    let result = run_selected(config, Creation { document: &doc, output: &output, peers: None, publication: None },
        Some(selected), || ElapsedTick(1000));
    let Err(error) = result else { panic!("bypass admitted"); };
    assert!(error.contains("initial topology was not certified"), "{error}");
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert!(state.control.ledger.stages.is_empty()); assert_eq!(state.executions, 0);
    assert!(output.is_file());
}

#[test]
fn malformed_mismatched_and_insufficient_topology_refuse_before_storage_creation() {
    for mode in 0..4 {
        let root = Root::new(); let config = configuration(&root);
        let text = match mode { 0 => "{}".into(),
            1 => graph_text(false).replace("\"run\":3", "\"run\":99"),
            2 => graph_text(false).replace("\"checks\":32", "\"checks\":1"),
            _ => graph_text(false).replace(&format!("\"read_bytes\":{}", 32 * TOPOLOGY_REFRESH_READ_BYTES), "\"read_bytes\":1") };
        let selected = selection(&root, &text); let doc = proposal(&config, ElapsedTick(100000));
        let output = root.path("after.anchor");
        assert!(run_selected(config, Creation { document: &doc, output: &output, peers: None, publication: None },
            Some(selected), || ElapsedTick(1000)).is_err());
        assert!(!root.path("store").exists()); assert!(!output.exists());
    }
}

#[test]
fn create_never_opens_an_existing_store_or_changes_its_canonical_history() {
    let root = Root::new(); let config = configuration(&root);
    let (host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap(); drop(host);
    let before = root.bytes(); let selected = selection(&root, &graph_text(false));
    let doc = proposal(&config, ElapsedTick(100000)); let output = root.path("after.anchor");
    assert!(run_selected(config, Creation { document: &doc, output: &output, peers: None, publication: None },
        Some(selected), || ElapsedTick(1000)).is_err());
    assert_eq!(root.bytes(), before); assert!(!output.exists());
}

#[test]
fn later_clock_after_initial_topology_check_expires_admission_before_proposal() {
    let root = Root::new(); let mut config = configuration(&root); evidence(&root, &config); config.programs.clear();
    let expected = requirements(&config, true); let store = config.store.clone(); let profile = config.profile.clone();
    let selected = selection(&root, &graph_text(false)); let doc = proposal(&config, ElapsedTick(100000));
    let output = root.path("after.anchor"); let mut expired = false;
    let result = run_selected(config, Creation { document: &doc, output: &output, peers: None, publication: None },
        Some(selected), || {
            if let Ok(state) = FileOversight::read_mediation(&store, &profile, &expected)
                && state.topology.accepted.is_some() { expired = true; }
            ElapsedTick(if expired { 100000 } else { 1000 })
        });
    let Err(error) = result else { panic!("expired topology admission succeeded"); };
    assert!(expired); assert!(error.contains("deadline"), "{error}");
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert!(state.control.ledger.stages.is_empty()); assert_eq!(state.executions, 0); assert!(output.is_file());
}

#[test]
fn topology_success_does_not_replace_the_original_human_decision() {
    let root = Root::new(); let config = configuration(&root); evidence(&root, &config);
    let selected = selection(&root, &graph_text(false)); let doc = proposal(&config, ElapsedTick(100000));
    let profile = config.profile.clone(); let store = config.store.clone(); let output = root.path("after.anchor");
    let human = reviewer(&config, ReviewDecision::Reject);
    let result = run_selected(config, Creation { document: &doc, output: &output, peers: None, publication: None },
        Some(selected), || ElapsedTick(1000));
    human.join().unwrap(); let result = result.unwrap();
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.available, profile.delivery.total);
    assert_eq!(result.cleanup_pending, 0);
}

#[test]
fn archive_output_race_preserves_committed_publication_and_reports_retention_failure() {
    let root = Root::new(); let config = configuration(&root); evidence(&root, &config);
    let selected = selection(&root, &graph_text(false)); let doc = proposal(&config, ElapsedTick(100000));
    let profile = config.profile.clone(); let store = config.store.clone(); let output = root.path("after.anchor");
    let human = reviewer(&config, ReviewDecision::Approve); let mut raced = false;
    let result = run_selected(config, Creation { document: &doc, output: &output, peers: None, publication: None },
        Some(selected), || {
            if !raced && let Ok(state) = FileOversight::read_publication(&store, &profile)
                && state.control.ledger.charged != 0 {
                fs::write(&output, b"another custodian").unwrap(); raced = true;
            }
            ElapsedTick(1000)
        });
    human.join().unwrap(); let result = result.unwrap(); assert!(raced);
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert!(result.failure.as_deref().unwrap().contains("history anchor retention failed"));
    assert_eq!(FileOversight::read_publication(&store, &profile).unwrap().executions, 1);
    assert_eq!(fs::read(output).unwrap(), b"another custodian"); assert_eq!(result.cleanup_pending, 0);
}

#[test]
fn closed_creation_arguments_reject_downgrades_duplicates_and_recovery_profiles() {
    let args = ["create", "config", "submit", "--retain-history-anchor", "/archive",
        "--topology-profile", "/topology"].map(str::to_owned).to_vec();
    assert!(arguments(&args).unwrap().3.is_some());
    let mut duplicate = args.clone(); duplicate.extend(["--topology-profile".into(), "/other".into()]);
    assert!(arguments(&duplicate).is_err());
    for mode in ["submit", "resume", "recover-stop", "create-checked"] {
        let mut bad = args.clone(); bad[0] = mode.into(); assert!(arguments(&bad).is_err());
    }
    for text in [r#"{"schema_version":"fa.guarded-stop/2","topology_path":"/a","inventory_path":"/b"}"#,
        r#"{"schema_version":"fa.mediated-create/1","topology_path":"/a","inventory_path":"/b","accept_stored":true}"#,
        r#"{"schema_version":"fa.mediated-create/1","topology_path":"/a","inventory_path":"/a"}"#,
        r#"{"schema_version":"fa.mediated-create/1","topology_path":"/a/../b","inventory_path":"/c"}"#] {
        assert!(topology::Selection::decode(text.as_bytes()).is_err());
    }
    assert!(arguments(&args[..5]).unwrap().3.is_none(), "legacy creation remains explicit");
}
