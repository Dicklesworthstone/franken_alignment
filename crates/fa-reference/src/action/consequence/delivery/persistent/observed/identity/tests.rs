//! Actual Store fault barriers and original-machine replay; no hardware crash claim.
#[path = "decoder/storage_tests.rs"] mod decoder_storage_tests;
use super::*;
use crate::action::{ActionSpec, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity};
use crate::action::consequence::activation::identity::IdentityAnchor;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalIo, JournalLimits};
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::action::consequence::oversight::identity::{IdentityOutcome, MAX_IDENTITY_CHECKS};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-identity-{}-{time}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("identity cleanup: {error}"); } }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
                tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".into(), MemberPolicy { cohort: "one".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(), retention_ticks: 1000,
            max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(InputProfileBinding {
            profile_id: 1, profile_bytes: b"identity-test".to_vec(), tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1,
        }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn passport() -> ModelPassport {
    let manifest = ModelManifest { tenant: 1, model: 9, model_generation: 1, host_generation: 1, tokenizer_generation: 1,
        weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32], architecture: [4; 32], numeric_profile: [5; 32] };
    ModelPassport::new(51, 1, manifest, vec![IdentityAnchor::new(10, CaptureProfile { tenant: 1, model: 9,
        model_generation: 1, tap: 4, layout_generation: 1 }, 5, vec![7], &[[-1.0, 1.0]]).unwrap()]).unwrap()
}
fn policy() -> IdentityPolicy { IdentityPolicy { observer_id: 99, timeout_ticks: 10, validity_ticks: 20, max_checks: 8 } }
fn frame(sequence: u64, value: f32) -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: passport().anchors()[&10].profile(), stream: 5, sequence, position: 0 }, &[value]).unwrap()
}
fn create(root: &Directory) -> (FileOversight, FileIdentityObserver) {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let observer = host.enable_identity_checks(host.revision(), passport(), policy()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); (host, observer)
}
fn begin(host: &mut FileOversight, observer: &FileIdentityObserver, id: u64) -> FileIdentityChallenge {
    let control = host.inspect().control;
    let check = host.begin_identity_check(host.revision(), id, control.sequence, host.actor_snapshot().unwrap().actor_revision).unwrap().unwrap();
    let revision = host.revision(); let now = host.inspect().control.ledger.elapsed.unwrap();
    observer.observe_manifest(host, revision, &check, passport().manifest().clone(), now).unwrap().measurement.unwrap(); check
}
fn measure(host: &mut FileOversight, observer: &FileIdentityObserver, check: &FileIdentityChallenge, sequence: u64) {
    let revision = host.revision(); let now = host.inspect().control.ledger.elapsed.unwrap();
    assert_eq!(observer.observe_anchor(host, revision, check, 10, &frame(sequence, 0.0), now).unwrap().measurement.unwrap().outcome, IdentityOutcome::Matched);
}
fn install(host: &mut FileOversight, check: &FileIdentityChallenge) {
    let control = host.inspect().control;
    host.apply_identity_check(host.revision(), check, control.sequence, control.ledger.epoch).unwrap();
}
fn canonical(host: &FileOversight) -> Machine {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    Machine::replay(&host.profile, &journal::decode(&host.profile, host.store.identity(), &bytes).unwrap()).unwrap()
}
fn fault(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected injected storage error"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn installation_faults_never_return_live_success_and_recovery_withdraws_visible_success() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, observer) = create(&root);
        let check = begin(&mut host, &observer, 1); measure(&mut host, &observer, &check, 1);
        assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
        let before = host.inspect(); let control = before.control.clone(); host.store.fail_once(stage);
        fault(host.apply_identity_check(host.revision(), &check, control.sequence, control.ledger.epoch).unwrap_err(), stage);
        assert_eq!(host.inspect(), before); assert_eq!(host.identity_status(), Err(JournalError::Unavailable));
        let disk = canonical(&host);
        assert_eq!(disk.broker.identity_installation(1).unwrap().is_some(), stage == JournalIo::DirectorySync);
        drop(host);
        let (mut host, _, observer) = FileOversight::open_with_identity_observer(root.store(), profile(), &passport(), policy()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
        let fresh = begin(&mut host, &observer, 2); measure(&mut host, &observer, &fresh, 2); install(&mut host, &fresh);
        assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Matching { check: 2, .. }));
    }
}

