//! Unix local-filesystem publication with a surviving controller recovery key.
//!
//! One atomic file replacement publishes the endpoint payload AND its terminal
//! receipt history. Replay uses the original endpoint transitions, not a second
//! resource ledger. Files and their parent directory must be operator-controlled.
//! This is not authenticated storage, whole-process recovery or distributed HA.

mod codec;
#[cfg(test)]
mod tests;

use super::*;
use std::cell::Cell;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub const MAX_FILE_MUTATIONS: usize = 4_096;
pub const MAX_PUBLICATION_FILE_BYTES: usize = 16 * 1_048_576;
const STATE: &str = "publication.bin";
const PENDING: &str = "publication.pending";
const LOCK: &str = "owner.lock";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePublicationLimits {
    pub mutations: usize,
    pub bytes: usize,
}

impl FilePublicationLimits {
    fn validate(self) -> Result<(), Error> {
        if self.mutations == 0 || self.bytes == 0 { return Err(Error::InvalidInput); }
        if self.mutations > MAX_FILE_MUTATIONS || self.bytes > MAX_PUBLICATION_FILE_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageStage {
    CreateDirectory,
    OpenDirectory,
    OpenLock,
    ReadPublication,
    CreatePending,
    WritePending,
    SyncPending,
    SyncPublication,
    Publish,
    SyncDirectory,
    SyncParentDirectory,
    RemovePending,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageFailure {
    pub stage: StorageStage,
    pub kind: io::ErrorKind,
    /// The new canonical file may already have been observed. Never a refund.
    pub publication_visible: bool,
}

#[derive(Debug)]
pub enum FilePublicationError {
    Refused(Error),
    Io { stage: StorageStage, kind: io::ErrorKind },
    LockUnavailable(String),
}

impl fmt::Display for FilePublicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") }
}
impl std::error::Error for FilePublicationError {}
impl From<Error> for FilePublicationError {
    fn from(error: Error) -> Self { Self::Refused(error) }
}
fn io_error(stage: StorageStage, error: io::Error) -> FilePublicationError {
    FilePublicationError::Io { stage, kind: error.kind() }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStorageStatus {
    /// Last visible canonical-file revision in this instance.
    pub visible_revision: u64,
    /// Last revision whose file and directory synchronization both returned Ok.
    pub synchronized_revision: u64,
    pub retained_mutations: usize,
    pub encoded_bytes: usize,
    pub clock_confirmation_required: bool,
    pub failure: Option<StorageFailure>,
}

/// Read-only data from a complete visible file. This is NOT a receipt accepted
/// by a broker, an authentication claim or proof that another process fsynced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePublicationSnapshot {
    pub revision: u64,
    pub target: ResolvedTarget,
    pub payload: Vec<u8>,
    pub stream: Option<StreamView>,
    pub dispatcher_epoch: u64,
    pub observed_time: Option<ElapsedTick>,
    pub execution_count: u64,
    pub terminal_receipts: usize,
}

struct RecoveryState {
    binding: Rc<()>,
    directory: PathBuf,
    initial: Vec<u8>,
    limits: FilePublicationLimits,
    visible_revision: Cell<u64>,
    observed_floor: Cell<Option<ElapsedTick>>,
}

/// Keep outside the endpoint worker while its ORIGINAL control authority lives.
/// The key neither contains nor recreates that authority. It is not serializable
/// or clonable. Reopening while an endpoint owns the directory lock refuses.
///
/// ```compile_fail
/// use fa_reference::action::consequence::delivery::FileEndpointRecovery;
/// fn duplicate(key: FileEndpointRecovery) { let _other = key.clone(); }
/// ```
pub struct FileEndpointRecovery { state: Rc<RecoveryState> }

impl fmt::Debug for FileEndpointRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileEndpointRecovery")
            .field("visible_revision", &self.state.visible_revision.get()).finish_non_exhaustive()
    }
}

pub(super) struct FileStore {
    recovery: Rc<RecoveryState>,
    _lock: File,
    directory: File,
    events: Vec<Vec<u8>>,
    visible_revision: u64,
    synchronized_revision: u64,
    encoded_bytes: usize,
    clock_confirmed: bool,
    failure: Option<StorageFailure>,
    #[cfg(test)]
    fail_at: Option<StorageStage>,
}

impl fmt::Debug for FileStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileStore").field("status", &self.status()).finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub(super) enum Operation {
    Attach(Scope),
    ObserveTime(ElapsedTick),
    Fence(FenceRequest),
    Deliver(DispatchEnvelope),
    Seal(StatusQuery),
    ResolveExpired(StatusQuery),
}

pub(super) enum OperationResult {
    Unit,
    Fence(FenceAcknowledgment),
    Receipt(EndpointReceipt),
}

