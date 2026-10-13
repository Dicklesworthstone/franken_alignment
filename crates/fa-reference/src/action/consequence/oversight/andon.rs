//! Join a sealed independent-label campaign to actual stopped congress reviews.
//!
//! This profile uses the SAME case/round IDs, 32-byte evidence roots and control-
//! sequence domain on both sides. It does not hash an arbitrary challenge into
//! that profile, authenticate an evaluator, or infer a stop from a minority vote.
//! Independent bootstrap supplies the exact campaign and generation binding.

use super::super::congress::credibility::{
    CredibilitySnapshot, EvaluationScope, HelperGeneration, LabelVerdict, Observation, MAX_CASES,
};
use crate::Error;
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use crate::round::review::andon::{AndonLedger, LedgerIdentity, RecordResult, TrustedFalseHold};
use crate::round::review::{CongressReview, ExactStatus, HoldReason, ReviewDecision};
use crate::round::{MemberOutcome, Verdict};
use std::collections::BTreeMap;

/// Independently retained context, not values discovered from an untrusted
/// snapshot. The helper inventory includes non-holders and their cohorts too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationBinding {
    identity: LedgerIdentity,
    scope: EvaluationScope,
    label_owner: String,
    helpers: BTreeMap<String, HelperGeneration>,
}

impl EvaluationBinding {
    pub fn new(
        identity: LedgerIdentity,
        scope: EvaluationScope,
        label_owner: String,
        helpers: BTreeMap<String, HelperGeneration>,
    ) -> Result<Self, Error> {
        if [identity.deployment, identity.evaluator, scope.campaign,
            scope.model_generation, scope.evaluator_generation].contains(&0)
            || scope.held_out_manifest == [0; 32] || helpers.is_empty()
        {
            return Err(Error::InvalidInput);
        }
        if helpers.len() > MAX_VOTES { return Err(Error::Limit); }
        identifier(&label_owner)?;
        if helpers.contains_key(&label_owner) { return Err(Error::Binding); }
        for (name, generation) in &helpers {
            identifier(name)?;
            identifier(&generation.cohort)?;
            if generation.generation == 0 { return Err(Error::InvalidInput); }
        }
        Ok(Self { identity, scope, label_owner, helpers })
    }

    fn check(&self, snapshot: &CredibilitySnapshot) -> Result<(), Error> {
        if snapshot.scope() != &self.scope || snapshot.label_owner() != self.label_owner.as_str()
            || snapshot.helpers() != &self.helpers
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

fn identifier(value: &str) -> Result<(), Error> {
    if value.len() > MAX_IDENTIFIER_BYTES { return Err(Error::Limit); }
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(Error::InvalidInput);
    }
    Ok(())
}

/// Derived observation, not authority. No caller-selected label, timestamps or
/// holder generations are accepted after the independent binding is frozen.
/// Retain the source snapshot independently: this receipt is not its replacement.
///
/// ```compile_fail,E0451
/// use fa_reference::action::consequence::oversight::andon::EvaluatedFalseHold;
/// use fa_reference::round::review::andon::TrustedFalseHold;
/// fn forge(fact: TrustedFalseHold) -> EvaluatedFalseHold {
///     EvaluatedFalseHold { fact }
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluatedFalseHold { fact: TrustedFalseHold }

impl EvaluatedFalseHold {
    /// Validate BOTH independent ground truth and the actual review decision.
    /// Safe labels for ready reviews and holds after dispatch cannot charge a
    /// stopped-action budget. Pending/censored evidence is not a safe label.
    pub fn from_review(
        binding: &EvaluationBinding,
        snapshot: &CredibilitySnapshot,
        case: u64,
        review: &CongressReview,
        exact: ExactStatus,
    ) -> Result<Self, Error> {
        binding.check(snapshot)?;
        let spec = snapshot.case_spec(case).ok_or(Error::Missing)?;
        let transcript = review.transcript();
        if transcript.id() != spec.id || transcript.evidence_root() != spec.evidence_root.as_slice() {
            return Err(Error::Binding);
        }
        match review.evaluate(exact)? {
            ReviewDecision::Hold(HoldReason::InsufficientPermitWeight
                | HoldReason::ExcessHoldWeight | HoldReason::InsufficientPermitCohorts) => {}
            ReviewDecision::Hold(_) => return Err(Error::Incomplete),
            _ => return Err(Error::Binding),
        }
        let label = snapshot.case_label(case).ok_or(Error::Incomplete)?;
        match label.verdict {
            LabelVerdict::Safe => {}
            LabelVerdict::Censored => return Err(Error::Incomplete),
            LabelVerdict::Violation => return Err(Error::Binding),
        }
        let observations = snapshot.case_observations(case).ok_or(Error::Incomplete)?;
        let outcomes = transcript.outcomes();
        if !outcomes.keys().copied().eq(binding.helpers.keys().map(String::as_str))
            || !observations.keys().eq(binding.helpers.keys())
        {
            return Err(Error::Binding);
        }
        let mut helper_generations = BTreeMap::new();
        for (member, outcome) in outcomes {
            match (outcome, observations.get(member).ok_or(Error::Incomplete)?) {
                (MemberOutcome::Revealed(Verdict::Allow), Observation::Clear) => {}
                (MemberOutcome::Revealed(Verdict::Hold | Verdict::Deny),
                    Observation::Hold { first_sequence }) =>
                {
                    if *first_sequence >= spec.dispatch_sequence { return Err(Error::Stale); }
                    helper_generations.insert(member.to_owned(), binding.helpers[member].generation);
                }
                (MemberOutcome::Missing | MemberOutcome::Revealed(Verdict::Abstain), _)
                    => return Err(Error::Incomplete),
                _ => return Err(Error::Binding),
            }
        }
        if helper_generations.is_empty() { return Err(Error::Incomplete); }
        Ok(Self { fact: TrustedFalseHold {
            identity: binding.identity,
            report: snapshot.scope().campaign,
            round: spec.id,
            occurred_at: spec.dispatch_sequence,
            confirmed_at: label.recorded_sequence,
            evidence_root: spec.evidence_root.to_vec(),
            helper_generations,
        } })
    }

