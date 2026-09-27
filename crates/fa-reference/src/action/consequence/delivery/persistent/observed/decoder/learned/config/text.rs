//! Exact text recipe binding within the original learned-generation journal.
//! Disk carries comparison bytes, never a tokenizer/model or generated output to install.
use super::super::{Configuration, DecoderEvent, LearnedEvent, Event, Machine, journal, storage};
use super::{FileLearnedConfig, LearnedDecoderBindingLimits, LearnedSourceConfig, DecoderModel,
    Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::action::{ActionSpec, FrozenAction, VERSION};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use crate::action::consequence::delivery::persistent::{JournalError,
    observed::{FileOversight, FileOversightProfile, FileHumanReviewer}};
use crate::action::consequence::oversight::{learned_host::text::LearnedTextTarget,
    learned_source::{LearnedEvidenceLimits, text::{LearnedTextConfig, LearnedTextCompletion, LearnedTextMessage}}};
use crate::{Error, Snapshot};
use std::path::Path;

#[derive(Clone)]
pub(super) struct TextRecipe {
    pub(super) tokenizer: ByteBpe,
    pub(super) source: LearnedTextConfig,
}

impl FileLearnedConfig {
    /// Construct from independently selected original model/tokenizer/monitor
    /// objects, not from the journal. Native tokenization and original constructor
    /// admission run here; no prompt or sample inference occurs. All text choices
    /// are frozen before the existing durable Enable operation can install them.
    pub fn new_text(model: DecoderModel, tokenizer: ByteBpe, source: LearnedTextConfig,
        limits: LearnedDecoderBindingLimits) -> Result<Self, Error>
    {
        let admitted = model.observed_learned_text_generation(tokenizer.clone(), source.clone())?;
        let numeric = LearnedSourceConfig { stream: source.stream,
            evaluation_origin: source.evaluation_origin, monitor_generation: source.monitor_generation,
            spec: admitted.original_generation().spec().clone(), policy: source.policy.clone(),
            budget: source.budget, telemetry: source.telemetry };
        let mut config = Self::new(model, numeric, limits)?;
        let tokenizer_bytes = tokenizer.to_bytes()?;
        let mut w = Writer::new(MAX_CONFIG_BYTES);
        w.raw(DOMAIN)?;
        // Legacy numeric recipes require a NONZERO monitor generation here.
        // Zero plus this versioned domain is therefore disjoint from every old
        // valid recipe; all legacy bytes and outer journal event tags stay intact.
        w.u64(0)?; w.raw(b"FALTEXT\x01")?;
        w.blob(config.bytes().as_ref())?;
        w.blob(&tokenizer_bytes)?;
        w.blob(source.prompt.as_bytes())?;
        for value in [source.tokenization.input_bytes, source.tokenization.pair_lookups,
            source.tokenization.heap_pops, source.output.max_bytes] { w.count(value)?; }
        w.u8(match source.output.completion {
            LearnedTextCompletion::StopRequired => 0,
            LearnedTextCompletion::StopOrTokenLimit => 1,
        })?;
        config.bytes = w.finish().into();
        config.text = Some(TextRecipe { tokenizer, source });
        Ok(config)
    }

    /// Configuration kind, not a statement that generation or recovery completed.
    pub fn is_text(&self) -> bool { self.text.is_some() }
}

impl FileOversight {
    /// The FIRST canonical image already contains the mandatory text recipe.
    /// Validate through original replay before creating storage; there is no
    /// visible empty/generic intermediate bootstrap. No clock or token is invented.
    pub fn create_with_learned_text(directory: impl AsRef<Path>, profile: FileOversightProfile,
        config: FileLearnedConfig) -> Result<(Self, FileHumanReviewer), JournalError>
    {
        if !config.is_text() { return Err(Error::Binding.into()); }
        let events = vec![Event::Decoder(DecoderEvent::Learned(
            LearnedEvent::Enable(Configuration::new(config))))];
        let machine = Machine::replay(&profile, &events)?;
        let store = storage::Store::create(directory.as_ref())?;
        let bytes = journal::encode(&profile, store.identity(), &events)?;
        store.replace(&bytes)?;
        Ok(Self::owner(profile, store, events, machine))
    }

    pub fn learned_text_required(&self) -> bool {
        self.machine.learned_contract().is_some_and(FileLearnedConfig::is_text)
    }

    /// Only acknowledged, resumed original output. The same recovery pause and
    /// pending-intent barrier that block new effects also block this live capture.
    /// Historical publication reads use the original replay projection; they do
    /// not imply that its reconstructed source is live or presently eligible.
    pub fn learned_text_message(&self, limits: LearnedEvidenceLimits)
        -> Result<LearnedTextMessage, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if !self.learned_text_required() { return Err(Error::Binding.into()); }
        let current = self.learned_generation_inspection()?;
        if !self.clock_ready() || current.paused || current.pending.is_some() {
            return Err(Error::Incomplete.into());
        }
        Ok(self.machine.broker.hosted_learned_text_message(limits)?)
    }

    /// Derive payload from the original completed model output and transact via
    /// the existing durable proposal path. Generic propose is constrained by the
    /// SAME broker text gate after every replay. No alternate permit or write path.
    pub fn propose_learned_text(&mut self, revision: u64, attempt: u64,
        target: LearnedTextTarget, snapshot: Snapshot) -> Result<FrozenAction, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let message = self.learned_text_message(LearnedEvidenceLimits::default())?;
        self.propose(revision, attempt, ActionSpec { version: VERSION,
            scope: self.profile.delivery.scope, target: Some(target.target),
            payload: message.bytes().to_vec(), required_witnesses: target.required_witnesses,
            policy_epoch: target.policy_epoch, deadline: target.deadline, units: target.units }, snapshot)
    }
}
