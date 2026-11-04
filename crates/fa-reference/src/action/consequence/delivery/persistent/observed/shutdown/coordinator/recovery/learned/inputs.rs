//! Immutable independent learned inputs, never a worker callback or native receipt.
use super::*;
use crate::action::consequence::delivery::persistent::{
    RecoveryReserve,
    observed::{
        Event, FileOversightProfile,
        decoder::learned::{FileLearnedConfig, FileLearnedRecoveryStatus},
        driver::topology::checkpoint::TopologyCheckpoint,
        guarded::{
            FileRecoveryRequirements,
            learned_recovery::FileGuardedLearnedRecovery,
            mediated::{
                FileMediatedRequirements, predictive::FilePredictiveMediatedRequirements,
                terminal::FileTerminalTopologyRecovery,
            },
            predictive::{
                FilePredictiveRequirements,
                learned::FilePredictiveLearnedRecovery,
                owned::{FileOwnedPredictiveRecovery, FileOwnedPredictiveRequirements},
            },
        },
    },
};
use std::path::Path;

/// Complete original requirement families. Selecting another variant cannot
/// remove a registered guard or supply a fallback for a refused learned mode.
#[derive(Clone, Debug)]
pub enum FileShutdownLearnedRequirements {
    Plain(FileRecoveryRequirements),
    Raw(FilePredictiveRequirements),
    Owned(FileOwnedPredictiveRequirements),
    Mediated(FileMediatedRequirements),
    PredictiveMediated(FilePredictiveMediatedRequirements),
}

/// Independent numerical recipe, full guards and original reserve. These are
/// operator input data, not evidence that a history has passed verification.
/// The coordinator alone chooses the registered path and bootstrap profile.
///
/// A mediated selection requires its original v2 source checkpoint, whose
/// history extends the complete fixed registration. Only the selected native
/// Stop/Fence/StopProgress suffix may follow that source cut. Source work and
/// current floors are never reset. Nonmediated selections require None.
#[derive(Clone, Debug)]
pub struct FileShutdownLearnedInputs {
    pub generation: FileLearnedConfig,
    pub requirements: FileShutdownLearnedRequirements,
    pub reserve: RecoveryReserve,
    pub topology_checkpoint: Option<TopologyCheckpoint>,
}

/// One explicit provisioning choice for EVERY fixed fleet member. None selects
/// the original ordinary recovery path; a failed learned choice never falls
/// through to it. Missing, duplicate or extra entries refuse the complete pass.
#[derive(Clone, Debug)]
pub struct FileShutdownLearnedMember {
    pub domain: u64,
    pub inputs: Option<FileShutdownLearnedInputs>,
}

