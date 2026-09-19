//! Real canonical replacements through public credential and two-key APIs.
//! Deterministic I/O barriers are not hardware power-loss qualification.
use super::*;
use super::super::{FILE_OVERSIGHT_CREDENTIAL_PROFILE, FileCredentialPermit};
use super::super::super::{FileHumanPermit, FileHumanReviewer, FileOversightProfile, FilePermit, ReviewWindow};
use super::super::super::super::{FileDeliveryProfile, JournalLimits};
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::credential_broker::{BrokerCredential, BrokerRouteBinding,
    CredentialRevocationRequest, CredentialRotationRequest, ProviderCredential};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, action_frame};
use crate::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::perimeter_inventory::LoadedPerimeterInventory;
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SECRET: &[u8] = b"credential-completion-secret-not-journal-data";
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-credential-complete-{}-{time}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("credential completion cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
                tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
                vec![1], vec![2], vec![3], 1).unwrap(),
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
fn inventory() -> LoadedPerimeterInventory {
    LoadedPerimeterInventory::from_json_bytes(format!(r#"{{"version":1,"families":[{{
      "scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"publication",
      "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
      "credentials":[{{"credential":"oversight-token","holder":"broker"}}],"routes":[{{
      "route":"adapter:file-oversight","effect":"file_write","profile":{{"id":"{}","generation":1}},
      "trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress",
      "actor_credential":{{"kind":"broker_mediated"}},"mediation":"brokered_effects","bypass":"blocked",
      "residual_nonclaims":["operator-owned reference boundary"]}}],
      "residual_nonclaims":["not provider authentication"]}}]}}"#, FILE_OVERSIGHT_CREDENTIAL_PROFILE).as_bytes()).unwrap()
}
fn route() -> BrokerRouteBinding {
    BrokerRouteBinding { family: "publication".into(), route: "adapter:file-oversight".into() }
}
fn bind(host: &FileOversight) -> FileCredentialPermit {
    host.bind_credential_pair(host.revision(), &inventory(), &route(),
        BrokerCredential::new(SECRET.to_vec()).unwrap(), ProviderCredential::new(SECRET.to_vec()).unwrap()).unwrap()
}
struct Ready {
    host: FileOversight,
    reviewer: FileHumanReviewer,
    action: FrozenAction,
    inputs: CommitteeInput,
    automatic: FilePermit,
    human: FileHumanPermit,
    credential: FileCredentialPermit,
}
impl Ready {
    fn complete(&mut self, tick: u64) -> Result<CheckedPublication, JournalError> {
        self.host.complete_credentialed_publication(self.host.revision(), CheckedCompletion {
            automatic: &self.automatic, human: &self.human, action: &self.action,
            current: &self.inputs, snapshot: snapshot(), now: ElapsedTick(tick),
        }, &self.credential)
    }
    fn bytes(&self) -> Vec<u8> { self.host.store.read(self.host.profile.delivery.limits.bytes).unwrap() }
}
fn ready(root: &Directory, p: FileOversightProfile) -> Ready {
    let (mut host, reviewer) = FileOversight::create(root.store(), p.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_credential_guard(host.revision(), &inventory(), &route()).unwrap();
    let credential = bind(&host);
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }, snapshot()).unwrap();
    let helper = &p.committee.members()["reviewer"];
    let mut bytes = action_frame(&action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(0), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 7, policy_epoch: 0, projected_originals: Vec::new(),
    }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &p.committee, BTreeMap::from([("reviewer".to_owned(), manifest)])).unwrap();
    host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer", commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    Ready { host, reviewer, action, inputs, automatic, human, credential }
}

