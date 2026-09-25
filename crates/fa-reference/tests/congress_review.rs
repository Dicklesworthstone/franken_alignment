use fa_reference::Error;
use fa_reference::reducer::Caps;
use fa_reference::round::review::{
    CongressReview, ExactStatus, HoldReason, Member, Requirements, ReviewDecision, ReviewPolicy,
};
use fa_reference::round::{Verdict, commitment};

fn member(id: &str, cohort: &str, weight: u64) -> Member {
    Member {
        id: id.to_owned(),
        cohort: cohort.to_owned(),
        weight,
    }
}

fn requirements() -> Requirements {
    Requirements {
        caps: Caps {
            per_member: 5,
            per_cohort: 6,
        },
        min_permit_weight: 6,
        max_hold_weight: 0,
        min_permit_cohorts: 2,
    }
}

fn review(requirements: Requirements) -> CongressReview {
    let policy = ReviewPolicy::new(
        vec![member("alice", "a", 8), member("bob", "b", 8)],
        requirements,
    )
    .unwrap();
    CongressReview::new(policy, 7, b"frozen-view").unwrap()
}

fn commit_votes(review: &mut CongressReview, alice: Verdict, bob: Verdict) {
    for (id, verdict) in [("alice", alice), ("bob", bob)] {
        let digest = commitment(7, id, b"frozen-view", verdict, b"salt").unwrap();
        review.commit(id, digest).unwrap();
    }
    review.open_reveals().unwrap();
}

fn complete(requirements: Requirements, alice: Verdict, bob: Verdict) -> CongressReview {
    let mut review = review(requirements);
    commit_votes(&mut review, alice, bob);
    review.reveal("alice", alice, b"salt").unwrap();
    review.reveal("bob", bob, b"salt").unwrap();
    review
}

#[test]
fn complete_transcript_produces_the_actual_capped_reduction() {
    let review = complete(requirements(), Verdict::Allow, Verdict::Allow);
    let ReviewDecision::Ready(tally) = review.evaluate(ExactStatus::Clear).unwrap() else {
        panic!("complete affirmative congress must be usable");
    };
    assert_eq!(tally.permit_weight, 10);
    assert_eq!(tally.hold_weight, 0);
    assert_eq!(tally.admitted_weights["alice"], 5);
    assert_eq!(tally.admitted_cohort_weights["b"], 5);
    assert_eq!(review.transcript().member_count(), 2);
}

#[test]
fn missing_reveals_and_explicit_abstentions_are_not_affirmative_votes() {
    let mut review = review(requirements());
    assert_eq!(
        review.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Hold(HoldReason::IndependentPhaseOpen))
    );
    commit_votes(&mut review, Verdict::Allow, Verdict::Allow);
    review.reveal("alice", Verdict::Allow, b"salt").unwrap();
    assert_eq!(
        review.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Hold(HoldReason::MissingReveals(vec![
            "bob".to_owned()
        ])))
    );
    review.reveal("bob", Verdict::Allow, b"salt").unwrap();
    assert!(matches!(
        review.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Ready(_))
    ));
    let abstaining = complete(requirements(), Verdict::Allow, Verdict::Abstain);
    assert_eq!(
        abstaining.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Hold(HoldReason::Abstentions(vec![
            "bob".to_owned()
        ])))
    );
}

#[test]
fn exact_unknown_and_disqualification_dominate_a_complete_majority() {
    let review = complete(requirements(), Verdict::Allow, Verdict::Allow);
    assert!(matches!(
        review.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Ready(_))
    ));
    assert_eq!(
        review.evaluate(ExactStatus::Unknown),
        Ok(ReviewDecision::Hold(HoldReason::ExactEvidenceUnknown))
    );
    assert_eq!(
        review.evaluate(ExactStatus::Disqualified),
        Ok(ReviewDecision::Disqualified)
    );
    let open = self::review(requirements());
    assert_eq!(
        open.evaluate(ExactStatus::Disqualified),
        Ok(ReviewDecision::Disqualified)
    );
}

#[test]
fn thresholds_and_cohort_diversity_are_independent_requirements() {
    let insufficient = complete(requirements(), Verdict::Allow, Verdict::Hold);
    assert_eq!(
        insufficient.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Hold(HoldReason::InsufficientPermitWeight))
    );
    let mut policy = requirements();
    policy.min_permit_weight = 5;
    policy.min_permit_cohorts = 1;
    let excessive = complete(policy, Verdict::Allow, Verdict::Deny);
    assert_eq!(
        excessive.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Hold(HoldReason::ExcessHoldWeight))
    );
    policy.max_hold_weight = 5;
    policy.min_permit_cohorts = 2;
    let correlated = complete(policy, Verdict::Allow, Verdict::Hold);
    assert_eq!(
        correlated.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Hold(HoldReason::InsufficientPermitCohorts))
    );
    policy.min_permit_cohorts = 1;
    let permitted = complete(policy, Verdict::Allow, Verdict::Deny);
    assert!(matches!(
        permitted.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Ready(_))
    ));
}

#[test]
fn invalid_or_unregistered_reveals_cannot_change_a_completed_decision() {
    let mut review = complete(requirements(), Verdict::Allow, Verdict::Allow);
    let before = review.clone();
    assert_eq!(
        review.reveal("alice", Verdict::Deny, b"salt"),
        Err(Error::Binding)
    );
    assert_eq!(
        review.reveal("mallory", Verdict::Allow, b"salt"),
        Err(Error::Missing)
    );
    assert_eq!(review, before);
    assert!(matches!(
        review.evaluate(ExactStatus::Clear),
        Ok(ReviewDecision::Ready(_))
    ));
}

#[test]
fn cohort_caps_apply_before_policy_reachability() {
    let members = vec![member("alice", "same", 100), member("bob", "same", 100)];
    let mut policy = requirements();
    policy.min_permit_cohorts = 1;
    // Each clips to 5, then proportional cohort clipping admits 3 + 3.
    assert!(ReviewPolicy::new(members.clone(), policy).is_ok());
    policy.min_permit_weight = 7;
    assert_eq!(ReviewPolicy::new(members, policy), Err(Error::InvalidInput));
}

#[test]
fn exact_registration_bounds_and_overflow_refuse_before_review() {
    let mut policy = requirements();
    policy.min_permit_cohorts = 1;
    policy.min_permit_weight = 1;
    let members: Vec<_> = (0..128)
        .map(|i| member(&format!("member-{i}"), &format!("cohort-{i}"), 1))
        .collect();
    assert!(ReviewPolicy::new(members.clone(), policy).is_ok());
    let mut too_many = members;
    too_many.push(member("extra", "extra", 1));
    assert_eq!(ReviewPolicy::new(too_many, policy), Err(Error::Limit));
    assert!(ReviewPolicy::new(vec![member(&"x".repeat(128), "c", 1)], policy).is_ok());
    assert_eq!(
        ReviewPolicy::new(vec![member(&"x".repeat(129), "c", 1)], policy),
        Err(Error::Limit)
    );
    assert_eq!(
        ReviewPolicy::new(vec![member("x", "c", 1), member("x", "d", 1)], policy),
        Err(Error::Duplicate)
    );
    policy.caps.per_member = u64::MAX;
    policy.caps.per_cohort = u64::MAX;
    assert_eq!(
        ReviewPolicy::new(
            vec![member("a", "a", u64::MAX), member("b", "b", u64::MAX)],
            policy
        ),
        Err(Error::Overflow)
    );
}
