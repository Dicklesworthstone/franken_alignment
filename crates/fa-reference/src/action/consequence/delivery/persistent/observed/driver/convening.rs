//! Consume a separately requested held successor through the ORIGINAL source,
//! round, helper and later two-key publication path. No automatic retry exists.
use super::evidence::{FileReviewLaunch, FileSourceReviewError};
use super::topology::FileDriverTopology;
use super::{FileOversight, FileSupervisedDriver, JournalError};
use crate::Error;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::requests::actor::FileConveneTicket;
use crate::action::consequence::oversight::evidence_source::{EvidenceFile, EvidenceIdentity};
use crate::action::consequence::oversight::helper_processes::HelperProgram;
use std::os::unix::net::UnixStream;

impl FileSupervisedDriver {
    /// Take at most one exact pending request. Active jobs and unreaped direct
    /// children retain their original custody and leave the request queued.
    /// This reads no source/clock and performs no native authority transition.
    pub fn take_pending_convene(
        &mut self,
        request: u64,
    ) -> Result<Option<FileConveneTicket>, JournalError> {
        if request == 0 {
            return Err(Error::InvalidInput.into());
        }
        if self.job.is_some() || !self.helpers_reaped() {
            return Ok(None);
        }
        if self.learned_review.is_some() {
            return Err(Error::Incomplete.into());
        }
        // A mediated host needs its original fixed source custodian before a
        // ticket can leave the queue. A retained certificate alone is not a
        // source, and no alternate source can be selected by the operator.
        if self.supervisor.host()?.convening_requires_topology() && self.topology.is_none() {
            return Err(Error::Incomplete.into());
        }
        self.supervisor.take_convene(request)
    }

    /// Explicit socket-backed successor. The selected ticket, original owner,
    /// current successful Hold and full predecessor are checked before source
    /// capture. Identical fresh complete evidence is allowed; no old vote, key,
    /// completed transcript or automatic reservation becomes the new review.
    /// A mediated owner first requalifies its ORIGINAL driver-owned topology
    /// source; its final publication check remains separately mandatory.
    pub fn start_file_convened_review<S, F>(
        &mut self,
        source: &mut S,
        mut ticket: FileConveneTicket,
        launch: FileReviewLaunch<UnixStream>,
        clock: F,
    ) -> Result<EvidenceIdentity, FileSourceReviewError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let result = self.check_convened_launch(&ticket, &launch).and_then(|()| {
            self.start_file_review_checked(source, launch, clock, |host, topology| {
                prepare_convened_capture(host, topology, &ticket)
            })
        });
        self.finish_convened_launch(&mut ticket, result)
    }

    /// Explicit process-backed successor with the original launcher's complete
    /// roster admission, fresh post-spawn clock and retained child-reap duty.
    /// Any error terminally spends this handoff; it never starts another round
    /// or restores the operation allowance. A caller must supply the original
    /// fixed helper recipe and preserve its overall service deadline/budgets.
    pub fn start_file_convened_process_review<S, F>(
        &mut self,
        source: &mut S,
        mut ticket: FileConveneTicket,
        launch: FileReviewLaunch<HelperProgram>,
        clock: F,
    ) -> Result<EvidenceIdentity, FileSourceReviewError>
    where
        S: EvidenceFile + ?Sized,
        F: FnMut() -> ElapsedTick,
    {
        let result = self.check_convened_launch(&ticket, &launch).and_then(|()| {
            self.start_file_process_review_checked(source, launch, clock, |host, topology| {
                prepare_convened_capture(host, topology, &ticket)
            })
        });
        self.finish_convened_launch(&mut ticket, result)
    }

    fn check_convened_launch<W>(
        &self,
        ticket: &FileConveneTicket,
        launch: &FileReviewLaunch<W>,
    ) -> Result<(), FileSourceReviewError> {
        if !ticket.belongs_to(&self.supervisor)
            || ticket.request() != launch.request
            || ticket.round() != launch.round
        {
            return Err(Error::Binding.into());
        }
        if launch.expected_input_revision != ticket.expected().input_revision {
            return Err(Error::Stale.into());
        }
        if self.learned_review.is_some() {
            return Err(Error::Incomplete.into());
        }
        if launch.window.commit_by >= launch.window.reveal_by
            || launch.window.reveal_by > ticket.expected().deadline
            || ticket
                .expected()
                .recorded_at
                .is_none_or(|now| now >= launch.window.commit_by)
        {
            return Err(Error::InvalidInput.into());
        }
        Ok(())
    }

    fn finish_convened_launch(
        &self,
        ticket: &mut FileConveneTicket,
        result: Result<EvidenceIdentity, FileSourceReviewError>,
    ) -> Result<EvidenceIdentity, FileSourceReviewError> {
        match result {
            Ok(observation) => {
                let opened = self
                    .supervisor
                    .host()
                    .and_then(|host| host.convene_opened(ticket.request(), ticket.round()));
                match opened {
                    Ok(opened) => {
                        ticket.finish(Some(opened));
                        Ok(observation)
                    }
                    Err(error) => {
                        ticket.finish(None);
                        Err(error.into())
                    }
                }
            }
            Err(error) => {
                ticket.finish(None);
                Err(error)
            }
        }
    }
}

fn prepare_convened_capture(
    host: &mut FileOversight,
    topology: Option<&mut FileDriverTopology>,
    ticket: &FileConveneTicket,
) -> Result<(), JournalError> {
    // Compare the full expected held predecessor before ANY topology source,
    // policy source or clock work. The exclusively owned topology refresh may
    // append a native check, so do not compare the old ticket against that
    // refresh's own acknowledged journal revision afterward.
    ticket.check(host)?;
    match topology {
        Some(topology) => {
            topology.refresh_for_review(host, ticket.request(), ticket.expected().attempt)
        }
        None if host.convening_requires_topology() => Err(Error::Incomplete.into()),
        None => Ok(()),
    }
}
