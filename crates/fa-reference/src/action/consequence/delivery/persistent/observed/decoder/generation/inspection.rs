//! Read canonical generation evidence without creating a live writer or role.
use super::{FileGenerationCommand, FileGenerationReceipt};
use super::super::{DecoderEvent, FileDecoderConfig, FileDecoderInspection, FileOversight,
    FileOversightProfile, JournalError, Machine, Event, journal, storage};
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;
use crate::Error;
use std::path::Path;

/// Pending means no completed result in THIS canonical image. Computation may
/// already have occurred before an unacknowledged write; it is not zero work.
#[derive(Clone, Debug)]
pub enum FileGenerationState {
    Pending(FileGenerationCommand),
    Recorded(FileGenerationReceipt),
}

/// Historical generation status and its enclosing (possibly later) canonical
/// cut. No live source, clock, key, model owner or implicit resume is returned.
#[derive(Clone, Debug)]
pub struct FileGenerationSnapshot {
    pub generation: FileGenerationState,
    pub publication: FileDeliverySnapshot,
    pub numerical: FileDecoderInspection,
}

impl FileOversight {
    /// Pin the exact independently supplied model/configuration BEFORE numerical
    /// replay. Check the ENTIRE canonical journal, not an untrusted saved receipt.
    /// This uses no writer lock, cleanup, fence, current-time claim or disk write.
    /// A missing ID means absent in this cut, not proof that no computation ran.
    /// Operator file authenticity and independent rollback floors remain required.
    pub fn read_decoder_generation(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileDecoderConfig, id: u64) -> Result<FileGenerationSnapshot, JournalError>
    {
        if id == 0 { return Err(Error::InvalidInput.into()); }
        profile.delivery.limits.check()?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let events = journal::decode(profile, &identity, &bytes)?;
        let mut configs = events.iter().filter_map(|event| match event {
            Event::Decoder(DecoderEvent::Enable(config)) => Some(config.as_ref()),
            _ => None,
        });
        if configs.next() != Some(expected) || configs.next().is_some() { return Err(Error::Binding.into()); }
        let machine = Machine::replay(profile, &events)?;
        let generation = if let Some(receipt) = machine.recorded_decoder_generation(id) {
            FileGenerationState::Recorded(receipt.clone())
        } else if let Some(command) = machine.pending_decoder_generation().filter(|command| command.id() == id) {
            FileGenerationState::Pending(command.clone())
        } else { return Err(Error::Missing.into()); };
        Ok(FileGenerationSnapshot { generation, publication: machine.snapshot(events.len()),
            numerical: FileDecoderInspection { journal_revision: events.len() as u64,
                paused: machine.decoder_paused(), numerical: machine.broker.hosted_decoder()? } })
    }
}
