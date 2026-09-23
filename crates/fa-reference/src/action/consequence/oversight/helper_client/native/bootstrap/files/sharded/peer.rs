//! Provisioned sockets and the ORIGINAL native worker; no implicit listener,
//! reconnect, retry task, new transcript, default vote or second executor.

use super::{NativeEvaluator, NativeShardFileBootstrap, NativeShardFileBootstrapError,
    NativeAssetReadBudget, WeightReadBudget, PretrainedShardReceipt};
use crate::action::consequence::oversight::helper_client::native::peer::{NativeHelperClient, MIN_NATIVE_SALT_BYTES};
use crate::action::consequence::oversight::helper_client::native::process::{
    NativeProcessBudget, NativeProcessError, NativeProcessReport, run_native_worker,
};
use crate::action::consequence::oversight::helper_workers::{MAX_WORKER_SALT_BYTES, io::WorkerIoError};
use crate::Error;
use std::fmt;
use std::os::unix::net::UnixStream;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeShardFilePeerError {
    Startup(NativeShardFileBootstrapError),
    Transport(WorkerIoError),
}
impl fmt::Display for NativeShardFilePeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeShardFilePeerError {}

impl NativeHelperClient<UnixStream> {
    /// Consume one connected, independently provisioned socket and salt, then
    /// load a fresh sharded native evaluator. Salt and socket checks precede
    /// asset I/O. No protocol bytes move and no inference runs at construction.
    /// The caller drives the SAME cooperative client afterward; only its original
    /// admitted input and fully monitored answer can produce commit/reveal bytes.
    /// Startup failure destroys the socket and returns no partial helper/vote.
    pub fn from_llama_shard_files(request: NativeShardFileBootstrap<'_>,
        socket: UnixStream, salt: Vec<u8>, assets: &mut NativeAssetReadBudget,
        weights: &mut WeightReadBudget) -> Result<(Self, PretrainedShardReceipt), NativeShardFilePeerError>
    {
        check_salt(&salt).map_err(|error| NativeShardFilePeerError::Transport(WorkerIoError::Protocol(error)))?;
        socket.peer_addr().map_err(|error| NativeShardFilePeerError::Transport(WorkerIoError::Io(error.kind())))?;
        socket.set_nonblocking(true)
            .map_err(|error| NativeShardFilePeerError::Transport(WorkerIoError::Io(error.kind())))?;
        let (evaluator, receipt) = NativeEvaluator::from_llama_shard_files(request, assets, weights)
            .map_err(NativeShardFilePeerError::Startup)?;
        let client = Self::from_unix(socket, evaluator, salt).map_err(NativeShardFilePeerError::Transport)?;
        Ok((client, receipt))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeShardProcessError {
    /// The supplied lifetime was already over, before opening any asset.
    ExpiredBeforeStartup,
    Startup(NativeShardFileBootstrapError),
    Process(NativeProcessError),
}
impl fmt::Display for NativeShardProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for NativeShardProcessError {}

/// Cold-start and run one dedicated Unix helper using its ORIGINAL process loop.
///
/// The caller creates the absolute NativeProcessBudget BEFORE startup and this
/// function passes that exact object onward. Weight loading cannot mint a fresh
/// protocol deadline or step allowance. Expiry during loading is checked by the
/// original worker BEFORE any request/commit/reveal step; a successful model load
/// with an expired lifetime returns its original Deadline report, not a vote.
/// Salt/socket failure or already-expired lifetime causes no disk reads.
///
/// Filesystem operations and individual numerical steps are not preempted. This
/// is a dedicated child-process API, not permission to block an executor thread.
/// The supervisor still enforces its own review deadlines and child termination.
/// Original reports distinguish reply transmission from acceptance; an error or
/// late write never claims that no protocol bytes reached the supervisor.
/// Every exit consumes the socket, evaluator and lifetime; nothing is resent.
pub fn run_native_worker_from_shard_files(request: NativeShardFileBootstrap<'_>,
    socket: UnixStream, salt: Vec<u8>, assets: &mut NativeAssetReadBudget,
    weights: &mut WeightReadBudget, lifetime: NativeProcessBudget)
    -> Result<(NativeProcessReport, PretrainedShardReceipt), NativeShardProcessError>
{
    check_salt(&salt).map_err(|error| NativeShardProcessError::Process(NativeProcessError::Contract(error)))?;
    socket.peer_addr().map_err(|error| NativeShardProcessError::Process(NativeProcessError::Socket(error.kind())))?;
    if lifetime.expired() { return Err(NativeShardProcessError::ExpiredBeforeStartup); }
    let (evaluator, receipt) = NativeEvaluator::from_llama_shard_files(request, assets, weights)
        .map_err(NativeShardProcessError::Startup)?;
    let report = run_native_worker(socket, evaluator, salt, lifetime).map_err(NativeShardProcessError::Process)?;
    Ok((report, receipt))
}

fn check_salt(salt: &[u8]) -> Result<(), Error> {
    if !(MIN_NATIVE_SALT_BYTES..=MAX_WORKER_SALT_BYTES).contains(&salt.len()) {
        return Err(Error::Limit);
    }
    Ok(())
}
