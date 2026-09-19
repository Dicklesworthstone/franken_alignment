//! Provisioned peer only: cold startup never dials, reads or writes the socket.
use super::{NativeAssetReadBudget, NativeEvaluator, NativeFileBootstrap, NativeFileBootstrapError,
    PretrainedReceipt, WeightReadBudget};
use super::super::super::peer::{NativeHelperClient, MIN_NATIVE_SALT_BYTES};
use crate::action::consequence::oversight::helper_workers::{MAX_WORKER_SALT_BYTES, io::WorkerIoError};
use crate::Error;
use std::fmt;
use std::os::unix::net::UnixStream;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeFilePeerError { Startup(NativeFileBootstrapError), Transport(WorkerIoError) }
impl fmt::Display for NativeFilePeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeFilePeerError {}

impl NativeHelperClient<UnixStream> {
    /// Consume an independently provisioned connected socket and salt. Validate
    /// salt length before disk work, bootstrap a cold evaluator, and select the
    /// original nonblocking client. No request or vote bytes move during startup.
    /// Failure drops this socket; no default response or partial worker escapes.
    /// Caller pumps the unchanged cooperative protocol after success.
    pub fn from_llama_files(request: NativeFileBootstrap<'_>, socket: UnixStream, salt: Vec<u8>,
        assets: &mut NativeAssetReadBudget, weights: &mut WeightReadBudget)
        -> Result<(Self, PretrainedReceipt), NativeFilePeerError>
    {
        if !(MIN_NATIVE_SALT_BYTES..=MAX_WORKER_SALT_BYTES).contains(&salt.len()) {
            return Err(NativeFilePeerError::Transport(WorkerIoError::Protocol(Error::Limit)));
        }
        socket.set_nonblocking(true).map_err(|error| NativeFilePeerError::Transport(WorkerIoError::Io(error.kind())))?;
        let (evaluator, receipt) = NativeEvaluator::from_llama_files(request, assets, weights)
            .map_err(NativeFilePeerError::Startup)?;
        let client = Self::from_unix(socket, evaluator, salt).map_err(NativeFilePeerError::Transport)?;
        Ok((client, receipt))
    }
}
