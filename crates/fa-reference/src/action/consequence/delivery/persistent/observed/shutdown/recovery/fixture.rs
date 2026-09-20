//! Real journal/two-key fixtures; deterministic votes are not model evaluation.
use super::*;
use crate::action::{ActionSpec, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit, JournalLimits, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::FileHumanPermit;
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
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Directory(PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-local-stop-recovery-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(super) fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("stop recovery cleanup: {error}"); }
    }
}
pub(super) fn profile() -> FileOversightProfile {
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
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
pub(super) fn request(host: &FileOversight) -> StopRequest {
    let control = host.inspect().control;
    StopRequest { operation: 900, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch }
}
pub(super) struct Ready {
    pub(super) host: FileOversight,
    action: FrozenAction, inputs: CommitteeInput, automatic: FilePermit, human: FileHumanPermit,
}
impl Ready {
    pub(super) fn new(root: &Directory, p: FileOversightProfile, reserve: bool) -> Self {
        let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
        if reserve { host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap(); }
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: p.delivery.scope,
            target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
        let helper = &p.committee.members()["reviewer"];
        let mut bytes = action_frame(&action); let boundary = bytes.len();
        bytes.extend_from_slice(helper.question()); let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
        ], Vec::new()).unwrap();
        let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
            policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
        let inputs = CommitteeInput::capture(&action, &p.committee, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
        host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
        host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
        let digest = commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
        host.commit_review(host.revision(), 101, "reviewer", digest).unwrap();
        host.open_reveals(host.revision(), 101).unwrap();
        host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
        host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
        let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
        let human_request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(10)).unwrap();
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &human_request).unwrap();
        Self { host, action, inputs, automatic, human }
    }
    pub(super) fn dispatch(&mut self) {
        self.host.dispatch(self.host.revision(), &self.automatic, &self.human, &self.action, &self.inputs, snapshot()).unwrap();
    }
}
