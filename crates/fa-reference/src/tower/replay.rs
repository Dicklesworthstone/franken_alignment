//! Bounded, immutable source records for paired historical policy experiments.
//!
//! A host supplies the original observed archive and a separately retained
//! pre-vote anchor. Installation checks identity and resource bounds only. Each
//! run first uses the original full-input/timing replay, then the original
//! capability-free policy experiment. Neither operation authenticates the
//! supplied history or creates current permission. This shares native policy
//! code and therefore is not the independent verifier required by `fa verify`.

use std::mem::size_of;
use std::rc::Rc;

use crate::action::consequence::Rule;
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::experiment::{
    CounterfactualReport, Intervention, InterventionScope, MAX_INTERVENTION_BYTES,
    MAX_INTERVENTIONS, PolicyExperiment,
};
use crate::action::consequence::gate::containment::session::policy::controller::{
    DecisionArchive, ReplayedDecision, ReviewAnchor,
};
use crate::action::consequence::gate::containment::session::policy::{
    Evaluation, MAX_POLICY_EDGES, MAX_POLICY_LITERAL_BYTES, MAX_POLICY_NODES, Policy, Predicate,
    Step,
};
use crate::action::consequence::gate::containment::session::transcript::{
    CommitRecord, MAX_TRANSCRIPT_BYTES, RevealRecord, RoundTranscript,
};
use crate::action::consequence::oversight::replay::{
    ObservedDecisionArchive, ObservedReviewAnchor,
};
use crate::action::consequence::oversight::{CommitteeInput, MAX_COMMITTEE_BYTES, ObservedReceipt};
use crate::action::{
    ElapsedTick, FrozenAction, MAX_ATTEMPTS, MAX_PAYLOAD_BYTES, MAX_REQUIRED_WITNESSES,
    MAX_WITNESS_BYTES, Scope,
};
use crate::evidence_view::{
    EvidencePartView, EvidenceViewManifest, MAX_EVIDENCE_PART_BINDINGS, MAX_PROJECTED_ORIGINALS,
    OriginalIdentity,
};
use crate::full_input::{
    MAX_OMISSIONS, MAX_PROFILE_BYTES, MAX_SUBMITTED_BYTES, MAX_SUBMITTED_PARTS, Omission,
    SubmittedPart,
};
use crate::observation_algebra::{Basis, Knowledge, UnknownReason};
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES, Reduction};
use crate::round::MAX_FIELD_LEN;
use crate::typestate_cx::ExperimentCx;
use crate::{Error, ReadWitness};

/// Maximum logical retained-data charge for one source record. The Tower also
/// caps the sum across records. This includes every retained occurrence of an
/// input, even when the caller supplied shared `Rc` values.
pub(super) const MAX_REPLAY_RECORD_BYTES: usize = 8 * 1_048_576;

// Fixed logical metadata allowance per retained ordered-map/set entry. This
// is resource accounting, not a claim to measure a particular allocator.
const TREE_ENTRY_BYTES: usize = 64;
const MAX_DECISION_RULES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayProfile {
    ObservedPolicyExperiment,
}

/// Each property states exactly what this replay establishes. In particular,
/// receipt binding covers the original verifier's checked policy/control/input
/// fields; it does not establish complete ledger accounting or execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayProperties {
    pub replay_consistency: Knowledge<bool>,
    pub retained_anchor_binding: Knowledge<bool>,
    pub receipt_binding: Knowledge<bool>,
    pub cryptographic_authentication: Knowledge<bool>,
    pub independent_verifier_isolation: Knowledge<bool>,
    pub provider_truth: Knowledge<bool>,
    pub external_execution: Knowledge<bool>,
    pub current_eligibility: Knowledge<bool>,
    pub full_ledger_accounting: Knowledge<bool>,
}

/// The actual registered edit surface, not a grant of live authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayInterventionRegistration {
    pub id: u64,
    pub allow_payload: bool,
    pub allow_target: bool,
    pub allow_units: bool,
    pub keys: Vec<u64>,
}

/// Inspection of installed data only. Native identities come from the retained
/// expected anchor; a present receipt's sequence has not been verified here.
/// The Tower supplies the full source basis outside this metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayPlanMetadata {
    pub profile: ReplayProfile,
    pub native_scope: Scope,
    pub native_attempt: u64,
    pub native_round: u64,
    pub native_expected_control_sequence: u64,
    pub native_receipt_control_sequence: Option<u64>,
    pub has_receipt: bool,
    pub retained_bytes: usize,
    pub registration: ReplayInterventionRegistration,
    pub properties: ReplayProperties,
}

