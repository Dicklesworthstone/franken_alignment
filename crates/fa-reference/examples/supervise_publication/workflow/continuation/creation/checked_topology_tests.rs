//! Compose actual topology custody with original witness-checked publication.
//! Synthetic ballots exercise plumbing, not detector accuracy or host isolation.
use super::*;
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    mediated::{FileMediatedRequirements, FileTopologyRequirement}};
use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::TOPOLOGY_REFRESH_READ_BYTES;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    FileCaptureIdentity, FilePublicationCapture};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress}, wire::ReviewDecision};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::encode_command;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::action::consequence::mediation::{CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use fa_reference::Snapshot;
use std::{collections::BTreeMap, fs, io, os::unix::net::UnixStream, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};

const TEMPLATE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"));
const INVENTORY: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));
const RECIPE: &str = r#"[{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1},{"kind":"empty_range","start":6,"end":9},{"kind":"range_members","start":2,"end":6}]"#;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-checked-topology-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self, name: &str) -> PathBuf { self.0.join(name) }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("checked topology cleanup: {error}"); }
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
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn evidence(root: &Root, config: &Config) {
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
        scope: config.profile.delivery.scope }, snapshot(), ["alpha", "beta"].into_iter()
        .map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    fs::write(root.path("evidence.json"), data.encode()).unwrap();
}
fn graph() -> String {
    let reads = 32 * TOPOLOGY_REFRESH_READ_BYTES;
    format!(r#"{{"schema":"fa.authority-topology/1","generation":1,"inventory_generation":1,
        "complete_inventory_generation":1,
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
            {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},
        "source_limits":{{"checks":32,"read_bytes":{reads}}}}}"#)
}
fn selection(root: &Root) -> topology::Selection {
    fs::write(root.path("topology.json"), graph()).unwrap();
    fs::write(root.path("inventory.json"), INVENTORY).unwrap();
    let text = format!(r#"{{"schema_version":"fa.mediated-create/1","topology_path":"{}","inventory_path":"{}"}}"#,
        root.path("topology.json").display(), root.path("inventory.json").display());
    fs::write(root.path("topology-profile.json"), &text).unwrap();
    topology::Selection::decode(text.as_bytes()).unwrap()
}
fn requirements(config: &Config) -> FileMediatedRequirements {
    let graph = LoadedAuthorityTopology::from_json_bytes(graph().as_bytes(), INVENTORY).unwrap().graph().clone();
    FileMediatedRequirements { oversight: FileRecoveryRequirements {
        guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: Some(config.source_policy),
            identity: None, campaigns: None, credential: None },
        effective_policy: config.profile.delivery.policy.clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 },
    }, topology: FileTopologyRequirement { initial: graph.clone(), current: graph, available: true },
        prediction: None, evaluation: None }
}
fn document(config: &Config, deadline: ElapsedTick) -> Vec<u8> {
    encode_command(&Command::Submit { request: 1, proposal: ActorProposal { target: config.profile.delivery.target,
        payload: b"once".to_vec(), units: 4, deadline, expected_policy_epoch: 0 } }).unwrap()
}
fn profile_text(root: &Root) -> String {
    format!(r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}","current":"{}","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":{RECIPE}}}"#,
        root.path("original.bin").display(), root.path("current.bin").display())
}
fn profile(root: &Root) -> PublicationProfile {
    let text = profile_text(root); fs::write(root.path("witness-profile.json"), &text).unwrap();
    PublicationProfile::decode(text.as_bytes()).unwrap()
}
fn prepare(root: &Root, config: &Config, deadline: ElapsedTick) -> (PublicationProfile, FrozenAction) {
    evidence(root, config);
    // Use the ORIGINAL policy to freeze the reference capture action. No guessed
    // digest and no second authorization implementation in the test fixture.
    let (mut host, _) = FileOversight::create(root.path("capture-owner"), config.profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION,
        scope: config.profile.delivery.scope, target: Some(config.profile.delivery.target),
        payload: b"once".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
        deadline, units: 4 }, snapshot()).unwrap();
    let original = packet(&action, 1, &[0, 2, 4]);
    write(&root.path("original.bin"), &original); write(&root.path("current.bin"), &original);
    (profile(root), action)
}
fn packet(action: &FrozenAction, generation: u64, keys: &[u64]) -> FilePublicationCapture {
    let key = ProjectionKey { source: 40, branch: action.spec().scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    let structured = FileWitnessInput::new(generation, generation, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap();
    FilePublicationCapture::new(1, FileCaptureIdentity { source: 91, generation }, action,
        FilePublicationInputs::new(Some(structured), None)).unwrap()
}
fn write(path: &Path, capture: &FilePublicationCapture) {
    let pending = path.with_extension("next");
    fs::write(&pending, capture.to_bytes().unwrap()).unwrap(); fs::rename(pending, path).unwrap();
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
fn executed(result: &RunResult) -> bool {
    matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}

#[test]
fn first_native_image_contains_both_guards_without_reading_witness_inputs() {
    let root = Root::new(); let config = configuration(&root); let publication = profile(&root);
    let prepared = selection(&root).prepare(&config, &mut || Ok(())).unwrap();
    let (host, _, _) = prepared.create(&config, Some(publication.simple_limits().unwrap())).unwrap();
    assert_eq!(host.publication_validation_profile().unwrap(), Some(publication.limits));
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    assert_eq!(host.file_source_status().unwrap().policy, config.source_policy);
    assert!(host.publication_guard_required());
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, config.profile.delivery.total);
    for input in ["evidence.json", "original.bin", "current.bin"] { assert!(!root.path(input).exists()); }
}

#[test]
fn checked_topology_command_executes_with_real_helpers_human_and_retained_history() {
    let root = Root::new(); let config = configuration(&root);
    let deadline = plus(crate::workflow::clock(), 30000).unwrap();
    let (_publication, _) = prepare(&root, &config, deadline); let _selection = selection(&root);
    fs::write(root.path("submit.json"), document(&config, deadline)).unwrap();
    let human = reviewer(&config, ReviewDecision::Approve);
    let args = vec!["create-checked".into(), root.path("config.json").display().to_string(),
        root.path("submit.json").display().to_string(), root.path("witness-profile.json").display().to_string(),
        "--topology-profile".into(), root.path("topology-profile.json").display().to_string(),
        "--retain-history-anchor".into(), root.path("after.anchor").display().to_string()];
    let result = crate::command(args); human.join().unwrap(); result.unwrap();
    let state = FileOversight::read_mediation(&config.store, &config.profile, &requirements(&config)).unwrap();
    assert_eq!(state.journal.executions, 1); assert_eq!(state.journal.payload, b"once");
    assert_eq!(state.journal.control.ledger.charged, 4);
    assert!(matches!(state.topology.last_check, Some(Ok(CutCheck::Verified(_)))));
    assert!(root.path("after.anchor").is_file());
}

#[test]
fn final_publication_checks_both_sources_after_dispatch_not_just_initial_certification() {
    // Positive unrelated mutation must still publish. Every negative is injected
    // only AFTER human approval and durable dispatch, at the actual final gate.
    for mode in ["unrelated", "phantom", "missing-witness", "missing-topology"] {
        let root = Root::new(); let config = configuration(&root); let deadline = ElapsedTick(100000);
        let (publication, action) = prepare(&root, &config, deadline); let selected = selection(&root);
        let doc = document(&config, deadline); let output = root.path("after.anchor");
        let store = config.store.clone(); let bootstrap = config.profile.clone();
        let human = reviewer(&config, ReviewDecision::Approve); let mut changed = false;
        let result = run_selected(config, Creation { document: &doc, output: &output, peers: None,
            publication: Some(&publication) }, Some(selected), || {
            if !changed && let Ok(state) = FileOversight::read_publication(&store, &bootstrap)
                && state.control.ledger.charged != 0 && state.executions == 0 {
                match mode {
                    "unrelated" => write(&root.path("current.bin"), &packet(&action, 2, &[0, 2, 4, 99])),
                    "phantom" => write(&root.path("current.bin"), &packet(&action, 2, &[0, 1, 2, 4])),
                    "missing-witness" => fs::remove_file(root.path("current.bin")).unwrap(),
                    "missing-topology" => fs::remove_file(root.path("inventory.json")).unwrap(),
                    _ => unreachable!(),
                }
                changed = true;
            }
            ElapsedTick(1000)
        });
        human.join().unwrap(); let result = result.unwrap(); assert!(changed, "{mode}");
        assert_eq!(executed(&result), mode == "unrelated", "{mode}");
        let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(state.executions, u64::from(mode == "unrelated"), "{mode}");
        assert_eq!(state.payload, if mode == "unrelated" { b"once".to_vec() } else { b"initial".to_vec() });
        assert_eq!(state.control.ledger.available + state.control.ledger.reserved + state.control.ledger.charged,
            bootstrap.delivery.total);
        assert!(output.is_file()); assert_eq!(result.cleanup_pending, 0);
    }
}

#[test]
fn valid_witnesses_and_topology_cannot_replace_human_rejection() {
    let root = Root::new(); let config = configuration(&root); let deadline = ElapsedTick(100000);
    let (publication, _) = prepare(&root, &config, deadline); let selected = selection(&root);
    let doc = document(&config, deadline); let output = root.path("after.anchor");
    let store = config.store.clone(); let bootstrap = config.profile.clone();
    let human = reviewer(&config, ReviewDecision::Reject);
    let result = run_selected(config, Creation { document: &doc, output: &output, peers: None,
        publication: Some(&publication) }, Some(selected), || ElapsedTick(1000));
    human.join().unwrap(); let result = result.unwrap(); assert!(!executed(&result));
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.available, bootstrap.delivery.total);
    assert!(output.is_file()); assert_eq!(result.cleanup_pending, 0);
}

#[test]
fn stronger_checked_profiles_refuse_before_clock_topology_or_store_access() {
    for waiting in [false, true] {
        let root = Root::new(); let config = configuration(&root);
        let schema = if waiting { "fa.supervised-witnesses/4" } else { "fa.supervised-witnesses/2" };
        let extra = if waiting { ",\"max_retries\":2" } else { "" };
        let text = profile_text(&root).replace("fa.supervised-witnesses/1", schema)
            .replace("\"source\":91", &format!(r#""source":91{extra},"feed":{{"path":"{}","source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}}"#,
                root.path("missing-feed.bin").display()));
        let publication = PublicationProfile::decode(text.as_bytes()).unwrap();
        let selected = selection(&root); fs::remove_file(root.path("topology.json")).unwrap();
        let doc = document(&config, ElapsedTick(100000)); let output = root.path("after.anchor");
        let result = run_selected(config, Creation { document: &doc, output: &output, peers: None,
            publication: Some(&publication) }, Some(selected), || panic!("preflight must not read the clock"));
        let Err(error) = result else { panic!("stronger profile was downgraded"); };
        assert!(error.contains("stronger feed/snapshot/joint publication profiles are not downgraded"), "{error}");
        assert!(!root.path("store").exists()); assert!(!output.exists());
    }
}

#[test]
fn checked_topology_arguments_preserve_required_positionals_and_closed_options() {
    for reversed in [false, true] {
        let mut args = ["create-checked", "config", "submit", "witnesses"].map(str::to_owned).to_vec();
        let mut pairs = vec![["--retain-history-anchor", "/archive"], ["--topology-profile", "/topology"],
            ["--reviewer-profile", "/reviewer"]];
        if reversed { pairs.reverse(); }
        for pair in pairs { args.extend(pair.map(str::to_owned)); }
        let (output, peers, checked, topology) = arguments(&args).unwrap();
        assert!(checked); assert_eq!(output, Path::new("/archive"));
        assert_eq!(peers, Some(Path::new("/reviewer"))); assert_eq!(topology, Some(Path::new("/topology")));
        let mut duplicate = args.clone(); duplicate.extend(["--topology-profile".into(), "/other".into()]);
        assert!(arguments(&duplicate).is_err());
        let mut missing = args.clone(); missing.remove(3); assert!(arguments(&missing).is_err());
    }
    for option in ["--resume", "--guarded-profile", "--allow-unknown", "--ignore-witnesses"] {
        let args = ["create-checked", "config", "submit", "witnesses", "--retain-history-anchor", "/archive",
            option, "/topology"].map(str::to_owned).to_vec();
        assert!(arguments(&args).is_err());
    }
}
