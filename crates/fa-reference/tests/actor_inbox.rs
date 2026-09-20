//! Native durable intake and real connected sockets; no helper/model verdicts.
#![cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "../examples/supervise_publication/config.rs"]
mod config;
use config::Config;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, driver::FileSupervisedDriver};
use fa_reference::action::consequence::delivery::persistent::requests::{FileRequestDisposition, actor::FileActorInbox};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, Command, WireError, WireResponse, decode_response, encode_command, MAX_RESPONSE_BYTES};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    config: Config,
    driver: FileSupervisedDriver,
    inbox: FileActorInbox,
    client: UnixStream,
}
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-actor-inbox-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap();
        let mut config = Config::decode(include_bytes!("../fixtures/supervised_publication.json")).unwrap();
        config.store = root.join("store");
        config.source = FileEvidenceSource::new(root.join("evidence.json"), 51, config.profile.delivery.scope, 1048576).unwrap();
        let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        host.enable_file_source(host.revision(), config.source_policy).unwrap();
        let (port, driver) = host.into_supervised_driver();
        let (server, client) = UnixStream::pair().unwrap();
        client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let peer = PeerCredentials::observe(&server).unwrap();
        let policy = PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap();
        let mut inbox = FileActorInbox::new(policy, ActorWire::new(port), ChannelLimits::default(), 3).unwrap();
        inbox.attach(server).unwrap();
        let fixture = Self { root, config, driver, inbox, client };
        fixture.evidence(); fixture
    }
    fn evidence(&self) {
        let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: self.config.profile.delivery.scope },
            Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
            ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
        std::fs::write(self.root.join("evidence.json"), data.encode()).unwrap();
    }
    fn submit(&self, request: u64, payload: &[u8]) -> Vec<u8> {
        encode_command(&Command::Submit { request, proposal: ActorProposal { target: self.config.profile.delivery.target,
            payload: payload.to_vec(), units: payload.len() as u64, deadline: ElapsedTick(100000), expected_policy_epoch: 0 } }).unwrap()
    }
    fn send(&mut self, bytes: &[u8]) { self.client.write_all(bytes).unwrap(); self.client.write_all(b"\n").unwrap(); }
    fn drive(&mut self) {
        self.inbox.drive(&mut self.driver, &mut self.config.source, || ElapsedTick(1000), DriveBudget::default()).unwrap();
    }
    fn reply(&mut self) -> WireResponse {
        let mut bytes = Vec::new();
        loop {
            let mut byte = [0]; self.client.read_exact(&mut byte).unwrap();
            if byte[0] == b'\n' { break; }
            assert!(bytes.len() < MAX_RESPONSE_BYTES); bytes.push(byte[0]);
        }
        decode_response(&bytes).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.root) { eprintln!("inbox cleanup: {error}"); } }
}

#[test]
fn admitted_work_is_fifo_journal_backed_and_not_an_execution() {
    let mut f = Fixture::new();
    for id in [90, 3] { f.send(&f.submit(id, b"work")); }
    f.drive();
    assert!(matches!(f.reply().result, Ok(Knowledge::Pending { request: 90 })));
    assert!(matches!(f.reply().result, Ok(Knowledge::Pending { request: 3 })));
    assert_eq!(f.inbox.queued(), 2);
    for id in [90, 3] {
        let next = f.inbox.next_request(&f.driver).unwrap().unwrap();
        assert_eq!(next.request, id);
        assert!(matches!(next.disposition, FileRequestDisposition::Admitted { stage: ActionState::Reviewing, .. }));
    }
    assert!(f.inbox.next_request(&f.driver).unwrap().is_none());
    let host = f.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.retained_requests(), 2);
}

