//! Faults at original replacement barriers; no fabricated stop acknowledgments.
use super::*;
use super::super::{FilePredictiveRoles, Event, ConsistencyEvent, BaseEvent};
use super::super::super::{FileGuardSet, FileRecoveryRequirements, FileRecoveryFloor};
use super::super::super::bootstrap::PreparedGuardedBootstrap;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope, ActionSpec, VERSION};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::{consistency::{FileConsistencyConfig, FileConsistencyParameters}, journal};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::consistency::ConsistencyStopCause;
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-consistency-stop-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("owner") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("stop fixture cleanup: {error}"); } }
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None, identity: None, campaigns: None, credential: None }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            vec![1], vec![2], vec![3], 1).unwrap(), suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::UnitsAtMost(16)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("helper".into(),
            MemberPolicy { cohort: "a".into(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
        max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("helper".into(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"full-input".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 },
        7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn config() -> FileConsistencyConfig {
    FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 },
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1, event_prefix: b"risk".to_vec(),
            negative: BinaryForecast::new(16384, 49152).unwrap(), at_threshold: BinaryForecast::new(32768, 32768).unwrap(),
            positive: BinaryForecast::new(49152, 16384).unwrap() },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16, max_prediction_age_ticks: 10,
    }).unwrap().with_terminal_stop(ConsistencyStopPolicy::new(11, 12, 7000).unwrap()).unwrap()
}
fn expected() -> FilePredictiveRequirements {
    FilePredictiveRequirements { oversight: FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 } },
        prediction: config(), evaluation: None }
}
fn start(root: &Directory) -> (FileOversight, FilePredictiveRoles) {
    let (mut h, roles) = FileOversight::create_predictive_guarded(root.store(), profile(), &guards(), None, config(), None).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap(); (h, roles)
}
fn forecast(h: &mut FileOversight, roles: &FilePredictiveRoles, attempt: u64) {
    let frame = SourceFrame::capture(FrameIdentity { profile: CaptureProfile { tenant: 1, model: 9, model_generation: 1,
        tap: 4, layout_generation: 1 }, stream: 17, sequence: attempt, position: 0 }, &[-1.0]).unwrap();
    let r = h.revision(); let actor = h.actor_snapshot().unwrap().actor_revision;
    roles.consistency_observer.forecast_action(h, r, attempt, actor, &frame).unwrap().unwrap();
}
fn propose(h: &mut FileOversight, attempt: u64) -> Result<Result<crate::action::FrozenAction, crate::Error>, JournalError> {
    let s = h.inspect(); h.propose_consistent(h.revision(), attempt, ActionSpec { version: VERSION,
        scope: profile().delivery.scope, target: Some(s.target), payload: b"risk".to_vec(), required_witnesses: vec![],
        policy_epoch: s.control.ledger.epoch, deadline: ElapsedTick(100), units: 16 },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() })
}

#[test]
fn initial_policy_and_recovery_stop_are_atomic_at_all_original_storage_barriers() {
    for stage in BARRIERS {
        let root = Directory::new();
        let prepared = PreparedGuardedBootstrap::prepare(profile(), &guards(), None).unwrap().predictive(config()).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(stage);
        assert!(prepared.publish(store).is_err());
        let disk = FileOversight::read_predictive_stop(root.store(), &profile(), &expected());
        if stage == JournalIo::DirectorySync {
            let disk = disk.unwrap(); assert_eq!(disk.policy, config().terminal_stop_policy()); assert!(disk.incident.is_none());
            assert_eq!(disk.history.journal.revision, 2);
            let (host, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &expected()).unwrap();
            assert!(host.inspect().stop.is_none());
        } else { assert!(disk.is_err()); }

        let root = Directory::new(); let (mut h, roles) = start(&root); forecast(&mut h, &roles, 1); drop(h);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(stage);
        assert!(FileOversight::open_predictive_store(store, profile(), &expected()).is_err());
        let disk = FileOversight::read_predictive_stop(root.store(), &profile(), &expected()).unwrap();
        assert_eq!(disk.incident.is_some(), stage == JournalIo::DirectorySync);
        let (mut h, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &expected()).unwrap();
        let incident = h.consistency_stop_incident().unwrap().unwrap();
        assert_eq!(incident.cause, ConsistencyStopCause::CoverageLost); assert_eq!(incident.pending_attempt, Some(1));
        assert_eq!(incident.observed_samples, 0); assert!(!h.clock_ready());
        assert!(h.progress_stop(h.revision(), ElapsedTick(2)).unwrap().progress.drained());
    }
}

#[test]
fn lost_crossing_acknowledgment_retains_actual_samples_and_recovers_unobserved_outcomes_as_loss() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut h, roles) = start(&root);
        forecast(&mut h, &roles, 1); propose(&mut h, 1).unwrap().unwrap(); forecast(&mut h, &roles, 2);
        let before = h.inspect(); h.store.fail_once(stage);
        assert!(propose(&mut h, 2).is_err()); assert!(h.storage_failure().is_some()); assert_eq!(h.inspect(), before);
        assert_eq!(h.consistency_stop_incident(), Err(JournalError::Unavailable));
        let disk = FileOversight::read_predictive_stop(root.store(), &profile(), &expected()).unwrap();
        let committed = stage == JournalIo::DirectorySync;
        assert_eq!(disk.history.consistency.evidence.samples(), if committed { 2 } else { 1 });
        assert_eq!(disk.incident.is_some(), committed);
        drop(h);
        let (mut h, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &expected()).unwrap();
        let incident = h.consistency_stop_incident().unwrap().unwrap();
        assert_eq!(incident.cause, if committed { ConsistencyStopCause::ThresholdCrossed { first_sample: 2 } }
            else { ConsistencyStopCause::CoverageLost });
        assert_eq!(incident.observed_samples, if committed { 2 } else { 1 });
        assert_eq!(h.inspect().executions, 0); assert!(h.progress_stop(h.revision(), ElapsedTick(2)).unwrap().progress.drained());
    }
}

#[test]
fn canonical_inspection_requires_the_whole_history_and_does_not_clean_staging() {
    let root = Directory::new(); let (mut h, roles) = start(&root); let r = h.revision();
    roles.consistency_observer.unavailable(&mut h, r).unwrap();
    let path = root.store(); let original = std::fs::read(path.join("delivery.bin")).unwrap();
    std::fs::write(path.join("delivery.pending"), b"retain this evidence").unwrap();
    let control = FileOversight::read_predictive_stop(&path, &profile(), &expected()).unwrap();
    assert!(control.incident.is_some());
    for event in [Event::Consistency(ConsistencyEvent::Enable(Rc::new(config()))), Event::Core(BaseEvent::Reconcile(99))] {
        let mut events = h.events.clone(); events.push(event);
        let bytes = journal::encode(&profile(), h.store.identity(), &events).unwrap();
        std::fs::write(path.join("delivery.bin"), bytes).unwrap();
        assert!(FileOversight::read_predictive_stop(&path, &profile(), &expected()).is_err());
        assert_eq!(std::fs::read(path.join("delivery.pending")).unwrap(), b"retain this evidence");
    }
    std::fs::write(path.join("delivery.bin"), original).unwrap();
    assert_eq!(FileOversight::read_predictive_stop(&path, &profile(), &expected()).unwrap(), control);
}
