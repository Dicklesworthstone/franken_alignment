//! Public-boundary tests for FA-001's in-memory action/permit reference model.
//!
//! These exercise typed values and lifecycle calls available to an engine
//! consumer.  They do not claim a broker, cryptographic issuer, durable clock,
//! remote dispatch, or Asupersync context separation.

use std::collections::BTreeMap;

use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, MAX_ATTEMPTS, MAX_PAYLOAD_BYTES,
    MAX_REQUIRED_WITNESSES, MAX_WITNESS_BYTES, Purpose, ReferenceAuthority, ResolvedTarget, Scope,
    TrustedOutcome, VERSION,
};
use fa_reference::{Error, Judgment, ReadWitness, Snapshot};

fn effect_scope() -> Scope {
    Scope {
        tenant: 11,
        principal: 12,
        run: 13,
        branch: 14,
        authority: 15,
        purpose: Purpose::Effect,
    }
}

fn exact_action() -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: effect_scope(),
        target: Some(ResolvedTarget {
            adapter: 21,
            object: 22,
            contract_version: 23,
            expected_version: 24,
            generation: 25,
        }),
        payload: b"the exact payload".to_vec(),
        required_witnesses: required_witnesses(),
        policy_epoch: 0,
        deadline: ElapsedTick(10),
        units: 3,
    })
    .unwrap()
}

fn required_witnesses() -> Vec<ReadWitness> {
    vec![ReadWitness::Exact {
        key: 99,
        value: Some(b"authorized precondition".to_vec()),
    }]
}

fn exact_snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 7,
        complete: true,
        values: BTreeMap::from([(99, b"authorized precondition".to_vec())]),
    }
}

fn exact_judgment(snapshot: &Snapshot) -> Judgment {
    Judgment::capture(snapshot, required_witnesses()).unwrap()
}

fn authorized_attempt(
    id: u64,
    total: u64,
) -> (
    ReferenceAuthority,
    FrozenAction,
    Snapshot,
    fa_reference::action::Permit,
) {
    let action = exact_action();
    let snapshot = exact_snapshot();
    let judgment = exact_judgment(&snapshot);
    let mut authority = ReferenceAuthority::new(effect_scope(), total, 8).unwrap();
    authority.observe_time(ElapsedTick(1)).unwrap();
    authority.propose(id, action.clone()).unwrap();
    authority.prepare(id).unwrap();
    authority.begin_review(id).unwrap();
    let permit = authority.authorize(id, &judgment, &snapshot).unwrap();
    (authority, action, snapshot, permit)
}

#[test]
fn action_permit_public_path_dispatches_once_and_keeps_rights_conserved() {
    let (mut authority, action, snapshot, permit) = authorized_attempt(1, 7);

    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Authorized)
    );
    assert_eq!(authority.inspect().available, 4);
    assert_eq!(authority.inspect().reserved, 3);
    assert_eq!(authority.inspect().charged, 0);

    authority.dispatch(&permit, &action, &snapshot).unwrap();
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Dispatching)
    );
    assert_eq!(authority.inspect().available, 4);
    assert_eq!(authority.inspect().reserved, 0);
    assert_eq!(authority.inspect().charged, 3);

    // The same opaque permit can be borrowed again, but authority-side state
    // makes that second dispatch a causal refusal rather than a second spend.
    let after_first_dispatch = authority.inspect();
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot),
        Err(Error::WrongState)
    );
    assert_eq!(authority.inspect(), after_first_dispatch);

    authority
        .record_trusted_outcome(1, TrustedOutcome::Executed)
        .unwrap();
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Confirmed)
    );
    assert_eq!(authority.inspect().available, 4);
    assert_eq!(authority.inspect().charged, 3);
}

