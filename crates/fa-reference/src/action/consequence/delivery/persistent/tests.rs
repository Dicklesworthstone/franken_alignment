//! Fault barriers exercise the real original reducers and real file replacement.
//! Injected failures are not power cuts or an execution qualification.
use super::*;
use crate::action::{Purpose, VERSION};
use crate::action::consequence::congress::MemberPolicy;
use crate::action::consequence::delivery::NonExecutionReason;
use crate::action::consequence::gate::containment::{RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::Predicate;
use crate::reducer::Caps;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-journal-barrier-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("journal test cleanup: {error}"); }
    }
}
fn profile() -> FileDeliveryProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
            model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
            grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
        congress: CongressPolicy {
            generation: 1,
            members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
        retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
fn create(root: &Directory) -> FileDelivery {
    let mut host = FileDelivery::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host
}
fn dispatch(host: &mut FileDelivery, id: u64, payload: &[u8]) {
    let spec = ActionSpec { version: VERSION, scope: profile().scope,
        target: Some(host.inspect().target), payload: payload.to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 };
    let action = host.propose(host.revision(), id, spec, snapshot()).unwrap();
    host.review(host.revision(), ReferenceReview { attempt: id, round: id + 100, evidence_root: [9; 32],
        snapshot: snapshot(), ballots: BTreeMap::from([("reviewer".to_owned(),
            ReferenceBallot { verdict: Verdict::Allow, salt: b"reference-salt".to_vec() })]) }).unwrap();
    let permit = host.authorize(host.revision(), id, snapshot()).unwrap();
    host.dispatch(host.revision(), &permit, &action, snapshot()).unwrap();
}
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];

#[test]
fn publication_faults_never_return_a_receipt_or_hide_a_successful_rename() {
    for stage in BARRIERS {
        let root = Directory::new();
        let mut host = create(&root);
        dispatch(&mut host, 1, b"visible");
        let before = host.inspect();
        host.store.fail_once(stage);
        let error = host.publish(host.revision(), 1).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected an I/O failure"); };
        assert_eq!(failure.operation, stage);
        assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
        assert_eq!(host.inspect(), before);
        assert!(!host.clock_ready());
        assert_eq!(host.reconcile_pending(host.revision()).unwrap_err(), JournalError::Unavailable);
        let visible = FileDelivery::read_publication(root.store(), &profile()).unwrap();
        let replaced = stage == JournalIo::DirectorySync;
        assert_eq!(visible.executions, u64::from(replaced));
        assert_eq!(visible.control.ledger.charged, 16);
        assert_eq!(visible.payload.as_slice(), if replaced { b"visible".as_slice() } else { b"initial".as_slice() });
        drop(host);
        let mut recovered = FileDelivery::open(root.store(), profile()).unwrap();
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        let outcomes = recovered.reconcile_pending(recovered.revision()).unwrap();
        if replaced {
            assert_eq!(outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
            assert_eq!(recovered.inspect().control.ledger.charged, 16);
        } else {
            assert_eq!(outcomes[&1], Ok(Reconciliation::AwaitingResolution));
            assert_eq!(recovered.inspect().control.ledger.charged, 16);
            assert_eq!(recovered.seal_unexecuted(recovered.revision(), 1).unwrap(),
                Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
            assert_eq!(recovered.inspect().control.ledger.available, 100);
        }
        assert_eq!(recovered.inspect().executions, u64::from(replaced));
    }
}

#[test]
fn a_failed_batch_exposes_no_candidate_refund_and_reopening_replays_the_canonical_cut() {
    for stage in BARRIERS {
        let root = Directory::new();
        let mut host = create(&root);
        dispatch(&mut host, 1, b"executed");
        host.publish(host.revision(), 1).unwrap();
        dispatch(&mut host, 2, b"unsent");
        host.observe_time(host.revision(), ElapsedTick(100)).unwrap();
        let before = host.inspect();
        assert_eq!(before.control.ledger.charged, 32);
        host.store.fail_once(stage);
        assert!(matches!(host.reconcile_pending(host.revision()), Err(JournalError::Io(_))));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.cancel(host.revision(), 2).unwrap_err(), JournalError::Unavailable);
        let disk = FileDelivery::read_publication(root.store(), &profile()).unwrap();
        let replaced = stage == JournalIo::DirectorySync;
        assert_eq!(disk.control.ledger.charged, if replaced { 16 } else { 32 });
        assert_eq!(disk.executions, 1);
        assert_eq!(disk.payload, b"executed");
        drop(host);
        let mut recovered = FileDelivery::open(root.store(), profile()).unwrap();
        recovered.observe_time(recovered.revision(), ElapsedTick(101)).unwrap();
        let outcomes = recovered.reconcile_pending(recovered.revision()).unwrap();
        if replaced {
            assert!(outcomes.is_empty());
        } else {
            assert_eq!(outcomes.len(), 2);
            assert_eq!(outcomes[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
            assert_eq!(outcomes[&2], Ok(Reconciliation::Resolved(EndpointOutcome::NotExecuted {
                reason: NonExecutionReason::DeadlineElapsed })));
        }
        let settled = recovered.inspect();
        assert_eq!(settled.control.ledger.charged, 16);
        assert_eq!(settled.control.ledger.available, 84);
        assert_eq!(settled.control.ledger.reserved, 0);
        assert_eq!(settled.control.ledger.stages[&1], ActionState::Confirmed);
        assert_eq!(settled.control.ledger.stages[&2], ActionState::ConfirmedNotExecuted);
        assert_eq!(settled.executions, 1);
    }
}

#[test]
fn framed_but_illegal_history_cannot_fabricate_a_publication_or_a_sweep_result() {
    let p = profile();
    let path = Path::new("/operator-owned/reference-publication");
    for events in [vec![Event::Time(ElapsedTick(1)), Event::Publish(1)],
        vec![Event::Sweep], vec![Event::Time(ElapsedTick(2)), Event::Time(ElapsedTick(1))]] {
        let bytes = codec::encode(&p, path, &events).unwrap();
        let parsed = codec::decode(&p, path, &bytes).unwrap();
        assert!(Machine::replay(&p, &parsed).is_err());
    }
    let events = vec![Event::Time(ElapsedTick(1)), Event::Sweep];
    let bytes = codec::encode(&p, path, &events).unwrap();
    for cut in 0..bytes.len() { assert!(codec::decode(&p, path, &bytes[..cut]).is_err()); }
    let parsed = codec::decode(&p, path, &bytes).unwrap();
    let replayed = Machine::replay(&p, &parsed).unwrap();
    assert_eq!(replayed.snapshot(parsed.len()).control.ledger.available, 100);
    assert_eq!(replayed.snapshot(parsed.len()).executions, 0);
}
