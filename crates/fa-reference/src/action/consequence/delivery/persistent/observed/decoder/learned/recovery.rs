//! Cooperative replay of the original learned numerical and authority history.
//! The existing locked store remains the only externally visible effect sink.
mod reset;
mod stopped;
mod receipt;
mod investigation;
mod policy_replay;
mod abandonment;
pub use receipt::FileReconciledLearnedReceipt;

use super::{FileHumanReviewer, FileLearnedConfig, FileOversight, FileOversightProfile,
    JournalError, Machine, bind_history, journal, storage};
use super::super::super::{BaseEvent, Event, Transition};
use super::super::super::shutdown::FileShutdownDrain;
use super::super::super::guarded::investigation::Selection;
use super::super::super::super::MAX_JOURNAL_EVENTS;
use crate::Error;
use crate::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use super::super::super::stream::FileStreamProposal;
use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileLearnedRecoveryStatus {
    Replaying,
    /// Every original event is verified; the durable recovery fence is NOT yet
    /// published. Only a successful `finish` returns a live owner and reviewer.
    Ready,
    /// An original reducer call did not return. A caught unwind cannot resume
    /// this partially mutated candidate or expose its historical authority.
    Interrupted,
    Failed(Error),
}

/// Reconstruction progress only. An event may include one whole numerical step
/// or another bounded original operation; event count is not a CPU/time receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileLearnedRecoveryProgress {
    pub replayed_events: usize,
    pub total_events: usize,
    pub status: FileLearnedRecoveryStatus,
}

/// Incomplete reconstruction with the original exclusive lock. No tokens,
/// writable owner, helper session, reviewer or old key escape while pending.
/// Dropping it (including after an early `finish`) leaves storage untouched.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedRecovery;
/// fn premature_authority(recovery: &FileLearnedRecovery) { recovery.owner(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedRecovery;
/// fn duplicate(recovery: FileLearnedRecovery) { let _ = recovery.clone(); }
/// ```
#[must_use = "advance and finish recovery, or drop it without exposing a live owner"]
pub struct FileLearnedRecovery {
    profile: FileOversightProfile,
    store: storage::Store,
    events: Vec<Event>,
    machine: Machine,
    expected: FileLearnedConfig,
    canonical: Vec<u8>,
    replayed: usize,
    status: FileLearnedRecoveryStatus,
    investigation: Option<Selection>,
    last_drain: Option<FileShutdownDrain>,
}

impl fmt::Debug for FileLearnedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileLearnedRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Validate the complete canonical framing and independently selected recipe
    /// before executing any journal event. The original lock and exact bytes are
    /// retained across quanta; unfinished recovery performs no storage cleanup.
    pub fn begin_open_with_learned_generation(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileLearnedConfig)
        -> Result<FileLearnedRecovery, JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        let canonical = store.read(profile.delivery.limits.bytes)?;
        let mut events = journal::decode(&profile, store.identity(), &canonical)?;
        bind_history(&mut events, expected)?;
        let machine = Machine::new(&profile)?;
        Ok(FileLearnedRecovery { profile, store, events, machine,
            expected: expected.clone(), canonical, replayed: 0,
            status: FileLearnedRecoveryStatus::Replaying, investigation: None, last_drain: None })
    }
}

impl FileLearnedRecovery {
    // Internal typed composition before the first reducer call. The original
    // binder compares BOTH independent recipes, rejects supplied captures and
    // changes only runtime bindings, never canonical event/witness bytes. No
    // public callback or partially replayed state can install new expectations.
    pub(in super::super::super) fn bind_owned_prediction(mut self,
        prediction: &super::super::super::consistency::learned::FileLearnedConsistencyConfig)
        -> Result<Self, JournalError>
    {
        if self.replayed != 0 || self.status != FileLearnedRecoveryStatus::Replaying {
            return Err(Error::WrongState.into());
        }
        super::super::super::consistency::learned::owned::bind_owned_history(
            &mut self.events, &self.expected, prediction)?;
        Ok(self)
    }

    pub fn progress(&self) -> FileLearnedRecoveryProgress {
        FileLearnedRecoveryProgress { replayed_events: self.replayed,
            total_events: self.events.len(), status: self.status }
    }