/// Select original work for the complete roster. Selected variants still
/// freshly resolve every unselected member; they do not shrink the denominator.
#[derive(Clone, Copy, Debug)]
pub enum FileShutdownLearnedPass<'a> {
    Recover,
    Resume,
    RecoverSelected(&'a [u64]),
    StopAdmission,
    ResumeAdmission,
    StopSelected(&'a [u64]),
    DrainStopped,
    ResumeStopped,
    DrainSelected(&'a [u64]),
}

impl FileShutdownLearnedInputs {
    pub(in crate::action::consequence::delivery::persistent::observed::shutdown::coordinator) fn check_registration(
        &self,
        domain: &FileShutdownDomain,
    ) -> Result<(), JournalError> {
        let mediated = matches!(
            &self.requirements,
            FileShutdownLearnedRequirements::Mediated(_)
                | FileShutdownLearnedRequirements::PredictiveMediated(_)
        );
        if mediated != self.topology_checkpoint.is_some() {
            return Err(Error::Binding.into());
        }
        if let Some(saved) = &self.topology_checkpoint {
            let events = saved
                .history_anchor()
                .decoded_events(&domain.profile, &domain.directory)?;
            if events.len() < domain.revision {
                return Err(Error::Stale.into());
            }
            let registered = journal::encode(
                &domain.profile,
                &domain.directory,
                &events[..domain.revision],
            )?;
            if registered.as_slice() != domain.anchor.as_ref() {
                return Err(Error::Binding.into());
            }
        }
        Ok(())
    }

    pub(super) fn begin(
        &self,
        campaign: &FileShutdownCampaign,
        index: usize,
    ) -> Result<Cursor, JournalError> {
        let domain = campaign.plan.domains.get(index).ok_or(Error::Missing)?;
        self.check_registration(domain)?;
        let profile = (*domain.profile).clone();
        let directory = &domain.directory;
        let run =
            match &self.requirements {
                FileShutdownLearnedRequirements::Plain(expected) => Cursor::Plain(
                    FileOversight::begin_open_guarded_with_learned_generation(
                        directory,
                        profile,
                        expected,
                        &self.generation,
                    )?
                    .require_recovery_reserve(self.reserve)?,
                ),
                FileShutdownLearnedRequirements::Raw(expected) => Cursor::Raw(
                    FileOversight::begin_open_predictive_guarded_with_learned_generation(
                        directory,
                        profile,
                        expected,
                        &self.generation,
                    )?
                    .require_recovery_reserve(self.reserve)?,
                ),
                FileShutdownLearnedRequirements::Owned(expected) => Cursor::Owned(
                    FileOversight::begin_open_predictive_guarded_with_owned_learned_consistency(
                        directory,
                        profile,
                        expected,
                        &self.generation,
                    )?
                    .require_recovery_reserve(self.reserve)?,
                ),
                FileShutdownLearnedRequirements::Mediated(expected) => {
                    let saved = self.topology_checkpoint.as_ref().ok_or(Error::Missing)?;
                    Cursor::Mediated(
                        FileOversight::begin_open_mediated_guarded_with_learned_generation(
                            directory,
                            profile,
                            expected,
                            &self.generation,
                        )?
                        .require_recovery_reserve(self.reserve)?
                        .require_terminal_checkpoint(saved, campaign.plan.operation)?,
                    )
                }
                FileShutdownLearnedRequirements::PredictiveMediated(expected) => {
                    let saved = self.topology_checkpoint.as_ref().ok_or(Error::Missing)?;
                    Cursor::Mediated(
                    FileOversight::begin_open_predictive_mediated_guarded_with_learned_generation(
                        directory, profile, expected, &self.generation)?
                        .require_recovery_reserve(self.reserve)?
                        .require_terminal_checkpoint(saved, campaign.plan.operation)?)
                }
            };
        // Every constructor above only reads, decodes and binds expectations.
        // Admit the SAME locked image against the NEWEST registered head before
        // the first original reducer event, including the numerical bootstrap rule.
        let (_, events, identity, bytes) = run.history();
        if identity != domain.directory.as_path() {
            return Err(Error::Binding.into());
        }
        campaign.check_head_size(index, bytes)?;
        campaign.check_canonical_events(index, identity, events)?;
        Ok(run)
    }
}

// No public extraction, construction, unchecked ready state or live role.
pub(super) enum Cursor {
    Plain(FileGuardedLearnedRecovery),
    Raw(FilePredictiveLearnedRecovery),
    Owned(FileOwnedPredictiveRecovery),
    Mediated(FileTerminalTopologyRecovery),
}
impl Cursor {
    fn history(&self) -> (&FileOversightProfile, &[Event], &Path, usize) {
        match self {
            Self::Plain(run) => run.shutdown_history(),
            Self::Raw(run) => run.shutdown_history(),
            Self::Owned(run) => run.shutdown_history(),
            Self::Mediated(run) => run.shutdown_history(),
        }
    }

    pub(super) fn progress(&self) -> FileStopRecoveryProgress {
        let progress = match self {
            Self::Plain(run) => run.progress(),
            Self::Raw(run) => run.progress(),
            Self::Owned(run) => run.progress(),
            Self::Mediated(run) => run.progress(),
        };
        let status = match progress.status {
            FileLearnedRecoveryStatus::Replaying => FileStopRecoveryStatus::Replaying,
            FileLearnedRecoveryStatus::Ready => FileStopRecoveryStatus::Ready,
            FileLearnedRecoveryStatus::Interrupted => FileStopRecoveryStatus::Interrupted,
            FileLearnedRecoveryStatus::Failed(error) => FileStopRecoveryStatus::Failed(error),
        };
        // A failed/unwound reducer attempted the next event. Ending guard
        // failure follows all events and must not invent another event attempt.
        let attempted_events = progress.replayed_events
            + usize::from(
                progress.replayed_events < progress.total_events
                    && matches!(
                        status,
                        FileStopRecoveryStatus::Interrupted | FileStopRecoveryStatus::Failed(_)
                    ),
            );
        FileStopRecoveryProgress {
            replayed_events: progress.replayed_events,
            attempted_events,
            total_events: progress.total_events,
            status,
        }
    }

    pub(super) fn advance(
        &mut self,
        expected: usize,
        maximum: usize,
    ) -> Result<FileStopRecoveryProgress, JournalError> {
        match self {
            Self::Plain(run) => {
                run.advance(expected, maximum)?;
            }
            Self::Raw(run) => {
                run.advance(expected, maximum)?;
            }
            Self::Owned(run) => {
                run.advance(expected, maximum)?;
            }
            Self::Mediated(run) => {
                run.advance(expected, maximum)?;
            }
        }
        Ok(self.progress())
    }

    pub(super) fn into_terminal(self, operation: u64) -> Result<FileStopRecovery, JournalError> {
        match self {
            Self::Plain(run) => run.into_shutdown_terminal(),
            Self::Raw(run) => run.into_shutdown_terminal(),
            Self::Owned(run) => run.into_shutdown_terminal(),
            Self::Mediated(run) => run.into_shutdown_terminal(operation),
        }
    }
}
