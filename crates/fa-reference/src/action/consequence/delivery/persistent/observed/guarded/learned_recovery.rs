//! Cooperative learned recovery with the original complete guard/role contract.
//! The inner candidate, original reset and sole persistence boundary stay sealed.
use super::{FileOversight, FileOversightProfile, FileOversightRoles, FileRecoveryRequirements,
    JournalError, anchored::FileHistoryAnchor, learned::check_profile};
use super::super::decoder::learned::{FileLearnedConfig, FileLearnedRecovery,
    FileLearnedRecoveryProgress, FileLearnedRecoveryStatus, checkpoint::FileLearnedResetIntent};
use crate::Error;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use super::super::stream::FileStreamProposal;
use crate::action::ElapsedTick;
use super::super::shutdown::recovery::{FileAdmissionStoppedHistory, FileStoppedHistory, FileStoppedRecovery};
use crate::action::consequence::delivery::StopRequest;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use super::{BaseEvent, Event};
use std::fmt;
use std::path::Path;

/// Holds the SAME original exclusive recovery, not a second writer or replay
/// engine. Guard requirements are frozen independently before event execution.
/// No partial role bundle, base-owner extraction or mutable machine is exposed.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::learned_recovery::FileGuardedLearnedRecovery;
/// fn bypass(run: FileGuardedLearnedRecovery) { run.into_inner().finish(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::learned_recovery::FileGuardedLearnedRecovery;
/// fn early_role(run: &FileGuardedLearnedRecovery) { run.roles(); }
/// ```
#[must_use = "advance and finish guarded recovery, or drop it without changing storage"]
pub struct FileGuardedLearnedRecovery {
    inner: FileLearnedRecovery,
    expected: FileRecoveryRequirements,
    failure: Option<Error>,
}
impl fmt::Debug for FileGuardedLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGuardedLearnedRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Match the original decoder inventory and exact learned recipe before any
    /// numerical replay. Completion additionally checks the full original guard,
    /// effective-policy, credential-epoch and independent recovery-floor contract.
    pub fn begin_open_guarded_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileRecoveryRequirements, config: &FileLearnedConfig)
        -> Result<FileGuardedLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, config, None)
    }

    /// Check an independently retained exact prefix on the SAME canonical bytes
    /// and exclusive lock before numerical replay. Genuine append-only successors
    /// are accepted by the original anchor checker; divergent/truncated cuts are
    /// not. This does not authenticate observations or advance the held anchor.
    pub fn begin_open_guarded_anchored_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileRecoveryRequirements,
        config: &FileLearnedConfig, anchor: &FileHistoryAnchor)
        -> Result<FileGuardedLearnedRecovery, JournalError>
    {
        begin(directory.as_ref(), profile, expected, config, Some(anchor))
    }
}

fn begin(directory: &Path, profile: FileOversightProfile, expected: &FileRecoveryRequirements,
    config: &FileLearnedConfig, anchor: Option<&FileHistoryAnchor>)
    -> Result<FileGuardedLearnedRecovery, JournalError>
{
    check_profile(&expected.guards)?;
    let inner = FileOversight::begin_open_with_learned_generation(directory, profile, config)?;
    let (profile, events, identity) = inner.guarded_history();
    if let Some(anchor) = anchor { anchor.check(profile, identity, events)?; }
    expected.guards.check_decoder_config(events)?;
    Ok(FileGuardedLearnedRecovery { inner, expected: expected.clone(), failure: None })
}

impl FileGuardedLearnedRecovery {
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