/// Historical evidence and the native paired experiment, never an authorization
/// token. Native control sequences retain their original namespace and need
/// not equal either the imported Tower cut or the branch journal sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedReplayReport {
    pub profile: ReplayProfile,
    pub native_scope: Scope,
    pub native_attempt: u64,
    pub native_round: u64,
    pub native_expected_control_sequence: u64,
    pub native_receipt_control_sequence: Option<u64>,
    pub replayed: ReplayedDecision,
    pub counterfactual: CounterfactualReport,
    pub properties: ReplayProperties,
}

/// Host-installed data with no live broker, controller, endpoint or permit.
/// Requests can select an installed record but cannot replace its expected
/// anchor or widen its intervention registration.
#[derive(Clone, Debug)]
pub struct ObservedReplayRecord {
    native_scope: Scope,
    expected: ObservedReviewAnchor,
    archive: ObservedDecisionArchive,
    receipt: Option<ObservedReceipt>,
    intervention_scope: InterventionScope,
    retained_bytes: usize,
}

impl ObservedReplayRecord {
    /// Check shapes and the explicit native identity before copying any source
    /// data. This does not verify a decision or derive an anchor from an archive.
    /// The host must separately bind this scope to the imported Tower subject.
    pub fn new(
        native_scope: Scope,
        expected: ObservedReviewAnchor,
        archive: ObservedDecisionArchive,
        receipt: Option<ObservedReceipt>,
        intervention_scope: InterventionScope,
    ) -> Result<Self, Error> {
        for action in [
            &expected.policy.action,
            expected.inputs.action(),
            &archive.policy.anchor.action,
            archive.inputs.action(),
        ] {
            if action.spec().scope != native_scope {
                return Err(Error::Binding);
            }
        }
        if let Some(receipt) = &receipt {
            if receipt.policy.control.action.spec().scope != native_scope
                || receipt.inputs.action().spec().scope != native_scope
            {
                return Err(Error::Binding);
            }
        }

        let mut budget = ImportBudget(0);
        budget.add(size_of::<Self>())?;
        // InterventionScope has private, constructor-bounded keys. Charge its
        // maximum retained key set without manufacturing an inspection API.
        budget.tree_entries::<u64>(MAX_INTERVENTIONS, MAX_INTERVENTIONS)?;
        budget.anchor(&expected.policy)?;
        budget.inputs(&expected.inputs)?;
        budget.archive(&archive.policy)?;
        budget.inputs(&archive.inputs)?;
        budget.items::<ElapsedTick>(archive.commit_times.len(), MAX_VOTES)?;
        budget.items::<ElapsedTick>(archive.reveal_times.len(), MAX_VOTES)?;
        if let Some(receipt) = &receipt {
            budget.receipt(receipt)?;
        }

        // Clone only after all bounds pass. Fresh copies of the immutable Rc
        // payloads and vectors avoid retaining a caller's excessive spare Vec
        // capacity. Every retained input occurrence was charged above.
        let mut retained_expected = expected.clone();
        retained_expected.inputs = Rc::new(expected.inputs.as_ref().clone());
        let mut retained_archive = archive.clone();
        retained_archive.inputs = Rc::new(archive.inputs.as_ref().clone());
        let retained_receipt = receipt.as_ref().map(|receipt| {
            let mut retained = receipt.clone();
            retained.inputs = Rc::new(receipt.inputs.as_ref().clone());
            retained.policy.policy = Rc::new(receipt.policy.policy.as_ref().clone());
            retained
        });

        Ok(Self {
            native_scope,
            expected: retained_expected,
            archive: retained_archive,
            receipt: retained_receipt,
            intervention_scope,
            retained_bytes: budget.0,
        })
    }

    pub fn native_scope(&self) -> Scope {
        self.native_scope
    }

    pub fn has_receipt(&self) -> bool {
        self.receipt.is_some()
    }

    pub(super) fn historical_policy_epoch(&self) -> u64 {
        self.expected.policy.action.spec().policy_epoch
    }

