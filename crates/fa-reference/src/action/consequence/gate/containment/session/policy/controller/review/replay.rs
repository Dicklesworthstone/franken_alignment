//! Controller-independent replay of the exact policy and congress decision.
//!
//! An expected anchor is retained before voting, separately from the archive.
//! Equality to that anchor is structural, NOT cryptographic authentication. A
//! self-consistent fabrication with a fabricated anchor is not detected. The
//! verifier checks supplied observations and the reference commitment oracle;
//! it does not prove provider truth, completeness, helper honesty, durability,
//! external execution or full control-ledger accounting. It never emits Permit.

mod wire;
pub use wire::MAX_ARCHIVE_BYTES;

use super::{PolicyReceipt, PolicyReview, PolicySession};
use super::super::{PolicyAuthority, validate_congress};
use crate::action::{FrozenAction, MAX_REQUIRED_WITNESSES, MAX_WITNESS_BYTES};
use crate::action::consequence::{Consequence, Decision};
use crate::action::consequence::congress::{CongressPolicy, evaluate_round};
use crate::action::consequence::gate::{ControlReceipt, TargetCeiling};
use crate::action::consequence::gate::containment::session::{SessionSpec, transcript::RoundTranscript};
use crate::action::consequence::gate::containment::session::policy::{Evaluation, MAX_POLICY_NODES, Policy, Truth};
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES, Reduction};
use crate::{Error, Judgment, ReadWitness, Snapshot};
use std::collections::BTreeMap;

pub const DECISION_ARCHIVE_VERSION: u32 = 1;

/// Retain this before voting and independently of a later received archive.
/// This contains only bounded reference data, no issuer brand or live rights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewAnchor {
    pub attempt: u64,
    pub action: FrozenAction,
    pub expected_control_sequence: u64,
    pub round: u64,
    pub evidence_root: [u8; 32],
    pub policy: Policy,
    pub congress: CongressPolicy,
    pub narrowed_targets: TargetCeiling,
    pub snapshot_semantic_epoch: u64,
    pub observations: Vec<ReadWitness>,
    pub complete: bool,
    pub contradiction: bool,
}

/// Untrusted transport data. Verification reconstructs every claimed result.
/// No current controller, clock, provider or retained mutable session is needed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionArchive {
    pub version: u32,
    pub anchor: ReviewAnchor,
    pub transcript: RoundTranscript,
    pub evaluation: Evaluation,
    pub decision: Decision,
    pub tally: Reduction,
    pub missing: Vec<String>,
    pub abstained: Vec<String>,
}

/// Successful replay is evidence consistency, not permission or harmlessness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayedDecision {
    decision: Decision,
    evaluation: Evaluation,
    tally: Reduction,
    missing: Vec<String>,
    abstained: Vec<String>,
}

impl ReplayedDecision {
    pub fn decision(&self) -> &Decision { &self.decision }
    pub fn evaluation(&self) -> &Evaluation { &self.evaluation }
    pub fn tally(&self) -> &Reduction { &self.tally }
    pub fn missing(&self) -> &[String] { &self.missing }
    pub fn abstained(&self) -> &[String] { &self.abstained }
}

