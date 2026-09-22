//! Bind exact text intent to the original numerical generation history.
use super::{Machine, Transition};
use super::super::super::decoder::text::{FileTextGenerationCommand,
    MAX_FILE_TEXT_INPUT_BYTES, MAX_FILE_TOKENIZER_BYTES};
use super::super::super::decoder::generation::MAX_FILE_GENERATIONS;
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::ByteBpe;
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct TextHistory {
    tokenizer: Option<ByteBpe>,
    commands: BTreeMap<u64, Rc<FileTextGenerationCommand>>,
    input_bytes: usize,
}

impl Machine {
    pub(in super::super::super) fn decoder_tokenizer(&self) -> Result<&ByteBpe, Error> {
        self.decoder.as_ref().and_then(|state| state.text.tokenizer.as_ref()).ok_or(Error::Incomplete)
    }
    pub(in super::super::super) fn decoder_text_command(&self, id: u64)
        -> Option<&Rc<FileTextGenerationCommand>>
    { self.decoder.as_ref()?.text.commands.get(&id) }

    pub(super) fn install_decoder_tokenizer(&mut self, bytes: &[u8]) -> Result<Transition, Error> {
        if bytes.is_empty() { return Err(Error::Incomplete); }
        if bytes.len() > MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit); }
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        if state.text.tokenizer.is_some() { return Err(Error::Duplicate); }
        let actual = self.broker.hosted_decoder()?;
        if actual.position != 0 || actual.sampled_draws != 0 || actual.status != MonitoringStatus::Ready
            || state.paused || self.pending_decoder_generation().is_some()
            || !self.actions.is_empty() || self.requests.len() != 0 || !self.sessions.is_empty()
            || self.broker.stop_receipt().is_some()
        { return Err(Error::WrongState); }
        let tokenizer = ByteBpe::from_bytes(state.config.profile(), bytes)?;
        // Original archive admission is strict; check exact spelling as well so
        // a future permissive reader cannot weaken the stored configuration pin.
        if tokenizer.to_bytes()?.as_slice() != bytes { return Err(Error::Binding); }
        self.decoder.as_mut().ok_or(Error::Incomplete)?.text.tokenizer = Some(tokenizer);
        Ok(Transition::Unit)
    }

    pub(in super::super::super) fn check_decoder_text_intent(&self, command: &FileTextGenerationCommand)
        -> Result<(), Error>
    {
        let bytes = command.check()?;
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        if state.text.commands.contains_key(&command.id())
            || self.recorded_decoder_generation(command.id()).is_some()
        { return Err(Error::Binding); }
        if state.text.commands.len() >= MAX_FILE_GENERATIONS
            || state.text.input_bytes.checked_add(bytes).ok_or(Error::Limit)? > MAX_FILE_TEXT_INPUT_BYTES
        { return Err(Error::Limit); }
        let (_, numerical) = command.compile(self.decoder_tokenizer()?)?;
        self.check_decoder_generation_intent(&numerical)
    }

    pub(super) fn apply_decoder_text_intent(&mut self, command: &Rc<FileTextGenerationCommand>)
        -> Result<Transition, Error>
    {
        self.check_decoder_text_intent(command)?;
        let bytes = command.check()?;
        let (_, numerical) = command.compile(self.decoder_tokenizer()?)?;
        // This is the SAME generation reservation and pending-input reducer as
        // BeginGeneration. No alternate token source, quota, cursor or result.
        self.apply_decoder_generation_intent(&Rc::new(numerical))?;
        let text = &mut self.decoder.as_mut().ok_or(Error::Incomplete)?.text;
        text.input_bytes += bytes; // checked above; generations never delete text
        text.commands.insert(command.id(), Rc::clone(command));
        Ok(Transition::Unit)
    }
}