#[test]
fn credential_required_publication_completes_and_uncredentialed_route_cannot_fallback() {
    let root = Directory::new(); let mut r = ready(&root, profile());
    let before = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.host.complete_checked_publication(r.host.revision(), CheckedCompletion {
        automatic: &r.automatic, human: &r.human, action: &r.action, current: &r.inputs,
        snapshot: snapshot(), now: ElapsedTick(2),
    }), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    let publication = r.complete(2).unwrap();
    assert_eq!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(r.host.revision(), before.revision + 4);
    let completed = r.host.inspect();
    assert_eq!(completed.payload, b"visible"); assert_eq!(completed.executions, 1);
    assert_eq!(completed.control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(completed.control.ledger.available, 84); assert_eq!(completed.control.ledger.reserved, 0);
    assert_eq!(completed.control.ledger.charged, 16);
    assert_eq!(r.host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), completed);
    assert!(!r.bytes().windows(SECRET.len()).any(|window| window == SECRET));
    let bytes = r.bytes(); assert!(r.complete(2).is_err());
    assert_eq!(r.host.inspect(), completed); assert_eq!(r.bytes(), bytes);
}

#[test]
fn four_original_sequential_operations_have_the_same_result_and_accounting() {
    let a = Directory::new(); let b = Directory::new();
    let mut atomic = ready(&a, profile()); let mut sequential = ready(&b, profile());
    let expected = atomic.complete(2).unwrap();
    sequential.host.observe_time(sequential.host.revision(), ElapsedTick(2)).unwrap();
    sequential.host.dispatch(sequential.host.revision(), &sequential.automatic, &sequential.human,
        &sequential.action, &sequential.inputs, snapshot()).unwrap();
    assert_eq!(sequential.host.publish_checked_with_credential(sequential.host.revision(), 1,
        Some(&sequential.inputs), snapshot(), ElapsedTick(2), &sequential.credential).unwrap(), expected);
    assert_eq!(sequential.host.reconcile(sequential.host.revision(), 1).unwrap(), Reconciliation::Resolved(expected.outcome));
    assert_eq!(atomic.host.inspect(), sequential.host.inspect());
}

#[test]
fn same_policy_foreign_credential_cannot_complete_this_owners_approvals() {
    let a = Directory::new(); let b = Directory::new();
    let mut r = ready(&a, profile()); let foreign = ready(&b, profile());
    let before = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.host.complete_credentialed_publication(r.host.revision(), CheckedCompletion {
        automatic: &r.automatic, human: &r.human, action: &r.action, current: &r.inputs,
        snapshot: snapshot(), now: ElapsedTick(2),
    }, &foreign.credential), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    assert!(r.complete(2).is_ok());
}

#[test]
fn rotation_invalidates_old_credential_but_new_pair_completes_original_approvals() {
    let root = Directory::new(); let mut r = ready(&root, profile());
    r.host.rotate_credential_guard(r.host.revision(), CredentialRotationRequest {
        operation: 20, expected_generation: 1, next_generation: 2,
    }).unwrap();
    let before = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.complete(2), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    r.credential = bind(&r.host); assert_eq!(r.credential.generation(), 2);
    assert!(r.complete(2).is_ok());
    assert_eq!(r.host.inspect().executions, 1);
}

