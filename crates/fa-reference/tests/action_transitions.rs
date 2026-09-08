//! Public transition-law tests for FA-001's in-memory action authority.
//!
//! These tests establish only the reference state machine's accounting and
//! refusal behavior. They do not establish durability, a broker, Cx, or an
//! external effect.

use std::collections::BTreeMap;

use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Inspection, Permit, Purpose,
    ReferenceAuthority, ResolvedTarget, Scope, TrustedOutcome, VERSION,
};
use fa_reference::{Error, Judgment, ReadWitness, Snapshot};

fn scope() -> Scope {
    Scope {
        tenant: 10,
        principal: 20,
        run: 30,
        branch: 40,
        authority: 50,
        purpose: Purpose::Effect,
    }
}

fn action(required_witnesses: Vec<ReadWitness>) -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: scope(),
        target: Some(ResolvedTarget {
            adapter: 60,
            object: 70,
            contract_version: 80,
            expected_version: 90,
            generation: 100,
        }),
        payload: b"frozen action bytes".to_vec(),
        required_witnesses,
        policy_epoch: 0,
        deadline: ElapsedTick(10),
        units: 3,
    })
    .expect("test action is within the public frozen-action contract")
}

fn exact_witnesses() -> Vec<ReadWitness> {
    vec![ReadWitness::Exact {
        key: 1,
        value: Some(b"required".to_vec()),
    }]
}

fn exact_snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 7,
        complete: true,
        values: BTreeMap::from([(1, b"required".to_vec())]),
    }
}

fn authority(total: u64) -> ReferenceAuthority {
    let mut authority =
        ReferenceAuthority::new(scope(), total, 16).expect("test authority has valid effect scope");
    authority
        .observe_time(ElapsedTick(1))
        .expect("trusted test clock observation is monotonic");
    authority
}

fn assert_inspection(
    authority: &ReferenceAuthority,
    total: u64,
    available: u64,
    reserved: u64,
    charged: u64,
    stages: &[(u64, ActionState)],
) {
    let expected = Inspection {
        available,
        reserved,
        charged,
        epoch: 0,
        elapsed: Some(ElapsedTick(1)),
        stages: stages.iter().copied().collect(),
    };
    assert_eq!(authority.inspect(), expected);
    assert_eq!(
        available + reserved + charged,
        total,
        "Inspection must expose B = A + R + S at every checked boundary"
    );
}

#[test]
fn lawful_nominal_transition_chain_is_reachable_with_exact_accounting() {
    let total = 10;
    let action = action(exact_witnesses());
    let snapshot = exact_snapshot();
    let judgment = Judgment::capture(&snapshot, exact_witnesses()).unwrap();
    let mut authority = authority(total);

    authority.propose(1, action.clone()).unwrap();
    assert_inspection(&authority, total, 10, 0, 0, &[(1, ActionState::Proposed)]);

    authority.prepare(1).unwrap();
    assert_inspection(&authority, total, 10, 0, 0, &[(1, ActionState::Prepared)]);

    authority.begin_review(1).unwrap();
    assert_inspection(&authority, total, 10, 0, 0, &[(1, ActionState::Reviewing)]);

    let permit = authority.authorize(1, &judgment, &snapshot).unwrap();
    assert_inspection(&authority, total, 7, 3, 0, &[(1, ActionState::Authorized)]);

    authority.dispatch(&permit, &action, &snapshot).unwrap();
    assert_inspection(&authority, total, 7, 0, 3, &[(1, ActionState::Dispatching)]);

    authority
        .record_trusted_outcome(1, TrustedOutcome::Executed)
        .unwrap();
    assert_inspection(&authority, total, 7, 0, 3, &[(1, ActionState::Confirmed)]);
}

#[test]
fn lawful_predispatch_cancel_and_deny_paths_preserve_or_release_only_reserved_rights() {
    let total = 10;
    let action = action(exact_witnesses());
    let snapshot = exact_snapshot();
    let judgment = Judgment::capture(&snapshot, exact_witnesses()).unwrap();
    let mut authority = authority(total);

    authority.propose(1, action.clone()).unwrap();
    authority.cancel(1).unwrap();
    assert_inspection(&authority, total, 10, 0, 0, &[(1, ActionState::Cancelled)]);

    authority.propose(2, action.clone()).unwrap();
    authority.prepare(2).unwrap();
    authority.begin_review(2).unwrap();
    let _permit = authority.authorize(2, &judgment, &snapshot).unwrap();
    assert_inspection(
        &authority,
        total,
        7,
        3,
        0,
        &[(1, ActionState::Cancelled), (2, ActionState::Authorized)],
    );

    authority.deny(2).unwrap();
    assert_inspection(
        &authority,
        total,
        10,
        0,
        0,
        &[(1, ActionState::Cancelled), (2, ActionState::Denied)],
    );
}

