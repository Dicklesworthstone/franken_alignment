//! One-shot blocking Unix worker on the ORIGINAL native client. Intended for a
//! dedicated child process, never an executor thread. No listener or retry task.
//! An absolute local deadline complements, but cannot replace, supervisor cutoffs.

use super::{NativeEvaluationProgress, NativeEvaluator};
use super::peer::{NativeClientError, NativeClientProgress, NativeHelperClient};
use super::super::{ClientPhase, ClientProgress};
use crate::Error;
use std::fmt;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

pub const MAX_PROCESS_STEPS: usize = 1_000_000;
pub const MAX_PROCESS_MILLIS: u64 = 3_600_000;

/// Starts immediately, so checkpoint startup cannot renew the protocol deadline.
/// Not Clone: consuming a worker consumes this allowance. Counts are client steps,
/// not syscalls or FLOPs. Token computation and filesystem calls are not preempted.
#[derive(Debug)]
pub struct NativeProcessBudget {
    started: Instant,
    deadline: Instant,
    max_steps: usize,
}
impl NativeProcessBudget {
    pub fn new(milliseconds: u64, max_steps: usize) -> Result<Self, Error> {
        if milliseconds == 0 || max_steps == 0 { return Err(Error::InvalidInput); }
        if milliseconds > MAX_PROCESS_MILLIS || max_steps > MAX_PROCESS_STEPS {
            return Err(Error::Limit);
        }
        let started = Instant::now();
        let deadline = started.checked_add(Duration::from_millis(milliseconds)).ok_or(Error::Overflow)?;
        Ok(Self { started, deadline, max_steps })
    }
    pub fn expired(&self) -> bool { Instant::now() >= self.deadline }
    fn remaining(&self, now: Instant) -> Option<Duration> {
        self.deadline.checked_duration_since(now).filter(|duration| !duration.is_zero())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeProcessError { Contract(Error), Socket(io::ErrorKind) }
impl fmt::Display for NativeProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeProcessError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeProcessStop {
    /// The original reveal frame was sent. This is NOT congress acceptance.
    ReplySent,
    Deadline,
    StepLimit,
    Client(NativeClientError),
    Socket(io::ErrorKind),
}

/// Historical diagnostics after destroying the client and closing its sockets.
/// On deadline/error, bytes may ALREADY have reached the supervisor. Only that
/// supervisor's original transcript decides what it accepted; never resend here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeProcessReport {
    pub stop: NativeProcessStop,
    pub steps: usize,
    pub phase: ClientPhase,
    pub evaluations: usize,
    pub evaluation: NativeEvaluationProgress,
    pub elapsed: Duration,
}

/// Attach to the original launch_helpers full-duplex stdin without unsafe/raw-fd
/// ownership. A pipe or regular file refuses. No bytes are read during attachment.
pub fn inherited_worker_socket() -> Result<UnixStream, NativeProcessError> {
    let input = io::stdin();
    let descriptor = input.as_fd().try_clone_to_owned().map_err(socket_error)?;
    let socket = UnixStream::from(descriptor);
    socket.peer_addr().map_err(socket_error)?;
    Ok(socket)
}

/// Consume one independently provisioned socket, evaluator, salt and lifetime.
/// Read/write waits use the REMAINING absolute deadline, not a new per-step lease.
/// Every native token also checks it before and after computing. A slow token may
/// finish late but cannot trigger the NEXT commitment/reveal write after expiry.
/// A late write may have become visible; Deadline does not mean no bytes were sent.
///
/// No threads, sleep polling, second executor, reconnect, hidden model reroll or
/// substitute vote. On every exit the sole evaluator/client are destroyed. The
/// parent still owns process termination and its authoritative review deadlines.
pub fn run_native_worker(socket: UnixStream, evaluator: NativeEvaluator, salt: Vec<u8>,
    budget: NativeProcessBudget) -> Result<NativeProcessReport, NativeProcessError>
{
    run_with_clock(socket, evaluator, salt, budget, |_, _| Instant::now())
}

fn socket_error(error: io::Error) -> NativeProcessError { NativeProcessError::Socket(error.kind()) }

// Private deterministic deadline seam for causal boundary tests. Production
// always samples Instant directly; no caller timestamp can extend its lifetime.
fn run_with_clock<F>(socket: UnixStream, evaluator: NativeEvaluator, salt: Vec<u8>,
    budget: NativeProcessBudget, mut now: F) -> Result<NativeProcessReport, NativeProcessError>
where F: FnMut(usize, &NativeHelperClient<UnixStream>) -> Instant {
    socket.peer_addr().map_err(socket_error)?;
    socket.set_nonblocking(false).map_err(socket_error)?;
    let control = socket.try_clone().map_err(socket_error)?;
    let mut client = NativeHelperClient::new(socket, evaluator, salt).map_err(NativeProcessError::Contract)?;
    let mut steps = 0;
    let stop = loop {
        let Some(remaining) = budget.remaining(now(steps, &client)) else { break NativeProcessStop::Deadline; };
        if steps == budget.max_steps { break NativeProcessStop::StepLimit; }
        if let Err(error) = control.set_read_timeout(Some(remaining))
            .and_then(|()| control.set_write_timeout(Some(remaining))) {
            break NativeProcessStop::Socket(error.kind());
        }
        steps += 1;
        let result = client.step();
        // Never acknowledge completion outside the configured interval, including
        // a final syscall whose bytes were accepted before the clock check.
        if budget.remaining(now(steps, &client)).is_none() { break NativeProcessStop::Deadline; }
        match result {
            Ok(NativeClientProgress::Protocol(ClientProgress::ReplySent)) => break NativeProcessStop::ReplySent,
            Err(error) => break NativeProcessStop::Client(error),
            Ok(_) => {}
        }
    };
    let phase = client.phase();
    client.cancel(); // retains first failure, final verdict and native work
    let report = NativeProcessReport { stop, steps, phase, evaluations: client.evaluations(),
        evaluation: client.evaluation_progress(), elapsed: budget.started.elapsed() };
    drop(client);
    drop(control);
    Ok(report)
}

#[cfg(test)]
mod tests;