type SpecMutation = (&'static str, fn(&mut ActionSpec));
type ScopeMutation = (&'static str, fn(&mut Scope));

fn mutate_tenant(spec: &mut ActionSpec) {
    spec.scope.tenant += 1;
}
fn mutate_principal(spec: &mut ActionSpec) {
    spec.scope.principal += 1;
}
fn mutate_run(spec: &mut ActionSpec) {
    spec.scope.run += 1;
}
fn mutate_branch(spec: &mut ActionSpec) {
    spec.scope.branch += 1;
}
fn mutate_authority(spec: &mut ActionSpec) {
    spec.scope.authority += 1;
}
fn mutate_purpose(spec: &mut ActionSpec) {
    spec.scope.purpose = Purpose::Experiment;
}
fn mutate_adapter(spec: &mut ActionSpec) {
    spec.target.as_mut().unwrap().adapter += 1;
}
fn mutate_object(spec: &mut ActionSpec) {
    spec.target.as_mut().unwrap().object += 1;
}
fn mutate_contract_version(spec: &mut ActionSpec) {
    spec.target.as_mut().unwrap().contract_version += 1;
}
fn mutate_expected_version(spec: &mut ActionSpec) {
    spec.target.as_mut().unwrap().expected_version += 1;
}
fn mutate_generation(spec: &mut ActionSpec) {
    spec.target.as_mut().unwrap().generation += 1;
}
fn mutate_payload(spec: &mut ActionSpec) {
    spec.payload[0] ^= 1;
}
fn mutate_required_witness(spec: &mut ActionSpec) {
    spec.required_witnesses[0] = ReadWitness::Exact {
        key: 100,
        value: None,
    };
}
fn mutate_policy_epoch(spec: &mut ActionSpec) {
    spec.policy_epoch += 1;
}
fn mutate_deadline(spec: &mut ActionSpec) {
    spec.deadline = ElapsedTick(11);
}
fn mutate_units(spec: &mut ActionSpec) {
    spec.units += 1;
}

#[test]
fn every_exact_action_binding_component_refuses_a_near_identical_final_view() {
    let mutations: [SpecMutation; 16] = [
        ("tenant", mutate_tenant),
        ("principal", mutate_principal),
        ("run", mutate_run),
        ("branch", mutate_branch),
        ("authority", mutate_authority),
        ("purpose", mutate_purpose),
        ("target adapter", mutate_adapter),
        ("target object", mutate_object),
        ("adapter contract version", mutate_contract_version),
        ("resource expected version", mutate_expected_version),
        ("target generation", mutate_generation),
        ("payload", mutate_payload),
        ("required witness", mutate_required_witness),
        ("policy epoch", mutate_policy_epoch),
        ("deadline", mutate_deadline),
        ("units", mutate_units),
    ];

    for (name, mutate) in mutations {
        let (mut authority, action, snapshot, permit) = authorized_attempt(1, 7);
        let mut changed_spec = action.spec().clone();
        mutate(&mut changed_spec);
        let changed = FrozenAction::freeze(changed_spec).unwrap();
        let before = authority.inspect();

        // Each case changes just one component of the final view. It must fail
        // before dispatch/spend, and the original view must still dispatch.
        assert_eq!(
            authority.dispatch(&permit, &changed, &snapshot),
            Err(Error::Binding),
            "accepted changed {name}"
        );
        assert_eq!(
            authority.inspect(),
            before,
            "mutation changed state: {name}"
        );
        authority.dispatch(&permit, &action, &snapshot).unwrap();
    }
}

#[test]
fn freeze_refuses_unknown_versions_zero_identities_and_oversized_payloads() {
    // Near-identical positive control: the fully versioned, resolved action
    // freezes successfully before it can enter an authority lifecycle.
    let valid = exact_action();
    assert_eq!(valid.spec().version, VERSION);

    let mut wrong_version = valid.spec().clone();
    wrong_version.version = VERSION + 1;
    assert_eq!(
        FrozenAction::freeze(wrong_version),
        Err(Error::InvalidInput)
    );

    let zeroes: [SpecMutation; 10] = [
        ("tenant", |spec: &mut ActionSpec| spec.scope.tenant = 0),
        ("principal", |spec: &mut ActionSpec| {
            spec.scope.principal = 0
        }),
        ("run", |spec: &mut ActionSpec| spec.scope.run = 0),
        ("branch", |spec: &mut ActionSpec| spec.scope.branch = 0),
        ("authority", |spec: &mut ActionSpec| {
            spec.scope.authority = 0
        }),
        ("adapter", |spec: &mut ActionSpec| {
            spec.target.as_mut().unwrap().adapter = 0
        }),
        ("object", |spec: &mut ActionSpec| {
            spec.target.as_mut().unwrap().object = 0
        }),
        ("adapter contract version", |spec: &mut ActionSpec| {
            spec.target.as_mut().unwrap().contract_version = 0
        }),
        ("expected resource version", |spec: &mut ActionSpec| {
            spec.target.as_mut().unwrap().expected_version = 0
        }),
        ("target generation", |spec: &mut ActionSpec| {
            spec.target.as_mut().unwrap().generation = 0
        }),
    ];
    for (name, zero) in zeroes {
        let mut malformed = valid.spec().clone();
        zero(&mut malformed);
        assert_eq!(
            FrozenAction::freeze(malformed),
            Err(Error::InvalidInput),
            "accepted zero {name}"
        );
    }

    let mut oversized_payload = valid.spec().clone();
    oversized_payload.payload = vec![0; MAX_PAYLOAD_BYTES + 1];
    assert_eq!(FrozenAction::freeze(oversized_payload), Err(Error::Limit));
}

#[test]
fn freeze_bounds_declared_witness_count_bytes_and_range_shape() {
    let valid = exact_action();

    let mut at_witness_count = valid.spec().clone();
    at_witness_count.required_witnesses = (0..MAX_REQUIRED_WITNESSES)
        .map(|key| ReadWitness::Exact {
            key: key as u64 + 1,
            value: None,
        })
        .collect();
    assert!(FrozenAction::freeze(at_witness_count.clone()).is_ok());
    at_witness_count
        .required_witnesses
        .push(ReadWitness::Exact {
            key: 9_999,
            value: None,
        });
    assert_eq!(FrozenAction::freeze(at_witness_count), Err(Error::Limit));

    let mut at_witness_bytes = valid.spec().clone();
    at_witness_bytes.required_witnesses = vec![ReadWitness::Exact {
        key: 1,
        value: Some(vec![0; MAX_WITNESS_BYTES]),
    }];
    assert!(FrozenAction::freeze(at_witness_bytes.clone()).is_ok());
    at_witness_bytes.required_witnesses[0] = ReadWitness::Exact {
        key: 1,
        value: Some(vec![0; MAX_WITNESS_BYTES + 1]),
    };
    assert_eq!(FrozenAction::freeze(at_witness_bytes), Err(Error::Limit));

    // Each value fits separately; the limit also applies across witnesses.
    let mut aggregate_bytes = valid.spec().clone();
    aggregate_bytes.required_witnesses = vec![
        ReadWitness::Exact {
            key: 1,
            value: Some(vec![0; MAX_WITNESS_BYTES / 2]),
        },
        ReadWitness::Exact {
            key: 2,
            value: Some(vec![0; MAX_WITNESS_BYTES / 2]),
        },
    ];
    assert!(FrozenAction::freeze(aggregate_bytes.clone()).is_ok());
    if let ReadWitness::Exact {
        value: Some(value), ..
    } = &mut aggregate_bytes.required_witnesses[1]
    {
        value.push(0);
    } else {
        panic!("test witness must contain a value");
    }
    assert_eq!(FrozenAction::freeze(aggregate_bytes), Err(Error::Limit));

    let mut valid_range = valid.spec().clone();
    valid_range.required_witnesses = vec![ReadWitness::EmptyRange { start: 1, end: 2 }];
    assert!(FrozenAction::freeze(valid_range).is_ok());
    for (name, witness) in [
        ("empty", ReadWitness::EmptyRange { start: 1, end: 1 }),
        ("reversed", ReadWitness::EmptyRange { start: 2, end: 1 }),
    ] {
        let mut malformed_range = valid.spec().clone();
        malformed_range.required_witnesses = vec![witness];
        assert_eq!(
            FrozenAction::freeze(malformed_range),
            Err(Error::InvalidInput),
            "accepted {name} range"
        );
    }
}

#[test]
fn absent_target_and_wrong_scope_refuse_before_any_authority_can_reserve_rights() {
    let mut missing_target = exact_action().spec().clone();
    missing_target.target = None;
    assert_eq!(
        FrozenAction::freeze(missing_target),
        Err(Error::Incomplete),
        "an unresolved target cannot become an authorizable action"
    );

    // Near-identical positive control: a resolved target can enter the public
    // lifecycle and reserve exactly its declared rights.
    let (authority, _, _, _) = authorized_attempt(1, 7);
    assert_eq!(authority.inspect().available, 4);
    assert_eq!(authority.inspect().reserved, 3);

    let wrong_scopes: [ScopeMutation; 2] = [
        ("tenant", |scope: &mut Scope| scope.tenant += 1),
        ("purpose", |scope: &mut Scope| {
            scope.purpose = Purpose::Experiment
        }),
    ];
    for (name, change_scope) in wrong_scopes {
        let mut wrong_scope = exact_action().spec().clone();
        change_scope(&mut wrong_scope.scope);
        let wrong_scope = FrozenAction::freeze(wrong_scope).unwrap();
        let mut authority = ReferenceAuthority::new(effect_scope(), 7, 8).unwrap();
        authority.observe_time(ElapsedTick(1)).unwrap();
        authority.propose(1, wrong_scope).unwrap();
        let before_prepare = authority.inspect();
        assert_eq!(authority.prepare(1), Err(Error::Binding), "accepted {name}");
        assert_eq!(
            authority.inspect(),
            before_prepare,
            "changed state for {name}"
        );
        assert_eq!(
            authority.inspect().stages.get(&1),
            Some(&ActionState::Proposed)
        );
    }
}

#[test]
fn caller_clock_is_monotonic_and_an_observed_deadline_prevents_effects() {
    let action = exact_action();
    let snapshot = exact_snapshot();
    let judgment = exact_judgment(&snapshot);
    let mut authority = ReferenceAuthority::new(effect_scope(), 7, 8).unwrap();

    authority.observe_time(ElapsedTick(5)).unwrap();
    let before_rollback = authority.inspect();
    assert_eq!(authority.observe_time(ElapsedTick(4)), Err(Error::Stale));
    assert_eq!(authority.inspect(), before_rollback);

    // At the exact deadline `now < deadline` is false: even authorization has
    // no reservation side effect.
    authority.observe_time(ElapsedTick(10)).unwrap();
    authority.propose(1, action).unwrap();
    let before_prepare = authority.inspect();
    assert_eq!(authority.prepare(1), Err(Error::Stale));
    assert_eq!(authority.inspect(), before_prepare);
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Proposed)
    );
    assert_eq!(authority.inspect().available, 7);
    assert_eq!(authority.inspect().reserved, 0);
    assert_eq!(authority.inspect().charged, 0);

    // A valid captured judgment does not override an observed expired deadline.
    assert!(judgment.valid_at(&snapshot).unwrap());
}