impl Operation {
    fn apply(&self, endpoint: &mut PublicationEndpoint) -> Result<OperationResult, Error> {
        match self {
            Self::Attach(scope) => endpoint.attach(*scope).map(|()| OperationResult::Unit),
            Self::ObserveTime(tick) => endpoint.observe_time(*tick).map(|()| OperationResult::Unit),
            Self::Fence(request) => endpoint.install_fence(request.clone()).map(OperationResult::Fence),
            Self::Deliver(message) => endpoint.deliver(message).map(OperationResult::Receipt),
            Self::Seal(query) => endpoint.seal_unexecuted(query).map(OperationResult::Receipt),
            Self::ResolveExpired(query) => endpoint.resolve_expired(query).map(OperationResult::Receipt),
        }
    }
}

impl PublicationEndpoint {
    /// Create a NEW private directory. The only published resource is the
    /// payload decoded from publication.bin, never the staging file's contents.
    pub fn create_file_publication(
        path: impl AsRef<Path>, resource: ResolvedTarget, payload: Vec<u8>,
        retention_ticks: u64, max_deliveries: usize, limits: FilePublicationLimits,
    ) -> Result<(Self, FileEndpointRecovery), FilePublicationError> {
        Self::new(resource, payload, retention_ticks, max_deliveries)?.create_store(path.as_ref(), limits)
    }

    pub fn create_file_stream(
        path: impl AsRef<Path>, resource: ResolvedTarget, profile: StreamProfile,
        retention_ticks: u64, max_deliveries: usize, limits: FilePublicationLimits,
    ) -> Result<(Self, FileEndpointRecovery), FilePublicationError> {
        Self::new_stream(resource, profile, retention_ticks, max_deliveries)?.create_store(path.as_ref(), limits)
    }

    fn create_store(
        mut self, path: &Path, limits: FilePublicationLimits,
    ) -> Result<(Self, FileEndpointRecovery), FilePublicationError> {
        limits.validate()?;
        let initial = codec::encode_initial(&self)?;
        let bytes = codec::encode_file(&initial, &[], limits)?;
        fs::DirBuilder::new().mode(0o700).create(path)
            .map_err(|error| io_error(StorageStage::CreateDirectory, error))?;
        let path = path.canonicalize().map_err(|error| io_error(StorageStage::OpenDirectory, error))?;
        let directory = open_directory(&path)?;
        let lock = open_lock(&path, true)?;
        let recovery = Rc::new(RecoveryState {
            binding: Rc::clone(&self.binding), directory: path, initial, limits,
            visible_revision: Cell::new(0), observed_floor: Cell::new(None),
        });
        let mut store = FileStore {
            recovery: Rc::clone(&recovery), _lock: lock, directory, events: Vec::new(),
            visible_revision: 0, synchronized_revision: 0, encoded_bytes: bytes.len(),
            clock_confirmed: false, failure: None,
            #[cfg(test)] fail_at: None,
        };
        if let Err(failure) = store.replace_file(&bytes, 0) {
            return Err(FilePublicationError::Io { stage: failure.stage, kind: failure.kind });
        }
        let parent = recovery.directory.parent().ok_or(Error::Binding)?;
        open_directory(parent)?.sync_all().map_err(|error| io_error(StorageStage::SyncParentDirectory, error))?;
        self.file_store = Some(store);
        Ok((self, FileEndpointRecovery { state: recovery }))
    }

    pub fn file_storage_status(&self) -> Option<FileStorageStatus> {
        self.file_store.as_ref().map(FileStore::status)
    }

    /// No lock or recovery key is required to inspect a complete visible file.
    /// An atomic replacement may make this a preceding snapshot, never a grant.
    pub fn read_file_publication(
        directory: impl AsRef<Path>,
    ) -> Result<FilePublicationSnapshot, FilePublicationError> {
        let bytes = read_file(&directory.as_ref().join(STATE), MAX_PUBLICATION_FILE_BYTES)?;
        let decoded = codec::decode_file(&bytes, Rc::new(()))?;
        Ok(decoded.endpoint.snapshot_view(decoded.events.len() as u64))
    }

    fn snapshot_view(&self, revision: u64) -> FilePublicationSnapshot {
        FilePublicationSnapshot {
            revision, target: self.resource, payload: self.payload.clone(), stream: self.stream.clone(),
            dispatcher_epoch: self.epoch, observed_time: self.elapsed,
            execution_count: self.executions, terminal_receipts: self.receipts.len(),
        }
    }

    pub(super) fn check_file_store(&self) -> Result<(), Error> {
        if self.file_store.as_ref().is_some_and(|store| store.failure.is_some() || !store.clock_confirmed) {
            return Err(Error::Incomplete);
        }
        Ok(())
    }

