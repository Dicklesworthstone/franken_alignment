//! Synthetic independent helpers/human choices exercise original process/socket
//! plumbing. No evaluator accuracy, human identity or OS containment is claimed.
use super::*;
use fa_reference::Snapshot;
use fa_reference::action::consequence::delivery::persistent::observed::{
    guarded::{anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES}, mediated::FileMediatedBootstrap},
    driver::topology::FileDriverTopology,
    reviewer::{client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress}, wire::ReviewDecision}};
use fa_reference::action::consequence::oversight::{
    actor::{ActorProposal, ActorOutcome, Knowledge}, actor_wire::encode_command,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    helper_processes::HelperProgram, helper_client::{HelperClient, ClientPhase}};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::round::Verdict;
use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::TOPOLOGY_REFRESH_READ_BYTES;
use fa_reference::action::consequence::mediation::MAX_CHECK_EDGE_VISITS;
use std::{collections::BTreeMap, ffi::OsString, fs, io, os::unix::net::UnixStream,
    path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

const TEMPLATE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"));
const INVENTORY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"));
const CHILD: &str = "workflow::continuation::guarded::mediated::tests::synthetic_helper_process";
const MEMBER: &str = "FA_MEDIATED_SUBMIT_MEMBER";
const EPOCH: &str = "FA_MEDIATED_SUBMIT_EPOCH";
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-mediated-submit-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn path(&self, name: &str) -> PathBuf { self.0.join(name) }
    fn bytes(&self) -> Vec<u8> { fs::read(self.path("store/delivery.bin")).unwrap() }
    fn input(&self, generation: u64, live: bool) -> PathBuf {
        if live { self.path("live-topology.json") } else { self.path(&format!("saved{generation}.json")) }
    }
    fn install(&self, generation: u64, live: bool) {
        fs::write(self.input(generation, live), graph_document(generation)).unwrap();
        fs::write(self.path(if live { "live-inventory.json" } else { "saved-inventory.json" }), INVENTORY).unwrap();
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("mediated submission cleanup: {error}"); }
    }
}
fn graph_document(generation: u64) -> String {
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
        "complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
        {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},"source_limits":{{"checks":8,"read_bytes":67108864}}}}"#)
}
fn loaded(generation: u64) -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(graph_document(generation).as_bytes(), INVENTORY.as_bytes()).unwrap()
}
fn configured(root: &Directory, epoch: u64) -> Config {
    let mut config = Config::decode(TEMPLATE.as_bytes()).unwrap();
    config.store = root.path("store");
    config.source = FileEvidenceSource::new(root.path("evidence.json"), 51, config.profile.delivery.scope, 1048576).unwrap();
    config.timing.poll_ms = 1; config.timing.runtime_ms = 15000; config.timing.cleanup_ms = 2000;
    config.programs = ["alpha", "beta"].into_iter().map(|member| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), CHILD.into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from(MEMBER), member.into()), (OsString::from(EPOCH), epoch.to_string().into())])).unwrap();
        (member.into(), program)
    }).collect();
    config
}
#[test]
fn synthetic_helper_process() {
    let Ok(member) = std::env::var(MEMBER) else { return; };
    let epoch: u64 = std::env::var(EPOCH).unwrap().parse().unwrap();
    let config = Config::decode(TEMPLATE.as_bytes()).unwrap();
    let mut client = HelperClient::from_process_stdin(config.profile.committee.members()[&member].profile_at(epoch)).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference { client.respond(Verdict::Allow, member.as_bytes()).unwrap(); }
        if client.phase() == ClientPhase::ReplySent { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn reviewer(config: &Config, request: u64, choice: ReviewDecision) -> std::thread::JoinHandle<()> {
    let path = config.socket(request);
    let expected = ReviewerExpectation { reviewer: config.profile.human.reviewer_id,
        scope: config.profile.delivery.scope, clock_domain: crate::config::CLOCK_DOMAIN };
    std::thread::spawn(move || {
        let started = Instant::now();
        let socket = loop {
            match UnixStream::connect(&path) {
                Ok(socket) => break socket,
                Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(started.elapsed() < Duration::from_secs(10)); std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("reviewer connection: {error}"),
            }
        };
        let mut client = ReviewerClient::from_unix(socket, expected).unwrap();
        loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(choice).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn document(config: &Config, request: u64, epoch: u64, version: u64, deadline: ElapsedTick) -> Vec<u8> {
    let mut target = config.profile.delivery.target; target.expected_version = version;
    let payload = if request == 1 { b"first".to_vec() } else { b"second".to_vec() };
    encode_command(&Command::Submit { request, proposal: ActorProposal { target,
        units: payload.len() as u64, payload, deadline, expected_policy_epoch: epoch } }).unwrap()
}
fn archive(host: &FileOversight, path: &Path) -> Result<(), String> {
    let mut bytes = Vec::new();
    host.history_anchor().map_err(debug)?.write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES).map_err(debug)?;
    fs::write(path, bytes).map_err(debug)
}
fn seed(root: &Directory) -> Vec<u8> { seed_with_checks(root, 0) }
fn seed_with_checks(root: &Directory, extra: u64) -> Vec<u8> {
    root.install(1, false); root.install(1, true); root.install(2, false);
    let config = configured(root, 0);
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: config.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|member| (member.into(), format!("review this fixture: {member}").into_bytes())).collect()).unwrap();
    fs::write(root.path("evidence.json"), evidence.encode()).unwrap();
    let guards = FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: Some(config.source_policy),
        identity: None, campaigns: None, credential: None };
    let (mut host, roles) = FileOversight::create_mediated_guarded_reserved(&config.store, config.profile.clone(), &guards, None,
        FileMediatedBootstrap { topology: loaded(1).graph().clone(), reserve: RecoveryReserve::terminal(), publication: None },
        None, None).unwrap();
    let mut source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        root.input(1, true), root.path("live-inventory.json"), loaded(1)).unwrap();
    for operation in 1..=extra {
        let revision = host.revision();
        let report = source.refresh(&mut host, revision, operation).unwrap();
        assert!(report.refusal.is_none());
        assert_eq!(source.work().checks, operation);
    }
    let document = document(&config, 1, 0, 1, ElapsedTick(100000));
    let Command::Submit { request, proposal } = decode_command(&document).unwrap() else { unreachable!() };
    let human = reviewer(&config, 1, ReviewDecision::Approve);
    let result = from_recovered_with_topology(config, host, roles.oversight.human, Continuation {
        document: &document, request, proposal, peers: None, publication: None, credibility: None,
        runtime_deadline: ElapsedTick(16000), started: Instant::now(),
    }, Some(FileDriverTopology::new(source)), || ElapsedTick(1000), |host| archive(host, &root.path("first.anchor"))).unwrap();
    human.join().unwrap(); assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0); assert!(executed(&result));
    // Keep historical expectations separate while updating the SAME live paths.
    root.install(2, true);
    document
}
fn executed(result: &RunResult) -> bool {
    matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn recovery_text(root: &Directory, output: Option<&str>) -> String {
    let config = configured(root, 0);
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let output = output.map(|name| format!(r#", "retain_history_anchor":"{}""#, root.path(name).display())).unwrap_or_default();
    format!(r#"{{"schema_version":"fa.guarded-submit/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},
        "history_anchor":"{}"{output}}}"#, state.revision, state.control.sequence, state.control.ledger.epoch, root.path("first.anchor").display())
}
fn selection_text(root: &Directory, current: u64, available: bool, next: u64, operation: u64) -> String {
    format!(r#"{{"schema_version":"fa.mediated-submit/1",
        "initial":{{"topology_path":"{}","inventory_path":"{}"}},
        "current":{{"topology_path":"{}","inventory_path":"{}"}},"available":{available},
        "next":{{"topology_path":"{}","inventory_path":"{}"}},"operation":{operation},
        "source":{{"checks":2,"reserved_read_bytes":{},"last_operation":2}}}}"#,
        root.input(1, false).display(), root.path("saved-inventory.json").display(),
        root.input(current, false).display(), root.path("saved-inventory.json").display(),
        root.input(next, true).display(), root.path("live-inventory.json").display(), 2 * TOPOLOGY_REFRESH_READ_BYTES)
}
fn selection(root: &Directory, current: u64, available: bool, next: u64, operation: u64) -> Selection {
    Selection::decode(selection_text(root, current, available, next, operation).as_bytes()).unwrap()
}
fn second(root: &Directory, choice: ReviewDecision) -> (Vec<u8>, RunResult) {
    let config = configured(root, 3);
    let document = document(&config, 2, 3, 2, ElapsedTick(100000));
    let recovery = SubmitProfile::decode(recovery_text(root, Some("second.anchor")).as_bytes()).unwrap();
    let human = reviewer(&config, 2, choice);
    let result = submit(config, Submission { document: &document, profile: &recovery, peers: None, credibility: None },
        &selection(root, 1, true, 2, 100), || ElapsedTick(1001)).unwrap();
    human.join().unwrap(); (document, result)
}
#[test]
fn second_publication_preserves_the_first_charge_and_recovers_receipts_without_live_assets() {
    let root = Directory::new(); seed(&root);
    let (original, result) = second(&root, ReviewDecision::Approve);
    assert!(result.failure.is_none(), "{:?}", result.failure); assert!(executed(&result));
    assert_eq!(result.cleanup_pending, 0);
    let mut config = configured(&root, 3);
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 2); assert_eq!(state.payload, b"second");
    assert_eq!(state.control.ledger.charged, 11); assert_eq!(state.control.ledger.reserved, 0);
    let anchor = FileHistoryAnchor::read_trusted(&mut io::Cursor::new(fs::read(root.path("second.anchor")).unwrap()), MAX_HISTORY_ANCHOR_BYTES).unwrap();
    assert_eq!(anchor.revision(), state.revision);
    fs::remove_file(root.path("evidence.json")).unwrap(); fs::remove_file(root.path("live-inventory.json")).unwrap();
    fs::remove_file(root.input(2, true)).unwrap(); config.programs.clear();
    let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
    let absent = root.path("absent-credibility.json");
    // Retain the completed two-check continuation, not the older creation work.
    // Receipt recovery validates shape but never spends this source allowance.
    let receipt_text = selection_text(&root, 2, true, 3, 200)
        .replace("\"checks\":2", "\"checks\":4")
        .replace(&format!("\"reserved_read_bytes\":{}", 2 * TOPOLOGY_REFRESH_READ_BYTES),
            &format!("\"reserved_read_bytes\":{}", 4 * TOPOLOGY_REFRESH_READ_BYTES))
        .replace("\"last_operation\":2", "\"last_operation\":102");
    let receipt = Selection::decode(receipt_text.as_bytes()).unwrap();
    let retried = submit(config, Submission { document: &original, profile: &recovery, peers: None, credibility: Some(&absent) },
        &receipt, || ElapsedTick(1000000)).unwrap();
    assert!(executed(&retried)); assert!(retried.failure.is_none());
    let mut changed = decode_command(&original).unwrap();
    let Command::Submit { proposal, .. } = &mut changed else { unreachable!() }; proposal.payload = b"other".to_vec();
    let changed = encode_command(&changed).unwrap();
    let mut config = configured(&root, 5); config.programs.clear();
    let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
    let receipt = Selection::decode(receipt_text.replace("\"available\":true", "\"available\":false").as_bytes()).unwrap();
    let conflict = submit(config, Submission { document: &changed, profile: &recovery, peers: None, credibility: Some(&absent) },
        &receipt, || ElapsedTick(1000001)).unwrap();
    assert!(conflict.response.result.is_err());
    let config = configured(&root, 0); let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 2); assert_eq!(after.control.ledger.charged, 11);
}
#[test]
fn loss_after_dispatch_seals_only_the_second_effect_and_keeps_the_original_publication() {
    let root = Directory::new(); seed(&root); let config = configured(&root, 3);
    let document = document(&config, 2, 3, 2, ElapsedTick(100000));
    let recovery = SubmitProfile::decode(recovery_text(&root, Some("loss.anchor")).as_bytes()).unwrap();
    let human = reviewer(&config, 2, ReviewDecision::Approve);
    let store = config.store.clone(); let profile = config.profile.clone(); let mut removed = false;
    let result = submit(config, Submission { document: &document, profile: &recovery, peers: None, credibility: None },
        &selection(&root, 1, true, 2, 100), || {
            let state = FileOversight::read_publication(&store, &profile).unwrap();
            if state.executions == 1 && state.control.ledger.charged > 5 && !removed {
                fs::remove_file(root.path("live-inventory.json")).unwrap(); removed = true;
            }
            ElapsedTick(1001)
        }).unwrap();
    human.join().unwrap(); assert!(removed); assert!(!executed(&result));
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(state.executions, 1); assert_eq!(state.payload, b"first"); assert_eq!(state.control.ledger.charged, 5);
    assert_eq!(state.control.ledger.reserved, 0); assert_eq!(result.cleanup_pending, 0);
}
#[test]
fn every_historical_replay_checkpoint_cancels_without_a_fence_or_staging_cleanup() {
    let root = Directory::new(); let original = seed(&root);
    let config = configured(&root, 0); let count = FileOversight::read_publication(&config.store, &config.profile).unwrap().revision as usize;
    fs::write(root.path("store/delivery.pending"), b"preserve").unwrap(); let before = root.bytes();
    let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
    // Two outer checkpoints, four per independent historical input pair, and
    // a checkpoint before each original event plus one at Ready.
    for boundary in 1..=count + 11 {
        let mut config = configured(&root, 3); config.programs.clear(); let mut calls = 0;
        let result = submit_with_elapsed(config, Submission { document: &original, profile: &recovery, peers: None, credibility: None },
            &selection(&root, 1, true, 2, 100), || ElapsedTick(1001), Instant::now(), &mut || {
                calls += 1; if calls == boundary { Duration::from_secs(1000) } else { Duration::ZERO }
            });
        assert!(matches!(result, Err(error) if error.contains("replay deadline")), "boundary {boundary}");
        assert_eq!(calls, boundary); assert_eq!(root.bytes(), before);
        assert_eq!(fs::read(root.path("store/delivery.pending")).unwrap(), b"preserve");
    }
    let mut calls = 0; let config = configured(&root, 0);
    let result = submit_with_elapsed(config, Submission { document: &original, profile: &recovery, peers: None, credibility: None },
        &selection(&root, 1, true, 2, 100), || ElapsedTick(1001), Instant::now(), &mut || { calls += 1; Duration::ZERO }).unwrap();
    assert_eq!(calls, count + 11); assert!(executed(&result));
}
#[test]
fn incompatible_history_refuses_before_mutation_and_missing_next_input_never_admits_a_request() {
    for bad_history in [false, true] {
        let root = Directory::new(); seed(&root); let mut config = configured(&root, 3); config.programs.clear();
        let document = document(&config, 2, 3, 2, ElapsedTick(100000));
        let recovery = SubmitProfile::decode(recovery_text(&root, Some("failure.anchor")).as_bytes()).unwrap();
        let before = root.bytes();
        if !bad_history { fs::remove_file(root.input(2, true)).unwrap(); }
        let result = submit(config, Submission { document: &document, profile: &recovery, peers: None, credibility: None },
            &selection(&root, if bad_history { 2 } else { 1 }, true, 2, 100), || ElapsedTick(1001));
        assert!(result.is_err());
        if bad_history { assert_eq!(root.bytes(), before); assert!(!root.path("failure.anchor").exists()); }
        else { assert!(root.path("failure.anchor").exists()); }
        let config = configured(&root, 0); let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 1); assert_eq!(state.control.ledger.charged, 5); assert_eq!(state.control.ledger.stages.len(), 1);
    }
}
#[test]
fn old_generation_and_changed_source_allowance_cannot_be_adopted_as_the_new_contract() {
    for altered in [false, true] {
        let root = Directory::new(); seed(&root); let mut config = configured(&root, 3); config.programs.clear();
        let document = document(&config, 2, 3, 2, ElapsedTick(100000));
        let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
        let changed = if altered { graph_document(2).replace("\"checks\":8", "\"checks\":9") } else { graph_document(1) };
        fs::write(root.input(2, true), changed).unwrap();
        let result = submit(config, Submission { document: &document, profile: &recovery, peers: None, credibility: None },
            &selection(&root, 1, true, if altered { 2 } else { 1 }, 100), || ElapsedTick(1001));
        assert!(result.is_err());
        let config = configured(&root, 0); let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, 1); assert_eq!(state.control.ledger.charged, 5);
    }
}
#[test]
fn stale_actor_epoch_is_not_rewritten_and_a_human_rejection_does_not_publish() {
    let root = Directory::new(); seed(&root); let mut config = configured(&root, 0); config.programs.clear();
    let document = document(&config, 2, 0, 2, ElapsedTick(100000));
    let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
    let stale = submit(config, Submission { document: &document, profile: &recovery, peers: None, credibility: None },
        &selection(&root, 1, true, 2, 100), || ElapsedTick(1001)).unwrap();
    assert!(!executed(&stale)); assert_eq!(stale.cleanup_pending, 0);
    let config = configured(&root, 0); let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1); assert_eq!(state.control.ledger.charged, 5);
    assert_eq!(state.control.ledger.reserved, 0); assert!(state.stop.is_none()); assert!(!config.socket(2).exists());
    let root = Directory::new(); seed(&root); let (_, rejected) = second(&root, ReviewDecision::Reject);
    assert!(!executed(&rejected)); assert_eq!(rejected.cleanup_pending, 0);
    let config = configured(&root, 0); let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 1); assert_eq!(state.control.ledger.charged, 5);
}
#[test]
fn lost_archive_and_response_output_do_not_repeat_or_erase_the_second_publication() {
    let root = Directory::new(); seed(&root); let config = configured(&root, 3);
    let document = document(&config, 2, 3, 2, ElapsedTick(100000));
    let recovery = SubmitProfile::decode(recovery_text(&root, Some("racing.anchor")).as_bytes()).unwrap();
    let human = reviewer(&config, 2, ReviewDecision::Approve);
    let store = config.store.clone(); let profile = config.profile.clone(); let mut raced = false;
    let result = submit(config, Submission { document: &document, profile: &recovery, peers: None, credibility: None },
        &selection(&root, 1, true, 2, 100), || {
            if !raced && FileOversight::read_publication(&store, &profile).unwrap().executions == 2 {
                fs::write(root.path("racing.anchor"), b"occupied").unwrap(); raced = true;
            }
            ElapsedTick(1001)
        }).unwrap();
    human.join().unwrap(); assert!(raced); assert!(executed(&result));
    assert!(result.failure.as_ref().unwrap().contains("history anchor retention failed"));
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let error = super::super::emit(result, &mut Broken).unwrap_err();
    assert!(error.contains("retention failed")); assert!(error.contains("output failed"));
    assert_eq!(fs::read(root.path("racing.anchor")).unwrap(), b"occupied");
    let state = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(state.executions, 2); assert_eq!(state.control.ledger.charged, 11);
}
#[test]
fn actual_top_level_submit_selects_the_mediated_workflow() {
    let root = Directory::new(); seed(&root); let config = configured(&root, 3);
    let now = crate::workflow::clock();
    let document = document(&config, 2, 3, 2, ElapsedTick(now.0 + 60000));
    fs::write(root.path("submit.json"), document).unwrap();
    fs::write(root.path("recovery.json"), recovery_text(&root, Some("cli.anchor"))).unwrap();
    fs::write(root.path("topology-profile.json"), selection_text(&root, 1, true, 2, 100)).unwrap();
    // Literal fixture substitutions only; production still uses Config's closed
    // parser and native profile constructors, not this test's assembled object.
    let mut text = TEMPLATE.replace("/operator/publication-store", root.path("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.path("evidence.json").to_str().unwrap())
        .replace("/operator/helpers/alpha", std::env::current_exe().unwrap().to_str().unwrap())
        .replace("/operator/helpers/beta", std::env::current_exe().unwrap().to_str().unwrap())
        .replace("/operator/helpers", root.0.to_str().unwrap())
        .replace("\"arguments\": []", &format!(r#""arguments": ["--exact", "{CHILD}", "--nocapture"]"#));
    for member in ["alpha", "beta"] {
        text = text.replacen("\"environment\": {}", &format!(r#""environment": {{"{MEMBER}":"{member}","{EPOCH}":"3"}}"#), 1);
    }
    Config::decode(text.as_bytes()).unwrap(); fs::write(root.path("config.json"), text).unwrap();
    let human = reviewer(&config, 2, ReviewDecision::Approve);
    crate::command(vec!["submit".into(), root.path("config.json").display().to_string(), root.path("submit.json").display().to_string(),
        "--guarded".into(), root.path("recovery.json").display().to_string(),
        "--topology-profile".into(), root.path("topology-profile.json").display().to_string()]).unwrap();
    human.join().unwrap();
    let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(state.executions, 2); assert_eq!(state.control.ledger.charged, 11); assert!(root.path("cli.anchor").exists());
}
#[test]
fn closed_command_and_profile_selection_cannot_fall_back_to_legacy_or_terminal_modes() {
    let root = Directory::new();
    let valid = selection_text(&root, 1, true, 2, 100); Selection::decode(valid.as_bytes()).unwrap();
    for text in [valid.replace("fa.mediated-submit/1", "fa.mediated-create/1"),
        valid.replace("\"operation\":100", "\"operation\":0"),
        valid.replace("\"operation\":100", "\"operation\":18446744073709551615"),
        valid.replace("\"available\":true", "\"available\":null"),
        valid.replace("\"operation\":100", "\"operation\":100,\"extra\":true"),
        valid.replace("\"checks\":2", "\"checks\":3"),
        valid.replace("\"last_operation\":2", "\"last_operation\":1"),
        valid.replace("\"last_operation\":2", "\"last_operation\":100"),
        valid.replace("\"last_operation\":2", "\"last_operation\":2,\"unpriced\":true")] {
        assert!(Selection::decode(text.as_bytes()).is_err());
    }
    let valid = vec!["submit", "config", "document", "--guarded", "recovery", "--topology-profile", "topology"]
        .into_iter().map(String::from).collect::<Vec<_>>();
    assert!(arguments(&valid).is_ok());
    for mode in ["create", "resume", "recover-stop", "submit-checked"] {
        let mut args = valid.clone(); args[0] = mode.into(); assert!(arguments(&args).is_err());
    }
    let mut duplicate = valid.clone(); duplicate[5] = "--guarded".into(); assert!(arguments(&duplicate).is_err());
    let mut unsupported = valid; unsupported[5] = "--unknown".into(); assert!(arguments(&unsupported).is_err());
}

#[test]
fn retained_work_must_leave_both_required_checks_instead_of_resetting_source_allowances() {
    for checks in [6, 7] {
        let root = Directory::new(); seed_with_checks(&root, checks - 2);
        let config = configured(&root, 3); let document = document(&config, 2, 3, 2, ElapsedTick(100000));
        let recovery = SubmitProfile::decode(recovery_text(&root, None).as_bytes()).unwrap();
        let text = selection_text(&root, 1, true, 2, 100)
            .replace("\"checks\":2", &format!("\"checks\":{checks}"))
            .replace(&format!("\"reserved_read_bytes\":{}", 2 * TOPOLOGY_REFRESH_READ_BYTES),
                &format!("\"reserved_read_bytes\":{}", checks * TOPOLOGY_REFRESH_READ_BYTES))
            .replace("\"last_operation\":2", &format!("\"last_operation\":{checks}"));
        let selected = Selection::decode(text.as_bytes()).unwrap();
        let human = (checks == 6).then(|| reviewer(&config, 2, ReviewDecision::Approve));
        let result = submit(config, Submission { document: &document, profile: &recovery, peers: None, credibility: None },
            &selected, || ElapsedTick(1001));
        if let Some(human) = human {
            human.join().unwrap(); let result = result.unwrap(); assert!(executed(&result)); assert!(result.failure.is_none());
        } else { assert!(matches!(result, Err(error) if error.contains("topology budget"))); }
        let config = configured(&root, 0); let state = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(state.executions, if checks == 6 { 2 } else { 1 });
        assert_eq!(state.control.ledger.charged, if checks == 6 { 11 } else { 5 });
    }
}
