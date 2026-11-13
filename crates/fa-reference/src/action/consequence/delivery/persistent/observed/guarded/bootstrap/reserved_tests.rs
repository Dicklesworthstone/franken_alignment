//! Original guarded first image, independent governor and retained recovery tail.
use super::super::{FileCampaignRequirement, FileRecoveryFloor, FileRecoveryRequirements};
use super::*;
use crate::Snapshot;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use crate::action::consequence::delivery::persistent::{
    FileDeliveryProfile, JournalIo, JournalLimits, RecoveryReserve, governance::PolicyUpdate,
};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource,
};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::oversight::policy_state::{
    StateFreshness, StateLimits, StateSource,
};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::policy_campaign::ReplayLimits;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fa-guarded-reserved-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf {
        self.0.join("publication")
    }
    fn canonical(&self) -> PathBuf {
        self.store().join(storage::CANONICAL)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("reserved guard cleanup: {error}");
        }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget {
        adapter: 10,
        object: 11,
        contract_version: 1,
        expected_version: 1,
        generation: 1,
    };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope {
                tenant: 1,
                principal: 2,
                run: 3,
                branch: 4,
                authority: 5,
                purpose: Purpose::Effect,
            },
            total: 100,
            max_attempts: 8,
            actor: ActorState::new(
                RestartProfile {
                    id: 1,
                    generation: 1,
                    host_generation: 1,
                    model_generation: 1,
                    tokenizer_generation: 1,
                    state_schema_generation: 1,
                    grade: RestartGrade::FunctionalRestart,
                },
                vec![1],
                vec![2],
                vec![3],
                1,
            )
            .unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy {
                generation: 1,
                members: BTreeMap::from([(
                    "reviewer".into(),
                    MemberPolicy {
                        cohort: "one".into(),
                        weight: 1,
                    },
                )]),
                caps: Caps {
                    per_member: 1,
                    per_cohort: 1,
                },
                continue_minimum: 1,
                continue_hold_maximum: 0,
                narrow_at: 2,
                suspend_at: 3,
                minimum_members: 1,
                minimum_cohorts: 1,
            },
            narrowed_targets: vec![target],
            target,
            initial_payload: b"initial".to_vec(),
            retention_ticks: 1000,
            max_deliveries: 8,
            clock_domain: 99,
            limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([(
            "reviewer".into(),
            HelperContract::new(
                InputProfileBinding {
                    profile_id: 1,
                    profile_bytes: b"reserved-guard-fixture".to_vec(),
                    tokenizer_epoch: 1,
                    policy_epoch: 0,
                    model_epoch: 1,
                },
                7,
                b"approve?".to_vec(),
            )
            .unwrap(),
        )]))
        .unwrap(),
        human: HumanReviewPolicy {
            reviewer_id: 77,
            max_validity_ticks: 50,
            max_requests: 8,
        },
    }
}
fn guards() -> FileGuardSet {
    FileGuardSet {
        stream: None,
        decoder: None,
        decoder_stop: None,
        source: Some(FileSourcePolicy {
            source: StateSource {
                scope: profile().delivery.scope,
                source: 51,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(50).unwrap(),
        }),
        identity: None,
        credential: None,
        campaigns: Some(FileCampaignRequirement {
            limits: ReplayLimits {
                cases: 16,
                input_bytes: 1_048_576,
            },
            max_campaigns: 8,
        }),
    }
}
fn expected(host: &FileOversight) -> FileRecoveryRequirements {
    let cut = host.inspect();
    FileRecoveryRequirements {
        guards: guards(),
        effective_policy: host.current_policy().unwrap().clone(),
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: cut.revision,
            control_sequence: cut.control.sequence,
            authority_epoch: cut.control.ledger.epoch,
        },
    }
}
fn seed(host: &mut FileOversight, root: &Directory, attempt: u64, tick: u64) {
    let capture = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: profile().delivery.scope,
        },
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        BTreeMap::from([("reviewer".to_owned(), b"original retained case".to_vec())]),
    )
    .unwrap();
    let path = root.0.join("source.json");
    fs::write(&path, capture.encode()).unwrap();
    let mut source =
        FileEvidenceSource::new(path, 51, profile().delivery.scope, 1_048_576).unwrap();
    host.observe_time(host.revision(), ElapsedTick(tick))
        .unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(tick))
        .unwrap();
    host.propose(
        host.revision(),
        attempt,
        ActionSpec {
            version: VERSION,
            scope: profile().delivery.scope,
            target: Some(host.inspect().target),
            payload: b"case".to_vec(),
            required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch,
            deadline: ElapsedTick(100),
            units: 8,
        },
        capture.snapshot().clone(),
    )
    .unwrap();
}

