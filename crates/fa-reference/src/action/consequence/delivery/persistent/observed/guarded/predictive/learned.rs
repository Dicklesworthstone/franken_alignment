//! Recover a pinned predictive learned owner through the original replay/fence.
//! The full guard inventory and recipe are independent inputs, never disk defaults.
mod inspection;
use crate::action::consequence::delivery::credential_broker::{
    CredentialChangeReceipt, CredentialRotationRequest, CredentialRevocationRequest,
};
use crate::action::consequence::delivery::persistent::observed::credential::FileCredentialStatus;
use super::{FilePredictiveRequirements, FilePredictiveRoles};
use super::super::{FileOversightRoles, anchored::FileHistoryAnchor, learned::check_profile,
    learned_recovery::check_recovery_reserve};
use super::super::super::{Event, FileOversight, FileOversightProfile, JournalError};
use super::super::super::shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory, FileStoppedRecovery};
use crate::action::consequence::delivery::StopRequest;
use crate::action::ElapsedTick;
use super::super::super::credibility::CredibilityEvent;
use super::super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent};
use crate::Error;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use super::super::super::stream::FileStreamProposal;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use std::fmt;
use std::path::Path;

/// The original exclusive learned recovery with an independently frozen complete
/// predictive guard contract. No partial owner, observer or candidate escapes.
/// The predictor must be pinned INSIDE the supplied learned recipe. An additional
/// standalone Enable cannot replace or supplement that required configuration.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::learned::FilePredictiveLearnedRecovery;
/// fn bypass(run: FilePredictiveLearnedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::learned::FilePredictiveLearnedRecovery;
/// fn early(run: &FilePredictiveLearnedRecovery) { run.roles(); }
/// ```
#[must_use = "finish the original guarded recovery or drop it without changing storage"]
pub struct FilePredictiveLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FilePredictiveRequirements,
    failure: Option<Error>,
}
impl fmt::Debug for FilePredictiveLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePredictiveLearnedRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Reconstruct the original learned journal and recover the complete original
    /// role bundle. Exact recipe/predictor and evaluator inventory are checked
    /// before any numerical replay; all guards, policy, credential epoch and
    /// independent counter floors are checked before cleanup or fence writes.
    /// Saved identity/source eligibility and effect keys remain withdrawn.
    pub fn begin_open_predictive_guarded_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FilePredictiveRequirements, config: &FileLearnedConfig)
        -> Result<FilePredictiveLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, config, None)
    }

    /// Additionally check the original independently retained exact history
    /// prefix on the SAME locked canonical bytes before numerical replay. Counter
    /// floors alone do not distinguish forks; this does not authenticate the
    /// anchor or make an out-of-date operator anchor current.
    pub fn begin_open_predictive_guarded_anchored_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FilePredictiveRequirements,
        config: &FileLearnedConfig, anchor: &FileHistoryAnchor)
        -> Result<FilePredictiveLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, config, Some(anchor))
    }

    /// Synchronous consumer of the SAME cooperative recovery. No second history
    /// replay, role getter or replacement forecast is introduced. Provision each
    /// returned role separately; unanswered forecasts remain permanently lost
    /// coverage even though a newly bound observer is returned.
    pub fn open_predictive_guarded_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FilePredictiveRequirements, config: &FileLearnedConfig)
        -> Result<(Self, FilePredictiveRoles), JournalError>
    {
        let mut run = Self::begin_open_predictive_guarded_with_learned_generation(
            directory, profile, expected, config)?;
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            let completed = run.progress().replayed_events;
            run.advance(completed, 1)?;
        }
        run.finish()
    }
}

