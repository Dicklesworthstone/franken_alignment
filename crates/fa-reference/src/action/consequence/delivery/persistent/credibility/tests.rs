use super::*;
use super::super::{FileDeliveryProfile, FilePermit, JournalIo, JournalLimits, ReferenceBallot,
    ReferenceReview, Reconciliation, RecoveryReserve, storage};
use crate::action::consequence::congress::{CongressPolicy, CredibilityBinding, CredibilityRequirements, MemberPolicy};
use crate::action::consequence::congress::credibility::{Campaign, CaseSpec, CredibilityLedger,
    CredibilitySnapshot, EvaluationLabel, EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason, StopRequest};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::reducer::Caps;
use crate::round::Verdict;
use crate::Snapshot;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("fa-credibility-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))))
    }
    fn bytes(&self) -> Vec<u8> { fs::read(self.0.join(storage::CANONICAL)).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() { fs::remove_dir_all(&self.0).unwrap(); }
    }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn spec(epoch: u64) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"hello".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 5 }
}
fn profile() -> FileDeliveryProfile {
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::ExactRestart },
        vec![1], vec![2], vec![3], 1).unwrap();
    FileDeliveryProfile { scope: spec(0).scope, total: 100, max_attempts: 128, actor,
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::Absent { key: 7 }]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([
            ("alice".into(), MemberPolicy { cohort: "a".into(), weight: 8 }),
            ("bob".into(), MemberPolicy { cohort: "b".into(), weight: 8 }),
        ]), caps: Caps { per_member: 10, per_cohort: 10 }, continue_minimum: 16, continue_hold_maximum: 0,
            narrow_at: 16, suspend_at: 20, minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: vec![spec(0).target.unwrap()], target: spec(0).target.unwrap(),
        initial_payload: Vec::new(), retention_ticks: 200, max_deliveries: 128, clock_domain: 1,
        limits: JournalLimits::default() }
}
fn reviewed(host: &mut FileDelivery, id: u64) -> FrozenAction {
    let mut spec = spec(host.inspect().control.ledger.epoch);
    spec.target = Some(host.inspect().target);
    let action = host.propose(host.revision(), id, spec, snapshot()).unwrap();
    host.review(host.revision(), ReferenceReview { attempt: id, round: 100 + id, evidence_root: [8; 32],
        snapshot: snapshot(), ballots: ["alice", "bob"].into_iter().map(|name| (name.to_owned(),
            ReferenceBallot { verdict: Verdict::Allow, salt: b"salt".to_vec() })).collect() }).unwrap();
    action
}
fn authorized(host: &mut FileDelivery, id: u64) -> (FrozenAction, FilePermit) {
    let action = reviewed(host, id);
    let permit = host.authorize(host.revision(), id, snapshot()).unwrap();
    (action, permit)
}
fn send(host: &mut FileDelivery, id: u64) -> (FrozenAction, FilePermit) {
    let (action, key) = authorized(host, id);
    host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
    (action, key)
}
fn seed(host: &mut FileDelivery) {
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    for id in 1..=2 { reviewed(host, id); host.cancel(host.revision(), id).unwrap(); }
}
fn ready() -> (Directory, FileDelivery, FileDeliveryProfile) {
    let directory = Directory::new();
    let profile = profile();
    let mut host = FileDelivery::create(&directory.0, profile.clone()).unwrap();
    seed(&mut host);
    (directory, host, profile)
}
fn evidence(campaign: u64) -> CredibilitySnapshot {
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign, model_generation: 1, evaluator_generation: 1, held_out_manifest: [9; 32] },
        label_owner: "evaluator".into(), helpers: BTreeMap::from([
            ("alice".into(), HelperGeneration { generation: 1, cohort: "a".into() }),
            ("bob".into(), HelperGeneration { generation: 1, cohort: "b".into() }),
        ]), strata: BTreeSet::from(["publication".into()]),
        cases: [1, u64::MAX].into_iter().enumerate().map(|(index, id)| CaseSpec { id, stratum: "publication".into(),
            evidence_root: [index as u8 + 1; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for (index, id) in [1, u64::MAX].into_iter().enumerate() {
        let observation = if index == 0 { Observation::Clear } else { Observation::Hold { first_sequence: 1 } };
        ledger.record_observations(id, ["alice", "bob"].into_iter().map(|name| (name.into(), observation)).collect()).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "evaluator".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [index as u8 + 1; 32], recorded_sequence: 2,
            verdict: if index == 0 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    ledger.seal(2).unwrap()
}
fn activation(host: &FileDelivery, operation: u64, generation: u64) -> CredibilityActivation {
    let snapshot = evidence(operation);
    CredibilityActivation { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_epoch: host.inspect().control.ledger.epoch, scope: host.profile.scope,
        policy_generation: host.current_policy().unwrap().generation(), actor_profile: host.profile.actor.profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(), reducer_generation: generation },
        stratum: "publication".into(), requirements: CredibilityRequirements { minimum_safe_cases: 1,
            minimum_violation_cases: 1, minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 10, lead_bonus_weight: 0, lead_saturation_sequences: 0,
            maximum_evidence_age: 100, maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}
fn withdrawal(host: &FileDelivery, operation: u64) -> CredibilityWithdrawalRequest {
    CredibilityWithdrawalRequest { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_epoch: host.inspect().control.ledger.epoch }
}

#[test]
fn activation_publishes_then_reopens_without_reissuing_keys_or_freshening_evidence() {
    let (directory, mut host, profile) = ready();
    let request = activation(&host, 10, 2);
    let change = host.activate_credibility(host.revision(), request.clone()).unwrap();
    assert_eq!(change.valid_through, 102);
    assert_eq!(host.inspect().dispatcher_epoch, 1);
    let (action, key) = send(&mut host, 10);
    assert_eq!(host.publish(host.revision(), 10).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    let bytes = directory.bytes();
    assert_eq!(FileDelivery::read_publication(&directory.0, &profile).unwrap(), host.inspect());
    assert_eq!(directory.bytes(), bytes); // Reading replays, but performs no write.
    drop(host);
    let mut host = FileDelivery::open(&directory.0, profile).unwrap();
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 5);
    assert_eq!(host.check_credibility(), Ok(()));
    let before = host.inspect();
    let bytes = directory.bytes();
    assert_eq!(host.activate_credibility(0, request).unwrap(), change);
    assert_eq!(host.inspect(), before);
    assert_eq!(directory.bytes(), bytes);
    assert_eq!(host.dispatch(host.revision(), &key, &action, snapshot()), Err(Error::Binding.into()));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 10).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    assert!(host.publish(host.revision(), 10).is_err());
    assert_eq!(host.inspect().control.ledger.charged, 5);
    send(&mut host, 11);
    assert_eq!(host.publish(host.revision(), 11).unwrap(), EndpointOutcome::Executed { resulting_version: 3 });
}

#[test]
fn withdrawal_fences_pending_and_dispatched_work_but_only_receipts_settle() {
    for executed in [false, true] {
        let (directory, mut host, profile) = ready();
        let request = activation(&host, 10, 2);
        host.activate_credibility(host.revision(), request).unwrap();
        send(&mut host, 10);
        if executed { host.publish(host.revision(), 10).unwrap(); }
        let (pending, key) = authorized(&mut host, 11);
        let request = withdrawal(&host, 20);
        let change = host.withdraw_credibility(host.revision(), request.clone()).unwrap();
        assert_eq!(change.cancelled, vec![11]);
        assert_eq!(change.refunded_units, 5);
        assert_eq!(host.inspect().control.ledger.charged, 5);
        assert!(host.dispatch(host.revision(), &key, &pending, snapshot()).is_err());
        assert!(host.publish(host.revision(), 10).is_err());
        drop(host);
        let mut host = FileDelivery::open(&directory.0, profile).unwrap();
        assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
        let before = host.inspect();
        assert_eq!(host.withdraw_credibility(0, request).unwrap(), change);
        assert_eq!(host.inspect(), before);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let outcome = host.reconcile(host.revision(), 10).unwrap();
        if executed {
            assert_eq!(outcome, Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
            assert_eq!(host.inspect().control.ledger.charged, 5);
        } else {
            assert_eq!(outcome, Reconciliation::AwaitingResolution);
            assert_eq!(host.inspect().control.ledger.available, 95);
            assert_eq!(host.seal_unexecuted(host.revision(), 10).unwrap(), Reconciliation::Resolved(
                EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
            assert_eq!(host.inspect().control.ledger.available, 100);
        }
    }
}

#[test]
fn refresh_does_not_make_old_operations_live_again() {
    let (directory, mut host, profile) = ready();
    let first = activation(&host, 10, 2);
    let receipt = host.activate_credibility(host.revision(), first.clone()).unwrap();
    let lost = withdrawal(&host, 20);
    let withdrawal = host.withdraw_credibility(host.revision(), lost.clone()).unwrap();
    assert_eq!(host.activate_credibility(0, first.clone()).unwrap(), receipt);
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    let next = activation(&host, 11, 3);
    let mut weakened = next.clone();
    weakened.requirements.minimum_precision_ppm -= 1;
    let before = directory.bytes();
    assert_eq!(host.activate_credibility(host.revision(), weakened), Err(Error::Binding.into()));
    assert_eq!(directory.bytes(), before);
    host.activate_credibility(host.revision(), next).unwrap();
    drop(host);
    let mut host = FileDelivery::open(&directory.0, profile).unwrap();
    let before = host.inspect();
    let bytes = directory.bytes();
    assert_eq!(host.withdraw_credibility(0, lost).unwrap(), withdrawal);
    assert_eq!(host.activate_credibility(0, first.clone()).unwrap(), receipt);
    assert_eq!(host.inspect(), before);
    assert_eq!(directory.bytes(), bytes);
    assert_eq!(host.check_credibility(), Ok(()));
    let mut conflicting = first;
    conflicting.requirements.maximum_evidence_age += 1;
    assert_eq!(host.activate_credibility(0, conflicting), Err(Error::Binding.into()));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    send(&mut host, 10);
    host.publish(host.revision(), 10).unwrap();
}

#[test]
fn expired_qualification_survives_reopen_and_cannot_authorize_or_dispatch() {
    let (directory, mut host, profile) = ready();
    let mut request = activation(&host, 10, 2);
    request.requirements.maximum_evidence_age = 2;
    let receipt = host.activate_credibility(host.revision(), request.clone()).unwrap();
    assert_eq!(receipt.valid_through, 4);
    let (action, key) = authorized(&mut host, 10); // Review advances 3 -> 4.
    reviewed(&mut host, 11); // Admitted review advances 4 -> 5, but grants no permit.
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    assert_eq!(host.dispatch(host.revision(), &key, &action, snapshot()), Err(Error::Stale.into()));
    assert_eq!(host.authorize(host.revision(), 11, snapshot()).unwrap_err(), Error::Stale.into());
    assert_eq!(host.inspect().control.ledger.reserved, 5);
    drop(host);
    let mut host = FileDelivery::open(&directory.0, profile).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.activate_credibility(0, request).unwrap(), receipt);
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    assert_eq!(host.inspect().control.ledger.available, 100);
    let mut denied = snapshot(); denied.values.insert(7, vec![1]);
    host.propose(host.revision(), 12, spec(host.inspect().control.ledger.epoch), denied).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&12], ActionState::Denied);
}

#[test]
fn unqualified_or_misbound_activation_never_changes_the_file_or_owner() {
    let (directory, mut host, _) = ready();
    let request = activation(&host, 10, 2);
    for lane in 0..7 {
        let mut invalid = request.clone();
        match lane {
            0 => invalid.scope.tenant += 1,
            1 => invalid.actor_profile.tokenizer_generation += 1,
            2 => invalid.expected_epoch += 1,
            3 => invalid.expected_control_sequence += 1,
            4 => invalid.binding.label_owner.push('x'),
            5 => invalid.requirements.minimum_violation_cases += 1,
            _ => invalid.policy_generation += 1,
        }
        let before = host.inspect(); let bytes = directory.bytes();
        assert!(host.activate_credibility(host.revision(), invalid).is_err());
        assert_eq!(host.inspect(), before); assert_eq!(directory.bytes(), bytes);
    }
    host.activate_credibility(host.revision(), request).unwrap();
    send(&mut host, 10); host.publish(host.revision(), 10).unwrap();
}

#[test]
fn every_storage_barrier_keeps_failure_uncertain_and_replays_only_canonical_inputs() {
    for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let (directory, mut host, profile) = ready();
        let request = activation(&host, 10, 2);
        let before = host.inspect();
        host.store.fail_once(stage);
        let error = host.activate_credibility(host.revision(), request.clone()).unwrap_err();
        assert!(matches!(error, JournalError::Io(_)));
        assert_eq!(host.inspect(), before);
        assert_eq!(host.credibility_change(10), Err(JournalError::Unavailable));
        assert_eq!(host.activate_credibility(0, request.clone()), Err(JournalError::Unavailable));
        let visible = FileDelivery::read_publication(&directory.0, &profile).unwrap();
        assert_eq!(visible.control.sequence, before.control.sequence + u64::from(stage == JournalIo::DirectorySync));
        assert_eq!(visible.executions, 0);
        drop(host);
        let mut host = FileDelivery::open(&directory.0, profile).unwrap();
        assert!(!host.clock_ready());
        if stage == JournalIo::DirectorySync {
            let current = host.inspect();
            assert_eq!(host.activate_credibility(0, request).unwrap().reducer_generation, 2);
            assert_eq!(host.inspect(), current);
        } else {
            assert_eq!(host.credibility_change(10), Err(Error::Missing.into()));
            let mut rebound = request;
            rebound.expected_epoch = host.inspect().control.ledger.epoch;
            rebound.expected_control_sequence = host.inspect().control.sequence;
            host.activate_credibility(host.revision(), rebound).unwrap();
        }
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        send(&mut host, 10); host.publish(host.revision(), 10).unwrap();
        assert_eq!(host.inspect().executions, 1);
    }
}

#[test]
fn withdrawal_after_visible_io_failure_recovers_as_withdrawn_not_reenabled() {
    let (directory, mut host, profile) = ready();
    let request = activation(&host, 10, 2);
    host.activate_credibility(host.revision(), request).unwrap();
    send(&mut host, 10);
    let request = withdrawal(&host, 20);
    host.store.fail_once(JournalIo::DirectorySync);
    assert!(host.withdraw_credibility(host.revision(), request.clone()).is_err());
    assert_eq!(host.check_credibility(), Err(JournalError::Unavailable));
    drop(host);
    let mut host = FileDelivery::open(&directory.0, profile).unwrap();
    assert_eq!(host.check_credibility(), Err(Error::Stale.into()));
    assert_eq!(host.inspect().control.ledger.charged, 5);
    let before = host.inspect();
    host.withdraw_credibility(0, request).unwrap();
    assert_eq!(host.inspect(), before);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 10).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 5);
}

