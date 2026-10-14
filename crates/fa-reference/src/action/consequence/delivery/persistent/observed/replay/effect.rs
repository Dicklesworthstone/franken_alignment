//! Join a selected review to its ORIGINAL authorization, dispatch and outcome.
//! This observer consumes successful Machine transitions; it drives no authority.
use super::{Error, Event, FileDeliverySnapshot, FileReviewReplay};
use super::super::{BaseEvent, Transition};
use crate::action::ActionState;
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::{EndpointOutcome, persistent::Reconciliation};
use crate::action::consequence::oversight::{OversightBroker, human::{HumanDisposition, HumanStatus}};

/// The original successful review whose automatic key was actually dispatched.
/// Merely completing or applying a review does not create this linkage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditedAuthorization {
    round: u64,
    review_revision: u64,
    revision: u64,
    input_revision: u64,
}
impl AuditedAuthorization {
    pub fn round(&self) -> u64 { self.round }
    pub fn review_revision(&self) -> u64 { self.review_revision }
    pub fn revision(&self) -> u64 { self.revision }
    pub fn input_revision(&self) -> u64 { self.input_revision }
}

/// Data from one original two-key dispatch, not either key or a sendable message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditedDispatch {
    revision: u64,
    authorization: AuditedAuthorization,
    human: HumanStatus,
}
impl AuditedDispatch {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn authorization(&self) -> &AuditedAuthorization { &self.authorization }
    pub fn human(&self) -> &HumanStatus { &self.human }
}

/// First terminal endpoint outcome observed in this verified canonical history.
/// A later query may lose retention; that cannot erase this historical observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditedOutcome { revision: u64, outcome: EndpointOutcome }
impl AuditedOutcome {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn outcome(&self) -> EndpointOutcome { self.outcome }
}

/// Last actual per-attempt reconciliation, including retained refusal/uncertainty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuditedReconciliation { revision: u64, result: Result<Reconciliation, Error> }
impl AuditedReconciliation {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn result(&self) -> Result<Reconciliation, Error> { self.result }
}

/// Exact selected-attempt observations at the fully verified historical cut.
/// None means no such original event was observed, NOT proved nonexecution.
/// This is the journal-as-publication reference sink, not a remote receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewEffectAudit {
    attempt: u64,
    round: u64,
    review_revision: u64,
    stage_at_review: ActionState,
    stage_at_end: ActionState,
    dispatch: Option<AuditedDispatch>,
    outcome: Option<AuditedOutcome>,
    reconciliation: Option<AuditedReconciliation>,
}
impl ReviewEffectAudit {
    pub fn attempt(&self) -> u64 { self.attempt }
    pub fn stage_at_review(&self) -> ActionState { self.stage_at_review }
    pub fn stage_at_end(&self) -> ActionState { self.stage_at_end }
    pub fn dispatch(&self) -> Option<&AuditedDispatch> { self.dispatch.as_ref() }
    pub fn outcome(&self) -> Option<&AuditedOutcome> { self.outcome.as_ref() }
    pub fn reconciliation(&self) -> Option<&AuditedReconciliation> { self.reconciliation.as_ref() }
    /// A later successful review of the SAME attempt is not the selected review.
    pub fn selected_review_dispatched(&self) -> bool {
        self.dispatch.is_some_and(|dispatch| dispatch.authorization.round == self.round
            && dispatch.authorization.review_revision == self.review_revision)
    }
}

/// The original complete review plus its selected-attempt effect observations.
/// Only complete original replay constructs this. It contains no live authority.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::replay::FileReviewEffectReplay;
/// fn authorize(audit: FileReviewEffectReplay) -> FilePermit { audit }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReviewEffectReplay {
    pub(super) review: FileReviewReplay,
    pub(super) effect: ReviewEffectAudit,
}
impl FileReviewEffectReplay {
    pub fn review(&self) -> &FileReviewReplay { &self.review }
    pub fn effect(&self) -> &ReviewEffectAudit { &self.effect }
}

#[derive(Clone, Copy)]
struct AppliedReview { round: u64, revision: u64, input_revision: u64, consequence: Consequence }

// Constant-size observational state, independent of journal length. No event
// list, payload clone, snapshot restore or alternate effect reducer lives here.
pub(super) struct EffectTrace {
    attempt: u64,
    applied: Option<AppliedReview>,
    authorized: Option<AuditedAuthorization>,
    dispatch: Option<AuditedDispatch>,
    outcome: Option<AuditedOutcome>,
    reconciliation: Option<AuditedReconciliation>,
}
impl EffectTrace {
    pub(super) fn new(attempt: u64) -> Self {
        Self { attempt, applied: None, authorized: None, dispatch: None,
            outcome: None, reconciliation: None }
    }

