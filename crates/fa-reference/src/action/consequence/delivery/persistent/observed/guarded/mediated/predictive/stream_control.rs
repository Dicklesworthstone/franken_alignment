//! One exact owned stream operation under complete predictive mediation custody.
use super::{FileLearnedRecoveryProgress, FileLearnedRecoveryStatus,
    FileMediatedPredictor, FilePredictiveMediatedLearnedRecovery, JournalError,
    check_recovery_reserve};
use crate::Error;
use crate::action::consequence::delivery::persistent::{RecoveryReserve, observed::{
    decoder::learned::{FileLearnedStreamCheckpointRequest, FileLearnedStreamControlHistory,
        FileLearnedStreamNarrowRequest, checkpoint::FileLearnedResetIntent},
    driver::topology::checkpoint::TopologyCheckpoint,
}};
use std::fmt;

/// Independently selected original operation. Binding this data before replay
/// does not mint a numerical handle, source certificate, forecast or effect key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileMediatedStreamControlRequest {
    Capture(FileLearnedStreamCheckpointRequest),
    Narrow(FileLearnedStreamNarrowRequest),
    BeginReset { intent: FileLearnedResetIntent, first_incident_sequence: u64 },
}
impl FileMediatedStreamControlRequest {
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Capture(_) => Ok(()),
            Self::Narrow(request) => request.validate(),
            Self::BeginReset { intent, first_incident_sequence } => {
                intent.validate()?;
                if *first_incident_sequence == 0 { return Err(Error::InvalidInput); }
                Ok(())
            }
        }
    }
}

/// Same-cut immutable native control and source-custody records. Graph
/// availability is not cut certification; narrowing withdraws the old cut.
/// No owner, source observer, predictor role or effect authority is recoverable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMediatedStreamControlHistory {
    control: FileLearnedStreamControlHistory,
    topology: TopologyCheckpoint,
}
impl FileMediatedStreamControlHistory {
    pub fn control(&self) -> &FileLearnedStreamControlHistory { &self.control }
    pub fn topology_checkpoint(&self) -> &TopologyCheckpoint { &self.topology }
    pub fn into_parts(self) -> (FileLearnedStreamControlHistory, TopologyCheckpoint) {
        (self.control, self.topology)
    }
}

/// The SAME original numerical replay, sealed to one exact stream operation
/// and independently retained source cut. Only an unchanged cut or the exact
/// selected operation can explain lost paired export. All independent guards,
/// policy, predictor, topology and floors remain mandatory through the write.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::predictive::FileMediatedStreamControlRecovery;
/// fn live(run: FileMediatedStreamControlRecovery) { run.finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::predictive::FileMediatedStreamControlRecovery;
/// fn change(run: FileMediatedStreamControlRecovery) { run.into_inner(); }
/// ```
#[must_use = "finish the selected stream operation or drop without changing storage"]
pub struct FileMediatedStreamControlRecovery {
    inner: FilePredictiveMediatedLearnedRecovery,
    saved: TopologyCheckpoint,
    request: FileMediatedStreamControlRequest,
}
impl fmt::Debug for FileMediatedStreamControlRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileMediatedStreamControlRecovery")
            .field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FilePredictiveMediatedLearnedRecovery {
    /// Admit the exact original owned prompt-audit recipe and retained source
    /// cut before event zero. This does not weaken raw/legacy/optional predictor
    /// refusal or import a new source quota. Terminal recovery remains reserved;
    /// the eventual original capture/narrow/intent consumes ordinary capacity.
    pub fn require_stream_control_checkpoint(mut self, saved: &TopologyCheckpoint,
        request: FileMediatedStreamControlRequest)
        -> Result<FileMediatedStreamControlRecovery, JournalError>
    {
        let progress = self.progress();
        if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
            return Err(Error::WrongState.into());
        }
        self.check_owned_stream_recipe()?;
        request.validate()?;
        check_recovery_reserve(&self.inner, RecoveryReserve::terminal())?;
        if &self.expected.topology.current != saved.expected().graph()
            || self.expected.topology.available != saved.available() {
            return Err(Error::Binding.into());
        }
        let (profile, events, identity) = self.inner.guarded_history();
        saved.stream_control_successor_available(profile, identity, events, &request)?;
        let floor = saved.minimum();
        let minimum = &mut self.expected.oversight.minimum;
        minimum.journal_revision = minimum.journal_revision.max(floor.journal_revision);
        minimum.control_sequence = minimum.control_sequence.max(floor.control_sequence);
        minimum.authority_epoch = minimum.authority_epoch.max(floor.authority_epoch);
        Ok(FileMediatedStreamControlRecovery { inner: self, saved: saved.clone(), request })
    }

    pub(super) fn check_owned_stream_recipe(&self) -> Result<(), JournalError> {
        if !matches!(&self.expected.prediction, FileMediatedPredictor::Owned(_)) {
            return Err(Error::Binding.into());
        }
        self.inner.check_owned_stream_control_recipe()
    }
}

impl FileMediatedStreamControlRecovery {
    pub fn progress(&self) -> FileLearnedRecoveryProgress { self.inner.progress() }
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        self.inner.advance(expected_events, max_events)
    }

    /// Apply only the selected original operation after complete guarded
    /// replay. Repeat every guard and paired-custody check on its private
    /// successor before the sole original canonical replacement. No Fence,
    /// source read, new numerical work or role is produced. Exact retry adds
    /// no event; lost archive output never authorizes an unrelated suffix.
    pub fn finish_stream_control(mut self) -> Result<FileMediatedStreamControlHistory, JournalError> {
        self.check_ready_custody()?;
        let (inner, receipt, appended) = match &self.request {
            FileMediatedStreamControlRequest::Capture(request) =>
                self.inner.inner.prepare_captured_stream_checkpoint(*request)?,
            FileMediatedStreamControlRequest::Narrow(request) =>
                self.inner.inner.prepare_narrowed_stream_checkpoint(request)?,
            FileMediatedStreamControlRequest::BeginReset { intent, first_incident_sequence } =>
                self.inner.inner.prepare_begun_stream_reset(intent, *first_incident_sequence)?,
        };
        self.inner.inner = inner;
        self.check_ready_custody()?;
        let (profile, _, identity) = self.inner.inner.guarded_history();
        let profile = profile.clone();
        let identity = identity.to_path_buf();
        let control = self.inner.inner.finish_stream_control(receipt, appended)?;
        let topology = self.saved.after_learned_stream_control(
            &profile, &identity, &self.request, &control)?;
        Ok(FileMediatedStreamControlHistory { control, topology })
    }

    fn check_ready_custody(&self) -> Result<(), JournalError> {
        self.inner.check_ready()?;
        self.inner.check_owned_stream_recipe()?;
        let (profile, events, identity) = self.inner.inner.guarded_history();
        let snapshot = self.inner.inner.verified_guarded_machine()?.snapshot(events.len());
        self.saved.check_stream_control_start(profile, identity, events, &snapshot, &self.request)
    }
}
