//! Finish and optionally refine the same original congress in one journal cut.
use super::{FileLearnedSidecar, FileOversight, JournalError, Machine, Event,
    DecoderEvent, LearnedEvent, SidecarEvent, Transition, journal};
use crate::action::consequence::activation::probe::learned::KvGroup;
use crate::action::consequence::oversight::{ObservedReceipt,
    replay::ObservedDecisionArchive, sidecar::SidecarRefinementOutcome};
use crate::{Error, Snapshot};

/// Result of an acknowledged original completed round. A refined packet needs
/// another independent review; a receipt still needs every original effect key.
/// The archive records accepted observations even if original application fails.
#[derive(Clone, Debug)]
pub enum FileLearnedSidecarFinish {
    Refined { group: KvGroup, input_revision: u64, archive: ObservedDecisionArchive },
    Applied { outcome: Option<SidecarRefinementOutcome>,
        receipt: Result<ObservedReceipt, Error>, archive: ObservedDecisionArchive },
}
impl FileLearnedSidecarFinish {
    pub fn archive(&self) -> &ObservedDecisionArchive {
        match self { Self::Refined { archive, .. } | Self::Applied { archive, .. } => archive }
    }
}

impl FileOversight {
    /// Consume the original completed review, then let its original planner buy
    /// one residual when requested. Only a non-refined review is applied here.
    /// Missing workers never buy evidence. This entry point cannot take over a
    /// round leased to the original helper transport or computed-probe adapter.
    pub fn finish_learned_sidecar_review(&mut self, revision: u64,
        sidecar: &mut FileLearnedSidecar, round: u64, snapshot: Snapshot)
        -> Result<FileLearnedSidecarFinish, JournalError>
    {
        self.check_manual_round(round)?;
        self.finish_learned_sidecar_review_inner(revision, sidecar, round, true, snapshot)
    }

    // Private worker adapters bypass only the manual-round prohibition. The
    // complete source, owner, revision, original session and storage checks remain.
    pub(in super::super::super::super) fn finish_learned_sidecar_review_inner(&mut self,
        revision: u64, sidecar: &mut FileLearnedSidecar, round: u64,
        allow_refinement: bool, snapshot: Snapshot)
        -> Result<FileLearnedSidecarFinish, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.checked_learned_sidecar(sidecar)?;
        // Use the original bounded snapshot codec before retaining any copy.
        let mut admission = super::Writer::new(self.profile.delivery.limits.bytes);
        admission.snapshot(&snapshot)?;
        self.check_source_admission(&Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(
            SidecarEvent::Finish { attempt: sidecar.attempt, actor_revision: sidecar.actor_revision,
                input_revision: sidecar.input_revision, round, allow_refinement,
                snapshot: snapshot.clone(), expected: std::rc::Rc::from(&b""[..]) }))))?;
        if self.events.len() >= self.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        let (event, result) = candidate.prepare_learned_sidecar_finish(sidecar.attempt,
            sidecar.actor_revision, sidecar.input_revision, round, allow_refinement, snapshot)?;
        let event = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(event)));
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &self.events, &event)?;
        let result = self.persist_candidate(event, bytes, candidate,
            Transition::LearnedSidecarFinished(Box::new(result)))?;
        let Transition::LearnedSidecarFinished(result) = result else { unreachable!("sidecar completion transition"); };
        if let FileLearnedSidecarFinish::Refined { input_revision, .. } = result.as_ref() {
            sidecar.input_revision = *input_revision;
        }
        Ok(*result)
    }
}
