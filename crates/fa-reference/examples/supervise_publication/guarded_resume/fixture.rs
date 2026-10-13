//! Actual original source-publication workflow; synthetic votes are not calibration.
use super::*;
use crate::config::Config;
use crate::workflow;
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::actor_wire::encode_command;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::Snapshot;
use std::{collections::BTreeMap, ffi::OsString, fs, io, path::{Path, PathBuf},
    os::unix::net::UnixStream, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Root(pub(super) PathBuf);
impl Root {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-guarded-receipt-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("guarded resume cleanup: {error}"); } }
}
pub(super) struct Case {
    pub(super) root: Root,
    pub(super) config: Config,
    pub(super) document: Vec<u8>,
    pub(super) anchor: PathBuf,
    pub(super) archive: Vec<u8>,
    pub(super) before: FileDeliverySnapshot,
}
fn configured(root: &Root) -> Config {
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
pub(super) fn document(c: &Config, request: u64, payload: &[u8]) -> Vec<u8> {
    encode_command(&Command::Submit { request, proposal: ActorProposal {
        target: c.profile.delivery.target, payload: payload.to_vec(), units: 4,
        deadline: ElapsedTick(100000), expected_policy_epoch: 0,
    }}).unwrap()
}
fn evidence(root: &Root, c: &Config) {
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    fs::write(root.0.join("evidence.json"), evidence.encode()).unwrap();
}
fn retained(root: Root, mut config: Config, document: Vec<u8>, host: FileOversight) -> Case {
    let before = host.inspect(); let mut archive = Vec::new();
    host.history_anchor().unwrap().write_to(&mut archive, MAX_HISTORY_ANCHOR_BYTES).unwrap();
    drop(host);
    let anchor = root.0.join("independent.anchor"); fs::write(&anchor, &archive).unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap(); config.programs.clear();
    fs::write(config.store.join("delivery.pending"), b"retain until acknowledgment").unwrap();
    Case { root, config, document, anchor, archive, before }
}
pub(super) fn queued() -> Case {
    let root = Root::new(); let mut c = configured(&root); evidence(&root, &c);
    let (mut host, _) = FileOversight::create(&c.store, c.profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    host.enable_file_source(host.revision(), c.source_policy).unwrap();
    let observed = host.refresh_file_source(host.revision(), &mut c.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let doc = document(&c, 1, b"once");
    let (port, mut driver) = host.into_supervised_driver(); let mut wire = ActorWire::new(port);
    let revision = driver.supervisor().host().unwrap().revision();
    driver.supervisor_mut().set_snapshot(revision, Some(observed.snapshot().clone())).unwrap();
    assert!(wire.exchange(&doc).result.is_ok());
    assert!(driver.supervisor().host().unwrap().request_status(1).is_ok());
    drop(wire); drop(driver);
    // This original fence only prepares the fixture's retained cut; it does not
    // run the new guarded resume code or manufacture a receipt.
    let (host, _) = FileOversight::open(&c.store, c.profile.clone()).unwrap();
    retained(root, c, doc, host)
}
fn reviewer(c: &Config) -> std::thread::JoinHandle<()> {
    let path = c.socket(1);
    let expected = ReviewerExpectation { reviewer: c.profile.human.reviewer_id,
        scope: c.profile.delivery.scope, clock_domain: CLOCK_DOMAIN };
    std::thread::spawn(move || {
        let start = Instant::now();
        let stream = loop {
            match UnixStream::connect(&path) {
                Ok(stream) => break stream,
                Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(start.elapsed() < Duration::from_secs(10)); std::thread::sleep(Duration::from_millis(1));
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
pub(super) fn published(expire: bool) -> (Case, u64) {
    let root = Root::new(); let c = configured(&root); evidence(&root, &c);
    let doc = document(&c, 1, b"once"); let human = reviewer(&c);
    let late = 100001 + c.profile.delivery.retention_ticks;
    let store = c.store.clone(); let bootstrap = c.profile.clone(); let mut expired = false;
    let result = workflow::run(c, &doc, false, || {
        if expire && !expired && let Ok(disk) = FileOversight::read_publication(&store, &bootstrap)
            && disk.control.ledger.charged != 0 && disk.executions == 0 { expired = true; }
        ElapsedTick(if expired { late } else { 1000 })
    }).unwrap(); human.join().unwrap();
    assert_eq!(expired, expire);
    assert_eq!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })), !expire);
    let c = configured(&root); let (host, _) = FileOversight::open(&c.store, c.profile.clone()).unwrap();
    (retained(root, c, doc, host), if expired { late + 1 } else { 1001 })
}
pub(super) fn profile_text(case: &Case, output: Option<&Path>) -> String {
    let retention = output.map_or_else(String::new, |p| format!(",\"retain_history_anchor\":\"{}\"", p.display()));
    format!("{{\"schema_version\":\"fa.guarded-resume/1\",\"minimum\":{{\"journal_revision\":{},\"control_sequence\":{},\"authority_epoch\":{}}},\"history_anchor\":\"{}\"{retention}}}",
        case.before.revision, case.before.control.sequence, case.before.control.ledger.epoch, case.anchor.display())
}
pub(super) fn profile(case: &Case, output: Option<&Path>) -> ResumeProfile {
    ResumeProfile::decode(profile_text(case, output).as_bytes()).unwrap()
}
pub(super) fn load(path: &Path) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(&mut std::io::Cursor::new(fs::read(path).unwrap()), MAX_HISTORY_ANCHOR_BYTES).unwrap()
}
pub(super) fn files(case: &Case) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&case.config.store).unwrap().map(|entry| {
        let entry = entry.unwrap(); assert!(entry.file_type().unwrap().is_file());
        let path = entry.path(); let bytes = fs::read(&path).unwrap(); (path, bytes)
    }).collect()
}
pub(super) fn state(case: &Case) -> FileDeliverySnapshot {
    FileOversight::read_publication(&case.config.store, &case.config.profile).unwrap()
}
pub(super) fn write_config(case: &Case) -> PathBuf {
    let text = include_str!("../../../fixtures/supervised_publication.json")
        .replace("/operator/publication-store", case.config.store.to_str().unwrap())
        .replace("/operator/evidence.json", case.root.0.join("evidence.json").to_str().unwrap());
    let path = case.root.0.join("config.json"); fs::write(&path, text).unwrap(); path
}