    pub(super) fn file_transition(&mut self, operation: Operation) -> Result<OperationResult, Error> {
        let store = self.file_store.as_ref().ok_or(Error::WrongState)?;
        if store.failure.is_some() { return Err(Error::Incomplete); }
        let observed = match &operation {
            Operation::ObserveTime(tick) => {
                if store.recovery.observed_floor.get().is_some_and(|floor| *tick < floor) {
                    return Err(Error::Stale);
                }
                Some(*tick)
            }
            Operation::Deliver(_) | Operation::Seal(_) | Operation::ResolveExpired(_) => {
                self.check_file_store()?;
                None
            }
            _ => None,
        };
        let mut next = self.memory_copy();
        let result = operation.apply(&mut next)?;
        if let Some(tick) = observed {
            let store = self.file_store.as_mut().expect("retained store");
            // A valid new clock observation cannot be forgotten by reopening
            // after a failed write or a storage-budget refusal.
            store.recovery.observed_floor.set(Some(tick));
            store.clock_confirmed = false;
        }
        if self.same_state(&next) {
            if observed.is_some() { self.file_store.as_mut().expect("retained store").clock_confirmed = true; }
            return Ok(result);
        }
        let encoded = codec::encode_operation(&operation)?;
        let store = self.file_store.as_mut().expect("retained store");
        let published = store.append(encoded);
        let visible = published.is_ok()
            || store.failure.as_ref().is_some_and(|failure| failure.publication_visible);
        if published.is_ok() && observed.is_some() { store.clock_confirmed = true; }
        // A rename can succeed even when the following directory sync fails.
        // Retain the possibly visible effect locally, but return NO receipt.
        if visible { self.install_memory(next); }
        published?;
        Ok(result)
    }

    fn memory_copy(&self) -> Self {
        Self {
            binding: Rc::clone(&self.binding), retention_ticks: self.retention_ticks,
            max_deliveries: self.max_deliveries, resource: self.resource,
            payload: self.payload.clone(), scope: self.scope, epoch: self.epoch, elapsed: self.elapsed,
            receipts: self.receipts.clone(), executions: self.executions, stream: self.stream.clone(),
            file_store: None,
        }
    }

    fn same_state(&self, other: &Self) -> bool {
        self.resource == other.resource && self.payload == other.payload && self.scope == other.scope
            && self.epoch == other.epoch && self.elapsed == other.elapsed
            && self.receipts == other.receipts && self.executions == other.executions
            && self.stream == other.stream
    }

    fn install_memory(&mut self, next: Self) {
        self.resource = next.resource;
        self.payload = next.payload;
        self.scope = next.scope;
        self.epoch = next.epoch;
        self.elapsed = next.elapsed;
        self.receipts = next.receipts;
        self.executions = next.executions;
        self.stream = next.stream;
    }
}

impl FileEndpointRecovery {
    pub fn directory(&self) -> &Path { &self.state.directory }
    pub fn visible_revision(&self) -> u64 { self.state.visible_revision.get() }

    /// The controller and this key must survive. Missing, truncated or rolled
    /// back publication files refuse; recovery never bootstraps an empty ledger.
    /// Observe the CURRENT clock after reopening, before status or delivery.
    pub fn reopen(&self) -> Result<PublicationEndpoint, FilePublicationError> {
        let directory = open_directory(&self.state.directory)?;
        let lock = open_lock(&self.state.directory, false)?;
        let bytes = read_file(&self.state.directory.join(STATE), self.state.limits.bytes)?;
        let decoded = codec::decode_file(&bytes, Rc::clone(&self.state.binding))?;
        if decoded.initial != self.state.initial || decoded.limits != self.state.limits {
            return Err(Error::Binding.into());
        }
        let revision = decoded.events.len() as u64;
        if revision != self.state.visible_revision.get() { return Err(Error::Stale.into()); }
        // The surviving floor binds the visible history cut. It is not a hash,
        // signature or anti-rollback primitive after loss of the whole process.
        let state_file = regular_file(&self.state.directory.join(STATE), StorageStage::ReadPublication)?;
        state_file.sync_all().map_err(|error| io_error(StorageStage::SyncPublication, error))?;
        directory.sync_all().map_err(|error| io_error(StorageStage::SyncDirectory, error))?;
        let pending = self.state.directory.join(PENDING);
        match fs::symlink_metadata(&pending) {
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                fs::remove_file(pending).map_err(|error| io_error(StorageStage::RemovePending, error))?;
                directory.sync_all().map_err(|error| io_error(StorageStage::SyncDirectory, error))?;
            }
            Ok(_) => return Err(Error::Binding.into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(StorageStage::RemovePending, error)),
        }
        let mut endpoint = decoded.endpoint;
        endpoint.file_store = Some(FileStore {
            recovery: Rc::clone(&self.state), _lock: lock, directory, events: decoded.events,
            visible_revision: revision, synchronized_revision: revision, encoded_bytes: bytes.len(),
            clock_confirmed: false, failure: None,
            #[cfg(test)] fail_at: None,
        });
        Ok(endpoint)
    }
}

