//! Native gate conservation, exact cut binding and monotone containment.
use super::*;
use crate::action::consequence::{DecisionInputs, Restriction};
use crate::action::consequence::gate::ReviewRequest;
use crate::action::consequence::gate::containment::{ActorState, RestartProfile};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget,
    TrustedOutcome, VERSION};
use crate::{Judgment, ReadWitness, Snapshot};
use std::collections::BTreeMap;

fn authority() -> ContainmentAuthority {
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap();
    let mut host = ContainmentAuthority::new(scope(), 20, 8, actor, 3).unwrap();
    host.observe_time(ElapsedTick(1)).unwrap(); host
}
fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target(object: u64) -> ResolvedTarget {
    ResolvedTarget { adapter: 1, object, contract_version: 1, expected_version: 1, generation: 1 }
}
fn action(object: u64, units: u64) -> FrozenAction {
    FrozenAction::freeze(ActionSpec { version: VERSION, scope: scope(), target: Some(target(object)),
        payload: vec![1], required_witnesses: vec![ReadWitness::Exact { key: 7, value: Some(vec![9]) }],
        policy_epoch: 0, deadline: ElapsedTick(100), units }).unwrap()
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 0, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}
fn authorize(host: &mut ContainmentAuthority, id: u64, action: &FrozenAction) -> Permit {
    host.propose(id, action.clone()).unwrap(); host.prepare(id).unwrap(); host.begin_review(id).unwrap();
    host.apply_review(ReviewRequest { attempt: id, expected_control_sequence: host.inspect().sequence,
        action: action.clone(), binding: ReviewBinding { round: id, reducer_generation: 1, evidence_root: [1; 32] },
        inputs: DecisionInputs { empirical: Restriction::Continue, exact_disqualifier: false,
            mandatory_absent: false, contradiction: false }, retained_targets: None }).unwrap();
    let judgment = Judgment::capture(&snapshot(), action.spec().required_witnesses.clone()).unwrap();
    host.authorize(id, &judgment, &snapshot()).unwrap()
}
fn request(host: &ContainmentAuthority, checkpoint: &CheckpointHandle, round: u64,
    targets: &[ResolvedTarget]) -> CheckpointNarrowRequest
{
    CheckpointNarrowRequest { checkpoint: checkpoint.clone(),
        expected_control_sequence: host.inspect().sequence, expected_actor_revision: host.actor_revision(),
        expected_authority_epoch: host.inspect().ledger.epoch,
        binding: ReviewBinding { round, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: TargetCeiling::new(targets).unwrap() }
}

#[test]
fn narrowing_refunds_only_undispatched_reservations_and_preserves_unknown_and_spent() {
    let mut host = authority(); let saved = host.capture_checkpoint(1, 0).unwrap();
    let unknown = action(1, 4); let unknown_key = authorize(&mut host, 1, &unknown);
    host.dispatch(&unknown_key, &unknown, &snapshot()).unwrap(); host.mark_unknown(1).unwrap();
    let reserved = action(1, 5); let reserved_key = authorize(&mut host, 2, &reserved);
    let executed = action(1, 2); let executed_key = authorize(&mut host, 3, &executed);
    host.dispatch(&executed_key, &executed, &snapshot()).unwrap();
    host.record_trusted_outcome(3, TrustedOutcome::Executed).unwrap();
    host.propose(4, action(2, 3)).unwrap();
    let before = host.inspect(); let actor = host.actor().clone();
    let input = request(&host, &saved, 800, &[target(1)]);
    let receipt = host.narrow_checkpoint(input.clone()).unwrap();
    assert_eq!(receipt.cancelled, [2, 4]); assert_eq!(receipt.refunded_units, 5);
    assert_eq!(receipt.sequence, before.sequence + 1);
    assert_eq!(receipt.checkpoint_control_sequence, 0);
    assert_eq!(receipt.revocation_floor, before.ledger.epoch + 1);
    assert_eq!(receipt.consequence, Consequence::NarrowAuthority);
    assert_eq!(receipt.incident_count, 0); assert_eq!(host.actor(), &actor); assert_eq!(host.actor_revision(), 0);
    let after = host.inspect().ledger;
    assert_eq!((after.available, after.reserved, after.charged), (14, 0, 6));
    assert_eq!(after.stages[&1], ActionState::Unknown); assert_eq!(after.stages[&3], ActionState::Confirmed);
    assert_eq!(after.available + after.reserved + after.charged, 20);
    assert!(host.dispatch(&reserved_key, &reserved, &snapshot()).is_err());
    let before_retry = host.inspect();
    assert_eq!(host.narrow_checkpoint(input), Err(Error::Stale));
    let duplicate = request(&host, &saved, 800, &[target(1)]);
    assert_eq!(host.narrow_checkpoint(duplicate), Err(Error::Duplicate));
    assert_eq!(host.inspect(), before_retry);
}

#[test]
fn original_checkpoint_actor_profile_and_every_predecessor_are_required_before_mutation() {
    for changed in 0..8 {
        let mut host = authority(); let saved = host.capture_checkpoint(1, 0).unwrap();
        let mut request = request(&host, &saved, 800, &[target(1)]);
        let expected = match changed {
            0 => { request.expected_control_sequence += 1; Error::Stale },
            1 => { request.expected_actor_revision += 1; Error::Stale },
            2 => { request.expected_authority_epoch += 1; Error::Stale },
            3 => { let mut foreign = authority(); request.checkpoint = foreign.capture_checkpoint(1, 0).unwrap(); Error::Binding },
            4 => { request.binding.round = 0; Error::InvalidInput },
            5 => { request.binding.evidence_root = [0; 32]; Error::InvalidInput },
            6 => { host.actor = ActorState::new(host.actor().profile(), vec![1], vec![9], vec![3], 1).unwrap(); Error::Binding },
            _ => { let mut profile = host.actor().profile(); profile.model_generation += 1;
                host.actor = ActorState::new(profile, vec![1], vec![2], vec![3], 1).unwrap(); Error::Binding },
        };
        let before = host.inspect(); let actor = host.actor().clone();
        assert_eq!(host.narrow_checkpoint(request), Err(expected));
        assert_eq!(host.inspect(), before); assert_eq!(host.actor(), &actor);
        assert_eq!(host.incident_count(), 0); assert!(host.gate.seen_rounds.is_empty());
    }
    let mut host = authority(); let saved = host.capture_checkpoint(1, 0).unwrap();
    let input = request(&host, &saved, 800, &[target(1)]);
    assert_eq!(host.narrow_checkpoint(input).unwrap().sequence, 1);
}

#[test]
fn narrower_current_authority_never_reopens_from_a_larger_request_or_later_reset() {
    let mut host = authority(); let saved = host.capture_checkpoint(1, 0).unwrap();
    let first = request(&host, &saved, 800, &[target(1)]); host.narrow_checkpoint(first).unwrap();
    let second = request(&host, &saved, 801, &[target(1), target(2)]);
    let receipt = host.narrow_checkpoint(second).unwrap();
    assert!(receipt.ceiling.contains(target(1))); assert!(!receipt.ceiling.contains(target(2)));
    let empty = request(&host, &saved, 802, &[]); host.narrow_checkpoint(empty).unwrap();
    let reopen = request(&host, &saved, 803, &[target(1), target(2)]);
    let receipt = host.narrow_checkpoint(reopen).unwrap();
    assert!(!receipt.ceiling.contains(target(1))); assert!(!receipt.ceiling.contains(target(2)));
    let reset = super::super::ResetRequest { checkpoint: saved,
        expected_control_sequence: receipt.sequence, expected_actor_revision: host.actor_revision(),
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: TargetCeiling::new(&[target(1), target(2)]).unwrap() };
    let reset = host.reset(reset).unwrap();
    assert_eq!(reset.incident_count, 1); assert!(!reset.ceiling.contains(target(1)));
    assert!(!reset.ceiling.contains(target(2)));
}

#[test]
fn exhaustion_never_partially_refunds_or_advances_the_native_gate() {
    for exhausted in 0..3 {
        let mut host = authority(); let saved = host.capture_checkpoint(1, 0).unwrap();
        let reserved = action(1, 5); authorize(&mut host, 1, &reserved);
        match exhausted {
            0 => host.gate.sequence = u64::MAX,
            1 => host.gate.authority.rights.epoch = u64::MAX,
            _ => { host.gate.seen_rounds.extend(1..=MAX_DECISIONS as u64); },
        }
        let input = request(&host, &saved, MAX_DECISIONS as u64 + 1, &[target(1)]);
        let before = host.inspect(); let actor = host.actor().clone();
        assert_eq!(host.narrow_checkpoint(input), Err(if exhausted == 2 { Error::Limit } else { Error::Overflow }));
        assert_eq!(host.inspect(), before); assert_eq!(host.actor(), &actor); assert_eq!(host.incident_count(), 0);
    }
}
