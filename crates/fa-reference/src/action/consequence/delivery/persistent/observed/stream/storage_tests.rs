//! Deterministic original Store barriers, not hardware power-loss evidence.
use super::*;
use crate::action::FrozenAction;
use crate::action::consequence::delivery::{EndpointOutcome, persistent::{FilePermit, JournalIo}};
use crate::action::consequence::delivery::persistent::observed::FileHumanPermit;
use crate::action::consequence::oversight::{CommitteeInput, ReviewWindow,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot}};
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-stream-barrier-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("stream test cleanup: {error}"); } }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
struct Keys { action: FrozenAction, inputs: CommitteeInput, automatic: FilePermit, human: FileHumanPermit }
fn prepare(host: &mut FileOversight, reviewer: &FileHumanReviewer, id: u64, message: &str) -> Keys {
    let spec = host.stream_message_spec(message, ElapsedTick(100)).unwrap();
    let action = host.propose(host.revision(), id, spec, snapshot()).unwrap();
    let capture = EvidenceSnapshot::new(EvidenceIdentity { scope: profile().delivery.scope, source: 7, generation: 1 },
        snapshot(), BTreeMap::from([("reviewer".to_owned(), b"context".to_vec())])).unwrap();
    let inputs = capture.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    let round = id + 100;
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human }
}
fn dispatch(host: &mut FileOversight, keys: &Keys) {
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
}
fn publish(host: &mut FileOversight, keys: &Keys) {
    assert!(matches!(host.publish_checked(host.revision(), keys.automatic.attempt(), Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome,
        EndpointOutcome::Executed { .. }));
}
fn setup(root: &Directory) -> (FileOversight, Keys, u64) {
    let (mut host, reviewer) = FileOversight::create_stream(root.store(), profile(), stream()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let first = prepare(&mut host, &reviewer, 1, "kept");
    dispatch(&mut host, &first); publish(&mut host, &first);
    host.reconcile(host.revision(), 1).unwrap();
    let prefix_charge = host.inspect().control.ledger.charged;
    let next = prepare(&mut host, &reviewer, 2, "candidate");
    dispatch(&mut host, &next);
    (host, next, prefix_charge)
}
fn failure(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected selected barrier failure"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn publication_fault_keeps_old_acknowledged_cut_and_recovers_only_actual_canonical_history() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, keys, prefix_charge) = setup(&root);
        let before = host.inspect(); let charged = before.control.ledger.charged;
        host.store.fail_once(stage);
        let error = host.publish_checked(host.revision(), 2, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap_err();
        failure(error, stage);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.stream_snapshot(), Err(JournalError::Unavailable));
        let disk = FileOversight::read_stream_publication(root.store(), &profile(), stream()).unwrap();
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(disk.published.message_count(), if visible { 2 } else { 1 });
        assert_eq!(disk.confirmed.message_count(), 1);
        assert_eq!(disk.pending, Some(2));
        drop(host);
        let (mut host, _) = FileOversight::open_stream(root.store(), profile(), stream()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, charged);
        if visible {
            let result = host.publish_checked(host.revision(), 2, None, Snapshot::default(), ElapsedTick(2)).unwrap();
            assert_eq!(result.basis, super::super::super::publication::PublicationBasis::PreviouslyResolved);
            host.reconcile(host.revision(), 2).unwrap();
            assert_eq!(host.inspect().control.ledger.charged, charged);
        } else {
            assert_eq!(host.publish_checked(host.revision(), 2, Some(&keys.inputs), snapshot(), ElapsedTick(2)),
                Err(JournalError::Contract(Error::Missing)));
            host.seal_unexecuted(host.revision(), 2).unwrap();
            assert_eq!(host.inspect().control.ledger.charged, prefix_charge);
        }
        let after = host.stream_snapshot().unwrap();
        assert_eq!(after.confirmed, after.published);
        assert_eq!(after.published.visible(), if visible { b"keptcandidate".as_slice() } else { b"kept".as_slice() });
        assert_eq!(after.pending, None);
    }
}

#[test]
fn reconciliation_fault_cannot_duplicate_or_erase_already_published_messages() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, keys, _) = setup(&root);
        publish(&mut host, &keys);
        let charged = host.inspect().control.ledger.charged;
        host.store.fail_once(stage);
        failure(host.reconcile(host.revision(), 2).unwrap_err(), stage);
        let disk = FileOversight::read_stream_publication(root.store(), &profile(), stream()).unwrap();
        assert_eq!(disk.published.messages().collect::<Vec<_>>(), vec!["kept", "candidate"]);
        assert_eq!(disk.confirmed.message_count(), if stage == JournalIo::DirectorySync { 2 } else { 1 });
        drop(host);
        let (mut host, _) = FileOversight::open_stream(root.store(), profile(), stream()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.reconcile(host.revision(), 2).unwrap();
        host.reconcile(host.revision(), 2).unwrap();
        let after = host.stream_snapshot().unwrap();
        assert_eq!(after.publication.executions, 2);
        assert_eq!(after.confirmed, after.published);
        assert_eq!(after.pending, None);
        assert_eq!(after.publication.control.ledger.charged, charged);
    }
}
