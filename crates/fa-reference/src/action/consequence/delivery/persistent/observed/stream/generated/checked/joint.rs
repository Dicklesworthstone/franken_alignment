//! Native checked publication with independently pinned joint-helper policy.
//! The original generated/witness bootstrap and held-out reducer own semantics.
use super::{BaseEvent, ByteBpe, DecoderEvent, Event, FileDecoderConfig, FileHumanReviewer,
    FileOversight, FileOversightProfile, GeneratedPublicationProfile, JournalError, Machine,
    MAX_FILE_TOKENIZER_BYTES, Path, Rc, journal, replay_text_events, storage};
use crate::action::consequence::delivery::persistent::observed::credibility::{
    CredibilityEvent, held_out_joint::HeldOutJointPolicy,
};
use crate::Error;

fn check_joint(events: &[Event], expected: HeldOutJointPolicy) -> Result<(), Error> {
    let actual = events.iter().filter_map(|event| match event {
        Event::Credibility(CredibilityEvent::EnableHeldOutJoint(policy)) => Some(*policy),
        _ => None,
    });
    if !actual.eq(std::iter::once(expected)) { return Err(Error::Binding); }
    Ok(())
}

impl FileOversight {
    /// Commit native message provenance, stream/reserve, publication witnesses,
    /// optional feed, joint qualification, decoder and tokenizer in ONE image.
    /// No tokens, source observations or approvals are supplied by this call.
    ///
    /// Joint policy uses the original held-out gate: baseline work can proceed,
    /// but later credibility promotion must pass its independent joint evidence
    /// requirements. Model generation is not helper qualification or permission.
    /// Invalid configuration fails native replay before storage is created.
    pub fn create_generated_text_stream_with_joint_publication(directory: impl AsRef<Path>,
        profile: FileOversightProfile, decoder: FileDecoderConfig, tokenizer: ByteBpe,
        selected: GeneratedPublicationProfile, joint: HeldOutJointPolicy)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let mut events = vec![Event::GeneratedStreamBootstrap(selected.stream)];
        if let Some(reserve) = selected.reserve {
            events.push(Event::Core(BaseEvent::ReserveRecovery(reserve)));
        }
        events.push(Event::Credibility(CredibilityEvent::EnableHeldOutJoint(joint)));
        events.extend(selected.witnesses().into_iter().map(Event::PublicationWitness));
        events.extend([Event::Decoder(DecoderEvent::Enable(Rc::new(decoder))),
            Event::Decoder(DecoderEvent::Tokenizer(canonical.into()))]);
        if events.len() > profile.delivery.limits.events { return Err(Error::Limit.into()); }
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&journal::encode(&profile, store.identity(), &events)?)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    /// Pin BOTH original native/witness configuration and exactly one joint
    /// policy before numerical replay, cleanup, role issuance or the recovery
    /// fence. An absent or changed joint gate cannot be installed at recovery.
    ///
    /// The original fence still pauses inference, withdraws previous sendable
    /// keys and invalidates old active credibility. Token/RNG/budget progress and
    /// historical results survive without becoming current evidence or approval.
    /// No source is read, no helper runs, and no message is published by opening.
    /// This does not provide an independent anti-rollback anchor or OS isolation.
    pub fn open_generated_text_stream_with_joint_publication(directory: impl AsRef<Path>,
        profile: FileOversightProfile, decoder: &FileDecoderConfig, tokenizer: &ByteBpe,
        selected: GeneratedPublicationProfile, joint: HeldOutJointPolicy)
        -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        selected.check(&events)?;
        check_joint(&events, joint)?;
        let machine = replay_text_events(&profile, decoder, &canonical, &events)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }

    /// Configuration equality only. Call before preparation on an already owned
    /// host; it cannot certify current source or held-out evaluation evidence.
    pub fn check_generated_text_joint_publication(&self,
        selected: GeneratedPublicationProfile, joint: HeldOutJointPolicy)
        -> Result<(), JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        selected.check(&self.events)?;
        check_joint(&self.events, joint).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
