//! Independent evaluation labels and weight-only promotion in the original journal.
//! Native credibility accounting remains authoritative; no saved score is imported.
mod codec;
pub(super) use codec::{read, write};

use super::{Event, FileOversight, JournalError, Transition};
use crate::action::consequence::oversight::credibility::{
    Assessment, CredibilityPromotion, CredibilityReport, EvaluationCase, EvaluationProtocol,
};
use crate::Error;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCredibilityUpdate {
    pub operation: u64,
    pub expected_control_sequence: u64,
    pub expected_authority_epoch: u64,
    pub expected_evaluation_revision: u64,
}
impl FileCredibilityUpdate {
    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.operation == 0 { Err(Error::InvalidInput) } else { Ok(()) }
    }
}

/// One original applied review, not a supplied verdict or a ground-truth label.
/// Recovery requires a new owner-branded ticket even for the same historical case.
#[derive(Clone, Debug)]
pub struct FileEvaluationTicket {
    issuer: Rc<()>,
    case: EvaluationCase,
}
impl FileEvaluationTicket {
    pub fn case(&self) -> &EvaluationCase { &self.case }
}

/// Assign this role to the independent evaluator, never the actor or its helper.
/// Process-local custody models role separation; it is not authentication.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileIndependentEvaluator;
/// fn duplicate(evaluator: FileIndependentEvaluator) { let _ = evaluator.clone(); }
/// ```
#[derive(Debug)]
pub struct FileIndependentEvaluator { pub(super) issuer: Rc<()> }

#[derive(Clone)]
pub(super) enum CredibilityEvent {
    Enable(EvaluationProtocol),
    Assess(u64, Assessment),
    Promote(FileCredibilityUpdate),
}

impl FileIndependentEvaluator {
    /// Exact repeats return false without another journal record. A censored
    /// label can resolve once through the original evaluator; final labels and
    /// origin identities cannot be overwritten. No consensus becomes truth.
    pub fn assess(&self, host: &mut FileOversight, revision: u64,
        ticket: &FileEvaluationTicket, assessment: Assessment) -> Result<bool, JournalError>
    {
        if host.fault.is_some() { return Err(JournalError::Unavailable); }
        if !Rc::ptr_eq(&self.issuer, &host.issuer) || !Rc::ptr_eq(&ticket.issuer, &host.issuer) {
            return Err(Error::Binding.into());
        }
        let round = ticket.case.round;
        if host.machine.broker.evaluation_ticket(round)?.case() != &ticket.case {
            return Err(Error::Binding.into());
        }
        if host.machine.broker.evaluation_history(round)?.last() == Some(&assessment) { return Ok(false); }
        match host.transact(revision, Event::Credibility(CredibilityEvent::Assess(round, assessment)))? {
            Transition::EvaluationRecorded(changed) => Ok(changed),
            _ => unreachable!("native independent assessment"),
        }
    }
}

impl FileOversight {
    /// Freeze the native protocol before ANY proposal/request and return the
    /// sole external labeling role only after canonical acknowledgment. First
    /// publication is guarded; there is no disable or protocol replacement API.
    pub fn enable_credibility(&mut self, revision: u64, protocol: EvaluationProtocol)
        -> Result<FileIndependentEvaluator, JournalError>
    {
        self.transact(revision, Event::Credibility(CredibilityEvent::Enable(protocol)))?;
        Ok(FileIndependentEvaluator { issuer: Rc::clone(&self.issuer) })
    }
    pub fn credibility_protocol(&self) -> Result<Option<&EvaluationProtocol>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.credibility_contract())
    }
    pub fn evaluation_ticket(&self, round: u64) -> Result<FileEvaluationTicket, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        let case = self.machine.broker.evaluation_ticket(round)?.case().clone();
        Ok(FileEvaluationTicket { issuer: Rc::clone(&self.issuer), case })
    }
    pub fn evaluation_history(&self, round: u64) -> Result<&[Assessment], JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.evaluation_history(round)?)
    }
    pub fn credibility_report(&self) -> Result<CredibilityReport, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.broker.credibility_report().map_err(Into::into)
    }
    pub fn credibility_promotion(&self, operation: u64) -> Result<&CredibilityPromotion, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.credibility_promotion(operation)?)
    }

    /// Supervisor-only native deterministic weight rule, not a roster, threshold
    /// or policy setter. Every started current-policy round and independent label
    /// participates. Exact retries return the ORIGINAL receipt, never new rights.
    ///
    /// ```compile_fail,E0308
    /// use fa_reference::action::consequence::oversight::credibility::CredibilityPromotion;
    /// use fa_reference::action::consequence::delivery::persistent::FilePermit;
    /// fn authorize(report: CredibilityPromotion) -> FilePermit { report }
    /// ```
    pub fn promote_credibility(&mut self, revision: u64, update: &FileCredibilityUpdate)
        -> Result<CredibilityPromotion, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        update.validate()?;
        if let Some(receipt) = self.machine.credibility_retry(update)? { return Ok(receipt.clone()); }
        self.transact(revision, Event::Credibility(CredibilityEvent::Promote(update.clone())))?;
        Ok(self.credibility_promotion(update.operation)?.clone())
    }
}
