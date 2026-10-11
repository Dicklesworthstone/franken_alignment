//! Cooperative comparison of ORIGINAL publication requirements at one frozen cut.
//! The synchronous gate shares the opaque comparator, but retains its historical
//! atomic admission/charging contract. A precheck is never an effect permit.

use super::{
    ActualHelperInput, Error, FrozenAction, OpaqueJudgment, PublicationBasis,
    PublicationInvalidation, PublicationJudgment, PublicationOutcome,
};
use crate::witness::refinement::{RefinementBudget, RefinementOutcome, WitnessRefinement};

/// Per-poll and lifetime comparison work. Success describes ONLY the borrowed
/// evidence; source freshness, authority and the actual effect boundary remain
/// separate. This report is neither accepted by dispatch nor stored as approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationProgress {
    pub outcome: PublicationOutcome,
    pub spent: RefinementBudget,
    pub total: RefinementBudget,
}

/// One immutable reviewed action and its complete structured/opaque input cut.
/// No payloads are cloned and no explanation can remove a dependency. Polling
/// budgets cannot refill the fixed lifetime ceiling; abandoning this object
/// releases borrows, not effect charges, keys, or source-capture obligations.
///
/// ```compile_fail,E0506
/// use fa_reference::action::consequence::oversight::publication::{
///     PublicationBasis, PublicationJudgment,
/// };
/// use fa_reference::full_input::ActualHelperInput;
/// use fa_reference::witness::refinement::RefinementBudget;
/// fn immutable_view(j: &PublicationJudgment, input: &mut ActualHelperInput,
///     replacement: ActualHelperInput) {
///     let mut work = j.begin_refinement(PublicationBasis {
///         structured: None, opaque: Some(input),
///     }, RefinementBudget { steps: 100, value_bytes: 100 });
///     *input = replacement;
///     let _ = work.advance(RefinementBudget::default());
/// }
/// ```
#[must_use = "unfinished comparison is not evidence of validity"]
pub struct PublicationRefinement<'a> {
    judgment: &'a PublicationJudgment,
    structured: Option<WitnessRefinement<'a>>,
    opaque: Option<OpaqueComparison<'a>>,
    ceiling: RefinementBudget,
    total: RefinementBudget,
    terminal: Option<PublicationOutcome>,
}

impl PublicationJudgment {
    /// Compare this judgment's OWN frozen action and retained requirements.
    /// Creation examines lane presence only; it performs no payload comparison.
    /// All referenced inputs must stay immutable until the session is released.
    /// Missing lanes refuse, extra lanes do not silently change the contract.
    ///
    /// This is the explicitly selected cooperative cost profile: every byte
    /// chunk and fixed-size metadata record costs one step. The legacy validate
    /// API keeps its atomic whole-input charge and exhaustion behavior unchanged.
    pub fn begin_refinement<'a>(
        &'a self,
        basis: PublicationBasis<'a>,
        ceiling: RefinementBudget,
    ) -> PublicationRefinement<'a> {
        let terminal = if (self.structured.is_some() && basis.structured.is_none())
            || (self.opaque.is_some() && basis.opaque.is_none())
        {
            Some(PublicationOutcome::Refused(Error::Incomplete))
        } else if self.structured.is_some() != basis.structured.is_some()
            || self.opaque.is_some() != basis.opaque.is_some()
        {
            Some(PublicationOutcome::Refused(Error::Binding))
        } else {
            None
        };
        PublicationRefinement {
            judgment: self,
            structured: self.structured.as_ref().zip(basis.structured)
                .map(|(judgment, (snapshot, frontiers))| judgment.begin_refinement(snapshot, frontiers)),
            opaque: self.opaque.as_ref().zip(basis.opaque)
                .map(|(judgment, actual)| OpaqueComparison::new(judgment, actual)),
            ceiling,
            total: RefinementBudget::default(),
            terminal,
        }
    }
}

