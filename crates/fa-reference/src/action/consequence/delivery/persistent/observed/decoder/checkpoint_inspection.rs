//! Read-only canonical reset evidence, including after an ambiguous write.
//! No writer lock, cleanup, fresh clock, recovery fence or live role is created.
use super::{DecoderEvent, FileDecoderConfig, FileDecoderInspection, FileOversight,
    FileOversightProfile, JournalError, Machine, Event, journal, storage};
use super::super::super::FileDeliverySnapshot;
use crate::action::consequence::oversight::decoder_host::{HostedRecoveryUsage, HostedResetReceipt};
use crate::Error;
use std::path::Path;

/// The original reset result and the later enclosing canonical image are kept
/// distinct. Neither inspection nor a historical restored result resumes a run.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::FilePermit;
/// use fa_reference::action::consequence::delivery::persistent::observed::decoder::checkpoint_inspection::FileDecoderResetSnapshot;
/// fn grant(image: FileDecoderResetSnapshot) -> FilePermit { image }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDecoderResetSnapshot {
    pub operation: u64,
    pub result: Result<HostedResetReceipt, Error>,
    pub publication: FileDeliverySnapshot,
    pub numerical: FileDecoderInspection,
    pub recovery: HostedRecoveryUsage,
}

impl FileOversight {
    /// Validate the ENTIRE canonical image, pin exact decoder inputs BEFORE
    /// numerical replay, and return one original reset result. An absent operation
    /// is Missing in this image, not a receipt proving that no work occurred.
    /// Caller-owned profile/path authenticity and independent floors are still
    /// required; an internally consistent image is not an authenticated history.
    pub fn read_decoder_reset(directory: impl AsRef<Path>, profile: &FileOversightProfile,
        expected: &FileDecoderConfig, operation: u64) -> Result<FileDecoderResetSnapshot, JournalError>
    {
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
        let result = machine.decoder_reset_result(operation)?;
        Ok(FileDecoderResetSnapshot { operation, result,
            publication: machine.snapshot(events.len()),
            numerical: FileDecoderInspection { journal_revision: events.len() as u64,
                paused: machine.decoder_paused(), numerical: machine.broker.hosted_decoder()? },
            recovery: machine.broker.hosted_recovery_usage()?,
        })
    }
}
