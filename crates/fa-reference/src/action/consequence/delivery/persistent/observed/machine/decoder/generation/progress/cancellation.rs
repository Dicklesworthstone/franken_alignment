//! Terminate the original cursor without advancing it or manufacturing a vote.
use super::{Machine, Transition, FileGenerationReceipt, GenerationReport, check_progress_key};
use crate::Error;
use std::rc::Rc;

impl Machine {
    pub(in super::super::super) fn apply_decoder_generation_cancel(&mut self,
        id: u64, revision: u64) -> Result<Transition, Error>
    {
        check_progress_key(id, revision)?;
        let state = self.decoder.as_ref().ok_or(Error::Incomplete)?;
        let history = &state.generations;
        // Duplicate events are not a replay shortcut. Idempotent public retries
        // return the old receipt WITHOUT appending another cancellation event.
        if history.records.contains_key(&id) { return Err(Error::Duplicate); }
        let command = history.pending.as_ref().ok_or(Error::Missing)?;
        if command.id() != id { return Err(Error::Binding); }
        if history.progress_revisions.get(&id).copied().unwrap_or(0) != revision {
            return Err(Error::Stale);
        }
        if revision >= command.check()? as u64 { return Err(Error::Limit); }
        let actual = self.broker.hosted_decoder()?;
        let progress = if let Some(state) = &history.progress {
            if actual.actor_revision != state.actor_revision || actual.position != state.cursor.position() {
                return Err(Error::Stale);
            }
            Some(state.cursor.progress())
        } else {
            if revision != 0 || actual.actor_revision != command.actor_revision()
                || actual.position != command.position() { return Err(Error::Stale); }
            None
        };
        let report = GenerationReport::cancelled(command.position(), command.request(), progress.as_ref())?;
        let command = Rc::clone(command);
        let next = revision.checked_add(1).ok_or(Error::Overflow)?;
        // The existing receipt book retains the FULL requested-step reservation.
        // Only the pending cursor is retired; no numerical or effect ledger rolls
        // back, and none of the broker's actual monitoring latches are touched.
        self.retain_generation(FileGenerationReceipt::recorded(command, Ok(report)))?;
        self.decoder.as_mut().ok_or(Error::Incomplete)?.generations.progress_revisions.insert(id, next);
        self.pause_decoder();
        self.clock_ready = false;
        Ok(Transition::Unit)
    }
}
