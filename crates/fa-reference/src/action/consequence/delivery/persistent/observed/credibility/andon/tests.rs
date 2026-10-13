use super::*;
use super::super::{FileIndependentEvaluator, codec};
use super::super::super::{FileOversightProfile, Machine, journal, storage};
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use crate::action::consequence::oversight::credibility::{Assessment, EvaluationProtocol, Fraction, GroundTruth};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write,
    JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
pub(crate) struct Directory(PathBuf);
impl Directory {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-native-andon-journal-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    pub(crate) fn store(&self) -> PathBuf { self.0.join("publication") }
    pub(crate) fn bytes(&self) -> Vec<u8> { std::fs::read(self.store().join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("Andon test cleanup: {error}"); }
    }
}
pub(crate) fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile { delivery: FileDeliveryProfile {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
            tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            vec![1], vec![2], vec![3], 1).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(),
            MemberPolicy { cohort: "one".into(), weight: 1 })]),
            caps: Caps { per_member: 2, per_cohort: 2 }, continue_minimum: 1, continue_hold_maximum: 0,
            narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
        max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
    }, committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
        profile_id: 1, profile_bytes: b"native-andon-test".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 11,
    }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 } }
}
pub(crate) fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3, minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 }, recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
pub(crate) fn budget() -> AndonBudget {
    AndonBudget { window: 1, start: 0, end: 5, false_stops_allowed: 2, cost_per_stop: u64::MAX }
}
pub(crate) fn config() -> FileAndonConfig {
    FileAndonConfig::new(LedgerIdentity { deployment: 101, evaluator: 202 }, vec![budget()]).unwrap()
}
pub(crate) fn owner(root: &Directory) -> (FileOversight, FileIndependentEvaluator) {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let evaluator = host.enable_credibility(host.revision(), protocol()).unwrap();
    host.enable_andon(host.revision(), &config()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, evaluator)
}
pub(crate) fn case(host: &mut FileOversight, id: u64, verdict: Verdict) {
    let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() };
    let p = profile(); let round = 100 + id;
    let action = host.propose(host.revision(), id, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(host.inspect().target), payload: b"reviewed".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }, snapshot.clone()).unwrap();
    let helper = &p.committee.members()["reviewer"];
    let mut bytes = action_frame(&action); let start = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: start } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start, end } }], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &p.committee, BTreeMap::from([("reviewer".into(), view)])).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot.clone()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], verdict, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", verdict, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot).unwrap().unwrap();
}
pub(crate) fn assess(host: &mut FileOversight, evaluator: &FileIndependentEvaluator, id: u64, truth: GroundTruth)
    -> Result<bool, JournalError>
{
    let ticket = host.evaluation_ticket(id + 100)?;
    let revision = host.revision();
    evaluator.assess(host, revision, &ticket, Assessment { origin: id, evidence_id: [17; 32], truth })
}

