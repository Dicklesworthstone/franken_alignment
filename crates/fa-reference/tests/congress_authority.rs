use fa_reference::action::congress::CongressAuthority;
use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, TrustedOutcome,
    VERSION,
};
use fa_reference::reducer::Caps;
use fa_reference::round::review::{ExactStatus, Member, Requirements, ReviewPolicy};
use fa_reference::round::{Verdict, commitment};
use fa_reference::{Error, ReadWitness, Snapshot};
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

fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 11,
        complete: true,
        values: BTreeMap::from([(7, vec![1])]),
    }
}

fn action(units: u64) -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: scope(),
        target: Some(ResolvedTarget {
            adapter: 1,
            object: 2,
            contract_version: 1,
            expected_version: 1,
            generation: 1,
        }),
        payload: b"publish exact package".to_vec(),
        required_witnesses: vec![ReadWitness::Exact {
            key: 7,
            value: Some(vec![1]),
        }],
        policy_epoch: 0,
        deadline: ElapsedTick(10),
        units,
    })
    .unwrap()
}

fn policy() -> ReviewPolicy {
    ReviewPolicy::new(
        vec![
            Member {
                id: "alice".to_owned(),
                cohort: "a".to_owned(),
                weight: 1,
            },
            Member {
                id: "bob".to_owned(),
                cohort: "b".to_owned(),
                weight: 1,
            },
        ],
        Requirements {
            caps: Caps {
                per_member: 1,
                per_cohort: 1,
            },
            min_permit_weight: 2,
            max_hold_weight: 0,
            min_permit_cohorts: 2,
        },
    )
    .unwrap()
}

fn authority() -> CongressAuthority {
    let mut authority = CongressAuthority::new(scope(), 10, 8, policy()).unwrap();
    authority.observe_time(ElapsedTick(0)).unwrap();
    authority
}

fn start(authority: &mut CongressAuthority, id: u64, action: FrozenAction) {
    authority
        .propose(id, action, &snapshot(), b"whole declared helper input")
        .unwrap();
    authority.prepare(id).unwrap();
    authority.begin_review(id).unwrap();
}

fn committed(authority: &mut CongressAuthority, id: u64) {
    let challenge = authority
        .review(id)
        .unwrap()
        .transcript()
        .evidence_root()
        .to_vec();
    for member in ["alice", "bob"] {
        let digest = commitment(id, member, &challenge, Verdict::Allow, b"salt").unwrap();
        authority.commit(id, member, digest).unwrap();
    }
    authority.open_reveals(id).unwrap();
    authority.reveal(id, "alice", Verdict::Allow, b"salt").unwrap();
}

fn ready(authority: &mut CongressAuthority, id: u64, action: FrozenAction) {
    start(authority, id, action);
    committed(authority, id);
    authority.reveal(id, "bob", Verdict::Allow, b"salt").unwrap();
}

const EVIDENCE: &[u8] = b"whole declared helper input";

#[test]
fn reviewed_action_reserves_dispatches_and_confirms_once() {
    let mut authority = authority();
    let action = action(3);
    ready(&mut authority, 7, action.clone());
    let permit = authority
        .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    assert_eq!(authority.inspect().available, 7);
    assert_eq!(authority.inspect().reserved, 3);
    authority
        .dispatch(&permit, &action, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    assert_eq!(authority.inspect().reserved, 0);
    assert_eq!(authority.inspect().charged, 3);
    let before = authority.inspect();
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot(), EVIDENCE, ExactStatus::Clear),
        Err(Error::WrongState)
    );
    assert_eq!(authority.inspect(), before);
    authority.record_trusted_outcome(7, TrustedOutcome::Executed).unwrap();
    assert_eq!(authority.inspect().stages[&7], ActionState::Confirmed);
    assert_eq!(authority.inspect().charged, 3);
}