#[test]
fn unknown_liability_requires_trusted_nonexecution_or_stays_irrecoverable() {
    let total = 10;
    let action = action(exact_witnesses());
    let snapshot = exact_snapshot();
    let judgment = Judgment::capture(&snapshot, exact_witnesses()).unwrap();

    let mut reconciled = authority(total);
    reconciled.propose(1, action.clone()).unwrap();
    reconciled.prepare(1).unwrap();
    reconciled.begin_review(1).unwrap();
    let permit = reconciled.authorize(1, &judgment, &snapshot).unwrap();
    reconciled.dispatch(&permit, &action, &snapshot).unwrap();
    reconciled.mark_unknown(1).unwrap();
    assert_inspection(&reconciled, total, 7, 0, 3, &[(1, ActionState::Unknown)]);

    reconciled
        .record_trusted_outcome(1, TrustedOutcome::NotExecuted)
        .unwrap();
    assert_inspection(
        &reconciled,
        total,
        10,
        0,
        0,
        &[(1, ActionState::ConfirmedNotExecuted)],
    );

    let mut irrecoverable = authority(total);
    irrecoverable.propose(1, action.clone()).unwrap();
    irrecoverable.prepare(1).unwrap();
    irrecoverable.begin_review(1).unwrap();
    let permit = irrecoverable.authorize(1, &judgment, &snapshot).unwrap();
    irrecoverable.dispatch(&permit, &action, &snapshot).unwrap();
    irrecoverable.mark_unknown(1).unwrap();
    irrecoverable.mark_irrecoverable(1).unwrap();
    assert_inspection(
        &irrecoverable,
        total,
        7,
        0,
        3,
        &[(1, ActionState::IrrecoverablyUnknown)],
    );
}

#[test]
fn missing_duplicate_and_postdispatch_operations_refuse_atomically() {
    let total = 10;
    let action = action(exact_witnesses());
    let snapshot = exact_snapshot();
    let judgment = Judgment::capture(&snapshot, exact_witnesses()).unwrap();
    let mut authority = authority(total);

    let empty = authority.inspect();
    assert_eq!(authority.prepare(99), Err(Error::Missing));
    assert_eq!(authority.cancel(99), Err(Error::Missing));
    assert_eq!(authority.deny(99), Err(Error::Missing));
    assert_eq!(authority.mark_unknown(99), Err(Error::Missing));
    assert_eq!(
        authority.record_trusted_outcome(99, TrustedOutcome::Executed),
        Err(Error::Missing)
    );
    assert_eq!(authority.mark_irrecoverable(99), Err(Error::Missing));
    assert_eq!(authority.inspect(), empty);

    authority.propose(1, action.clone()).unwrap();
    let proposed = authority.inspect();
    assert_eq!(authority.propose(1, action.clone()), Err(Error::Duplicate));
    assert_eq!(authority.inspect(), proposed);
    assert_eq!(authority.prepare(1), Ok(()));
    let prepared = authority.inspect();
    assert_eq!(authority.prepare(1), Err(Error::WrongState));
    assert_eq!(authority.inspect(), prepared);

    authority.begin_review(1).unwrap();
    let permit = authority.authorize(1, &judgment, &snapshot).unwrap();
    let authorized = authority.inspect();
    assert_eq!(authority.mark_unknown(1), Err(Error::WrongState));
    assert_eq!(
        authority.record_trusted_outcome(1, TrustedOutcome::NotExecuted),
        Err(Error::WrongState)
    );
    assert_eq!(authority.inspect(), authorized);

    authority.dispatch(&permit, &action, &snapshot).unwrap();
    let dispatched = authority.inspect();
    assert_eq!(authority.cancel(1), Err(Error::WrongState));
    assert_eq!(authority.deny(1), Err(Error::WrongState));
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot),
        Err(Error::WrongState),
        "reborrowing an opaque consumed permit must not spend twice"
    );
    assert_eq!(authority.inspect(), dispatched);
}

