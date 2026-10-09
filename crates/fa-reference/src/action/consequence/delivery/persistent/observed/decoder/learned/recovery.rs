//! Cooperative replay of the original learned numerical and authority history.
//! The existing locked store remains the only externally visible effect sink.
mod reset;

use super::{FileHumanReviewer, FileLearnedConfig, FileOversight, FileOversightProfile,
    JournalError, Machine, bind_history, journal, storage};
use super::super::super::{BaseEvent, Event};
use super::super::super::super::MAX_JOURNAL_EVENTS;
use crate::Error;
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
            status: FileLearnedRecoveryStatus::Replaying })
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
            if let Err(error) = self.machine.apply(&self.events[self.replayed]) {
                self.status = FileLearnedRecoveryStatus::Failed(error);
                return Err(error.into());
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

    // Immutable composition views stop at the original observed-owner family.
    // Framing is available before replay, but the machine only after full
    // verification. Neither method exposes a publicly executable candidate.
    pub(in super::super::super) fn guarded_history(&self)
        -> (&FileOversightProfile, &[Event], &Path)
    {
        (&self.profile, &self.events, self.store.identity())
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
