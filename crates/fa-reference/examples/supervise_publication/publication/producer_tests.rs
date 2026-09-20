//! Real producer/actor/helper/reviewer plumbing. Synthetic helper verdicts below
//! exercise enforcement only, not model accuracy or authenticated deployment.
use super::*;
use crate::{config::{Config, CLOCK_DOMAIN}, workflow};
use fa_reference::action::{ActionSpec, FrozenAction, VERSION};
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
        let path = std::env::temp_dir().join(format!("fa-producer-command-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
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
fn identity(c: &Config) -> PublicationProducerProfile {
    PublicationProducerProfile { source: 91, scope: c.profile.delivery.scope, feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 }
}
fn recipe(root: &Directory, c: &Config) -> String {
    let s = c.profile.delivery.scope;
    format!(r#"{{"schema":"fa.supervised-witnesses/3","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), s.tenant, s.principal, s.run, s.branch, s.authority)
}
fn inputs(c: &Config, revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: c.profile.delivery.scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    FilePublicationInputs::new(Some(FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        keys.iter().map(|&key| SnapshotEntry::new(key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap()), None)
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn propose(host: &mut FileOversight, c: &Config, attempt: u64) -> FrozenAction {
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    host.propose(host.revision(), attempt, ActionSpec { version: VERSION, scope: c.profile.delivery.scope,
        target: Some(c.profile.delivery.target), payload: b"actual frame".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100000), units: 12 }, snapshot()).unwrap()
}
fn evidence(root: &Directory, c: &Config) {
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        snapshot(), ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
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
fn producer_profile_has_one_source_path_and_no_weaker_fallback() {
    let root = Directory::new(); let c = configured(&root); let valid = recipe(&root, &c);
    assert!(PublicationProfile::decode(valid.as_bytes()).is_ok());
    for invalid in [valid.replace("/3\"", "/1\""), valid.replace("/3\"", "/2\""),
        valid.replace("\"source\":91", "\"source\":0"), valid.replace("\"source\":41", "\"source\":0"),
        valid.replace("\"after\":0", "\"after\":0,\"path\":\"/fallback.bin\""),
        valid.replace("\"producer\":", "\"original\":\"/old.bin\",\"producer\":"),
        valid.replace("\"max_age_ms\":5000", "\"max_age_ms\":0"),
        valid.replace("unix_milliseconds", "saved_tick"),
        valid.replace("\"scope\":{", "\"scope\":{\"purpose\":\"effect\","),
        valid.replace("\"source\":91", "\"source\":91,\"source\":92")] {
        assert!(PublicationProfile::decode(invalid.as_bytes()).is_err(), "{invalid}");
    }
    assert!(!root.0.join("producer").exists()); assert!(!c.store.exists());
}

#[test]
fn original_producer_capture_uses_actual_owner_frame_and_retains_live_readers() {
    let root = Directory::new(); let c = configured(&root);
    let profile = PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap();
    let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), inputs(&c, 1, &[0]), ElapsedTick(1000)).unwrap();
    let (mut host, _) = profile.create(&c.store, c.profile.clone()).unwrap();
    let action = propose(&mut host, &c, 42);
    let prepared = profile.prepare(&mut host, 42).unwrap();
    let capture = prepared.current.reader().read_capture().unwrap();
    assert_eq!(capture, producer.image().capture(42, &action).unwrap());
    assert_eq!(host.publication_source(42).unwrap().unwrap().source, 91);
    assert_eq!(host.retained_publication_evidence(42).unwrap().requests().len(), 2);
    producer.publish(1, inputs(&c, 2, &[0, 99]), ElapsedTick(1001)).unwrap();
    assert_eq!(prepared.current.reader().read_capture().unwrap().identity().generation, 2);
    assert_eq!(prepared.feed.unwrap().reader.read_batch().unwrap().heartbeat().through, 1);
    assert_eq!(capture.identity().generation, 1, "original observation was not replaced by current data");
}

#[test]
fn foreign_or_missing_producer_refuses_before_binding_or_helper_launch() {
    for missing in [false, true] {
        let root = Directory::new(); let c = configured(&root);
        let profile = PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap();
        if !missing {
            let mut foreign = identity(&c); foreign.scope.run += 1;
            let _ = FilePublicationProducer::create(root.0.join("producer"), foreign, inputs(&c, 1, &[0]), ElapsedTick(1000)).unwrap();
        }
        let (mut host, _) = profile.create(&c.store, c.profile.clone()).unwrap();
        propose(&mut host, &c, 42); let revision = host.revision();
        assert!(profile.prepare(&mut host, 42).is_err());
        assert_eq!(host.revision(), revision); assert_eq!(host.inspect().executions, 0);
        assert!(host.publication_producer_reader(999, root.0.join("producer/delivery.bin"), identity(&c)).is_err());
        let mut wrong = identity(&c); wrong.scope.authority += 1;
        assert!(host.publication_producer_reader(42, root.0.join("producer/delivery.bin"), wrong).is_err());
    }
}

#[test]
fn checked_workflow_uses_derived_changes_after_dispatch_not_original_cached_bytes() {
    for phantom in [false, true] {
        let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
        let profile = PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap();
        let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), inputs(&c, 1, &[0]), ElapsedTick(1000)).unwrap();
        let changed_inputs = inputs(&c, 2, if phantom { &[0, 1] } else { &[0, 99] });
        let document = document(&c); let store = c.store.clone(); let bootstrap = c.profile.clone();
        let human = reviewer(&c); let mut changed = false;
        let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || {
            if !changed && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
                && disk.control.ledger.charged != 0 && disk.executions == 0 {
                producer.publish(1, changed_inputs.clone(), ElapsedTick(1000)).unwrap(); changed = true;
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
fn completed_producer_workflow_recovers_without_source_or_helper_access() {
    let root = Directory::new(); let c = configured(&root); evidence(&root, &c);
    let profile = PublicationProfile::decode(recipe(&root, &c).as_bytes()).unwrap();
    let (producer, _) = FilePublicationProducer::create(root.0.join("producer"), identity(&c), inputs(&c, 1, &[0]), ElapsedTick(1000)).unwrap();
    let document = document(&c); let human = reviewer(&c);
    let result = workflow::run_with_publication(c, &document, false, None, Some(&profile), || ElapsedTick(1000)).unwrap();
    human.join().unwrap(); assert!(executed(&result)); assert!(result.failure.is_none());
    drop(producer);
    std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut c = configured(&root); c.programs.clear();
    let recovered = workflow::run_with_publication(c, &document, true, None, Some(&profile), || ElapsedTick(200000)).unwrap();
    assert!(executed(&recovered)); assert!(recovered.failure.is_none()); assert_eq!(recovered.cleanup_pending, 0);
    let c = configured(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.charged, 7);
}
