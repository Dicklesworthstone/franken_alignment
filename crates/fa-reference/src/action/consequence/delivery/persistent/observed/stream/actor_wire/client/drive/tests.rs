//! Original Store fault barriers and native-step controls, not a mock journal.
use super::*;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::oversight::actor::Knowledge;
use crate::action::consequence::oversight::actor_wire::client::drive::MAX_CLIENT_DRIVE_STEPS;
use std::{cell::RefCell, fs, io::{self, Cursor}, path::PathBuf, rc::Rc,
    sync::atomic::{AtomicU64, Ordering}};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-client-batch-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("client") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(e) = fs::remove_dir_all(&self.0) { eprintln!("client batch cleanup: {e}"); } }
}
#[derive(Default, Debug, PartialEq, Eq)]
struct Trace { calls: usize, written: Vec<u8>, read: usize }
struct Peer { input: Cursor<Vec<u8>>, trace: Rc<RefCell<Trace>>, panic_at: Option<usize>, chunk: usize }
impl Peer {
    fn new(chunk: usize) -> Self {
        let mut bytes = response().encode(); bytes.push(b'\n');
        Self { input: Cursor::new(bytes), trace: Rc::new(RefCell::new(Trace::default())), panic_at: None, chunk }
    }
    fn enter(&self) {
        let n = { let mut t = self.trace.borrow_mut(); t.calls += 1; t.calls };
        assert_ne!(self.panic_at, Some(n), "deliberate client I/O interruption");
    }
}
impl Write for Peer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.enter(); let n = bytes.len().min(self.chunk); self.trace.borrow_mut().written.extend_from_slice(&bytes[..n]); Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> { self.enter(); Ok(()) }
}
impl Read for Peer {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.enter(); let n = bytes.len().min(self.chunk);
        let n = self.input.read(&mut bytes[..n])?; self.trace.borrow_mut().read += n; Ok(n)
    }
}
fn response() -> WireResponse { WireResponse { request: Some(7), result: Ok(Knowledge::Pending { request: 7 }) } }
fn proposal() -> ActorProposal {
    ActorProposal { target: ResolvedTarget { adapter: 1, object: 2, contract_version: 1,
        expected_version: 3, generation: 4 }, payload: vec![b'x'; 9000],
        expected_policy_epoch: 0, deadline: ElapsedTick(50), units: 16 }
}
fn profile() -> FileClientProfile {
    FileClientProfile::new(b"independent-server/raw-v1".to_vec(), ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap()
}
fn prepared(root: &Directory, peer: Peer, profile: FileClientProfile) -> FileActorConnection<Peer> {
    let mut c = FileActorClient::create(root.store(), profile).unwrap().connect(peer);
    c.submit(0, 7, &proposal()).unwrap(); c
}
fn failure(error: FileClientError, stage: JournalIo) {
    let FileClientError::Storage(JournalError::Io(failure)) = error else { panic!("missing injected failure"); };
    assert_eq!(failure.operation, stage);
}

#[test]
fn a_complete_large_exchange_uses_one_pair_and_matches_native_steps() {
    for budget in [ClientDriveBudget::new(1).unwrap(), ClientDriveBudget::default(), ClientDriveBudget::new(64).unwrap()] {
        let root = Directory::new(); let peer = Peer::new(4096); let trace = Rc::clone(&peer.trace);
        let mut durable = prepared(&root, peer, profile());
        let peer = Peer::new(4096); let oracle_trace = Rc::clone(&peer.trace);
        let mut original = ActorClientState::new(ClientSessionLimits::default()).unwrap().connect(peer);
        original.submit(7, &proposal()).unwrap();
        let mut original_steps = 0;
        loop { original_steps += 1; if matches!(original.step().unwrap(), ClientProgress::Response(_)) { break; } }
        let mut batches = 0; let mut observed_steps = 0;
        loop {
            let revision = durable.revision(); let report = durable.drive(revision, budget).unwrap(); batches += 1;
            assert_eq!(durable.revision(), revision + 2); observed_steps += report.steps;
            if report.progress == ClientProgress::Response(response()) { break; }
        }
        assert_eq!(observed_steps, original_steps);
        if budget.steps() > 1 { assert_eq!(batches, 1); assert!(original_steps > 2); }
        assert_eq!(durable.revision(), 1 + 2 * batches);
        assert_eq!(durable.state().unwrap().work(), original.state().work());
        assert_eq!(*trace.borrow(), *oracle_trace.borrow());
        let disk = FileActorClient::read(root.store(), &profile(), durable.revision()).unwrap();
        assert!(disk.reserved_work().is_none()); assert!(disk.reserved_drive().is_none());
        assert_eq!(disk.last_response(7), Some(&response()));
        let revision = durable.revision(); let calls = trace.borrow().calls;
        assert_eq!(durable.drive(revision, budget).unwrap().steps, 0);
        assert_eq!(durable.revision(), revision); assert_eq!(trace.borrow().calls, calls);
    }
}

#[test]
fn all_intent_and_partial_or_response_completion_barriers_preserve_exact_uncertainty() {
    for phase in 0..3 {
        for stage in BARRIERS {
            let root = Directory::new(); let peer = Peer::new(4096); let trace = Rc::clone(&peer.trace);
            let mut c = prepared(&root, peer, profile());
            let budget = ClientDriveBudget::new(if phase == 1 { 2 } else { 16 }).unwrap();
            let before = c.state().unwrap().work();
            let ceiling = c.client.as_ref().unwrap().drive_ceiling(budget).unwrap().unwrap();
            if phase == 0 { c.owner.store.fail_once(stage); } else { c.completion_failure = Some(stage); }
            failure(c.drive(1, budget).unwrap_err(), stage);
            assert!(c.unavailable()); assert!(c.state().is_err());
            let calls = trace.borrow().calls;
            assert_eq!(calls == 0, phase == 0);
            assert!(c.drive(c.revision(), budget).is_err()); assert_eq!(trace.borrow().calls, calls);
            let disk = FileActorClient::read(root.store(), &profile(), 1).unwrap();
            let complete = phase != 0 && stage == JournalIo::DirectorySync;
            let has_intent = !complete && (phase != 0 || stage == JournalIo::DirectorySync);
            assert_eq!(disk.reserved_drive(), has_intent.then_some(budget));
            assert_eq!(disk.reserved_work(), has_intent.then_some(ceiling));
            assert_eq!(disk.last_response(7).is_some(), complete && phase == 2);
            assert_eq!(disk.original_proposal(7), Some(&proposal()));
            let retained = if has_intent { ceiling } else if complete { disk.work() } else { before };
            let revision = disk.revision(); drop(c);
            let recovered = FileActorClient::open(root.store(), profile(), revision).unwrap();
            assert_eq!(recovered.state().work(), retained);
            assert_eq!(recovered.state().original_proposal(7), Some(&proposal()));
            drop(recovered);
            let recovered = FileActorClient::open(root.store(), profile(), revision).unwrap();
            assert_eq!(recovered.state().work(), retained, "reopening cannot debit twice or reset work");
            let mut again = recovered.connect(Peer::new(4096));
            assert_eq!(again.poll(revision, 7), Err(FileClientError::Client(ClientError::TicketUnavailable)));
        }
    }
}

#[test]
fn interrupted_batch_consumes_the_reservation_not_a_fabricated_response() {
    for call in [1, 3, 7] {
        let root = Directory::new(); let mut peer = Peer::new(4096); peer.panic_at = Some(call);
        let trace = Rc::clone(&peer.trace); let mut c = prepared(&root, peer, profile());
        let budget = ClientDriveBudget::default();
        let ceiling = c.client.as_ref().unwrap().drive_ceiling(budget).unwrap().unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.drive(1, budget))).is_err());
        assert!(c.unavailable()); let revision = c.revision();
        assert!(c.drive(revision, budget).is_err()); assert_eq!(trace.borrow().calls, call);
        let disk = FileActorClient::read(root.store(), &profile(), revision).unwrap();
        assert!(disk.last_response(7).is_none()); assert_eq!(disk.reserved_work(), Some(ceiling));
        drop(c); let recovered = FileActorClient::open(root.store(), profile(), revision).unwrap();
        assert_eq!(recovered.state().work(), ceiling); assert!(recovered.state().interrupted_command().is_some());
    }
}