    // Call ONLY after the original complete Machine::apply returned successfully.
    // The selected Finish's attempt was read from its original session BEFORE
    // apply consumed it; an inner review refusal does not replace applied evidence.
    pub(super) fn observe(&mut self, event: &Event, transition: &Transition,
        reviewed_attempt: Option<u64>, revision: u64, broker: &OversightBroker) -> Result<(), Error>
    {
        match (event, transition) {
            (Event::Finish(round, ..), Transition::Reviewed(Ok(receipt)))
                if reviewed_attempt == Some(self.attempt) => {
                self.applied = Some(AppliedReview { round: *round, revision,
                    input_revision: receipt.input_revision, consequence: receipt.policy.control.decision.consequence });
            }
            (Event::Authorize(attempt, input_revision, _), Transition::Unit) if *attempt == self.attempt => {
                let applied = self.applied.ok_or(Error::Incomplete)?;
                if applied.consequence != Consequence::Continue || applied.input_revision != *input_revision {
                    return Err(Error::Binding);
                }
                self.authorized = Some(AuditedAuthorization { round: applied.round,
                    review_revision: applied.revision, revision, input_revision: *input_revision });
            }
            (Event::Dispatch(attempt, request, input_revision, _), Transition::Unit) if *attempt == self.attempt => {
                let authorization = self.authorized.take().ok_or(Error::Incomplete)?;
                let human = broker.human_status(*request)?;
                if authorization.input_revision != *input_revision || human.attempt != self.attempt
                    || human.disposition != HumanDisposition::Consumed { return Err(Error::Binding); }
                if self.dispatch.is_some() { return Err(Error::Duplicate); }
                self.dispatch = Some(AuditedDispatch { revision, authorization, human });
            }
            (Event::Core(BaseEvent::Publish(attempt)), Transition::Published(outcome))
                if *attempt == self.attempt => self.observe_outcome(revision, *outcome)?,
            (Event::PublishChecked(attempt, ..) | Event::PublishCredentialed(attempt, ..),
                Transition::PublicationChecked(publication)) if *attempt == self.attempt => {
                self.observe_outcome(revision, publication.outcome)?;
            }
            (Event::Core(BaseEvent::Reconcile(attempt) | BaseEvent::Seal(attempt)),
                Transition::Reconciled(result)) if *attempt == self.attempt => {
                self.observe_reconciliation(revision, Ok(*result))?;
            }
            (_, Transition::Swept(outcomes)) => {
                if let Some(result) = outcomes.get(&self.attempt) {
                    self.observe_reconciliation(revision, *result)?;
                }
            }
            (_, Transition::StopProgressed(sweep)) => {
                if let Some(result) = sweep.outcomes.get(&self.attempt) {
                    self.observe_reconciliation(revision, *result)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn observe_outcome(&mut self, revision: u64, outcome: EndpointOutcome) -> Result<(), Error> {
        if self.dispatch.is_none() { return Err(Error::Incomplete); }
        match self.outcome {
            Some(previous) if previous.outcome != outcome => return Err(Error::Binding),
            None => self.outcome = Some(AuditedOutcome { revision, outcome }),
            _ => {}
        }
        Ok(())
    }
    fn observe_reconciliation(&mut self, revision: u64, result: Result<Reconciliation, Error>)
        -> Result<(), Error>
    {
        if let Ok(Reconciliation::Resolved(outcome)) = result { self.observe_outcome(revision, outcome)?; }
        self.reconciliation = Some(AuditedReconciliation { revision, result });
        Ok(())
    }
    pub(super) fn finish(self, round: u64, completed: &FileDeliverySnapshot,
        journal: &FileDeliverySnapshot) -> Result<ReviewEffectAudit, Error>
    {
        let stage_at_review = *completed.control.ledger.stages.get(&self.attempt).ok_or(Error::Missing)?;
        let stage_at_end = *journal.control.ledger.stages.get(&self.attempt).ok_or(Error::Missing)?;
        Ok(ReviewEffectAudit { attempt: self.attempt, round, review_revision: completed.revision,
            stage_at_review, stage_at_end, dispatch: self.dispatch, outcome: self.outcome,
            reconciliation: self.reconciliation })
    }
}