impl DecisionArchive {
    pub fn verify(&self, expected: &ReviewAnchor) -> Result<ReplayedDecision, Error> {
        if self.version != DECISION_ARCHIVE_VERSION {
            return Err(Error::InvalidInput);
        }
        // Bound public collection shapes before structural comparisons.
        validate_congress(&self.anchor.congress)?;
        validate_congress(&expected.congress)?;
        validate_observations(&self.anchor.observations)?;
        validate_observations(&expected.observations)?;
        validate_observations(self.evaluation.witnesses())?;
        if self.evaluation.trace().len() > MAX_POLICY_NODES || self.decision.rules.len() > 4
            || self.tally.admitted_weights.len() > MAX_VOTES
            || self.tally.admitted_cohort_weights.len() > MAX_VOTES
            || self.missing.len() > MAX_VOTES || self.abstained.len() > MAX_VOTES
        {
            return Err(Error::Limit);
        }
        for name in self.tally.admitted_weights.keys()
            .chain(self.tally.admitted_cohort_weights.keys())
            .chain(self.missing.iter()).chain(self.abstained.iter())
        {
            if name.len() > MAX_IDENTIFIER_BYTES { return Err(Error::Limit); }
            if name.is_empty() { return Err(Error::InvalidInput); }
        }
        let round = self.transcript.replay()?;
        if &self.anchor != expected || expected.attempt == 0
            || round.id() != expected.round
            || round.evidence_root() != expected.evidence_root.as_slice()
        {
            return Err(Error::Binding);
        }
        if !expected.complete {
            return Err(Error::Incomplete);
        }
        let snapshot = reconstruct_snapshot(expected)?;
        let evaluation = expected.policy.evaluate(&expected.action, &snapshot)?;
        if evaluation.witnesses() != expected.observations.as_slice()
            || evaluation != self.evaluation
        {
            return Err(Error::Binding);
        }
        // A positive review cannot manufacture a new read set for an old action.
        // A later exact denial may legitimately have different observed values.
        if evaluation.certifiable()
            && evaluation.witnesses() != expected.action.spec().required_witnesses.as_slice()
        {
            return Err(Error::Binding);
        }
        let reduced = evaluate_round(
            &round, &expected.congress, evaluation.result() == Truth::Violated,
            expected.contradiction,
        )?;
        if reduced.decision() != self.decision || reduced.tally() != &self.tally
            || reduced.missing() != self.missing.as_slice()
            || reduced.abstained() != self.abstained.as_slice()
        {
            return Err(Error::Binding);
        }
        Ok(ReplayedDecision {
            decision: reduced.decision(), evaluation, tally: reduced.tally().clone(),
            missing: reduced.missing().to_vec(), abstained: reduced.abstained().to_vec(),
        })
    }

    /// Bind a replayed judgment to the reported control transition. This checks
    /// identity, order and consequence, not refunds or undisclosed other actions.
    pub fn verify_control_binding(
        &self,
        expected: &ReviewAnchor,
        receipt: &ControlReceipt,
    ) -> Result<ReplayedDecision, Error> {
        let replayed = self.verify(expected)?;
        let sequence = expected.expected_control_sequence.checked_add(1).ok_or(Error::Overflow)?;
        if receipt.sequence != sequence || receipt.attempt != expected.attempt
            || receipt.action != expected.action || receipt.binding.round != expected.round
            || receipt.binding.evidence_root != expected.evidence_root
            || receipt.binding.reducer_generation != expected.congress.generation
            || receipt.decision != replayed.decision
        {
            return Err(Error::Binding);
        }
        Ok(replayed)
    }
}

fn validate_observations(observations: &[ReadWitness]) -> Result<(), Error> {
    if observations.len() > MAX_REQUIRED_WITNESSES {
        return Err(Error::Limit);
    }
    let mut bytes = 0_usize;
    for witness in observations {
        match witness {
            ReadWitness::Exact { value: Some(value), .. } => {
                bytes = bytes.checked_add(value.len()).ok_or(Error::Limit)?;
            }
            ReadWitness::EmptyRange { start, end } if start >= end => {
                return Err(Error::InvalidInput);
            }
            _ => {}
        }
        if bytes > MAX_WITNESS_BYTES {
            return Err(Error::Limit);
        }
    }
    Ok(())
}

/// Build only the policy's observed slice, not an invented complete provider
/// database. `complete` is the retained trusted declaration. Every recorded
/// absence/empty range must agree with every recorded positive member.
fn reconstruct_snapshot(anchor: &ReviewAnchor) -> Result<Snapshot, Error> {
    let mut exact: BTreeMap<u64, Option<Vec<u8>>> = BTreeMap::new();
    for witness in &anchor.observations {
        if let ReadWitness::Exact { key, value } = witness {
            if let Some(previous) = exact.get(key) {
                if previous != value {
                    return Err(Error::Binding);
                }
            } else {
                exact.insert(*key, value.clone());
            }
        }
    }
    let snapshot = Snapshot {
        semantic_epoch: anchor.snapshot_semantic_epoch,
        complete: anchor.complete,
        values: exact.into_iter().filter_map(|(key, value)| value.map(|v| (key, v))).collect(),
    };
    Judgment::capture(&snapshot, anchor.observations.clone())?;
    Ok(snapshot)
}

