//! Install the mandatory file-source contract with the native publication owner.
//! This records configuration, never a fabricated capture, clock or permission.
use super::{
    BaseEvent, ByteBpe, DecoderEvent, Event, FileDecoderConfig, FileHumanReviewer, FileOversight,
    FileOversightProfile, GeneratedPublicationProfile, JournalError, MAX_FILE_TOKENIZER_BYTES,
    Machine, Path, Rc, RecoveryReserve, StreamProfile, journal, storage,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::{
    credibility::{CredibilityEvent, held_out_joint::HeldOutJointPolicy},
    governance::campaigns::{CampaignEvent, FilePolicyGovernor},
    guarded::FileCampaignRequirement,
    guarded::anchored::FileHistoryAnchor,
    source::{FileSourcePolicy, SourceEvent},
};
use crate::action::consequence::delivery::persistent::recovery_capacity::{Admission, Class};

pub mod evaluation;
mod recovery;
pub use recovery::{
    FileGeneratedSourceRecovery, FileGeneratedSourceRecoveryProgress,
    FileGeneratedSourceRecoveryStatus,
};

/// The complete publication selection, not a flag that can drop a joint policy.
/// All variants retain native provenance, whole-input review and both keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratedSourcePublication {
    Stream {
        stream: StreamProfile,
        reserve: Option<RecoveryReserve>,
    },
    Checked(GeneratedPublicationProfile),
    Joint {
        publication: GeneratedPublicationProfile,
        policy: HeldOutJointPolicy,
    },
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
        match self {
            Self::Joint { policy, .. } => Some(policy),
            _ => None,
        }
    }
}

impl GeneratedSourceProfile {
    fn events(self, decoder: FileDecoderConfig, tokenizer: &ByteBpe) -> Result<Vec<Event>, Error> {
        if !tokenizer.binds(decoder.profile()) {
            return Err(Error::Binding);
        }
        let canonical = tokenizer.to_bytes()?;
        if canonical.len() > MAX_FILE_TOKENIZER_BYTES {
            return Err(Error::Limit);
        }
        let mut events = vec![Event::GeneratedStreamBootstrap(self.publication.stream())];
        if let Some(reserve) = self.publication.reserve() {
            events.push(Event::Core(BaseEvent::ReserveRecovery(reserve)));
        }
        if let Some(policy) = self.publication.joint() {
            events.push(Event::Credibility(CredibilityEvent::EnableHeldOutJoint(
                policy,
            )));
        }
        if let Some(publication) = self.publication.witnesses() {
            events.extend(
                publication
                    .witnesses()
                    .into_iter()
                    .map(Event::PublicationWitness),
            );
        }
        events.extend([
            Event::Decoder(DecoderEvent::Enable(Rc::new(decoder))),
            Event::Decoder(DecoderEvent::Tokenizer(canonical.into())),
            Event::Source(SourceEvent::Enable(self.source)),
        ]);
        Ok(events)
    }

    // Only compare recorded configuration inputs here. Replay still validates
    // every event, predecessor and authority change; a selected generation is
    // never imported as state. This accepts compatible older two-write creates.
    fn check(self, events: &[Event]) -> Result<(), Error> {
        if !matches!(events.first(), Some(Event::GeneratedStreamBootstrap(p))
            if *p == self.publication.stream())
        {
            return Err(Error::Binding);
        }
        let reserves = events.iter().filter_map(|event| match event {
            Event::Core(BaseEvent::ReserveRecovery(p)) => Some(*p),
            _ => None,
        });
        if !reserves.eq(self.publication.reserve()) {
            return Err(Error::Binding);
        }
        match self.publication.witnesses() {
            Some(publication) => publication.check(events)?,
            None => {
                if events.iter().any(|event| {
                    matches!(event,
                    Event::PublicationWitness(witness) if witness.bootstrap())
                }) {
                    return Err(Error::Binding);
                }
            }
        }
        let joints = events.iter().filter_map(|event| match event {
            Event::Credibility(CredibilityEvent::EnableHeldOutJoint(p)) => Some(*p),
            _ => None,
        });
        if !joints.eq(self.publication.joint()) {
            return Err(Error::Binding);
        }
        let mut sources = events.iter().filter_map(|event| match event {
            Event::Source(SourceEvent::Enable(p)) => Some(*p),
            _ => None,
        });
        let mut source = sources.next().ok_or(Error::Binding)?;
        if sources.next().is_some() {
            return Err(Error::Binding);
        }
        // Replacement changes only the generation, not scope, limits or lease.
        // Use its requested generation for early mismatch rejection. The actual
        // validity/order of those requests is checked by Machine::replay below.
        if let Some(next) = events.iter().rev().find_map(|event| match event {
            Event::Source(SourceEvent::Replace(request)) => Some(request.next_generation),
            _ => None,
        }) {
            source.source.generation = next;
        }
        if source != self.source {
            return Err(Error::Binding);
        }
        Ok(())
    }

