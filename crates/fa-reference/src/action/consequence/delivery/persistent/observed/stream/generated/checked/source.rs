//! Install the mandatory file-source contract with the native publication owner.
//! This records configuration, never a fabricated capture, clock or permission.
use super::{BaseEvent, ByteBpe, DecoderEvent, Event, FileDecoderConfig, FileHumanReviewer,
    FileOversight, FileOversightProfile, GeneratedPublicationProfile, JournalError, Machine,
    MAX_FILE_TOKENIZER_BYTES, Path, Rc, RecoveryReserve, StreamProfile, journal,
    replay_text_events, storage};
use crate::action::consequence::delivery::persistent::observed::{
    credibility::{CredibilityEvent, held_out_joint::HeldOutJointPolicy},
    source::{FileSourcePolicy, SourceEvent},
};
use crate::Error;

/// The complete publication selection, not a flag that can drop a joint policy.
/// All variants retain native provenance, whole-input review and both keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedSourcePublication {
    Stream { stream: StreamProfile, reserve: Option<RecoveryReserve> },
    Checked(GeneratedPublicationProfile),
    Joint { publication: GeneratedPublicationProfile, policy: HeldOutJointPolicy },
}

/// Independently selected source identity, limits, lease and publication mode.
/// On recovery source.generation names the CURRENT registered generation: prior
/// replacements must still replay through the original governance reducer.
/// File paths and captures are not trusted merely because this contract matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedSourceProfile {
    pub source: FileSourcePolicy,
    pub publication: GeneratedSourcePublication,
}

impl GeneratedSourcePublication {
    fn stream(self) -> StreamProfile {
        match self {
            Self::Stream { stream, .. } => stream,
            Self::Checked(p) | Self::Joint { publication: p, .. } => p.stream,
        }
    }
    fn reserve(self) -> Option<RecoveryReserve> {
        match self {
            Self::Stream { reserve, .. } => reserve,
            Self::Checked(p) | Self::Joint { publication: p, .. } => p.reserve,
        }
    }
    fn witnesses(self) -> Option<GeneratedPublicationProfile> {
        match self {
            Self::Stream { .. } => None,
            Self::Checked(p) | Self::Joint { publication: p, .. } => Some(p),
        }
    }
    fn joint(self) -> Option<HeldOutJointPolicy> {
        match self { Self::Joint { policy, .. } => Some(policy), _ => None }
    }
}