impl PublicationRefinement<'_> {
    pub fn action(&self) -> &FrozenAction { self.judgment.action() }
    pub fn total_work(&self) -> RefinementBudget { self.total }
    pub fn ceiling(&self) -> RefinementBudget { self.ceiling }

    /// Discard unfinished comparison and return work already performed. No
    /// success report, source-capture refresh or refundable resource is returned.
    #[must_use]
    pub fn cancel(self) -> RefinementBudget { self.total }

    /// Advance both original evidence lanes under ONE per-poll allowance and
    /// ONE fixed lifetime ceiling. A depleted poll can resume; a depleted total
    /// ceiling latches Limit. Completed/refused results are idempotent and cost
    /// no additional work. Even single-byte polls never rescan a completed prefix.
    pub fn advance(&mut self, budget: RefinementBudget) -> PublicationProgress {
        let mut spent = RefinementBudget::default();
        if let Some(outcome) = self.terminal { return self.report(outcome, spent); }
        let available = RefinementBudget {
            steps: budget.steps.min(self.ceiling.steps - self.total.steps),
            value_bytes: budget.value_bytes.min(self.ceiling.value_bytes - self.total.value_bytes),
        };
        if let Some(cursor) = &mut self.structured {
            let report = cursor.advance(available);
            spent = report.spent;
            self.charge(spent);
            let outcome = match report.outcome {
                RefinementOutcome::StillValid => None,
                RefinementOutcome::Invalidated { reason, witness } => Some(
                    PublicationOutcome::Invalidated(PublicationInvalidation::Structured { reason, witness }),
                ),
                RefinementOutcome::NeedsRefinement { minimum } => Some(
                    PublicationOutcome::NeedsRefinement { minimum },
                ),
                RefinementOutcome::Refused(error) => Some(PublicationOutcome::Refused(error)),
            };
            if let Some(outcome) = outcome { return self.finish_poll(outcome, spent); }
            self.structured = None;
        }
        if let Some(cursor) = &mut self.opaque {
            let remaining = RefinementBudget {
                steps: available.steps - spent.steps,
                value_bytes: available.value_bytes - spent.value_bytes,
            };
            let (outcome, work) = cursor.advance(remaining);
            self.charge(work);
            spent.steps += work.steps;
            spent.value_bytes += work.value_bytes;
            match outcome {
                OpaqueOutcome::Equal => self.opaque = None,
                OpaqueOutcome::Changed => return self.finish_poll(
                    PublicationOutcome::Invalidated(PublicationInvalidation::OpaqueInput), spent,
                ),
                OpaqueOutcome::Pending(minimum) => return self.finish_poll(
                    PublicationOutcome::NeedsRefinement { minimum }, spent,
                ),
            }
        }
        self.finish_poll(PublicationOutcome::StillValid, spent)
    }

    fn charge(&mut self, work: RefinementBudget) {
        // Both original cursors receive at most ceiling-total; the opaque lane
        // additionally receives at most the unused portion of this poll. These
        // additions are bounded even when a ceiling or poll uses u64::MAX.
        self.total.steps += work.steps;
        self.total.value_bytes += work.value_bytes;
    }

    fn finish_poll(&mut self, mut outcome: PublicationOutcome, spent: RefinementBudget) -> PublicationProgress {
        if let PublicationOutcome::NeedsRefinement { minimum } = outcome {
            if minimum.steps > self.ceiling.steps - self.total.steps
                || minimum.value_bytes > self.ceiling.value_bytes - self.total.value_bytes
            {
                outcome = PublicationOutcome::Refused(Error::Limit);
            }
        }
        if !matches!(outcome, PublicationOutcome::NeedsRefinement { .. }) {
            self.terminal = Some(outcome);
        }
        self.report(outcome, spent)
    }

    fn report(&self, outcome: PublicationOutcome, spent: RefinementBudget) -> PublicationProgress {
        PublicationProgress { outcome, spent, total: self.total }
    }
}

#[derive(Clone, Copy)]
enum OpaqueStage {
    Header,
    Submitted(usize),
    Profile(usize),
    Part(usize),
    Omission(usize),
    Done(bool),
}

#[derive(Clone, Copy)]
enum OpaqueOutcome {
    Equal,
    Changed,
    Pending(RefinementBudget),
}

struct OpaqueComparison<'a> {
    captured: &'a ActualHelperInput,
    actual: &'a ActualHelperInput,
    stage: OpaqueStage,
}

impl<'a> OpaqueComparison<'a> {
    fn new(judgment: &'a OpaqueJudgment, actual: &'a ActualHelperInput) -> Self {
        Self { captured: judgment.witness().actual_input(), actual, stage: OpaqueStage::Header }
    }