#[test]
fn credibility_cannot_spend_reserved_recovery_space_and_retry_needs_no_new_slot() {
    let directory = Directory::new();
    let mut profile = profile(); profile.limits.events = 12;
    let mut host = FileDelivery::create(&directory.0, profile.clone()).unwrap();
    host.enable_recovery_reserve(0, RecoveryReserve::terminal()).unwrap();
    seed(&mut host); // Eight events including the reserve.
    let request = activation(&host, 10, 2);
    let change = host.activate_credibility(host.revision(), request.clone()).unwrap();
    assert_eq!(host.revision(), 9);
    assert_eq!(host.journal_capacity().unwrap().ordinary_remaining().events, 0);
    let lost = withdrawal(&host, 20);
    let before = directory.bytes();
    assert_eq!(host.withdraw_credibility(host.revision(), lost), Err(Error::Limit.into()));
    assert_eq!(directory.bytes(), before);
    assert_eq!(host.activate_credibility(0, request).unwrap(), change);
    host.request_stop(host.revision(), StopRequest { operation: 30,
        expected_control_sequence: host.inspect().control.sequence,
        expected_authority_epoch: host.inspect().control.ledger.epoch }).unwrap();
    host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    drop(host);
    let host = FileDelivery::open(&directory.0, profile).unwrap();
    assert_eq!(host.revision(), 12);
    assert!(host.inspect().control.suspended);
}

