//! Runnable helper/reviewer workflows. Synthetic helper votes exercise native
//! enforcement and liveness, not detector accuracy or authenticated deployment.
use super::*;
use crate::{config::{Config, CLOCK_DOMAIN}, workflow};
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{FileCaptureIdentity, FilePublicationCapture};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedBatch;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationInputs, FileWitnessInput};
use fa_reference::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationInputCut};
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::{PublicationHeartbeat};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use fa_reference::witness::refinement::index::routing::WitnessChange;
use fa_reference::Snapshot;
use std::ffi::OsString;
use std::io;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-supervised-wait-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("supervised wait cleanup: {error}"); }
    }
}
fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(include_bytes!("../../../fixtures/supervised_publication.json")).unwrap();
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
fn proposal(c: &Config, payload: &[u8]) -> Vec<u8> {
    encode_command(&Command::Submit { request: 1, proposal: ActorProposal {
        target: c.profile.delivery.target, payload: payload.to_vec(), units: payload.len() as u64,
        deadline: ElapsedTick(100000), expected_policy_epoch: 0,
    } }).unwrap()
}
fn packet(action: &FrozenAction, generation: u64, through: u64, keys: &[u64]) -> FilePublicationCapture {
    let key = ProjectionKey { source: 40, branch: action.spec().scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    let input = FileWitnessInput::new(generation, generation, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        keys.iter().map(|&key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap();
    FilePublicationCapture::new_at_cut(1, FileCaptureIdentity { source: 91, generation }, action,
        FilePublicationInputs::new(Some(input), None), PublicationInputCut { source: 41, through }).unwrap()
}
fn write(root: &Directory, name: &str, bytes: &[u8]) {
    let pending = root.0.join(format!("{name}.next"));
    std::fs::write(&pending, bytes).unwrap(); std::fs::rename(pending, root.0.join(name)).unwrap();
}
fn feed(root: &Directory, through: u64) {
    let batch = PublicationFeedBatch::new(PublicationHeartbeat { source: 41, clock_domain: CLOCK_DOMAIN,
        generation: through + 1, through, produced_at: ElapsedTick(1000) }, 0,
        (1..=through).map(|sequence| PublicationChange { source: 41, sequence, change: WitnessChange::All }).collect()).unwrap();
    write(root, "feed.bin", &batch.to_bytes().unwrap());
}
fn profile_json(root: &Directory, max_retries: u64) -> String {
    format!(r#"{{"schema":"fa.supervised-witnesses/4","max_retries":{max_retries},"source":91,"original":"{0}/original.bin","current":"{0}/current.bin","feed":{{"source":41,"path":"{0}/feed.bin","after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}}]}}"#, root.0.display())
}
fn prepare(root: &Directory, c: &Config, payload: &[u8]) -> FrozenAction {
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        snapshot(), ["alpha", "beta"].into_iter().map(|name| (name.to_owned(), format!("review this fixture: {name}").into_bytes())).collect()).unwrap();
    write(root, "evidence.json", &evidence.encode());
    // Only the fixture derives its matching original frame through native policy.
    // The runnable workflow never fabricates a second owner or rewrites proposals.
    let (mut host, _) = FileOversight::create(root.0.join("capture-owner"), c.profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: c.profile.delivery.scope,
        target: Some(c.profile.delivery.target), payload: payload.to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100000), units: payload.len() as u64 }, snapshot()).unwrap();
    let original = packet(&action, 1, 0, &[0, 2, 4]);
    write(root, "original.bin", &original.to_bytes().unwrap()); write(root, "current.bin", &original.to_bytes().unwrap());
    action
}
fn reviewer(c: &Config) -> std::thread::JoinHandle<()> {
    let path = c.socket(1);
    let expected = ReviewerExpectation { reviewer: c.profile.human.reviewer_id, scope: c.profile.delivery.scope, clock_domain: CLOCK_DOMAIN };
    std::thread::spawn(move || {
        let start = Instant::now();
        let stream = loop {
            match UnixStream::connect(&path) {
                Ok(stream) => break stream,
                Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(start.elapsed() < Duration::from_secs(10), "no human offer");
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("reviewer socket: {error}"),
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
fn captures(path: &Path) -> usize {
    // The initial source-binding event writes its fields, not a capture packet.
    // Only subsequent Captured/OrDefer records embed this exact packet header.
    // All fixture payloads/context strings are fixed and exclude this marker.
    std::fs::read(path).unwrap_or_default().windows(8).filter(|window| *window == b"FAPCAP02").count()
}

#[test]
fn waiting_schema_is_explicit_bounded_and_does_not_change_legacy_admission() {
    let root = Directory::new(); let valid = profile_json(&root, 2);
    assert!(PublicationProfile::decode(valid.as_bytes()).unwrap().wait.is_some());
    let legacy = valid.replace("fa.supervised-witnesses/4", "fa.supervised-witnesses/2").replace("\"max_retries\":2,", "");
    assert!(PublicationProfile::decode(legacy.as_bytes()).unwrap().wait.is_none());
    for invalid in [valid.replace("\"max_retries\":2,", ""), profile_json(&root, 0), profile_json(&root, 65),
        valid.replace("\"max_retries\":2", "\"max_retries\":\"2\""),
        valid.replace("\"max_retries\":2", "\"max_retries\":2,\"max_retries\":3"),
        valid.replace("fa.supervised-witnesses/4", "fa.supervised-witnesses/2"),
        valid.replace("\"feed\":", "\"missing_feed\":"),
        valid.replace("\"max_retries\":2", "\"max_retries\":2,\"retry_errors\":true")] {
        assert!(PublicationProfile::decode(invalid.as_bytes()).is_err(), "{invalid}");
    }
    assert!(!root.0.join("store").exists());
}

#[test]
fn runnable_wait_catches_up_without_new_review_but_does_not_rebase_changed_evidence() {
    for phantom in [false, true] {
        let root = Directory::new(); let c = configured(&root); let payload = b"wait for actual data";
        let action = prepare(&root, &c, payload); feed(&root, 1);
        let selected = PublicationProfile::decode(profile_json(&root, 2).as_bytes()).unwrap();
        let document = proposal(&c, payload); let human = reviewer(&c);
        let canonical = c.store.join("delivery.bin"); let mut changed = false;
        let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || {
            if !changed && captures(&canonical) != 0 {
                // This first deferred observation is already durable. Replacing
                // the file cannot change that observation or the frozen recipe.
                let keys: &[u64] = if phantom { &[0, 1, 2, 4] } else { &[0, 2, 4, 99] };
                write(&root, "current.bin", &packet(&action, 2, 1, keys).to_bytes().unwrap());
                changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed); assert_eq!(executed(&result), !phantom);
        let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert_eq!(disk.executions, u64::from(!phantom)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if phantom { 0 } else { payload.len() as u64 });
        if !phantom { assert!(result.failure.is_none(), "{:?}", result.failure); }
        assert_eq!(result.cleanup_pending, 0);
    }
}

#[test]
fn persistent_lag_uses_exact_finite_retry_allowance_then_native_stop_and_drain() {
    for retries in [1, 2] {
        let root = Directory::new(); let c = configured(&root); let payload = b"bounded wait";
        prepare(&root, &c, payload); feed(&root, 1);
        let selected = PublicationProfile::decode(profile_json(&root, retries).as_bytes()).unwrap();
        let document = proposal(&c, payload); let human = reviewer(&c); let canonical = c.store.join("delivery.bin");
        let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || ElapsedTick(1000)).unwrap();
        human.join().unwrap(); assert!(!executed(&result));
        assert!(result.failure.as_ref().unwrap().contains("retry budget exhausted"));
        assert_eq!(captures(&canonical), retries as usize + 1);
        let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
        assert!(disk.stop.is_some()); assert_eq!(disk.executions, 0);
        assert_eq!(disk.control.ledger.reserved, 0); assert_eq!(disk.control.ledger.charged, 0);
        assert_eq!(result.cleanup_pending, 0);
    }
}

#[test]
fn opted_in_waiting_cannot_synthesize_a_missing_initial_cut() {
    let root = Directory::new(); let c = configured(&root); let payload = b"no metadata upgrade";
    let action = prepare(&root, &c, payload); feed(&root, 0);
    let old = packet(&action, 1, 0, &[0, 2, 4]);
    let legacy = FilePublicationCapture::new(1, old.identity(), &action, old.inputs().clone()).unwrap();
    write(&root, "original.bin", &legacy.to_bytes().unwrap());
    let selected = PublicationProfile::decode(profile_json(&root, 2).as_bytes()).unwrap();
    let (mut host, _) = selected.create(&c.store, c.profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let mut spec = action.spec().clone(); spec.required_witnesses.clear();
    host.propose(host.revision(), 1, spec, snapshot()).unwrap();
    let before = host.revision();
    assert!(selected.prepare_with_clock(&mut host, 1, || panic!("raw preparation must not sample time")).is_err());
    assert_eq!(host.revision(), before); assert!(host.publication_source(1).unwrap().is_none());
}

#[test]
fn finished_waiting_work_recovers_without_reopening_sources_or_relaunching_helpers() {
    let root = Directory::new(); let c = configured(&root); let payload = b"settled once";
    let action = prepare(&root, &c, payload); feed(&root, 1);
    let selected = PublicationProfile::decode(profile_json(&root, 2).as_bytes()).unwrap();
    let document = proposal(&c, payload); let human = reviewer(&c); let canonical = c.store.join("delivery.bin");
    let mut changed = false;
    let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || {
        if !changed && captures(&canonical) != 0 {
            write(&root, "current.bin", &packet(&action, 2, 1, &[0, 2, 4]).to_bytes().unwrap()); changed = true;
        }
        ElapsedTick(1000)
    }).unwrap();
    human.join().unwrap(); assert!(changed); assert!(executed(&result));
    for name in ["original.bin", "current.bin", "feed.bin", "evidence.json"] { std::fs::remove_file(root.0.join(name)).unwrap(); }
    let mut c = configured(&root); c.programs.clear();
    let resumed = workflow::run_with_publication(c, &document, true, None, Some(&selected), || ElapsedTick(200000)).unwrap();
    assert!(executed(&resumed)); assert!(resumed.failure.is_none()); assert_eq!(resumed.cleanup_pending, 0);
    let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.charged, payload.len() as u64);
}

#[test]
fn lag_after_dispatch_does_not_enter_the_wait_loop_or_refund_an_unknown_charge() {
    let root = Directory::new(); let c = configured(&root); let payload = b"no second execution window";
    prepare(&root, &c, payload); feed(&root, 0);
    let selected = PublicationProfile::decode(profile_json(&root, 4).as_bytes()).unwrap();
    let document = proposal(&c, payload); let human = reviewer(&c);
    let store = c.store.clone(); let profile = c.profile.clone(); let mut changed = false;
    let result = workflow::run_with_publication(c, &document, false, None, Some(&selected), || {
        if !changed && let Ok(disk) = FileOversight::read_publication(&store, &profile)
            && disk.control.ledger.charged != 0 && disk.executions == 0 {
            feed(&root, 1); changed = true;
        }
        ElapsedTick(1000)
    }).unwrap();
    human.join().unwrap(); assert!(changed); assert!(!executed(&result)); assert!(result.failure.is_some());
    assert!(!result.failure.as_ref().unwrap().contains("retry budget exhausted"));
    let disk = FileOversight::read_publication(&store, &profile).unwrap();
    assert_eq!(disk.executions, 0); assert_eq!(disk.control.ledger.charged, payload.len() as u64);
    assert_eq!(disk.control.ledger.reserved, 0);
}