fn make_anchor(
    spec: &SessionSpec,
    action: &FrozenAction,
    sequence: u64,
    policy: &Policy,
    evaluation: &Evaluation,
    semantic_epoch: u64,
) -> ReviewAnchor {
    ReviewAnchor {
        attempt: spec.attempt, action: action.clone(), expected_control_sequence: sequence,
        round: spec.round, evidence_root: spec.evidence_root, policy: policy.clone(),
        congress: spec.policy.clone(), narrowed_targets: spec.narrowed_targets.clone(),
        snapshot_semantic_epoch: semantic_epoch, observations: evaluation.witnesses().to_vec(),
        complete: evaluation.complete, contradiction: spec.contradiction,
    }
}

impl PolicySession {
    /// Obtain before any votes so a later archive can be checked against it.
    pub fn replay_anchor(&self) -> ReviewAnchor {
        let context = &self.session.context;
        make_anchor(&context.spec, &context.action, context.expected_control_sequence,
            &self.policy, &self.evaluation, self.snapshot_semantic_epoch)
    }
}

impl PolicyReview {
    pub fn replay_archive(&self) -> DecisionArchive {
        let context = &self.review.context;
        DecisionArchive {
            version: DECISION_ARCHIVE_VERSION,
            anchor: make_anchor(&context.spec, &context.action, context.expected_control_sequence,
                &self.policy, &self.evaluation, self.snapshot_semantic_epoch),
            transcript: self.review.transcript().clone(),
            evaluation: self.evaluation.clone(), decision: self.review.decision().clone(),
            tally: self.review.tally().clone(), missing: self.review.missing().to_vec(),
            abstained: self.review.abstained().to_vec(),
        }
    }

    pub(crate) fn verify_replay(&self) -> Result<(), Error> {
        let archive = self.replay_archive();
        let replayed = archive.verify(&archive.anchor)?;
        let spec = &self.review.context.spec;
        if spec.exact_disqualifier != (replayed.evaluation.result() == Truth::Violated) {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

impl PolicyReceipt {
    pub fn verify_replay(
        &self,
        archive: &DecisionArchive,
        expected: &ReviewAnchor,
    ) -> Result<ReplayedDecision, Error> {
        let replayed = archive.verify_control_binding(expected, &self.control)?;
        if self.policy.as_ref() != &expected.policy || self.evaluation != replayed.evaluation
            || self.snapshot_semantic_epoch != expected.snapshot_semantic_epoch
        {
            return Err(Error::Binding);
        }
        Ok(replayed)
    }
}

/// A transportable decision plus its actual reference control receipt. There is
/// no authority brand in the archive. Cloning it cannot clone a live permission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchivedPolicyReceipt {
    pub receipt: PolicyReceipt,
    pub archive: DecisionArchive,
}

impl ArchivedPolicyReceipt {
    pub fn verify(&self, expected: &ReviewAnchor) -> Result<ReplayedDecision, Error> {
        self.receipt.verify_replay(&self.archive, expected)
    }
}

impl PolicyAuthority {
    /// Apply through the normal current-evidence/issuer/epoch checks and return
    /// the replay material alongside the actual receipt. Verification occurs
    /// before mutation; export failure cannot leave an unreported transition.
    pub fn apply_review_archived(
        &mut self,
        review: PolicyReview,
        snapshot: &Snapshot,
    ) -> Result<ArchivedPolicyReceipt, Error> {
        review.verify_replay()?;
        let archive = review.replay_archive();
        let receipt = self.apply_review(review, snapshot)?;
        Ok(ArchivedPolicyReceipt { receipt, archive })
    }
}
