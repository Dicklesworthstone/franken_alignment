//! Opt-in complete driving of original computed congress and two-key effects.
//! The legacy socket-only step contract remains unchanged. This adapter samples
//! evidence, invokes the original learned advance, then rejoins that same driver.
use super::super::{FileDriverEvent, FileHumanPermit, FileSupervisedDriver, FrozenAction,
    CommitteeContract, DriverEvidence, Phase, observe, sample};
use super::super::provider::{self, EvidenceProvider};
use super::super::learned::FileDriverLearnedEvent;
use super::super::super::credential::FileCredentialPermit;
use super::super::super::helpers::learned::FileLearnedProbeStatus;
use super::super::super::super::JournalError;
use crate::action::consequence::oversight::helper_workers::io::HelperPump;
use crate::action::ElapsedTick;
use crate::Error;
use std::collections::BTreeMap;

impl FileSupervisedDriver {
    /// Drive a source-bound computed review AND its original effect lifecycle
    /// through one host loop. Start with start_learned_probe_review; callers do
    /// not also advance that evaluator in a second loop. Every review tick reads
    /// fresh provider evidence, but only the privately owned original evaluator
    /// supplies votes/refinement/finish. Completed Continue still awaits a human.
    ///
    /// The original step_with_evidence intentionally remains socket-only while
    /// a computed review is active. Opting into this method does not relax that
    /// existing contract or expose an alternate finish operation.
    pub fn step_with_computed_evidence<F, P>(&mut self, clock: F, provider: P,
        human: Option<&FileHumanPermit>) -> Result<FileDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        self.step_computed_with_provider(clock, &mut provider::Callback(provider), human, None)
    }

    /// Same complete loop, with the existing separate credential capability
    /// consulted by the ORIGINAL final publication path, never by a helper.
    pub fn step_with_computed_evidence_and_credential<F, P>(&mut self, clock: F,
        provider: P, human: Option<&FileHumanPermit>, credential: &FileCredentialPermit)
        -> Result<FileDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick,
        P: FnMut(&FrozenAction, &CommitteeContract) -> Result<DriverEvidence, Error>,
    {
        self.step_computed_with_provider(clock, &mut provider::Callback(provider), human, Some(credential))
    }

    pub(super) fn step_computed_with_provider<F, P>(&mut self, mut clock: F,
        provider: &mut P, human: Option<&FileHumanPermit>, credential: Option<&FileCredentialPermit>)
        -> Result<FileDriverEvent, JournalError>
    where F: FnMut() -> ElapsedTick, P: EvidenceProvider {
        self.reap_helpers();
        let running = self.job.as_ref().is_some_and(|job| job.phase == Phase::Review
            && self.learned_probe_review().is_some_and(|saved| saved.request() == job.request
                && saved.review().status() == FileLearnedProbeStatus::Running));
        if !running {
            // Includes all socket/process work, human approval, dispatch, final
            // source/credential checks and query-only unknown reconciliation.
            return self.step_with_provider(clock, provider, human, credential);
        }
        let result = (|| {
            let (revision, now, snapshot) = {
                let job = self.job.as_ref().ok_or(Error::Missing)?;
                let saved = self.learned_probe_review().ok_or(Error::Missing)?;
                let revision = saved.review().revision();
                let mut host = self.supervisor.host_mut()?;
                job.check_owner(&host)?;
                if host.storage_failure().is_some() { return Err(JournalError::Unavailable); }
                observe(&mut host, clock())?;
                let captured = sample(&mut host, job, provider, &mut clock)?;
                if let Some(error) = captured.failure { return Err(error.into()); }
                if captured.evidence.inputs.is_none() { return Err(Error::Incomplete.into()); }
                if captured.evidence.inputs.as_ref() != job.inputs.as_ref() { return Err(Error::Stale.into()); }
                let now = clock();
                observe(&mut host, now)?;
                (revision, now, captured.evidence.snapshot)
            };
            // No host borrow or substituted session crosses into the original
            // owner. Its revision, source, deadline, custody, storage and finish
            // checks remain the authority, including after every refinement.
            match self.advance_learned_probe_review(revision, now, snapshot)? {
                FileDriverLearnedEvent::Completed(event) => Ok(event),
                FileDriverLearnedEvent::Progress { request, .. } => {
                    let saved = self.learned_probe_review().ok_or(Error::Incomplete)?;
                    Ok(FileDriverEvent::Workers { request, report: HelperPump {
                        io: BTreeMap::new(), workers: saved.review().worker_statuses(),
                    } })
                }
            }
        })();
        if result.is_err() {
            // A failed observation cannot leave this automatic loop silently
            // scoring from a previous input. Original maintenance cancels the
            // evaluator while retaining its numerical work and leased rounds.
            if let Some(job) = &mut self.job { job.close(); }
            self.reap_helpers();
            if self.job.as_ref().is_some_and(|job| job.phase == Phase::Closed) { self.job = None; }
        }
        result
    }
}

#[cfg(test)]
mod tests;
