//! Cooperative native text recovery with the complete original guard contract.
//! One anchored reconstruction retains custody through fencing or terminal Stop.
use super::{
    FileOversight, FileOversightProfile, FileOversightRoles, FileRecoveryRequirements,
    JournalError, anchored::FileHistoryAnchor,
};
use crate::Error;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::observed::{
    decoder::FileDecoderConfig,
    shutdown::recovery::FileAdmissionStoppedHistory,
    stream::generated::checked::source::{
        FileGeneratedSourceRecovery, FileGeneratedSourceRecoveryProgress,
        FileGeneratedSourceRecoveryStatus, GeneratedSourceProfile, GeneratedSourcePublication,
    },
};
use std::{fmt, path::Path};

/// Holds the original source-bound numerical recovery, not a second replay or
/// writer. Exact guard requirements and an independent history anchor are
/// mandatory before any governor or human role can be recovered. The selected
/// source, tokenizer, decoder, stream, reserve and publication contract remain
/// enforced by the original cursor throughout its lifetime.
///
/// No candidate owner, mutable machine or partial role bundle is exposed.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::generated_recovery::FileGuardedGeneratedSourceRecovery;
/// fn bypass(run: FileGuardedGeneratedSourceRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::generated_recovery::FileGuardedGeneratedSourceRecovery;
/// fn early_role(run: &FileGuardedGeneratedSourceRecovery) { run.roles(); }
/// ```
#[must_use = "advance and finish guarded recovery, or drop it without changing storage"]
pub struct FileGuardedGeneratedSourceRecovery {
    inner: FileGeneratedSourceRecovery,
    expected: FileRecoveryRequirements,
    failure: Option<Error>,
}

impl fmt::Debug for FileGuardedGeneratedSourceRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGuardedGeneratedSourceRecovery")
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Admit one independently anchored native history with the exact current
    /// policy, campaign inventory, credential epoch and recovery floors. None
    /// requires a guard's absence, never acceptance of whatever is on disk.
    ///
    /// Decoder, source and stream expectations must agree with the separately
    /// selected native contract before opening. The original constructor checks
    /// the anchor and all native bindings before executing any numerical event.
    /// Full replay and every independent requirement must pass before cleanup,
    /// Fence, terminal Stop or role issuance. No live evidence or clock is read.
    pub fn begin_open_guarded_generated_text_stream_from_source_anchored(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileRecoveryRequirements,
        decoder: &FileDecoderConfig,
        tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile,
        anchor: &FileHistoryAnchor,
    ) -> Result<FileGuardedGeneratedSourceRecovery, JournalError> {
        expected.guards.validate()?;
        let stream = match selected.publication {
            GeneratedSourcePublication::Stream { stream, .. } => stream,
            GeneratedSourcePublication::Checked(publication)
            | GeneratedSourcePublication::Joint { publication, .. } => publication.stream,
        };
        if expected.guards.decoder.as_ref() != Some(decoder)
            || expected.guards.stream != Some(stream)
            || expected.guards.source != Some(selected.source)
        {
            return Err(Error::Binding.into());
        }
        let inner = Self::begin_open_generated_text_stream_from_source_anchored(
            directory, profile, decoder, tokenizer, selected, anchor,
        )?;
        let (_, events, _) = inner.guarded_history();
        expected.guards.check_decoder_config(events)?;
        Ok(FileGuardedGeneratedSourceRecovery {
            inner,
            expected: expected.clone(),
            failure: None,
        })
    }
}

impl FileGuardedGeneratedSourceRecovery {
    pub fn progress(&self) -> FileGeneratedSourceRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure {
            progress.status = FileGeneratedSourceRecoveryStatus::Failed(error);
        }
        progress
    }

    /// Execute at most the requested count of original transitions. Complete
    /// independent requirements qualify Ready, and a mismatch remains terminal
    /// for this cursor. Event quanta are not elapsed-time or CPU guarantees.
    pub fn advance(
        &mut self,
        expected_events: usize,
        max_events: usize,
    ) -> Result<FileGeneratedSourceRecoveryProgress, JournalError> {
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileGeneratedSourceRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) =
                self.expected
                    .check(profile, self.inner.verified_guarded_machine()?, events)
            {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    /// Drop only private replay state. Preserve canonical and pending bytes,
    /// including when complete reconstruction was cancelled before fencing.
    pub fn cancel(self) -> FileGeneratedSourceRecoveryProgress {
        let mut progress = self.progress();
        if matches!(
            progress.status,
            FileGeneratedSourceRecoveryStatus::Replaying | FileGeneratedSourceRecoveryStatus::Ready
        ) {
            progress.status = FileGeneratedSourceRecoveryStatus::Cancelled;
        }
        progress
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        let machine = self.inner.verified_guarded_machine()?;
        let (profile, events, _) = self.inner.guarded_history();
        self.expected.check(profile, machine, events)?;
        Ok(())
    }

    /// Persist the original recovery Fence, then provision the exact original
    /// role bundle. Old source/time eligibility and opaque approvals remain
    /// withdrawn; numerical work, stream history and charged liabilities remain.
    /// Neither recovery nor role provisioning replays numerical events twice.
    pub fn finish(self) -> Result<(FileOversight, FileOversightRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }

    /// Select a terminal Stop only after the complete independent contract
    /// passes. This reads no clock, issues no role and appends no Fence.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.check_ready()?;
        self.inner.stop_request(operation)
    }

    /// Consume the same guarded reconstruction into the original no-clock
    /// Stop/Fence transaction. No ordinary recovery Fence or live role precedes
    /// stopping; unknown charges remain liabilities and are never refunded.
    pub fn finish_admission_stop(
        self,
        request: StopRequest,
    ) -> Result<FileAdmissionStoppedHistory, JournalError> {
        self.check_ready()?;
        self.inner.finish_admission_stop(request)
    }

    /// Retain the same complete guard contract and locked numerical replay
    /// through the original authenticated Stop exchange. A retained terminal
    /// reserve is mandatory; no weaker cursor or live owner is released.
    #[cfg(target_os = "linux")]
    pub fn into_recovery_stop(
        self,
        operation: u64,
    ) -> Result<
        crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop,
        JournalError,
    >{
        self.check_ready()?;
        self.inner.into_recovery_stop(operation)
    }
}