    fn advance(&mut self, budget: RefinementBudget) -> (OpaqueOutcome, RefinementBudget) {
        let mut spent = RefinementBudget::default();
        loop {
            // Empty components and finished phases require no scan. At most
            // four such transitions can occur between charged steps.
            match self.stage {
                OpaqueStage::Submitted(offset) if offset == self.captured.submitted_bytes().len() => {
                    self.stage = OpaqueStage::Profile(0); continue;
                }
                OpaqueStage::Profile(offset) if offset == self.captured.input_profile().profile_bytes.len() => {
                    self.stage = OpaqueStage::Part(0); continue;
                }
                OpaqueStage::Part(index) if index == self.captured.ordered_parts().len() => {
                    self.stage = OpaqueStage::Omission(0); continue;
                }
                OpaqueStage::Omission(index) if index == self.captured.omissions().len() => {
                    self.stage = OpaqueStage::Done(true); continue;
                }
                OpaqueStage::Done(equal) => return (
                    if equal { OpaqueOutcome::Equal } else { OpaqueOutcome::Changed }, spent,
                ),
                _ => {}
            }
            let minimum = RefinementBudget {
                steps: 1,
                value_bytes: u64::from(matches!(self.stage, OpaqueStage::Submitted(_) | OpaqueStage::Profile(_))),
            };
            let remaining_bytes = budget.value_bytes - spent.value_bytes;
            if spent.steps == budget.steps || remaining_bytes < minimum.value_bytes {
                return (OpaqueOutcome::Pending(minimum), spent);
            }
            spent.steps += 1;
            match self.stage {
                OpaqueStage::Header => {
                    let left = self.captured.input_profile();
                    let right = self.actual.input_profile();
                    let equal = left.profile_id == right.profile_id
                        && left.tokenizer_epoch == right.tokenizer_epoch
                        && left.policy_epoch == right.policy_epoch
                        && left.model_epoch == right.model_epoch
                        && left.profile_bytes.len() == right.profile_bytes.len()
                        && self.captured.submitted_bytes().len() == self.actual.submitted_bytes().len()
                        && self.captured.ordered_parts().len() == self.actual.ordered_parts().len()
                        && self.captured.omissions().len() == self.actual.omissions().len();
                    self.stage = if equal { OpaqueStage::Submitted(0) } else { OpaqueStage::Done(false) };
                }
                OpaqueStage::Submitted(offset) | OpaqueStage::Profile(offset) => {
                    let submitted = matches!(self.stage, OpaqueStage::Submitted(_));
                    let (left, right) = if submitted {
                        (self.captured.submitted_bytes(), self.actual.submitted_bytes())
                    } else {
                        (self.captured.input_profile().profile_bytes.as_slice(), self.actual.input_profile().profile_bytes.as_slice())
                    };
                    let bytes = remaining_bytes.min((left.len() - offset) as u64) as usize;
                    let end = offset + bytes;
                    spent.value_bytes += bytes as u64;
                    self.stage = if left[offset..end] != right[offset..end] {
                        OpaqueStage::Done(false)
                    } else if submitted {
                        OpaqueStage::Submitted(end)
                    } else {
                        OpaqueStage::Profile(end)
                    };
                }
                OpaqueStage::Part(index) => {
                    self.stage = if self.captured.ordered_parts()[index] == self.actual.ordered_parts()[index] {
                        OpaqueStage::Part(index + 1)
                    } else { OpaqueStage::Done(false) };
                }
                OpaqueStage::Omission(index) => {
                    self.stage = if self.captured.omissions()[index] == self.actual.omissions()[index] {
                        OpaqueStage::Omission(index + 1)
                    } else { OpaqueStage::Done(false) };
                }
                OpaqueStage::Done(_) => unreachable!("terminal handled before charging"),
            }
        }
    }
}

/// The live synchronous gate consumes the SAME opaque comparison. Admission
/// and its full historical charge happen in validate BEFORE this call; internal
/// chunk accounting is deliberately not substituted into existing journal data.
/// OpaqueJudgment::valid_at remains an independent eager differential oracle.
pub(super) fn opaque_equal(judgment: &OpaqueJudgment, actual: &ActualHelperInput) -> Result<bool, Error> {
    let (outcome, _) = OpaqueComparison::new(judgment, actual).advance(RefinementBudget {
        steps: u64::MAX, value_bytes: u64::MAX,
    });
    match outcome {
        OpaqueOutcome::Equal => Ok(true),
        OpaqueOutcome::Changed => Ok(false),
        OpaqueOutcome::Pending(_) => Err(Error::Incomplete),
    }
}

#[cfg(test)]
mod tests;