    fn check_machine(self, machine: &Machine) -> Result<(), Error> {
        if !machine.generated_text_only
            || machine.file_source_status().map(|status| status.policy) != Some(self.source)
            || machine.broker.held_out_joint_policy() != self.publication.joint()
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
}

// Shared initial-commit boundary. Tests inject failures into the ORIGINAL Store,
// not a replacement filesystem or an externally supplied publication callback.
enum InitialStore<'a> {
    Create(&'a Path),
    #[cfg(test)]
    Injected(storage::Store),
}

struct Prepared {
    profile: FileOversightProfile,
    events: Vec<Event>,
    machine: Machine,
}
impl Prepared {
    fn new(
        profile: FileOversightProfile,
        decoder: FileDecoderConfig,
        tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile,
    ) -> Result<Self, JournalError> {
        Self::with_campaigns(profile, decoder, tokenizer, selected, None)
    }

    fn with_campaigns(
        profile: FileOversightProfile,
        decoder: FileDecoderConfig,
        tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile,
        campaigns: Option<FileCampaignRequirement>,
    ) -> Result<Self, JournalError> {
        profile.delivery.limits.check()?;
        let mut events = selected.events(decoder, tokenizer)?;
        if let Some(campaigns) = campaigns {
            events.push(Event::Campaign(CampaignEvent::Enable(
                campaigns.limits,
                campaigns.max_campaigns,
            )));
        }
        if events.len() > profile.delivery.limits.events {
            return Err(Error::Limit.into());
        }
        if let Some(reserve) = selected.publication.reserve() {
            // Validate the complete first image, including campaign guards,
            // before creating a directory that cannot be retried in place.
            // Canonical encoding still checks actual bytes against its path.
            Admission::new(profile.delivery.limits).record(
                Class::Install(reserve),
                events.len(),
                0,
            )?;
        }
        selected.check(&events)?;
        let machine = Machine::replay(&profile, &events)?;
        selected.check_machine(&machine)?;
        Ok(Self {
            profile,
            events,
            machine,
        })
    }
    fn publish(
        self,
        destination: InitialStore<'_>,
    ) -> Result<(FileOversight, FileHumanReviewer), JournalError> {
        let encode = |identity: &Path| journal::encode(&self.profile, identity, &self.events);
        let store = match destination {
            InitialStore::Create(directory) => storage::Store::create_with_image(directory, encode)?,
            #[cfg(test)]
            InitialStore::Injected(store) => {
                store.replace(&encode(store.identity())?)?;
                store
            }
        };
        Ok(FileOversight::owner(
            self.profile,
            store,
            self.events,
            self.machine,
        ))
    }

