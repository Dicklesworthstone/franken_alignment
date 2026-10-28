//! Original learned replay can produce a sealed historical policy diagnosis.
use super::super::super::super::guarded::investigation::FileProposalInvestigation;
use super::{FileLearnedRecovery, FileLearnedRecoveryStatus, JournalError, Selection};
use crate::Error;
use crate::action::consequence::experiment::InterventionScope;

impl FileLearnedRecovery {
    /// Select one external request before ANY original event is replayed.
    /// The first submission's original policy input is captured by the same
    /// reducer that verifies numerical history. An attempt with an equal number
    /// or a later request/retry cannot replace that admission. A retained refusal
    /// without a frozen action supplies no fictional proposal.
    ///
    /// The scope authorizes hypothetical edits only. Registration supplies no
    /// writable owner, role, clock or partial replay result. A duplicate or late
    /// registration consumes and drops this candidate without touching storage.
    pub fn require_request_proposal_investigation(
        mut self,
        request: u64,
        scope: InterventionScope,
    ) -> Result<Self, JournalError> {
        if self.replayed != 0 || self.status != FileLearnedRecoveryStatus::Replaying {
            return Err(Error::WrongState.into());
        }
        if self.investigation.is_some() {
            return Err(Error::Duplicate.into());
        }
        self.investigation = Some(Selection::request(request, scope)?);
        Ok(self)
    }

    /// Return only the immutable proposal captured during this original replay.
    /// Every suffix event and the selected learned recipe must have verified;
    /// guarded adapters also enforce their original independent requirements.
    /// No further numerical replay, source refresh, helper call, clock, fence,
    /// sync, cleanup or journal append runs. Full journals remain inspectable.
    ///
    /// The canonical image is re-read under the SAME exclusive recovery lock
    /// before the captured policy input escapes. Changed bytes, incomplete or
    /// failed replay and missing/non-admitted requests return no investigation.
    /// Staging evidence is preserved, and the private machine and lock are
    /// dropped on either outcome. Access authorization remains the caller's duty.
    pub fn finish_proposal_investigation(self) -> Result<FileProposalInvestigation, JournalError> {
        self.verified_guarded_machine()?;
        let Self {
            profile,
            store,
            events,
            machine,
            canonical,
            investigation,
            ..
        } = self;
        let selection = investigation.ok_or(Error::Incomplete)?;
        if store.read(profile.delivery.limits.bytes)? != canonical {
            return Err(Error::Binding.into());
        }
        selection.finish(store.identity(), &machine, events.len())
    }
}
