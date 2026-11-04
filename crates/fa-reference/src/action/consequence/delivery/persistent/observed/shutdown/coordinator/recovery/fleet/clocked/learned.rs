//! One closed provisioning roster for the existing full-fleet pass engine.
use super::super::super::learned::{
    FileShutdownLearnedMember, FileShutdownLearnedPass, FileShutdownLearnedVisitResolution,
};
use super::super::super::resolution::FileShutdownVisitResolution;
use super::*;
use crate::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStopRecoveryProgress;

impl FileShutdownCoordinator {
    /// Drive the ORIGINAL full-roster selector and terminal/passive visit
    /// protocol with explicit per-member numerical provisioning. Every fixed
    /// member must occur once, including ordinary members with inputs=None.
    /// Input roster and source-checkpoint registration checks precede callbacks
    /// and writes. A failed learned member never falls back to ordinary replay.
    ///
    /// Learned members use their original fully guarded numerical cursor once.
    /// Both new visits and passive resolutions preserve the actual last native
    /// drain, complete suffix, current credentials, floors and newest whole head.
    /// Mediated members retain their original source checkpoint across only
    /// selected terminal successors. Passive learned reads hold the original
    /// exclusive domain lock and perform no domain cleanup or write.
    ///
    /// All original visit budgets, full denominators, explicit selection,
    /// cancellation, Unacknowledged/NotVisited outcomes and clock boundaries
    /// remain unchanged. Admission-only modes never invoke the clock provider.
    /// An outer Ok is still a report to inspect, never a fleet-stop guarantee.
    pub fn run_registered_learned_pass<C, P>(
        &mut self,
        revision: u64,
        bindings: &[FileShutdownClockBinding],
        members: &[FileShutdownLearnedMember],
        selection: FileShutdownLearnedPass<'_>,
        max_events: usize,
        clock: C,
        checkpoint: P,
    ) -> Result<FileShutdownClockedRecoveryPass, JournalError>
    where
        C: FnMut(FileShutdownClockBinding) -> Result<ElapsedTick, JournalError>,
        P: FnMut(FileShutdownRecoveryCheckpoint) -> bool,
    {
        let (selection, phase) = match selection {
            FileShutdownLearnedPass::Recover => (VisitSelection::All, RegisteredPhase::Combined),
            FileShutdownLearnedPass::Resume => (VisitSelection::Resume, RegisteredPhase::Combined),
            FileShutdownLearnedPass::RecoverSelected(ids) => {
                (VisitSelection::Explicit(ids), RegisteredPhase::Combined)
            }
            FileShutdownLearnedPass::StopAdmission => {
                (VisitSelection::All, RegisteredPhase::Admission)
            }
            FileShutdownLearnedPass::ResumeAdmission => {
                (VisitSelection::Resume, RegisteredPhase::Admission)
            }
            FileShutdownLearnedPass::StopSelected(ids) => {
                (VisitSelection::Explicit(ids), RegisteredPhase::Admission)
            }
            FileShutdownLearnedPass::DrainStopped => (VisitSelection::All, RegisteredPhase::Drain),
            FileShutdownLearnedPass::ResumeStopped => {
                (VisitSelection::Resume, RegisteredPhase::Drain)
            }
            FileShutdownLearnedPass::DrainSelected(ids) => {
                (VisitSelection::Explicit(ids), RegisteredPhase::Drain)
            }
        };
        let clock = if matches!(phase, RegisteredPhase::Admission) {
            None
        } else {
            Some(clock)
        };
        self.run_selected_pass_with_inputs(
            revision,
            bindings,
            max_events,
            (selection, phase),
            Some(members),
            clock,
            checkpoint,
        )
    }
}

pub(super) fn check_members(
    plan: &FileShutdownPlan,
    members: &[FileShutdownLearnedMember],
) -> Result<(), JournalError> {
    if members.len() != plan.domains.len() {
        return Err(Error::Binding.into());
    }
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(members.len())
        .map_err(|_| Error::Limit)?;
    ordered.extend(members.iter());
    ordered.sort_by_key(|member| member.domain);
    if ordered
        .windows(2)
        .any(|pair| pair[0].domain == pair[1].domain)
    {
        return Err(Error::Duplicate.into());
    }
    for (member, domain) in ordered.into_iter().zip(plan.domains.iter()) {
        if member.domain != domain.id() {
            return Err(Error::Binding.into());
        }
        if let Some(inputs) = &member.inputs {
            inputs.check_registration(domain)?;
        }
    }
    Ok(())
}

// These are sealed original readers, never a public callback or supplied result.
pub(super) enum Resolution<'a> {
    Ordinary(FileShutdownVisitResolution<'a>),
    Learned(FileShutdownLearnedVisitResolution<'a>),
}
impl Resolution<'_> {
    pub(super) fn progress(&self) -> FileStopRecoveryProgress {
        match self {
            Self::Ordinary(read) => read.progress(),
            Self::Learned(read) => read.progress(),
        }
    }
    pub(super) fn advance(
        &mut self,
        expected: usize,
        maximum: usize,
    ) -> Result<FileStopRecoveryProgress, JournalError> {
        match self {
            Self::Ordinary(read) => read.advance(expected, maximum),
            Self::Learned(read) => read.advance(expected, maximum),
        }
    }
    pub(super) fn finish(self) -> Result<FileShutdownObservation, JournalError> {
        match self {
            Self::Ordinary(read) => read.finish(),
            Self::Learned(read) => read.finish(),
        }
    }
    pub(super) fn cancel(self) -> Result<(), JournalError> {
        match self {
            Self::Ordinary(read) => read.cancel(),
            Self::Learned(read) => read.cancel(),
        }
    }
}