#[test]
fn complete_first_image_provisions_a_usable_governor_and_preserves_reserve_through_guarded_recovery()
 {
    let root = Directory::new();
    let (mut host, roles) = FileOversight::create_guarded_reserved(
        root.store(),
        profile(),
        &guards(),
        None,
        RecoveryReserve::terminal(),
    )
    .unwrap();
    assert!(host.publication_guard_required());
    assert!(host.policy_campaigns_required());
    assert_eq!(
        host.file_source_status().unwrap().policy,
        guards().source.unwrap()
    );
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    assert_eq!(host.revision(), 4);
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
    let bytes = fs::read(root.canonical()).unwrap();
    let events = journal::decode(&profile(), host.store.identity(), &bytes).unwrap();
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0], Event::PublicationGuard));
    assert!(matches!(
        events[1],
        Event::Core(super::super::super::BaseEvent::ReserveRecovery(_))
    ));
    let machine = Machine::replay(&profile(), &events).unwrap();
    guards().check(&machine, &events).unwrap();
    let governor = roles.policy_governor.unwrap();
    seed(&mut host, &root, 1, 1);
    let cut = host.inspect();
    let update = PolicyUpdate::new(
        91,
        cut.control.sequence,
        cut.control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(64)]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        host.replace_policy(host.revision(), &update),
        Err(JournalError::Contract(Error::Incomplete))
    );
    let review = host
        .request_policy_campaign(host.revision(), &update)
        .unwrap();
    assert_eq!(review.report().cases().len(), 1);
    let revision = host.revision();
    let key = governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    host.promote_policy_campaign(host.revision(), &key).unwrap();
    let expected = expected(&host);
    let before = host.revision();
    drop(host);
    let mut run = FileOversight::begin_open_guarded(root.store(), profile(), &expected)
        .unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal())
        .unwrap();
    while run.progress().status == super::super::recovery::FileGuardedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let (mut host, fresh_roles) = run.finish().unwrap();
    assert_eq!(host.revision(), before + 1);
    assert!(!host.clock_ready());
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    assert_eq!(
        host.file_source_status().unwrap().policy,
        guards().source.unwrap()
    );
    assert!(fresh_roles.policy_governor.is_some());
    let revision = host.revision();
    assert_eq!(
        governor.approve(&mut host, revision, &review, false).err(),
        Some(JournalError::Contract(Error::Binding))
    );
    seed(&mut host, &root, 2, 2);
    let cut = host.inspect();
    let update = PolicyUpdate::new(
        92,
        cut.control.sequence,
        cut.control.ledger.epoch,
        Policy::new(3, vec![Predicate::PayloadAtMost(32)]).unwrap(),
    )
    .unwrap();
    let review = host
        .request_policy_campaign(host.revision(), &update)
        .unwrap();
    let revision = host.revision();
    let key = fresh_roles
        .policy_governor
        .unwrap()
        .approve(&mut host, revision, &review, false)
        .unwrap();
    host.promote_policy_campaign(host.revision(), &key).unwrap();
    assert_eq!(host.current_policy().unwrap().generation(), 3);
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn independently_mismatched_guard_or_reserve_refuses_recovery_without_changing_canonical_history() {
    let root = Directory::new();
    let (host, _) = FileOversight::create_guarded_reserved(
        root.store(),
        profile(),
        &guards(),
        None,
        RecoveryReserve::terminal(),
    )
    .unwrap();
    let expected = expected(&host);
    let bytes = fs::read(root.canonical()).unwrap();
    drop(host);
    for mode in 0..3 {
        let mut wrong = expected.clone();
        match mode {
            0 => wrong.guards.campaigns = None,
            1 => wrong.guards.campaigns.as_mut().unwrap().max_campaigns += 1,
            _ => wrong.guards.source.as_mut().unwrap().source.generation += 1,
        }
        assert!(FileOversight::open_guarded(root.store(), profile(), &wrong).is_err());
        assert_eq!(fs::read(root.canonical()).unwrap(), bytes);
    }
    let mut wrong = RecoveryReserve::terminal();
    wrong.events += 1;
    assert!(
        FileOversight::begin_open_guarded(root.store(), profile(), &expected)
            .unwrap()
            .require_recovery_reserve(wrong)
            .is_err()
    );
    assert_eq!(fs::read(root.canonical()).unwrap(), bytes);
    let (host, roles) = FileOversight::open_guarded(root.store(), profile(), &expected).unwrap();
    assert!(roles.policy_governor.is_some());
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
}

