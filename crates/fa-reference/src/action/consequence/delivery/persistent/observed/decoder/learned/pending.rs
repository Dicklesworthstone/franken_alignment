//! A fixed original recovery sequence with one acknowledged canonical cut.
//! No partial clock/resume, candidate token, effect key or refund escapes it.
use super::{DecoderEvent, Event, FileOversight, GenerationEvent, JournalError,
    JournalFailure, JournalIo, LearnedEvent, LearnedStepIntent, Machine, Transition,
    journal};
use super::super::super::BaseEvent;
use crate::action::ElapsedTick;
use crate::Error;
use std::rc::Rc;

impl FileOversight {
    /// Observe a fresh trusted tick, resume a recovered learned owner, and
    /// complete its EXACT existing numerical intent in one canonical replacement.
    /// The same three original records and numerical witness remain replayable;
    /// no new event format or alternate authority/numerical engine is introduced.
    ///
    /// This is only for a paused owner with a pending intent. New work still
    /// uses begin/complete or advance. A stale predecessor, mismatched intent,
    /// refused clock/resume or insufficient ordinary record capacity changes
    /// neither the acknowledged owner nor its canonical journal.
    ///
    /// Once original computation starts, an encoding or storage failure poisons
    /// the live owner and returns no candidate outcome. Recovery must inspect the
    /// actual old-or-new replacement. An inner numerical error, unlike an outer
    /// journal error, is an acknowledged original result with retained costs.
    /// No old effect permission, budget, unknown charge or source latch resets.
    pub fn resume_pending_learned_step_at(
        &mut self,
        revision: u64,
        actor_revision: u64,
        position: u64,
        observed_at: ElapsedTick,
    ) -> Result<Result<Rc<GenerationEvent>, Error>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        let pending = self.machine.pending_learned_step().ok_or(Error::Incomplete)?;
        if pending != (LearnedStepIntent { actor_revision, position }) {
            return Err(Error::Binding.into());
        }
        if !self.machine.learned_paused() { return Err(Error::WrongState.into()); }
        self.check_learned_event_capacity(revision, 3)?;

        let prefix = [
            Event::Core(BaseEvent::Time(observed_at)),
            Event::Decoder(DecoderEvent::Learned(LearnedEvent::Resume { actor_revision, position })),
        ];
        let shape = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Step {
            actor_revision, position, witness: Rc::from(&b""[..]),
        }));
        for event in prefix.iter().chain(std::iter::once(&shape)) {
            self.check_source_admission(event)?;
        }
        let count = self.events.len().checked_add(3).ok_or(Error::Overflow)?;
        let mut history = Vec::new();
        history.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        history.extend(self.events.iter().cloned());
        // Enforce original framing, byte ceilings and reserve admission on each
        // prefix before reconstruction or new inference. Never persist a prefix.
        for event in &prefix {
            journal::encode_appended(&self.profile, self.store.identity(), &history, event)?;
            history.push(event.clone());
        }
        let mut candidate = Machine::replay(&self.profile, &self.events)?;
        for event in &prefix {
            candidate.preflight_consistency(event)?;
            candidate.apply(event)?;
        }
        candidate.preflight_learned_step(actor_revision, position)?;
        // A caught unwind, failed witness encoding or late byte-capacity refusal
        // cannot make an older quiet prefix eligible after attempted inference.
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        let (step, result) = candidate.prepare_learned_step(actor_revision, position)?;
        let step = Event::Decoder(DecoderEvent::Learned(step));
        let bytes = journal::encode_appended(&self.profile, self.store.identity(), &history, &step)?;
        history.push(step);
        self.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: true });
        if let Err(error) = self.store.replace(&bytes) {
            if let JournalError::Io(failure) = &error { self.fault = Some(failure.clone()); }
            return Err(error);
        }
        for event in &history[self.events.len()..] {
            self.source_operation_committed(event);
        }
        self.events = history;
        self.machine = candidate;
        self.fault = None;
        match result {
            Transition::Learned(result) => Ok(result),
            _ => unreachable!("original pending learned-step transition"),
        }
    }
}
