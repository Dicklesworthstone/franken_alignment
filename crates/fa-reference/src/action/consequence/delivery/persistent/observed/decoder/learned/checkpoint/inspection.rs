//! Historical reset discovery without taking ownership of the journal.
//! Replay verifies the whole canonical cut; no saved result becomes authority.
use super::{CheckpointEvent, DecoderEvent, Event, FileLearnedResetIntent, FileOversight,
    JournalError, LearnedEvent, Machine, PendingLearnedReset, journal};
use super::super::{FileLearnedConfig, FileOversightProfile, bind_history, storage};
use crate::action::consequence::oversight::learned_host::checkpoint::HostedLearnedResetReceipt;
use crate::Error;
use std::path::Path;

/// What the complete original replay established at the selected journal cut.
/// NotRecorded is absence from THAT cut, not proof that no reset work ever ran.
/// Pending does not prove an active worker, even when interrupted is false.
#[derive(Clone, Debug)]
pub enum FileLearnedResetRecord {
    NotRecorded,
    Pending(PendingLearnedReset),
    Completed {
        intent: FileLearnedResetIntent,
        result: Result<HostedLearnedResetReceipt, Error>,
    },
}

/// Read-only evidence about canonical bytes. This creates no writable owner,
/// checkpoint handle, reviewer, fence, resumed source or effect key. A visible
/// completion after lost directory-sync acknowledgment is not a new durability
/// acknowledgment. The writer may advance after this cut was captured.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::checkpoint::FileLearnedResetSnapshot;
/// fn grant(read: FileLearnedResetSnapshot) -> FilePermit { read }
/// ```
#[derive(Clone, Debug)]
pub struct FileLearnedResetSnapshot {
    journal_revision: u64,
    operation: u64,
    record: FileLearnedResetRecord,
}
impl FileLearnedResetSnapshot {
    pub fn journal_revision(&self) -> u64 { self.journal_revision }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn record(&self) -> &FileLearnedResetRecord { &self.record }
}

impl FileOversight {
    /// Recover the original reset result even when the writer is faulted, still
    /// holds its lock, or cannot append another fence because the journal is full.
    /// The independently supplied recipe must match BEFORE numerical replay.
    /// Every event, including events after this operation, must verify. Neither
    /// canonical nor staged files are changed; an unfinished intent is not run.
    ///
    /// This is synchronous, bounded original replay, not a cached receipt reader
    /// or authentication of local storage. Absence and progress are relative to
    /// one complete canonical read under the existing storage trust boundary.
    pub fn read_learned_reset_result(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileLearnedConfig, operation: u64) -> Result<FileLearnedResetSnapshot, JournalError>
    {
        if operation == 0 { return Err(Error::InvalidInput.into()); }
        super::super::super::super::super::codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let mut events = journal::decode(profile, &identity, &bytes)?;
        bind_history(&mut events, expected)?;
        let machine = Machine::replay(profile, &events)?;
        let intent = events.iter().find_map(|event| match event {
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Checkpoint(
                CheckpointEvent::BeginReset(intent)))) if intent.control().operation == operation =>
                    Some(intent.as_ref().clone()),
            _ => None,
        });
        let record = if let Some(result) = machine.learned_reset_result_optional(operation)? {
            FileLearnedResetRecord::Completed { intent: intent.ok_or(Error::Binding)?, result }
        } else if let Some(pending) = machine.pending_learned_reset()
            .filter(|pending| pending.intent.control().operation == operation) {
            if intent.as_ref() != Some(&pending.intent) { return Err(Error::Binding.into()); }
            FileLearnedResetRecord::Pending(pending.clone())
        } else {
            if intent.is_some() { return Err(Error::Binding.into()); }
            FileLearnedResetRecord::NotRecorded
        };
        Ok(FileLearnedResetSnapshot { journal_revision: u64::try_from(events.len()).map_err(|_| Error::Overflow)?,
            operation, record })
    }
}
