//! Real canonical-file recovery tests; fixtures are not authentication evidence.
mod portable;

use super::*;
use super::super::{FileGuardSet, FileRecoveryFloor, FileCampaignRequirement};
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::policy_campaign::ReplayLimits;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-history-anchor-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn canonical(&self) -> PathBuf { self.store().join(storage::CANONICAL) }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("history anchor test cleanup: {error}");
        }
    }
}

fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1,
        expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4,
                authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1,
                host_generation: 1, model_generation: 1, tokenizer_generation: 1,
                state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
                vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("reviewer".into(), MemberPolicy {
                    cohort: "one".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
                continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99,
            limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(),
            HelperContract::new(InputProfileBinding { profile_id: 1,
                profile_bytes: b"anchor-fixture".to_vec(), tokenizer_epoch: 1,
                policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn requirements(host: &FileOversight) -> FileRecoveryRequirements {
    let control = host.inspect().control;
    FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
        credential_epoch: None, minimum: FileRecoveryFloor {
            journal_revision: host.revision(), control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch } }
}
fn setup(root: &Directory) -> (FileOversight, FileRecoveryRequirements) {
    let (mut host, _) = FileOversight::create_guarded(root.store(), profile(), &guards(), None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let expected = requirements(&host);
    (host, expected)
}

#[test]
fn exact_anchor_and_genuine_successors_recover_without_restoring_clock_eligibility() {
    let root = Directory::new();
    let (host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap();
    assert_eq!(anchor.revision(), host.revision());
    assert_eq!(anchor.canonical, std::fs::read(root.canonical()).unwrap());
    let initial_revision = host.revision();
    drop(host);
    let (mut host, _roles) = FileOversight::open_guarded_anchored(
        root.store(), profile(), &expected, &anchor).unwrap();
    assert_eq!(host.revision(), initial_revision + 1);
    assert!(!host.clock_ready());
    assert!(host.inspect().control.ledger.epoch > expected.minimum.authority_epoch);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let successor_revision = host.revision();
    let newer = host.history_anchor().unwrap();
    assert!(newer.revision() > anchor.revision());
    drop(host);
    // The header's event count changed, but the original prefix did not.
    let (host, _) = FileOversight::open_guarded_anchored(
        root.store(), profile(), &expected, &anchor).unwrap();
    assert_eq!(host.revision(), successor_revision + 1);
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().control.ledger.available, 100);
    newer.check(&host.profile, host.store.identity(), &host.events).unwrap();
}

#[test]
fn equal_counter_valid_fork_is_rejected_before_any_recovery_write() {
    let root = Directory::new();
    let (host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap();
    let original = std::fs::read(root.canonical()).unwrap();
    let mut fork = host.events.clone();
    *fork.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(2)));
    let fork_machine = Machine::replay(&host.profile, &fork).unwrap();
    // Causal control: the EXISTING counter/guard checks accept this other valid
    // history. The new prefix comparison, not parser failure, must reject it.
    expected.check(&host.profile, &fork_machine, &fork).unwrap();
    assert_eq!(fork_machine.broker.inspect().sequence, host.inspect().control.sequence);
    let bytes = journal::encode(&host.profile, host.store.identity(), &fork).unwrap();
    assert_ne!(bytes, original);
    drop(host);
    std::fs::write(root.canonical(), &bytes).unwrap();
    assert_eq!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor)
        .unwrap_err(), JournalError::Contract(Error::Binding));
    assert_eq!(std::fs::read(root.canonical()).unwrap(), bytes);
    std::fs::write(root.canonical(), &original).unwrap();
    assert!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor).is_ok());
}

#[test]
fn truncated_history_cannot_use_a_newer_anchor_even_with_an_older_numeric_floor() {
    let root = Directory::new();
    let (mut host, expected) = setup(&root);
    let old = std::fs::read(root.canonical()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let anchor = host.history_anchor().unwrap();
    let current = std::fs::read(root.canonical()).unwrap();
    drop(host);
    std::fs::write(root.canonical(), &old).unwrap();
    assert_eq!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor)
        .unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(std::fs::read(root.canonical()).unwrap(), old);
    std::fs::write(root.canonical(), current).unwrap();
    assert!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor).is_ok());
}