    /// A conservative logical charge for retained buffers and bounded metadata,
    /// not a measurement of physical allocator or inference work.
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    /// Read the frozen registration without verifying or executing its history.
    /// Every epistemic property remains unknown, including after a prior replay.
    pub fn plan_metadata(&self) -> ReplayPlanMetadata {
        let unavailable = || Knowledge::Unknown {
            reason: UnknownReason::Unavailable,
        };
        ReplayPlanMetadata {
            profile: ReplayProfile::ObservedPolicyExperiment,
            native_scope: self.native_scope,
            native_attempt: self.expected.policy.attempt,
            native_round: self.expected.policy.round,
            native_expected_control_sequence: self.expected.policy.expected_control_sequence,
            native_receipt_control_sequence: self
                .receipt
                .as_ref()
                .map(|receipt| receipt.policy.control.sequence),
            has_receipt: self.has_receipt(),
            retained_bytes: self.retained_bytes,
            registration: ReplayInterventionRegistration {
                id: self.intervention_scope.id(),
                allow_payload: self.intervention_scope.allows_payload(),
                allow_target: self.intervention_scope.allows_target(),
                allow_units: self.intervention_scope.allows_units(),
                keys: self.intervention_scope.keys().iter().copied().collect(),
            },
            properties: ReplayProperties {
                replay_consistency: unavailable(),
                retained_anchor_binding: unavailable(),
                receipt_binding: unavailable(),
                cryptographic_authentication: unavailable(),
                independent_verifier_isolation: unavailable(),
                provider_truth: unavailable(),
                external_execution: unavailable(),
                current_eligibility: unavailable(),
                full_ledger_accounting: unavailable(),
            },
        }
    }

    /// Recompute the original observed history before every paired experiment.
    /// The immutable baseline makes repeats stable and errors leave it intact.
    /// The purpose context must name the same native tenant/run and a different
    /// branch from the historical source. Its epoch belongs to the current
    /// branch; it is deliberately not compared to the historical action epoch.
    /// Host address mapping, current authorization and charging remain Tower
    /// admission responsibilities.
    pub fn run(
        &self,
        cx: &ExperimentCx,
        edits: &[Intervention],
        source_basis: Basis,
    ) -> Result<ObservedReplayReport, Error> {
        if cx.tenant_id() != self.native_scope.tenant
            || cx.run_id() != self.native_scope.run
            || cx.branch_id() == self.native_scope.branch
        {
            return Err(Error::Binding);
        }
        validate_edits(edits)?;
        let replayed = match &self.receipt {
            Some(receipt) => self.archive.verify_receipt(&self.expected, receipt)?,
            None => self.archive.verify(&self.expected)?,
        };
        let experiment = PolicyExperiment::from_archive(
            &self.archive.policy,
            &self.expected.policy,
            self.intervention_scope.clone(),
        )?;
        let counterfactual = experiment.run(edits)?;
        let established = || Knowledge::Known {
            value: true,
            basis: source_basis,
        };
        let unavailable = || Knowledge::Unknown {
            reason: UnknownReason::Unavailable,
        };
        Ok(ObservedReplayReport {
            profile: ReplayProfile::ObservedPolicyExperiment,
            native_scope: self.native_scope,
            native_attempt: self.expected.policy.attempt,
            native_round: self.expected.policy.round,
            native_expected_control_sequence: self.expected.policy.expected_control_sequence,
            native_receipt_control_sequence: self
                .receipt
                .as_ref()
                .map(|receipt| receipt.policy.control.sequence),
            replayed,
            counterfactual,
            properties: ReplayProperties {
                replay_consistency: established(),
                retained_anchor_binding: established(),
                receipt_binding: if self.has_receipt() {
                    established()
                } else {
                    unavailable()
                },
                cryptographic_authentication: unavailable(),
                independent_verifier_isolation: unavailable(),
                provider_truth: unavailable(),
                external_execution: unavailable(),
                current_eligibility: unavailable(),
                full_ledger_accounting: unavailable(),
            },
        })
    }
}

/// Resource-only admission check matching the native experiment's edit bounds.
/// Duplicate, out-of-registration and stale expected-value edits remain native
/// execution errors and consume admitted work at the Tower boundary.
pub(super) fn validate_edits(edits: &[Intervention]) -> Result<(), Error> {
    if edits.len() > MAX_INTERVENTIONS {
        return Err(Error::Limit);
    }
    let mut bytes = 0_usize;
    for edit in edits {
        let size = match edit {
            Intervention::Payload(value) => {
                if value.len() > MAX_PAYLOAD_BYTES {
                    return Err(Error::Limit);
                }
                value.len()
            }
            Intervention::Key {
                expected,
                replacement,
                ..
            } => expected
                .as_ref()
                .map_or(0, Vec::len)
                .checked_add(replacement.as_ref().map_or(0, Vec::len))
                .ok_or(Error::Limit)?,
            Intervention::Target(_) | Intervention::Units(_) => 0,
        };
        bytes = bytes.checked_add(size).ok_or(Error::Limit)?;
        if bytes > MAX_INTERVENTION_BYTES {
            return Err(Error::Limit);
        }
    }
    Ok(())
}

