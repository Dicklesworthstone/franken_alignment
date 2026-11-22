//! Fresh windows consume only separately preprovisioned unnumbered owners.
//! The original wrapper, offer, ticket, source and publication job remain sole.
use super::*;
use crate::action::consequence::delivery::persistent::observed::source::FileSourceError;
use crate::action::consequence::oversight::ReviewWindow;
use crate::action::consequence::oversight::evidence_source::EvidenceError;

impl FileNativeSupervisedDriver {
    /// Current eligibility under the original finite grant. This performs no
    /// source read or model work and does not reserve an ID or operation.
    pub fn has_reserved_successor(&self) -> bool {
        self.convene_custody.is_some()
            && self
                .supervisor()
                .has_reserved_native_convene_offer(self.request)
                .ok()
                == Some(true)
            && self.reserved_successor_ready()
    }

    pub(super) fn reserved_successor_ready(&self) -> bool {
        let Ok(host) = self.supervisor().host() else {
            return false;
        };
        let Some(job) = self.driver.job.as_ref() else {
            return false;
        };
        if job.request != self.request
            || job.phase != Phase::Closed
            || job.input_revision != self.input_revision
            || job.inputs.as_ref() != Some(self.review.input())
            || job.check_owner(&host).is_err()
            || !self.driver.helpers_reaped()
            || (host.convening_requires_topology() && self.driver.topology.is_none())
        {
            return false;
        }
        let Ok((basis, blocked)) = host.native_convene_status(self.request) else {
            return false;
        };
        blocked.is_none() && self.review.check_reserved_successor(&host, &basis).is_ok()
    }

    /// Begin one explicit successor with the next exact model roster paid for
    /// before the first review. A fresh window is allowed only in this distinct
    /// reserved mode; fixed refinement slots keep their original windows/API.
    ///
    /// The full original ticket is checked before topology, source or clock
    /// work. Once acquisition begins, failure/unwind retires all unused native
    /// owners without refunding their original reservation or the operation.
    /// No numerical quantum, human key or publication is performed here.
    pub fn convene_reserved_from_policy_file<S, F>(
        &mut self,
        source: &mut S,
        mut ticket: FileConveneTicket,
        window: ReviewWindow,
        mut clock: F,
    ) -> FileEvidenceReport<FileNativeDriverProgress>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let mut observations = Vec::with_capacity(1);
        let mut source_updates = Vec::with_capacity(1);
        let checked = (|| {
            if !ticket.belongs_to(&self.driver.supervisor)
                || !ticket.is_reserved_native_learned()
                || ticket.request() != self.request
                || !self.has_reserved_successor()
            {
                return Err(JournalError::from(Error::Binding));
            }
            let host = self.supervisor().host()?;
            ticket.check(&host)?;
            self.review
                .check_reserved_successor(&host, &ticket.expected())?;
            if window.commit_by >= window.reveal_by
                || window.reveal_by > ticket.expected().deadline
                || ticket
                    .expected()
                    .recorded_at
                    .is_none_or(|now| now >= window.commit_by)
            {
                return Err(Error::InvalidInput.into());
            }
            Ok(())
        })();
        if let Err(error) = checked {
            ticket.finish(None);
            return FileEvidenceReport {
                observations,
                source_updates,
                result: Err(error),
            };
        }
        let mut operation = NativeConveneOperation {
            owner: self,
            acknowledged: false,
        };
        let result = (|| {
            let owner = &mut *operation.owner;
            let expected = ticket.expected();
            let job = owner.driver.job.as_ref().ok_or(Error::Missing)?;
            let mut host = owner.driver.supervisor.host_mut()?;
            // All unused actual owners leave the sequence before ANY callback.
            let selected =
                owner
                    .review
                    .take_reserved_successor(&host, &expected, ticket.round())?;
            match owner.driver.topology.as_mut() {
                Some(topology) => {
                    topology.refresh_for_review(&mut host, ticket.request(), expected.attempt)?
                }
                None if host.convening_requires_topology() => return Err(Error::Incomplete.into()),
                None => {}
            }
            let before = clock();
            observe(&mut host, before)?;
            if before >= window.commit_by {
                return Err(Error::Stale.into());
            }
            // Acquisition may renew an expired lease, but never stale learned
            // provenance or an input replacement. This is the original source
            // transaction; its exact capture also supplies the new review root.
            if host.source_interrupted || host.machine.pending_learned_reset().is_some() {
                return Err(Error::Incomplete.into());
            }
            let original = host
                .machine
                .checked_learned_sidecar_source(expected.attempt)?;
            if original.input_revision() != expected.input_revision
                || original.round().input() != owner.review.input()
                || original.round().input().action() != &job.action
            {
                return Err(Error::Stale.into());
            }
            let revision = host.revision();
            let captured = host.refresh_file_source(revision, source, before);
            source_updates.push(
                captured
                    .as_ref()
                    .map(|capture| capture.identity())
                    .map_err(Clone::clone),
            );
            let captured = match captured {
                Ok(capture) => {
                    observations.push(Ok(capture.identity()));
                    capture
                }
                Err(FileSourceError::Refused(error)) => {
                    observations.push(Err(EvidenceError::Data(error)));
                    return Err(error.into());
                }
                Err(FileSourceError::Read { error, withdrawal }) => {
                    observations.push(Err(error));
                    return Err(withdrawal.unwrap_or_else(|| match error {
                        EvidenceError::Data(error) => error.into(),
                        EvidenceError::Io(_) => Error::Incomplete.into(),
                    }));
                }
                Err(FileSourceError::Journal(error)) => return Err(error),
            };
            let now = clock();
            observe(&mut host, now)?;
            if now >= window.commit_by
                || now >= expected.deadline
                || host.input_revision(expected.attempt)? != expected.input_revision
                || host.inspect().control.sequence != expected.control_sequence
                || host.inspect().control.ledger.epoch != expected.authority_epoch
            {
                return Err(Error::Stale.into());
            }
            let round = LearnedWorkerRound {
                round: ticket.round(),
                evidence_root: captured.reference_root(),
                window,
            };
            owner.review.begin_reserved_successor(
                &mut host,
                selected,
                round,
                captured.snapshot().clone(),
            )?;
            let opened = host.convene_opened(ticket.request(), ticket.round())?;
            let job = owner.driver.job.as_mut().ok_or(Error::Missing)?;
            job.control_sequence = None;
            job.permit = None;
            job.phase = Phase::Review;
            Ok(opened)
        })();
        let result = match result {
            Ok(opened) => {
                ticket.finish(Some(opened));
                match operation.owner.clear_native_convene_offer() {
                    Ok(()) => {
                        operation.acknowledged = true;
                        Ok(operation.owner.progress())
                    }
                    Err(error) => Err(error),
                }
            }
            Err(error) => {
                ticket.finish(None);
                Err(error)
            }
        };
        drop(operation);
        FileEvidenceReport {
            observations,
            source_updates,
            result,
        }
    }
}
