//! Evaluated source-bound native text recovery over the original replay cursor.
//! Independent history, guard and accounting contracts stay with one locked cut.
use super::super::FileEvaluatedOversightRoles;
use super::super::super::{
    FileOversight, FileOversightProfile, FileOversightRoles, JournalError,
    anchored::FileHistoryAnchor,
};
use super::FileAndonRecoveryRequirements;
use crate::Error;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::{
    decoder::FileDecoderConfig,
    shutdown::recovery::FileAdmissionStoppedHistory,
    stream::generated::checked::source::{
        FileGeneratedSourceRecovery, FileGeneratedSourceRecoveryProgress,
        FileGeneratedSourceRecoveryStatus, GeneratedSourceProfile, GeneratedSourcePublication,
    },
};
use crate::action::consequence::oversight::credibility::EvaluationProtocol;
use std::{fmt, path::Path};

/// One original generated-source replay, sealed with the complete independently
/// selected guard, evaluation and Andon requirements. This owns the same store,
/// canonical bytes and numerical reconstruction as the existing source cursor;
/// it neither imports accounting summaries nor repeats historical inference.
///
/// The independent exact history anchor is mandatory. A matching prefix never
/// skips full replay or supplies a fresh clock, source capture, helper judgment
/// or human key. No mutable owner or role escapes before the final acknowledgment.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::generated::FileAndonGeneratedSourceRecovery;
/// fn bypass(run: FileAndonGeneratedSourceRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::generated::FileAndonGeneratedSourceRecovery;
/// fn early_evaluator(run: &FileAndonGeneratedSourceRecovery) { run.evaluator(); }
/// ```
#[must_use = "finish verified recovery or drop it without issuing live roles"]
pub struct FileAndonGeneratedSourceRecovery {
    inner: FileGeneratedSourceRecovery,
    expected: FileAndonRecoveryRequirements,
    protocol: EvaluationProtocol,
    failure: Option<Error>,
}

impl fmt::Debug for FileAndonGeneratedSourceRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileAndonGeneratedSourceRecovery")
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Pin the original source, decoder, tokenizer, stream, publication and
    /// reserve selection together with ALL guards, current policy, credential
    /// epoch, evaluation protocol, joint policy, Andon windows and counter
    /// floors. None means required absence, never acceptance of an on-disk flag.
    ///
    /// The original source opener checks the independent history anchor before
    /// executing any numerical event. Evaluation/Andon preflight follows over
    /// that same locked image before returning a cursor. Only a fully verified
    /// finalizer may clean staging, append the recovery fence or release roles.
    pub fn begin_open_andon_guarded_generated_text_stream_from_source_anchored(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements,
        decoder: &FileDecoderConfig,
        tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile,
        protocol: &EvaluationProtocol,
        anchor: &FileHistoryAnchor,
    ) -> Result<FileAndonGeneratedSourceRecovery, JournalError> {
        expected.base().guards.validate()?;
        let (stream, publication) = match selected.publication {
            GeneratedSourcePublication::Stream { stream, .. } => (stream, None),
            GeneratedSourcePublication::Checked(publication)
            | GeneratedSourcePublication::Joint { publication, .. } => {
                (publication.stream, Some(publication.limits))
            }
        };
        let guards = &expected.base().guards;
        if guards.decoder.as_ref() != Some(decoder)
            || guards.stream != Some(stream)
            || guards.source != Some(selected.source)
            || expected.publication() != publication
        {
            return Err(Error::Binding.into());
        }
        let inner = Self::begin_open_generated_text_stream_from_source_anchored(
            directory, profile, decoder, tokenizer, selected, anchor,
        )?;
        let (_, events, _) = inner.guarded_history();
        expected.preflight_evaluated(events, protocol)?;
        Ok(FileAndonGeneratedSourceRecovery {
            inner,
            expected: expected.clone(),
            protocol: protocol.clone(),
            failure: None,
        })
    }

    /// Synchronous convenience over the same bounded cursor and its sole
    /// finalizer. Use begin/advance when the host must arbitrate Stop between
    /// original events; neither form provides an elapsed-time replay bound.
    pub fn open_andon_guarded_generated_text_stream_from_source_anchored(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        expected: &FileAndonRecoveryRequirements,
        decoder: &FileDecoderConfig,
        tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile,
        protocol: &EvaluationProtocol,
        anchor: &FileHistoryAnchor,
    ) -> Result<(Self, FileEvaluatedOversightRoles), JournalError> {
        let mut run = Self::begin_open_andon_guarded_generated_text_stream_from_source_anchored(
            directory, profile, expected, decoder, tokenizer, selected, protocol, anchor,
        )?;
        while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, MAX_JOURNAL_EVENTS)?;
        }
        run.finish()
    }
}