#[test]
fn original_history_recovers_labels_receipts_and_equal_tick_acceptance_order_together() {
    let root = Directory::new(); let (mut host, evaluator) = owner(&root);
    case(&mut host, 1, Verdict::Hold); case(&mut host, 2, Verdict::Hold);
    let control = host.inspect().control;
    assess(&mut host, &evaluator, 2, GroundTruth::Benign).unwrap();
    assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap();
    assert_eq!(host.inspect().control, control);
    let ledger = host.andon_ledger().unwrap().clone(); let report = host.credibility_report().unwrap();
    let receipts: Vec<_> = host.andon_source().unwrap().receipts().iter().map(|r| (**r).clone()).collect();
    assert_eq!(receipts.iter().map(|r| r.case().round).collect::<Vec<_>>(), vec![102, 101]);
    assert_eq!(ledger.windows()[&1].incident.as_ref().unwrap().opened_by_round, 101);
    assert_eq!(ledger.windows()[&1].total_cost, 2 * u128::from(u64::MAX));
    drop(host);
    let (restored, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(!restored.clock_ready());
    assert_eq!(restored.andon_ledger().unwrap(), &ledger);
    assert_eq!(restored.credibility_report().unwrap(), report);
    let recovered: Vec<_> = restored.andon_source().unwrap().receipts().iter().map(|r| (**r).clone()).collect();
    assert_eq!(recovered, receipts);
}

#[test]
fn censored_resolution_and_delayed_labels_recover_the_original_occurrence_window() {
    let root = Directory::new(); let (mut host, evaluator) = owner(&root);
    case(&mut host, 1, Verdict::Hold);
    assess(&mut host, &evaluator, 1, GroundTruth::Censored).unwrap();
    host.register_andon_window(host.revision(), AndonBudget { window: 2, start: 5, end: 100, ..budget() }).unwrap();
    host.observe_time(host.revision(), ElapsedTick(40)).unwrap();
    assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap();
    let receipt = host.andon_receipt(101).unwrap().clone();
    assert_eq!((receipt.confirmation().occurred_at, receipt.confirmation().confirmed_at), (1, 40));
    let bytes = root.bytes(); let revision = host.revision();
    assert!(!assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap());
    assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), bytes);
    drop(host); let (restored, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(restored.evaluation_history(101).unwrap().len(), 2);
    assert_eq!(restored.andon_receipt(101).unwrap(), &receipt);
    assert_eq!(restored.andon_ledger().unwrap().windows()[&2].false_stops, 0);
}

#[test]
fn failed_native_accounting_does_not_commit_a_label_and_corrected_retry_succeeds() {
    let root = Directory::new(); let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let evaluator = host.enable_credibility(host.revision(), protocol()).unwrap();
    let early = FileAndonConfig::new(config().identity(), vec![AndonBudget { end: 1, ..budget() }]).unwrap();
    host.enable_andon(host.revision(), &early).unwrap(); host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    case(&mut host, 1, Verdict::Hold);
    let bytes = root.bytes(); let before = host.credibility_report().unwrap(); let revision = host.revision();
    assert_eq!(assess(&mut host, &evaluator, 1, GroundTruth::Benign), Err(JournalError::Contract(Error::Missing)));
    assert_eq!(root.bytes(), bytes); assert_eq!(host.revision(), revision);
    assert_eq!(host.credibility_report().unwrap(), before); assert!(host.storage_failure().is_none());
    assert!(host.evaluation_history(101).unwrap().is_empty());
    host.register_andon_window(host.revision(), AndonBudget { window: 2, start: 1, end: 10, ..budget() }).unwrap();
    assert!(assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap());
    assert_eq!(host.andon_ledger().unwrap().windows()[&2].false_stops, 1);
}

#[test]
fn every_original_store_barrier_keeps_label_and_charge_in_the_same_canonical_cut() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, evaluator) = owner(&root);
        case(&mut host, 1, Verdict::Hold);
        let before = host.inspect(); host.store.fail_once(stage);
        let error = assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected Store failure"); };
        assert_eq!(failure.operation, stage); assert_eq!(host.inspect(), before);
        assert_eq!(host.andon_ledger(), Err(JournalError::Unavailable));
        assert_eq!(host.andon_receipt(101), Err(JournalError::Unavailable));
        let bytes = root.bytes();
        let events = journal::decode(&profile(), host.store.identity(), &bytes).unwrap();
        let recovered = Machine::replay(&profile(), &events).unwrap();
        let visible = stage == JournalIo::DirectorySync;
        assert_eq!(recovered.broker.evaluation_history(101).unwrap().len(), usize::from(visible));
        assert_eq!(recovered.broker.andon_ledger().unwrap().windows()[&1].false_stops, u64::from(visible));
        assert_eq!(recovered.broker.andon_receipt(101).is_ok(), visible);
        drop(host); let (restored, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(restored.andon_ledger().unwrap(), recovered.broker.andon_ledger().unwrap());
    }
}