#[test]
fn exact_negative_domain_and_semantic_epoch_are_rechecked_at_authorize_and_dispatch() {
    let total = 10;
    let negative_domain = vec![ReadWitness::EmptyRange { start: 20, end: 30 }];
    let action = action(negative_domain.clone());
    let baseline = Snapshot {
        semantic_epoch: 7,
        complete: true,
        values: BTreeMap::from([(1, b"unrelated".to_vec())]),
    };
    let judgment = Judgment::capture(&baseline, negative_domain).unwrap();

    let mut authorization_epoch = authority(total);
    authorization_epoch.propose(1, action.clone()).unwrap();
    authorization_epoch.prepare(1).unwrap();
    authorization_epoch.begin_review(1).unwrap();
    let changed_epoch = Snapshot {
        semantic_epoch: 8,
        ..baseline.clone()
    };
    let before_epoch_refusal = authorization_epoch.inspect();
    assert_eq!(
        authorization_epoch
            .authorize(1, &judgment, &changed_epoch)
            .unwrap_err(),
        Error::Binding
    );
    assert_eq!(authorization_epoch.inspect(), before_epoch_refusal);
    assert_inspection(
        &authorization_epoch,
        total,
        10,
        0,
        0,
        &[(1, ActionState::Reviewing)],
    );

    let permit = authorization_epoch
        .authorize(1, &judgment, &baseline)
        .unwrap();
    assert_inspection(
        &authorization_epoch,
        total,
        7,
        3,
        0,
        &[(1, ActionState::Authorized)],
    );

    let epoch_changed_after_authorization = Snapshot {
        semantic_epoch: 8,
        ..baseline.clone()
    };
    let before_dispatch_epoch_refusal = authorization_epoch.inspect();
    assert_eq!(
        authorization_epoch.dispatch(&permit, &action, &epoch_changed_after_authorization),
        Err(Error::Binding)
    );
    assert_eq!(authorization_epoch.inspect(), before_dispatch_epoch_refusal);

    let mut populated_negative_domain = baseline.clone();
    populated_negative_domain
        .values
        .insert(25, b"present".to_vec());
    let before_negative_refusal = authorization_epoch.inspect();
    assert_eq!(
        authorization_epoch.dispatch(&permit, &action, &populated_negative_domain),
        Err(Error::Binding),
        "a value in a witnessed-empty interval invalidates the judgment"
    );
    assert_eq!(authorization_epoch.inspect(), before_negative_refusal);

    authorization_epoch
        .dispatch(&permit, &action, &baseline)
        .unwrap();
    assert_inspection(
        &authorization_epoch,
        total,
        7,
        0,
        3,
        &[(1, ActionState::Dispatching)],
    );
}

#[derive(Clone, Copy)]
enum LifecycleMethod {
    Prepare,
    BeginReview,
    Authorize,
    Dispatch,
    Cancel,
    Deny,
    MarkUnknown,
    TrustedExecuted,
    TrustedNotExecuted,
    MarkIrrecoverable,
}

