//! Single-pass guarded recovery over the original journal and machine.
//! Polls retain the original Store lock. Only the acknowledged fence releases
//! an owner and its separate roles; no partially replayed authority is exported.
use super::{BaseEvent, Event, FileOversight, FileOversightProfile, FileOversightRoles,
    FileRecoveryRequirements, JournalError, Machine, anchored::FileHistoryAnchor, journal, storage};
use super::super::machine::Transition;
use crate::action::consequence::delivery::persistent::{MAX_JOURNAL_EVENTS, RecoveryReserve};
use crate::Error;
use std::fmt;
use std::path::Path;

mod stopped;
mod publication;
pub use publication::FilePublicationRecoveryRequirements;
mod reconciled;
pub use reconciled::ReconciledGuardedOwner;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileGuardedRecoveryStatus {
    Replaying,
    /// All original events AND the independent guard requirements passed.
    /// No fence has been published and no live owner or role has been issued.
    Ready,
    /// An original transition did not return; its private state is unusable.
    Interrupted,
    Failed(Error),
    Cancelled,
}

/// Counts original transition calls, not physical numerical work or CPU time.
/// A failed/unwound call is attempted but not counted as a verified event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileGuardedRecoveryProgress {
    pub replayed_events: usize,
    pub attempted_events: usize,
    pub total_events: usize,
    pub status: FileGuardedRecoveryStatus,
}

/// One frozen canonical image, independently selected guard inventory, and
/// private reconstruction. Construction performs the existing bounded file read,
/// framing and configuration/anchor checks. Only event execution is cooperative.
/// One event can contain a complete original decoder operation; a quantum is not
/// a wall-clock bound, and no original numerical or journal limit is enlarged.
///
/// Dropping or cancelling, including after Ready, releases the lock without
/// cleaning staging files, fencing, changing canonical bytes or issuing roles.
/// This is the base guarded profile: evaluated, predictive, mediated and learned
/// profiles keep their own explicit recovery contracts and are not downgraded.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::recovery::FileGuardedRecovery;
/// fn early_owner(run: FileGuardedRecovery) { run.owner(); }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::recovery::FileGuardedRecovery;
/// fn early_roles(run: &FileGuardedRecovery) { run.roles(); }
/// ```
/// ```compile_fail,E0382
/// use fa_reference::action::consequence::delivery::persistent::observed::guarded::recovery::FileGuardedRecovery;
/// fn twice(run: FileGuardedRecovery) { let _ = run.finish(); let _ = run.finish(); }
/// ```
#[must_use = "complete recovery or drop it without issuing an owner or roles"]
pub struct FileGuardedRecovery {
    profile: FileOversightProfile,
    expected: FileRecoveryRequirements,
    store: storage::Store,
    canonical: Vec<u8>,
    events: Vec<Event>,
    machine: Machine,
    replayed: usize,
    attempted: usize,
    status: FileGuardedRecoveryStatus,
}

impl fmt::Debug for FileGuardedRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileGuardedRecovery").field("progress", &self.progress()).finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Freeze the original guarded recovery inputs and retain the exclusive
    /// writer lock without executing history. No fresh observation is invented.
    pub fn begin_open_guarded(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements) -> Result<FileGuardedRecovery, JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        FileGuardedRecovery::begin_store(store, profile, expected, None)
    }

    /// Check the exact independently retained prefix before ANY event execution.
    /// The same locked canonical image supplies prefix and complete-suffix replay.
    /// Matching the prefix never excuses an invalid suffix or wrong guard set.
    pub fn begin_open_guarded_anchored(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor)
        -> Result<FileGuardedRecovery, JournalError>
    {
        profile.delivery.limits.check()?;
        let store = storage::Store::open(directory.as_ref())?;
        FileGuardedRecovery::begin_store(store, profile, expected, Some(anchor))
    }
}