#[test]
fn terminal_revocation_cannot_be_bypassed_by_a_retained_pair() {
    let root = Directory::new(); let mut r = ready(&root, profile());
    r.host.revoke_credential_guard(r.host.revision(), CredentialRevocationRequest {
        operation: 21, expected_generation: 1,
    }).unwrap();
    let before = r.host.inspect(); let bytes = r.bytes();
    assert_eq!(r.complete(2), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    assert_eq!(r.host.inspect().executions, 0);
    // Separate live control preserves the revoked domain's terminal state.
    let control_root = Directory::new(); assert!(ready(&control_root, profile()).complete(2).is_ok());
}

#[test]
fn revoked_human_key_and_expiry_still_block_credentialed_completion() {
    let root = Directory::new(); let mut r = ready(&root, profile());
    let before = r.host.inspect(); let bytes = r.bytes();
    assert!(r.complete(10).is_err()); assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    let revision = r.host.revision(); r.reviewer.revoke_all(&mut r.host, revision).unwrap();
    let before = r.host.inspect(); let bytes = r.bytes();
    assert!(r.complete(2).is_err()); assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    let control_root = Directory::new(); assert!(ready(&control_root, profile()).complete(9).is_ok());
}

#[test]
fn changed_payload_snapshot_or_interrupted_source_cannot_consume_approvals() {
    let root = Directory::new(); let mut r = ready(&root, profile());
    let before = r.host.inspect(); let bytes = r.bytes();
    let mut spec = r.action.spec().clone(); spec.payload = b"substituted".to_vec();
    let changed = FrozenAction::freeze(spec).unwrap();
    assert_eq!(r.host.complete_credentialed_publication(r.host.revision(), CheckedCompletion {
        automatic: &r.automatic, human: &r.human, action: &changed, current: &r.inputs,
        snapshot: snapshot(), now: ElapsedTick(2),
    }, &r.credential), Err(JournalError::Contract(Error::Binding)));
    let mut stale = snapshot(); stale.values.insert(7, b"different".to_vec());
    assert!(r.host.complete_credentialed_publication(r.host.revision(), CheckedCompletion {
        automatic: &r.automatic, human: &r.human, action: &r.action, current: &r.inputs,
        snapshot: stale, now: ElapsedTick(2),
    }, &r.credential).is_err());
    r.host.source_interrupted = true;
    assert_eq!(r.complete(2), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
    assert!(r.host.source_interrupted);
    let control_root = Directory::new(); assert!(ready(&control_root, profile()).complete(2).is_ok());
}

#[test]
fn stale_revision_and_full_batch_capacity_refuse_before_any_prefix_write() {
    let baseline_root = Directory::new(); let baseline = ready(&baseline_root, profile());
    let count = usize::try_from(baseline.host.revision()).unwrap();
    for spare in [3, 4] {
        let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = count + spare;
        let mut r = ready(&root, p); let before = r.host.inspect(); let bytes = r.bytes();
        assert_eq!(r.host.complete_credentialed_publication(r.host.revision() - 1, CheckedCompletion {
            automatic: &r.automatic, human: &r.human, action: &r.action, current: &r.inputs,
            snapshot: snapshot(), now: ElapsedTick(2),
        }, &r.credential), Err(JournalError::Contract(Error::Stale)));
        if spare == 3 {
            assert_eq!(r.complete(2), Err(JournalError::Contract(Error::Limit)));
            assert_eq!(r.host.inspect(), before); assert_eq!(r.bytes(), bytes);
            assert!(r.host.storage_failure().is_none());
        } else { assert!(r.complete(2).is_ok()); }
    }
}

#[test]
fn five_storage_barriers_preserve_old_or_fully_reconciled_cut_and_no_secret() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut r = ready(&root, profile());
        let before = r.host.inspect(); r.host.store.fail_once(barrier);
        let JournalError::Io(failure) = r.complete(2).unwrap_err() else { panic!("selected storage barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(r.host.inspect(), before); assert_eq!(r.complete(2), Err(JournalError::Unavailable));
        assert!(!r.bytes().windows(SECRET.len()).any(|window| window == SECRET));
        let visible = barrier == JournalIo::DirectorySync;
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.revision, before.revision + if visible { 4 } else { 0 });
        assert_eq!(disk.executions, u64::from(visible));
        assert_eq!(disk.control.ledger.stages[&1], if visible { ActionState::Confirmed } else { ActionState::Authorized });
        let Ready { host, automatic, human, action, inputs, credential, .. } = r;
        drop(host);
        let recovered = FileOversight::open_reconciled_publication(root.store(), profile(), ElapsedTick(3)).unwrap();
        let mut host = recovered.owner;
        let after = host.inspect();
        assert_eq!(after.control.ledger.available, if visible { 84 } else { 100 });
        assert_eq!(after.control.ledger.charged, if visible { 16 } else { 0 });
        assert_eq!(after.control.ledger.stages[&1], if visible { ActionState::Confirmed } else { ActionState::Cancelled });
        assert_eq!(host.complete_credentialed_publication(host.revision(), CheckedCompletion {
            automatic: &automatic, human: &human, action: &action, current: &inputs,
            snapshot: snapshot(), now: ElapsedTick(3),
        }, &credential), Err(JournalError::Contract(Error::Binding)));
        assert_eq!(host.inspect(), after);
    }
}