#[test]
fn authority_requires_an_explicit_clock_observation_before_preparation() {
    let action = exact_action();
    let snapshot = exact_snapshot();
    let judgment = exact_judgment(&snapshot);
    let mut authority = ReferenceAuthority::new(effect_scope(), 7, 8).unwrap();
    authority.propose(1, action.clone()).unwrap();

    // An unobserved clock is unknown, not implicitly elapsed tick zero. This
    // refusal must leave the proposed action and all rights untouched.
    let before_clock_observation = authority.inspect();
    assert_eq!(authority.prepare(1), Err(Error::Incomplete));
    assert_eq!(authority.inspect(), before_clock_observation);
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Proposed)
    );
    assert_eq!(authority.inspect().available, 7);
    assert_eq!(authority.inspect().reserved, 0);
    assert_eq!(authority.inspect().charged, 0);

    // An explicit zero observation is distinct from absent time and admits the
    // otherwise identical action through its ordinary public lifecycle.
    authority.observe_time(ElapsedTick(0)).unwrap();
    authority.prepare(1).unwrap();
    authority.begin_review(1).unwrap();
    let permit = authority.authorize(1, &judgment, &snapshot).unwrap();
    authority.dispatch(&permit, &action, &snapshot).unwrap();
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Dispatching)
    );
}

