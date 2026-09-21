//! Only the coupled owner can perform this read-check-publish/seal transition.
//! Reuse original policy, congress and endpoint operations; no refund happens here.
use super::{Machine, Transition};
use super::super::publication::{CheckedPublication, PublicationBasis};
use super::super::publication::witness_gate::WitnessEvent;
use super::super::publication::witness_gate::freshness::FreshnessEvent;
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
            WitnessEvent::Freshness(event) => {
                match event {
                    FreshnessEvent::Enable(policy) => self.broker.enable_publication_change_freshness(*policy)?,
                    FreshnessEvent::SnapshotFallback => self.broker.enable_publication_snapshot_fallback()?,
                    FreshnessEvent::Unavailable(source) => self.broker.publication_changes_unavailable(*source)?,
                    FreshnessEvent::Observed(heartbeat, now) => {
                        self.observe(*now)?;
                        // Restrictive observations remain in the original replay;
                        // a future head never fills its missing change records.
                        self.broker.record_publication_heartbeat(*heartbeat)?;
                    }
                }
                Ok(Transition::Unit)
            }
            WitnessEvent::ChangeProfile(policy) => {
                self.broker.enable_publication_changes(*policy)?;
                Ok(Transition::Unit)
            }
            WitnessEvent::Change(notice) => {
                self.broker.record_publication_change(*notice)?;
                Ok(Transition::Unit)
            }
            WitnessEvent::Enable(limits) => {
                let control = self.broker.inspect();
                if !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
                    || control.sequence != 0 || control.suspended || self.broker.stop_receipt().is_some()
                { return Err(Error::WrongState); }
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
            WitnessEvent::SourceBind(attempt, binding) => {
                let action = self.actions.get(attempt).ok_or(Error::Missing)?;
                binding.check_action(*attempt, action)?;
                let judgment = binding.evidence.capture_for(action.clone())?;
                let original = binding.evidence.original().materialize()?;
                let identity = binding.identity();
                // Both operations occur on the unexposed transaction candidate.
                self.broker.bind_publication_judgment(*attempt, judgment)?;
                match binding.input_cut() {
                    Some(cut) => self.broker.bind_publication_source_at_cut(*attempt, identity.source, identity.generation, original, cut)?,
                    None => self.broker.bind_publication_source(*attempt, identity.source, identity.generation, original)?,
                }
                Ok(Transition::Unit)
            }
            WitnessEvent::CapturedOrDefer(attempt, revision, capture) => {
                capture.check_action(*attempt, self.actions.get(attempt).ok_or(Error::Missing)?)?;
                let identity = capture.identity();
                let inputs = capture.inputs().materialize()?;
                let cut = capture.input_cut().ok_or(Error::Incomplete)?;
                let observed = self.broker.record_captured_publication_inputs_or_defer(*attempt, *revision,
                    identity.source, identity.generation, inputs, cut)?;
                // No serialized success bit is imported. Native replay recomputes
                // both Installed and Deferred, retaining their exact source floor.
                Ok(Transition::Inputs(observed.revision()))
            }
            WitnessEvent::Captured(attempt, revision, capture) => {
                capture.check_action(*attempt, self.actions.get(attempt).ok_or(Error::Missing)?)?;
                let identity = capture.identity();
                let inputs = capture.inputs().materialize()?;
                let next = match capture.input_cut() {
                    Some(cut) => self.broker.record_captured_publication_inputs_at_cut(*attempt, *revision,
                        identity.source, identity.generation, inputs, cut)?,
                    None => self.broker.record_captured_publication_inputs(*attempt, *revision,
                        identity.source, identity.generation, inputs)?,
                };
                Ok(Transition::Inputs(next))
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