#[test]
fn mismatch_faults_recover_only_the_canonical_incident_never_an_unacknowledged_result() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, observer) = create(&root);
        let old = begin(&mut host, &observer, 1); measure(&mut host, &observer, &old, 1); install(&mut host, &old);
        let check = begin(&mut host, &observer, 2); let before = host.inspect();
        host.store.fail_once(stage); let revision = host.revision();
        fault(observer.observe_anchor(&mut host, revision, &check, 10, &frame(2, 3.0), ElapsedTick(1)).unwrap_err(), stage);
        assert_eq!(host.inspect(), before); assert_eq!(host.identity_status(), Err(JournalError::Unavailable));
        assert_eq!(canonical(&host).broker.inspect().suspended, stage == JournalIo::DirectorySync);
        drop(host);
        let (mut host, _, observer) = FileOversight::open_with_identity_observer(root.store(), profile(), &passport(), policy()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        if stage == JournalIo::DirectorySync {
            assert_eq!(host.identity_status().unwrap(), IdentityStatus::Mismatch { check: 2 });
            assert!(host.identity_installation(2).unwrap().is_some());
        } else {
            assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
            let check = begin(&mut host, &observer, 3); measure(&mut host, &observer, &check, 2); install(&mut host, &check);
        }
    }
}

#[test]
fn identity_measurement_cannot_clear_an_interrupted_policy_source_latch() {
    let root = Directory::new(); let (mut host, observer) = create(&root);
    host.source_interrupted = true; // The exact latch set before source acquisition.
    let check = begin(&mut host, &observer, 1); measure(&mut host, &observer, &check, 1); install(&mut host, &check);
    assert!(host.source_interrupted);
    let spec = ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"held".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100), units: 16 };
    let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() };
    assert_eq!(host.propose(host.revision(), 1, spec.clone(), snapshot.clone()), Err(JournalError::Contract(Error::Incomplete)));
    host.source_interrupted = false; // Paired positive control for the same identity and proposal.
    assert!(host.propose(host.revision(), 1, spec, snapshot).is_ok());
}

#[test]
fn journal_identity_inputs_roundtrip_and_cannot_install_an_unmeasured_match() {
    let events = [IdentityEvent::Enable(Rc::new(passport()), policy()), IdentityEvent::Begin(1, 0, 0),
        IdentityEvent::Manifest(1, passport().manifest().clone(), ElapsedTick(1)),
        IdentityEvent::Anchor(1, 10, frame(1, -0.0), ElapsedTick(1)), IdentityEvent::Apply(1, 0, 0), IdentityEvent::Unavailable(1)];
    for event in &events {
        let mut w = Writer::new(1_048_576); write(&mut w, event).unwrap(); let bytes = w.finish();
        let mut r = Reader::new(&bytes); let decoded = read(&mut r).unwrap(); r.end().unwrap();
        let mut w = Writer::new(1_048_576); write(&mut w, &decoded).unwrap(); assert_eq!(w.finish(), bytes);
        for end in 0..bytes.len() { assert!(read(&mut Reader::new(&bytes[..end])).is_err()); }
    }
    let mut settings = policy(); settings.max_checks = MAX_IDENTITY_CHECKS + 1;
    assert_eq!(write(&mut Writer::new(1_048_576), &IdentityEvent::Enable(Rc::new(passport()), settings)), Err(Error::Limit));
    let root = Directory::new(); let (mut host, observer) = create(&root); let check = begin(&mut host, &observer, 1);
    let control = host.inspect().control; let revision = host.revision();
    assert!(host.apply_identity_check(revision, &check, control.sequence, control.ledger.epoch).is_err());
    assert_eq!(host.revision(), revision);
    let mut history = host.events.clone(); history.push(Event::Identity(IdentityEvent::Apply(1, control.sequence, control.ledger.epoch)));
    assert!(Machine::replay(&profile(), &history).is_err());
    measure(&mut host, &observer, &check, 1); install(&mut host, &check);
}