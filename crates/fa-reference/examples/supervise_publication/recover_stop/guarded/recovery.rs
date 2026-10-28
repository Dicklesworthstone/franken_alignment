//! Closed selection between the original base and complete mediated cursors.
//! No public checker callback, fallback, intermediate owner or repeated replay.
use super::{Config, FileOversight, FileRecoveryRequirements, JournalError, RecoveryReserve};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{StopRequest, persistent::observed::{
    guarded::{anchored::FileHistoryAnchor,
        mediated::{FileMediatedRecovery, FileMediatedRequirements, FileTopologyRequirement},
        recovery::{FileAbandonedHistory, FileGuardedRecovery, FileGuardedRecoveryProgress}},
    shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory}}};
use fa_reference::action::consequence::delivery::persistent::abandonment::FileAbandonRequest;

// These variants are private to the executable. Neither can be converted into
// the other; all mode-specific validation remains in its original library type.
pub(super) enum Recovery {
    Base(FileGuardedRecovery),
    Mediated(FileMediatedRecovery),
}

impl Recovery {
    pub(super) fn begin(config: &Config, expected: FileRecoveryRequirements,
        topology: Option<FileTopologyRequirement>, anchor: &FileHistoryAnchor)
        -> Result<Self, JournalError>
    {
        match topology {
            None => FileOversight::begin_open_guarded_anchored(
                &config.store, config.profile.clone(), &expected, anchor)
                .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                .map(Self::Base),
            Some(topology) => {
                let expected = FileMediatedRequirements { oversight: expected, topology,
                    prediction: None, evaluation: None };
                FileOversight::begin_open_mediated_guarded_anchored(
                    &config.store, config.profile.clone(), &expected, anchor)
                    .and_then(|run| run.require_recovery_reserve(RecoveryReserve::terminal()))
                    .map(Self::Mediated)
            }
        }
    }

    pub(super) fn progress(&self) -> FileGuardedRecoveryProgress {
        match self { Self::Base(run) => run.progress(), Self::Mediated(run) => run.progress() }
    }

    pub(super) fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileGuardedRecoveryProgress, JournalError>
    {
        match self {
            Self::Base(run) => run.advance(expected_events, max_events),
            Self::Mediated(run) => run.advance(expected_events, max_events),
        }
    }

    pub(super) fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        match self { Self::Base(run) => run.stop_request(operation),
            Self::Mediated(run) => run.stop_request(operation) }
    }

    // This consuming path cannot return a live owner or silently acquire a
    // recovery Fence that changes the supplied abandonment predecessor.
    pub(super) fn finish_abandoned_with_anchor(self, request: FileAbandonRequest)
        -> Result<FileAbandonedHistory, JournalError>
    {
        match self {
            Self::Base(run) => run.finish_abandoned_with_anchor(request),
            Self::Mediated(run) => run.finish_abandoned_with_anchor(request),
        }
    }

    // Forward only to the selected sealed cursor; it rechecks its complete
    // independent requirements before invoking the original terminal consumer.
    pub(super) fn finish_admission_stop(self, request: StopRequest)
        -> Result<FileAdmissionStoppedHistory, JournalError>
    {
        match self { Self::Base(run) => run.finish_admission_stop(request),
            Self::Mediated(run) => run.finish_admission_stop(request) }
    }

    pub(super) fn finish_stopped_drain<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        match self { Self::Base(run) => run.finish_stopped_drain(request, clock),
            Self::Mediated(run) => run.finish_stopped_drain(request, clock) }
    }

    pub(super) fn finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        match self { Self::Base(run) => run.try_finish_stopped_with_anchor(request, clock),
            Self::Mediated(run) => run.try_finish_stopped_with_anchor(request, clock) }
    }
}
