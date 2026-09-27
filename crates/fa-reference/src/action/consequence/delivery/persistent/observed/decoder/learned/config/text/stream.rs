//! Original learned text release survives the existing journal replay and fence.
//! No new audience ledger, restored permission, or saved-output import is added.
use super::{ByteBpe, Configuration, DecoderEvent, DecoderModel, Event, FileHumanReviewer,
    FileLearnedConfig, FileOversight, FileOversightProfile, JournalError,
    LearnedDecoderBindingLimits, LearnedEvent, LearnedTextConfig, Machine, Writer,
    DOMAIN, MAX_CONFIG_BYTES, journal, storage};
use crate::action::{ElapsedTick, FrozenAction};
use crate::action::consequence::delivery::{stream::StreamProfile, persistent::observed::stream::{
    FileStreamSnapshot, write_profile,
}};
use crate::action::consequence::oversight::learned_host::text::stream::Release;
use crate::{Error, ReadWitness, Snapshot};
use std::path::Path;

impl FileLearnedConfig {
    /// Pin the complete original text recipe AND independent stream contract.
    /// The stream domain differs from every legacy numeric/raw-text recipe;
    /// those encodings are unchanged. Configuration does no token inference and
    /// trusts no tokenizer, weight, monitor, or output supplied by the journal.
    pub fn new_text_stream(model: DecoderModel, tokenizer: ByteBpe, source: LearnedTextConfig,
        limits: LearnedDecoderBindingLimits, stream: StreamProfile) -> Result<Self, Error>
    {
        if source.output.max_bytes > stream.max_message_bytes()
            || source.output.max_bytes > stream.max_stream_bytes() { return Err(Error::Limit); }
        let mut config = Self::new_text(model, tokenizer, source, limits)?;
        let mut w = Writer::new(MAX_CONFIG_BYTES);
        w.raw(DOMAIN)?; w.u64(0)?; w.raw(b"FALSTRM\x01")?;
        w.blob(config.bytes().as_ref())?;
        write_profile(&mut w, stream)?;
        config.bytes = w.finish().into();
        config.text.as_mut().expect("admitted original text recipe").stream = Some(stream);
        Ok(config)
    }

    pub fn text_stream_profile(&self) -> Option<StreamProfile> {
        self.text.as_ref().and_then(|text| text.stream)
    }
}

impl FileOversight {
    /// The FIRST canonical image contains BOTH original bootstrap operations.
    /// There is no acknowledged stream-only or unconstrained-generation phase.
    /// Original replay validates the complete pair before storage is created.
    pub fn create_with_learned_text_stream(directory: impl AsRef<Path>, profile: FileOversightProfile,
        config: FileLearnedConfig) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        let stream = config.text_stream_profile().ok_or(Error::Binding)?;
        let events = vec![Event::StreamBootstrap(stream),
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Enable(Configuration::new(config))))];
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory.as_ref())?;
        store.replace(&journal::encode(&profile, store.identity(), &events)?)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    /// Uses original exact recipe matching, numerical/authority replay, storage
    /// cleanup and recovery fencing. The pinned recipe's installer also checks
    /// the actual StreamBootstrap profile BEFORE replaying any numerical step.
    /// Reconstructed generation remains paused until fresh time and explicit resume.
    pub fn open_with_learned_text_stream(directory: impl AsRef<Path>, profile: FileOversightProfile,
        expected: &FileLearnedConfig) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        expected.text_stream_profile().ok_or(Error::Binding)?;
        Self::open_with_learned_generation(directory, profile, expected)
    }

    pub fn learned_text_stream_required(&self) -> bool {
        self.machine.learned_contract().is_some_and(|config| config.text_stream_profile().is_some())
    }

    /// Historical confirmed AND published cuts, including any unresolved send.
    /// This does not acquire a writer, clean up staging files, fence, observe a
    /// clock, resume generation, or return an executable owner/approval key.
    pub fn read_stream_publication_with_learned_generation(directory: impl AsRef<Path>,
        profile: &FileOversightProfile, expected: &FileLearnedConfig)
        -> Result<FileStreamSnapshot, JournalError>
    {
        let stream = expected.text_stream_profile().ok_or(Error::Binding)?;
        crate::action::consequence::delivery::persistent::codec::validate_profile(&profile.delivery)?;
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.delivery.limits.bytes)?;
        let mut events = journal::decode(profile, &identity, &bytes)?;
        if !matches!(events.first(), Some(Event::StreamBootstrap(actual)) if *actual == stream) {
            return Err(Error::Binding.into());
        }
        super::super::super::bind_history(&mut events, expected)?;
        Ok(Machine::replay(profile, &events)?.stream_snapshot(events.len())?)
    }

    /// Derive the ORIGINAL complete cumulative frame from acknowledged model
    /// output, never caller-selected content. The existing durable propose path
    /// enforces all source/currentness/witness/authority checks and records it.
    pub fn propose_learned_text_stream_message(&mut self, revision: u64, attempt: u64,
        deadline: ElapsedTick, required_witnesses: Vec<ReadWitness>, snapshot: Snapshot)
        -> Result<FrozenAction, JournalError>
    {
        self.propose_learned_stream(revision, attempt, Release::Message,
            deadline, required_witnesses, snapshot)
    }

    /// A separately reviewed finish is available only after an ORIGINAL receipt
    /// confirms this generation's message. It adds no generation step or sampler
    /// draw; ordinary journal replay still recomputes historical inference.
    /// An unknown append/finish remains an original reconciliation obligation.
    pub fn propose_learned_text_stream_finish(&mut self, revision: u64, attempt: u64,
        deadline: ElapsedTick, required_witnesses: Vec<ReadWitness>, snapshot: Snapshot)
        -> Result<FrozenAction, JournalError>
    {
        self.propose_learned_stream(revision, attempt, Release::Finish,
            deadline, required_witnesses, snapshot)
    }

    fn propose_learned_stream(&mut self, revision: u64, attempt: u64, release: Release,
        deadline: ElapsedTick, required_witnesses: Vec<ReadWitness>, snapshot: Snapshot)
        -> Result<FrozenAction, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        if !self.learned_text_stream_required() { return Err(Error::Binding.into()); }
        let current = self.learned_generation_inspection()?;
        if !self.clock_ready() || current.paused || current.pending.is_some() {
            return Err(Error::Incomplete.into());
        }
        // This is a crate-private READ of the original broker builder. Do not
        // mutate its authority behind the journal or reproduce frame logic here.
        let mut spec = self.machine.broker.learned_stream_spec(release, deadline)?;
        spec.required_witnesses = required_witnesses;
        self.propose(revision, attempt, spec, snapshot)
    }
}