#[test]
fn batch_intent_capacity_failure_happens_before_any_socket_operation() {
    let root = Directory::new(); let mut c = prepared(&root, Peer::new(4096), profile());
    let archive = c.client.as_ref().unwrap().recovery_image(c.owner.profile.binding()).unwrap();
    let initial = image::encode(c.owner.store.identity(), &profile(), 1, &archive, None).unwrap();
    // The capacity is an independently provisioned bootstrap, not a live grant.
    // Recreate in a separate root with equal path-byte length and this exact cap.
    drop(c);
    let second = Directory::new(); let mut p = profile();
    let first_path_len = root.store().as_os_str().as_encoded_bytes().len();
    let second_path_len = second.store().as_os_str().as_encoded_bytes().len();
    p.image_bytes = initial.len() + second_path_len - first_path_len;
    let peer = Peer::new(4096); let trace = Rc::clone(&peer.trace);
    c = prepared(&second, peer, p.clone());
    assert_eq!(c.drive(0, ClientDriveBudget::default()), Err(Error::Stale.into()));
    assert!(!c.unavailable()); assert_eq!(trace.borrow().calls, 0);
    assert!(c.drive(1, ClientDriveBudget::default()).is_err()); assert!(c.unavailable());
    assert_eq!(trace.borrow().calls, 0);
    let saved = FileActorClient::read(second.store(), &p, 1).unwrap();
    assert!(saved.reserved_work().is_none()); assert!(saved.last_response(7).is_none());
}