fn begin(directory: &Path, profile: FileOversightProfile, expected: &FilePredictiveRequirements,
    config: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FilePredictiveLearnedRecovery, JournalError>
{
    check_configuration(expected, config)?;
    let inner = FileOversight::begin_open_with_learned_generation(directory, profile, config)?;
    let (profile, events, identity) = inner.guarded_history();
    if let Some(anchor) = anchor { anchor.check(profile, identity, events)?; }
    check_history(events, expected)?;
    Ok(FilePredictiveLearnedRecovery { inner, expected: expected.clone(), failure: None })
}

fn check_configuration(expected: &FilePredictiveRequirements, config: &FileLearnedConfig) -> Result<(), Error> {
    check_profile(&expected.oversight.guards)?;
    if config.required_pre_output_forecast() != Some(&expected.prediction) { return Err(Error::Binding); }
    Ok(())
}

// Used only AFTER binding the independently supplied recipe to the exact bytes.
// Both the passive reader and locked recovery admit the same original inventory.
fn check_history(events: &[Event], expected: &FilePredictiveRequirements) -> Result<(), Error> {
    // The independent recipe has already been bound byte-for-byte. Its single
    // learned Enable installs the predictor. The base inventory deliberately
    // requires NO standalone predictor, sampled decoder, stop mode or topology.
    // Never append a synthetic Enable or filter actual events to satisfy it.
    expected.oversight.guards.check_decoder_config(events)?;
    let mut protocols = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::Enable(protocol)) => Some(protocol), _ => None,
    });
    if protocols.next() != expected.evaluation.as_ref() || protocols.next().is_some() {
        return Err(Error::Binding);
    }
    Ok(())
}

impl FilePredictiveLearnedRecovery {
    // Sealed fleet composition: framing is inspected before event zero, while
    // extraction requires this wrapper's COMPLETE independent Ready checks.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn shutdown_history(&self)
        -> (&FileOversightProfile, &[Event], &Path, usize)
    {
        self.inner.shutdown_history()
    }

    pub(in crate::action::consequence::delivery::persistent::observed)
    fn into_shutdown_terminal(self)
        -> Result<crate::action::consequence::delivery::persistent::observed::shutdown::recovery::FileStopRecovery, JournalError>
    {
        self.check_ready()?;
        self.inner.into_terminal()
    }

    /// Select one original external request for offline historical policy
    /// diagnosis before any replay. The same exclusive recovery lock, numerical
    /// recipe and independent guard requirements remain held; no new admission
    /// or fictional proposal is created for a request that was not admitted.
    pub fn require_request_proposal_investigation(mut self, request: u64,
        scope: crate::action::consequence::experiment::InterventionScope)
        -> Result<Self, JournalError>
    {
        self.inner = self.inner.require_request_proposal_investigation(request, scope)?;
        Ok(self)
    }

    /// Return only the selected historical policy experiment after the whole
    /// numerical history and every independent guard qualify. The original
    /// finalizer rechecks the canonical bytes and releases the lock without
    /// cleanup, clock, fence, effect or live owner.
    pub fn finish_proposal_investigation(self)
        -> Result<crate::action::consequence::delivery::persistent::observed::guarded::investigation::FileProposalInvestigation, JournalError>
    {
        self.check_ready()?;
        self.inner.finish_proposal_investigation()
    }

    /// Complete only the selected Unknown -> IrrecoverablyUnknown disposition
    /// after this cursor's full original guard, numerical, policy, credential
    /// and floor checks. No recovery Fence, clock, live owner or role escapes.
    /// Exact retries retain the original receipt without another event; charges
    /// and blocked stream successors remain unchanged.
    pub fn finish_abandoned_with_anchor(
        self,
        request: crate::action::consequence::delivery::persistent::abandonment::FileAbandonRequest,
    ) -> Result<super::super::recovery::FileAbandonedHistory, JournalError> {
        self.check_ready()?;
        self.inner.finish_abandoned_with_anchor(request)
    }

    /// Preserve independently selected source custody through only this exact
    /// native abandonment. The saved cut or its one matching event is checked
    /// before cleanup or mutation, retaining every original wrapper requirement.
    pub fn finish_abandoned_with_checkpoint(
        self,
        request: crate::action::consequence::delivery::persistent::abandonment::FileAbandonRequest,
        saved: &crate::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::TopologyCheckpoint,
    ) -> Result<super::super::recovery::FileAbandonedHistory, JournalError> {
        self.check_ready()?;
        self.inner.finish_abandoned_with_checkpoint(request, saved)
    }

    /// Inspect only the fully replayed, independently guarded credential state.
    /// This grants no owner, permit, observer or authority and publishes no fence.
    pub fn credential_status(&self) -> Result<Option<FileCredentialStatus>, JournalError> {
        self.check_ready()?;
        Ok(self.inner.verified_guarded_machine()?.credential_status())
    }

