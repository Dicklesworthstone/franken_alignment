//! Public containment lifecycle tests. These exercise the supplied-state
//! reference host, not an actual inference runtime or authenticated controller.

use fa_reference::action::consequence::gate::containment::{
    ActorState, CheckpointHandle, ContainmentAuthority, MAX_CACHE_BYTES, MAX_CHECKPOINTS,
    RestartGrade, RestartProfile, ResetRequest,
};
use fa_reference::action::consequence::gate::{ReviewBinding, ReviewRequest, TargetCeiling};
use fa_reference::action::consequence::{Consequence, DecisionInputs, Restriction};
use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope,
    TrustedOutcome, VERSION,
};
use fa_reference::{Error, Judgment, Snapshot};
use std::collections::BTreeMap;

fn scope() -> Scope {
    Scope {
        tenant: 1,
        principal: 2,
        run: 3,
        branch: 4,
        authority: 5,
        purpose: Purpose::Effect,
    }
}

fn profile() -> RestartProfile {
    RestartProfile {
        id: 1,
        generation: 1,
        host_generation: 1,
        model_generation: 1,
        tokenizer_generation: 1,
        state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart,
    }
}

fn actor(marker: u8) -> ActorState {
    ActorState::new(
        profile(), vec![u32::from(marker)], vec![marker, 11], vec![marker, 22], 1,
    )
    .unwrap()
}

fn target(object: u64) -> ResolvedTarget {
    ResolvedTarget {
        adapter: 1,
        object,
        contract_version: 1,
        expected_version: 1,
        generation: 1,
    }
}

fn action(epoch: u64, object: u64, units: u64) -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: scope(),
        target: Some(target(object)),
        payload: vec![42],
        required_witnesses: vec![],
        policy_epoch: epoch,
        deadline: ElapsedTick(1_000),
        units,
    })
    .unwrap()
}

fn authority(limit: u64) -> ContainmentAuthority {
    let mut authority = ContainmentAuthority::new(scope(), 100, 32, actor(1), limit).unwrap();
    authority.observe_time(ElapsedTick(10)).unwrap();
    authority
}

fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 0,
        complete: true,
        values: BTreeMap::new(),
    }
}

fn review(gate: &ContainmentAuthority, id: u64, action: &FrozenAction) -> ReviewRequest {
    let sequence = gate.inspect().sequence;
    ReviewRequest {
        attempt: id,
        expected_control_sequence: sequence,
        action: action.clone(),
        binding: ReviewBinding {
            round: sequence + 1,
            evidence_root: [7; 32],
            reducer_generation: 1,
        },
        inputs: DecisionInputs {
            empirical: Restriction::Continue,
            exact_disqualifier: false,
            mandatory_absent: false,
            contradiction: false,
        },
        retained_targets: None,
    }
}

fn authorize(gate: &mut ContainmentAuthority, id: u64, action: &FrozenAction) -> Permit {
    gate.propose(id, action.clone()).unwrap();
    gate.prepare(id).unwrap();
    gate.begin_review(id).unwrap();
    gate.apply_review(review(gate, id, action)).unwrap();
    let judgment = Judgment::capture(&snapshot(), vec![]).unwrap();
    gate.authorize(id, &judgment, &snapshot()).unwrap()
}

fn reset_request(gate: &ContainmentAuthority, checkpoint: &CheckpointHandle) -> ResetRequest {
    let sequence = gate.inspect().sequence;
    ResetRequest {
        checkpoint: checkpoint.clone(),
        expected_control_sequence: sequence,
        expected_actor_revision: gate.actor_revision(),
        binding: ReviewBinding {
            round: sequence + 1,
            evidence_root: [8; 32],
            reducer_generation: 1,
        },
        retained_targets: TargetCeiling::new(&[target(1)]).unwrap(),
    }
}

fn conserved(gate: &ContainmentAuthority) {
    let ledger = gate.inspect().ledger;
    assert_eq!(ledger.available + ledger.reserved + ledger.charged, 100);
}