#[test]
fn fragments_do_not_acquire_inputs_and_cancelled_hints_never_resurrect() {
    let mut f = Fixture::new(); let bytes = f.submit(1, b"work");
    f.client.write_all(&bytes[..bytes.len() / 2]).unwrap();
    let before = f.driver.supervisor().host().unwrap().revision();
    f.inbox.drive(&mut f.driver, &mut f.config.source, || panic!("partial frame read a clock"), DriveBudget::default()).unwrap();
    assert_eq!(f.inbox.queued(), 0); assert_eq!(f.driver.supervisor().host().unwrap().revision(), before);
    f.send(&bytes[bytes.len() / 2..]); f.drive(); f.reply();
    std::fs::remove_file(f.root.join("evidence.json")).unwrap();
    f.send(&encode_command(&Command::Cancel { request: 1 }).unwrap());
    f.inbox.drive(&mut f.driver, &mut f.config.source, || panic!("cancel acquired evidence"), DriveBudget::default()).unwrap();
    assert!(matches!(f.reply().result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
    assert!(f.inbox.next_request(&f.driver).unwrap().is_none());
    assert_eq!(f.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);
}

#[test]
fn missing_source_never_becomes_work_but_a_new_valid_capture_can_admit_it() {
    let mut f = Fixture::new(); std::fs::remove_file(f.root.join("evidence.json")).unwrap();
    let document = f.submit(1, b"work"); f.send(&document); f.drive();
    assert_eq!(f.reply().result, Err(WireError::Unavailable)); assert_eq!(f.inbox.queued(), 0);
    assert!(matches!(f.driver.supervisor().host().unwrap().request_status(1), Err(JournalError::Contract(Error::Missing))));
    f.evidence(); f.send(&document); f.drive();
    assert!(matches!(f.reply().result, Ok(Knowledge::Pending { .. })));
    assert_eq!(f.inbox.next_request(&f.driver).unwrap().unwrap().request, 1);
}

#[test]
fn reconnect_keeps_tickets_and_exact_or_conflicting_retry_never_duplicates_work() {
    let mut f = Fixture::new(); let document = f.submit(1, b"work");
    f.send(&document); f.drive(); f.reply();
    let before = f.driver.supervisor().host().unwrap().revision();
    std::fs::remove_file(f.root.join("evidence.json")).unwrap();
    assert!(f.inbox.disconnect());
    let (server, client) = UnixStream::pair().unwrap(); client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    f.inbox.attach(server).unwrap(); f.client = client;
    for bytes in [encode_command(&Command::Poll { request: 1 }).unwrap(), document, f.submit(1, b"other")] {
        f.send(&bytes);
        f.inbox.drive(&mut f.driver, &mut f.config.source, || panic!("retry acquired evidence"), DriveBudget::default()).unwrap();
        let result = f.reply().result;
        assert!(matches!(result, Ok(Knowledge::Pending { .. }) | Err(WireError::IdempotencyConflict)));
    }
    assert_eq!(f.driver.supervisor().host().unwrap().revision(), before);
    assert_eq!(f.inbox.queued(), 1); assert_eq!(f.inbox.status().connections_admitted, 2);
}

#[test]
fn wrong_owner_is_rejected_before_consuming_socket_input() {
    let mut f = Fixture::new(); let mut other = Fixture::new();
    f.send(&f.submit(1, b"work"));
    assert!(f.inbox.drive(&mut other.driver, &mut other.config.source,
        || panic!("foreign owner read time"), DriveBudget::default()).is_err());
    assert_eq!(f.inbox.queued(), 0); assert_eq!(other.driver.supervisor().host().unwrap().retained_requests(), 0);
    f.drive(); assert_eq!(f.inbox.next_request(&f.driver).unwrap().unwrap().request, 1);
}

#[test]
fn accepted_work_survives_unsent_reply_and_ingress_revocation() {
    let mut f = Fixture::new(); f.send(&f.submit(1, b"work"));
    let report = f.inbox.drive(&mut f.driver, &mut f.config.source, || ElapsedTick(1000),
        DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap();
    assert_eq!(report.drive.progress.written_bytes, 0); assert_eq!(f.inbox.queued(), 1);
    assert!(f.inbox.revoke()); assert!(f.inbox.status().revoked);
    let next = f.inbox.next_request(&f.driver).unwrap().unwrap();
    assert_eq!(next.request, 1);
    assert!(matches!(next.disposition, FileRequestDisposition::Admitted { stage: ActionState::Reviewing, .. }));
    assert_eq!(f.driver.supervisor().host().unwrap().retained_requests(), 1);
}

#[test]
fn exhausted_or_invalid_drive_budget_cannot_perform_intake() {
    let mut f = Fixture::new(); f.send(&f.submit(1, b"work"));
    let before = f.driver.supervisor().host().unwrap().revision();
    for budget in [DriveBudget { frames: 0, ..DriveBudget::default() }, DriveBudget { io_calls: 0, ..DriveBudget::default() }] {
        let report = f.inbox.drive(&mut f.driver, &mut f.config.source, || panic!("zero budget acquired evidence"), budget).unwrap();
        assert_eq!(report.drive.progress.frames, 0);
    }
    assert!(f.inbox.drive(&mut f.driver, &mut f.config.source, || panic!("invalid budget acquired evidence"),
        DriveBudget { frames: usize::MAX, ..DriveBudget::default() }).is_err());
    assert_eq!(f.driver.supervisor().host().unwrap().revision(), before); assert_eq!(f.inbox.queued(), 0);
    f.drive(); assert_eq!(f.inbox.queued(), 1);
}
