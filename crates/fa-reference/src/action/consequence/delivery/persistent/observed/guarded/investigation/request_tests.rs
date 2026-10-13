//! Actual durable request allocation, original admission cuts and sealed replay.
//! Requests, attempts and equal numeric IDs must never be interchangeable.
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
        let path = std::env::temp_dir().join(format!("fa-request-diagnosis-{}-{stamp}-{}",
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
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("request diagnosis cleanup: {error}"); } }
}
fn scope() -> InterventionScope { InterventionScope::new(701, true, false, false, &[]).unwrap() }
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
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
            InputProfileBinding { profile_id: 1, profile_bytes: b"request-diagnosis".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn spec(payload: &[u8]) -> ActionSpec {
    let p = profile();
    ActionSpec { version: VERSION, scope: p.delivery.scope, target: Some(p.delivery.target),
        payload: payload.to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
        deadline: ElapsedTick(100), units: 2 }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn setup(root: &Root) -> (FileOversight, FileRecoveryRequirements, FileHistoryAnchor) {
    let (mut host, _) = FileOversight::create_guarded(root.store(), profile(), &guards(), None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.propose(host.revision(), 1, spec(b"prior"), snapshot()).unwrap();
    assert_eq!(host.submit_request(host.revision(), 901, spec(b"blocked"), snapshot()).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 2, stage: ActionState::Denied });
    assert_eq!(host.submit_request(host.revision(), 2, spec(b"ok"), snapshot()).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 3, stage: ActionState::Reviewing });
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let view = host.inspect();
    let expected = FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: view.revision,
            control_sequence: view.control.sequence, authority_epoch: view.control.ledger.epoch } };
    let anchor = host.history_anchor().unwrap();
    fs::write(root.store().join("delivery.pending"), b"preserve original staging").unwrap();
    (host, expected, anchor)
}
fn begin(root: &Root, expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor, request: u64) -> FileProposalRead {
    FileOversight::begin_read_request_proposal_investigation_anchored(
        root.store(), &profile(), expected, anchor, request, scope()).unwrap()
}
fn finish(mut read: FileProposalRead, quantum: usize) -> FileProposalInvestigation {
    while read.progress().status == FileProposalReadStatus::Reading {
        let before = read.progress();
        let after = read.advance(before.replayed_events, quantum).unwrap();
        assert!(after.attempted_events - before.attempted_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    read.finish().unwrap()
}

#[test]
fn external_request_lookup_matches_original_attempt_diagnosis_not_equal_numeric_ids() {
    let root = Root::new(); let (host, expected, anchor) = setup(&root); let files = root.files();
    for (request, attempt, state) in [(901, 2, ActionState::Denied), (2, 3, ActionState::Reviewing)] {
        let original = host.investigate_proposal(attempt, scope()).unwrap();
        for quantum in [1, 2, MAX_JOURNAL_EVENTS] {
            let result = finish(begin(&root, &expected, &anchor, request), quantum);
            assert_eq!(result.origin(), original.origin());
            assert_eq!(result.origin().external_request(), Some(request));
            assert_eq!(result.origin().proposal().attempt, attempt);
            assert_eq!(result.origin().proposal().state, state);
            assert_eq!(result.run(&[]).unwrap().result(), original.run(&[]).unwrap().result());
        }
        let result = FileOversight::read_request_proposal_investigation_anchored(
            root.store(), &profile(), &expected, &anchor, request, scope()).unwrap();
        assert_eq!(result.origin(), original.origin());
    }
    let result = finish(begin(&root, &expected, &anchor, 901), 1);
    let repaired = result.run(&[Intervention::Payload(b"ok".to_vec())]).unwrap();
    assert_eq!(repaired.result().baseline(), Truth::Violated);
    assert_eq!(repaired.result().counterfactual(), Truth::Satisfied);
    assert_eq!(repaired.result().next_requirement(), NextRequirement::FreshIndependentReview);
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Denied);
    assert_eq!(host.inspect().executions, 0); assert_eq!(root.files(), files);
}

#[test]
fn terminal_outcome_and_exact_retry_do_not_replace_the_original_request_admission() {
    let root = Root::new(); let (mut host, expected, anchor) = setup(&root);
    let original = host.investigate_proposal(3, scope()).unwrap();
    host.cancel_request(host.revision(), 2).unwrap();
    let before_retry = host.revision();
    let status = host.submit_request(0, 2, spec(b"ok"), Snapshot::default()).unwrap();
    assert_eq!(status.disposition, FileRequestDisposition::Admitted { attempt: 3, stage: ActionState::Cancelled });
    assert_eq!(host.revision(), before_retry);
    let files = root.files();
    let result = finish(begin(&root, &expected, &anchor, 2), 1);
    assert_eq!(result.origin().admission_snapshot(), original.origin().admission_snapshot());
    assert_eq!(result.origin().proposal(), original.origin().proposal());
    assert_eq!(result.origin().journal_snapshot(), &host.inspect());
    assert_eq!(result.origin().proposal().state, ActionState::Reviewing);
    assert_eq!(result.origin().journal_snapshot().control.ledger.stages[&3], ActionState::Cancelled);
    assert_eq!(root.files(), files);
}

#[test]
fn not_admitted_missing_and_direct_only_keys_never_select_another_proposal() {
    let root = Root::new(); let (mut host, expected, anchor) = setup(&root);
    let rejected = host.submit_request(host.revision(), 3, spec(b"bad"), Snapshot::default()).unwrap();
    assert!(matches!(rejected.disposition, FileRequestDisposition::NotAdmitted(_)));
    let allowed = host.submit_request(host.revision(), 4, spec(b"yes"), snapshot()).unwrap();
    assert!(matches!(allowed.disposition, FileRequestDisposition::Admitted { attempt: 5, .. }));
    let files = root.files();
    // 1 is a direct attempt, 3 is an existing different request's internal
    // attempt, and 777 is absent. None is a proposal for that external key.
    for request in [1, 3, 777] {
        let mut read = begin(&root, &expected, &anchor, request);
        assert_eq!(read.advance(0, MAX_JOURNAL_EVENTS).err(), Some(Error::Missing.into()));
        assert_eq!(read.progress().replayed_events, host.revision() as usize);
        assert_eq!(read.finish().err(), Some(Error::Missing.into()));
    }
    assert_eq!(FileOversight::begin_read_request_proposal_investigation_anchored(
        root.0.join("absent"), &profile(), &expected, &anchor, 0, scope()).err(), Some(Error::InvalidInput.into()));
    assert_eq!(finish(begin(&root, &expected, &anchor, 4), 1).origin().proposal().attempt, 5);
    assert_eq!(root.files(), files);
}

#[test]
fn finding_the_request_never_bypasses_suffix_or_ending_guard_validation() {
    let root = Root::new(); let (host, expected, anchor) = setup(&root);
    let mut wrong = expected.clone(); wrong.minimum.journal_revision = host.revision() + 1;
    let mut read = begin(&root, &wrong, &anchor, 901);
    assert_eq!(read.advance(0, MAX_JOURNAL_EVENTS).err(), Some(Error::Stale.into()));
    assert_eq!(read.finish().err(), Some(Error::Stale.into()));
    let original = root.files();
    let mut events = host.events.clone(); events.push(Event::Core(BaseEvent::Publish(999)));
    let error = Machine::replay(&profile(), &events).err().unwrap();
    let path = root.store().join(storage::CANONICAL);
    fs::write(&path, journal::encode(&profile(), host.store.identity(), &events).unwrap()).unwrap();
    let corrupted = root.files();
    let mut read = begin(&root, &expected, &anchor, 901);
    assert_eq!(read.advance(0, MAX_JOURNAL_EVENTS).err(), Some(error.into()));
    assert_eq!(read.finish().err(), Some(error.into())); assert_eq!(root.files(), corrupted);
    fs::write(&path, &original[&path]).unwrap();
    assert_eq!(finish(begin(&root, &expected, &anchor, 901), 1).origin().proposal().attempt, 2);
    assert_eq!(root.files(), original);
}

#[test]
fn anchored_request_reads_reject_forks_and_preserve_every_cancelled_cut() {
    let root = Root::new(); let (host, expected, anchor) = setup(&root); let files = root.files();
    for cut in 0..=host.events.len() {
        let mut read = begin(&root, &expected, &anchor, 901);
        for done in 0..cut { read.advance(done, 1).unwrap(); }
        let stopped = read.cancel();
        assert_eq!(stopped.replayed_events, cut);
        assert_eq!(stopped.status, FileProposalReadStatus::Cancelled); assert_eq!(root.files(), files);
    }
    let mut fork = host.events.clone(); *fork.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    let machine = Machine::replay(&profile(), &fork).unwrap();
    expected.check(&profile(), &machine, &fork).unwrap();
    let path = root.store().join(storage::CANONICAL);
    fs::write(&path, journal::encode(&profile(), host.store.identity(), &fork).unwrap()).unwrap();
    let forked = root.files();
    assert_eq!(FileOversight::begin_read_request_proposal_investigation_anchored(root.store(), &profile(),
        &expected, &anchor, 901, scope()).err(), Some(Error::Binding.into()));
    assert_eq!(root.files(), forked);
    fs::write(&path, &files[&path]).unwrap();
    assert!(finish(begin(&root, &expected, &anchor, 901), 1).run(&[]).is_ok());
    assert_eq!(root.files(), files);
}
