//! Validate conservative batch reservations against the ORIGINAL client state.
//! No response, ticket, proposal or new lifetime allowance is manufactured here.
use super::{ActorClientState, ClientError, ClientIoWork, ResponseError};
use super::super::{CLIENT_IO_CHUNK, MAX_RESPONSE_BYTES, drive::ClientDriveBudget};

impl ActorClientState {
    pub(in crate::action::consequence) fn check_recovery_drive_reservation(&self,
        ceiling: ClientIoWork, budget: ClientDriveBudget) -> Result<(), ClientError>
    {
        if budget.steps() == 1 { return self.check_recovery_reservation(ceiling); }
        let invalid = || ClientError::Response(ResponseError::Binding);
        let work = self.work(); let limits = self.limits().io;
        // Reuse original interrupted-command and baseline representation checks.
        self.check_recovery_reservation(work)?;
        if ceiling.exchanges != work.exchanges { return Err(invalid()); }
        for (next, old, maximum) in [
            (ceiling.calls, work.calls, limits.calls),
            (ceiling.written_bytes, work.written_bytes, limits.written_bytes),
            (ceiling.read_bytes, work.read_bytes, limits.read_bytes),
        ] { if next < old || next > maximum { return Err(invalid()); } }
        let calls = ceiling.calls - work.calls;
        let written = ceiling.written_bytes - work.written_bytes;
        let read = ceiling.read_bytes - work.read_bytes;
        if calls > 2 * budget.steps()
            || written > CLIENT_IO_CHUNK as u64 * budget.steps()
            || read > MAX_RESPONSE_BYTES as u64 + 1
            || (calls == 0 && (written != 0 || read != 0))
        { return Err(invalid()); }
        Ok(())
    }

    /// An interrupted batch spends its entire saved ceiling, exactly once as an
    /// absolute counter floor. Reopening the same image cannot refill anything.
    pub(in crate::action::consequence) fn consume_recovery_drive_reservation(&mut self,
        ceiling: ClientIoWork, budget: ClientDriveBudget) -> Result<(), ClientError>
    {
        self.check_recovery_drive_reservation(ceiling, budget)?;
        self.budget.work = ceiling;
        Ok(())
    }
}
