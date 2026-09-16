//! Exercise the actual guarded recovery path with the original Store fault seam.
use super::*;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use crate::action::consequence::activation::identity::{IdentityAnchor, ModelManifest};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::Predicate;
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::identity::IdentityStatus;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-guarded-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("guarded cleanup: {error}"); } }
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
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("reviewer".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
            profile_id: 1, profile_bytes: b"guarded-fixture".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
        }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn guards() -> FileGuardSet {
    let manifest = ModelManifest { tenant: 1, model: 9, model_generation: 1, host_generation: 1, tokenizer_generation: 1,
        weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32], architecture: [4; 32], numeric_profile: [5; 32] };
    let anchor = IdentityAnchor::new(10, CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 },
        5, vec![7], &[[-1.0, 1.0]]).unwrap();
    FileGuardSet {
        stream: None, decoder: None, source: None, credential: None,
        identity: Some(FileIdentityRequirement { passport: ModelPassport::new(51, 1, manifest, vec![anchor]).unwrap(),
            policy: IdentityPolicy { observer_id: 99, timeout_ticks: 10, validity_ticks: 20, max_checks: 8 } }),
        campaigns: Some(FileCampaignRequirement { limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 }, max_campaigns: 8 }),
    }
}
fn setup(root: &Directory) -> (FileOversight, FileRecoveryRequirements) {
    let g = guards(); let identity = g.identity.as_ref().unwrap(); let campaign = g.campaigns.unwrap();
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let observer = host.enable_identity_checks(host.revision(), identity.passport.clone(), identity.policy).unwrap();
    host.enable_policy_campaigns(host.revision(), campaign.limits, campaign.max_campaigns).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let check = host.begin_identity_check(host.revision(), 1, 0, 0).unwrap().unwrap();
    let revision = host.revision();
    observer.observe_manifest(&mut host, revision, &check, identity.passport.manifest().clone(), ElapsedTick(1)).unwrap().measurement.unwrap();
    let anchor = &identity.passport.anchors()[&10];
    let frame = SourceFrame::capture(FrameIdentity { profile: anchor.profile(), stream: anchor.stream(), sequence: 1, position: 0 }, &[0.0]).unwrap();
    let revision = host.revision();
    observer.observe_anchor(&mut host, revision, &check, 10, &frame, ElapsedTick(1)).unwrap().measurement.unwrap();
    host.apply_identity_check(host.revision(), &check, 0, 0).unwrap();
    let control = host.inspect().control;
    let expected = FileRecoveryRequirements { guards: g, effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: control.sequence, authority_epoch: control.ledger.epoch } };
    (host, expected)
}

#[test]
fn every_fence_storage_failure_returns_no_owner_or_roles() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (host, expected) = setup(&root); let before = host.revision(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let error = FileOversight::open_guarded_store(store, profile(), &expected).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected injected fence failure"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.revision, before + u64::from(barrier == JournalIo::DirectorySync));
        // Reopening never resurrects saved eligibility, whether the first fence
        // reached rename or not. Only this successful fence exposes both roles.
        let (mut host, roles) = FileOversight::open_guarded(root.store(), profile(), &expected).unwrap();
        assert_eq!(host.revision(), disk.revision + 1); assert!(!host.clock_ready());
        assert!(roles.identity_observer.is_some()); assert!(roles.policy_governor.is_some());
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    }
}

#[test]
fn semantically_invalid_suffix_is_not_ignored_after_a_valid_guard_prefix() {
    let root = Directory::new(); let (host, expected) = setup(&root);
    let mut events = host.events.clone();
    events.push(Event::Identity(super::super::identity::IdentityEvent::Apply(999, 0, 0)));
    let malformed = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    let path = root.store().join(storage::CANONICAL); drop(host); std::fs::write(&path, &malformed).unwrap();
    assert!(FileOversight::open_guarded(root.store(), profile(), &expected).is_err());
    assert_eq!(std::fs::read(path).unwrap(), malformed);
}

#[test]
fn initial_publication_faults_never_leave_a_partially_guarded_canonical_owner() {
    use super::bootstrap::PreparedGuardedBootstrap;
    for barrier in BARRIERS {
        let root = Directory::new(); let declared = guards();
        let prepared = PreparedGuardedBootstrap::prepare(profile(), &declared, None).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(barrier);
        let error = prepared.publish(store).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected injected first-image failure"); };
        assert_eq!(failure.operation, barrier);
        let path = root.store().join(storage::CANONICAL);
        if barrier == JournalIo::DirectorySync {
            // The only visible candidate contains BOTH gates and the original
            // publication guard, despite the missing creation acknowledgment.
            let store = storage::Store::open(&root.store()).unwrap();
            let bytes = store.read(profile().delivery.limits.bytes).unwrap();
            let events = journal::decode(&profile(), store.identity(), &bytes).unwrap();
            let machine = Machine::replay(&profile(), &events).unwrap();
            declared.check(&machine, &events).unwrap();
            assert!(machine.identity_contract().is_some()); assert!(machine.broker.policy_campaigns_required());
            drop(store);
            let expected = FileRecoveryRequirements { guards: declared, effective_policy: profile().delivery.policy,
                credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: events.len() as u64,
                    control_sequence: 0, authority_epoch: 0 } };
            let (host, roles) = FileOversight::open_guarded(root.store(), profile(), &expected).unwrap();
            assert!(!host.clock_ready()); assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
        } else {
            assert!(!path.exists());
            // A pending bootstrap is never promoted just because it parses.
            assert!(FileOversight::open(root.store(), profile()).is_err());
        }
    }
}