#[test]
fn stale_epoch_and_expiry_refuse_dispatch_without_changing_the_reservation() {
    let (mut authority, action, snapshot, permit) = authorized_attempt(1, 7);
    authority.revoke_epoch().unwrap();
    let after_revocation = authority.inspect();
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot),
        Err(Error::Stale)
    );
    assert_eq!(authority.inspect(), after_revocation);
    authority.cancel(1).unwrap();
    assert_eq!(authority.inspect().available, 7);
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Cancelled)
    );

    let (mut authority, action, snapshot, permit) = authorized_attempt(1, 7);
    authority.observe_time(ElapsedTick(10)).unwrap();
    let at_deadline = authority.inspect();
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot),
        Err(Error::Stale)
    );
    assert_eq!(authority.inspect(), at_deadline);
}

#[test]
fn declared_judgment_basis_and_current_witnesses_are_both_required_without_spending() {
    let action = exact_action();
    let snapshot = exact_snapshot();
    let judgment = exact_judgment(&snapshot);
    let mut authority = ReferenceAuthority::new(effect_scope(), 7, 8).unwrap();
    authority.observe_time(ElapsedTick(1)).unwrap();
    authority.propose(1, action.clone()).unwrap();
    authority.prepare(1).unwrap();
    authority.begin_review(1).unwrap();

    let empty_basis = Judgment::capture(&snapshot, Vec::new()).unwrap();
    let wrong_basis = Judgment::capture(
        &snapshot,
        vec![ReadWitness::Exact {
            key: 100,
            value: None,
        }],
    )
    .unwrap();
    let required = required_witnesses();
    let extra_basis = Judgment::capture(
        &snapshot,
        vec![
            required[0].clone(),
            ReadWitness::Exact {
                key: 100,
                value: None,
            },
        ],
    )
    .unwrap();
    for (name, unrelated_judgment) in [
        ("empty", &empty_basis),
        ("wrong", &wrong_basis),
        ("extra", &extra_basis),
    ] {
        let before_wrong_basis = authority.inspect();
        assert!(matches!(
            authority.authorize(1, unrelated_judgment, &snapshot),
            Err(Error::Binding)
        ));
        assert_eq!(
            authority.inspect(),
            before_wrong_basis,
            "{name} basis changed authority state"
        );
        assert_eq!(
            authority.inspect().stages.get(&1),
            Some(&ActionState::Reviewing)
        );
    }

    let incomplete_snapshot = Snapshot {
        complete: false,
        ..snapshot.clone()
    };
    let before_missing_evidence = authority.inspect();
    assert!(matches!(
        authority.authorize(1, &judgment, &incomplete_snapshot),
        Err(Error::Incomplete)
    ));
    assert_eq!(authority.inspect(), before_missing_evidence);
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Reviewing)
    );

    let permit = authority.authorize(1, &judgment, &snapshot).unwrap();
    let mut invalidated_snapshot = snapshot.clone();
    invalidated_snapshot.values.insert(99, b"changed".to_vec());
    let before_invalidated_dispatch = authority.inspect();
    assert_eq!(
        authority.dispatch(&permit, &action, &invalidated_snapshot),
        Err(Error::Binding)
    );
    assert_eq!(authority.inspect(), before_invalidated_dispatch);

    // The original exact witness is the near-identical positive control.
    authority.dispatch(&permit, &action, &snapshot).unwrap();
}