/// Bounds are checked by length before any collection walk or clone. This
/// intentionally does not call a wire encoder: the native archive encoder
/// verifies against its own anchor, which is not installation admission.
struct ImportBudget(usize);

impl ImportBudget {
    fn add(&mut self, bytes: usize) -> Result<(), Error> {
        self.0 = self.0.checked_add(bytes).ok_or(Error::Limit)?;
        if self.0 > MAX_REPLAY_RECORD_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }

    fn items<T>(&mut self, count: usize, maximum: usize) -> Result<(), Error> {
        if count > maximum {
            return Err(Error::Limit);
        }
        self.add(count.checked_mul(size_of::<T>()).ok_or(Error::Limit)?)
    }

    fn tree_entries<T>(&mut self, count: usize, maximum: usize) -> Result<(), Error> {
        self.items::<T>(count, maximum)?;
        self.add(count.checked_mul(TREE_ENTRY_BYTES).ok_or(Error::Limit)?)
    }

    fn identifier(&mut self, value: &str) -> Result<(), Error> {
        if value.len() > MAX_IDENTIFIER_BYTES {
            return Err(Error::Limit);
        }
        self.add(value.len())
    }

    fn witnesses(&mut self, values: &[ReadWitness]) -> Result<(), Error> {
        self.items::<ReadWitness>(values.len(), MAX_REQUIRED_WITNESSES)?;
        let mut bytes = 0_usize;
        for witness in values {
            if let ReadWitness::Exact {
                value: Some(value), ..
            } = witness
            {
                bytes = bytes.checked_add(value.len()).ok_or(Error::Limit)?;
                if bytes > MAX_WITNESS_BYTES {
                    return Err(Error::Limit);
                }
            }
        }
        self.add(bytes)
    }

    fn action(&mut self, action: &FrozenAction) -> Result<(), Error> {
        let spec = action.spec();
        if spec.payload.len() > MAX_PAYLOAD_BYTES {
            return Err(Error::Limit);
        }
        self.add(spec.payload.len())?;
        self.witnesses(&spec.required_witnesses)
    }

    fn policy(&mut self, policy: &Policy) -> Result<(), Error> {
        self.items::<Predicate>(policy.nodes().len(), MAX_POLICY_NODES)?;
        let mut edges = 0_usize;
        let mut literals = 0_usize;
        for node in policy.nodes() {
            match node {
                Predicate::All(children) | Predicate::Any(children) => {
                    edges = edges.checked_add(children.len()).ok_or(Error::Limit)?;
                    self.items::<usize>(children.len(), MAX_POLICY_EDGES)?;
                }
                Predicate::Not(_) => {
                    edges = edges.checked_add(1).ok_or(Error::Limit)?;
                }
                Predicate::PayloadIs(value) | Predicate::ExactValue { value, .. } => {
                    literals = literals.checked_add(value.len()).ok_or(Error::Limit)?;
                }
                _ => {}
            }
            if edges > MAX_POLICY_EDGES || literals > MAX_POLICY_LITERAL_BYTES {
                return Err(Error::Limit);
            }
        }
        self.add(literals)
    }

    fn congress(&mut self, congress: &CongressPolicy) -> Result<(), Error> {
        self.tree_entries::<(String, MemberPolicy)>(congress.members.len(), MAX_VOTES)?;
        for (name, member) in &congress.members {
            self.identifier(name)?;
            self.identifier(&member.cohort)?;
        }
        Ok(())
    }

    fn ceiling(&mut self) -> Result<(), Error> {
        // TargetCeiling is private and constructor-bounded. Conservatively
        // reserve its maximum instead of serializing or inspecting private data.
        self.tree_entries::<[u64; 5]>(MAX_ATTEMPTS, MAX_ATTEMPTS)
    }

    fn anchor(&mut self, anchor: &ReviewAnchor) -> Result<(), Error> {
        self.action(&anchor.action)?;
        self.policy(&anchor.policy)?;
        self.congress(&anchor.congress)?;
        self.ceiling()?;
        self.witnesses(&anchor.observations)
    }