impl FileStore {
    fn status(&self) -> FileStorageStatus {
        FileStorageStatus {
            visible_revision: self.visible_revision, synchronized_revision: self.synchronized_revision,
            retained_mutations: self.events.len(), encoded_bytes: self.encoded_bytes,
            clock_confirmation_required: !self.clock_confirmed, failure: self.failure.clone(),
        }
    }

    fn append(&mut self, event: Vec<u8>) -> Result<(), Error> {
        if self.events.len() >= self.recovery.limits.mutations { return Err(Error::Limit); }
        self.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let bytes = codec::encode_appended_file(&self.recovery.initial, &self.events, &event, self.recovery.limits)?;
        let revision = self.visible_revision.checked_add(1).ok_or(Error::Overflow)?;
        match self.replace_file(&bytes, revision) {
            Ok(()) => {
                self.events.push(event);
                self.encoded_bytes = bytes.len();
                Ok(())
            }
            Err(failure) => {
                if failure.publication_visible {
                    self.events.push(event);
                    self.encoded_bytes = bytes.len();
                }
                self.failure = Some(failure);
                Err(Error::Incomplete)
            }
        }
    }

    fn replace_file(&mut self, bytes: &[u8], revision: u64) -> Result<(), StorageFailure> {
        let mut stage = StorageStage::CreatePending;
        let mut visible = false;
        let result = (|| -> io::Result<()> {
            self.inject(stage)?;
            let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600)
                .open(self.recovery.directory.join(PENDING))?;
            stage = StorageStage::WritePending;
            self.inject(stage)?;
            file.write_all(bytes)?;
            stage = StorageStage::SyncPending;
            self.inject(stage)?;
            file.sync_all()?;
            stage = StorageStage::Publish;
            self.inject(stage)?;
            fs::rename(self.recovery.directory.join(PENDING), self.recovery.directory.join(STATE))?;
            visible = true;
            self.visible_revision = revision;
            self.recovery.visible_revision.set(revision);
            stage = StorageStage::SyncDirectory;
            self.inject(stage)?;
            self.directory.sync_all()?;
            self.synchronized_revision = revision;
            Ok(())
        })();
        result.map_err(|error| StorageFailure { stage, kind: error.kind(), publication_visible: visible })
    }

    fn inject(&self, stage: StorageStage) -> io::Result<()> {
        #[cfg(test)]
        if self.fail_at == Some(stage) { return Err(io::Error::other("injected publication barrier failure")); }
        let _ = stage;
        Ok(())
    }
}

fn open_directory(path: &Path) -> Result<File, FilePublicationError> {
    let meta = fs::symlink_metadata(path).map_err(|error| io_error(StorageStage::OpenDirectory, error))?;
    if !meta.is_dir() || meta.file_type().is_symlink() { return Err(Error::Binding.into()); }
    File::open(path).map_err(|error| io_error(StorageStage::OpenDirectory, error))
}

fn open_lock(path: &Path, create: bool) -> Result<File, FilePublicationError> {
    let path = path.join(LOCK);
    if !create {
        let meta = fs::symlink_metadata(&path).map_err(|error| io_error(StorageStage::OpenLock, error))?;
        if !meta.is_file() || meta.file_type().is_symlink() { return Err(Error::Binding.into()); }
    }
    let file = OpenOptions::new().read(true).write(true).create_new(create).mode(0o600)
        .open(path).map_err(|error| io_error(StorageStage::OpenLock, error))?;
    file.try_lock().map_err(|error| FilePublicationError::LockUnavailable(error.to_string()))?;
    Ok(file)
}

fn regular_file(path: &Path, stage: StorageStage) -> Result<File, FilePublicationError> {
    let meta = fs::symlink_metadata(path).map_err(|error| io_error(stage, error))?;
    if !meta.is_file() || meta.file_type().is_symlink() { return Err(Error::Binding.into()); }
    let file = File::open(path).map_err(|error| io_error(stage, error))?;
    if !file.metadata().map_err(|error| io_error(stage, error))?.is_file() { return Err(Error::Binding.into()); }
    Ok(file)
}

fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>, FilePublicationError> {
    let file = regular_file(path, StorageStage::ReadPublication)?;
    if file.metadata().map_err(|error| io_error(StorageStage::ReadPublication, error))?.len() > limit as u64 {
        return Err(Error::Limit.into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)
        .map_err(|error| io_error(StorageStage::ReadPublication, error))?;
    if bytes.len() > limit { return Err(Error::Limit.into()); }
    Ok(bytes)
}
