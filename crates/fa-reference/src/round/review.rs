//! Frozen congress policy and transcript-to-reducer integration.
//!
//! A bounded reference profile, not production consensus or authentication.
//! The enclosing round still uses its non-cryptographic comparison oracle.
//! Membership, cohorts, weights and thresholds freeze before any commitment.
//! Missing reveals and abstentions cannot become affirmative observations.

use super::{Digest, MemberOutcome, Phase, Round, Verdict};
use crate::Error;
use crate::reducer::{self, Caps, Recommendation, Reduction, Vote};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub id: String,
    pub cohort: String,
    pub weight: u64,
}

/// A registered reference policy, not inferred statistical independence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirements {
    pub caps: Caps,
    pub min_permit_weight: u64,
    pub max_hold_weight: u64,
    pub min_permit_cohorts: usize,
}

/// Immutable policy. There is deliberately no post-commit reweighting API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewPolicy {
    members: BTreeMap<String, Member>,
    requirements: Requirements,
}

impl ReviewPolicy {
    pub fn new(members: Vec<Member>, requirements: Requirements) -> Result<Self, Error> {
        if members.is_empty() {
            return Err(Error::Missing);
        }
        if members.len() > reducer::MAX_VOTES {
            return Err(Error::Limit);
        }
        if requirements.caps.per_member == 0
            || requirements.caps.per_cohort == 0
            || requirements.min_permit_weight == 0
            || requirements.min_permit_cohorts == 0
        {
            return Err(Error::InvalidInput);
        }
        let mut registered = BTreeMap::new();
        for member in members {
            if member.id.is_empty() || member.cohort.is_empty() {
                return Err(Error::InvalidInput);
            }
            if member.id.len() > reducer::MAX_IDENTIFIER_BYTES
                || member.cohort.len() > reducer::MAX_IDENTIFIER_BYTES
            {
                return Err(Error::Limit);
            }
            if registered.insert(member.id.clone(), member).is_some() {
                return Err(Error::Duplicate);
            }
        }
        let policy = Self {
            members: registered,
            requirements,
        };
        // Check reachability after BOTH influence caps, not raw member weights.
        // This also refuses policies whose complete tally would overflow u64.
        let votes = policy.votes(|_| Recommendation::Permit);
        let maximum = reducer::reduce(&votes, &policy.weights(), requirements.caps, false)?;
        let active_cohorts = maximum
            .admitted_cohort_weights
            .values()
            .filter(|weight| **weight > 0)
            .count();
        if requirements.min_permit_weight > maximum.permit_weight
            || requirements.min_permit_cohorts > active_cohorts
        {
            return Err(Error::InvalidInput);
        }
        Ok(policy)
    }

    pub fn requirements(&self) -> Requirements {
        self.requirements
    }

    pub(crate) fn members(&self) -> impl ExactSizeIterator<Item = &Member> {
        self.members.values()
    }

    fn weights(&self) -> BTreeMap<String, u64> {
        self.members
            .iter()
            .map(|(id, member)| (id.clone(), member.weight))
            .collect()
    }

    fn votes(&self, mut recommendation: impl FnMut(&str) -> Recommendation) -> Vec<Vote> {
        self.members
            .iter()
            .map(|(id, member)| Vote::Empirical {
                member: id.clone(),
                cohort: member.cohort.clone(),
                recommendation: recommendation(id),
            })
            .collect()
    }
}

/// Caller-supplied exact-validator status, separate from every empirical vote.
/// `Clear` is a declared reference fact, NOT proof that a validator ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExactStatus {
    Clear,
    Disqualified,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HoldReason {
    ExactEvidenceUnknown,
    IndependentPhaseOpen,
    MissingReveals(Vec<String>),
    Abstentions(Vec<String>),
    InsufficientPermitWeight,
    ExcessHoldWeight,
    InsufficientPermitCohorts,
}

/// A diagnostic decision, never an authority-bearing permit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewDecision {
    Ready(Reduction),
    Hold(HoldReason),
    Disqualified,
}

/// Owns the only mutable transcript for a pre-registered policy and evidence view.
/// A clone is merely another reference experiment; it conveys no action rights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CongressReview {
    policy: ReviewPolicy,
    round: Round,
}

