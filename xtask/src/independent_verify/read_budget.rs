//! Caller-owned work accounting for both verification input streams.

use super::{ErrorKind, Location, VerifyError};

/// Additional read attempts in the default budget, beyond one-byte reads of
/// both maximum-length inputs and their EOF probes. These can cover interrupted
/// reads; all attempts share one ceiling rather than separate per-input pools.
pub const DEFAULT_INTERRUPTED_READ_ALLOWANCE: u64 = 1024;

/// A finite allowance shared by the journal, retained prefix, and all retries
/// performed with this value. Every underlying `Read::read` invocation spends
/// one call, including interruptions, errors, and EOF probes. No outcome refunds
/// spent work. The budget is intentionally neither `Clone` nor `Copy`.
///
/// This bounds attempted calls, not elapsed time: a single caller-supplied
/// reader may block. It does not measure physical storage I/O or charge bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct ReadBudget {
    remaining_calls: u64,
    spent_calls: u64,
}

impl ReadBudget {
    /// Select a finite total allowance. Zero refuses before invoking a reader;
    /// independently invalid context still takes precedence over exhaustion.
    pub const fn new(calls: u64) -> Self {
        Self {
            remaining_calls: calls,
            spent_calls: 0,
        }
    }

    pub const fn remaining_calls(&self) -> u64 {
        self.remaining_calls
    }

    pub const fn spent_calls(&self) -> u64 {
        self.spent_calls
    }

    pub(super) fn spend(&mut self, location: Location) -> Result<(), VerifyError> {
        if self.remaining_calls == 0 {
            return Err(VerifyError::at(
                ErrorKind::ReadBudgetExhausted,
                location,
                "shared input-read allowance is exhausted; the next read was not attempted",
            ));
        }
        self.remaining_calls -= 1;
        // The immutable original allowance is at most u64::MAX. Whenever a
        // call remains, spent_calls is strictly below that allowance.
        self.spent_calls += 1;
        Ok(())
    }
}
