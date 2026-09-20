//! Advanced producer startup through real helper-process and reviewer sockets.
//! Synthetic votes exercise control plumbing, not model accuracy or human identity.
use super::*;
use crate::{config::{Config, CLOCK_DOMAIN}, workflow};
use fa_reference::action::{ActionSpec, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationInputs, FileWitnessInput};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::FilePublicationProducer;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
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
        let path = std::env::temp_dir().join(format!("fa-producer-bootstrap-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("bootstrap cleanup: {error}"); }
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
    }).collect(); c
}
fn producer_identity(c: &Config) -> PublicationProducerProfile {
    PublicationProducerProfile { source: 91, scope: c.profile.delivery.scope, feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 }
}
fn recipe(root: &Directory, c: &Config) -> PublicationProfile {
    let s = c.profile.delivery.scope;
    let json = format!(r#"{{"schema":"fa.supervised-witnesses/3","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), s.tenant, s.principal, s.run, s.branch, s.authority);
    PublicationProfile::decode(json.as_bytes()).unwrap()
}
fn image(c: &Config, revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: c.profile.delivery.scope.branch, projection: 7, source_epoch: 1 };
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(marker).unwrap();
    FilePublicationInputs::new(Some(FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(marker)),
        keys.iter().map(|&key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap()), None)
}
fn advanced(root: &Directory, c: &Config) -> FilePublicationProducer {
    let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), producer_identity(c),
        image(c, 1, &[0]), ElapsedTick(1000)).unwrap();
    producer.publish(1, image(c, 2, &[0, 99]), ElapsedTick(1000)).unwrap();
    producer.publish(2, image(c, 3, &[0, 99, 100]), ElapsedTick(1000)).unwrap();
    assert_eq!(producer.image().batch().heartbeat().through, 2); producer
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn evidence(root: &Directory, c: &Config) {
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        snapshot(), ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("independent {m}").into_bytes())).collect()).unwrap();
    std::fs::write(root.0.join("evidence.json"), data.encode()).unwrap();
}
fn document(c: &Config) -> Vec<u8> {
    encode_command(&Command::Submit { request: 7, proposal: ActorProposal { target: c.profile.delivery.target,
        payload: b"publish".to_vec(), units: 7, deadline: ElapsedTick(100000), expected_policy_epoch: 0 } }).unwrap()
}
fn reviewer(c: &Config) -> std::thread::JoinHandle<()> {
    let path = c.socket(7);
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
fn preparation_catches_up_before_binding_without_claiming_fresh_current_evidence() {
    let root = Directory::new(); let c = configured(&root); let producer = advanced(&root, &c);
    let profile = recipe(&root, &c); let (mut host, _) = profile.create(&c.store, c.profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    host.propose(host.revision(), 42, ActionSpec { version: VERSION, scope: c.profile.delivery.scope,
        target: Some(c.profile.delivery.target), payload: b"actual frame".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100000), units: 12 }, snapshot()).unwrap();
    let revision = host.revision();
    assert!(profile.prepare(&mut host, 42).is_err(), "old path must not weaken cut admission");
    assert_eq!(host.revision(), revision);
    let prepared = profile.prepare_with_clock(&mut host, 42, || ElapsedTick(1000)).unwrap();
    assert_eq!(host.publication_change_status().unwrap().through, 2);
    assert_eq!(host.retained_publication_evidence(42).unwrap().original(), producer.image().inputs());
    assert_eq!(prepared.current.reader().read_capture().unwrap().identity().generation, 3);
    assert!(!host.publication_source(42).unwrap().unwrap().fresh);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn advanced_producer_workflow_executes_and_receipt_only_resume_needs_no_sources() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let profile = recipe(&root, &c); let producer = advanced(&root, &c);
    let bytes = document(&c); let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &bytes, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(executed(&result)); assert!(result.failure.is_none());
    assert_eq!(result.cleanup_pending, 0); drop(producer);
    std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut c = configured(&root); c.programs.clear(); let bootstrap = c.profile.clone(); let store = c.store.clone();
    let result = workflow::run_with_publication(c, &bytes, true, None, Some(&profile), || ElapsedTick(200000)).unwrap();
    assert!(executed(&result)); assert!(result.failure.is_none()); assert_eq!(result.cleanup_pending, 0);
    let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.charged, 7);
}

#[test]
fn advanced_startup_does_not_rebase_later_forbidden_changes() {
    for phantom in [false, true] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let profile = recipe(&root, &c); let mut producer = advanced(&root, &c);
        let next = image(&c, 4, if phantom { &[0, 1, 99, 100] } else { &[0, 99, 100, 200] });
        let bytes = document(&c); let bootstrap = c.profile.clone(); let store = c.store.clone();
        let human = reviewer(&c); let mut changed = false;
        let result = workflow::run_with_publication(c, &bytes, false, None, Some(&profile), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                producer.publish(3, next.clone(), ElapsedTick(1000)).unwrap(); changed = true;
            }
            ElapsedTick(1000)
        }).unwrap();
        human.join().unwrap(); assert!(changed); assert_eq!(executed(&result), !phantom);
        let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
        assert_eq!(disk.executions, u64::from(!phantom)); assert_eq!(disk.control.ledger.reserved, 0);
        assert_eq!(disk.control.ledger.charged, if phantom { 0 } else { 7 });
    }
}

#[test]
fn expired_producer_time_is_not_renewed_by_successful_startup_catch_up() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let profile = recipe(&root, &c); let _producer = advanced(&root, &c);
    let bytes = document(&c); let bootstrap = c.profile.clone(); let store = c.store.clone();
    let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &bytes, false, None, Some(&profile), || ElapsedTick(7000)).unwrap();
    human.join().unwrap(); assert!(!executed(&result)); assert!(result.failure.is_some());
    let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(disk.executions, 0); assert_eq!(disk.control.ledger.charged, 0);
    assert_eq!(disk.control.ledger.reserved, 0);
}
