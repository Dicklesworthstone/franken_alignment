//! Real Store fault barriers around the original client; bounded scripted I/O.
use super::*;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::oversight::actor::{ActorBasis, ActorOutcome, BasisSource, Knowledge};
use crate::action::consequence::oversight::actor_wire::client::ClientIoLimits;
use std::cell::Cell;
use std::io::{self, Cursor};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-outbox-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("client") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("outbox cleanup: {error}"); } }
}
#[derive(Default)]
struct Counts { writes: Cell<u64>, reads: Cell<u64>, flushes: Cell<u64> }
struct Peer { counts: Rc<Counts>, input: Cursor<Vec<u8>>, short: bool, panic_write: bool }
impl Read for Peer {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.counts.reads.set(self.counts.reads.get() + 1); self.input.read(bytes)
    }
}
impl Write for Peer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.counts.writes.set(self.counts.writes.get() + 1);
        assert!(!self.panic_write, "entered write interruption");
        Ok(if self.short { bytes.len().min(5) } else { bytes.len() })
    }
    fn flush(&mut self) -> io::Result<()> { self.counts.flushes.set(self.counts.flushes.get() + 1); Ok(()) }
}
fn profile() -> FileClientProfile { FileClientProfile::new(b"exact-domain/raw-submit-v1".to_vec(), ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap() }
fn proposal() -> ActorProposal {
    ActorProposal { target: ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 },
        payload: b"exact request".to_vec(), expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }
}
fn response() -> WireResponse { WireResponse { request: Some(99), result: Ok(Knowledge::Known {
    value: ActorOutcome::Executed, basis: ActorBasis { request: 99, generation: 3, source: BasisSource::ControlLedger } }) } }
fn connect(root: &Directory, short: bool, panic_write: bool) -> (FileActorConnection<Peer>, Rc<Counts>) {
    let counts = Rc::new(Counts::default());
    let mut bytes = response().encode(); bytes.push(b'\n');
    let state = FileActorClient::create(root.store(), profile()).unwrap();
    (state.connect(Peer { counts: Rc::clone(&counts), input: Cursor::new(bytes), short, panic_write }), counts)
}
fn failure(result: Result<ClientProgress, FileClientError>, stage: JournalIo) {
    let Err(FileClientError::Storage(JournalError::Io(error))) = result else { panic!("missing original storage failure") };
    assert_eq!(error.operation, stage);
}

#[test]
fn intent_barriers_prevent_every_socket_call_and_preserve_exact_pending_requests() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut client, counts) = connect(&root, false, false);
        client.submit(0, 99, &proposal()).unwrap(); assert_eq!(client.revision(), 1);
        client.owner.store.fail_once(stage);
        failure(client.step(1), stage);
        assert!(client.unavailable()); assert!(client.state().is_err());
        assert_eq!((counts.writes.get(), counts.reads.get(), counts.flushes.get()), (0, 0, 0));
        let cut = FileActorClient::read(root.store(), &profile(), 1).unwrap();
        assert_eq!(cut.reserved_work().is_some(), stage == JournalIo::DirectorySync);
        assert_eq!(cut.original_proposal(99), Some(&proposal())); assert!(cut.last_response(99).is_none());
        assert!(client.step(client.revision()).is_err()); assert_eq!(counts.writes.get(), 0);
        drop(client);
        let reopened = FileActorClient::open(root.store(), profile(), 1).unwrap();
        assert_eq!(reopened.state().work(), cut.reserved_work().unwrap_or(cut.work()));
        assert_eq!(reopened.state().interrupted_command(), cut.interrupted_command());
    }
}

#[test]
fn completed_partial_write_barriers_spend_the_reserved_ceiling_only_when_completion_is_unknown() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut client, counts) = connect(&root, true, false);
        client.submit(0, 99, &proposal()).unwrap(); client.completion_failure = Some(stage);
        failure(client.step(1), stage);
        assert_eq!(counts.writes.get(), 1); assert_eq!(counts.flushes.get(), 0);
        let cut = FileActorClient::read(root.store(), &profile(), 2).unwrap();
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(cut.reserved_work().is_none(), visible);
        let charged = cut.reserved_work().unwrap_or(cut.work());
        if visible { assert_eq!(charged.written_bytes, 5); assert_eq!(charged.calls, 1); }
        else { assert!(charged.written_bytes > 5); assert_eq!(charged.calls, 2); }
        drop(client);
        let reopened = FileActorClient::open(root.store(), profile(), 2).unwrap();
        assert_eq!(reopened.state().work(), charged); assert!(reopened.state().last_response(99).is_none());
        assert_eq!(reopened.state().original_proposal(99), Some(&proposal()));
    }
}

#[test]
fn received_response_barriers_never_return_uncommitted_success_or_erase_the_intent() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut client, counts) = connect(&root, false, false);
        client.submit(0, 99, &proposal()).unwrap(); assert_eq!(client.step(1).unwrap(), ClientProgress::Progress);
        assert_eq!(client.revision(), 3); client.completion_failure = Some(stage);
        failure(client.step(3), stage); assert_eq!(counts.reads.get(), 1);
        assert!(client.state().is_err());
        let cut = FileActorClient::read(root.store(), &profile(), 4).unwrap();
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(cut.last_response(99), visible.then_some(&response()));
        assert_eq!(cut.reserved_work().is_none(), visible);
        drop(client);
        let reopened = FileActorClient::open(root.store(), profile(), 4).unwrap();
        assert_eq!(reopened.state().work(), cut.reserved_work().unwrap_or(cut.work()));
        let mut client = reopened.connect(Cursor::new(Vec::<u8>::new()));
        assert_eq!(client.poll(client.revision(), 99), Err(FileClientError::Client(ClientError::TicketUnavailable)));
    }
}

