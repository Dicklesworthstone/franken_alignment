//! End-to-end reference composition; no real effects or cryptographic claims.

use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::congress::{
    CongressPolicy, MemberPolicy, evaluate_round,
};
use fa_reference::action::consequence::gate::ConsequenceAuthority;
use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION,
};
use fa_reference::reducer::Caps;
use fa_reference::round::{Round, Verdict, commitment};
use fa_reference::{Error, Judgment, ReadWitness, Snapshot};
use std::collections::BTreeMap;

fn action() -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: Scope {
            tenant: 1,
            principal: 2,
            run: 3,
            branch: 4,
            authority: 5,
            purpose: Purpose::Effect,
        },
        target: Some(ResolvedTarget {
            adapter: 1,
            object: 1,
            contract_version: 1,
            expected_version: 1,
            generation: 1,
        }),
        payload: b"publish-reviewed-bytes".to_vec(),
        required_witnesses: vec![ReadWitness::Exact {
            key: 7,
            value: Some(vec![9]),
        }],
        policy_epoch: 0,
        deadline: ElapsedTick(100),
        units: 4,
    })
    .unwrap()
}

fn policy() -> CongressPolicy {
    CongressPolicy {
        generation: 1,
        members: BTreeMap::from([
            (
                "alice".to_owned(),
                MemberPolicy {
                    cohort: "a".to_owned(),
                    weight: 5,
                },
            ),
            (
                "bob".to_owned(),
                MemberPolicy {
                    cohort: "b".to_owned(),
                    weight: 5,
                },
            ),
        ]),
        caps: Caps {
            per_member: 5,
            per_cohort: 5,
        },
        continue_minimum: 1,
        continue_hold_maximum: 0,
        narrow_at: 8,
        suspend_at: 10,
        minimum_members: 1,
        minimum_cohorts: 1,
    }
}

fn round(id: u64, reveal_bob: bool) -> Round {
    let mut round = Round::new(id, &[7; 32]).unwrap();
    round.add_member("alice").unwrap();
    round.add_member("bob").unwrap();
    for member in ["alice", "bob"] {
        let digest = commitment(id, member, &[7; 32], Verdict::Allow, b"salt").unwrap();
        round.commit(member, digest).unwrap();
    }
    round.open_reveals().unwrap();
    round.reveal("alice", Verdict::Allow, b"salt").unwrap();
    if reveal_bob {
        round.reveal("bob", Verdict::Allow, b"salt").unwrap();
    }
    round
}

fn setup() -> (ConsequenceAuthority, FrozenAction, Snapshot, Judgment) {
    let action = action();
    let snapshot = Snapshot {
        semantic_epoch: 0,
        complete: true,
        values: BTreeMap::from([(7, vec![9])]),
    };
    let judgment = Judgment::capture(&snapshot, action.spec().required_witnesses.clone()).unwrap();
    let mut gate = ConsequenceAuthority::new(action.spec().scope, 10, 4).unwrap();
    gate.observe_time(ElapsedTick(1)).unwrap();
    gate.propose(1, action.clone()).unwrap();
    gate.prepare(1).unwrap();
    gate.begin_review(1).unwrap();
    (gate, action, snapshot, judgment)
}

#[test]
fn complete_round_reaches_dispatch_only_after_normal_authorization() {
    let (mut gate, action, snapshot, judgment) = setup();
    let evaluation = evaluate_round(&round(21, true), &policy(), false, false).unwrap();
    let request = evaluation.into_request(1, 0, action.clone(), None);
    let receipt = gate.apply_review(request).unwrap();
    assert_eq!(receipt.binding.round, 21);
    assert_eq!(receipt.binding.evidence_root, [7; 32]);
    assert_eq!(receipt.binding.reducer_generation, 1);
    assert_eq!(receipt.action, action);
    assert_eq!(receipt.decision.consequence, Consequence::Continue);
    assert_eq!(gate.inspect().ledger.available, 10);
    assert_eq!(gate.inspect().ledger.stages[&1], ActionState::Reviewing);
    let permit = gate.authorize(1, &judgment, &snapshot).unwrap();
    gate.dispatch(&permit, &action, &snapshot).unwrap();
    assert_eq!(gate.inspect().ledger.charged, 4);
    assert_eq!(gate.dispatch(&permit, &action, &snapshot), Err(Error::WrongState));
}

#[test]
fn late_reveal_requires_new_round_instead_of_rewriting_an_applied_hold() {
    let (mut gate, action, snapshot, judgment) = setup();
    let mut frozen = round(21, false);
    let evaluation = evaluate_round(&frozen, &policy(), false, false).unwrap();
    assert_eq!(evaluation.missing(), &["bob".to_owned()]);
    let request = evaluation.into_request(1, 0, action.clone(), None);
    let receipt = gate.apply_review(request).unwrap();
    assert_eq!(receipt.decision.consequence, Consequence::HoldEffect);
    assert_eq!(gate.authorize(1, &judgment, &snapshot).unwrap_err(), Error::WrongState);
    frozen.reveal("bob", Verdict::Allow, b"salt").unwrap();
    let evaluation = evaluate_round(&frozen, &policy(), false, false).unwrap();
    let request = evaluation.into_request(1, 1, action.clone(), None);
    assert_eq!(gate.apply_review(request), Err(Error::Duplicate));
    let evaluation = evaluate_round(&round(22, true), &policy(), false, false).unwrap();
    let request = evaluation.into_request(1, 1, action.clone(), None);
    gate.apply_review(request).unwrap();
    let permit = gate.authorize(1, &judgment, &snapshot).unwrap();
    gate.dispatch(&permit, &action, &snapshot).unwrap();
}

#[test]
fn unanimous_empirical_approval_cannot_override_an_exact_disqualifier() {
    let (mut gate, action, snapshot, judgment) = setup();
    let evaluation = evaluate_round(&round(21, true), &policy(), true, false).unwrap();
    let request = evaluation.into_request(1, 0, action, None);
    gate.apply_review(request).unwrap();
    assert_eq!(gate.inspect().ledger.stages[&1], ActionState::Denied);
    assert_eq!(gate.inspect().ledger.available, 10);
    assert_eq!(gate.authorize(1, &judgment, &snapshot).unwrap_err(), Error::WrongState);
}

#[test]
fn reviewed_action_substitution_does_not_mutate_authority() {
    let (mut gate, action, _, _) = setup();
    let before = gate.inspect();
    let mut substituted = action.spec().clone();
    substituted.payload = b"different-publish-bytes".to_vec();
    let substituted = FrozenAction::freeze(substituted).unwrap();
    let evaluation = evaluate_round(&round(21, true), &policy(), false, false).unwrap();
    let request = evaluation.into_request(1, 0, substituted, None);
    assert_eq!(gate.apply_review(request), Err(Error::Binding));
    assert_eq!(gate.inspect(), before);
    assert!(gate.receipts().is_empty());
}
