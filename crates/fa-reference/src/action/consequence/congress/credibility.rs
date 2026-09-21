//! Independent-label, offline helper credibility (plan section 9.9).
//!
//! This is a bounded logical oracle, not a calibrated estimator or a label
//! authenticator. The caller supplies the independently owned held-out manifest,
//! evaluator identity, cohort assignments and control-sequence facts. Neither
//! committee consensus nor a helper's own label is admissible ground truth.
//! Cases and helper generations are frozen before observations arrive. Missing,
//! censored and delayed outcomes remain explicit; minority votes are not errors.
//! Sealing consumes the ledger and never modifies a live congress policy.

use crate::Error;
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_CASES: usize = 4096;
pub const MAX_STRATA: usize = 64;

/// All sequence positions in a campaign belong to this declared scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationScope {
    pub campaign: u64,
    pub model_generation: u64,
    pub evaluator_generation: u64,
    pub held_out_manifest: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperGeneration {
    pub generation: u64,
    pub cohort: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseSpec {
    pub id: u64,
    pub stratum: String,
    pub evidence_root: [u8; 32],
    /// The counterfactual dispatch position, not the time a label arrived.
    pub dispatch_sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Campaign {
    pub scope: EvaluationScope,
    pub label_owner: String,
    pub helpers: BTreeMap<String, HelperGeneration>,
    pub strata: BTreeSet<String>,
    pub cases: Vec<CaseSpec>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Clear,
    Hold { first_sequence: u64 },
    Abstain,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelSource {
    IndependentEvaluation,
    CommitteeConsensus,
    HelperSelfReport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelVerdict {
    Safe,
    Violation,
    Censored,
}

/// A trusted evaluator fact, not a signature or an authorization token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluationLabel {
    pub owner: String,
    pub evaluator_generation: u64,
    pub source: LabelSource,
    pub evidence_root: [u8; 32],
    pub recorded_sequence: u64,
    pub verdict: LabelVerdict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CaseRecord {
    spec: CaseSpec,
    observations: Option<BTreeMap<String, Observation>>,
    label: Option<EvaluationLabel>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CredibilityLedger {
    scope: EvaluationScope,
    label_owner: String,
    helpers: BTreeMap<String, HelperGeneration>,
    strata: BTreeSet<String>,
    cases: BTreeMap<u64, CaseRecord>,
    last_sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ratio {
    pub numerator: u64,
    pub denominator: u64,
}

/// Descriptive counts for ONE helper generation and ONE declared stratum.
/// No population extrapolation, confidence bound or propensity correction is
/// implied. Pending/censored cases do not silently enter the safe denominator.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CredibilityScore {
    pub cases: u64,
    pub pending: u64,
    pub censored: u64,
    pub missing: u64,
    pub abstained: u64,
    pub safe: u64,
    pub violations: u64,
    pub true_positives: u64,
    pub false_positives: u64,
    pub true_negatives: u64,
    pub false_negatives: u64,
    pub timely_true_positives: u64,
    pub lead_time_credit: u128,
}

impl CredibilityScore {
    pub fn precision(&self) -> Option<Ratio> {
        ratio(self.true_positives, self.true_positives.checked_add(self.false_positives)?)
    }

    pub fn recall(&self) -> Option<Ratio> {
        ratio(self.true_positives, self.violations)
    }

    pub fn timely_recall(&self) -> Option<Ratio> {
        ratio(self.timely_true_positives, self.violations)
    }

    pub fn coverage(&self) -> Option<Ratio> {
        ratio(self.cases.checked_sub(self.missing)?.checked_sub(self.abstained)?, self.cases)
    }
}

fn ratio(numerator: u64, denominator: u64) -> Option<Ratio> {
    (denominator != 0).then_some(Ratio { numerator, denominator })
}

/// Immutable, sealed evidence. It has no permit-minting or live-update API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredibilitySnapshot {
    scope: EvaluationScope,
    label_owner: String,
    records: BTreeMap<u64, CaseRecord>,
    oldest_case_sequence: u64,
    helpers: BTreeMap<String, HelperGeneration>,
    strata: BTreeSet<String>,
    scores: BTreeMap<String, BTreeMap<String, CredibilityScore>>,
    sealed_sequence: u64,
}

impl CredibilitySnapshot {
    pub fn label_owner(&self) -> &str {
        &self.label_owner
    }

    /// Freshness is measured from evidence, not a caller-selected sealing time.
    pub fn oldest_case_sequence(&self) -> u64 {
        self.oldest_case_sequence
    }

    pub fn case_spec(&self, case: u64) -> Option<&CaseSpec> {
        self.records.get(&case).map(|record| &record.spec)
    }

    pub fn case_label(&self, case: u64) -> Option<&EvaluationLabel> {
        self.records.get(&case).and_then(|record| record.label.as_ref())
    }

    pub fn case_observations(&self, case: u64) -> Option<&BTreeMap<String, Observation>> {
        self.records.get(&case).and_then(|record| record.observations.as_ref())
    }

    pub fn scope(&self) -> &EvaluationScope {
        &self.scope
    }

    pub fn helpers(&self) -> &BTreeMap<String, HelperGeneration> {
        &self.helpers
    }

    pub fn strata(&self) -> &BTreeSet<String> {
        &self.strata
    }

    pub fn scores(&self) -> &BTreeMap<String, BTreeMap<String, CredibilityScore>> {
        &self.scores
    }

    pub fn sealed_sequence(&self) -> u64 {
        self.sealed_sequence
    }
}

fn identifier(value: &str) -> Result<(), Error> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(Error::InvalidInput);
    }
    if value.len() > MAX_IDENTIFIER_BYTES {
        return Err(Error::Limit);
    }
    Ok(())
}

impl CredibilityLedger {
    pub fn new(campaign: Campaign) -> Result<Self, Error> {
        let Campaign { scope, label_owner, helpers, strata, cases } = campaign;
        if helpers.len() > MAX_VOTES || strata.len() > MAX_STRATA || cases.len() > MAX_CASES {
            return Err(Error::Limit);
        }
        if scope.campaign == 0 || scope.model_generation == 0
            || scope.evaluator_generation == 0 || scope.held_out_manifest == [0; 32]
            || helpers.is_empty() || strata.is_empty() || cases.is_empty()
        {
            return Err(Error::InvalidInput);
        }
        identifier(&label_owner)?;
        if helpers.contains_key(&label_owner) {
            return Err(Error::Binding);
        }
        for (helper, profile) in &helpers {
            identifier(helper)?;
            identifier(&profile.cohort)?;
            if profile.generation == 0 {
                return Err(Error::InvalidInput);
            }
        }
        for stratum in &strata {
            identifier(stratum)?;
        }
        let mut records = BTreeMap::new();
        let mut roots = BTreeSet::new();
        let mut represented = BTreeSet::new();
        let mut last_sequence = 0;
        for spec in cases {
            if spec.id == 0 || spec.dispatch_sequence == 0 || spec.evidence_root == [0; 32] {
                return Err(Error::InvalidInput);
            }
            if !strata.contains(&spec.stratum) {
                return Err(Error::Binding);
            }
            if records.contains_key(&spec.id) || !roots.insert(spec.evidence_root) {
                return Err(Error::Duplicate);
            }
            represented.insert(spec.stratum.clone());
            last_sequence = last_sequence.max(spec.dispatch_sequence);
            records.insert(spec.id, CaseRecord { spec, observations: None, label: None });
        }
        if represented != strata {
            return Err(Error::Incomplete);
        }
        Ok(Self { scope, label_owner, helpers, strata, cases: records, last_sequence })
    }

    /// One immutable report vector per predeclared case. Every registered helper
    /// must be named, including missing and abstaining helpers. No post-label
    /// backfilling of predictions is allowed.
    pub fn record_observations(
        &mut self,
        case: u64,
        observations: BTreeMap<String, Observation>,
    ) -> Result<(), Error> {
        if observations.len() > MAX_VOTES {
            return Err(Error::Limit);
        }
        let record = self.cases.get(&case).ok_or(Error::Missing)?;
        if record.label.is_some() || record.observations.is_some() {
            return Err(Error::WrongState);
        }
        if !observations.keys().eq(self.helpers.keys()) {
            return Err(Error::Binding);
        }
        let mut last_sequence = self.last_sequence;
        for observation in observations.values() {
            if let Observation::Hold { first_sequence } = observation {
                if *first_sequence == 0 {
                    return Err(Error::InvalidInput);
                }
                last_sequence = last_sequence.max(*first_sequence);
            }
        }
        self.cases.get_mut(&case).ok_or(Error::Missing)?.observations = Some(observations);
        self.last_sequence = last_sequence;
        Ok(())
    }

    /// Censoring may be resolved by a later independent label. Final labels are
    /// immutable; changing ground truth requires a separate versioned campaign.
    /// Validation precedes mutation, including for rejected censor resolutions.
    pub fn record_label(&mut self, case: u64, label: EvaluationLabel) -> Result<(), Error> {
        let record = self.cases.get(&case).ok_or(Error::Missing)?;
        if label.source != LabelSource::IndependentEvaluation
            || label.owner != self.label_owner
            || label.evaluator_generation != self.scope.evaluator_generation
            || label.evidence_root != record.spec.evidence_root
        {
            return Err(Error::Binding);
        }
        let latest_observation = record.observations.iter().flat_map(|o| o.values())
            .filter_map(|o| match o {
                Observation::Hold { first_sequence } => Some(*first_sequence),
                _ => None,
            }).max().unwrap_or(0).max(record.spec.dispatch_sequence);
        if label.recorded_sequence < latest_observation {
            return Err(Error::Stale);
        }
        if let Some(previous) = &record.label {
            if previous.verdict != LabelVerdict::Censored || label.verdict == LabelVerdict::Censored {
                return Err(Error::Duplicate);
            }
            if label.recorded_sequence <= previous.recorded_sequence {
                return Err(Error::Stale);
            }
        }
        self.last_sequence = self.last_sequence.max(label.recorded_sequence);
        self.cases.get_mut(&case).ok_or(Error::Missing)?.label = Some(label);
        Ok(())
    }

    /// Sealing includes ALL manifest cases, not just the cases selected for
    /// review. Missing observations are materialized as Missing in the counts.
    pub fn seal(self, sequence: u64) -> Result<CredibilitySnapshot, Error> {
        if sequence < self.last_sequence {
            return Err(Error::Stale);
        }
        let mut scores: BTreeMap<String, BTreeMap<String, CredibilityScore>> = self.helpers.keys()
            .map(|helper| (helper.clone(), self.strata.iter()
                .map(|stratum| (stratum.clone(), CredibilityScore::default())).collect()))
            .collect();
        // Counts are bounded by MAX_CASES; sums of u64 lead times use u128.
        for record in self.cases.values() {
            for helper in self.helpers.keys() {
                let score = scores.get_mut(helper).ok_or(Error::Missing)?
                    .get_mut(&record.spec.stratum).ok_or(Error::Missing)?;
                score.cases += 1;
                let observation = record.observations.as_ref().and_then(|o| o.get(helper))
                    .copied().unwrap_or(Observation::Missing);
                match observation {
                    Observation::Missing => score.missing += 1,
                    Observation::Abstain => score.abstained += 1,
                    _ => {}
                }
                let Some(label) = &record.label else {
                    score.pending += 1;
                    continue;
                };
                match label.verdict {
                    LabelVerdict::Censored => score.censored += 1,
                    LabelVerdict::Safe => {
                        score.safe += 1;
                        match observation {
                            Observation::Hold { .. } => score.false_positives += 1,
                            Observation::Clear => score.true_negatives += 1,
                            _ => {}
                        }
                    }
                    LabelVerdict::Violation => {
                        score.violations += 1;
                        if let Observation::Hold { first_sequence } = observation {
                            score.true_positives += 1;
                            if first_sequence < record.spec.dispatch_sequence {
                                score.timely_true_positives += 1;
                                let peer_or_dispatch = record.observations.as_ref().into_iter()
                                    .flat_map(|o| o.iter())
                                    .filter(|(peer, _)| *peer != helper)
                                    .filter_map(|(_, o)| match o {
                                        Observation::Hold { first_sequence } => Some(*first_sequence),
                                        _ => None,
                                    }).min().unwrap_or(record.spec.dispatch_sequence)
                                    .min(record.spec.dispatch_sequence);
                                score.lead_time_credit += u128::from(
                                    peer_or_dispatch.saturating_sub(first_sequence));
                            }
                        } else {
                            score.false_negatives += 1;
                        }
                    }
                }
            }
        }
        let oldest_case_sequence = self.cases.values().map(|record| record.spec.dispatch_sequence)
            .min().ok_or(Error::Incomplete)?;
        Ok(CredibilitySnapshot {
            label_owner: self.label_owner, records: self.cases, oldest_case_sequence,
            scope: self.scope, helpers: self.helpers, strata: self.strata,
            scores, sealed_sequence: sequence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn campaign() -> Campaign {
        Campaign {
            scope: EvaluationScope {
                campaign: 1,
                model_generation: 2,
                evaluator_generation: 3,
                held_out_manifest: [4; 32],
            },
            label_owner: "independent-evaluation".into(),
            helpers: BTreeMap::from([
                ("alice".into(), HelperGeneration { generation: 1, cohort: "a".into() }),
                ("bob".into(), HelperGeneration { generation: 1, cohort: "b".into() }),
            ]),
            strata: BTreeSet::from(["tool".into()]),
            cases: (1..=4).map(|id| CaseSpec {
                id,
                stratum: "tool".into(),
                evidence_root: [id as u8; 32],
                dispatch_sequence: 100,
            }).collect(),
        }
    }

    fn observations(alice: Observation, bob: Observation) -> BTreeMap<String, Observation> {
        BTreeMap::from([("alice".into(), alice), ("bob".into(), bob)])
    }

    fn label(case: u8, verdict: LabelVerdict, sequence: u64) -> EvaluationLabel {
        EvaluationLabel {
            owner: "independent-evaluation".into(),
            evaluator_generation: 3,
            source: LabelSource::IndependentEvaluation,
            evidence_root: [case; 32],
            recorded_sequence: sequence,
            verdict,
        }
    }

    #[test]
    fn independent_truth_rewards_early_dissent_not_early_false_holds() {
        let mut ledger = CredibilityLedger::new(campaign()).unwrap();
        ledger.record_observations(1, observations(
            Observation::Hold { first_sequence: 10 },
            Observation::Hold { first_sequence: 20 },
        )).unwrap();
        ledger.record_observations(2, observations(
            Observation::Hold { first_sequence: 1 }, Observation::Clear,
        )).unwrap();
        ledger.record_observations(3, observations(
            Observation::Hold { first_sequence: 30 }, Observation::Clear,
        )).unwrap();
        ledger.record_observations(4, observations(Observation::Clear, Observation::Clear)).unwrap();
        for (case, verdict) in [(1, LabelVerdict::Violation), (2, LabelVerdict::Safe),
            (3, LabelVerdict::Violation), (4, LabelVerdict::Safe)] {
            ledger.record_label(case.into(), label(case, verdict, 120)).unwrap();
        }
        let snapshot = ledger.seal(120).unwrap();
        let alice = &snapshot.scores()["alice"]["tool"];
        let bob = &snapshot.scores()["bob"]["tool"];
        assert_eq!(alice.precision(), Some(Ratio { numerator: 2, denominator: 3 }));
        assert_eq!(alice.recall(), Some(Ratio { numerator: 2, denominator: 2 }));
        assert_eq!(alice.false_positives, 1);
        assert_eq!(alice.lead_time_credit, 10 + 70);
        assert_eq!(bob.lead_time_credit, 0);
        assert_eq!(bob.false_negatives, 1);
        assert_eq!(bob.recall(), Some(Ratio { numerator: 1, denominator: 2 }));
    }

    #[test]
    fn censoring_and_unreviewed_cases_never_become_safe_examples() {
        let mut ledger = CredibilityLedger::new(campaign()).unwrap();
        ledger.record_observations(1, observations(Observation::Abstain, Observation::Missing)).unwrap();
        ledger.record_label(1, label(1, LabelVerdict::Violation, 110)).unwrap();
        ledger.record_label(2, label(2, LabelVerdict::Censored, 110)).unwrap();
        let snapshot = ledger.seal(110).unwrap();
        let alice = &snapshot.scores()["alice"]["tool"];
        assert_eq!(alice.cases, 4);
        assert_eq!(alice.pending, 2);
        assert_eq!(alice.censored, 1);
        assert_eq!(alice.safe, 0);
        assert_eq!(alice.false_negatives, 1);
        assert_eq!(alice.abstained, 1);
        assert_eq!(alice.missing, 3);
        assert_eq!(alice.precision(), None);
        assert_eq!(alice.coverage(), Some(Ratio { numerator: 0, denominator: 4 }));
    }

    #[test]
    fn labels_are_bound_to_owner_generation_source_and_case() {
        for mutation in 0..5 {
            let mut ledger = CredibilityLedger::new(campaign()).unwrap();
            let mut invalid = label(1, LabelVerdict::Safe, 100);
            match mutation {
                0 => invalid.owner = "alice".into(),
                1 => invalid.evaluator_generation += 1,
                2 => invalid.source = LabelSource::CommitteeConsensus,
                3 => invalid.source = LabelSource::HelperSelfReport,
                _ => invalid.evidence_root = [2; 32],
            }
            assert_eq!(ledger.record_label(1, invalid), Err(Error::Binding));
            // Rejection is atomic: a genuine label still succeeds.
            ledger.record_label(1, label(1, LabelVerdict::Safe, 100)).unwrap();
        }
    }

    #[test]
    fn delayed_labels_do_not_allow_backfilled_predictions_or_final_label_rewrites() {
        let mut ledger = CredibilityLedger::new(campaign()).unwrap();
        ledger.record_label(1, label(1, LabelVerdict::Censored, 110)).unwrap();
        assert_eq!(ledger.record_observations(1, observations(Observation::Clear, Observation::Clear)),
            Err(Error::WrongState));
        assert_eq!(ledger.record_label(1, label(1, LabelVerdict::Safe, 110)), Err(Error::Stale));
        ledger.record_label(1, label(1, LabelVerdict::Violation, 120)).unwrap();
        assert_eq!(ledger.record_label(1, label(1, LabelVerdict::Safe, 130)), Err(Error::Duplicate));
        let snapshot = ledger.seal(120).unwrap();
        assert_eq!(snapshot.scores()["alice"]["tool"].false_negatives, 1);
        assert_eq!(snapshot.scores()["alice"]["tool"].censored, 0);
    }

    #[test]
    fn malformed_reports_are_rejected_atomically_and_exact_roster_is_required() {
        let mut ledger = CredibilityLedger::new(campaign()).unwrap();
        let mut partial = observations(Observation::Clear, Observation::Clear);
        partial.remove("bob");
        assert_eq!(ledger.record_observations(1, partial), Err(Error::Binding));
        assert_eq!(ledger.record_observations(1, observations(
            Observation::Hold { first_sequence: 0 }, Observation::Clear,
        )), Err(Error::InvalidInput));
        ledger.record_observations(1, observations(Observation::Clear, Observation::Clear)).unwrap();
        assert_eq!(ledger.record_observations(1, observations(Observation::Clear, Observation::Clear)),
            Err(Error::WrongState));
        assert_eq!(ledger.record_label(99, label(1, LabelVerdict::Safe, 100)), Err(Error::Missing));
    }

    #[test]
    fn late_or_tied_holds_do_not_receive_unearned_lead_credit() {
        let mut ledger = CredibilityLedger::new(campaign()).unwrap();
        ledger.record_observations(1, observations(
            Observation::Hold { first_sequence: 100 }, Observation::Hold { first_sequence: 101 },
        )).unwrap();
        ledger.record_observations(2, observations(
            Observation::Hold { first_sequence: 20 }, Observation::Hold { first_sequence: 20 },
        )).unwrap();
        assert_eq!(ledger.record_label(1, label(1, LabelVerdict::Violation, 100)), Err(Error::Stale));
        ledger.record_label(1, label(1, LabelVerdict::Violation, 110)).unwrap();
        ledger.record_label(2, label(2, LabelVerdict::Violation, 110)).unwrap();
        let snapshot = ledger.seal(110).unwrap();
        for helper in ["alice", "bob"] {
            let score = &snapshot.scores()[helper]["tool"];
            assert_eq!(score.true_positives, 2);
            assert_eq!(score.timely_true_positives, 1);
            assert_eq!(score.lead_time_credit, 0);
        }
    }

    #[test]
    fn predeclared_manifest_prevents_case_aliasing_and_unrepresented_strata() {
        let mut duplicate = campaign();
        duplicate.cases[1].evidence_root = duplicate.cases[0].evidence_root;
        assert_eq!(CredibilityLedger::new(duplicate).unwrap_err(), Error::Duplicate);
        let mut duplicate = campaign();
        duplicate.cases[1].id = duplicate.cases[0].id;
        assert_eq!(CredibilityLedger::new(duplicate).unwrap_err(), Error::Duplicate);
        let mut incomplete = campaign();
        incomplete.strata.insert("unseen".into());
        assert_eq!(CredibilityLedger::new(incomplete).unwrap_err(), Error::Incomplete);
        let mut self_labelled = campaign();
        self_labelled.label_owner = "alice".into();
        assert_eq!(CredibilityLedger::new(self_labelled).unwrap_err(), Error::Binding);
        let mut invalid = campaign();
        invalid.helpers.get_mut("alice").unwrap().generation = 0;
        assert_eq!(CredibilityLedger::new(invalid).unwrap_err(), Error::InvalidInput);
    }

    #[test]
    fn sealing_is_not_allowed_before_all_observation_and_label_positions() {
        let mut ledger = CredibilityLedger::new(campaign()).unwrap();
        ledger.record_label(1, label(1, LabelVerdict::Safe, 200)).unwrap();
        assert_eq!(ledger.seal(199).unwrap_err(), Error::Stale);
    }

    #[test]
    fn lead_time_accumulation_uses_wide_arithmetic() {
        let mut manifest = campaign();
        for spec in &mut manifest.cases {
            spec.dispatch_sequence = u64::MAX;
        }
        let mut ledger = CredibilityLedger::new(manifest).unwrap();
        for case in 1..=4_u8 {
            ledger.record_observations(case.into(), observations(
                Observation::Hold { first_sequence: 1 }, Observation::Clear,
            )).unwrap();
            ledger.record_label(case.into(), label(case, LabelVerdict::Violation, u64::MAX)).unwrap();
        }
        let snapshot = ledger.seal(u64::MAX).unwrap();
        assert_eq!(snapshot.scores()["alice"]["tool"].lead_time_credit,
            4 * u128::from(u64::MAX - 1));
    }

    #[test]
    fn strata_and_helper_generations_remain_separate() {
        let mut manifest = campaign();
        manifest.helpers.get_mut("alice").unwrap().generation = 9;
        manifest.strata.insert("publication".into());
        manifest.cases[0].stratum = "publication".into();
        let mut ledger = CredibilityLedger::new(manifest).unwrap();
        ledger.record_observations(1, observations(Observation::Clear, Observation::Clear)).unwrap();
        ledger.record_label(1, label(1, LabelVerdict::Safe, 100)).unwrap();
        let snapshot = ledger.seal(100).unwrap();
        assert_eq!(snapshot.helpers()["alice"].generation, 9);
        assert_eq!(snapshot.scores()["alice"]["publication"].safe, 1);
        assert_eq!(snapshot.scores()["alice"]["tool"].pending, 3);
        assert_eq!(snapshot.scores()["alice"]["tool"].safe, 0);
    }
}