#[derive(Clone, Copy)]
enum ExpectedCell {
    Advances(ActionState),
    Refuses(Error),
    Unavailable(&'static str),
}

const METHODS: [LifecycleMethod; 10] = [
    LifecycleMethod::Prepare,
    LifecycleMethod::BeginReview,
    LifecycleMethod::Authorize,
    LifecycleMethod::Dispatch,
    LifecycleMethod::Cancel,
    LifecycleMethod::Deny,
    LifecycleMethod::MarkUnknown,
    LifecycleMethod::TrustedExecuted,
    LifecycleMethod::TrustedNotExecuted,
    LifecycleMethod::MarkIrrecoverable,
];

const WRONG: ExpectedCell = ExpectedCell::Refuses(Error::WrongState);
const NO_PERMIT: ExpectedCell = ExpectedCell::Unavailable(
    "no Permit exists before authorization; dispatch is statically unavailable",
);

// This is the public transition law from plan §8.2.  It deliberately names
// every callable lifecycle method from every ActionState; the three
// preauthorization dispatch cells are documented as unavailable rather than
// manufactured with a forged permit.
const TRANSITION_TABLE: [(ActionState, [ExpectedCell; 10]); 11] = [
    (
        ActionState::Proposed,
        [
            ExpectedCell::Advances(ActionState::Prepared),
            WRONG,
            WRONG,
            NO_PERMIT,
            ExpectedCell::Advances(ActionState::Cancelled),
            ExpectedCell::Advances(ActionState::Denied),
            WRONG,
            WRONG,
            WRONG,
            WRONG,
        ],
    ),
    (
        ActionState::Prepared,
        [
            WRONG,
            ExpectedCell::Advances(ActionState::Reviewing),
            WRONG,
            NO_PERMIT,
            ExpectedCell::Advances(ActionState::Cancelled),
            ExpectedCell::Advances(ActionState::Denied),
            WRONG,
            WRONG,
            WRONG,
            WRONG,
        ],
    ),
    (
        ActionState::Reviewing,
        [
            WRONG,
            WRONG,
            ExpectedCell::Advances(ActionState::Authorized),
            NO_PERMIT,
            ExpectedCell::Advances(ActionState::Cancelled),
            ExpectedCell::Advances(ActionState::Denied),
            WRONG,
            WRONG,
            WRONG,
            WRONG,
        ],
    ),
    (
        ActionState::Authorized,
        [
            WRONG,
            WRONG,
            WRONG,
            ExpectedCell::Advances(ActionState::Dispatching),
            ExpectedCell::Advances(ActionState::Cancelled),
            ExpectedCell::Advances(ActionState::Denied),
            WRONG,
            WRONG,
            WRONG,
            WRONG,
        ],
    ),
    (
        ActionState::Dispatching,
        [
            WRONG,
            WRONG,
            WRONG,
            WRONG,
            WRONG,
            WRONG,
            ExpectedCell::Advances(ActionState::Unknown),
            ExpectedCell::Advances(ActionState::Confirmed),
            ExpectedCell::Advances(ActionState::ConfirmedNotExecuted),
            WRONG,
        ],
    ),
    (
        ActionState::Confirmed,
        [
            WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG,
        ],
    ),
    (
        ActionState::Denied,
        [
            WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG,
        ],
    ),
    (
        ActionState::Cancelled,
        [
            WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG,
        ],
    ),
    (
        ActionState::Unknown,
        [
            WRONG,
            WRONG,
            WRONG,
            WRONG,
            WRONG,
            WRONG,
            WRONG,
            ExpectedCell::Advances(ActionState::Confirmed),
            ExpectedCell::Advances(ActionState::ConfirmedNotExecuted),
            ExpectedCell::Advances(ActionState::IrrecoverablyUnknown),
        ],
    ),
    (
        ActionState::ConfirmedNotExecuted,
        [
            WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG,
        ],
    ),
    (
        ActionState::IrrecoverablyUnknown,
        [
            WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG, WRONG,
        ],
    ),
];

struct StateFixture {
    authority: ReferenceAuthority,
    action: FrozenAction,
    snapshot: Snapshot,
    judgment: Judgment,
    permit: Option<Permit>,
}

impl StateFixture {
    fn construct(state: ActionState) -> Self {
        let action = action(exact_witnesses());
        let snapshot = exact_snapshot();
        let judgment = Judgment::capture(&snapshot, exact_witnesses()).unwrap();
        let mut fixture = Self {
            authority: authority(10),
            action,
            snapshot,
            judgment,
            permit: None,
        };
        fixture
            .authority
            .propose(1, fixture.action.clone())
            .unwrap();
        if state == ActionState::Proposed {
            return fixture;
        }
        fixture.authority.prepare(1).unwrap();
        if state == ActionState::Prepared {
            return fixture;
        }
        fixture.authority.begin_review(1).unwrap();
        if state == ActionState::Reviewing {
            return fixture;
        }
        fixture.authorize().unwrap();
        if state == ActionState::Authorized {
            return fixture;
        }
        match state {
            ActionState::Denied => {
                fixture.authority.deny(1).unwrap();
                return fixture;
            }
            ActionState::Cancelled => {
                fixture.authority.cancel(1).unwrap();
                return fixture;
            }
            _ => {}
        }

        fixture.dispatch().unwrap();
        if state == ActionState::Dispatching {
            return fixture;
        }
        match state {
            ActionState::Confirmed => fixture
                .authority
                .record_trusted_outcome(1, TrustedOutcome::Executed)
                .unwrap(),
            ActionState::ConfirmedNotExecuted => fixture
                .authority
                .record_trusted_outcome(1, TrustedOutcome::NotExecuted)
                .unwrap(),
            ActionState::Unknown => fixture.authority.mark_unknown(1).unwrap(),
            ActionState::IrrecoverablyUnknown => {
                fixture.authority.mark_unknown(1).unwrap();
                fixture.authority.mark_irrecoverable(1).unwrap();
            }
            ActionState::Proposed
            | ActionState::Prepared
            | ActionState::Reviewing
            | ActionState::Authorized
            | ActionState::Dispatching
            | ActionState::Denied
            | ActionState::Cancelled => unreachable!("handled by prior public transitions"),
        }
        fixture
    }

