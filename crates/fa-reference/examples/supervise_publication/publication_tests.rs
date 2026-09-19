//! Real supervisor/child/reviewer/file paths. The child deliberately returns a
//! synthetic verdict; these tests assert enforcement, not detector accuracy.
use super::{config::{Config, CLOCK_DOMAIN}, publication::PublicationProfile, workflow};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureIdentity, FilePublicationCapture};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationInputs, FileWitnessInput};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
const TEMPLATE: &[u8] = include_bytes!("../../fixtures/supervised_publication.json");
const RECIPE: &str = r#"[{"kind":"exact_value","key":0,"role":"subject"},{"kind":"absent_key","key":1},{"kind":"empty_range","start":6,"end":9},{"kind":"range_members","start":2,"end":6}]"#;
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-checked-run-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("checked example cleanup: {error:?}"); }
    }
}
fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(TEMPLATE).unwrap();
    c.store = root.0.join("store");
    c.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, c.profile.delivery.scope, 1048576).unwrap();
    c.timing.poll_ms = 1; c.timing.runtime_ms = 15000; c.timing.cleanup_ms = 2000;
    c.programs = ["alpha", "beta"].into_iter().map(|name| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), "tests::synthetic_helper_process".into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(name))])).unwrap();
        (name.to_owned(), program)
    }).collect();
    c
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn evidence(root: &Directory, c: &Config) {
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        snapshot(), ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    std::fs::write(root.0.join("evidence.json"), data.encode()).unwrap();
}
fn proposal(c: &Config, payload: &[u8]) -> Vec<u8> {
    encode_command(&Command::Submit { request: 1, proposal: ActorProposal {
        target: c.profile.delivery.target, payload: payload.to_vec(), units: payload.len() as u64,
        deadline: ElapsedTick(100000), expected_policy_epoch: 0,
    }}).unwrap()
}
fn action(root: &Directory, c: &Config, payload: &[u8]) -> FrozenAction {
    // The ORIGINAL policy produces the reference frozen action, not a manually
    // guessed action digest or a replacement authorization calculation.
    let (mut host, _) = FileOversight::create(root.0.join("capture-owner"), c.profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: c.profile.delivery.scope,
        target: Some(c.profile.delivery.target), payload: payload.to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100000), units: payload.len() as u64 }, snapshot()).unwrap()
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
    std::fs::write(&pending, capture.to_bytes().unwrap()).unwrap(); std::fs::rename(pending, path).unwrap();
}
fn profile_json(root: &Directory) -> String {
    format!(r#"{{"schema":"fa.supervised-witnesses/1","source":91,"original":"{}/original.bin","current":"{}/current.bin","limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":{RECIPE}}}"#,
        root.0.display(), root.0.display())
}
fn prepare(root: &Directory, c: &Config, payload: &[u8]) -> (PublicationProfile, FrozenAction) {
    evidence(root, c); let action = action(root, c, payload); let original = packet(&action, 1, &[0, 2, 4]);
    write(&root.0.join("original.bin"), &original); write(&root.0.join("current.bin"), &original);
    (PublicationProfile::decode(profile_json(root).as_bytes()).unwrap(), action)
}
fn reviewer(c: &Config) -> std::thread::JoinHandle<()> {
    let path = c.socket(1);
    let expected = ReviewerExpectation { reviewer: c.profile.human.reviewer_id, scope: c.profile.delivery.scope, clock_domain: CLOCK_DOMAIN };
    std::thread::spawn(move || {
        let start = Instant::now();
        let stream = loop {
            match UnixStream::connect(&path) {
                Ok(s) => break s,
                Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(start.elapsed() < Duration::from_secs(10), "no human review offer");
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("reviewer connection: {e:?}"),
            }
        };
        let mut client = ReviewerClient::from_unix(stream, expected).unwrap();
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(ReviewDecision::Approve).unwrap(),
                ReviewClientProgress::Complete => break,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn executed(result: &workflow::RunResult) -> bool {
    matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}

#[test]
fn witness_profile_accepts_all_exact_lanes_and_rejects_malformed_or_weakened_recipes() {
    let root = Directory::new(); let valid = profile_json(&root);
    assert!(PublicationProfile::decode(valid.as_bytes()).is_ok());
    for invalid in [valid.replace("/1\"", "/2\""), valid.replace("\"source\":91", "\"source\":0"),
        valid.replace("\"bindings\":8", "\"bindings\":17"), valid.replace(RECIPE, "[]"),
        valid.replace("\"source\":91", "\"source\":91,\"extra\":true"),
        valid.replace("\"source\":91", "\"source\":91,\"source\":92"),
        valid.replace("\"subject\"", "\"allow\""), valid.replace("\"start\":6,\"end\":9", "\"start\":9,\"end\":9"),
        valid.replace(RECIPE, r#"[{"kind":"absent_key","key":1},{"kind":"exact_value","key":1,"role":"subject"}]"#)] {
        assert!(PublicationProfile::decode(invalid.as_bytes()).is_err(), "{invalid}");
    }
    assert!(!root.0.join("store").exists());
}

#[test]
fn checked_supervision_executes_through_both_original_keys_and_retains_requirements() {
    let root = Directory::new(); let c = configured(&root); let payload = b"checked once";
    let (profile, _) = prepare(&root, &c, payload); let document = proposal(&c, payload);
    let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
    human.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure); assert!(executed(&result)); assert_eq!(result.cleanup_pending, 0);
    let c = configured(&root);
    let (host, _) = FileOversight::open_with_publication_validation(&c.store, c.profile, profile.limits).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, payload);
    assert_eq!(host.inspect().control.ledger.charged, payload.len() as u64);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.publication_source(1).unwrap().unwrap().source, 91);
    assert_eq!(host.retained_publication_evidence(1).unwrap().requests().len(), 4);
}

#[test]
fn current_phantom_never_replaces_original_absence_while_unrelated_changes_publish() {
    for phantom in [false, true] {
        let root = Directory::new(); let c = configured(&root); let payload = b"dependency";
        let (profile, action) = prepare(&root, &c, payload); let document = proposal(&c, payload);
        let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 99] };
        write(&root.0.join("current.bin"), &packet(&action, 2, keys));
        let human = reviewer(&c);
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
        human.join().unwrap(); assert_eq!(executed(&result), !phantom);
        let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(disk.executions, u64::from(!phantom)); assert_eq!(disk.control.ledger.reserved, 0);
        if phantom { assert_eq!(disk.payload, b"initial"); assert_eq!(disk.control.ledger.available, c.profile.delivery.total); }
    }
}

