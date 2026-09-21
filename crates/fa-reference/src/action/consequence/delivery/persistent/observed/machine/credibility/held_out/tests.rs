//! Actual original two-key reviews and canonical-file publication/recovery.
//! Faults are deterministic storage barriers, not hardware power-cut evidence.
use super::*;
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, CredibilityBinding, CredibilityRequirements, MemberPolicy};
use crate::action::consequence::congress::credibility::{Campaign, CaseSpec, CredibilityLedger,
    EvaluationLabel, EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit,
    JournalLimits, Reconciliation, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit,
    FileHumanReviewer, FileOversightProfile, journal};
use crate::action::consequence::delivery::persistent::observed::publication::{CheckedCompletion, PublicationBasis};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeInput, HelperContract, ReviewWindow};
use crate::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        Self(std::env::temp_dir().join(format!("fa-held-out-{}-{nonce}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.0.join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() { fs::remove_dir_all(&self.0).unwrap(); }
    }
}
fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn contracts(model: u64) -> CommitteeContract {
    CommitteeContract::new(["a", "b"].into_iter().map(|name| (name.to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: b"profile".to_vec(),
            model_epoch: model, tokenizer_epoch: 1, policy_epoch: 0 }, 1, b"approve?".to_vec()).unwrap()
    )).collect()).unwrap()
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile { scope: scope(), total: 100, max_attempts: 64,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::ExactRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::Absent { key: 7 }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: ["a", "b"].into_iter().map(|name|
                (name.to_owned(), MemberPolicy { cohort: name.to_owned(), weight: 8 })).collect(),
                caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 16,
                continue_hold_maximum: 0, narrow_at: 16, suspend_at: 20,
                minimum_members: 2, minimum_cohorts: 2 },
            narrowed_targets: vec![target], target, initial_payload: Vec::new(),
            retention_ticks: 200, max_deliveries: 32, clock_domain: 1, limits: JournalLimits::default(),
        }, committee: contracts(1),
        human: HumanReviewPolicy { reviewer_id: 7, max_validity_ticks: 100, max_requests: 32 },
    }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(), target: Some(host.inspect().target), payload: b"hello".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 5 }
}
fn source(id: u64, text: &[u8]) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 1, generation: id, scope: scope() }, snapshot(),
        ["a", "b"].into_iter().map(|name| (name.to_owned(), text.to_vec())).collect()).unwrap()
}
fn reviewed(host: &mut FileOversight, id: u64) -> (FrozenAction, CommitteeInput) {
    let source = source(id, b"context");
    let action = host.propose(host.revision(), id, spec(host), snapshot()).unwrap();
    let inputs = source.inputs_for(&action, &host.profile.committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    let round = id + 100; let root = source.reference_root();
    host.begin_review(host.revision(), id, round, root,
        ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, snapshot()).unwrap();
    for name in ["a", "b"] {
        let digest = commitment(round, name, &root, Verdict::Allow, b"salt").unwrap();
        host.commit_review(host.revision(), round, name, digest).unwrap();
    }
    host.open_reveals(host.revision(), round).unwrap();
    for name in ["a", "b"] {
        host.reveal_review(host.revision(), round, name, Verdict::Allow, b"salt".to_vec()).unwrap();
    }
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    (action, inputs)
}
fn seed(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for id in [1, 2] { reviewed(host, id); host.cancel(host.revision(), id).unwrap(); }
}
fn ready() -> (Directory, FileOversight, FileHumanReviewer) {
    let root = Directory::new();
    let (mut host, reviewer) = FileOversight::create(&root.0, profile()).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    seed(&mut host);
    (root, host, reviewer)
}
fn keys(host: &mut FileOversight, reviewer: &FileHumanReviewer, id: u64)
    -> (FrozenAction, CommitteeInput, FilePermit, FileHumanPermit)
{
    let (action, inputs) = reviewed(host, id);
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), id, id, &inputs, ElapsedTick(90)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    (action, inputs, automatic, human)
}
fn activation(host: &FileOversight, operation: u64, generation: u64) -> CredibilityActivation {
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: operation, model_generation: 1, evaluator_generation: 1,
            held_out_manifest: [9; 32] }, label_owner: "evaluator".into(),
        helpers: ["a", "b"].into_iter().map(|name| (name.to_owned(), HelperGeneration {
            generation: 1, cohort: name.to_owned() })).collect(),
        strata: BTreeSet::from(["publication".into()]),
        cases: [1, 2].into_iter().map(|id| CaseSpec { id, stratum: "publication".into(),
            evidence_root: [id as u8; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for id in [1, 2] {
        let observation = if id == 1 { Observation::Clear } else { Observation::Hold { first_sequence: 1 } };
        ledger.record_observations(id, ["a", "b"].into_iter().map(|name| (name.to_owned(), observation)).collect()).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "evaluator".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32], recorded_sequence: 2,
            verdict: if id == 1 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let evidence = ledger.seal(2).unwrap();
    CredibilityActivation { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_epoch: host.inspect().control.ledger.epoch, scope: scope(), policy_generation: 1,
        actor_profile: host.profile.delivery.actor.profile(), binding: CredibilityBinding {
            scope: evidence.scope().clone(), label_owner: evidence.label_owner().into(),
            helpers: evidence.helpers().clone(), strata: evidence.strata().clone(), reducer_generation: generation },
        stratum: "publication".into(), requirements: CredibilityRequirements { minimum_safe_cases: 1,
            minimum_violation_cases: 1, minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 10, lead_bonus_weight: 0, lead_saturation_sequences: 0,
            maximum_evidence_age: 100, maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 },
        snapshot: evidence }
}
fn withdrawal(host: &FileOversight, operation: u64) -> CredibilityWithdrawalRequest {
    CredibilityWithdrawalRequest { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_epoch: host.inspect().control.ledger.epoch }
}
fn complete(host: &mut FileOversight, reviewer: &FileHumanReviewer, id: u64) {
    let (action, inputs, automatic, human) = keys(host, reviewer, id);
    let result = host.complete_checked_publication(host.revision(), CheckedCompletion {
        automatic: &automatic, human: &human, action: &action, current: &inputs,
        snapshot: snapshot(), now: ElapsedTick(2),
    }).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
}

#[test]
fn guarded_two_key_publication_reopens_with_a_latched_requirement_for_reactivation() {
    let (root, mut host, reviewer) = ready();
    let request = activation(&host, 20, 2);
    let receipt = host.activate_credibility(host.revision(), request.clone(), &contracts(1)).unwrap();
    complete(&mut host, &reviewer, 10);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 5);
    let bytes = root.bytes();
    assert_eq!(FileOversight::read_publication(&root.0, &profile()).unwrap(), host.inspect());
    assert_eq!(root.bytes(), bytes);
    drop(host);
    let (mut host, reviewer) = FileOversight::open(&root.0, profile()).unwrap();
    assert!(!host.clock_ready());
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    let before = host.inspect(); let bytes = root.bytes();
    assert_eq!(host.activate_credibility(0, request, &contracts(1)).unwrap(), receipt);
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.propose(host.revision(), 11, spec(&host), snapshot()), Err(Error::Stale.into()));
    assert_eq!(host.reconcile(host.revision(), 10).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    let next = activation(&host, 21, 3);
    host.activate_credibility(host.revision(), next, &contracts(1)).unwrap();
    complete(&mut host, &reviewer, 11);
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(host.inspect().control.ledger.charged, 10);
}

#[test]
fn every_withdrawal_io_barrier_closes_the_owner_and_old_disk_evidence_cannot_reopen_it() {
    for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let (root, mut host, reviewer) = ready();
        let request = activation(&host, 20, 2);
        host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
        let (action, inputs, automatic, human) = keys(&mut host, &reviewer, 10);
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        let lost = withdrawal(&host, 21); let before = host.inspect();
        host.store.fail_once(stage);
        let error = host.withdraw_credibility(host.revision(), lost).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("expected injected I/O failure"); };
        assert_eq!(failure.operation, stage);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.check_credibility(), Err(JournalError::Unavailable));
        drop(host);
        let (mut host, _) = FileOversight::open(&root.0, profile()).unwrap();
        assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
        assert_eq!(host.inspect().control.ledger.charged, 5);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert!(host.publish_checked(host.revision(), 10, Some(&inputs), snapshot(), ElapsedTick(2)).is_err());
        assert_eq!(host.reconcile(host.revision(), 10).unwrap(), Reconciliation::AwaitingResolution);
        assert_eq!(host.inspect().control.ledger.charged, 5);
        assert_eq!(host.seal_unexecuted(host.revision(), 10).unwrap(), Reconciliation::Resolved(
            EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn withdrawal_does_not_refund_an_executed_effect_after_its_acknowledgment_is_lost() {
    let (root, mut host, reviewer) = ready();
    let request = activation(&host, 20, 2);
    host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
    let (action, inputs, automatic, human) = keys(&mut host, &reviewer, 10);
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let published = host.publish_checked(host.revision(), 10, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    let lost = withdrawal(&host, 21);
    host.withdraw_credibility(host.revision(), lost).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 5);
    drop(host);
    let (mut host, _) = FileOversight::open(&root.0, profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 10).unwrap(), Reconciliation::Resolved(published.outcome));
    assert_eq!(host.inspect().control.ledger.charged, 5);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn historical_operations_preserve_a_later_reviews_real_human_and_automatic_keys() {
    let (root, mut host, reviewer) = ready();
    let first = activation(&host, 20, 2);
    let receipt = host.activate_credibility(host.revision(), first.clone(), &contracts(1)).unwrap();
    let lost = withdrawal(&host, 21);
    let loss = host.withdraw_credibility(host.revision(), lost.clone()).unwrap();
    let next = activation(&host, 22, 3);
    host.activate_credibility(host.revision(), next, &contracts(1)).unwrap();
    let (action, inputs, automatic, human) = keys(&mut host, &reviewer, 10);
    let before = host.inspect(); let bytes = root.bytes();
    assert_eq!(host.activate_credibility(0, first, &contracts(1)).unwrap(), receipt);
    assert_eq!(host.withdraw_credibility(0, lost).unwrap(), loss);
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
    assert_eq!(host.human_status(10).unwrap().disposition, HumanDisposition::Approved);
    let result = host.complete_checked_publication(host.revision(), CheckedCompletion {
        automatic: &automatic, human: &human, action: &action, current: &inputs,
        snapshot: snapshot(), now: ElapsedTick(2),
    }).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn invalid_bindings_and_loss_predecessors_leave_a_working_owner_unchanged() {
    let (root, mut host, reviewer) = ready();
    let request = activation(&host, 20, 2); let before = host.inspect(); let bytes = root.bytes();
    assert_eq!(host.activate_credibility(host.revision(), request.clone(), &contracts(2)), Err(Error::Binding.into()));
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
    host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
    let mut lost = withdrawal(&host, 21); lost.expected_epoch += 1;
    assert_eq!(host.withdraw_credibility(host.revision(), lost), Err(Error::Stale.into()));
    let mut lost = withdrawal(&host, 21); lost.operation = 0;
    assert_eq!(host.withdraw_credibility(host.revision(), lost), Err(Error::InvalidInput.into()));
    assert_eq!(host.check_credibility(), Ok(()));
    complete(&mut host, &reviewer, 10);
}

#[test]
fn exhausted_ordinary_capacity_cannot_keep_lost_qualification_live_or_spend_recovery_reserve() {
    let root = Directory::new(); let mut profile = profile(); profile.delivery.limits.events = 48;
    let (mut host, _) = FileOversight::create(&root.0, profile.clone()).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    seed(&mut host);
    let request = activation(&host, 20, 2);
    host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
    while host.revision() < 45 { host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); }
    let before = host.inspect(); let bytes = root.bytes();
    let lost = withdrawal(&host, 21);
    assert_eq!(host.withdraw_credibility(host.revision(), lost), Err(Error::Limit.into()));
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
    assert_eq!(host.check_credibility(), Err(JournalError::Unavailable));
    drop(host);
    let (host, _) = FileOversight::open(&root.0, profile).unwrap();
    assert_eq!(host.revision(), 46);
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn no_raw_core_activation_or_unguarded_publication_can_bypass_the_original_two_key_profile() {
    use crate::action::consequence::delivery::persistent::Event as BaseEvent;
    let root = Directory::new();
    let (mut host, _) = FileOversight::create(&root.0, profile()).unwrap(); seed(&mut host);
    let request = activation(&host, 20, 2);
    let before = host.inspect(); let bytes = root.bytes();
    assert_eq!(host.activate_credibility(host.revision(), request.clone(), &contracts(1)), Err(Error::Incomplete.into()));
    assert_eq!(host.inspect(), before); assert_eq!(root.bytes(), bytes);
    assert_eq!(journal::encode(&profile(), &root.0,
        &[Event::Core(BaseEvent::ActivateCredibility(Box::new(request)))]), Err(Error::Binding));
    let (guarded, mut host, reviewer) = ready();
    let request = activation(&host, 20, 2);
    host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
    let (action, inputs, automatic, human) = keys(&mut host, &reviewer, 10);
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 10).is_err());
    assert_eq!(FileOversight::read_publication(&guarded.0, &profile()).unwrap().executions, 0);
    host.publish_checked(host.revision(), 10, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn changed_whole_input_seals_first_publication_without_rereview_or_refund_shortcuts() {
    for changed in [false, true] {
        let (_root, mut host, reviewer) = ready();
        let request = activation(&host, 20, 2);
        host.activate_credibility(host.revision(), request, &contracts(1)).unwrap();
        let (action, inputs, automatic, human) = keys(&mut host, &reviewer, 10);
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        let supplied = if changed { source(11, b"changed").inputs_for(&action, &contracts(1)).unwrap() } else { inputs };
        let published = host.publish_checked(host.revision(), 10, Some(&supplied), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(matches!(published.basis, PublicationBasis::Rejected(_)), changed);
        assert_eq!(host.inspect().control.ledger.charged, 5);
        host.reconcile(host.revision(), 10).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, if changed { 0 } else { 5 });
        assert_eq!(host.inspect().executions, u64::from(!changed));
    }
}

#[test]
fn legacy_unconfigured_recovery_is_unchanged_and_does_not_require_offline_evidence() {
    let (root, host, _) = ready(); drop(host);
    let (mut host, reviewer) = FileOversight::open(&root.0, profile()).unwrap();
    assert_eq!(host.check_credibility(), Ok(()));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    complete(&mut host, &reviewer, 10);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn held_out_subtags_preserve_exact_inputs_and_full_archive_activation_budget() {
    use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
    use crate::action::consequence::delivery::persistent::observed::credibility::{read, write};
    let (root, host, _) = ready();
    let request = activation(&host, 20, 2);
    let event = CredibilityEvent::ActivateHeldOut(Box::new(request.clone()));
    let mut w = Writer::new(8192); write(&mut w, &event).unwrap(); let bytes = w.finish();
    assert_eq!(bytes[0], 3);
    let mut r = Reader::new(&bytes);
    let CredibilityEvent::ActivateHeldOut(decoded) = read(&mut r).unwrap() else { panic!("activation subtag"); };
    r.end().unwrap(); assert_eq!(*decoded, request);
    for end in [0, 1, 4, bytes.len() - 1] { assert!(read(&mut Reader::new(&bytes[..end])).is_err()); }
    let loss = withdrawal(&host, 21);
    let mut expected = vec![4];
    for value in [loss.operation, loss.expected_control_sequence, loss.expected_epoch] { expected.extend_from_slice(&value.to_be_bytes()); }
    let mut w = Writer::new(100); write(&mut w, &CredibilityEvent::WithdrawHeldOut(loss.clone())).unwrap();
    assert_eq!(w.finish(), expected);
    let mut r = Reader::new(&expected);
    let CredibilityEvent::WithdrawHeldOut(decoded) = read(&mut r).unwrap() else { panic!("withdrawal subtag"); };
    r.end().unwrap(); assert_eq!(decoded, loss);
    for end in 0..expected.len() { assert!(read(&mut Reader::new(&expected[..end])).is_err()); }
    // This is encoding/decoding resource admission, not authority replay. Repeated
    // operation IDs are intentionally not represented as accepted activations.
    let event = Event::Credibility(event);
    let mut events = vec![event.clone(); 64];
    let encoded = journal::encode(&profile(), &root.0, &events).unwrap();
    assert_eq!(journal::decode(&profile(), &root.0, &encoded).unwrap().len(), 64);
    events.push(event);
    assert_eq!(journal::encode(&profile(), &root.0, &events), Err(Error::Limit));
    // Assemble a one-over archive independently of the guarded encoder: skip
    // the three length-prefixed bootstrap fields, change only the event count,
    // and append another well-formed credibility frame.
    let mut overflow = encoded;
    let mut offset = 8;
    for _ in 0..3 {
        let length = u32::from_be_bytes(overflow[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4 + length;
    }
    assert_eq!(&overflow[offset..offset + 4], &64_u32.to_be_bytes());
    overflow[offset..offset + 4].copy_from_slice(&65_u32.to_be_bytes());
    let framed_length = bytes.len() + 1;
    overflow.extend_from_slice(&(framed_length as u32).to_be_bytes());
    overflow.push(27); overflow.extend_from_slice(&bytes);
    assert!(matches!(journal::decode(&profile(), &root.0, &overflow), Err(Error::Limit)));
}