#[test]
fn bootstrap_window_retries_and_invalid_history_do_not_reset_accounting() {
    let root = Directory::new(); let (mut host, evaluator) = owner(&root);
    case(&mut host, 1, Verdict::Hold); assess(&mut host, &evaluator, 1, GroundTruth::Benign).unwrap();
    let bytes = root.bytes(); let revision = host.revision(); let binding = host.andon_binding().unwrap().clone();
    assert_eq!(host.enable_andon(0, &config()).unwrap(), binding);
    assert!(!host.register_andon_window(0, budget()).unwrap());
    let changed = FileAndonConfig::new(config().identity(), vec![AndonBudget { cost_per_stop: 1, ..budget() }]).unwrap();
    assert_eq!(host.enable_andon(revision, &changed), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.register_andon_window(revision, AndonBudget { false_stops_allowed: 100, ..budget() }), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), bytes);
    for event in [CredibilityEvent::EnableAndon(config()), CredibilityEvent::AndonWindow(budget())] {
        let mut invalid = host.events.clone(); invalid.push(Event::Credibility(event));
        assert!(Machine::replay(&profile(), &invalid).is_err());
    }
}

#[test]
fn pending_and_real_violation_labels_survive_without_fabricating_false_stops() {
    let root = Directory::new(); let (mut host, evaluator) = owner(&root);
    case(&mut host, 1, Verdict::Hold); case(&mut host, 2, Verdict::Allow);
    assess(&mut host, &evaluator, 1, GroundTruth::Violation).unwrap();
    let before = host.credibility_report().unwrap(); assert_eq!(before.pending_cases, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    drop(host); let (restored, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(restored.credibility_report().unwrap(), before);
    assert_eq!(restored.andon_receipt(101), Err(JournalError::Contract(Error::Missing)));
}

#[test]
fn fixed_new_tags_reject_every_truncation_noncanonical_order_and_oversize_count() {
    let event = CredibilityEvent::EnableAndon(config());
    let mut w = Writer::new(10_000); codec::write(&mut w, &event).unwrap(); let bytes = w.finish();
    let mut expected = vec![7]; expected.extend_from_slice(&101_u64.to_be_bytes());
    expected.extend_from_slice(&202_u64.to_be_bytes()); expected.extend_from_slice(&1_u32.to_be_bytes());
    for value in [1_u64, 0, 5, 2, u64::MAX] { expected.extend_from_slice(&value.to_be_bytes()); }
    assert_eq!(bytes, expected);
    for end in 0..bytes.len() { assert!(codec::read(&mut Reader::new(&bytes[..end])).is_err()); }
    let mut reader = Reader::new(&bytes); let decoded = codec::read(&mut reader).unwrap(); reader.end().unwrap();
    let mut w = Writer::new(10_000); codec::write(&mut w, &decoded).unwrap(); assert_eq!(w.finish(), bytes);
    let mut excessive = bytes.clone(); excessive[17..21].copy_from_slice(&((MAX_WINDOWS + 1) as u32).to_be_bytes());
    assert!(codec::read(&mut Reader::new(&excessive)).is_err());
    let two = FileAndonConfig::new(config().identity(), vec![budget(), AndonBudget { window: 2, start: 5, end: 10, ..budget() }]).unwrap();
    let mut w = Writer::new(10_000); codec::write(&mut w, &CredibilityEvent::EnableAndon(two)).unwrap();
    let mut reversed = w.finish(); reversed[21..101].rotate_left(40);
    assert!(codec::read(&mut Reader::new(&reversed)).is_err());
    assert_eq!(FileAndonConfig::new(config().identity(), vec![budget(), budget()]), Err(Error::Duplicate));
    assert_eq!(FileAndonConfig::new(config().identity(), vec![]), Err(Error::Incomplete));
    assert_eq!(FileAndonConfig::new(config().identity(), vec![budget(); MAX_WINDOWS + 1]), Err(Error::Limit));
}
