//! Independently require a native-message-only stream while retaining the
//! original base guarded/text/history recovery checks and role custody.
use super::{ByteBpe, Error, Event, FileHistoryAnchor, FileOversight, FileOversightProfile,
    FileOversightRoles, FileRecoveryRequirements, JournalError, Machine, Path,
    open_text_store, storage, tokenizer_bytes};

impl FileOversight {
    /// Recover only a stream bootstrapped with mandatory native-text provenance.
    /// Require the exact stream, model, tokenizer, guards, policy, credential
    /// epoch and independently retained history anchor. Even a valid legacy
    /// caller-text stream with its own matching anchor is NOT this contract.
    ///
    /// Check mode before numerical replay. Reuse the original anchored text
    /// loader and base guard validator, with one recovery fence and no role
    /// returned before its storage acknowledgment. Saved approvals, clock and
    /// decoder readiness remain withdrawn; history, spent work and the source
    /// requirement survive. This does not resume inference or resend a message.
    ///
    /// This base entry point refuses additional evaluation/prediction/topology
    /// contracts just like open_guarded_text_anchored. Their existing composed
    /// anchored openers preserve this mode when their retained anchor covers it;
    /// they do not independently assert this additional bootstrap requirement.
    pub fn open_generated_text_stream_anchored(directory: impl AsRef<Path>,
        profile: FileOversightProfile, expected: &FileRecoveryRequirements,
        tokenizer: &ByteBpe, anchor: &FileHistoryAnchor)
        -> Result<(Self, FileOversightRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        expected.guards.stream.ok_or(Error::Incomplete)?;
        let canonical = tokenizer_bytes(expected, tokenizer)?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_generated_text_stream_store(store, profile, expected, &canonical, anchor)
    }

    fn open_generated_text_stream_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, canonical: &[u8], anchor: &FileHistoryAnchor)
        -> Result<(Self, FileOversightRoles), JournalError>
    {
        let stream = expected.guards.stream.ok_or(Error::Incomplete)?;
        let config = expected.guards.decoder.as_ref().ok_or(Error::Incomplete)?;
        let (host, human) = open_text_store(store, profile, config, canonical, anchor, |profile, events| {
            if !matches!(events.first(), Some(Event::GeneratedStreamBootstrap(actual)) if *actual == stream) {
                return Err(Error::Binding.into());
            }
            expected.guards.check_decoder_config(events)?;
            let machine = Machine::replay(profile, events)?;
            expected.check(profile, &machine, events)?;
            Ok(machine)
        })?;
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }
}

#[cfg(test)]
mod tests;
