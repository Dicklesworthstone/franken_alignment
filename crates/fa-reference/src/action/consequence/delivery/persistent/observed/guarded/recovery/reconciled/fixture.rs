//! Original two-key native publication fixtures, not calibration evidence.
use super::*;
use crate::action::{ActionSpec, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanReviewer};
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
        let path = std::env::temp_dir().join(format!("fa-guarded-reconcile-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(super) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(super) fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join("delivery.bin")).unwrap() }
    pub(super) fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        std::fs::read_dir(self.store()).unwrap().map(|entry| {
            let entry = entry.unwrap(); assert!(entry.file_type().unwrap().is_file());
            let path = entry.path(); let bytes = std::fs::read(&path).unwrap(); (path, bytes)
        }).collect()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("guarded reconciliation cleanup: {error}"); }
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
pub(super) fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
pub(super) fn requirements(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let state = host.inspect();
    FileRecoveryRequirements { guards, effective_policy: host.profile.delivery.policy.clone(),
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: state.revision,
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch } }
}
pub(super) fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
pub(super) struct Ready {
    pub(super) host: FileOversight,
    pub(super) automatic: FilePermit,
    pub(super) human: FileHumanPermit,
    pub(super) action: FrozenAction,
    pub(super) inputs: CommitteeInput,
}
impl Ready {
    pub(super) fn new(root: &Directory) -> Self {
        let p = profile();
        let (mut host, roles) = FileOversight::create_guarded(root.store(), p.clone(), &guards(), None).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        Self::from_owner(host, roles.human, 1, 101, ElapsedTick(1))
    }
    pub(super) fn from_owner(mut host: FileOversight, reviewer: FileHumanReviewer,
        attempt: u64, round: u64, now: ElapsedTick) -> Self {
        let p = host.profile.clone();
        host.observe_time(host.revision(), now).unwrap();
        let action = host.propose(host.revision(), attempt, ActionSpec { version: VERSION, scope: p.delivery.scope,
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
        host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
        host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
        let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
        host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
        host.open_reveals(host.revision(), round).unwrap();
        host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
        host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
        let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
        let request = host.request_human_approval(host.revision(), 1000 + attempt, attempt, &inputs, ElapsedTick(10)).unwrap();
        let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
        Self { host, action, inputs, automatic, human }
    }
    pub(super) fn dispatch(&mut self) {
        self.host.dispatch(self.host.revision(), &self.automatic, &self.human,
            &self.action, &self.inputs, snapshot()).unwrap();
    }
    pub(super) fn publish(&mut self) { self.publish_at(1, ElapsedTick(2)); }
    pub(super) fn publish_at(&mut self, attempt: u64, now: ElapsedTick) {
        self.host.publish_checked(self.host.revision(), attempt, Some(&self.inputs), snapshot(), now).unwrap();
    }
}