    /// Require exactly the independently selected original logical reserve
    /// from the SAME locked canonical history, before the first replay step.
    /// Missing or different reserves return no owner and perform no cleanup or
    /// fence. No reserve is installed or enlarged during recovery. The original
    /// decoder already verifies its unique pre-work position and both limits;
    /// finish still verifies the retained canonical cut before writing.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        check_recovery_reserve(&self.inner, expected)?;
        Ok(self)
    }

    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        let mut progress = self.inner.progress();
        if let Some(error) = self.failure { progress.status = FileLearnedRecoveryStatus::Failed(error); }
        progress
    }

    /// Execute at most the requested count of ORIGINAL journal transitions.
    /// A guard mismatch is retained as terminal, never reported as Ready or
    /// recoverable by changing requirements. Costs and numerical failures stay
    /// those of the original replay. Event quanta are not wall-clock bounds.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        if let Some(error) = self.failure { return Err(error.into()); }
        self.inner.advance(expected_events, max_events)?;
        if self.inner.progress().status == FileLearnedRecoveryStatus::Ready {
            let (profile, events, _) = self.inner.guarded_history();
            if let Err(error) = self.expected.check(profile, self.inner.verified_guarded_machine()?, events) {
                self.failure = Some(error);
                return Err(error.into());
            }
        }
        Ok(self.progress())
    }

    fn check_ready(&self) -> Result<(), JournalError> {
        if let Some(error) = self.failure { return Err(error.into()); }
        let machine = self.inner.verified_guarded_machine()?;
        let (profile, events, _) = self.inner.guarded_history();
        self.expected.check(profile, machine, events)?;
        Ok(())
    }

    /// Publish the original recovery fence, then provision ALL original roles.
    /// No numerical history is replayed twice. Saved identity/clock eligibility
    /// and old keys stay withdrawn; a pending reset is quarantined as before.
    pub fn finish(self) -> Result<(FileOversight, FileOversightRoles), JournalError> {
        self.check_ready()?;
        let (host, human) = self.inner.finish()?;
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }

    /// Select the original stop predecessor only after the complete independent
    /// guard, effective-policy, credential and recovery-floor contract passes.
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

    /// Consume the same guarded replay directly into original stopped recovery.
    /// No ordinary fence precedes stopping and no live role bundle escapes. The
    /// exact retained reserve, anchor and canonical-cut checks remain in force.
    pub fn finish_stopped<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedRecovery, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_stopped(request, clock)
    }

    /// Return the acknowledged stopped cut together with its exact private
    /// history anchor. All independent guards and the original canonical lock
    /// remain mandatory; capture grants no role and adds no replay or fence.
    pub fn finish_stopped_with_anchor<F>(self, request: StopRequest, clock: F)
        -> Result<FileStoppedHistory, JournalError>
    where F: FnOnce() -> ElapsedTick {
        self.check_ready()?;
        self.inner.finish_stopped_with_anchor(request, clock)
    }

    /// Check the complete guard contract before AND after executing the exact
    /// independently selected original pending reset. Only the existing atomic
    /// completion-plus-fence replacement exposes owner/roles. Completed matching
    /// commands do not reset again; interrupted or changed commands still refuse.
    /// Original native refusals remain acknowledged results, not permitting keys.
    pub fn finish_pending_reset(mut self, intent: &FileLearnedResetIntent)
        -> Result<(FileOversight, FileOversightRoles), JournalError>
    {
        self.check_ready()?;
        self.inner = self.inner.prepare_pending_reset(intent)?;
        self.finish()
    }

    #[cfg(test)]
    pub(in super::super) fn fail_once(&self, operation: super::super::super::JournalIo) {
        self.inner.fail_once(operation);
    }
}

// All learned recovery wrappers inspect the SAME locked original event list.
// This is admission before replay, never reserve installation or replenishment.
pub(super) fn check_recovery_reserve(inner: &FileLearnedRecovery, expected: RecoveryReserve)
    -> Result<(), JournalError>
{
    let progress = inner.progress();
    if progress.replayed_events != 0 || progress.status != FileLearnedRecoveryStatus::Replaying {
        return Err(Error::WrongState.into());
    }
    let (_, events, _) = inner.guarded_history();
    let mut reserves = events.iter().filter_map(|event| match event {
        Event::Core(BaseEvent::ReserveRecovery(actual)) => Some(*actual),
        _ => None,
    });
    if reserves.next() != Some(expected) || reserves.next().is_some() {
        return Err(Error::Binding.into());
    }
    Ok(())
}