    /// Validate the original rotation, including exact retained-operation retry,
    /// before cleanup or the recovery fence. Some is the HISTORICAL receipt;
    /// current generation remains available separately through credential_status.
    pub fn check_credential_rotation(&self, request: CredentialRotationRequest)
        -> Result<Option<CredentialChangeReceipt>, JournalError>
    {
        self.check_ready()?;
        Ok(self.inner.verified_guarded_machine()?.check_credential_rotation(request)?)
    }

    /// Apply the same original revocation preflight without issuing live roles
    /// or changing canonical/staging bytes. Terminal exact retries remain reads.
    pub fn check_credential_revocation(&self, request: CredentialRevocationRequest)
        -> Result<Option<CredentialChangeReceipt>, JournalError>
    {
        self.check_ready()?;
        Ok(self.inner.verified_guarded_machine()?.check_credential_revocation(request)?)
    }

    /// Complete this independently guarded numerical history with the original
    /// single-replacement Fence/Time/Sweep, then return only the selected actor
    /// outcome. Guard failures and bad selection precede clock and mutation.
    /// Ordinary capacity, exact canonical checks and unknown liability rules
    /// remain unchanged; no observer, reviewer or live owner escapes.
    pub fn finish_reconciled_receipt<F>(self, request: u64, finish_after: Option<u64>, clock: F)
        -> Result<Knowledge<ActorOutcome>, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_reconciled_receipt(request, finish_after, clock)
    }

    /// Retain the same original acknowledged reconciliation cut without a live
    /// owner. Every independent wrapper guard is checked before the clock.
    pub fn finish_reconciled_receipt_with_history<F>(self, request: u64,
        finish_after: Option<u64>, clock: F)
        -> Result<crate::action::consequence::delivery::persistent::observed::decoder::learned::FileReconciledLearnedReceipt, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_reconciled_receipt_with_history(request, finish_after, clock)
    }

    /// Admit the exact independently retained source checkpoint on the SAME
    /// fully guarded cursor before any reconciliation clock or mutation.
    pub fn finish_reconciled_receipt_with_checkpoint<F>(self, request: u64,
        finish_after: Option<u64>, saved: &crate::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::TopologyCheckpoint,
        clock: F)
        -> Result<crate::action::consequence::delivery::persistent::observed::decoder::learned::FileReconciledLearnedReceipt, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_reconciled_receipt_with_checkpoint(request, finish_after, saved, clock)
    }

    /// Return only the original historical actor observation after all of this
    /// wrapper's independent guards pass. None selects a message; Some selects
    /// a finish's exact confirmed predecessor. The inner finalizer rechecks the
    /// locked canonical cut without cleanup, fencing, reconciliation or roles.
    /// No reserve is consumed; pending/unknown liabilities remain unresolved.
    pub fn finish_receipt(self, request: u64, finish_after: Option<u64>)
        -> Result<Knowledge<ActorOutcome>, JournalError>
    {
        self.check_ready()?;
        self.inner.finish_receipt(request, finish_after)
    }

    /// Select an original message only after the complete independent guard,
    /// policy, recipe and floor checks pass. Read-only; no fence or role issuance.
    pub fn learned_text_stream_message_request(&self, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        self.check_ready()?;
        self.inner.learned_text_stream_message_request(request)
    }

    /// Select the exact retained finish and confirmed predecessor under the
    /// SAME full guard contract. A failed selection changes no stored bytes.
    pub fn learned_text_stream_finish_request(&self, after_request: u64, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        self.check_ready()?;
        self.inner.learned_text_stream_finish_request(after_request, request)
    }

    /// Require the exact independently selected original terminal reserve before
    /// replay. Missing, changed or late requirements release no observer, perform
    /// no cleanup/fence and replenish no allowance. Full predictive guard and
    /// policy/floor checks still run before the original finish returns roles.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        check_recovery_reserve(&self.inner, expected)?;
        Ok(self)
    }

    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure { progress.status = FileLearnedRecoveryStatus::Failed(error); }
        progress
    }

    /// One bounded count of ORIGINAL reducer events, including every numerical
    /// witness. Original replay errors/unwinds stay terminal. A completed replay
    /// with a mismatching guard is Failed, never Ready; requirements cannot be
    /// changed afterward. One event, final encoding and I/O remain synchronous.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        if let Some(error) = self.failure { return Err(error.into()); }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileLearnedRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) = self.expected.oversight.check_predictive(profile,
                self.inner.verified_guarded_machine()?, events,
                self.expected.evaluation.as_ref(), Some(&self.expected.prediction)) {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    /// Check all original guards again, acknowledge the single original fence,
    /// THEN provision the original human/identity/governance/evaluation/forecast
    /// roles. No live credential or prior approval is reconstructed. A pending
    /// forecast retains its outcome uncertainty and spent lifetime budget. The
    /// numerical owner remains paused until explicit admissible requalification
    /// and resume; returning observer custody does not bypass either requirement.
    pub fn finish(self) -> Result<(FileOversight, FilePredictiveRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let oversight = FileOversightRoles::provision(&host, human);
        let roles = FilePredictiveRoles::provision(&host, oversight, self.expected.evaluation.is_some());
        Ok((host, roles))
    }

    /// Transfer only a fixed Stop target after rechecking this wrapper's
    /// COMPLETE original recipe, guards, policy, credentials and history floors.
    /// The original private replay and exclusive lock survive; no weaker cursor,
    /// live owner or role is released before or after the nonce-bound exchange.
    #[cfg(target_os = "linux")]
    pub fn into_recovery_stop(self, operation: u64)
        -> Result<crate::action::consequence::delivery::persistent::observed::reviewer::peer::control::FileRecoveryStop, JournalError>
    {
        self.check_ready()?;
        self.inner.into_recovery_stop(operation)
    }

    /// Select the original stop predecessor after all independently required
    /// guards, predictor ownership and optional evaluation checks succeed.
    pub fn stop_request(&self, operation: u64) -> Result<StopRequest, JournalError> {
        self.check_ready()?;
        self.inner.stop_request(operation)
    }

    /// Withdraw admission through the original Stop/Fence without acquiring
    /// settlement time. Complete independent requirements, the retained reserve
    /// and canonical custody still apply; no live role or drain receipt escapes.
    pub fn finish_admission_stop(self, request: StopRequest)
        -> Result<FileAdmissionStoppedHistory, JournalError>
    {
        self.check_ready()?;
        self.inner.finish_admission_stop(request)
    }

    /// Drain only the matching acknowledged stop through the original terminal
    /// event. A failed clock preserves that stop and its charged liabilities.
    /// An already-drained retry does not acquire time or append another event.
    pub fn finish_stopped_drain<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.check_ready()?;
        self.inner.finish_stopped_drain(request, clock)
    }

    /// Retain the same fully guarded combined terminal transaction while
    /// allowing the caller to refuse unavailable or late settlement time.
    /// Refusal returns neither a candidate receipt nor a history anchor.
    pub fn try_finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> Result<ElapsedTick, JournalError> {
        self.check_ready()?;
        self.inner.try_finish_stopped_with_anchor(request, clock)
    }

    /// Finish the sealed predictive replay with original Stop/Fence/StopProgress,
    /// returning only stopped observations. No observer, evaluator or human role
    /// escapes; unknown liability and the exact retained reserve are preserved.
    pub fn finish_stopped<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_stopped(request, clock)
    }

    /// Capture the exact acknowledged stopped history without returning an
    /// observer or evaluator. The same complete predictive guard checks run
    /// before the original terminal transaction and its anchor capture.
    pub fn finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_stopped_with_anchor(request, clock)
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        if let Some(error) = self.failure { return Err(error.into()); }
        let (profile, events, _) = self.inner.guarded_history();
        self.expected.oversight.check_predictive(profile, self.inner.verified_guarded_machine()?,
            events, self.expected.evaluation.as_ref(), Some(&self.expected.prediction))?;
        Ok(())
    }

    /// Complete the exact independently retained pending reset before the
    /// original recovery fence. Recheck the full predictive/evaluator/guard
    /// contract before and after the original reset, then publish both events
    /// together before releasing any role. An already completed matching reset
    /// takes only the original fence; an interrupted or changed intent refuses.
    /// Predictor timing, lifetime spend and unanswered-forecast coverage loss
    /// remain outside the restored checkpoint. Reset is never a fresh forecast.
    pub fn finish_pending_reset(mut self, intent: &FileLearnedResetIntent)
        -> Result<(FileOversight, FilePredictiveRoles), JournalError>
    {
        self.check_ready()?;
        self.inner = self.inner.prepare_pending_reset(intent)?;
        self.finish()
    }
}

#[cfg(test)]
mod tests;
