//! Forward actual applied review cases and independent labels to the native ledger.
use super::{Machine, Transition};
use super::super::credibility::{CredibilityEvent, FileCredibilityUpdate};
use crate::action::consequence::oversight::credibility::{
    CredibilityPromotion, EvaluationProtocol, IndependentEvaluator, MAX_WEIGHT_CHANGES,
};
use crate::Error;
use std::collections::BTreeMap;

pub(super) struct CredibilityState {
    protocol: EvaluationProtocol,
    evaluator: IndependentEvaluator,
    // Only exact operation identity and index into ORIGINAL retained promotions.
    // No copied score history, alternative denominator, or mutable congress.
    operations: BTreeMap<u64, (FileCredibilityUpdate, usize)>,
}

impl Machine {
    pub(in super::super) fn credibility_contract(&self) -> Option<&EvaluationProtocol> {
        self.credibility.as_ref().map(|state| &state.protocol)
    }
    pub(in super::super) fn credibility_promotion(&self, operation: u64) -> Result<&CredibilityPromotion, Error> {
        let state = self.credibility.as_ref().ok_or(Error::Incomplete)?;
        let (_, index) = state.operations.get(&operation).ok_or(Error::Missing)?;
        self.broker.credibility_promotions()?.get(*index).ok_or(Error::Binding)
    }
    pub(in super::super) fn credibility_retry(&self, update: &FileCredibilityUpdate)
        -> Result<Option<&CredibilityPromotion>, Error>
    {
        let state = self.credibility.as_ref().ok_or(Error::Incomplete)?;
        match state.operations.get(&update.operation) {
            Some((original, _)) if original == update => Ok(Some(self.credibility_promotion(update.operation)?)),
            Some(_) => Err(Error::Binding),
            None => Ok(None),
        }
    }
    pub(super) fn apply_credibility(&mut self, event: &CredibilityEvent) -> Result<Transition, Error> {
        match event {
            CredibilityEvent::Enable(protocol) => {
                if self.credibility.is_some() { return Err(Error::Duplicate); }
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || self.broker.stop_receipt().is_some() { return Err(Error::WrongState); }
                let evaluator = self.broker.enable_credibility(protocol.clone())?;
                if !self.publication_guard { self.enable_publication_guard()?; }
                self.credibility = Some(CredibilityState {
                    protocol: protocol.clone(), evaluator, operations: BTreeMap::new(),
                });
            }
            CredibilityEvent::Assess(round, assessment) => {
                let ticket = self.broker.evaluation_ticket(*round)?;
                let evaluator = &self.credibility.as_ref().ok_or(Error::Incomplete)?.evaluator;
                let label = evaluator.assess(&ticket, *assessment)?;
                return Ok(Transition::EvaluationRecorded(self.broker.record_evaluation(label)?));
            }
            CredibilityEvent::Promote(update) => {
                update.validate()?;
                if !self.clock_ready { return Err(Error::Incomplete); }
                if self.broker.stop_receipt().is_some() || self.broker.inspect().suspended {
                    return Err(Error::WrongState);
                }
                let state = self.credibility.as_ref().ok_or(Error::Incomplete)?;
                if state.operations.contains_key(&update.operation) { return Err(Error::Duplicate); }
                if state.operations.len() >= MAX_WEIGHT_CHANGES { return Err(Error::Limit); }
                let index = self.broker.credibility_promotions()?.len();
                self.broker.promote_credibility(update.expected_control_sequence,
                    update.expected_authority_epoch, update.expected_evaluation_revision)?;
                // Reuse original withdrawal paths. Do not discard sent envelopes
                // or refund unresolved effects: their own receipts still decide.
                self.withdraw_keys()?;
                self.withdraw_identity()?;
                self.withdraw_policy_campaigns()?;
                self.automatic.clear();
                self.sessions.clear();
                self.credibility.as_mut().expect("configured evaluation").operations
                    .insert(update.operation, (update.clone(), index));
            }
        }
        Ok(Transition::Unit)
    }
}
