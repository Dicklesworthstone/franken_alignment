//! Expire a silent actor's unanswered forecast, never synthesize its outcome.
use super::OversightBroker;
use crate::action::ElapsedTick;
use crate::Error;

/// Exact timer basis retained from the original pending forecast. This is data,
/// not a role, an action observation, a fresh clock or a transferable permission.
/// Altered fields and timers for already answered forecasts refuse before loss.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsistencyDeadline {
    pub attempt: u64,
    pub actor_revision: u64,
    pub authority_epoch: u64,
    pub source_sequence: u64,
    pub created_at: ElapsedTick,
    pub expires_at: ElapsedTick,
}

impl OversightBroker {
    /// A scheduler may retain this basis and wake at expires_at. Reading does
    /// not renew the forecast or assert that the current clock is fresh.
    pub fn consistency_deadline(&self) -> Result<Option<ConsistencyDeadline>, Error> {
        let state = self.consistency.as_ref().ok_or(Error::Incomplete)?;
        Ok(state.pending.as_ref().map(|pending| ConsistencyDeadline {
            attempt: pending.attempt,
            actor_revision: pending.actor_revision,
            authority_epoch: pending.epoch,
            source_sequence: pending.prediction.observation().frame().sequence,
            created_at: pending.created_at,
            expires_at: pending.valid_until,
        }))
    }

    /// Pure admission for a proposed clock observation. It cannot set time or
    /// withdraw eligibility. Used by durable owners BEFORE entering persistence.
    pub fn check_consistency_deadline(&self, expected: ConsistencyDeadline,
        at: ElapsedTick) -> Result<bool, Error>
    {
        let actual = self.consistency_deadline()?.ok_or(Error::Missing)?;
        if actual != expected { return Err(Error::Stale); }
        let previous = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        if at < previous || at < actual.created_at { return Err(Error::Stale); }
        Ok(at >= actual.expires_at)
    }

    /// Use the ORIGINAL recorded clock, not an actor-supplied timestamp. The
    /// boundary is inclusive. Early checks do nothing; due checks retain the
    /// pending forecast and spent prediction budget, and add NO likelihood sample.
    /// The original optional stop policy chooses containment. A containment error
    /// leaves coverage lost and its incident retained, never restores eligibility.
    /// Repeating a due check cannot generate another sample or rewind any effect.
    pub fn expire_consistency_deadline(&mut self, expected: ConsistencyDeadline)
        -> Result<bool, Error>
    {
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        if !self.check_consistency_deadline(expected, now)? { return Ok(false); }
        self.consistency_unavailable()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