#[test]
fn canonical_codec_roundtrips_complete_evidence_and_refuses_all_truncated_prefixes() {
    let (_directory, host, _) = ready();
    let request = activation(&host, 10, 2);
    let bytes = codec::encode_activation(&request).unwrap();
    let restored = codec::decode_activation(&bytes).unwrap();
    assert_eq!(restored, request);
    assert_eq!(restored.snapshot.case_specs().map(|case| case.id).collect::<Vec<_>>(), vec![1, u64::MAX]);
    for end in 0..bytes.len() { assert!(codec::decode_activation(&bytes[..end]).is_err(), "prefix {end}"); }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(codec::decode_activation(&trailing).is_err());
    let mut version = bytes.clone(); version[7] += 1;
    assert_eq!(codec::decode_activation(&version), Err(Error::InvalidInput));
    let mut unsealed = bytes; let end = unsealed.len(); unsealed[end - 8..].copy_from_slice(&1_u64.to_be_bytes());
    assert_eq!(codec::decode_activation(&unsealed), Err(Error::Stale));
}

#[test]
fn imported_duplicate_activation_frames_cannot_repeat_a_native_transition() {
    let (_directory, mut host, profile) = ready();
    let request = activation(&host, 10, 2);
    host.activate_credibility(host.revision(), request).unwrap();
    let mut events = host.events.clone();
    events.push(events.last().unwrap().clone());
    assert!(matches!(Machine::replay(&profile, &events), Err(Error::Duplicate)));
    assert_eq!(host.credibility_change(10).unwrap().reducer_generation, 2);
    assert_eq!(host.machine.broker.controller().credibility_changes().len(), 1);
}