impl FileGuardedRecovery {
    pub(super) fn begin_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, anchor: Option<&FileHistoryAnchor>)
        -> Result<Self, JournalError>
    {
        let canonical = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &canonical)?;
        if let Some(anchor) = anchor { anchor.check(&profile, store.identity(), &events)?; }
        expected.guards.check_decoder_config(&events)?;
        let machine = Machine::new(&profile)?;
        Ok(Self { profile, expected: expected.clone(), store, canonical, events, machine,
            replayed: 0, attempted: 0, status: FileGuardedRecoveryStatus::Replaying })
    }

    /// Pin the existing original recovery reserve before the first replay call.
    /// This consumes a mismatching/late cursor without cleanup or writes. It
    /// never installs a missing reserve, changes limits or replenishes capacity.
    /// Original journal decoding validates the reserve's placement and bounds.
    pub fn require_recovery_reserve(self, expected: RecoveryReserve) -> Result<Self, JournalError> {
        if self.attempted != 0 || self.status != FileGuardedRecoveryStatus::Replaying {
            return Err(Error::WrongState.into());
        }
        let mut reserves = self.events.iter().filter_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(actual)) => Some(*actual),
            _ => None,
        });
        if reserves.next() != Some(expected) || reserves.next().is_some() {
            return Err(Error::Binding.into());
        }
        Ok(self)
    }

    pub fn progress(&self) -> FileGuardedRecoveryProgress {
        FileGuardedRecoveryProgress { replayed_events: self.replayed,
            attempted_events: self.attempted, total_events: self.events.len(), status: self.status }
    }

    /// Execute at most max_events original transitions from the exact verified
    /// predecessor. Stale/invalid polls do no work. Failures and unwinds latch
    /// before any later call can reuse a partially changed private machine.
    pub fn advance(&mut self, expected_events: usize, max_events: usize)
        -> Result<FileGuardedRecoveryProgress, JournalError>
    {
        self.advance_with(expected_events, max_events, Machine::apply)
    }

    fn advance_with(&mut self, expected_events: usize, max_events: usize,
        mut apply: impl FnMut(&mut Machine, &Event) -> Result<Transition, Error>)
        -> Result<FileGuardedRecoveryProgress, JournalError>
    {
        match self.status {
            FileGuardedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileGuardedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            FileGuardedRecoveryStatus::Cancelled => return Err(Error::WrongState.into()),
            _ => {}
        }
        if expected_events != self.replayed { return Err(Error::Stale.into()); }
        if max_events == 0 { return Err(Error::InvalidInput.into()); }
        if max_events > MAX_JOURNAL_EVENTS { return Err(Error::Limit.into()); }
        if self.status == FileGuardedRecoveryStatus::Ready { return Ok(self.progress()); }
        let end = self.replayed.saturating_add(max_events).min(self.events.len());
        while self.replayed < end {
            self.status = FileGuardedRecoveryStatus::Interrupted;
            self.attempted += 1; // At most the decoded, bounded original event count.
            if let Err(error) = apply(&mut self.machine, &self.events[self.replayed]) {
                self.status = FileGuardedRecoveryStatus::Failed(error);
                return Err(error.into());
            }
            self.replayed += 1;
        }
        // The guard check can itself inspect original retained history. Do not
        // expose Ready or allow resume if that check fails or unwinds either.
        self.status = FileGuardedRecoveryStatus::Interrupted;
        if self.replayed == self.events.len() {
            if let Err(error) = self.expected.check(&self.profile, &self.machine, &self.events) {
                self.status = FileGuardedRecoveryStatus::Failed(error);
                return Err(error.into());
            }
            self.status = FileGuardedRecoveryStatus::Ready;
        } else {
            self.status = FileGuardedRecoveryStatus::Replaying;
        }
        Ok(self.progress())
    }

    /// Discard private reconstruction only; no canonical or staging file changes.
    /// Preserve a terminal failure/interruption in the returned progress record.
    pub fn cancel(self) -> FileGuardedRecoveryProgress {
        let mut progress = self.progress();
        if matches!(progress.status, FileGuardedRecoveryStatus::Replaying | FileGuardedRecoveryStatus::Ready) {
            progress.status = FileGuardedRecoveryStatus::Cancelled;
        }
        progress
    }

    /// Apply ONE original fence to the already verified machine, then publish
    /// through the existing canonical replacement/poison boundary. No historical
    /// transition is replayed again. All old approvals and saved eligibility are
    /// withdrawn; pending effects remain original reconciliation obligations.
    ///
    /// Capacity, encoding and the exact retained-canonical comparison precede
    /// staging cleanup. File I/O, final encoding and the fence are synchronous.
    /// An error or unwind returns neither an owner nor any independently held role.
    pub fn finish(self) -> Result<(FileOversight, FileOversightRoles), JournalError> {
        match self.status {
            FileGuardedRecoveryStatus::Ready => {}
            FileGuardedRecoveryStatus::Failed(error) => return Err(error.into()),
            FileGuardedRecoveryStatus::Interrupted => return Err(JournalError::Unavailable),
            _ => return Err(Error::Incomplete.into()),
        }
        self.expected.check(&self.profile, &self.machine, &self.events)?;
        let Self { profile, store, canonical, events, mut machine, .. } = self;
        if events.len() >= profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let event = Event::Core(BaseEvent::Fence);
        let bytes = journal::encode_appended(&profile, store.identity(), &events, &event)?;
        machine.preflight_consistency(&event)?;
        let result = machine.apply(&event)?;
        // The original persistence seam installs the verified/fenced machine.
        // This empty placeholder runs no historical/numerical events and cannot
        // escape on either success or a failed replacement.
        let placeholder = Machine::new(&profile)?;
        if store.read(profile.delivery.limits.bytes)? != canonical { return Err(Error::Binding.into()); }
        store.confirm_and_cleanup()?;
        let (mut host, human) = FileOversight::owner(profile, store, events, placeholder);
        host.check_source_admission(&event)?;
        host.persist_candidate(event, bytes, machine, result)?;
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }

    pub(super) fn finish_synchronously(mut self)
        -> Result<(FileOversight, FileOversightRoles), JournalError>
    {
        while self.status == FileGuardedRecoveryStatus::Replaying {
            self.advance(self.replayed, MAX_JOURNAL_EVENTS)?;
        }
        self.finish()
    }

    #[cfg(test)]
    pub(super) fn interrupt_after_original_event(&mut self) {
        let _ = self.advance_with(self.replayed, 1, |machine, event| {
            let _ = machine.apply(event)?;
            panic!("injected interruption after original event application");
        });
    }
}
