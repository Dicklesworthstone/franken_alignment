//! Real socket replies plus the existing journal's five storage-failure hooks.
//! These are source scenarios, not hardware power-loss or runtime qualification.
use super::*;
use super::super::{FileOversightProfile, journal, machine::Machine};
use super::super::super::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::{ActionSpec, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::Consequence;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, action_frame};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-worker-barrier-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("worker barrier cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
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
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"worker-fault-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
fn setup(root: &Directory) -> (FileOversight, FileHelperPool, HelperClient<UnixStream>, CommitteeInput) {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let contracts = profile().committee;
    let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action);
    let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(0), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: 0, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
    let (server, peer) = UnixStream::pair().unwrap();
    let client = HelperClient::from_unix(peer, helper.profile_at(0)).unwrap();
    let pool = host.begin_helper_review(host.revision(), FileHelperLaunch { attempt: 1, round: 101,
        evidence_root: [9; 32], window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
        expected_input_revision: 1, streams: BTreeMap::from([("reviewer".to_owned(), server)]),
        limits: HelperLimits::default() }, snapshot()).unwrap();
    (host, pool, client, inputs)
}
fn queue_commit(host: &mut FileOversight, pool: &mut FileHelperPool, client: &mut HelperClient<UnixStream>) {
    for _ in 0..32 {
        pool.pump(host, ElapsedTick(1)).unwrap();
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference { client.respond(Verdict::Allow, b"salt").unwrap(); }
        if client.phase() == ClientPhase::SendingCommitment { client.step().unwrap(); }
        if client.phase() == ClientPhase::AwaitingReveal {
            assert!(!pool.statuses()["reviewer"].committed);
            return;
        }
    }
    panic!("commitment was not queued");
}
fn canonical(host: &FileOversight) -> (usize, Machine) {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    (events.len(), Machine::replay(&host.profile, &events).unwrap())
}

#[test]
fn commitment_storage_failure_never_opens_the_reveal_channel_even_after_visible_rename() {
    for stage in BARRIERS {
        let root = Directory::new();
        let (mut host, mut pool, mut client, input) = setup(&root);
        queue_commit(&mut host, &mut pool, &mut client);
        let before = host.revision();
        host.store.fail_once(stage);
        let failure = pool.pump(&mut host, ElapsedTick(1)).unwrap_err();
        let JournalError::Io(error) = failure.error else { panic!("expected storage barrier failure"); };
        assert_eq!(error.operation, stage);
        assert_eq!(failure.progress.io.len(), 1);
        assert!(pool.is_closed());
        assert!(!pool.statuses()["reviewer"].committed);
        assert!(!host.clock_ready());
        assert!(client.step().is_err(), "no reveal request may escape an unacknowledged phase");
        let (count, mut disk) = canonical(&host);
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(count as u64, before + u64::from(visible));
        let phase = disk.sessions.get_mut(&101).unwrap().1.open_reveals(ElapsedTick(1));
        if visible { phase.unwrap(); } else { assert_eq!(phase.unwrap_err(), Error::Incomplete); }
        assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
        drop(host);
        let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
        assert_eq!(recovered.inspect().executions, 0);
        assert!(recovered.commit_review(recovered.revision(), 101, "reviewer", 0).is_err());
    }
}

#[test]
fn finished_worker_review_is_not_returned_from_an_unacknowledged_journal_cut() {
    for stage in BARRIERS {
        let root = Directory::new();
        let (mut host, mut pool, mut client, input) = setup(&root);
        queue_commit(&mut host, &mut pool, &mut client);
        for _ in 0..32 {
            pool.pump(&mut host, ElapsedTick(1)).unwrap();
            client.step().unwrap();
            if pool.ready_to_finish() { break; }
        }
        assert!(pool.ready_to_finish());
        let before = host.revision();
        host.store.fail_once(stage);
        assert!(matches!(pool.finish(&mut host, ElapsedTick(1), Some(&input), snapshot()), Err(JournalError::Io(_))));
        assert!(pool.is_closed());
        assert_eq!(host.revision(), before);
        let (count, disk) = canonical(&host);
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(count as u64, before + u64::from(visible));
        assert_eq!(disk.broker.inspect().decisions.get(&1), if visible { Some(&Consequence::Continue) } else { None });
        assert_eq!(disk.sessions.contains_key(&101), !visible);
        assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
        drop(host);
        let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
        assert!(recovered.authorize(recovered.revision(), 1, &input, snapshot()).is_err());
        assert_eq!(recovered.inspect().executions, 0);
    }
}
