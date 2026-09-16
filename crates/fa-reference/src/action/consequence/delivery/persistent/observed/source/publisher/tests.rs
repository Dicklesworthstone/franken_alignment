use super::*;
use super::super::{FileSourceError, FileSourcePolicy};
use super::super::super::{FileOversight, FileOversightProfile};
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, ReviewWindow};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-evidence-publisher-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn source(&self) -> PathBuf { self.0.join("source") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("publisher cleanup: {error}"); }
    }
}
fn profile() -> EvidencePublisherProfile {
    EvidencePublisherProfile { source: 9,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        minimum_generation: 1, max_bytes: MAX_EVIDENCE_FILE_BYTES }
}
fn snapshot(generation: u64, context: &[u8]) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 9, generation, scope: profile().scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        BTreeMap::from([("reviewer".to_owned(), context.to_vec())])).unwrap()
}

#[test]
fn original_reader_observes_atomic_versions_and_retry_keeps_the_same_generation() {
    let root = Directory::new();
    let first = snapshot(1, b"first");
    let second = snapshot(2, b"second");
    let (mut owner, created) = FileEvidencePublisher::create(root.source(), profile(), first.clone()).unwrap();
    assert_eq!(created.kind, EvidencePublicationKind::Created);
    assert_eq!(created.encoded_bytes, first.encode().len());
    let mut reader = owner.reader().unwrap();
    assert_eq!(reader.read().unwrap().as_ref(), &first);
    assert!(matches!(FileEvidencePublisher::open(root.source(), profile()), Err(JournalError::Busy)));
    assert_eq!(owner.publish(1, second.clone()).unwrap().kind, EvidencePublicationKind::Replaced);
    assert_eq!(reader.read().unwrap().as_ref(), &second);
    let bytes = std::fs::read(root.source().join(storage::CANONICAL)).unwrap();
    assert_eq!(owner.publish(1, second.clone()).unwrap().kind, EvidencePublicationKind::AlreadyCurrent);
    assert_eq!(std::fs::read(root.source().join(storage::CANONICAL)).unwrap(), bytes);
    drop(owner);
    let mut reopened = FileEvidencePublisher::open(root.source(), profile()).unwrap();
    assert_eq!(reopened.snapshot(), &second);
    assert_eq!(reopened.publish(1, second).unwrap().kind, EvidencePublicationKind::AlreadyCurrent);
}

#[test]
fn changed_same_generation_wrong_predecessor_source_scope_and_semantics_refuse() {
    let root = Directory::new();
    let (mut owner, _) = FileEvidencePublisher::create(root.source(), profile(), snapshot(1, b"first")).unwrap();
    let original = std::fs::read(root.source().join(storage::CANONICAL)).unwrap();
    assert_eq!(owner.publish(0, snapshot(2, b"second")), Err(Error::Stale.into()));
    assert_eq!(owner.publish(2, snapshot(3, b"third")), Err(Error::Stale.into()));
    assert_eq!(owner.publish(0, snapshot(1, b"substitution")), Err(Error::Binding.into()));
    let mut alien = snapshot(2, b"second").identity();
    alien.source += 1;
    let foreign = EvidenceSnapshot::new(alien, snapshot(2, b"second").snapshot().clone(),
        snapshot(2, b"second").contexts().clone()).unwrap();
    assert_eq!(owner.publish(1, foreign), Err(Error::Binding.into()));
    alien = snapshot(2, b"second").identity();
    alien.scope.tenant += 1;
    let foreign = EvidenceSnapshot::new(alien, snapshot(2, b"second").snapshot().clone(),
        snapshot(2, b"second").contexts().clone()).unwrap();
    assert_eq!(owner.publish(1, foreign), Err(Error::Binding.into()));
    let next = snapshot(2, b"second");
    let mut rolled = next.snapshot().clone();
    rolled.semantic_epoch = 0;
    let rolled = EvidenceSnapshot::new(next.identity(), rolled, next.contexts().clone()).unwrap();
    assert_eq!(owner.publish(1, rolled), Err(Error::Stale.into()));
    assert_eq!(std::fs::read(root.source().join(storage::CANONICAL)).unwrap(), original);
    assert!(owner.failure().is_none());
    owner.publish(1, next).unwrap();
}