#[test]
fn caught_io_unwind_cannot_reenter_and_exact_lifetime_allowances_survive_restart() {
    let root = Directory::new();
    let p = FileClientProfile::new(b"domain".to_vec(), ClientSessionLimits { io: ClientIoLimits {
        exchanges: 2, calls: 2, written_bytes: 4096, read_bytes: 512 }, ..ClientSessionLimits::default() }, MAX_JOURNAL_BYTES).unwrap();
    let counts = Rc::new(Counts::default());
    let mut client = FileActorClient::create(root.store(), p.clone()).unwrap().connect(Peer {
        counts: Rc::clone(&counts), input: Cursor::new(Vec::new()), short: false, panic_write: true });
    client.submit(0, 99, &proposal()).unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| client.step(1))).is_err());
    assert!(client.unavailable()); assert_eq!(counts.writes.get(), 1);
    assert!(client.step(client.revision()).is_err()); assert_eq!(counts.writes.get(), 1);
    drop(client);
    let reopened = FileActorClient::open(root.store(), p, 2).unwrap();
    assert_eq!(reopened.state().work().calls, 2);
    let mut client = reopened.connect(Cursor::new(Vec::<u8>::new()));
    assert_eq!(client.retry_submission(client.revision(), 99), Err(FileClientError::Client(ClientError::Limit)));
}

#[test]
fn exact_contract_revision_path_and_corrupt_suffix_checks_precede_staging_cleanup() {
    let root = Directory::new(); let (mut client, _) = connect(&root, false, false);
    client.submit(0, 99, &proposal()).unwrap(); drop(client);
    let original = std::fs::read(root.store().join(storage::CANONICAL)).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"untrusted staging").unwrap();
    let mut wrong = profile(); wrong.binding.push(b'x');
    assert!(FileActorClient::open(root.store(), wrong, 0).is_err());
    let mut wrong = profile(); wrong.limits.io.calls -= 1;
    assert!(FileActorClient::open(root.store(), wrong, 0).is_err());
    assert!(FileActorClient::open(root.store(), profile(), 2).is_err());
    assert_eq!(std::fs::read(&pending).unwrap(), b"untrusted staging");
    let mut bad = original.clone(); bad.push(0); std::fs::write(root.store().join(storage::CANONICAL), &bad).unwrap();
    assert!(FileActorClient::open(root.store(), profile(), 0).is_err());
    assert_eq!(std::fs::read(&pending).unwrap(), b"untrusted staging");
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    let other = Directory::new(); let owner = FileActorClient::create(other.store(), profile()).unwrap(); drop(owner);
    std::fs::write(other.store().join(storage::CANONICAL), &original).unwrap();
    assert!(FileActorClient::open(other.store(), profile(), 0).is_err());
    let reopened = FileActorClient::open(root.store(), profile(), 1).unwrap();
    assert_eq!(reopened.state().original_proposal(99), Some(&proposal())); assert!(!pending.exists());
}

#[test]
fn capacity_refusal_never_sends_and_normal_responses_are_saved_before_return() {
    let root = Directory::new(); let state = FileActorClient::create(root.0.join("large1"), profile()).unwrap();
    let capacity = std::fs::metadata(root.0.join("large1").join(storage::CANONICAL)).unwrap().len() as usize;
    drop(state);
    let p = FileClientProfile::new(profile().binding, profile().limits, capacity).unwrap();
    let counts = Rc::new(Counts::default());
    let mut limited = FileActorClient::create(root.0.join("small1"), p).unwrap().connect(Peer {
        counts: Rc::clone(&counts), input: Cursor::new(Vec::new()), short: false, panic_write: false });
    assert!(limited.submit(0, 99, &proposal()).is_err()); assert!(limited.unavailable());
    assert_eq!(counts.writes.get(), 0);
    let root = Directory::new(); let (mut client, _) = connect(&root, false, false);
    client.submit(0, 99, &proposal()).unwrap(); client.step(1).unwrap();
    assert_eq!(client.step(3).unwrap(), ClientProgress::Response(response()));
    let cut = FileActorClient::read(root.store(), &profile(), 5).unwrap();
    assert_eq!(cut.last_response(99), Some(&response())); assert!(cut.reserved_work().is_none());
    assert_eq!(cut.work(), client.state().unwrap().work());
    let revision = client.revision(); assert_eq!(client.step(revision).unwrap(), ClientProgress::Complete);
    assert_eq!(client.revision(), revision);
    let state = client.into_disconnected().unwrap();
    assert!(FileActorClient::open(root.store(), profile(), 0).is_err(), "exclusive client lock lost");
    drop(state);
    assert!(FileActorClient::open(root.store(), profile(), 5).is_ok());
}