    /// Execute at most `max_events` original transitions. A stale predecessor or
    /// invalid quantum performs no work. No original numerical budget is refilled
    /// between calls. One event and its witness comparison remain synchronous.
    /// The first reducer failure is retained, even if it changed private state.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileLearnedRecoveryProgress, JournalError>
    {
        match self.status {
            FileLearnedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileLearnedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => {}
        }
        if expected_events != self.replayed { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        if self.status == FileLearnedRecoveryStatus::Ready { return Ok(self.progress()); }
        let end = self.replayed.saturating_add(max_events).min(self.events.len());
        while self.replayed < end {
            // Latch BEFORE the original reducer, including its numerical work.
            // An unwinding caller cannot retry partially applied history.
            self.status = FileLearnedRecoveryStatus::Interrupted;
            // A selected investigation captures the admission in this SAME
            // reducer pass; it never replays numerical history a second time.
            let applied = match self.investigation.as_mut() {
                Some(selection) => selection.apply_transition(&self.profile, &mut self.machine,
                    self.replayed, &self.events[self.replayed]),
                None => self.machine.apply(&self.events[self.replayed]).map_err(JournalError::from),
            };
            let transition = match applied {
                Ok(transition) => transition,
                Err(error) => {
                    if let JournalError::Contract(cause) = &error {
                        self.status = FileLearnedRecoveryStatus::Failed(*cause);
                    }
                    return Err(error);
                }
            };
            if let Transition::StopProgressed(sweep) = transition {
                self.last_drain = Some(FileShutdownDrain {
                    journal_revision: (self.replayed + 1) as u64, sweep,
                });
            }
            self.replayed += 1;
            self.status = FileLearnedRecoveryStatus::Replaying;
        }
        if self.replayed == self.events.len() {
            if self.machine.learned_contract() != Some(&self.expected) {
                self.status = FileLearnedRecoveryStatus::Failed(Error::Binding);
                return Err(Error::Binding.into());
            }
            self.status = FileLearnedRecoveryStatus::Ready;
        }
        Ok(self.progress())
    }

    /// Verify the exact canonical cut, then acknowledge the ORIGINAL recovery
    /// fence before returning either role. Old approvals are withdrawn, unknown
    /// effects remain charged, and learned inference is paused. This does not
    /// replay numerical history again. Encoding, final storage I/O and the fence
    /// remain synchronous; cooperating-writer exclusion is still required.
    pub fn finish(self) -> Result<(FileOversight, FileHumanReviewer), JournalError> {
        match self.status {
            FileLearnedRecoveryStatus::Ready => {}
            FileLearnedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileLearnedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            FileLearnedRecoveryStatus::Replaying => return Err(Error::Incomplete.into()),
        }
        let Self { profile, store, events, mut machine, canonical, .. } = self;
        if events.len() >= profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let event = Event::Core(BaseEvent::Fence);
        let bytes = journal::encode_appended(&profile, store.identity(), &events, &event)?;
        machine.preflight_consistency(&event)?;
        let result = machine.apply(&event)?;
        // An empty private placeholder lets the SAME persist_candidate boundary
        // install the fully replayed/fenced machine. It performs no learned work
        // and is never exposed as a usable owner, even on a storage failure.
        let placeholder = Machine::new(&profile)?;
        if store.read(profile.delivery.limits.bytes)? != canonical { return Err(Error::Binding.into()); }
        store.confirm_and_cleanup()?;
        let (mut owner, reviewer) = FileOversight::owner(profile, store, events, placeholder);
        owner.check_source_admission(&event)?;
        owner.persist_candidate(event, bytes, machine, result)?;
        Ok((owner, reviewer))
    }

    /// Historical message identity from a COMPLETE original replay, including
    /// policy refusals. No live owner, fence, cleanup, clock or approval escapes.
    /// The descriptor is not proof of publication. Finish still checks the exact
    /// canonical cut; dropping this cursor after a bad selection writes nothing.
    pub fn learned_text_stream_message_request(&self, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        Ok(self.verified_guarded_machine()?.learned_text_stream_message_request(
            self.profile.delivery.scope, request)?)
    }

    /// Historical finish plus its exact accepted predecessor receipt. Missing,
    /// wrong-kind or unconfirmed requests cannot consume a recovery fence.
    /// As above, incomplete/failed replay supplies no descriptor or live role.
    pub fn learned_text_stream_finish_request(&self, after_request: u64, request: u64)
        -> Result<FileStreamProposal, JournalError>
    {
        Ok(self.verified_guarded_machine()?.learned_text_stream_finish_request(
            self.profile.delivery.scope, after_request, request)?)
    }

    /// Consume a complete replay into the ORIGINAL redacted actor observation,
    /// without recovering a live owner or spending any journal capacity.
    /// `None` selects a message; `Some(previous_request)` explicitly selects a
    /// finish with that exact confirmed predecessor. There is no format fallback.
    ///
    /// This is a historical observation at the verified canonical cut, not a
    /// fresh evaluation or a reconciliation. Pending and unknown outcomes stay
    /// pending and unknown even when publication is ahead of its receipt. No
    /// clock, source, helper, fence, cleanup, sync, reset or new inference runs.
    /// Numerical work needed to VERIFY the original history still runs in replay.
    ///
    /// The exact canonical bytes are re-read under the SAME exclusive lock before
    /// anything escapes. Changed bytes or incomplete/failed replay return no
    /// observation. The consumed private machine and lock are dropped on either
    /// outcome. Full journals remain inspectable: no spare event or byte is needed.
    /// Callers must still independently authenticate access to the selected run.
    pub fn finish_receipt(self, request: u64, finish_after: Option<u64>)
        -> Result<Knowledge<ActorOutcome>, JournalError>
    {
        match finish_after {
            None => { self.learned_text_stream_message_request(request)?; }
            Some(after) => { self.learned_text_stream_finish_request(after, request)?; }
        }
        let status = self.verified_guarded_machine()?.requests.status(request)?;
        if self.store.read(self.profile.delivery.limits.bytes)? != self.canonical {
            return Err(Error::Binding.into());
        }
        Ok(status.actor_observation())
    }

    /// Settle original endpoint receipts after complete numerical replay, using
    /// the SAME fixed Fence -> Time -> Sweep transaction as guarded recovery.
    /// Only the selected redacted actor outcome escapes; no live owner or role
    /// is returned. A sweep covers existing liabilities, never sends an effect.
    ///
    /// Selection (including an exact finish predecessor) happens before the
    /// trusted clock or any write. The clock is called once after replay and
    /// canonical-cut validation. Every original prefix/byte/source check still
    /// applies, and the canonical image is checked again before replacement.
    /// A failed write may expose the whole new image but returns no receipt.
    ///
    /// This is MUTATING recovery, unlike finish_receipt: it spends three events
    /// and needs ordinary capacity for Time/Sweep. Terminal-only reserve cannot
    /// fund it. Missing receipts remain Unknown and charged; no numerical work,
    /// source eligibility, old approval or lost forecast budget is reinstated.
    /// Repeated calls are not zero-event retries. For observation without any
    /// write use finish_receipt; for exhausted ordinary capacity use stopped
    /// recovery. Independent access authorization remains the caller's duty.
    pub fn finish_reconciled_receipt<F>(self, request: u64, finish_after: Option<u64>, clock: F)
        -> Result<Knowledge<ActorOutcome>, JournalError>
    where F: FnOnce() -> crate::action::ElapsedTick {
        match finish_after {
            None => { self.learned_text_stream_message_request(request)?; }
            Some(after) => { self.learned_text_stream_finish_request(after, request)?; }
        }
        let Self { profile, store, events, machine, canonical, .. } = self;
        let recovered = FileOversight::finish_reconciled_history(
            profile, store, events, machine, canonical, clock)?;
        // The retained book has no removal/reassignment operation. Reuse the
        // original live-poll projection only AFTER the replacement acknowledges.
        Ok(recovered.owner.request_status(request)?.actor_observation())
    }

    // Immutable composition views stop at the original observed-owner family.
    // Framing is available before replay, but the machine only after full
    // verification. Neither method exposes a publicly executable candidate.
    pub(in super::super::super) fn guarded_history(&self)
        -> (&FileOversightProfile, &[Event], &Path)
    {
        (&self.profile, &self.events, self.store.identity())
    }

    // Fleet admission inspects this SAME locked image before event zero.
    // Only a fully checked wrapper may consume the later terminal handoff.
    pub(in super::super::super) fn shutdown_history(&self)
        -> (&FileOversightProfile, &[Event], &Path, usize)
    {
        (&self.profile, &self.events, self.store.identity(), self.canonical.len())
    }

    pub(in super::super::super) fn verified_guarded_machine(&self) -> Result<&Machine, JournalError> {
        match self.status {
            FileLearnedRecoveryStatus::Ready => Ok(&self.machine),
            FileLearnedRecoveryStatus::Failed(error) => Err(error.into()),
            FileLearnedRecoveryStatus::Interrupted => Err(JournalError::Unavailable),
            FileLearnedRecoveryStatus::Replaying => Err(Error::Incomplete.into()),
        }
    }

    #[cfg(test)]
    pub(in super::super::super) fn fail_once(&self, operation: super::JournalIo) {
        self.store.fail_once(operation);
    }
}