#[test]
fn identical_profile_and_events_at_another_storage_identity_do_not_match() {
    let first = Directory::new();
    let second = Directory::new();
    let (host, _) = setup(&first);
    let foreign = host.history_anchor().unwrap();
    let (other, expected) = setup(&second);
    let own = other.history_anchor().unwrap();
    assert_eq!(own.revision(), foreign.revision());
    let before = std::fs::read(second.canonical()).unwrap();
    drop(other);
    assert_eq!(FileOversight::open_guarded_anchored(second.store(), profile(), &expected, &foreign)
        .unwrap_err(), JournalError::Contract(Error::Binding));
    assert_eq!(std::fs::read(second.canonical()).unwrap(), before);
    assert!(FileOversight::open_guarded_anchored(second.store(), profile(), &expected, &own).is_ok());
}

#[test]
fn matching_anchor_cannot_replace_independent_guards_or_higher_floors() {
    let root = Directory::new();
    let (host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap();
    let before = std::fs::read(root.canonical()).unwrap();
    drop(host);
    let mut wrong = expected.clone();
    wrong.guards.campaigns = Some(FileCampaignRequirement {
        limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 }, max_campaigns: 8 });
    assert_eq!(FileOversight::open_guarded_anchored(root.store(), profile(), &wrong, &anchor)
        .unwrap_err(), JournalError::Contract(Error::Binding));
    wrong = expected.clone();
    wrong.minimum.control_sequence = u64::MAX;
    assert_eq!(FileOversight::open_guarded_anchored(root.store(), profile(), &wrong, &anchor)
        .unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(std::fs::read(root.canonical()).unwrap(), before);
    assert!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor).is_ok());
}

#[test]
fn exact_prefix_never_excuses_an_invalid_unanchored_suffix() {
    let root = Directory::new();
    let (host, expected) = setup(&root);
    let anchor = host.history_anchor().unwrap();
    let original = std::fs::read(root.canonical()).unwrap();
    let mut events = host.events.clone();
    events.push(Event::Core(BaseEvent::Time(ElapsedTick(0))));
    anchor.check(&host.profile, host.store.identity(), &events).unwrap();
    let bytes = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    drop(host);
    std::fs::write(root.canonical(), &bytes).unwrap();
    assert_eq!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor)
        .unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(std::fs::read(root.canonical()).unwrap(), bytes);
    std::fs::write(root.canonical(), original).unwrap();
    assert!(FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor).is_ok());
}

#[test]
fn every_fence_storage_barrier_returns_no_owner_and_allows_anchored_recovery() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync]
    {
        let root = Directory::new();
        let (host, expected) = setup(&root);
        let anchor = host.history_anchor().unwrap();
        let revision = host.revision();
        drop(host);
        let store = storage::Store::open(&root.store()).unwrap();
        store.fail_once(barrier);
        let error = FileOversight::open_guarded_anchored_store(store, profile(), &expected, &anchor).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("original injected fence failure"); };
        assert_eq!(failure.operation, barrier);
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.revision, revision + u64::from(barrier == JournalIo::DirectorySync));
        let (host, _) = FileOversight::open_guarded_anchored(root.store(), profile(), &expected, &anchor).unwrap();
        assert_eq!(host.revision(), disk.revision + 1);
        assert!(!host.clock_ready());
    }
}

#[test]
fn failed_owner_cannot_anchor_old_ram_as_current_acknowledged_history() {
    let root = Directory::new();
    let (mut host, _) = setup(&root);
    let before = host.history_anchor().unwrap();
    assert!(!format!("{before:?}").contains("anchor-fixture"));
    host.store.fail_once(JournalIo::Write);
    assert!(matches!(host.observe_time(host.revision(), ElapsedTick(2)), Err(JournalError::Io(_))));
    assert_eq!(host.history_anchor(), Err(JournalError::Unavailable));
}