    fn evaluation(&mut self, evaluation: &Evaluation) -> Result<(), Error> {
        self.items::<Step>(evaluation.trace().len(), MAX_POLICY_NODES)?;
        self.witnesses(evaluation.witnesses())
    }

    fn transcript(&mut self, transcript: &RoundTranscript) -> Result<(), Error> {
        self.items::<String>(transcript.members.len(), MAX_VOTES)?;
        self.items::<CommitRecord>(transcript.commits.len(), MAX_VOTES)?;
        self.items::<RevealRecord>(transcript.reveals.len(), MAX_VOTES)?;
        let mut bytes = 0_usize;
        for name in transcript
            .members
            .iter()
            .chain(transcript.commits.iter().map(|entry| &entry.member))
            .chain(transcript.reveals.iter().map(|entry| &entry.member))
        {
            self.identifier(name)?;
            bytes = bytes.checked_add(name.len()).ok_or(Error::Limit)?;
        }
        for reveal in &transcript.reveals {
            if reveal.salt.len() > MAX_FIELD_LEN {
                return Err(Error::Limit);
            }
            self.add(reveal.salt.len())?;
            bytes = bytes.checked_add(reveal.salt.len()).ok_or(Error::Limit)?;
        }
        if bytes > MAX_TRANSCRIPT_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }

    fn reduction(&mut self, tally: &Reduction) -> Result<(), Error> {
        self.tree_entries::<(String, u64)>(tally.admitted_weights.len(), MAX_VOTES)?;
        self.tree_entries::<(String, u64)>(tally.admitted_cohort_weights.len(), MAX_VOTES)?;
        for name in tally
            .admitted_weights
            .keys()
            .chain(tally.admitted_cohort_weights.keys())
        {
            self.identifier(name)?;
        }
        Ok(())
    }

    fn archive(&mut self, archive: &DecisionArchive) -> Result<(), Error> {
        self.anchor(&archive.anchor)?;
        self.transcript(&archive.transcript)?;
        self.evaluation(&archive.evaluation)?;
        self.items::<Rule>(archive.decision.rules.len(), MAX_DECISION_RULES)?;
        self.reduction(&archive.tally)?;
        self.items::<String>(archive.missing.len(), MAX_VOTES)?;
        self.items::<String>(archive.abstained.len(), MAX_VOTES)?;
        for name in archive.missing.iter().chain(archive.abstained.iter()) {
            self.identifier(name)?;
        }
        Ok(())
    }

    fn inputs(&mut self, inputs: &CommitteeInput) -> Result<(), Error> {
        if inputs.logical_bytes() > MAX_COMMITTEE_BYTES {
            return Err(Error::Limit);
        }
        self.add(size_of::<CommitteeInput>())?;
        self.action(inputs.action())?;
        self.tree_entries::<(String, EvidenceViewManifest)>(inputs.views().len(), MAX_VOTES)?;
        for (name, view) in inputs.views() {
            self.identifier(name)?;
            let actual = view.actual_input();
            if actual.submitted_bytes().len() > MAX_SUBMITTED_BYTES
                || actual.input_profile().profile_bytes.len() > MAX_PROFILE_BYTES
            {
                return Err(Error::Limit);
            }
            self.add(actual.submitted_bytes().len())?;
            self.add(actual.input_profile().profile_bytes.len())?;
            self.items::<SubmittedPart>(actual.ordered_parts().len(), MAX_SUBMITTED_PARTS)?;
            self.items::<Omission>(actual.omissions().len(), MAX_OMISSIONS)?;
            self.items::<OriginalIdentity>(
                view.authorization().projected_originals.len(),
                MAX_PROJECTED_ORIGINALS,
            )?;
            self.items::<EvidencePartView>(
                view.evidence_parts().len(),
                MAX_EVIDENCE_PART_BINDINGS,
            )?;
        }
        Ok(())
    }

    fn receipt(&mut self, receipt: &ObservedReceipt) -> Result<(), Error> {
        let policy = &receipt.policy;
        // These fields are retained even though the native replay does not
        // establish their accounting meaning. In particular `stopped` is public.
        self.items::<u64>(policy.control.stopped.len(), MAX_ATTEMPTS)?;
        self.items::<Rule>(policy.control.decision.rules.len(), MAX_DECISION_RULES)?;
        self.action(&policy.control.action)?;
        if policy.control.ceiling.is_some() {
            self.ceiling()?;
        }
        self.add(size_of::<Policy>())?;
        self.policy(&policy.policy)?;
        self.evaluation(&policy.evaluation)?;
        self.inputs(&receipt.inputs)
    }
}