#[test]
fn exact_byte_limit_succeeds_and_one_over_does_not_create_or_poison_a_store() {
    let root = Directory::new();
    let first = snapshot(1, b"first");
    let bytes = first.encode().len();
    let small = EvidencePublisherProfile { max_bytes: bytes - 1, ..profile() };
    assert!(matches!(FileEvidencePublisher::create(root.source(), small, first.clone()),
        Err(JournalError::Contract(Error::Limit))));
    assert!(!root.source().exists());
    let exact = EvidencePublisherProfile { max_bytes: bytes, ..profile() };
    let (mut owner, _) = FileEvidencePublisher::create(root.source(), exact, first).unwrap();
    assert_eq!(owner.publish(1, snapshot(2, b"first!")), Err(Error::Limit.into()));
    assert!(owner.failure().is_none());
    assert_eq!(owner.publish(1, snapshot(2, b"first")).unwrap().encoded_bytes, bytes);
}

#[test]
fn observed_external_replacement_poisoned_owner_cannot_overwrite_or_acknowledge_it() {
    let root = Directory::new();
    let (mut owner, _) = FileEvidencePublisher::create(root.source(), profile(), snapshot(1, b"first")).unwrap();
    let replaced = snapshot(1, b"out-of-band").encode();
    std::fs::write(root.source().join(storage::CANONICAL), &replaced).unwrap();
    assert_eq!(owner.publish(0, snapshot(1, b"first")), Err(Error::Binding.into()));
    assert_eq!(owner.publish(1, snapshot(2, b"second")), Err(JournalError::Unavailable));
    assert!(matches!(owner.reader(), Err(JournalError::Unavailable)));
    assert_eq!(std::fs::read(root.source().join(storage::CANONICAL)).unwrap(), replaced);
}

#[test]
fn all_replace_barriers_preserve_old_or_new_complete_image_and_reopen_reconciles_retry() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new();
        let first = snapshot(1, b"first");
        let second = snapshot(2, b"second");
        let (mut owner, _) = FileEvidencePublisher::create(root.source(), profile(), first.clone()).unwrap();
        let mut reader = owner.reader().unwrap();
        assert_eq!(reader.read().unwrap().as_ref(), &first);
        owner.store.fail_once(barrier);
        let Err(JournalError::Io(failure)) = owner.publish(1, second.clone()) else { panic!("expected injected barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        assert_eq!(owner.snapshot(), &first);
        assert!(owner.failure().is_some());
        assert_eq!(owner.publish(1, second.clone()), Err(JournalError::Unavailable));
        let visible = if barrier == JournalIo::DirectorySync { &second } else { &first };
        assert_eq!(reader.read().unwrap().as_ref(), visible);
        drop(owner);
        let mut reopened = FileEvidencePublisher::open(root.source(), profile()).unwrap();
        assert_eq!(reopened.snapshot(), visible);
        assert!(!root.source().join("delivery.pending").exists());
        let result = reopened.publish(1, second.clone()).unwrap();
        assert_eq!(result.kind, if barrier == JournalIo::DirectorySync {
            EvidencePublicationKind::AlreadyCurrent
        } else { EvidencePublicationKind::Replaced });
        assert_eq!(reader.read().unwrap().as_ref(), &second);
    }
}

#[test]
fn recovery_validates_canonical_scope_and_floor_before_discarding_staged_bytes() {
    let root = Directory::new();
    let (owner, _) = FileEvidencePublisher::create(root.source(), profile(), snapshot(1, b"first")).unwrap();
    drop(owner);
    let pending = root.source().join("delivery.pending");
    std::fs::write(&pending, snapshot(2, b"not published").encode()).unwrap();
    let floor = EvidencePublisherProfile { minimum_generation: 2, ..profile() };
    assert!(matches!(FileEvidencePublisher::open(root.source(), floor), Err(JournalError::Contract(Error::Stale))));
    assert!(pending.exists());
    let foreign = EvidencePublisherProfile { source: 10, ..profile() };
    assert!(matches!(FileEvidencePublisher::open(root.source(), foreign), Err(JournalError::Contract(Error::Binding))));
    assert!(pending.exists());
    let owner = FileEvidencePublisher::open(root.source(), profile()).unwrap();
    assert_eq!(owner.snapshot().identity().generation, 1);
    assert!(!pending.exists());
}

#[test]
fn invalid_canonical_image_never_promotes_a_valid_pending_image() {
    let root = Directory::new();
    let (owner, _) = FileEvidencePublisher::create(root.source(), profile(), snapshot(1, b"first")).unwrap();
    drop(owner);
    let pending = root.source().join("delivery.pending");
    let pending_bytes = snapshot(2, b"not published").encode();
    std::fs::write(&pending, &pending_bytes).unwrap();
    std::fs::write(root.source().join(storage::CANONICAL), b"{truncated").unwrap();
    assert!(matches!(FileEvidencePublisher::open(root.source(), profile()), Err(JournalError::Contract(_))));
    assert_eq!(std::fs::read(pending).unwrap(), pending_bytes);
}