impl CongressReview {
    pub fn new(policy: ReviewPolicy, round_id: u64, evidence_root: &[u8]) -> Result<Self, Error> {
        if round_id == 0 {
            return Err(Error::InvalidInput);
        }
        let mut round = Round::new(round_id, evidence_root)?;
        for member in policy.members.keys() {
            round.add_member(member)?;
        }
        Ok(Self { policy, round })
    }

    pub fn transcript(&self) -> &Round {
        &self.round
    }

    pub fn commit(&mut self, member: &str, digest: Digest) -> Result<(), Error> {
        self.round.commit(member, digest)
    }

    pub fn open_reveals(&mut self) -> Result<(), Error> {
        self.round.open_reveals()
    }

    pub fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8]) -> Result<(), Error> {
        self.round.reveal(member, verdict, salt)
    }

    pub fn evaluate(&self, exact: ExactStatus) -> Result<ReviewDecision, Error> {
        match exact {
            ExactStatus::Disqualified => return Ok(ReviewDecision::Disqualified),
            ExactStatus::Unknown => {
                return Ok(ReviewDecision::Hold(HoldReason::ExactEvidenceUnknown));
            }
            ExactStatus::Clear => {}
        }
        if self.round.phase() != Phase::Reveal {
            return Ok(ReviewDecision::Hold(HoldReason::IndependentPhaseOpen));
        }
        let mut missing = Vec::new();
        let mut abstaining = Vec::new();
        let mut recommendations = BTreeMap::new();
        for member in self.policy.members.keys() {
            match self.round.outcome(member)? {
                MemberOutcome::Missing => missing.push(member.clone()),
                MemberOutcome::Revealed(Verdict::Abstain) => abstaining.push(member.clone()),
                MemberOutcome::Revealed(Verdict::Allow) => {
                    recommendations.insert(member.as_str(), Recommendation::Permit);
                }
                MemberOutcome::Revealed(Verdict::Hold | Verdict::Deny) => {
                    // An empirical Deny is not an exact disqualifier.
                    recommendations.insert(member.as_str(), Recommendation::Hold);
                }
            }
        }
        if !missing.is_empty() {
            return Ok(ReviewDecision::Hold(HoldReason::MissingReveals(missing)));
        }
        if !abstaining.is_empty() {
            return Ok(ReviewDecision::Hold(HoldReason::Abstentions(abstaining)));
        }
        let votes = self.policy.votes(|member| recommendations[member]);
        let requirements = self.policy.requirements;
        let reduction = reducer::reduce(&votes, &self.policy.weights(), requirements.caps, false)?;
        if reduction.permit_weight < requirements.min_permit_weight {
            return Ok(ReviewDecision::Hold(HoldReason::InsufficientPermitWeight));
        }
        if reduction.hold_weight > requirements.max_hold_weight {
            return Ok(ReviewDecision::Hold(HoldReason::ExcessHoldWeight));
        }
        let permit_cohorts: BTreeSet<_> = self
            .policy
            .members
            .iter()
            .filter(|(id, _)| {
                recommendations[id.as_str()] == Recommendation::Permit
                    && reduction.admitted_weights[*id] > 0
            })
            .map(|(_, member)| member.cohort.as_str())
            .collect();
        if permit_cohorts.len() < requirements.min_permit_cohorts {
            return Ok(ReviewDecision::Hold(HoldReason::InsufficientPermitCohorts));
        }
        Ok(ReviewDecision::Ready(reduction))
    }
}

/// False-stop accounting (plan §9.9). These are observations, never authority.
/// Independent evaluator authentication and clock provenance remain caller facts.
/// Exhaustion opens an incident; it cannot reweight, remove or bypass a reviewer.
pub mod andon {
    use super::{CongressReview, ExactStatus, HoldReason, ReviewDecision};
    use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
    use crate::round::{MemberOutcome, Verdict, MAX_FIELD_LEN};
    use crate::Error;
    use std::collections::{BTreeMap, BTreeSet};

    pub const MAX_WINDOWS: usize = 128;
    pub const MAX_FALSE_HOLDS: usize = 4_096;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct LedgerIdentity {
        pub deployment: u64,
        pub evaluator: u64,
    }