#[test]
fn final_publication_rereads_after_dispatch_and_settles_original_nonexecution() {
    for phantom in [false, true] {
        let root = Directory::new(); let c = configured(&root); let payload = b"final cut";
        let (profile, action) = prepare(&root, &c, payload); let document = proposal(&c, payload);
        let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut changed = false;
        let human = reviewer(&c);
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 99] };
                write(&root.0.join("current.bin"), &packet(&action, 2, keys)); changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed); assert_eq!(executed(&result), !phantom);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(!phantom)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if phantom { 0 } else { payload.len() as u64 });
    }
}

#[test]
fn missing_original_or_different_frozen_action_refuses_before_any_helper_launch() {
    for changed_action in [false, true] {
        let root = Directory::new(); let mut c = configured(&root); let payload = b"original";
        let (profile, _) = prepare(&root, &c, payload);
        if !changed_action { std::fs::remove_file(root.0.join("original.bin")).unwrap(); }
        let document = proposal(&c, if changed_action { b"replacement" } else { payload });
        // A missing roster would fail launch. The required earlier failure must
        // identify original acquisition/action binding instead, not roster checks.
        c.programs.clear();
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
        let failure = result.failure.as_ref().unwrap();
        assert!(failure.contains(if changed_action { "Binding" } else { "NotFound" }), "{failure}");
        assert!(!executed(&result)); assert_eq!(result.cleanup_pending, 0);
        let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(disk.executions, 0); assert_eq!(disk.control.ledger.charged, 0);
    }
}

#[test]
fn missing_current_image_cannot_fall_back_to_original_or_human_approval() {
    let root = Directory::new(); let c = configured(&root); let payload = b"read required";
    let (profile, _) = prepare(&root, &c, payload); let document = proposal(&c, payload);
    std::fs::remove_file(root.0.join("current.bin")).unwrap();
    let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(!executed(&result)); assert!(result.failure.is_some());
    let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 0); assert_eq!(disk.control.ledger.reserved, 0); assert_eq!(disk.control.ledger.charged, 0);
}

#[test]
fn checked_resume_is_receipt_only_and_wrong_limits_refuse_before_recovery_fence() {
    let root = Directory::new(); let c = configured(&root); let payload = b"receipt only";
    let (profile, _) = prepare(&root, &c, payload); let document = proposal(&c, payload); let human = reviewer(&c);
    assert!(executed(&workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap()));
    human.join().unwrap();
    for name in ["original.bin", "current.bin", "evidence.json"] { std::fs::remove_file(root.0.join(name)).unwrap(); }
    let mut c = configured(&root); c.programs.clear();
    let result = workflow::run_with_publication(c, &document, true, None, Some(&profile), || ElapsedTick(1001)).unwrap();
    assert!(executed(&result)); assert!(result.failure.is_none()); assert_eq!(result.cleanup_pending, 0);
    let c = configured(&root); let before = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    let wrong = PublicationProfile::decode(profile_json(&root).replace("\"steps\":10000", "\"steps\":10001").as_bytes()).unwrap();
    assert!(workflow::run_with_publication(c, &document, true, None, Some(&wrong), || ElapsedTick(1002)).is_err());
    let c = configured(&root); assert_eq!(FileOversight::read_publication(&c.store, &c.profile).unwrap(), before);
}

#[path = "publication_feed_tests.rs"]
mod feed_tests;
