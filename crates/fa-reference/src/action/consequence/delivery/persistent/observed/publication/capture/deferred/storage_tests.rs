//! Deterministic canonical replacement barriers, not hardware power-cut tests.
use super::*;
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::FileOversightProfile;
use crate::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
use crate::action::consequence::delivery::publication_gate::PublicationLimits;
use crate::action::consequence::delivery::publication_gate::changes::{PublicationChange, PublicationChangePolicy, PublicationInputCut};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::witness::refinement::RefinementBudget;
use crate::witness::refinement::index::routing::{RoutingBudget, WitnessChange};
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-deferred-storage-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn source(&self) -> PublicationInputFile { PublicationInputFile::new(self.0.join("input.bin"), 91).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("deferred fixture cleanup: {error}"); }
    }
}
fn profile(events: usize) -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(64)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99,
            limits: JournalLimits { events, ..JournalLimits::default() },
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn fixture(root: &Directory, p: FileOversightProfile) -> FileOversight {
    let (mut host, _) = FileOversight::create_with_publication_validation(root.store(), p,
        PublicationLimits { bindings: 8, validation: RefinementBudget { steps: 10_000, value_bytes: 1_048_576 } }).unwrap();
    host.enable_publication_changes(host.revision(), PublicationChangePolicy { source: 41, after: 0,
        lookup: RoutingBudget { steps: 10_000, bytes: 1_048_576 } }).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: host.profile.delivery.scope,
        target: Some(host.inspect().target), payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 },
        Snapshot { semantic_epoch: 1, complete: true, ..Snapshot::default() }).unwrap();
    // The trusted binding freezes a bounded opaque witness; no helper votes,
    // human approval, automatic permit or inference result is fabricated here.
    let actual = ActualHelperInput::new(b"opaque".to_vec(), InputProfileBinding { profile_id: 1,
        profile_bytes: b"deferred-storage".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 },
        vec![SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: 6 } }], Vec::new()).unwrap();
    let inputs = FilePublicationInputs::new(None, Some(actual));
    let cut = PublicationInputCut { source: 41, through: 0 };
    let original = FilePublicationCapture::new_at_cut(1, FileCaptureIdentity { source: 91, generation: 1 },
        &action, inputs.clone(), cut).unwrap();
    host.bind_publication_file_source(host.revision(), 1, original, Vec::new()).unwrap();
    host.record_publication_change(host.revision(), PublicationChange { source: 41, sequence: 1,
        change: WitnessChange::All }).unwrap();
    let lag = FilePublicationCapture::new_at_cut(1, FileCaptureIdentity { source: 91, generation: 2 },
        &action, inputs, cut).unwrap();
    std::fs::write(root.0.join("input.bin"), lag.to_bytes().unwrap()).unwrap();
    assert_eq!(host.revision(), 6); host
}
fn disk(host: &FileOversight) -> Machine {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}

#[test]
fn every_withdrawal_failure_prevents_a_positive_read_acknowledgment() {
    for barrier in BARRIERS {
        let root = Directory::new(); let p = profile(32); let mut host = fixture(&root, p.clone());
        let before = host.inspect(); host.store.fail_once(barrier);
        assert!(matches!(host.refresh_publication_from_file_or_defer(before.revision, 1, &root.source()),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(host.inspect(), before); assert!(!host.clock_ready());
        assert_eq!(disk(&host).broker.publication_source(1).unwrap().unwrap().generation, 1);
        drop(host);
        let (recovered, _) = FileOversight::open(root.store(), p).unwrap();
        assert_eq!(recovered.publication_source(1).unwrap().unwrap().generation, 1);
        assert!(!recovered.publication_source(1).unwrap().unwrap().fresh);
        assert_eq!(recovered.inspect().executions, 0);
    }
}

#[test]
fn ambiguous_observation_acknowledgments_recover_the_actual_new_or_old_generation() {
    for barrier in BARRIERS {
        let root = Directory::new(); let p = profile(32); let mut host = fixture(&root, p.clone());
        let expected = host.begin_deferrable_publication_capture(host.revision(), 1, 91).unwrap();
        let capture = root.source().read_capture().unwrap(); let before = host.inspect();
        host.store.fail_once(barrier);
        assert!(matches!(host.finish_publication_capture_or_defer(1, expected, capture),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert_eq!(host.inspect(), before); assert!(!host.clock_ready());
        assert_eq!(host.refresh_publication_from_file_or_defer(host.revision(), 1, &root.source()),
            Err(JournalError::Unavailable));
        let generation = if barrier == JournalIo::DirectorySync { 2 } else { 1 };
        let recovered = disk(&host);
        assert_eq!(recovered.broker.publication_source(1).unwrap().unwrap().generation, generation);
        assert!(!recovered.broker.publication_source(1).unwrap().unwrap().fresh);
        drop(host);
        let (recovered, _) = FileOversight::open(root.store(), p).unwrap();
        assert_eq!(recovered.publication_source(1).unwrap().unwrap().generation, generation);
        assert!(!recovered.publication_source(1).unwrap().unwrap().fresh);
        assert_eq!(recovered.inspect().executions, 0);
    }
}

#[test]
fn exact_event_capacity_never_acknowledges_an_unpersisted_deferral() {
    for enough in [false, true] {
        let root = Directory::new(); let p = profile(if enough { 8 } else { 7 });
        let mut host = fixture(&root, p);
        let result = host.refresh_publication_from_file_or_defer(host.revision(), 1, &root.source());
        if enough {
            assert!(result.unwrap().unwrap().outcome.deferred()); assert!(host.storage_failure().is_none());
        } else {
            assert_eq!(result, Err(Error::Limit.into())); assert!(host.storage_failure().is_some());
        }
        assert_eq!(host.revision(), if enough { 8 } else { 7 });
        assert_eq!(disk(&host).broker.publication_source(1).unwrap().unwrap().generation, if enough { 2 } else { 1 });
        assert!(!disk(&host).broker.publication_source(1).unwrap().unwrap().fresh);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn explicit_deferral_records_recompute_native_truth_instead_of_importing_an_outcome() {
    let root = Directory::new(); let mut host = fixture(&root, profile(32));
    host.refresh_publication_from_file_or_defer(host.revision(), 1, &root.source()).unwrap().unwrap();
    let bytes = journal::encode(&host.profile, host.store.identity(), &host.events).unwrap();
    let mut events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    let native = Machine::replay(&host.profile, &events).unwrap();
    assert_eq!(native.broker.publication_source(1).unwrap().unwrap().generation, 2);
    assert!(!native.broker.publication_source(1).unwrap().unwrap().fresh);
    let Some(Event::PublicationWitness(WitnessEvent::CapturedOrDefer(id, revision, capture))) = events.pop() else {
        panic!("new explicit observation operation");
    };
    // Downgrading an otherwise canonical frame to the strict operation cannot
    // launder a lagging image into installed inputs. Original replay refuses it.
    events.push(Event::PublicationWitness(WitnessEvent::Captured(id, revision, capture)));
    let strict = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    let decoded = journal::decode(&host.profile, host.store.identity(), &strict).unwrap();
    assert!(matches!(Machine::replay(&host.profile, &decoded), Err(Error::Stale)));
}