    /// Read-only bridge to existing durable trusted-input adapters. This does
    /// not make arbitrary TrustedFalseHold values authenticated or equivalent.
    pub fn confirmation(&self) -> &TrustedFalseHold { &self.fact }

    pub fn record(&self, ledger: &mut AndonLedger) -> Result<RecordResult, Error> {
        ledger.record_trusted(&self.fact)
    }
}

/// Atomically validate/import a bounded group in memory using ORIGINAL Andon
/// transitions. A bad later receipt cannot leave earlier charges published.
/// Exact duplicates remain free, including duplicates within the same group.
/// Disk adapters must persist the group before publishing these observations.
pub fn record_evaluated_batch(
    ledger: &mut AndonLedger,
    receipts: &[EvaluatedFalseHold],
) -> Result<Vec<RecordResult>, Error> {
    if receipts.len() > MAX_CASES { return Err(Error::Limit); }
    if receipts.is_empty() { return Ok(Vec::new()); }
    let mut candidate = ledger.clone();
    let mut results = Vec::with_capacity(receipts.len());
    for receipt in receipts { results.push(receipt.record(&mut candidate)?); }
    *ledger = candidate;
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::congress::credibility::{
        Campaign, CaseSpec, CredibilityLedger, EvaluationLabel, LabelSource,
    };
    use crate::reducer::Caps;
    use crate::round::commitment;
    use crate::round::review::{Member, Requirements, ReviewPolicy};
    use crate::round::review::andon::AndonBudget;

    fn identity() -> LedgerIdentity { LedgerIdentity { deployment: 1, evaluator: 2 } }
    fn scope() -> EvaluationScope {
        EvaluationScope { campaign: 7, model_generation: 3, evaluator_generation: 4,
            held_out_manifest: [8; 32] }
    }
    fn helpers() -> BTreeMap<String, HelperGeneration> {
        BTreeMap::from([
            ("a".into(), HelperGeneration { generation: 10, cohort: "first".into() }),
            ("b".into(), HelperGeneration { generation: 20, cohort: "second".into() }),
        ])
    }
    fn binding() -> EvaluationBinding {
        EvaluationBinding::new(identity(), scope(), "evaluator".into(), helpers()).unwrap()
    }
    fn snapshot(case: u64, label: Option<LabelVerdict>, observations: Option<[Observation; 2]>) -> CredibilitySnapshot {
        let mut ledger = CredibilityLedger::new(Campaign {
            scope: scope(), label_owner: "evaluator".into(), helpers: helpers(),
            strata: ["attack".into()].into(),
            cases: vec![CaseSpec { id: case, stratum: "attack".into(),
                evidence_root: [9; 32], dispatch_sequence: 10 }],
        }).unwrap();
        if let Some([a, b]) = observations {
            ledger.record_observations(case, BTreeMap::from([("a".into(), a), ("b".into(), b)])).unwrap();
        }
        if let Some(verdict) = label {
            ledger.record_label(case, EvaluationLabel {
                owner: "evaluator".into(), evaluator_generation: 4,
                source: LabelSource::IndependentEvaluation, evidence_root: [9; 32],
                recorded_sequence: 20, verdict,
            }).unwrap();
        }
        ledger.seal(100).unwrap()
    }
    fn review(case: u64, verdicts: [Verdict; 2], permissive: bool) -> CongressReview {
        review_root(case, verdicts, permissive, [9; 32])
    }
    fn review_root(case: u64, verdicts: [Verdict; 2], permissive: bool, root: [u8; 32]) -> CongressReview {
        let policy = ReviewPolicy::new(vec![
            Member { id: "a".into(), cohort: "first".into(), weight: 1 },
            Member { id: "b".into(), cohort: "second".into(), weight: 1 },
        ], Requirements {
            caps: Caps { per_member: 1, per_cohort: 1 },
            min_permit_weight: if permissive { 1 } else { 2 },
            max_hold_weight: if permissive { 1 } else { 0 }, min_permit_cohorts: 1,
        }).unwrap();
        let mut review = CongressReview::new(policy, case, &root).unwrap();
        for (name, verdict) in ["a", "b"].into_iter().zip(verdicts) {
            review.commit(name, commitment(case, name, &root, verdict, b"s").unwrap()).unwrap();
        }
        review.open_reveals().unwrap();
        for (name, verdict) in ["a", "b"].into_iter().zip(verdicts) {
            review.reveal(name, verdict, b"s").unwrap();
        }
        review
    }
    fn observations() -> [Observation; 2] {
        [Observation::Hold { first_sequence: 5 }, Observation::Clear]
    }
    fn receipt(case: u64) -> EvaluatedFalseHold {
        EvaluatedFalseHold::from_review(&binding(),
            &snapshot(case, Some(LabelVerdict::Safe), Some(observations())), case,
            &review(case, [Verdict::Hold, Verdict::Allow], false), ExactStatus::Clear).unwrap()
    }
    fn ledger() -> AndonLedger {
        let mut ledger = AndonLedger::new(identity()).unwrap();
        ledger.register_window(AndonBudget { window: 1, start: 0, end: 15,
            false_stops_allowed: 2, cost_per_stop: 5 }).unwrap();
        ledger.register_window(AndonBudget { window: 2, start: 15, end: 200,
            false_stops_allowed: 2, cost_per_stop: 5 }).unwrap();
        ledger
    }

    #[test]
    fn sealed_label_charges_actual_stop_in_original_window() {
        let receipt = receipt(1);
        assert_eq!(receipt.confirmation().confirmed_at, 20); // NOT the sealing position 100
        assert_eq!(receipt.confirmation().helper_generations, BTreeMap::from([("a".into(), 10)]));
        let mut ledger = ledger();
        receipt.record(&mut ledger).unwrap();
        assert_eq!(ledger.windows()[&1].false_stops, 1);
        assert_eq!(ledger.windows()[&2].false_stops, 0);
        assert!(matches!(receipt.record(&mut ledger), Ok(RecordResult::AlreadyRecorded { window: 1 })));
    }

    #[test]
    fn minority_hold_does_not_imply_stopped_action() {
        let snapshot = snapshot(1, Some(LabelVerdict::Safe), Some(observations()));
        assert!(EvaluatedFalseHold::from_review(&binding(), &snapshot, 1,
            &review(1, [Verdict::Hold, Verdict::Allow], false), ExactStatus::Clear).is_ok());
        assert_eq!(EvaluatedFalseHold::from_review(&binding(), &snapshot, 1,
            &review(1, [Verdict::Hold, Verdict::Allow], true), ExactStatus::Clear), Err(Error::Binding));
    }

    #[test]
    fn pending_censored_and_true_violations_never_become_false_holds() {
        let review = review(1, [Verdict::Hold, Verdict::Allow], false);
        for (label, error) in [(None, Error::Incomplete), (Some(LabelVerdict::Censored), Error::Incomplete),
            (Some(LabelVerdict::Violation), Error::Binding)]
        {
            assert_eq!(EvaluatedFalseHold::from_review(&binding(), &snapshot(1, label, Some(observations())),
                1, &review, ExactStatus::Clear), Err(error));
        }
    }

    #[test]
    fn unknown_exact_and_exact_disqualification_refuse() {
        let snapshot = snapshot(1, Some(LabelVerdict::Safe), Some(observations()));
        let review = review(1, [Verdict::Hold, Verdict::Allow], false);
        for (exact, error) in [(ExactStatus::Unknown, Error::Incomplete), (ExactStatus::Disqualified, Error::Binding)] {
            assert_eq!(EvaluatedFalseHold::from_review(&binding(), &snapshot, 1, &review, exact), Err(error));
        }
    }

    #[test]
    fn all_observations_not_just_holders_must_match_actual_review() {
        let review = review(1, [Verdict::Hold, Verdict::Allow], false);
        for observations in [None, Some([Observation::Clear, Observation::Clear]),
            Some([Observation::Hold { first_sequence: 5 }, Observation::Hold { first_sequence: 6 }]),
            Some([Observation::Hold { first_sequence: 5 }, Observation::Missing])]
        {
            assert!(EvaluatedFalseHold::from_review(&binding(),
                &snapshot(1, Some(LabelVerdict::Safe), observations), 1, &review, ExactStatus::Clear).is_err());
        }
    }

    #[test]
    fn exact_dispatch_boundary_and_late_holds_refuse() {
        for sequence in [10, 11] {
            assert_eq!(EvaluatedFalseHold::from_review(&binding(),
                &snapshot(1, Some(LabelVerdict::Safe), Some([
                    Observation::Hold { first_sequence: sequence }, Observation::Clear])),
                1, &review(1, [Verdict::Hold, Verdict::Allow], false), ExactStatus::Clear), Err(Error::Stale));
        }
    }

    #[test]
    fn independently_bound_campaign_owner_and_every_generation_are_checked() {
        let snapshot = snapshot(1, Some(LabelVerdict::Safe), Some(observations()));
        let review = review(1, [Verdict::Hold, Verdict::Allow], false);
        for field in 0..9 {
            let mut changed = binding();
            match field {
                0 => changed.scope.campaign += 1,
                1 => changed.scope.model_generation += 1,
                2 => changed.scope.evaluator_generation += 1,
                3 => changed.scope.held_out_manifest[0] ^= 1,
                4 => changed.label_owner.push('x'),
                5 => changed.helpers.get_mut("a").unwrap().generation += 1,
                6 => changed.helpers.get_mut("b").unwrap().generation += 1,
                7 => changed.helpers.get_mut("b").unwrap().cohort.push('x'),
                _ => { changed.helpers.remove("b"); }
            }
            assert_eq!(EvaluatedFalseHold::from_review(&changed, &snapshot, 1, &review, ExactStatus::Clear), Err(Error::Binding));
        }
    }

    #[test]
    fn case_round_and_full_evidence_root_must_match() {
        let snapshot = snapshot(1, Some(LabelVerdict::Safe), Some(observations()));
        assert_eq!(EvaluatedFalseHold::from_review(&binding(), &snapshot, 1,
            &review(2, [Verdict::Hold, Verdict::Allow], false), ExactStatus::Clear), Err(Error::Binding));
        let good = review(1, [Verdict::Hold, Verdict::Allow], false);
        assert_eq!(EvaluatedFalseHold::from_review(&binding(), &snapshot, 2, &good, ExactStatus::Clear), Err(Error::Missing));
        assert_eq!(EvaluatedFalseHold::from_review(&binding(), &snapshot, 1,
            &review_root(1, [Verdict::Hold, Verdict::Allow], false, [8; 32]), ExactStatus::Clear), Err(Error::Binding));
    }

    #[test]
    fn group_is_atomic_and_duplicates_are_free() {
        let mut ledger = ledger();
        let one = receipt(1);
        let results = record_evaluated_batch(&mut ledger, &[one.clone(), one.clone(), receipt(2)]).unwrap();
        assert_eq!(results.len(), 3);
        assert!(matches!(results[1], RecordResult::AlreadyRecorded { .. }));
        assert_eq!(ledger.windows()[&1].false_stops, 2);
        assert_eq!(ledger.windows()[&1].incident.as_ref().unwrap().rounds.len(), 2);
        let before = ledger.clone();
        let mut foreign = receipt(3);
        foreign.fact.identity.deployment += 1;
        assert_eq!(record_evaluated_batch(&mut ledger, &[receipt(4), foreign]), Err(Error::Binding));
        assert_eq!(ledger, before);
        assert_eq!(record_evaluated_batch(&mut ledger, &vec![one; MAX_CASES + 1]), Err(Error::Limit));
        assert_eq!(ledger, before);
    }
}
