//! Only the coupled owner can perform this read-check-publish/seal transition.
//! Reuse original policy, congress and endpoint operations; no refund happens here.
use super::{Machine, Transition};
use super::super::publication::{CheckedPublication, PublicationBasis};
use super::super::publication::witness_gate::WitnessEvent;
use super::super::views::Views;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::EndpointStatus;
use crate::{Error, Snapshot};

impl Machine {
    pub(super) fn enable_publication_guard(&mut self) -> Result<Transition, Error> {
        if self.publication_guard { return Err(Error::Duplicate); }
        let control = self.broker.inspect();
        if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
            || control.sequence != 0 || control.suspended || self.broker.stop_receipt().is_some()
        { return Err(Error::WrongState); }
        self.publication_guard = true;
        Ok(Transition::Unit)
    }

    pub(super) fn apply_publication_witness(&mut self, event: &WitnessEvent) -> Result<Transition, Error> {
        match event {
            WitnessEvent::Enable(limits) => {
                let control = self.broker.inspect();
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || control.sequence != 0 || control.suspended || self.broker.stop_receipt().is_some()
                { return Err(Error::WrongState); }
                // The original broker validates limits/duplicate configuration
                // before mutation. No later fallible operation can partially
                // install only one half of this mandatory profile.
                self.broker.enable_publication_validation(*limits)?;
                self.publication_guard = true;
                Ok(Transition::Unit)
            }
            WitnessEvent::Bind(attempt, evidence) => {
                let action = self.actions.get(attempt).ok_or(Error::Missing)?.clone();
                let judgment = evidence.capture_for(action)?;
                self.broker.bind_publication_judgment(*attempt, judgment)?;
                Ok(Transition::Unit)
            }
            WitnessEvent::Inputs(attempt, revision, inputs) => {
                let current = inputs.as_ref().map(|inputs| inputs.materialize()).transpose()?;
                Ok(Transition::Inputs(self.broker.record_publication_inputs(*attempt, *revision, current)?))
            }
        }
    }

    pub(in super::super) fn publication_needs_evidence(&self, attempt: u64) -> Result<bool, Error> {
        if !self.publication_guard { return Err(Error::WrongState); }
        if !self.clock_ready { return Err(Error::Incomplete); }
        let now = self.now()?;
        let query = self.broker.status_query(attempt)?;
        match self.endpoint.status(&query)? {
            EndpointStatus::AwaitingResolution => Ok(self.envelopes.get(&attempt)
                .is_some_and(|envelope| now < envelope.request().execution_deadline())),
            EndpointStatus::Resolved(_) | EndpointStatus::RetentionExpired => Ok(false),
        }
    }

    pub(super) fn publish_checked(&mut self, attempt: u64, supplied: Option<&Views>,
        snapshot: &Snapshot, now: ElapsedTick, credentialed: bool) -> Result<Transition, Error>
    {
        if !self.publication_guard { return Err(Error::WrongState); }
        if credentialed {
            if self.credential_policy.is_none() { return Err(Error::Binding); }
            if self.credential_revoked { return Err(Error::WrongState); }
        }
        self.observe(now)?;
        let query = self.broker.status_query(attempt)?;
        match self.endpoint.status(&query)? {
            EndpointStatus::Resolved(receipt) => return Ok(Transition::PublicationChecked(CheckedPublication {
                outcome: receipt.outcome(), basis: PublicationBasis::PreviouslyResolved,
            })),
            EndpointStatus::RetentionExpired => return Err(Error::Stale),
            EndpointStatus::AwaitingResolution => {}
        }
        let envelope = self.envelopes.get(&attempt).ok_or(Error::Missing)?;
        if now >= envelope.request().execution_deadline() {
            let receipt = self.endpoint.resolve_expired(&query)?;
            return Ok(Transition::PublicationChecked(CheckedPublication {
                outcome: receipt.outcome(), basis: PublicationBasis::DeadlineElapsed,
            }));
        }
        let approval = envelope.request().approval().ok_or(Error::Incomplete)?;
        // Dispatch may have happened before newer negative evidence. Recheck
        // the original witness binding here too, immediately before publication.
        // Existing outcomes/expiry above never demand new evidence or refund an
        // executed effect. A failed fresh check seals, rather than rerolls, it.
        let checked = self.broker.revalidate_publication_witnesses(attempt)
            .and_then(|()| supplied.ok_or(Error::Incomplete))
            .and_then(|views| self.capture(attempt, views))
            .and_then(|current| self.broker.revalidate_publication(attempt, approval, Some(&current), snapshot));
        let (receipt, basis) = match checked {
            Ok(()) => {
                if self.credential_policy.is_some() && !credentialed { return Err(Error::Incomplete); }
                (self.endpoint.deliver(envelope)?, PublicationBasis::Revalidated)
            }
            Err(error) => {
                (self.endpoint.seal_unexecuted(&query)?, PublicationBasis::Rejected(error))
            }
        };
        Ok(Transition::PublicationChecked(CheckedPublication { outcome: receipt.outcome(), basis }))
    }
}