#[test]
fn maximum_generation_can_be_retried_but_never_wrapped() {
    let root = Directory::new();
    let last = snapshot(u64::MAX, b"last");
    let (mut owner, _) = FileEvidencePublisher::create(root.source(), profile(), last.clone()).unwrap();
    assert_eq!(owner.publish(u64::MAX - 1, last).unwrap().kind, EvidencePublicationKind::AlreadyCurrent);
    assert_eq!(owner.publish(u64::MAX, snapshot(1, b"wrapped")), Err(Error::Overflow.into()));
    assert!(owner.failure().is_none());
}

fn gate_profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile { scope: profile().scope, total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default() },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn source_policy() -> FileSourcePolicy {
    FileSourcePolicy { source: StateSource { scope: profile().scope, source: 9, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(20).unwrap() }
}

#[test]
fn published_source_change_after_two_key_dispatch_seals_instead_of_rebasing_review() {
    for changed in [false, true] {
        let root = Directory::new();
        let first = snapshot(1, b"reviewed context");
        let (mut producer, _) = FileEvidencePublisher::create(root.source(), profile(), first.clone()).unwrap();
        let mut reader = producer.reader().unwrap();
        let p = gate_profile();
        let (mut host, reviewer) = FileOversight::create(root.0.join("effect"), p.clone()).unwrap();
        host.enable_file_source(host.revision(), source_policy()).unwrap();
        host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
        let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION,
            scope: p.delivery.scope, target: Some(p.delivery.target), payload: b"visible".to_vec(),
            required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100), units: 16,
        }, first.snapshot().clone()).unwrap();
        let inputs = first.inputs_for(&action, &p.committee).unwrap();
        host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
        host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, first.snapshot().clone()).unwrap();
        let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
        host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
        host.open_reveals(host.revision(), 101).unwrap();
        host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
        host.finish_review(host.revision(), 101, Some(&inputs), first.snapshot().clone()).unwrap().unwrap();
        let automatic = host.authorize(host.revision(), 1, &inputs, first.snapshot().clone()).unwrap();
        let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(10)).unwrap();
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, first.snapshot().clone()).unwrap();
        if changed { producer.publish(1, snapshot(2, b"different context")).unwrap(); }
        host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)).unwrap();
        let publication = host.publish_checked(host.revision(), 1, Some(&inputs), first.snapshot().clone(), ElapsedTick(2)).unwrap();
        let expected = if changed { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
            else { EndpointOutcome::Executed { resulting_version: 2 } };
        assert_eq!(publication.outcome, expected);
        assert_eq!(publication.basis, if changed { PublicationBasis::Rejected(Error::Incomplete) }
            else { PublicationBasis::Revalidated });
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().executions, u64::from(!changed));
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(expected));
        assert_eq!(host.inspect().control.ledger.available, if changed { 100 } else { 84 });
    }
}

#[test]
fn published_incomplete_image_withdraws_gate_until_a_new_complete_version_arrives() {
    let root = Directory::new();
    let first = snapshot(1, b"first");
    let (mut producer, _) = FileEvidencePublisher::create(root.source(), profile(), first.clone()).unwrap();
    let mut reader = producer.reader().unwrap();
    let (mut host, _) = FileOversight::create(root.0.join("effect"), gate_profile()).unwrap();
    host.enable_file_source(host.revision(), source_policy()).unwrap();
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
    let next = snapshot(2, b"incomplete");
    let mut incomplete = next.snapshot().clone();
    incomplete.complete = false;
    producer.publish(1, EvidenceSnapshot::new(next.identity(), incomplete, next.contexts().clone()).unwrap()).unwrap();
    assert_eq!(host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)), Err(FileSourceError::Refused(Error::Incomplete)));
    let status = host.file_source_status().unwrap();
    assert_eq!(status.producer.unwrap().generation, 2);
    assert!(status.capture.closed.is_none());
    producer.publish(2, snapshot(3, b"fresh complete context")).unwrap();
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(3)).unwrap();
    let status = host.file_source_status().unwrap();
    assert_eq!(status.producer.unwrap().generation, 3);
    assert!(status.capture.closed.is_some());
}
