//! Deterministic original Store barriers, not hardware power-loss qualification.
use super::*;
use super::super::{FileOversightProfile, Machine, journal};
use super::super::super::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::{ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-prediction-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory { fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("prediction cleanup: {error}"); } } }
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("helper".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
        max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("helper".into(), HelperContract::new(InputProfileBinding {
        profile_id: 1, profile_bytes: b"prediction-test".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
    }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
fn parameters() -> FileConsistencyParameters {
    let neutral = BinaryForecast::new(32768, 32768).unwrap();
    FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 },
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16, max_prediction_age_ticks: 10 }
}
fn source() -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: parameters().profile, stream: 17, sequence: 1, position: 0 }, &[0.0]).unwrap()
}
fn owner(root: &Directory) -> (FileOversight, FileConsistencyObserver) {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let observer = host.enable_action_consistency(host.revision(), FileConsistencyConfig::new(parameters()).unwrap()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); (host, observer)
}
fn predict(host: &mut FileOversight, observer: &FileConsistencyObserver) -> Result<Result<Prediction, Error>, JournalError> {
    let revision = host.revision(); let actor = host.actor_snapshot().unwrap().actor_revision;
    observer.forecast_action(host, revision, 1, actor, &source())
}
fn proposal(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"risk".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(50), units: 16 }
}
fn canonical(host: &FileOversight) -> FileConsistencySnapshot {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    Machine::replay(&host.profile, &events).unwrap().consistency_snapshot(events.len() as u64).unwrap()
}
fn check_failure(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("missing injected failure"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn prediction_failure_barriers_never_return_a_speculative_forecast_or_erase_visible_pending_work() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, observer) = owner(&root);
        let before = host.inspect(); host.store.fail_once(stage);
        check_failure(predict(&mut host, &observer).unwrap_err(), stage);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.action_consistency_snapshot(), Err(JournalError::Unavailable));
        let disk = canonical(&host); let visible = stage == JournalIo::DirectorySync;
        assert_eq!(disk.pending_attempt, visible.then_some(1)); assert_eq!(disk.evidence.samples(), 0);
        drop(host);
        let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let recovered = host.action_consistency_snapshot().unwrap();
        assert_eq!(recovered.pending_attempt, disk.pending_attempt);
        assert_eq!(recovered.coverage_lost, visible);
        assert_eq!(recovered.evidence, disk.evidence); assert!(!host.clock_ready());
    }
}

#[test]
fn refused_proposal_failure_barriers_keep_the_canonical_observation_or_censor_the_pending_forecast() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, observer) = owner(&root);
        predict(&mut host, &observer).unwrap().unwrap();
        let before = host.inspect(); let spec = proposal(&host); host.store.fail_once(stage);
        let missing = Snapshot { semantic_epoch: 1, complete: false, values: BTreeMap::new() };
        check_failure(host.propose_consistent(host.revision(), 1, spec, missing).unwrap_err(), stage);
        assert_eq!(host.inspect(), before);
        let disk = canonical(&host); let visible = stage == JournalIo::DirectorySync;
        assert_eq!(disk.evidence.samples(), usize::from(visible));
        assert_eq!(disk.pending_attempt, (!visible).then_some(1));
        assert!(host.propose(host.revision(), 1, proposal(&host), Snapshot { semantic_epoch: 1,
            complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }).is_err());
        drop(host);
        let (host, _) = FileOversight::open(root.store(), profile()).unwrap();
        let recovered = host.action_consistency_snapshot().unwrap();
        assert_eq!(recovered.evidence, disk.evidence);
        assert_eq!(recovered.coverage_lost, !visible);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert!(!host.inspect().control.ledger.stages.contains_key(&1));
    }
}

#[test]
fn explicit_capture_loss_remains_available_without_clearing_source_interruption() {
    let root = Directory::new(); let (mut host, observer) = owner(&root);
    predict(&mut host, &observer).unwrap().unwrap();
    host.source_interrupted = true;
    let revision = host.revision(); observer.unavailable(&mut host, revision).unwrap();
    assert!(host.source_interrupted);
    assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(host.action_consistency_snapshot().unwrap().pending_attempt, Some(1));
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn exact_configuration_vector_and_every_truncation_use_original_constructors() {
    let config = FileConsistencyConfig::new(parameters()).unwrap();
    let mut expected = b"FACPRED\x01".to_vec();
    for n in [1_u64, 1, 1, 9, 1, 4, 1] { expected.extend_from_slice(&n.to_be_bytes()); }
    expected.extend_from_slice(&1_u32.to_be_bytes());
    for n in [1.0_f32.to_bits(), 0, 0] { expected.extend_from_slice(&n.to_be_bytes()); }
    for n in [19_u64, 1, 1] { expected.extend_from_slice(&n.to_be_bytes()); }
    expected.extend_from_slice(&4_u32.to_be_bytes()); expected.extend_from_slice(b"risk");
    for _ in 0..6 { expected.extend_from_slice(&32768_u32.to_be_bytes()); }
    for n in [1_u64, 4, 17] { expected.extend_from_slice(&n.to_be_bytes()); }
    expected.extend_from_slice(&16_u32.to_be_bytes()); expected.extend_from_slice(&10_u64.to_be_bytes());
    assert_eq!(config.encoded(), expected);
    for end in 0..expected.len() { assert!(FileConsistencyConfig::from_bytes(&expected[..end]).is_err()); }
    let mut bad = parameters(); bad.weights[0] = f32::NAN;
    assert_eq!(FileConsistencyConfig::new(bad), Err(Error::InvalidInput));
    let mut different = parameters(); different.bias = -0.0;
    assert_ne!(FileConsistencyConfig::new(different).unwrap(), config);
    let events = [ConsistencyEvent::Enable(Rc::new(config)), ConsistencyEvent::Forecast(1, 0, source()), ConsistencyEvent::Unavailable];
    for event in events {
        let mut w = super::super::super::codec::shared::Writer::new(10000); write(&mut w, &event).unwrap(); let bytes = w.finish();
        for end in 0..bytes.len() { assert!(read(&mut super::super::super::codec::shared::Reader::new(&bytes[..end])).is_err()); }
        let mut r = super::super::super::codec::shared::Reader::new(&bytes); let decoded = read(&mut r).unwrap(); r.end().unwrap();
        let mut w = super::super::super::codec::shared::Writer::new(10000); write(&mut w, &decoded).unwrap(); assert_eq!(w.finish(), bytes);
    }
}