    fn authorize(&mut self) -> Result<(), Error> {
        self.authority
            .authorize(1, &self.judgment, &self.snapshot)
            .map(|permit| self.permit = Some(permit))
    }

    fn dispatch(&mut self) -> Result<(), Error> {
        let permit = self
            .permit
            .as_ref()
            .expect("permit is minted only by the preceding public authorization");
        self.authority
            .dispatch(permit, &self.action, &self.snapshot)
    }

    fn invoke(&mut self, method: LifecycleMethod) -> Result<(), Error> {
        match method {
            LifecycleMethod::Prepare => self.authority.prepare(1),
            LifecycleMethod::BeginReview => self.authority.begin_review(1),
            LifecycleMethod::Authorize => self.authorize(),
            LifecycleMethod::Dispatch => self.dispatch(),
            LifecycleMethod::Cancel => self.authority.cancel(1),
            LifecycleMethod::Deny => self.authority.deny(1),
            LifecycleMethod::MarkUnknown => self.authority.mark_unknown(1),
            LifecycleMethod::TrustedExecuted => self
                .authority
                .record_trusted_outcome(1, TrustedOutcome::Executed),
            LifecycleMethod::TrustedNotExecuted => self
                .authority
                .record_trusted_outcome(1, TrustedOutcome::NotExecuted),
            LifecycleMethod::MarkIrrecoverable => self.authority.mark_irrecoverable(1),
        }
    }
}

fn accounting_for(state: ActionState) -> (u64, u64, u64) {
    match state {
        ActionState::Proposed
        | ActionState::Prepared
        | ActionState::Reviewing
        | ActionState::Denied
        | ActionState::Cancelled
        | ActionState::ConfirmedNotExecuted => (10, 0, 0),
        ActionState::Authorized => (7, 3, 0),
        ActionState::Dispatching
        | ActionState::Confirmed
        | ActionState::Unknown
        | ActionState::IrrecoverablyUnknown => (7, 0, 3),
    }
}

#[test]
fn public_transition_table_exercises_every_state_method_cell_without_forging_permits() {
    let mut exercised = 0_usize;
    let mut unavailable = 0_usize;

    for (from, row) in TRANSITION_TABLE {
        for (method, expected) in METHODS.into_iter().zip(row) {
            let mut fixture = StateFixture::construct(from);
            let before = fixture.authority.inspect();
            let (available, reserved, charged) = accounting_for(from);
            assert_inspection(
                &fixture.authority,
                10,
                available,
                reserved,
                charged,
                &[(1, from)],
            );
            match expected {
                ExpectedCell::Advances(target) => {
                    assert_eq!(fixture.invoke(method), Ok(()));
                    let (available, reserved, charged) = accounting_for(target);
                    assert_inspection(
                        &fixture.authority,
                        10,
                        available,
                        reserved,
                        charged,
                        &[(1, target)],
                    );
                    exercised += 1;
                }
                ExpectedCell::Refuses(error) => {
                    assert_eq!(fixture.invoke(method), Err(error));
                    assert_eq!(
                        fixture.authority.inspect(),
                        before,
                        "forbidden public operation must be atomic"
                    );
                    exercised += 1;
                }
                ExpectedCell::Unavailable(reason) => {
                    assert!(
                        fixture.permit.is_none(),
                        "dispatch is unavailable only in preauthorization states"
                    );
                    assert_eq!(fixture.authority.inspect(), before, "{reason}");
                    unavailable += 1;
                }
            }
        }
    }

    assert_eq!(
        exercised, 107,
        "all callable state/method cells were exercised"
    );
    assert_eq!(
        unavailable, 3,
        "only proposed, prepared, and reviewing lack a public permit"
    );
}