#[test]
fn missing_vote_and_exact_unknown_do_not_reserve_rights() {
    let mut authority = authority();
    start(&mut authority, 7, action(3));
    committed(&mut authority, 7);
    let before = authority.inspect();
    assert_eq!(
        authority
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .unwrap_err(),
        Error::Incomplete
    );
    assert_eq!(authority.inspect(), before);
    authority.reveal(7, "bob", Verdict::Allow, b"salt").unwrap();
    assert_eq!(
        authority
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Unknown)
            .unwrap_err(),
        Error::Incomplete
    );
    assert_eq!(authority.inspect(), before);
    assert!(
        authority
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .is_ok()
    );
}

#[test]
fn whole_view_and_semantic_epoch_are_rechecked_before_authorization() {
    let mut authority = authority();
    ready(&mut authority, 7, action(3));
    let before = authority.inspect();
    assert_eq!(
        authority
            .authorize(7, &snapshot(), b"omitted context", ExactStatus::Clear)
            .unwrap_err(),
        Error::Binding
    );
    let mut changed = snapshot();
    changed.semantic_epoch += 1;
    assert_eq!(
        authority
            .authorize(7, &changed, EVIDENCE, ExactStatus::Clear)
            .unwrap_err(),
        Error::Binding
    );
    changed = snapshot();
    changed.values.insert(7, vec![2]);
    assert_eq!(
        authority
            .authorize(7, &changed, EVIDENCE, ExactStatus::Clear)
            .unwrap_err(),
        Error::Binding
    );
    assert_eq!(authority.inspect(), before);
    assert!(
        authority
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .is_ok()
    );
}

#[test]
fn dispatch_rechecks_action_evidence_and_exact_status_without_refunding() {
    let mut authority = authority();
    let action = action(3);
    ready(&mut authority, 7, action.clone());
    let permit = authority
        .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    let before = authority.inspect();
    let mut spec = action.spec().clone();
    spec.payload.push(b'!');
    let changed = FrozenAction::freeze(spec).unwrap();
    assert_eq!(
        authority.dispatch(&permit, &changed, &snapshot(), EVIDENCE, ExactStatus::Clear),
        Err(Error::Binding)
    );
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot(), b"changed", ExactStatus::Clear),
        Err(Error::Binding)
    );
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot(), EVIDENCE, ExactStatus::Unknown),
        Err(Error::Incomplete)
    );
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot(), EVIDENCE, ExactStatus::Disqualified),
        Err(Error::Binding)
    );
    assert_eq!(authority.inspect(), before);
    authority
        .dispatch(&permit, &action, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
}

#[test]
fn revoked_or_expired_permits_cannot_dispatch() {
    for revoke in [false, true] {
        let mut authority = authority();
        let action = action(3);
        ready(&mut authority, 7, action.clone());
        let permit = authority
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .unwrap();
        if revoke {
            authority.revoke_epoch().unwrap();
        } else {
            authority.observe_time(ElapsedTick(10)).unwrap();
        }
        let before = authority.inspect();
        assert_eq!(
            authority.dispatch(&permit, &action, &snapshot(), EVIDENCE, ExactStatus::Clear),
            Err(Error::Stale)
        );
        assert_eq!(authority.inspect(), before);
        authority.cancel(7).unwrap();
        assert_eq!(authority.inspect().available, 10);
    }
}

