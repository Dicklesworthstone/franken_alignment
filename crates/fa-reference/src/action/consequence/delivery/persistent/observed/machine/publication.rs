//! Only the coupled owner can perform this read-check-publish/seal transition.
//! Reuse original policy, congress and endpoint operations; no refund happens here.
use super::{Machine, Transition};
use super::super::publication::{CheckedPublication, PublicationBasis};
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

    pub(super) fn publish_checked(&mut self, attempt: u64, supplied: Option<&Views>,
        snapshot: &Snapshot, now: ElapsedTick) -> Result<Transition, Error>
    {
        if !self.publication_guard { return Err(Error::WrongState); }
        self.observe(now)?;
        let query = self.broker.status_query(attempt)?;
        // Historical execution/nonexecution is not invalidated by a later source
        // failure. Retention loss, however, cannot produce a terminal assertion.
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
        let checked = supplied.ok_or(Error::Incomplete)
            .and_then(|views| self.capture(attempt, views))
            .and_then(|current| self.broker.revalidate_publication(attempt, approval, Some(&current), snapshot));
        let (receipt, basis) = match checked {
            Ok(()) => (self.endpoint.deliver(envelope)?, PublicationBasis::Revalidated),
            Err(error) => (self.endpoint.seal_unexecuted(&query)?, PublicationBasis::Rejected(error)),
        };
        // The candidate is only RAM. Original file replacement makes the chosen
        // endpoint transition visible before its result leaves FileOversight.
        // Broker acknowledgment remains a separate durable transaction.
        Ok(Transition::PublicationChecked(CheckedPublication { outcome: receipt.outcome(), basis }))
    }
}