    fn publish_governed(
        self,
        destination: InitialStore<'_>,
    ) -> Result<(FileOversight, FileHumanReviewer, FilePolicyGovernor), JournalError> {
        // This private path cannot issue a role from an unguarded preparation.
        // The ORIGINAL reducer, not the caller's selected flag, owns the guard.
        if !self.machine.broker.policy_campaigns_required() {
            return Err(Error::Binding.into());
        }
        let (host, reviewer) = self.publish(destination)?;
        let governor = FilePolicyGovernor {
            issuer: Rc::clone(&host.issuer),
        };
        Ok((host, reviewer, governor))
    }
}

impl FileOversight {
    /// Publish native provenance, model, tokenizer, file-source requirement,
    /// optional reserve and exact witness/joint selection in ONE first image.
    /// An interrupted initial replacement cannot expose a model-only bootstrap
    /// that lacks the source required by its service. No token is computed.
    ///
    /// The existing source, numerical and authority constructors validate the
    /// configuration and exact path-bound byte cost before storage is created.
    /// The complete image must be acknowledged before issuing either role.
    /// A fresh actual source capture and clock are required before numerical
    /// admission. No source file is opened or absence promoted to evidence here.
    pub fn create_generated_text_stream_from_source(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        decoder: FileDecoderConfig,
        tokenizer: ByteBpe,
        selected: GeneratedSourceProfile,
    ) -> Result<(Self, FileHumanReviewer), JournalError> {
        Prepared::new(profile, decoder, &tokenizer, selected)?
            .publish(InitialStore::Create(directory.as_ref()))
    }

    /// Atomically create the original native stream with its independent policy
    /// campaign guard. The exact limits are validated by the original reducer
    /// alongside source, decoder, tokenizer, witnesses and terminal reserve
    /// before creating a store. The final path-bound encoding must also fit
    /// before creation. All configuration is published in a single acknowledged
    /// replacement, so a partial bootstrap cannot omit governance.
    ///
    /// The governor is separately owned and issued only AFTER acknowledgment of
    /// the complete image. It cannot publish an effect or replace human review.
    /// A policy promotion still needs a complete original-corpus campaign,
    /// independent approval and the original exact-current-input checks. This
    /// constructor performs no inference, capture, clock read or policy change.
    ///
    /// Governed recovery requires its own complete independently selected guard,
    /// policy and retained-history contracts; this creation entry point is not
    /// a live governor getter or an implicit recovery profile.
    pub fn create_generated_text_stream_from_source_governed(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        decoder: FileDecoderConfig,
        tokenizer: ByteBpe,
        selected: GeneratedSourceProfile,
        campaigns: FileCampaignRequirement,
    ) -> Result<(Self, FileHumanReviewer, FilePolicyGovernor), JournalError> {
        Prepared::with_campaigns(profile, decoder, &tokenizer, selected, Some(campaigns))?
            .publish_governed(InitialStore::Create(directory.as_ref()))
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
    /// Independently retained prefixes require the explicit anchored opener;
    /// role-isolation contracts still require their own composed recovery.
    /// This reference store does not authenticate a file or latest head.
    pub fn open_generated_text_stream_from_source(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        decoder: &FileDecoderConfig,
        tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile,
    ) -> Result<(Self, FileHumanReviewer), JournalError> {
        Self::begin_open_generated_text_stream_from_source(
            directory, profile, decoder, tokenizer, selected,
        )?
        .finish_synchronously()
    }

    /// Require an independently retained exact prefix before any original event
    /// executes. The same locked canonical image supplies the prefix comparison
    /// and full source-bound native replay; a match never skips source, decoder,
    /// tokenizer, publication or semantic checks. Only the existing finalizer
    /// may publish its one Fence and return live custody.
    ///
    /// The anchor is trusted operator input, not authentication or fresh source
    /// evidence. An unanchored suffix must still replay, and rolling back both
    /// the journal and the independently retained anchor defeats this protection.
    pub fn open_generated_text_stream_from_source_anchored(
        directory: impl AsRef<Path>,
        profile: FileOversightProfile,
        decoder: &FileDecoderConfig,
        tokenizer: &ByteBpe,
        selected: GeneratedSourceProfile,
        anchor: &FileHistoryAnchor,
    ) -> Result<(Self, FileHumanReviewer), JournalError> {
        Self::begin_open_generated_text_stream_from_source_anchored(
            directory, profile, decoder, tokenizer, selected, anchor,
        )?
        .finish_synchronously()
    }
}

#[cfg(test)]
mod tests;
