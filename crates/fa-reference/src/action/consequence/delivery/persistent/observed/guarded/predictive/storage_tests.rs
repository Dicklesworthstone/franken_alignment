//! Original Store failure barriers, not a substitute for hardware crash tests.
use super::*;
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::consistency::FileConsistencyParameters;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use super::super::FileRecoveryFloor;
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
        let root = std::env::temp_dir().join(format!("fa-predictive-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("predictive cleanup: {error}"); } }
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
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
                MemberPolicy { cohort: "one".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
            profile_id: 1, profile_bytes: b"predictive-fixture".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
        }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn capture() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 }
}
fn expected() -> FilePredictiveRequirements {
    let pair = BinaryForecast::new(16384, 49152).unwrap();
    FilePredictiveRequirements {
        oversight: FileRecoveryRequirements {
            guards: FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
                identity: None, campaigns: None, credential: None },
            effective_policy: profile().delivery.policy, credential_epoch: None,
            minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 },
        },
        prediction: FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 1, probe_generation: 1,
            profile: capture(), weights: vec![1.0], bias: 0.0, threshold: 0.0,
            forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1,
                event_prefix: b"risk".to_vec(), negative: pair, at_threshold: pair, positive: pair },
            alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16, max_prediction_age_ticks: 10,
        }).unwrap(), evaluation: None,
    }
}
fn failure(error: JournalError, barrier: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected original Store failure"); };
    assert_eq!(failure.operation, barrier);
    assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn first_image_failure_never_exposes_a_partial_guarded_owner() {
    for barrier in BARRIERS {
        let root = Directory::new(); let expected = expected();
        let prepared = PreparedGuardedBootstrap::prepare(profile(), &expected.oversight.guards, None).unwrap()
            .predictive(expected.prediction.clone()).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(barrier);
        failure(prepared.publish(store).unwrap_err(), barrier);
        let visible = root.store().join(storage::CANONICAL).exists();
        assert_eq!(visible, barrier == JournalIo::DirectorySync);
        if visible {
            let disk = FileOversight::read_predictive_consistency(root.store(), &profile(), &expected).unwrap();
            assert_eq!(disk.journal.revision, 2); assert_eq!(disk.consistency.evidence.samples(), 0);
            let (host, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &expected).unwrap();
            assert_eq!(host.revision(), 3); assert!(!host.clock_ready());
        } else {
            assert!(FileOversight::open_predictive_guarded(root.store(), profile(), &expected).is_err());
        }
    }
}

#[test]
fn recovery_fence_failure_cannot_erase_unanswered_forecasts_or_issue_old_roles() {
    for barrier in BARRIERS {
        let root = Directory::new(); let expected = expected();
        let (mut host, old) = FileOversight::create_predictive_guarded(root.store(), profile(),
            &expected.oversight.guards, None, expected.prediction.clone(), None).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let frame = SourceFrame::capture(FrameIdentity { profile: capture(), stream: 17, sequence: 1, position: 0 }, &[-1.0]).unwrap();
        let revision = host.revision();
        old.consistency_observer.forecast_action(&mut host, revision, 1, 0, &frame).unwrap().unwrap();
        let before = host.revision(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        failure(FileOversight::open_predictive_store(store, profile(), &expected).unwrap_err(), barrier);
        let disk = FileOversight::read_predictive_consistency(root.store(), &profile(), &expected).unwrap();
        assert_eq!(disk.journal.revision, before + u64::from(barrier == JournalIo::DirectorySync));
        assert_eq!(disk.consistency.coverage_lost, barrier == JournalIo::DirectorySync);
        assert_eq!(disk.consistency.pending_attempt, Some(1));
        let (mut host, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &expected).unwrap();
        assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 0);
        let revision = host.revision();
        assert!(matches!(old.consistency_observer.unavailable(&mut host, revision), Err(JournalError::Contract(Error::Binding))));
        assert_eq!(host.revision(), revision);
    }
}

#[test]
fn duplicate_or_omitted_prediction_is_rejected_by_preflight_not_accepted_as_a_new_budget() {
    let expected = expected();
    let event = Event::Consistency(ConsistencyEvent::Enable(Rc::new(expected.prediction.clone())));
    assert!(check_prediction(std::slice::from_ref(&event), Some(&expected.prediction)).is_ok());
    assert_eq!(check_prediction(std::slice::from_ref(&event), None), Err(Error::Binding));
    assert_eq!(check_prediction(&[], Some(&expected.prediction)), Err(Error::Binding));
    assert_eq!(check_prediction(&[event.clone(), event], Some(&expected.prediction)), Err(Error::Binding));
}