    /// Immutable, half-open occurrence-time window in one trusted tick domain.
    /// Late confirmations are charged to this window, not the arrival window.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct AndonBudget {
        pub window: u64,
        pub start: u64,
        pub end: u64,
        pub false_stops_allowed: u64,
        pub cost_per_stop: u64,
    }

    /// An independently confirmed *false* hold. A pending/censored label or a
    /// confirmed violation must never be supplied as this trusted input.
    /// Round identifiers must be unique within the deployment across restarts.
    /// Multiple holding helpers in one round still describe ONE stopped action.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct TrustedFalseHold {
        pub identity: LedgerIdentity,
        pub report: u64,
        pub round: u64,
        pub occurred_at: u64,
        pub confirmed_at: u64,
        /// Exact frozen evidence/challenge bytes, not an unbound short digest.
        pub evidence_root: Vec<u8>,
        pub helper_generations: BTreeMap<String, u64>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CalibrationIncident {
        pub window: u64,
        /// First confirmation that reached the allowance (first stop for zero).
        pub opened_by_round: u64,
        /// Includes contributors before exhaustion and every later false hold.
        pub rounds: BTreeSet<u64>,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct WindowAccounting {
        pub budget: AndonBudget,
        pub false_stops: u64,
        pub total_cost: u128,
        pub incident: Option<CalibrationIncident>,
        rounds: BTreeSet<u64>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum RecordResult {
        Recorded { window: u64, incident_opened: bool },
        AlreadyRecorded { window: u64 },
    }

    /// Bounded observational state. Cloning it copies no rights or policy.
    /// There is no reset, refund, reviewer removal or threshold mutation API.
    /// Retain/replay its records in a trusted durable store before acknowledging
    /// confirmations; constructing a new instance is NOT recovery.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::round::review::andon::AndonLedger;
    /// fn bypass(ledger: &mut AndonLedger) { ledger.policy_mut(); }
    /// ```
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct AndonLedger {
        identity: LedgerIdentity,
        windows: BTreeMap<u64, WindowAccounting>,
        confirmations: BTreeMap<u64, (u64, TrustedFalseHold)>,
    }

    impl AndonLedger {
        pub fn new(identity: LedgerIdentity) -> Result<Self, Error> {
            if identity.deployment == 0 || identity.evaluator == 0 {
                return Err(Error::InvalidInput);
            }
            Ok(Self {
                identity,
                windows: BTreeMap::new(),
                confirmations: BTreeMap::new(),
            })
        }

        pub fn identity(&self) -> LedgerIdentity {
            self.identity
        }

        pub fn windows(&self) -> &BTreeMap<u64, WindowAccounting> {
            &self.windows
        }

        pub fn confirmation(&self, round: u64) -> Option<&TrustedFalseHold> {
            self.confirmations.get(&round).map(|(_, confirmation)| confirmation)
        }

        /// True on insertion, false on an exact retry, including at capacity.
        /// A changed or overlapping window cannot erase historical accounting.
        pub fn register_window(&mut self, budget: AndonBudget) -> Result<bool, Error> {
            if let Some(existing) = self.windows.get(&budget.window) {
                return if existing.budget == budget { Ok(false) } else { Err(Error::Binding) };
            }
            if budget.window == 0 || budget.start >= budget.end {
                return Err(Error::InvalidInput);
            }
            if self.windows.len() >= MAX_WINDOWS {
                return Err(Error::Limit);
            }
            if self.windows.values().any(|existing| {
                budget.start < existing.budget.end && existing.budget.start < budget.end
            }) {
                return Err(Error::Binding);
            }
            self.windows.insert(budget.window, WindowAccounting {
                budget,
                false_stops: 0,
                total_cost: 0,
                incident: None,
                rounds: BTreeSet::new(),
            });
            Ok(true)
        }

        /// Integrate a completed congress transcript with a trusted false label.
        /// Missing reveals, abstentions, unknown exact evidence, disqualifiers
        /// and ready-to-dispatch reviews are not confirmed empirical false stops.
        /// The confirmation must name exactly the holding/denying helpers.
        pub fn record_review(
            &mut self,
            review: &CongressReview,
            exact: ExactStatus,
            confirmation: &TrustedFalseHold,
        ) -> Result<RecordResult, Error> {
            if review.transcript().id() != confirmation.round
                || review.transcript().evidence_root() != confirmation.evidence_root.as_slice()
            {
                return Err(Error::Binding);
            }
            match review.evaluate(exact)? {
                ReviewDecision::Hold(
                    HoldReason::InsufficientPermitWeight
                    | HoldReason::ExcessHoldWeight
                    | HoldReason::InsufficientPermitCohorts,
                ) => {}
                ReviewDecision::Hold(_) => return Err(Error::Incomplete),
                _ => return Err(Error::Binding),
            }
            let holders: BTreeSet<_> = review.transcript().outcomes().into_iter()
                .filter_map(|(member, outcome)| {
                    matches!(outcome, MemberOutcome::Revealed(Verdict::Hold | Verdict::Deny))
                        .then_some(member)
                })
                .collect();
            if holders.iter().copied().ne(confirmation.helper_generations.keys().map(String::as_str)) {
                return Err(Error::Binding);
            }
            self.record_trusted(confirmation)
        }

        /// Trusted evaluator/replay entry point, not an evaluator authenticator.
        /// Replaying the identical round is free; any changed identity, report,
        /// timestamp or generation attribution for that round refuses atomically.
        pub fn record_trusted(&mut self, confirmation: &TrustedFalseHold) -> Result<RecordResult, Error> {
            if confirmation.identity != self.identity {
                return Err(Error::Binding);
            }
            if let Some((window, existing)) = self.confirmations.get(&confirmation.round) {
                return if existing == confirmation {
                    Ok(RecordResult::AlreadyRecorded { window: *window })
                } else {
                    Err(Error::Binding)
                };
            }
            if confirmation.report == 0 || confirmation.round == 0
                || confirmation.confirmed_at < confirmation.occurred_at
                || confirmation.helper_generations.is_empty()
                || confirmation.evidence_root.is_empty()
            {
                return Err(Error::InvalidInput);
            }
            if self.confirmations.len() >= MAX_FALSE_HOLDS
                || confirmation.helper_generations.len() > MAX_VOTES
                || confirmation.evidence_root.len() > MAX_FIELD_LEN
                || confirmation.helper_generations.keys().any(|id| id.len() > MAX_IDENTIFIER_BYTES)
            {
                return Err(Error::Limit);
            }
            if confirmation.helper_generations.iter().any(|(id, generation)| id.is_empty() || *generation == 0) {
                return Err(Error::InvalidInput);
            }
            let window = self.windows.values().find(|state| {
                state.budget.start <= confirmation.occurred_at && confirmation.occurred_at < state.budget.end
            }).map(|state| state.budget.window).ok_or(Error::Missing)?;
            let state = &self.windows[&window];
            let count = state.false_stops.checked_add(1).ok_or(Error::Overflow)?;
            let cost = state.total_cost.checked_add(u128::from(state.budget.cost_per_stop))
                .ok_or(Error::Overflow)?;
            let incident_opened = state.incident.is_none() && count >= state.budget.false_stops_allowed;
            // All fallible semantic checks precede either map mutation.
            let state = self.windows.get_mut(&window).expect("selected registered window");
            state.false_stops = count;
            state.total_cost = cost;
            state.rounds.insert(confirmation.round);
            if incident_opened {
                state.incident = Some(CalibrationIncident {
                    window,
                    opened_by_round: confirmation.round,
                    rounds: state.rounds.clone(),
                });
            } else if let Some(incident) = &mut state.incident {
                incident.rounds.insert(confirmation.round);
            }
            self.confirmations.insert(confirmation.round, (window, confirmation.clone()));
            Ok(RecordResult::Recorded { window, incident_opened })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use super::super::{Member, Requirements, ReviewPolicy};
        use crate::reducer::Caps;
        use crate::round::commitment;

        fn identity() -> LedgerIdentity { LedgerIdentity { deployment: 1, evaluator: 2 } }
        fn budget() -> AndonBudget {
            AndonBudget { window: 7, start: 10, end: 20, false_stops_allowed: 2, cost_per_stop: u64::MAX }
        }
        fn confirmation(round: u64) -> TrustedFalseHold {
            TrustedFalseHold {
                identity: identity(), report: 100 + round, round, occurred_at: 12, confirmed_at: 30,
                evidence_root: b"evidence".to_vec(),
                helper_generations: BTreeMap::from([("helper".into(), 1)]),
            }
        }
        fn ledger() -> AndonLedger {
            let mut ledger = AndonLedger::new(identity()).unwrap();
            ledger.register_window(budget()).unwrap();
            ledger
        }
        fn review(verdict: Verdict, reveal: bool) -> CongressReview {
            let policy = ReviewPolicy::new(vec![Member { id: "helper".into(), cohort: "c".into(), weight: 1 }],
                Requirements { caps: Caps { per_member: 1, per_cohort: 1 }, min_permit_weight: 1,
                    max_hold_weight: 0, min_permit_cohorts: 1 }).unwrap();
            let mut review = CongressReview::new(policy, 1, b"evidence").unwrap();
            review.commit("helper", commitment(1, "helper", b"evidence", verdict, b"salt").unwrap()).unwrap();
            review.open_reveals().unwrap();
            if reveal { review.reveal("helper", verdict, b"salt").unwrap(); }
            review
        }

        #[test]
        fn exhaustion_opens_once_and_retains_every_contributor() {
            let mut ledger = ledger();
            assert_eq!(ledger.record_trusted(&confirmation(1)), Ok(RecordResult::Recorded { window: 7, incident_opened: false }));
            assert_eq!(ledger.record_trusted(&confirmation(2)), Ok(RecordResult::Recorded { window: 7, incident_opened: true }));
            assert_eq!(ledger.record_trusted(&confirmation(3)), Ok(RecordResult::Recorded { window: 7, incident_opened: false }));
            let state = &ledger.windows()[&7];
            assert_eq!(state.false_stops, 3);
            assert_eq!(state.total_cost, 3 * u128::from(u64::MAX));
            let incident = state.incident.as_ref().unwrap();
            assert_eq!(incident.opened_by_round, 2);
            assert_eq!(incident.rounds, BTreeSet::from([1, 2, 3]));
        }

        #[test]
        fn exact_retry_is_free_but_relabeling_refuses_without_mutation() {
            let mut ledger = ledger();
            ledger.record_trusted(&confirmation(1)).unwrap();
            let before = ledger.clone();
            assert_eq!(ledger.record_trusted(&confirmation(1)), Ok(RecordResult::AlreadyRecorded { window: 7 }));
            for field in 0..6 {
                let mut changed = confirmation(1);
                match field {
                    0 => changed.report += 1,
                    1 => changed.occurred_at += 1,
                    2 => changed.confirmed_at += 1,
                    3 => changed.identity.evaluator += 1,
                    4 => { changed.helper_generations.insert("helper".into(), 2); }
                    _ => changed.evidence_root.push(0),
                }
                assert_eq!(ledger.record_trusted(&changed), Err(Error::Binding));
                assert_eq!(ledger, before);
            }
        }

        #[test]
        fn delayed_labels_and_rotation_do_not_reset_old_windows() {
            let mut ledger = ledger();
            ledger.register_window(AndonBudget { window: 8, start: 20, end: 40, ..budget() }).unwrap();
            ledger.record_trusted(&confirmation(1)).unwrap();
            let mut rotated = confirmation(2);
            rotated.helper_generations.insert("helper".into(), 2);
            ledger.record_trusted(&rotated).unwrap();
            assert_eq!(ledger.windows()[&7].false_stops, 2);
            assert_eq!(ledger.windows()[&8].false_stops, 0);
            assert_eq!(ledger.confirmation(1).unwrap().helper_generations["helper"], 1);
            assert_eq!(ledger.confirmation(2).unwrap().helper_generations["helper"], 2);
        }

        #[test]
        fn half_open_windows_and_zero_allowance_are_explicit() {
            let mut ledger = ledger();
            ledger.register_window(AndonBudget { window: 8, start: 20, end: 40, false_stops_allowed: 0, ..budget() }).unwrap();
            let mut at_boundary = confirmation(1);
            at_boundary.occurred_at = 20;
            assert_eq!(ledger.record_trusted(&at_boundary), Ok(RecordResult::Recorded { window: 8, incident_opened: true }));
            assert_eq!(ledger.windows()[&7].false_stops, 0);
            at_boundary.round = 2;
            at_boundary.occurred_at = 40;
            at_boundary.confirmed_at = 40;
            let before = ledger.clone();
            assert_eq!(ledger.record_trusted(&at_boundary), Err(Error::Missing));
            assert_eq!(ledger, before);
        }

        #[test]
        fn window_reconfiguration_cannot_erase_or_overlap_history() {
            let mut ledger = ledger();
            ledger.record_trusted(&confirmation(1)).unwrap();
            let before = ledger.clone();
            assert_eq!(ledger.register_window(budget()), Ok(false));
            assert_eq!(ledger.register_window(AndonBudget { false_stops_allowed: 100, ..budget() }), Err(Error::Binding));
            assert_eq!(ledger.register_window(AndonBudget { window: 8, ..budget() }), Err(Error::Binding));
            assert_eq!(ledger, before);
        }

        #[test]
        fn transcript_integration_cannot_weaken_congress_policy() {
            let mut ledger = AndonLedger::new(identity()).unwrap();
            ledger.register_window(AndonBudget { false_stops_allowed: 1, ..budget() }).unwrap();
            let review = review(Verdict::Hold, true);
            let before = review.clone();
            ledger.record_review(&review, ExactStatus::Clear, &confirmation(1)).unwrap();
            assert_eq!(review, before);
            assert!(ledger.windows()[&7].incident.is_some());
            assert!(matches!(review.evaluate(ExactStatus::Clear).unwrap(), ReviewDecision::Hold(_)));
            let mut wrong_helpers = confirmation(2);
            wrong_helpers.round = 1;
            wrong_helpers.helper_generations.insert("outsider".into(), 1);
            let before = ledger.clone();
            assert_eq!(ledger.record_review(&review, ExactStatus::Clear, &wrong_helpers), Err(Error::Binding));
            assert_eq!(ledger, before);
        }

        #[test]
        fn unknown_exact_missing_abstaining_and_ready_are_not_false_holds() {
            for (verdict, reveal, exact) in [
                (Verdict::Hold, true, ExactStatus::Unknown),
                (Verdict::Hold, true, ExactStatus::Disqualified),
                (Verdict::Hold, false, ExactStatus::Clear),
                (Verdict::Abstain, true, ExactStatus::Clear),
                (Verdict::Allow, true, ExactStatus::Clear),
            ] {
                let mut ledger = ledger();
                let before = ledger.clone();
                assert!(ledger.record_review(&review(verdict, reveal), exact, &confirmation(1)).is_err());
                assert_eq!(ledger, before);
            }
        }

        #[test]
        fn multiple_holding_helpers_count_one_stop() {
            let mut ledger = ledger();
            let mut fact = confirmation(1);
            fact.helper_generations.insert("peer".into(), 3);
            ledger.record_trusted(&fact).unwrap();
            assert_eq!(ledger.windows()[&7].false_stops, 1);
            assert_eq!(ledger.confirmation(1).unwrap().helper_generations.len(), 2);
        }

        #[test]
        fn exhausted_capacity_preserves_retries_and_rejects_new_records() {
            let mut ledger = ledger();
            for id in 1..=MAX_FALSE_HOLDS as u64 { ledger.record_trusted(&confirmation(id)).unwrap(); }
            let before = ledger.clone();
            assert_eq!(ledger.record_trusted(&confirmation(1)), Ok(RecordResult::AlreadyRecorded { window: 7 }));
            assert_eq!(ledger.record_trusted(&confirmation(MAX_FALSE_HOLDS as u64 + 1)), Err(Error::Limit));
            assert_eq!(ledger, before);
        }

        #[test]
        fn malformed_evaluator_facts_are_atomic_refusals() {
            for case in 0..7 {
                let mut fact = confirmation(1);
                match case {
                    0 => fact.report = 0,
                    1 => fact.round = 0,
                    2 => fact.confirmed_at = fact.occurred_at - 1,
                    3 => fact.helper_generations.clear(),
                    4 => { fact.helper_generations.insert("helper".into(), 0); }
                    5 => { fact.helper_generations.insert("".into(), 1); }
                    _ => fact.evidence_root.clear(),
                }
                let mut ledger = ledger();
                let before = ledger.clone();
                assert_eq!(ledger.record_trusted(&fact), Err(Error::InvalidInput));
                assert_eq!(ledger, before);
            }
        }
    }
}
