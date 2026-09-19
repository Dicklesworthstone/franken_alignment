//! Credentialed completion over the original two-key broker and journal sink.
//! Credentials are checked live; only the nonsecret original events are retained.
//! This is not a transaction over a remote provider or an independent file sink.

use super::FileCredentialPermit;
use super::super::{
    BaseEvent, Event, FileOversight, JournalError, JournalFailure, JournalIo, Machine,
    Reconciliation, Transition, journal,
};
use super::super::publication::{CheckedCompletion, CheckedPublication};
use crate::{Error, Snapshot};
use std::rc::Rc;

impl FileOversight {
    /// Consume both existing approvals, perform credential-required checked
    /// publication, and reconcile the original endpoint receipt in ONE canonical
    /// replacement. The original Time, Dispatch, PublishCredentialed, Reconcile
    /// events remain unchanged and the journal revision advances by four.
    ///
    /// The credential must belong to this owner, its bound perimeter policy and
    /// the current unrevoked generation. Secret bytes never enter an event, a
    /// receipt or a replay projection. Recovery invalidates all old live keys.
    ///
    /// Every original source, witness, identity, policy and two-key check still
    /// runs. A source-bound attempt needing two independent acquisitions should
    /// use complete_publication_from_source; this API does not manufacture fresh
    /// captures from the supplied snapshot or reuse one capture for two gates.
    ///
    /// A refused preflight leaves the acknowledged cut unchanged. Storage failure
    /// makes the live owner unavailable, including across an unwind after rename.
    /// Reopen the original journal to discover the old-or-complete result; never
    /// infer nonexecution or resend from a failed acknowledgment.
    pub fn complete_credentialed_publication(
        &mut self,
        revision: u64,
        completion: CheckedCompletion<'_>,
        credential: &FileCredentialPermit,
    ) -> Result<CheckedPublication, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        if revision != self.revision() {
            return Err(Error::Stale.into());
        }
        self.check_credential_permit(credential)?;
        let CheckedCompletion { automatic, human, action, current, snapshot, now } = completion;
        if !Rc::ptr_eq(&self.issuer, &automatic.issuer)
            || !Rc::ptr_eq(&self.issuer, &human.issuer)
            || automatic.attempt != human.attempt
        {
            return Err(Error::Binding.into());
        }
        let attempt = automatic.attempt;
        self.check_action(attempt, action)?;
        self.check_action(attempt, current.action())?;
        let input_revision = self.current_reference(attempt, current)?;
        check_snapshot(&snapshot)?;
        let events = [
            Event::Core(BaseEvent::Time(now)),
            Event::Dispatch(attempt, human.request, input_revision, snapshot.clone()),
            Event::PublishCredentialed(attempt, Some(current.views().clone()), snapshot, now),
            Event::Core(BaseEvent::Reconcile(attempt)),
        ];
        for event in &events {
            self.check_source_admission(event)?;
        }
        let count = self.events.len().checked_add(events.len()).ok_or(Error::Overflow)?;
        if count > self.profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        let mut history = Vec::new();
        history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        history.extend(self.events.iter().cloned());
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let mut bytes = Vec::new();
        let mut publication = None;
        for (index, event) in events.iter().enumerate() {
            // Admission includes the original recovery reserve at EVERY prefix,
            // not merely a check that the final serialized bytes fit.
            bytes = journal::encode_appended(
                &self.profile, self.store.identity(), &history, event,
            )?;
            candidate.preflight_consistency(event)?;
            let transition = candidate.apply(event)?;
            match (index, transition) {
                (0 | 1, Transition::Unit) => {}
                (2, Transition::PublicationChecked(result)) => publication = Some(result),
                (3, Transition::Reconciled(Reconciliation::Resolved(outcome)))
                    if publication.as_ref().is_some_and(|result| result.outcome == outcome) => {}
                _ => return Err(Error::Binding.into()),
            }
            history.push(event.clone());
        }
        let publication = publication.ok_or(Error::Incomplete)?;
        // Recheck the real capability at the effect cut, never a journaled
        // assertion of credential ownership. There is no external callback here.
        self.check_credential_permit(credential)?;
        self.fault = Some(storage_uncertain());
        if let Err(error) = self.store.replace(&bytes) {
            self.fault = Some(match &error {
                JournalError::Io(failure) => failure.clone(),
                _ => storage_uncertain(),
            });
            return Err(error);
        }
        for event in &events {
            self.source_operation_committed(event);
        }
        self.events = history;
        self.machine = candidate;
        self.fault = None;
        Ok(publication)
    }
}

fn check_snapshot(snapshot: &Snapshot) -> Result<(), Error> {
    if snapshot.values.len() > super::super::super::MAX_SNAPSHOT_ENTRIES {
        return Err(Error::Limit);
    }
    let bytes = snapshot.values.values().try_fold(0_usize, |total, value| {
        total.checked_add(value.len())
    }).ok_or(Error::Limit)?;
    if bytes > super::super::super::MAX_SNAPSHOT_BYTES {
        return Err(Error::Limit);
    }
    Ok(())
}

fn storage_uncertain() -> JournalFailure {
    JournalFailure {
        operation: JournalIo::Stage,
        kind: std::io::ErrorKind::Other,
        replacement_may_be_visible: true,
    }
}

#[cfg(test)]
mod tests;
