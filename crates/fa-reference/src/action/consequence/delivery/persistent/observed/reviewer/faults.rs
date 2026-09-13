//! Original approval and real reviewer sockets at every existing journal barrier.
//! These injected API failures are not hardware power-cut qualification.
use super::*;
use super::super::{FileOversightProfile, journal, machine::Machine};
use super::super::super::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::{ActionSpec, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-reviewer-barrier-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("reviewer barrier cleanup: {error}"); }
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
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::UnitsAtMost(100)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"reviewer-fault-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn setup(root: &Directory) -> (FileOversight, FileHumanReviewer, ReviewerConnection<UnixStream>, UnixStream, ReviewPacket) {
    let (mut host, role) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let contracts = profile().committee;
    let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(0), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: 0, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let input = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    host.record_inputs(host.revision(), 1, 0, input.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
    let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&input), snapshot()).unwrap().unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(20)).unwrap();
    let (socket, mut peer) = UnixStream::pair().unwrap(); peer.set_nonblocking(true).unwrap();
    let mut server = ReviewerConnection::from_unix(&host, &role, request, socket, [31; 32]).unwrap();
    let mut raw = Vec::new();
    for _ in 0..128 {
        server.step(&mut host, &role, || panic!("offer cannot decide")).unwrap();
        let mut scratch = [0; 4096];
        match peer.read(&mut scratch) {
            Ok(count) => raw.extend_from_slice(&scratch[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {},
            Err(error) => panic!("offer read: {error}"),
        }
        if raw.len() >= wire::OFFER_HEADER_BYTES && raw.len() == wire::offer_frame_len(&raw[..wire::OFFER_HEADER_BYTES]).unwrap() {
            let packet = ReviewPacket::decode(&raw).unwrap();
            return (host, role, server, peer, packet);
        }
    }
    panic!("offer exceeded fixture bound");
}

#[test]
fn no_second_key_or_receipt_escapes_any_failed_approval_barrier_even_if_rename_is_visible() {
    for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new();
        let (mut host, role, mut server, mut peer, packet) = setup(&root);
        peer.write_all(&packet.decision_frame(ReviewDecision::Approve)).unwrap();
        let before = host.revision();
        host.store.fail_once(stage);
        // Same clock tick isolates the injected failure to APPROVAL, not time.
        let error = server.step(&mut host, &role, || ElapsedTick(1)).unwrap_err();
        let ReviewerError::Journal(JournalError::Io(failure)) = error else { panic!("expected selected journal barrier"); };
        assert_eq!(failure.operation, stage);
        assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
        assert_eq!(server.committed(), None);
        assert_eq!(server.phase(), ReviewerPhase::Failed);
        assert_eq!(host.revision(), before);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
        assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0, "no success receipt may escape");
        let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
        let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
        let disk = Machine::replay(&host.profile, &events).unwrap();
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(events.len() as u64, before + u64::from(visible));
        assert_eq!(disk.broker.human_status(1001).unwrap().disposition,
            if visible { HumanDisposition::Approved } else { HumanDisposition::Pending });
        assert!(host.storage_failure().is_some());
        assert!(server.step(&mut host, &role, || panic!("latched failure cannot retry")).is_err());
        drop(host);
        let (mut recovered, fresh_role) = FileOversight::open(root.store(), profile()).unwrap();
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(recovered.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
        assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
        let request = recovered.human_request(1001).unwrap();
        let revision = recovered.revision();
        assert!(fresh_role.approve(&mut recovered, revision, &request).is_err());
        assert_eq!(recovered.inspect().executions, 0);
        assert_eq!(recovered.inspect().control.ledger.available, 100);
    }
}