#[test]
fn unknown_effect_is_not_refunded_by_cancellation_or_a_later_hold() {
    let mut authority = authority();
    let action = action(3);
    ready(&mut authority, 7, action.clone());
    let permit = authority
        .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    authority
        .dispatch(&permit, &action, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    authority.mark_unknown(7).unwrap();
    let before = authority.inspect();
    assert_eq!(authority.cancel(7), Err(Error::WrongState));
    assert_eq!(
        authority.dispatch(&permit, &action, &snapshot(), EVIDENCE, ExactStatus::Unknown),
        Err(Error::Incomplete)
    );
    assert_eq!(authority.inspect(), before);
    assert_eq!(before.charged, 3);
    authority.record_trusted_outcome(7, TrustedOutcome::NotExecuted).unwrap();
    assert_eq!(authority.inspect().available, 10);
}

#[test]
fn cloned_transcripts_do_not_transfer_a_permit_between_authorities() {
    let mut first = authority();
    let mut second = authority();
    let action = action(3);
    ready(&mut first, 7, action.clone());
    ready(&mut second, 7, action.clone());
    let first_permit = first
        .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    let second_permit = second
        .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    let before = second.inspect();
    assert_eq!(
        second.dispatch(&first_permit, &action, &snapshot(), EVIDENCE, ExactStatus::Clear),
        Err(Error::Binding)
    );
    assert_eq!(second.inspect(), before);
    second
        .dispatch(&second_permit, &action, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
}

#[test]
fn budget_contention_preserves_reviews_and_only_cancellation_releases_reservations() {
    let mut authority = authority();
    ready(&mut authority, 7, action(7));
    ready(&mut authority, 8, action(7));
    let _first = authority
        .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    let before = authority.inspect();
    assert_eq!(
        authority
            .authorize(8, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .unwrap_err(),
        Error::Limit
    );
    assert_eq!(authority.inspect(), before);
    authority.cancel(7).unwrap();
    assert!(
        authority
            .authorize(8, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .is_ok()
    );
}

#[test]
fn exact_binding_capacity_is_accepted_and_one_over_never_inserts_an_attempt() {
    let mut sizing = authority();
    sizing.propose(7, action(3), &snapshot(), b"").unwrap();
    let overhead = sizing.review(7).unwrap().transcript().evidence_root().len();
    let exact_evidence = vec![b'x'; fa_reference::round::MAX_FIELD_LEN - overhead];
    let mut accepted = authority();
    accepted
        .propose(7, action(3), &snapshot(), &exact_evidence)
        .unwrap();
    assert_eq!(
        accepted.review(7).unwrap().transcript().evidence_root().len(),
        fa_reference::round::MAX_FIELD_LEN
    );
    accepted.prepare(7).unwrap();
    accepted.begin_review(7).unwrap();
    committed(&mut accepted, 7);
    accepted.reveal(7, "bob", Verdict::Allow, b"salt").unwrap();
    let permit = accepted
        .authorize(7, &snapshot(), &exact_evidence, ExactStatus::Clear)
        .unwrap();
    accepted
        .dispatch(
            &permit,
            &action(3),
            &snapshot(),
            &exact_evidence,
            ExactStatus::Clear,
        )
        .unwrap();

    let mut rejected = authority();
    let before = rejected.inspect();
    let mut too_large = exact_evidence;
    too_large.push(b'x');
    assert_eq!(
        rejected.propose(7, action(3), &snapshot(), &too_large),
        Err(Error::Limit)
    );
    assert_eq!(rejected.inspect(), before);
    assert!(matches!(rejected.review(7), Err(Error::Missing)));
    ready(&mut rejected, 7, action(3));
    assert!(
        rejected
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .is_ok()
    );
}

#[test]
fn policy_changes_bind_commitments_even_when_the_same_votes_would_pass() {
    let mut original = authority();
    start(&mut original, 7, action(3));
    let original_view = original.review(7).unwrap().transcript().evidence_root();
    let old_digest = commitment(7, "alice", original_view, Verdict::Allow, b"salt").unwrap();
    let mut requirements = policy().requirements();
    requirements.max_hold_weight = 1;
    let changed_policy = ReviewPolicy::new(
        vec![
            Member {
                id: "alice".into(),
                cohort: "a".into(),
                weight: 1,
            },
            Member {
                id: "bob".into(),
                cohort: "b".into(),
                weight: 1,
            },
        ],
        requirements,
    )
    .unwrap();
    let mut changed = CongressAuthority::new(scope(), 10, 8, changed_policy).unwrap();
    changed.observe_time(ElapsedTick(0)).unwrap();
    start(&mut changed, 7, action(3));
    let changed_view = changed.review(7).unwrap().transcript().evidence_root();
    assert_ne!(original_view, changed_view);
    let bob = commitment(7, "bob", changed_view, Verdict::Allow, b"salt").unwrap();
    changed.commit(7, "alice", old_digest).unwrap();
    changed.commit(7, "bob", bob).unwrap();
    changed.open_reveals(7).unwrap();
    // Positive reveal from the current binding; old-policy reveal is refused.
    changed.reveal(7, "bob", Verdict::Allow, b"salt").unwrap();
    assert_eq!(
        changed.reveal(7, "alice", Verdict::Allow, b"salt"),
        Err(Error::Binding)
    );
    assert_eq!(
        changed
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .unwrap_err(),
        Error::Incomplete
    );
    committed(&mut original, 7);
    original.reveal(7, "bob", Verdict::Allow, b"salt").unwrap();
    assert!(
        original
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .is_ok()
    );
}

#[test]
fn dispatch_rechecks_snapshot_closure_epoch_and_witnesses_after_authorization() {
    let mut authority = authority();
    let action = action(3);
    ready(&mut authority, 7, action.clone());
    let permit = authority
        .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
        .unwrap();
    let before = authority.inspect();
    let mut incomplete = snapshot();
    incomplete.complete = false;
    assert_eq!(
        authority.dispatch(&permit, &action, &incomplete, EVIDENCE, ExactStatus::Clear),
        Err(Error::Incomplete)
    );
    let mut changed = snapshot();
    changed.semantic_epoch += 1;
    assert_eq!(
        authority.dispatch(&permit, &action, &changed, EVIDENCE, ExactStatus::Clear),
        Err(Error::Binding)
    );
    changed = snapshot();
    changed.values.insert(7, vec![2]);
    assert_eq!(
        authority.dispatch(&permit, &action, &changed, EVIDENCE, ExactStatus::Clear),
        Err(Error::Binding)
    );
    assert_eq!(authority.inspect(), before);
    // An unrelated snapshot change does not invalidate exact declared reads.
    let mut unrelated = snapshot();
    unrelated.values.insert(99, vec![3]);
    authority
        .dispatch(&permit, &action, &unrelated, EVIDENCE, ExactStatus::Clear)
        .unwrap();
    assert_eq!(authority.inspect().charged, 3);
}

#[test]
fn valid_empirical_holds_and_exact_disqualification_never_mint_permits() {
    for (bob, exact, expected) in [
        (Verdict::Hold, ExactStatus::Clear, Error::Incomplete),
        (Verdict::Deny, ExactStatus::Clear, Error::Incomplete),
        (Verdict::Abstain, ExactStatus::Clear, Error::Incomplete),
        (Verdict::Allow, ExactStatus::Disqualified, Error::Binding),
    ] {
        let mut held = authority();
        start(&mut held, 7, action(3));
        let challenge = held
            .review(7)
            .unwrap()
            .transcript()
            .evidence_root()
            .to_vec();
        for (member, verdict) in [("alice", Verdict::Allow), ("bob", bob)] {
            let digest = commitment(7, member, &challenge, verdict, b"salt").unwrap();
            held.commit(7, member, digest).unwrap();
        }
        held.open_reveals(7).unwrap();
        held.reveal(7, "alice", Verdict::Allow, b"salt").unwrap();
        held.reveal(7, "bob", bob, b"salt").unwrap();
        let before = held.inspect();
        assert_eq!(
            held.authorize(7, &snapshot(), EVIDENCE, exact).unwrap_err(),
            expected
        );
        assert_eq!(held.inspect(), before);
        assert_eq!(before.available, 10);
        assert_eq!(before.reserved, 0);
    }
    // The otherwise identical fully affirmative and exact-clear control works.
    let mut permitted = authority();
    ready(&mut permitted, 7, action(3));
    assert!(
        permitted
            .authorize(7, &snapshot(), EVIDENCE, ExactStatus::Clear)
            .is_ok()
    );
}
