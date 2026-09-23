//! Recover exact text history without losing the composed guarded-role contract.
//! Only the original machine replays tokens and only its fence renews ownership.

use super::{BaseEvent, Event, FileHistoryAnchor, FileOversight, FileOversightProfile,
    FileOversightRoles, FileRecoveryRequirements, JournalError, Machine, journal, storage};
use super::super::super::FileHumanReviewer;
use super::super::super::decoder::{DecoderEvent, FileDecoderConfig};
use super::super::super::decoder::text::MAX_FILE_TOKENIZER_BYTES;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use crate::Error;
use std::path::Path;

impl FileOversight {
    /// Recover a durable text owner AND the original separately held guard roles.
    /// Require an independently retained history anchor, the exact tokenizer,
    /// model/monitor/sampler, guard inventory, effective policy, credential epoch
    /// and all numeric floors. Neither the text-only nor counter-only recovery
    /// entry point provides this combined contract.
    ///
    /// Reject tokenizer substitution and divergent/rolled-back history BEFORE
    /// numerical replay. Validate the complete unanchored suffix through the
    /// original machine before any cleanup, write or returned role. The one
    /// original recovery fence pauses inference, withdraws old effect keys and
    /// source/identity eligibility, and preserves spent work and text cursors.
    /// A pending generation is not restarted or replenished by this operation.
    ///
    /// Return the existing human, identity and policy-governor roles only after
    /// acknowledged fencing. Provision them to their independent custodians;
    /// no old effect approval or credential secret is reconstructed. Fresh time
    /// and explicit native resume/guard revalidation are still required.
    ///
    /// The base guard profile rejects additional evaluation, prediction and
    /// mediation gates rather than silently omitting their roles. Storage and
    /// anchor custody remain host duties: rolling back BOTH defeats the anchor.
    /// No remote publication, authentication or production qualification follows.
    pub fn open_guarded_text_anchored(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, tokenizer: &ByteBpe, anchor: &FileHistoryAnchor)
        -> Result<(Self, FileOversightRoles), JournalError>
    {
        profile.delivery.limits.check()?;
        let canonical = tokenizer_bytes(expected, tokenizer)?;
        let store = storage::Store::open(directory.as_ref())?;
        Self::open_guarded_text_store(store, profile, expected, &canonical, anchor)
    }

    // Actual Store seam for replacement-barrier tests, not a public loader.
    fn open_guarded_text_store(store: storage::Store, profile: FileOversightProfile,
        expected: &FileRecoveryRequirements, canonical: &[u8], anchor: &FileHistoryAnchor)
        -> Result<(Self, FileOversightRoles), JournalError>
    {
        let config = expected.guards.decoder.as_ref().ok_or(Error::Incomplete)?;
        let (host, human) = open_text_store(store, profile, config, canonical, anchor, |profile, events| {
            expected.guards.check_decoder_config(events)?;
            let machine = Machine::replay(profile, events)?;
            expected.check(profile, &machine, events)?;
            Ok(machine)
        })?;
        let roles = FileOversightRoles::provision(&host, human);
        Ok((host, roles))
    }
}

// The caller's immutable tokenizer is admitted before opening a writer/reader.
// Keep the original durable-text limit rather than importing the larger asset
// loader allowance into journal recovery. No tokenizer is chosen from disk.
fn tokenizer_bytes(expected: &FileRecoveryRequirements, tokenizer: &ByteBpe)
    -> Result<Vec<u8>, JournalError>
{
    let config = expected.guards.decoder.as_ref().ok_or(Error::Incomplete)?;
    if !tokenizer.binds(config.profile()) { return Err(Error::Binding.into()); }
    let canonical = tokenizer.to_bytes()?;
    if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
    Ok(canonical)
}

// Fixed internal compositions only. This callback is never supplied by an actor
// or library caller; each entry point uses its original guard/replay validator.
// Every path decodes once, replays once, then writes exactly one original fence.
fn open_text_store<C>(store: storage::Store, profile: FileOversightProfile,
    config: &FileDecoderConfig, canonical: &[u8], anchor: &FileHistoryAnchor, check: C)
    -> Result<(FileOversight, FileHumanReviewer), JournalError>
where C: FnOnce(&FileOversightProfile, &[Event]) -> Result<Machine, JournalError> {
    let bytes = store.read(profile.delivery.limits.bytes)?;
    let events = journal::decode(&profile, store.identity(), &bytes)?;
    drop(bytes);
    anchor.check(&profile, store.identity(), &events)?;
    // Compare the ORIGINAL event inventory, including exact named-control
    // recognition bytes, not just tokenizer/model generation labels or length.
    let mut models = events.iter().filter_map(|event| match event {
        Event::Decoder(DecoderEvent::Enable(config)) => Some(config.as_ref()), _ => None,
    });
    let mut tokenizers = events.iter().filter_map(|event| match event {
        Event::Decoder(DecoderEvent::Tokenizer(bytes)) => Some(bytes.as_ref()), _ => None,
    });
    if models.next() != Some(config) || models.next().is_some()
        || tokenizers.next() != Some(canonical) || tokenizers.next().is_some()
    { return Err(Error::Binding.into()); }
    let machine = check(&profile, &events)?;
    if machine.decoder_contract() != Some(config)
        || machine.decoder_tokenizer()?.to_bytes()?.as_slice() != canonical
    { return Err(Error::Binding.into()); }
    store.confirm_and_cleanup()?;
    let (mut host, human) = FileOversight::owner(profile, store, events, machine);
    host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
    Ok((host, human))
}

#[cfg(test)]
mod tests;
