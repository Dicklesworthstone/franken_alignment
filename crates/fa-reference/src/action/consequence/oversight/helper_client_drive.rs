//! Bounded pumping of a single worker client; the host owns scheduling/inference.

use super::helper_client::{ClientPhase, ClientProgress, HelperClient};
use super::helper_workers::io::WorkerIoError;
use crate::Error;
use std::io::{Read, Write};

pub const MAX_CLIENT_DRIVE_STEPS: usize = 256;

/// Steps count calls to the bounded state machine, NOT syscalls or inference
/// work. A later error retains progress instead of asserting nothing happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientDrive {
    pub steps: usize,
    pub phase: ClientPhase,
    pub progress: Result<ClientProgress, WorkerIoError>,
}

impl<S: Read + Write> HelperClient<S> {
    /// Invalid limits refuse before any I/O. Transient backpressure yields at
    /// once; inference is never invoked and a selected response is never rerolled.
    /// Even a long sequence of Interrupted errors cannot create a hidden loop.
    pub fn drive(&mut self, max_steps: usize) -> Result<ClientDrive, Error> {
        if max_steps == 0 { return Err(Error::InvalidInput); }
        if max_steps > MAX_CLIENT_DRIVE_STEPS { return Err(Error::Limit); }
        if let Some(error) = self.failure() {
            return Ok(ClientDrive { steps: 0, phase: self.phase(), progress: Err(error) });
        }
        let ready = match self.phase() {
            ClientPhase::NeedsInference => Some(ClientProgress::NeedsInference),
            ClientPhase::ReplySent => Some(ClientProgress::ReplySent),
            _ => None,
        };
        if let Some(progress) = ready {
            return Ok(ClientDrive { steps: 0, phase: self.phase(), progress: Ok(progress) });
        }
        let mut report = ClientDrive { steps: 0, phase: self.phase(), progress: Ok(ClientProgress::Progress) };
        for _ in 0..max_steps {
            report.steps += 1;
            report.progress = self.step();
            report.phase = self.phase();
            if report.progress != Ok(ClientProgress::Progress) { break; }
        }
        Ok(report)
    }
}
