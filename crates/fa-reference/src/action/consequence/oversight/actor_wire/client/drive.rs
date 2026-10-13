//! Bounded cooperative quanta over the ORIGINAL client step implementation.
//! No new exchange, parser, retry policy, counter ledger or executor lives here.
use super::{ActorClient, ClientError, ClientIoWork, ClientPhase, ClientProgress,
    CLIENT_IO_CHUNK, MAX_RESPONSE_BYTES};
use std::io::{Read, Write};

pub const MAX_CLIENT_DRIVE_STEPS: u64 = 64;

/// A scheduling quantum, not a fresh lifetime allowance. Each native step does
/// at most one read or write, plus its final flush. Generic I/O must already be
/// bounded/nonblocking; this is not a wall-clock deadline or preemptive timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientDriveBudget { steps: u64 }
impl ClientDriveBudget {
    pub fn new(steps: u64) -> Result<Self, ClientError> {
        if steps == 0 || steps > MAX_CLIENT_DRIVE_STEPS { return Err(ClientError::Limit); }
        Ok(Self { steps })
    }
    pub fn steps(self) -> u64 { self.steps }
}
impl Default for ClientDriveBudget {
    fn default() -> Self { Self { steps: 16 } }
}

/// Actual steps and original counter differences in this quantum. Progress at
/// the quantum boundary is NOT an acknowledgment. A response is still only the
/// server's observation, never permission or proof of connection authenticity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientDriveReport {
    pub progress: ClientProgress,
    pub steps: u64,
    pub work: ClientIoWork,
}
impl<S: Read + Write> ActorClient<S> {
    /// Advance only the already-prepared exchange. Stop at the FIRST response,
    /// transient I/O refusal, error, or quantum boundary. Never poll, reconnect,
    /// admit another command, reroll an interrupted call, or read past a response.
    /// On error, the original state retains its spent work and failure record.
    pub fn drive(&mut self, budget: ClientDriveBudget) -> Result<ClientDriveReport, ClientError> {
        let before = self.state().work();
        if self.phase().is_none() {
            return self.step().map(|progress| ClientDriveReport {
                progress, steps: 0, work: ClientIoWork::default(),
            });
        }
        for steps in 1..=budget.steps {
            let progress = self.step()?;
            if progress != ClientProgress::Progress || steps == budget.steps {
                let after = self.state().work();
                return Ok(ClientDriveReport { progress, steps, work: ClientIoWork {
                    exchanges: after.exchanges - before.exchanges,
                    calls: after.calls - before.calls,
                    written_bytes: after.written_bytes - before.written_bytes,
                    read_bytes: after.read_bytes - before.read_bytes,
                } });
            }
        }
        unreachable!("a validated drive has at least one step")
    }
}
impl<S> ActorClient<S> {
    /// Absolute upper bounds on this ONE exchange's next quantum. This grants
    /// nothing and does no I/O. Durable adapters must record the bound before
    /// drive, and release unused reservation only after recording actual results.
    /// Two calls per step conservatively covers the final write AND flush.
    pub fn drive_ceiling(&self,
        budget: ClientDriveBudget) -> Result<Option<ClientIoWork>, ClientError>
    {
        // Preserve the original interruption check and exact one-step bound.
        let Some(single) = self.recovery_step_ceiling()? else { return Ok(None); };
        if budget.steps == 1 { return Ok(Some(single)); }
        let work = self.state().work();
        let limits = self.state().limits().io;
        let mut ceiling = work;
        if matches!(self.phase(), Some(ClientPhase::Sending | ClientPhase::Receiving)) {
            ceiling.calls += (2 * budget.steps).min(limits.calls - work.calls);
            if ceiling.calls != work.calls {
                if self.phase() == Some(ClientPhase::Sending) {
                    ceiling.written_bytes += (CLIENT_IO_CHUNK as u64 * budget.steps)
                        .min(limits.written_bytes - work.written_bytes);
                }
                // A drive stops on the first response/error. Across fragmented
                // reads the original buffer accepts at most MAX_RESPONSE_BYTES
                // plus the newline/overflow byte, not this much on every step.
                ceiling.read_bytes += (MAX_RESPONSE_BYTES as u64 + 1)
                    .min(limits.read_bytes - work.read_bytes);
            }
        }
        Ok(Some(ceiling))
    }
}

#[cfg(test)]
mod tests;
