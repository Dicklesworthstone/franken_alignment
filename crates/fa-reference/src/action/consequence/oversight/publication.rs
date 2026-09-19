//! Final-cut conjunction of structured dependencies and opaque whole-input evidence.
//!
//! This is an L4 reference gate for FA-062, not a new authority or an adapter
//! authenticator. Binding asserts that the supplied judgments reviewed this exact
//! action. The caller must capture the live adapter/helper inputs at publication;
//! a historical successful report is never accepted as a permit.

use crate::action::FrozenAction;
use crate::full_input::{ActualHelperInput, OpaqueJudgment};
use crate::product_frontier::ProductFrontiers;
use crate::witness::refinement::{RefinementBudget, RefinementOutcome};
use crate::witness::{Invalidation, WitnessJudgment, WitnessSnapshot};
use crate::Error;

/// Immutable evidence requirements. There is no operation to remove a lane,
/// replace a judgment, or rebind the action after review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationJudgment {
    action: FrozenAction,
    structured: Option<WitnessJudgment>,
    opaque: Option<OpaqueJudgment>,
}

/// Borrow the actual final-cut inputs for exactly the retained lanes. These
/// references cannot be mutated while validation runs. They are observations,
/// not proof that an adapter is authenticated or that a remote cut is current.
#[derive(Clone, Copy, Default)]
pub struct PublicationBasis<'a> {
    pub structured: Option<(&'a WitnessSnapshot, &'a ProductFrontiers)>,
    pub opaque: Option<&'a ActualHelperInput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationInvalidation {
    Structured { reason: Invalidation, witness: Option<usize> },
    OpaqueInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationOutcome {
    StillValid,
    Invalidated(PublicationInvalidation),
    NeedsRefinement { minimum: RefinementBudget },
    Refused(Error),
}

/// Work is retained on success, invalidation, refusal, and budget exhaustion.
/// These are logical witness/helper comparison costs, not CPU or effect charges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationReport {
    pub outcome: PublicationOutcome,
    pub spent: RefinementBudget,
}

impl PublicationReport {
    /// Fail-closed conversion for an immediate dispatch call. A broker must
    /// recompute this report itself; accepting a caller's report is not safe.
    pub fn require_valid(self) -> Result<(), Error> {
        match self.outcome {
            PublicationOutcome::StillValid => Ok(()),
            PublicationOutcome::Invalidated(_) => Err(Error::Stale),
            PublicationOutcome::NeedsRefinement { .. } => Err(Error::Incomplete),
            PublicationOutcome::Refused(error) => Err(error),
        }
    }
}

impl PublicationJudgment {
    pub fn bind(
        action: FrozenAction,
        structured: Option<WitnessJudgment>,
        opaque: Option<OpaqueJudgment>,
    ) -> Result<Self, Error> {
        if structured.is_none() && opaque.is_none() {
            return Err(Error::Incomplete);
        }
        Ok(Self { action, structured, opaque })
    }

    pub fn action(&self) -> &FrozenAction { &self.action }

    /// Recheck from scratch at the supplied final cut. Partial work is not
    /// carried across mutable cuts. The existing FA-060 cursor remains the API
    /// for resumable refinement against one immutable snapshot.
    ///
    /// Structured validation and opaque validation share one budget. Opaque
    /// equality charges all retained byte payloads and bounded metadata entries
    /// before comparing them, including policy/tokenizer/model semantics,
    /// omissions and part order. Helper explanations never prune dependencies.
    pub fn validate(
        &self,
        action: &FrozenAction,
        basis: PublicationBasis<'_>,
        budget: RefinementBudget,
    ) -> PublicationReport {
        let mut spent = RefinementBudget::default();
        let refused = |error| PublicationReport {
            outcome: PublicationOutcome::Refused(error),
            spent: RefinementBudget::default(),
        };
        if &self.action != action { return refused(Error::Binding); }
        if (self.structured.is_some() && basis.structured.is_none())
            || (self.opaque.is_some() && basis.opaque.is_none())
        {
            return refused(Error::Incomplete);
        }
        if self.structured.is_some() != basis.structured.is_some()
            || self.opaque.is_some() != basis.opaque.is_some()
        {
            return refused(Error::Binding);
        }
        if let (Some(judgment), Some((snapshot, frontiers))) = (&self.structured, basis.structured) {
            let report = judgment.begin_refinement(snapshot, frontiers).advance(budget);
            spent = report.spent;
            let outcome = match report.outcome {
                RefinementOutcome::StillValid => None,
                RefinementOutcome::Invalidated { reason, witness } => Some(
                    PublicationOutcome::Invalidated(PublicationInvalidation::Structured { reason, witness }),
                ),
                RefinementOutcome::NeedsRefinement { minimum } => {
                    Some(PublicationOutcome::NeedsRefinement { minimum })
                }
                RefinementOutcome::Refused(error) => Some(PublicationOutcome::Refused(error)),
            };
            if let Some(outcome) = outcome { return PublicationReport { outcome, spent }; }
        }
        if let (Some(judgment), Some(actual)) = (&self.opaque, basis.opaque) {
            let captured = judgment.witness().actual_input();
            // Constructors bound these sizes to 72 KiB and 192 metadata items.
            // Equality can short-circuit, but never compares more than charged.
            let minimum = RefinementBudget {
                steps: 1 + captured.ordered_parts().len() as u64 + captured.omissions().len() as u64,
                value_bytes: captured.submitted_bytes().len() as u64
                    + captured.input_profile().profile_bytes.len() as u64,
            };
            if budget.steps - spent.steps < minimum.steps
                || budget.value_bytes - spent.value_bytes < minimum.value_bytes
            {
                return PublicationReport {
                    outcome: PublicationOutcome::NeedsRefinement { minimum }, spent,
                };
            }
            spent.steps += minimum.steps;
            spent.value_bytes += minimum.value_bytes;
            if !judgment.valid_at(actual) {
                return PublicationReport {
                    outcome: PublicationOutcome::Invalidated(PublicationInvalidation::OpaqueInput), spent,
                };
            }
        }
        PublicationReport { outcome: PublicationOutcome::StillValid, spent }
    }
}

#[cfg(test)]
mod tests;