#[test]
fn rewind_restores_actor_only_and_preserves_every_effect_disposition() {
    let mut gate = authority(3);
    let initial = gate.actor().clone();
    let checkpoint = gate.capture_checkpoint(1, 0).unwrap();
    let reserved = action(0, 1, 10);
    let dispatched = action(0, 1, 20);
    let executed = action(0, 1, 30);
    let unknown = action(0, 1, 5);
    let irrecoverable = action(0, 1, 6);
    let old_permit = authorize(&mut gate, 1, &reserved);
    let dispatched_permit = authorize(&mut gate, 2, &dispatched);
    let executed_permit = authorize(&mut gate, 3, &executed);
    let unknown_permit = authorize(&mut gate, 4, &unknown);
    let irrecoverable_permit = authorize(&mut gate, 5, &irrecoverable);
    for (permit, action) in [
        (&dispatched_permit, &dispatched),
        (&executed_permit, &executed),
        (&unknown_permit, &unknown),
        (&irrecoverable_permit, &irrecoverable),
    ] {
        gate.dispatch(permit, action, &snapshot()).unwrap();
    }
    gate.record_trusted_outcome(3, TrustedOutcome::Executed).unwrap();
    gate.mark_unknown(4).unwrap();
    gate.mark_unknown(5).unwrap();
    gate.mark_irrecoverable(5).unwrap();
    for id in [6, 7, 8] {
        gate.propose(id, reserved.clone()).unwrap();
    }
    gate.prepare(7).unwrap();
    gate.prepare(8).unwrap();
    gate.begin_review(8).unwrap();
    gate.replace_actor_state(0, actor(99)).unwrap();
    gate.revoke_epoch().unwrap();
    gate.observe_time(ElapsedTick(50)).unwrap();
    let receipt = gate.reset(reset_request(&gate, &checkpoint)).unwrap();
    assert_eq!(receipt.consequence, Consequence::ResetToCheckpoint);
    assert!(receipt.restored);
    assert_eq!(gate.actor(), &initial);
    assert_eq!(receipt.cancelled, vec![1, 6, 7, 8]);
    assert_eq!(receipt.refunded_units, 10);
    assert_eq!(receipt.incident_count, 1);
    assert_eq!(receipt.revocation_floor, 2);
    assert_eq!(receipt.actor_revision, 2);
    let ledger = gate.inspect().ledger;
    assert_eq!(ledger.available, 39);
    assert_eq!(ledger.reserved, 0);
    assert_eq!(ledger.charged, 61);
    assert_eq!(ledger.elapsed, Some(ElapsedTick(50)));
    assert_eq!(ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(ledger.stages[&2], ActionState::Dispatching);
    assert_eq!(ledger.stages[&3], ActionState::Confirmed);
    assert_eq!(ledger.stages[&4], ActionState::Unknown);
    assert_eq!(ledger.stages[&5], ActionState::IrrecoverablyUnknown);
    assert!(gate.dispatch(&old_permit, &reserved, &snapshot()).is_err());
    assert_eq!(gate.cancel(4), Err(Error::WrongState));
    assert_eq!(gate.cancel(5), Err(Error::WrongState));
    gate.record_trusted_outcome(4, TrustedOutcome::NotExecuted).unwrap();
    assert_eq!(gate.inspect().ledger.available, 44);
    conserved(&gate);
}

#[test]
fn resumed_actor_needs_fresh_attempt_epoch_review_and_one_use_permit() {
    let mut gate = authority(3);
    let checkpoint = gate.capture_checkpoint(1, 0).unwrap();
    let old = action(0, 1, 10);
    let old_permit = authorize(&mut gate, 1, &old);
    let old_review = review(&gate, 1, &old);
    gate.reset(reset_request(&gate, &checkpoint)).unwrap();
    assert!(gate.dispatch(&old_permit, &old, &snapshot()).is_err());
    assert_eq!(gate.apply_review(old_review), Err(Error::Stale));
    gate.propose(2, old).unwrap();
    assert_eq!(gate.prepare(2), Err(Error::Stale));
    let fresh = action(gate.inspect().ledger.epoch, 1, 10);
    gate.propose(3, fresh.clone()).unwrap();
    gate.prepare(3).unwrap();
    gate.begin_review(3).unwrap();
    let judgment = Judgment::capture(&snapshot(), vec![]).unwrap();
    assert_eq!(gate.authorize(3, &judgment, &snapshot()).unwrap_err(), Error::Incomplete);
    gate.apply_review(review(&gate, 3, &fresh)).unwrap();
    let permit = gate.authorize(3, &judgment, &snapshot()).unwrap();
    gate.dispatch(&permit, &fresh, &snapshot()).unwrap();
    assert_eq!(gate.dispatch(&permit, &fresh, &snapshot()), Err(Error::WrongState));
    conserved(&gate);
}

#[test]
fn repeated_resets_escalate_without_restoring_actor_or_blocking_reconciliation() {
    let mut gate = authority(3);
    let checkpoint = gate.capture_checkpoint(1, 0).unwrap();
    let pending = action(0, 1, 7);
    let permit = authorize(&mut gate, 1, &pending);
    gate.dispatch(&permit, &pending, &snapshot()).unwrap();
    gate.mark_unknown(1).unwrap();
    for incident in 1..=2 {
        gate.replace_actor_state(gate.actor_revision(), actor(90)).unwrap();
        let receipt = gate.reset(reset_request(&gate, &checkpoint)).unwrap();
        assert!(receipt.restored);
        assert_eq!(gate.actor(), &actor(1));
        assert_eq!(receipt.incident_count, incident);
        assert_eq!(gate.inspect().ledger.charged, 7);
        conserved(&gate);
    }
    gate.replace_actor_state(gate.actor_revision(), actor(99)).unwrap();
    let receipt = gate.reset(reset_request(&gate, &checkpoint)).unwrap();
    assert_eq!(receipt.consequence, Consequence::SuspendRun);
    assert!(!receipt.restored);
    assert_eq!(gate.actor(), &actor(99));
    assert_eq!(gate.incident_count(), 3);
    assert!(gate.inspect().suspended);
    assert_eq!(gate.inspect().ledger.epoch, 3);
    assert_eq!(gate.reset(reset_request(&gate, &checkpoint)), Err(Error::WrongState));
    assert_eq!(gate.propose(2, action(3, 1, 1)), Err(Error::WrongState));
    assert_eq!(gate.replace_actor_state(gate.actor_revision(), actor(1)), Err(Error::WrongState));
    gate.record_trusted_outcome(1, TrustedOutcome::Executed).unwrap();
    assert_eq!(gate.inspect().ledger.charged, 7);
    assert_eq!(gate.inspect().ledger.available, 93);
    assert_eq!(gate.reset_receipts().len(), 3);
    conserved(&gate);
}

#[test]
fn stale_actor_updates_and_reset_requests_are_atomic() {
    let mut gate = authority(3);
    let checkpoint = gate.capture_checkpoint(1, 0).unwrap();
    let stale = reset_request(&gate, &checkpoint);
    gate.replace_actor_state(0, actor(2)).unwrap();
    let before = gate.inspect();
    assert_eq!(gate.reset(stale), Err(Error::Stale));
    assert_eq!(gate.actor(), &actor(2));
    assert_eq!(gate.inspect(), before);
    assert_eq!(gate.incident_count(), 0);
    gate.reset(reset_request(&gate, &checkpoint)).unwrap();
    assert_eq!(gate.replace_actor_state(1, actor(3)), Err(Error::Stale));
    assert_eq!(gate.actor(), &actor(1));
    assert_eq!(gate.actor_revision(), 2);
}

#[test]
fn checkpoint_from_another_controller_is_not_a_same_number_alias() {
    let mut first = authority(3);
    let mut second = authority(3);
    let foreign = first.capture_checkpoint(1, 0).unwrap();
    let local = second.capture_checkpoint(1, 0).unwrap();
    let before = second.inspect();
    assert_eq!(second.reset(reset_request(&second, &foreign)), Err(Error::Binding));
    assert_eq!(second.inspect(), before);
    assert_eq!(second.incident_count(), 0);
    assert!(second.reset_receipts().is_empty());
    assert!(second.reset(reset_request(&second, &local)).unwrap().restored);
}

#[test]
fn reset_cannot_widen_a_previously_narrowed_target_ceiling() {
    let mut gate = authority(4);
    let checkpoint = gate.capture_checkpoint(1, 0).unwrap();
    gate.reset(reset_request(&gate, &checkpoint)).unwrap();
    let mut request = reset_request(&gate, &checkpoint);
    request.retained_targets = TargetCeiling::new(&[target(1), target(2)]).unwrap();
    gate.reset(request).unwrap();
    let epoch = gate.inspect().ledger.epoch;
    assert_eq!(gate.propose(1, action(epoch, 2, 1)), Err(Error::Binding));
    let allowed = action(epoch, 1, 1);
    let permit = authorize(&mut gate, 2, &allowed);
    gate.dispatch(&permit, &allowed, &snapshot()).unwrap();
    conserved(&gate);
}

#[test]
fn duplicate_round_and_invalid_evidence_do_not_increment_incidents() {
    let mut gate = authority(4);
    let checkpoint = gate.capture_checkpoint(1, 0).unwrap();
    let mut invalid = reset_request(&gate, &checkpoint);
    invalid.binding.evidence_root = [0; 32];
    let before = gate.inspect();
    assert_eq!(gate.reset(invalid), Err(Error::InvalidInput));
    assert_eq!(gate.inspect(), before);
    let request = reset_request(&gate, &checkpoint);
    gate.reset(request.clone()).unwrap();
    let before = gate.inspect();
    let mut replayed = request;
    replayed.expected_control_sequence = before.sequence;
    replayed.expected_actor_revision = gate.actor_revision();
    assert_eq!(gate.reset(replayed), Err(Error::Duplicate));
    assert_eq!(gate.inspect(), before);
    assert_eq!(gate.incident_count(), 1);
    assert_eq!(gate.reset_receipts().len(), 1);
}

#[test]
fn restart_qualification_and_profile_identity_are_not_inferred_from_byte_shape() {
    let mut audit = profile();
    audit.grade = RestartGrade::AuditOnly;
    let state = ActorState::new(audit, vec![1], vec![2], vec![3], 1).unwrap();
    let mut gate = ContainmentAuthority::new(scope(), 100, 32, state, 3).unwrap();
    assert_eq!(gate.capture_checkpoint(1, 0).unwrap_err(), Error::Incomplete);
    let mut gate = authority(3);
    for changed in 0..6 {
        let mut profile = profile();
        match changed {
            0 => profile.id += 1,
            1 => profile.generation += 1,
            2 => profile.host_generation += 1,
            3 => profile.model_generation += 1,
            4 => profile.tokenizer_generation += 1,
            _ => profile.state_schema_generation += 1,
        }
        let state = ActorState::new(profile, vec![1], vec![2], vec![3], 1).unwrap();
        assert_eq!(gate.replace_actor_state(0, state), Err(Error::Binding));
        assert_eq!(gate.actor_revision(), 0);
    }
    gate.capture_checkpoint(1, 0).unwrap();
}

#[test]
fn checkpoint_count_and_aggregate_byte_bounds_keep_existing_checkpoints_usable() {
    let mut gate = authority(3);
    let first = gate.capture_checkpoint(1, 0).unwrap();
    for id in 2..=MAX_CHECKPOINTS as u64 {
        gate.capture_checkpoint(id, 0).unwrap();
    }
    assert_eq!(
        gate.capture_checkpoint(MAX_CHECKPOINTS as u64 + 1, 0).unwrap_err(),
        Error::Limit
    );
    gate.reset(reset_request(&gate, &first)).unwrap();
    let state = ActorState::new(
        profile(), vec![], vec![1; MAX_CACHE_BYTES - 1], vec![1], 0,
    )
    .unwrap();
    let mut gate = ContainmentAuthority::new(scope(), 100, 32, state, 3).unwrap();
    let first = gate.capture_checkpoint(1, 0).unwrap();
    for id in 2..=8 {
        gate.capture_checkpoint(id, 0).unwrap();
    }
    assert_eq!(gate.capture_checkpoint(9, 0).unwrap_err(), Error::Limit);
    gate.reset(reset_request(&gate, &first)).unwrap();
    assert_eq!(gate.actor().cache().len(), MAX_CACHE_BYTES - 1);
}