#[test]
fn version_two_and_reservation_bounds_reject_truncation_downgrade_and_budget_refill() {
    let root = Directory::new(); let c = prepared(&root, Peer::new(4096), profile());
    let budget = ClientDriveBudget::default();
    let archive = c.client.as_ref().unwrap().recovery_image(c.owner.profile.binding()).unwrap();
    let ceiling = c.client.as_ref().unwrap().drive_ceiling(budget).unwrap().unwrap();
    let bytes = image::encode_drive(c.owner.store.identity(), &profile(), 2, &archive, ceiling, budget).unwrap();
    assert_eq!(&bytes[..8], b"FACDISK\x02");
    // Independent wrapper oracle: the original image fields remain identical;
    // only the version byte and explicit terminal quantum word are new.
    let mut vector = image::encode(c.owner.store.identity(), &profile(), 2, &archive, Some(ceiling)).unwrap();
    vector[7] = 2; vector.extend_from_slice(&budget.steps().to_be_bytes());
    assert_eq!(bytes, vector);
    for end in 0..bytes.len() { assert!(image::decode(c.owner.store.identity(), &profile(), 0, &bytes[..end]).is_err()); }
    let mut downgraded = bytes.clone(); downgraded[7] = 1;
    assert!(image::decode(c.owner.store.identity(), &profile(), 0, &downgraded).is_err());
    for steps in [0, 1, MAX_CLIENT_DRIVE_STEPS + 1, u64::MAX] {
        let mut bad = bytes.clone(); let at = bad.len() - 8; bad[at..].copy_from_slice(&steps.to_be_bytes());
        assert!(image::decode(c.owner.store.identity(), &profile(), 0, &bad).is_err());
    }
    for field in 0..4 {
        let mut bad = ceiling;
        match field { 0 => bad.exchanges += 1, 1 => bad.calls += 1,
            2 => bad.written_bytes += 1, _ => bad.read_bytes += 1 }
        let image = image::encode_drive(c.owner.store.identity(), &profile(), 2, &archive, bad, budget).unwrap();
        assert!(image::decode(c.owner.store.identity(), &profile(), 0, &image).is_err());
    }
    let cut = image::decode(c.owner.store.identity(), &profile(), 0, &bytes).unwrap();
    assert_eq!(cut.reserved_drive(), Some(budget)); assert_eq!(cut.reserved_work(), Some(ceiling));
    let one = c.client.as_ref().unwrap().recovery_step_ceiling().unwrap().unwrap();
    let legacy = image::encode(c.owner.store.identity(), &profile(), 2, &archive, Some(one)).unwrap();
    let old = image::decode(c.owner.store.identity(), &profile(), 0, &legacy).unwrap();
    assert_eq!(old.reserved_work(), Some(one)); assert_eq!(old.reserved_drive(), None);
}

#[test]
fn lost_batch_budget_cannot_be_refreshed_by_reconnect_or_new_quantum() {
    let root = Directory::new(); let mut limits = ClientSessionLimits::default(); limits.io.calls = 4;
    let p = FileClientProfile::new(b"bounded-server".to_vec(), limits, MAX_JOURNAL_BYTES).unwrap();
    let mut c = prepared(&root, Peer::new(1), p.clone()); c.completion_failure = Some(JournalIo::Stage);
    assert!(c.drive(1, ClientDriveBudget::default()).is_err()); drop(c);
    let recovered = FileActorClient::open(root.store(), p, 2).unwrap();
    assert_eq!(recovered.state().work().calls, 4);
    let peer = Peer::new(4096); let trace = Rc::clone(&peer.trace); let mut c = recovered.connect(peer);
    assert_eq!(c.retry_submission(2, 7), Err(FileClientError::Client(ClientError::Limit)));
    assert_eq!(c.revision(), 2); assert!(!c.unavailable()); assert_eq!(trace.borrow().calls, 0);
}