#[test]
fn issuer_brand_blocks_same_scope_authority_and_capacity_failure_is_atomic() {
    let (mut first, action, snapshot, permit) = authorized_attempt(1, 7);
    let mut independently_bootstrapped = ReferenceAuthority::new(effect_scope(), 7, 8).unwrap();
    independently_bootstrapped
        .observe_time(ElapsedTick(1))
        .unwrap();
    independently_bootstrapped
        .propose(1, action.clone())
        .unwrap();
    independently_bootstrapped.prepare(1).unwrap();
    independently_bootstrapped.begin_review(1).unwrap();
    let independently_issued_permit = independently_bootstrapped
        .authorize(1, &exact_judgment(&snapshot), &snapshot)
        .unwrap();
    let other_before = independently_bootstrapped.inspect();
    // The receiving authority has the same authorized id and action. The
    // foreign token still fails solely because its issuer brand differs.
    assert_eq!(
        independently_bootstrapped.dispatch(&permit, &action, &snapshot),
        Err(Error::Binding)
    );
    assert_eq!(independently_bootstrapped.inspect(), other_before);
    independently_bootstrapped
        .dispatch(&independently_issued_permit, &action, &snapshot)
        .unwrap();
    first.dispatch(&permit, &action, &snapshot).unwrap();

    let second_action = FrozenAction::freeze(ActionSpec {
        units: 2,
        ..exact_action().spec().clone()
    })
    .unwrap();
    let second_snapshot = exact_snapshot();
    let second_judgment = exact_judgment(&second_snapshot);
    let mut capacity = ReferenceAuthority::new(effect_scope(), 4, 8).unwrap();
    capacity.observe_time(ElapsedTick(1)).unwrap();
    capacity.propose(1, exact_action()).unwrap();
    capacity.prepare(1).unwrap();
    capacity.begin_review(1).unwrap();
    let _first_permit = capacity
        .authorize(1, &second_judgment, &second_snapshot)
        .unwrap();

    capacity.propose(2, second_action).unwrap();
    capacity.prepare(2).unwrap();
    capacity.begin_review(2).unwrap();
    let before_capacity_refusal = capacity.inspect();
    assert!(matches!(
        capacity.authorize(2, &second_judgment, &second_snapshot),
        Err(Error::Limit)
    ));
    assert_eq!(capacity.inspect(), before_capacity_refusal);
    assert_eq!(
        capacity.inspect().stages.get(&2),
        Some(&ActionState::Reviewing)
    );
}

