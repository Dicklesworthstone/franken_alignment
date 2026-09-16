//! Storage barriers for durable credential generation and revocation.
use super::*;
use super::super::{FileOversight, FileOversightProfile, journal, machine::Machine};
use super::super::super::{FileDeliveryProfile, JournalError, JournalIo, JournalLimits};
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::credential_broker::{CredentialRevocationRequest, CredentialRotationRequest};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Error;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-credential-barrier-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
                tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
            max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 },
            7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn create(root: &Directory) -> FileOversight {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.transact(host.revision(), Event::CredentialGuard(FileCredentialPolicy {
        family: "publication".into(), route: "adapter:file-oversight".into(), credential: "oversight-token".into(), profile_generation: 1,
    })).unwrap(); host
}
fn canonical(host: &FileOversight) -> Machine {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}
fn check_failure(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected selected storage failure"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn rotation_barriers_expose_no_candidate_generation_and_reopen_uses_the_canonical_cut() {
    for barrier in BARRIERS {
        let root = Directory::new(); let mut host = create(&root);
        let request = CredentialRotationRequest { operation: 11, expected_generation: 1, next_generation: 2 };
        host.store.fail_once(barrier);
        check_failure(host.rotate_credential_guard(host.revision(), request).unwrap_err(), barrier);
        assert_eq!(host.credential_status(), Err(JournalError::Unavailable));
        let disk = canonical(&host); let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.credential_generation, if visible { 2 } else { 1 });
        assert_eq!(disk.credential_changes.len(), if visible { 1 } else { 0 });
        drop(host);
        let (mut reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
        let revision = reopened.revision();
        if visible {
            let receipt = reopened.credential_change(11).unwrap();
            assert_eq!(receipt.generation, 2);
            assert_eq!(reopened.rotate_credential_guard(0, request).unwrap(), receipt);
            assert_eq!(reopened.revision(), revision);
        } else {
            assert_eq!(reopened.credential_change(11), Err(JournalError::Contract(Error::Missing)));
            assert_eq!(reopened.rotate_credential_guard(revision, request).unwrap().generation, 2);
        }
    }
}

#[test]
fn revocation_barriers_never_guess_terminal_credential_state_after_ambiguous_io() {
    for barrier in BARRIERS {
        let root = Directory::new(); let mut host = create(&root);
        let request = CredentialRevocationRequest { operation: 21, expected_generation: 1 };
        host.store.fail_once(barrier);
        check_failure(host.revoke_credential_guard(host.revision(), request).unwrap_err(), barrier);
        assert_eq!(host.credential_status(), Err(JournalError::Unavailable));
        let disk = canonical(&host); let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.credential_revoked, visible);
        assert_eq!(disk.credential_changes.len(), if visible { 1 } else { 0 });
        drop(host);
        let (mut reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
        let revision = reopened.revision();
        if visible {
            let receipt = reopened.credential_change(21).unwrap(); assert!(receipt.revoked);
            assert_eq!(reopened.revoke_credential_guard(0, request).unwrap(), receipt);
            assert_eq!(reopened.revision(), revision);
        } else {
            assert_eq!(reopened.credential_change(21), Err(JournalError::Contract(Error::Missing)));
            assert!(reopened.revoke_credential_guard(revision, request).unwrap().revoked);
        }
    }
}
