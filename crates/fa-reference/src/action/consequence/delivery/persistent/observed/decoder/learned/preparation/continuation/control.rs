//! Reuse an existing learned replay cache in the ORIGINAL control transactions.
//! No public prepared-event executor, cache constructor or second authority.
use super::{Error, FileLearnedReplayContinuation, FileOversight, JournalError, Machine, Rc};
use crate::action::consequence::delivery::persistent::observed::{Event, Transition};

impl FileOversight {
    // Call only AFTER the operation's original live preflights. Evidence-loss
    // operations deliberately poison the live owner BEFORE replay; checking its
    // fault flag here would prevent recording that loss. Do not clear the latch.
    // The original transaction still owns encoding, its new-event checks and
    // application, and its existing acknowledgment/failure boundary.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn replay_control_candidate(&mut self) -> Result<(Machine, bool), JournalError> {
        let Some(carry) = self.learned_replay.take() else {
            return Ok((Machine::replay(&self.profile, &self.events)?, false));
        };
        if !Rc::ptr_eq(&carry.issuer, &self.issuer) || carry.through > self.events.len() {
            return Err(Error::Binding.into());
        }
        let mut candidate = carry.candidate;
        for event in &self.events[carry.through..] { candidate.apply(event)?; }
        // A replay failure or unwind discards this private candidate. Never
        // retry a cold replay inside the same call or conceal witness divergence.
        Ok((candidate, true))
    }

    // Use ONLY where the live Machine represents all current acknowledged
    // events. In particular generic persist_candidate and recovery must NOT
    // call this: cooperative recovery retires an empty placeholder, not a
    // verified predecessor. A cold owner stays cold until learned generation
    // explicitly seeds its existing cache; no new per-owner slot is allocated.
    pub(in crate::action::consequence::delivery::persistent::observed)
    fn persist_control_candidate(&mut self, event: Event, bytes: Vec<u8>, candidate: Machine,
        result: Transition, retain_replay: bool) -> Result<Transition, JournalError>
    {
        if !retain_replay { return self.persist_candidate(event, bytes, candidate, result); }
        let through = self.events.len();
        let (result, retired) = self.persist_candidate_retaining(event, bytes, candidate, result)?;
        self.learned_replay = Some(FileLearnedReplayContinuation::new(
            Rc::clone(&self.issuer), through, retired));
        Ok(result)
    }
}