#[test]
fn attempt_limit_and_lifecycle_ordering_refuse_without_reserving_rights() {
    let action = exact_action();
    let snapshot = exact_snapshot();
    let judgment = exact_judgment(&snapshot);
    let mut authority = ReferenceAuthority::new(effect_scope(), 7, 2).unwrap();
    authority.observe_time(ElapsedTick(1)).unwrap();
    authority.propose(1, action.clone()).unwrap();
    authority.propose(2, action.clone()).unwrap();
    let at_attempt_limit = authority.inspect();
    assert_eq!(authority.propose(3, action.clone()), Err(Error::Limit));
    assert_eq!(authority.inspect(), at_attempt_limit);
    assert_eq!(authority.inspect().stages.len(), 2);

    // Beginning review or authorizing a merely proposed action cannot skip the
    // preparation transition or reserve rights.
    assert_eq!(authority.begin_review(1), Err(Error::WrongState));
    assert!(matches!(
        authority.authorize(1, &judgment, &snapshot),
        Err(Error::WrongState)
    ));
    assert_eq!(authority.inspect().available, 7);
    assert_eq!(authority.inspect().reserved, 0);

    // A denial is a valid terminal pre-dispatch path and has no refund to
    // fabricate because no reservation was made.
    authority.deny(1).unwrap();
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::Denied)
    );
    assert_eq!(authority.inspect().available, 7);
    assert_eq!(authority.inspect().charged, 0);

    // `MAX_ATTEMPTS` is the absolute admission ceiling, not an advisory value.
    assert!(matches!(
        ReferenceAuthority::new(effect_scope(), 7, MAX_ATTEMPTS + 1),
        Err(Error::InvalidInput)
    ));
}

#[test]
fn unknown_outcome_stays_charged_until_explicit_trusted_reconciliation() {
    let (mut authority, action, snapshot, permit) = authorized_attempt(1, 7);
    authority.dispatch(&permit, &action, &snapshot).unwrap();
    authority.mark_unknown(1).unwrap();
    assert_eq!(authority.inspect().charged, 3);
    assert_eq!(authority.cancel(1), Err(Error::WrongState));
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot),
        Err(Error::WrongState)
    );
    assert_eq!(authority.inspect().charged, 3);

    authority
        .record_trusted_outcome(1, TrustedOutcome::NotExecuted)
        .unwrap();
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::ConfirmedNotExecuted)
    );
    assert_eq!(authority.inspect().available, 7);
    assert_eq!(authority.inspect().charged, 0);

    let (mut authority, action, snapshot, permit) = authorized_attempt(1, 7);
    authority.dispatch(&permit, &action, &snapshot).unwrap();
    authority.mark_unknown(1).unwrap();
    authority.mark_irrecoverable(1).unwrap();
    assert_eq!(
        authority.inspect().stages.get(&1),
        Some(&ActionState::IrrecoverablyUnknown)
    );
    assert_eq!(
        authority.record_trusted_outcome(1, TrustedOutcome::NotExecuted),
        Err(Error::WrongState)
    );
    assert_eq!(authority.inspect().charged, 3);
}
