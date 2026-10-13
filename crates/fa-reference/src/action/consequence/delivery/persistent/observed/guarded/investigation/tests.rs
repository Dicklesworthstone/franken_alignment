//! Native denied proposals, original reducers and immutable historical evidence.
use super::*;
use super::super::{FileGuardSet, FileRecoveryFloor};
use super::super::anchored::FileHistoryAnchor;
use crate::action::{ActionSpec, ActionState, ElapsedTick, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits, MAX_JOURNAL_EVENTS};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate, Truth};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract};
use crate::action::consequence::oversight::human::HumanReviewPolicy;
use crate::action::consequence::experiment::NextRequirement;
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-proposal-read-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("store") }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(self.store()).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
    }
}
impl Drop for Root {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("proposal read cleanup: {error}"); } }
}
fn scope() -> InterventionScope { InterventionScope::new(700, true, false, false, &[]).unwrap() }
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1,
        expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(4)]).unwrap(),
            congress: CongressPolicy { generation: 1,
                members: BTreeMap::from([("reviewer".into(), MemberPolicy { cohort: "fixture".into(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".into(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"proposal-read".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn setup(root: &Root) -> (FileOversight, FileRecoveryRequirements, FileHistoryAnchor) {
    let p = profile();
    let (mut host, _) = FileOversight::create_guarded(root.store(), p.clone(), &guards(), None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: p.delivery.scope,
        target: Some(p.delivery.target), payload: b"blocked".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100), units: 2 },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }).unwrap();
    assert_eq!(action.spec().payload, b"blocked");
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Denied);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let view = host.inspect();
    let expected = FileRecoveryRequirements { guards: guards(), effective_policy: p.delivery.policy,
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: view.revision,
            control_sequence: view.control.sequence, authority_epoch: view.control.ledger.epoch } };
    let anchor = host.history_anchor().unwrap();
    fs::write(root.store().join("delivery.pending"), b"never promote or delete investigation staging").unwrap();
    (host, expected, anchor)
}
fn begin(root: &Root, expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor) -> FileProposalRead {
    FileOversight::begin_read_proposal_investigation_anchored(root.store(), &profile(), expected, anchor, 1, scope()).unwrap()
}
fn complete(read: &mut FileProposalRead, quantum: usize) {
    for _ in 0..=read.progress().total_events {
        let before = read.progress();
        let after = read.advance(before.replayed_events, quantum).unwrap();
        assert!(after.attempted_events - before.attempted_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
        if after.status == FileProposalReadStatus::Ready { return; }
        assert_eq!(after.status, FileProposalReadStatus::Reading);
    }
    panic!("bounded original replay failed to finish");
}

#[test]
fn cooperative_and_eager_diagnoses_match_the_original_denial_without_a_writer_lock() {
    let root = Root::new(); let (host, expected, anchor) = setup(&root); let files = root.files();
    let original = host.investigate_proposal(1, scope()).unwrap();
    assert_eq!(original.origin().proposal().state, ActionState::Denied);
    assert!(original.origin().admission_snapshot().revision < original.origin().journal_snapshot().revision);
    for quantum in [1, 2, MAX_JOURNAL_EVENTS] {
        let mut read = begin(&root, &expected, &anchor);
        complete(&mut read, quantum);
        let result = read.finish().unwrap();
        assert_eq!(result.origin(), original.origin());
        for edits in [vec![], vec![Intervention::Payload(b"ok".to_vec())]] {
            assert_eq!(result.run(&edits).unwrap().result(), original.run(&edits).unwrap().result());
        }
        let repaired = result.run(&[Intervention::Payload(b"ok".to_vec())]).unwrap();
        assert_eq!(repaired.result().baseline(), Truth::Violated);
        assert_eq!(repaired.result().counterfactual(), Truth::Satisfied);
        assert_eq!(repaired.result().next_requirement(), NextRequirement::FreshIndependentReview);
        assert_eq!(repaired.purpose(), Purpose::Experiment); assert_eq!(root.files(), files);
    }
    let result = FileOversight::read_proposal_investigation_anchored(root.store(), &profile(),
        &expected, &anchor, 1, scope()).unwrap();
    assert_eq!(result.origin(), original.origin()); assert_eq!(host.inspect(), *original.origin().journal_snapshot());
}

#[test]
fn every_partial_cut_and_ready_cancellation_preserves_all_bytes_and_yields_no_partial_proposal() {
    let root = Root::new(); let (host, expected, anchor) = setup(&root); let files = root.files();
    let count = usize::try_from(host.revision()).unwrap();
    for cut in 0..=count {
        let mut read = begin(&root, &expected, &anchor);
        for done in 0..cut { read.advance(done, 1).unwrap(); }
        let progress = read.cancel();
        assert_eq!(progress.status, FileProposalReadStatus::Cancelled);
        assert_eq!(progress.replayed_events, cut); assert_eq!(root.files(), files);
        if cut < count {
            let mut read = begin(&root, &expected, &anchor);
            for done in 0..cut { read.advance(done, 1).unwrap(); }
            assert_eq!(read.finish().err(), Some(Error::Incomplete.into()));
        }
    }
    let mut read = begin(&root, &expected, &anchor); complete(&mut read, 1);
    assert!(read.finish().unwrap().run(&[Intervention::Payload(vec![])]).is_ok());
    assert_eq!(root.files(), files);
}

#[test]
fn stale_polls_do_no_work_and_wrong_ending_requirements_latch_without_returning_the_good_prefix() {
    let root = Root::new(); let (_host, expected, anchor) = setup(&root); let files = root.files();
    let mut read = begin(&root, &expected, &anchor); let before = read.progress();
    for (done, quantum, error) in [(1, 1, Error::Stale), (0, 0, Error::InvalidInput),
        (0, MAX_JOURNAL_EVENTS + 1, Error::Limit)] {
        assert_eq!(read.advance(done, quantum).err(), Some(error.into())); assert_eq!(read.progress(), before);
    }
    complete(&mut read, 1); assert!(read.finish().is_ok());
    for case in 0..4 {
        let mut wrong = expected.clone();
        let error = match case {
            0 => { wrong.minimum.journal_revision += 1; Error::Stale }
            1 => { wrong.minimum.control_sequence += 1; Error::Stale }
            2 => { wrong.minimum.authority_epoch += 1; Error::Stale }
            _ => { wrong.effective_policy = Policy::new(1, vec![Predicate::PayloadAtMost(3)]).unwrap(); Error::Binding }
        };
        let mut read = begin(&root, &wrong, &anchor);
        assert_eq!(read.advance(0, MAX_JOURNAL_EVENTS).err(), Some(error.into()));
        assert_eq!(read.progress().status, FileProposalReadStatus::Failed(error));
        assert_eq!(read.advance(read.progress().replayed_events, 1).err(), Some(error.into()));
        assert_eq!(read.finish().err(), Some(error.into())); assert_eq!(root.files(), files);
    }
}

#[test]
fn valid_equal_counter_fork_and_rollback_refuse_before_any_historical_execution() {
    let root = Root::new(); let (host, expected, anchor) = setup(&root); let files = root.files();
    let mut fork = host.events.clone();
    *fork.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    let fork_machine = Machine::replay(&profile(), &fork).unwrap();
    expected.check(&profile(), &fork_machine, &fork).unwrap();
    assert_eq!(fork_machine.snapshot(fork.len()).control.sequence, host.inspect().control.sequence);
    let path = root.store().join(storage::CANONICAL);
    let bytes = journal::encode(&profile(), host.store.identity(), &fork).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(FileOversight::begin_read_proposal_investigation_anchored(root.store(), &profile(),
        &expected, &anchor, 1, scope()).err(), Some(Error::Binding.into()));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let old = journal::encode(&profile(), host.store.identity(), &host.events[..host.events.len()-1]).unwrap();
    fs::write(&path, &old).unwrap();
    assert_eq!(FileOversight::begin_read_proposal_investigation_anchored(root.store(), &profile(),
        &expected, &anchor, 1, scope()).err(), Some(Error::Stale.into()));
    fs::write(&path, &files[&path]).unwrap();
    let mut read = begin(&root, &expected, &anchor); complete(&mut read, 1); assert!(read.finish().is_ok());
    assert_eq!(root.files(), files);
}

#[test]
fn a_semantically_invalid_suffix_never_exports_its_valid_denied_proposal() {
    let root = Root::new(); let (host, expected, anchor) = setup(&root);
    let mut events = host.events.clone(); events.push(Event::Core(BaseEvent::Publish(999)));
    let error = Machine::replay(&profile(), &events).err().unwrap();
    let path = root.store().join(storage::CANONICAL);
    let original = fs::read(&path).unwrap();
    let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    fs::write(&path, &bytes).unwrap(); let files = root.files();
    let mut read = begin(&root, &expected, &anchor);
    for done in 0..host.events.len() { read.advance(done, 1).unwrap(); }
    assert_eq!(read.advance(host.events.len(), 1).err(), Some(error.into()));
    assert_eq!(read.progress().attempted_events, events.len());
    assert_eq!(read.progress().replayed_events, events.len()-1);
    assert_eq!(read.finish().err(), Some(error.into())); assert_eq!(root.files(), files);
    fs::write(&path, &original).unwrap();
    let mut read = begin(&root, &expected, &anchor); complete(&mut read, 1); assert!(read.finish().is_ok());
}

#[test]
fn an_unwind_after_any_native_event_permanently_disqualifies_the_private_read() {
    let root = Root::new(); let (host, expected, anchor) = setup(&root); let files = root.files();
    for cut in 0..host.events.len() {
        let mut read = begin(&root, &expected, &anchor);
        for done in 0..cut { read.advance(done, 1).unwrap(); }
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read.interrupt_after_event())).is_err());
        assert_eq!(read.progress().status, FileProposalReadStatus::Interrupted);
        assert_eq!(read.progress().attempted_events, cut+1); assert_eq!(read.progress().replayed_events, cut);
        assert_eq!(read.advance(cut, 1).err(), Some(JournalError::Unavailable));
        assert_eq!(read.finish().err(), Some(JournalError::Unavailable)); assert_eq!(root.files(), files);
    }
    let mut read = begin(&root, &expected, &anchor); complete(&mut read, 1); assert!(read.finish().is_ok());
}