impl FileAndonGeneratedSourceRecovery {
    pub fn progress(&self) -> FileGeneratedSourceRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure {
            progress.status = FileGeneratedSourceRecoveryStatus::Failed(error);
        }
        progress
    }

    /// At most max_events original transitions with exact predecessor checks.
    /// A complete native reconstruction becomes Ready only after the independently
    /// selected evaluation, accounting, policy and guard requirements pass.
    /// A mismatched floor or inventory latches failure rather than exposing the
    /// inner cursor's otherwise valid prefix or Ready state.
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
            if let Err(error) = self.expected.check_replayed(
                profile,
                self.inner.verified_guarded_machine()?,
                events,
                &self.protocol,
            ) {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    /// Consume private replay state only. Canonical and pending bytes remain
    /// unchanged even when cancellation occurs immediately before finalization.
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
        self.expected
            .check_replayed(profile, machine, events, &self.protocol)?;
        Ok(())
    }

    /// Select a recorded native Message only after the complete independent
    /// anchor, guard, policy, accounting and floor checks. This immutable check
    /// issues no live role and writes no Fence; later work does not invalidate
    /// the original historical Message and its exact raw generation request.
    pub fn recorded_message(
        &self,
        request: u64,
        generation: u64,
        expected: &TextGenerationRequest,
    ) -> Result<(), JournalError> {
        self.check_ready()?;
        self.inner.recorded_message(request, generation, expected)
    }

    /// Select a recorded native Finish with its explicitly named native Message
    /// predecessor, original Executed receipt and complete cumulative prefix.
    /// All independently selected recovery contracts pass before this lookup.
    /// No proposal, reconciliation, numerical work or effect right is produced.
    pub fn recorded_finish(
        &self,
        after_request: u64,
        request: u64,
        generation: u64,
        expected: &TextGenerationRequest,
    ) -> Result<(), JournalError> {
        self.check_ready()?;
        self.inner.recorded_finish(after_request, request, generation, expected)
    }

    /// Publish exactly the original recovery Fence over the already verified
    /// machine, then return fresh separate role custody. Pending/censored labels,
    /// Andon incidents, numerical spend and unresolved effect liabilities survive.
    /// Old evaluator/ticket brands, approvals, source eligibility and clock do not.
    /// A failed or uncertain replacement returns no owner or role bundle.
    pub fn finish(self) -> Result<(FileOversight, FileEvaluatedOversightRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FileEvaluatedOversightRoles::provision(&host, oversight);
        Ok((host, roles))
    }

    /// Select the native terminal predecessor only after the complete contract
    /// passes. This issues neither a role nor a recovery Fence and reads no clock.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.check_ready()?;
        self.inner.stop_request(operation)
    }

    /// Preserve the separate no-clock Stop/Fence path over the same verified
    /// reconstruction. No normal recovery Fence or live evaluator comes first.
    pub fn finish_admission_stop(
        self,
        request: StopRequest,
    ) -> Result<FileAdmissionStoppedHistory, JournalError> {
        self.check_ready()?;
        self.inner.finish_admission_stop(request)
    }

    /// Carry the selected contracts into the original authenticated Stop
    /// exchange. The original terminal reserve is mandatory and never refilled.
    #[cfg(target_os = "linux")]
    pub fn into_recovery_stop(
        self,
        operation: u64,
    ) -> Result<
        crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop,
        JournalError,
    > {
        self.check_ready()?;
        self.inner.into_recovery_stop(operation)
    }
}

#[cfg(test)]
mod tests;