#[test]
fn invalid_native_guard_and_reserve_inputs_create_no_store_and_cannot_replace_an_existing_one() {
    for mode in 0..5 {
        let root = Directory::new();
        let mut selected = profile();
        let mut guards = guards();
        let mut reserve = RecoveryReserve::terminal();
        match mode {
            0 => reserve.events = 0,
            1 => reserve.bytes = 0,
            2 => selected.delivery.limits.events = reserve.events,
            3 => guards.campaigns.as_mut().unwrap().max_campaigns = 0,
            _ => guards.source.as_mut().unwrap().source.scope.run += 1,
        }
        assert!(
            FileOversight::create_guarded_reserved(root.store(), selected, &guards, None, reserve)
                .is_err()
        );
        assert!(!root.store().exists());
    }
    let root = Directory::new();
    let (host, _) = FileOversight::create_guarded_reserved(
        root.store(),
        profile(),
        &guards(),
        None,
        RecoveryReserve::terminal(),
    )
    .unwrap();
    let bytes = fs::read(root.canonical()).unwrap();
    drop(host);
    assert!(
        FileOversight::create_guarded_reserved(
            root.store(),
            profile(),
            &guards(),
            None,
            RecoveryReserve::terminal()
        )
        .is_err()
    );
    assert_eq!(fs::read(root.canonical()).unwrap(), bytes);
}

#[test]
fn every_original_write_barrier_returns_no_partial_guarded_owner() {
    for barrier in [
        JournalIo::Stage,
        JournalIo::Write,
        JournalIo::FileSync,
        JournalIo::Rename,
        JournalIo::DirectorySync,
    ] {
        let root = Directory::new();
        let prepared = PreparedGuardedBootstrap::prepare_reserved(
            profile(),
            &guards(),
            None,
            RecoveryReserve::terminal(),
        )
        .unwrap();
        let store = storage::Store::create(&root.store()).unwrap();
        store.fail_once(barrier);
        let Err(JournalError::Io(failure)) = prepared.publish(store) else {
            panic!("expected original write barrier");
        };
        assert_eq!(failure.operation, barrier);
        assert_eq!(
            root.canonical().exists(),
            barrier == JournalIo::DirectorySync
        );
        if root.canonical().exists() {
            let store = storage::Store::open(&root.store()).unwrap();
            let bytes = store.read(profile().delivery.limits.bytes).unwrap();
            let events = journal::decode(&profile(), store.identity(), &bytes).unwrap();
            let machine = Machine::replay(&profile(), &events).unwrap();
            guards().check(&machine, &events).unwrap();
            assert_eq!(events.len(), 4);
            assert!(
                matches!(events[1], Event::Core(super::super::super::BaseEvent::ReserveRecovery(reserve)) if reserve == RecoveryReserve::terminal())
            );
            assert!(machine.publication_guard);
            assert!(machine.broker.policy_campaigns_required());
        }
    }
}