#[test]
fn later_writer_progress_never_becomes_the_captured_diagnosis_or_a_new_source_observation() {
    let root = Root::new(); let (mut host, expected, anchor) = setup(&root);
    let original = host.investigate_proposal(1, scope()).unwrap();
    let mut read = begin(&root, &expected, &anchor);
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap(); let later = root.files();
    complete(&mut read, 1); let captured = read.finish().unwrap();
    assert_eq!(captured.origin(), original.origin());
    assert!(captured.origin().journal_snapshot().revision < host.revision());
    assert_eq!(root.files(), later);
    let successor = FileOversight::read_proposal_investigation_anchored(root.store(), &profile(),
        &expected, &anchor, 1, scope()).unwrap();
    assert_eq!(successor.origin().admission_snapshot(), original.origin().admission_snapshot());
    assert_eq!(successor.origin().journal_snapshot(), &host.inspect()); assert_eq!(root.files(), later);
}

#[test]
fn absent_or_invalid_attempts_never_acquire_a_fictional_proposal() {
    let root = Root::new(); let (_host, expected, anchor) = setup(&root); let files = root.files();
    assert_eq!(FileOversight::begin_read_proposal_investigation_anchored(root.store(), &profile(),
        &expected, &anchor, 0, scope()).err(), Some(Error::InvalidInput.into()));
    let mut missing = FileOversight::begin_read_proposal_investigation_anchored(root.store(), &profile(),
        &expected, &anchor, 777, scope()).unwrap();
    assert_eq!(missing.advance(0, MAX_JOURNAL_EVENTS).err(), Some(Error::Missing.into()));
    assert_eq!(missing.finish().err(), Some(Error::Missing.into())); assert_eq!(root.files(), files);
    let mut read = begin(&root, &expected, &anchor); complete(&mut read, 1); assert!(read.finish().is_ok());
}