impl GeneratedSourceProfile {
    fn events(self, decoder: FileDecoderConfig, tokenizer: &ByteBpe) -> Result<Vec<Event>, Error> {
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit); }
        let mut events = vec![Event::GeneratedStreamBootstrap(self.publication.stream())];
        if let Some(reserve) = self.publication.reserve() {
            events.push(Event::Core(BaseEvent::ReserveRecovery(reserve)));
        }
        if let Some(policy) = self.publication.joint() {
            events.push(Event::Credibility(CredibilityEvent::EnableHeldOutJoint(policy)));
        }
        if let Some(publication) = self.publication.witnesses() {
            events.extend(publication.witnesses().into_iter().map(Event::PublicationWitness));
        }
        events.extend([Event::Decoder(DecoderEvent::Enable(Rc::new(decoder))),
            Event::Decoder(DecoderEvent::Tokenizer(canonical.into())),
            Event::Source(SourceEvent::Enable(self.source))]);
        Ok(events)
    }

    // Only compare recorded configuration inputs here. Replay still validates
    // every event, predecessor and authority change; a selected generation is
    // never imported as state. This accepts compatible older two-write creates.
    fn check(self, events: &[Event]) -> Result<(), Error> {
        if !matches!(events.first(), Some(Event::GeneratedStreamBootstrap(p))
            if *p == self.publication.stream()) { return Err(Error::Binding); }
        let reserves = events.iter().filter_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(p)) => Some(*p), _ => None,
        });
        if !reserves.eq(self.publication.reserve()) { return Err(Error::Binding); }
        match self.publication.witnesses() {
            Some(publication) => publication.check(events)?,
            None => {
                if events.iter().any(|event| matches!(event,
                    Event::PublicationWitness(witness) if witness.bootstrap())) {
                    return Err(Error::Binding);
                }
            }
        }
        let joints = events.iter().filter_map(|event| match event {
            Event::Credibility(CredibilityEvent::EnableHeldOutJoint(p)) => Some(*p), _ => None,
        });
        if !joints.eq(self.publication.joint()) { return Err(Error::Binding); }
        let mut sources = events.iter().filter_map(|event| match event {
            Event::Source(SourceEvent::Enable(p)) => Some(*p), _ => None,
        });
        let mut source = sources.next().ok_or(Error::Binding)?;
        if sources.next().is_some() { return Err(Error::Binding); }
        // Replacement changes only the generation, not scope, limits or lease.
        // Use its requested generation for early mismatch rejection. The actual
        // validity/order of those requests is checked by Machine::replay below.
        if let Some(next) = events.iter().rev().find_map(|event| match event {
            Event::Source(SourceEvent::Replace(request)) => Some(request.next_generation), _ => None,
        }) { source.source.generation = next; }
        if source != self.source { return Err(Error::Binding); }
        Ok(())
    }

    fn check_machine(self, machine: &Machine) -> Result<(), Error> {
        if !machine.generated_text_only
            || machine.file_source_status().map(|status| status.policy) != Some(self.source)
            || machine.broker.held_out_joint_policy() != self.publication.joint() {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

// Shared initial-commit boundary. Tests inject failures into the ORIGINAL Store,
// not a replacement filesystem or an externally supplied publication callback.
struct Prepared {
    profile: FileOversightProfile,
    events: Vec<Event>,
    machine: Machine,
}
impl Prepared {
    fn new(profile: FileOversightProfile, decoder: FileDecoderConfig, tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile) -> Result<Self, JournalError>
    {
        profile.delivery.limits.check()?;
        let events = selected.events(decoder, tokenizer)?;
        if events.len() > profile.delivery.limits.events { return Err(Error::Limit.into()); }
        selected.check(&events)?;
        let machine = Machine::replay(&profile, &events)?;
        selected.check_machine(&machine)?;
        Ok(Self { profile, events, machine })
    }
    fn publish(self, store: storage::Store) -> Result<(FileOversight, FileHumanReviewer), JournalError> {
        let bytes = journal::encode(&self.profile, store.identity(), &self.events)?;
        store.replace(&bytes)?;
        Ok(FileOversight::owner(self.profile, store, self.events, self.machine))
    }
}

impl FileOversight {
    /// Publish native provenance, model, tokenizer, file-source requirement,
    /// optional reserve and exact witness/joint selection in ONE first image.
    /// An interrupted initial replacement cannot expose a model-only bootstrap
    /// that lacks the source required by its service. No token is computed.
    ///
    /// The existing source, numerical and authority constructors validate the
    /// configuration before storage is created. Encoding/storage can still fail;
    /// no owner or reviewer is returned before the complete image is acknowledged.
    /// A fresh actual source capture and clock are required before numerical
    /// admission. No source file is opened or absence promoted to evidence here.
    pub fn create_generated_text_stream_from_source(directory: impl AsRef<Path>,
        profile: FileOversightProfile, decoder: FileDecoderConfig, tokenizer: ByteBpe,
        selected: GeneratedSourceProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        Prepared::new(profile, decoder, &tokenizer, selected)?
            .publish(storage::Store::create(directory.as_ref())?)
    }

    /// Pin source identity/generation/limits/lease AND complete publication mode
    /// against the SAME locked image before replay, cleanup or recovery fencing.
    /// Exactly matching older journals with separately installed sources remain
    /// compatible; a missing source is rejected, never installed during recovery.
    /// Source rotations are reconstructed only by the original replacement law.
    ///
    /// The original recovery fence pauses the decoder and withdraws old source
    /// eligibility, active qualification and effect keys. It preserves recorded
    /// intent, numerical/RNG spend, publication history and unresolved liabilities.
    /// Fresh source/time and explicit resume remain mandatory for new work.
    /// Additional anchored/role-isolation contracts still need their own composed
    /// opener. This reference store does not authenticate a file or latest head.
    pub fn open_generated_text_stream_from_source(directory: impl AsRef<Path>,
        profile: FileOversightProfile, decoder: &FileDecoderConfig, tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        profile.delivery.limits.check()?;
        if !tokenizer.binds(decoder.profile()) { return Err(Error::Binding.into()); }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit.into()); }
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.delivery.limits.bytes)?;
        let events = journal::decode(&profile, store.identity(), &bytes)?;
        selected.check(&events)?;
        let machine = replay_text_events(&profile, decoder, &canonical, &events)?;
        selected.check_machine(&machine)?;
        store.confirm_and_cleanup()?;
        let (mut host, reviewer) = Self::owner(profile, store, events, machine);
        host.transact(host.revision(), Event::Core(BaseEvent::Fence))?;
        Ok((host, reviewer))
    }
}

#[cfg(test)]
mod tests;
