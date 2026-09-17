//! Exact-policy diagnosis of original admissions, including terminal denials
//! that never convened a congress. No round or hypothetical helper vote is made.

pub mod search;

use super::{EvidenceSlice, Intervention, InterventionScope, NextRequirement, NodeChange,
    apply_interventions};
use crate::action::{ActionState, Purpose};
use crate::action::consequence::gate::containment::session::policy::{Truth, controller::Proposal};
use crate::{Error, Snapshot};

/// Immutable observation data. The original evaluator checks the complete
/// proposal against an independently supplied historical snapshot, then only its
/// witnessed facts/closed domains are retained for branching. Structural checks
/// are not signatures, provider authentication, or authority to change the world.
#[derive(Clone, Debug)]
pub struct ProposalExperiment {
    proposal: Proposal,
    scope: InterventionScope,
    evidence: EvidenceSlice,
}

/// A satisfied counterfactual is an exact-predicate result, never permission.
/// There was no congress at this boundary; neither old nor predicted votes are
/// included. Changing a policy-state key is hypothetical, not a provider write.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::experiment::proposal::ProposalCounterfactualReport;
/// fn approve(report: ProposalCounterfactualReport) -> Permit { report }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalCounterfactualReport {
    scope: u64,
    attempt: u64,
    initial_state: ActionState,
    baseline: Truth,
    counterfactual: Truth,
    trace: Vec<Truth>,
    changes: Vec<NodeChange>,
    interventions: Vec<Intervention>,
    next: NextRequirement,
}
impl ProposalCounterfactualReport {
    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn scope(&self) -> u64 { self.scope }
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn initial_state(&self) -> ActionState { self.initial_state }
    pub fn baseline(&self) -> Truth { self.baseline }
    pub fn counterfactual(&self) -> Truth { self.counterfactual }
    pub fn trace(&self) -> &[Truth] { &self.trace }
    pub fn changes(&self) -> &[NodeChange] { &self.changes }
    pub fn interventions(&self) -> &[Intervention] { &self.interventions }
    pub fn next_requirement(&self) -> NextRequirement { self.next }
}

impl ProposalExperiment {
    /// Consumes bounded native proposal data; verifies before retaining a clone
    /// of anything supplied through its publicly constructible outer record.
    /// The snapshot is not retained or treated as current permitting evidence.
    pub fn new(proposal: Proposal, snapshot: &Snapshot, scope: InterventionScope) -> Result<Self, Error> {
        if proposal.attempt == 0 || proposal.action.spec().scope.purpose != Purpose::Effect {
            return Err(Error::InvalidInput);
        }
        if !snapshot.complete { return Err(Error::Incomplete); }
        if snapshot.semantic_epoch != proposal.snapshot_semantic_epoch { return Err(Error::Stale); }
        let evaluation = proposal.policy.evaluate(&proposal.action, snapshot)?;
        if evaluation != proposal.evaluation
            || proposal.action.spec().required_witnesses.as_slice() != evaluation.witnesses()
            || evaluation.trace().iter().any(|step| step.result == Truth::Unknown)
        { return Err(Error::Binding); }
        let state = if evaluation.certifiable() { ActionState::Reviewing } else { ActionState::Denied };
        if proposal.state != state { return Err(Error::Binding); }
        let evidence = EvidenceSlice::new(evaluation.witnesses())?;
        for key in &scope.keys {
            if evidence.lookup(*key).is_none() { return Err(Error::Incomplete); }
        }
        // Preserve the archived experiment's partial-evidence law: a positive
        // member proves nonemptiness, NOT closure of its entire queried range.
        let trace = evidence.evaluate(&proposal.policy, &proposal.action)?;
        if trace.len() != evaluation.trace().len()
            || trace.iter().zip(evaluation.trace()).any(|(a, b)| *a != b.result)
        { return Err(Error::Binding); }
        Ok(Self { proposal, scope, evidence })
    }

    pub fn purpose(&self) -> Purpose { Purpose::Experiment }
    pub fn original(&self) -> &Proposal { &self.proposal }
    pub fn scope(&self) -> &InterventionScope { &self.scope }

    /// Every invocation starts at the SAME original admission, even when later
    /// policy epochs, source values or effect outcomes have changed elsewhere.
    pub fn run(&self, edits: &[Intervention]) -> Result<ProposalCounterfactualReport, Error> {
        let (action, evidence, _) = apply_interventions(
            &self.proposal.action, &self.evidence, &self.scope, edits,
        )?;
        let trace = evidence.evaluate(&self.proposal.policy, &action)?;
        let counterfactual = *trace.last().ok_or(Error::Incomplete)?;
        let changes = trace.iter().zip(self.proposal.evaluation.trace()).enumerate()
            .filter_map(|(node, (after, before))| (*after != before.result).then_some(NodeChange {
                node, baseline: before.result, counterfactual: *after,
            })).collect();
        let next = match counterfactual {
            Truth::Violated => NextRequirement::ExactPolicyViolation,
            Truth::Satisfied if !trace.contains(&Truth::Unknown) => NextRequirement::FreshIndependentReview,
            _ => NextRequirement::MoreEvidence,
        };
        Ok(ProposalCounterfactualReport { scope: self.scope.id, attempt: self.proposal.attempt,
            initial_state: self.proposal.state, baseline: self.proposal.evaluation.result(),
            counterfactual, trace, changes, interventions: edits.to_vec(), next })
    }
}
