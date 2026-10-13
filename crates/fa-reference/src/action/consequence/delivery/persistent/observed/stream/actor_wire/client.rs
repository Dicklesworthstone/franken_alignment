//! Write-ahead client ownership around the ORIGINAL ActorClient and Store.
//! This is not a second server request ledger, effect authority or socket loop.
mod image;
#[cfg(test)]
mod tests;
use crate::action::consequence::delivery::persistent::{storage, JournalError, MAX_JOURNAL_BYTES};
use crate::action::consequence::oversight::actor::ActorProposal;
use crate::action::consequence::oversight::actor_wire::{Command, WireResponse};
use crate::action::consequence::oversight::actor_wire::client::{ActorClient, ActorClientState,
    ClientError, ClientFailure, ClientIoWork, ClientPhase, ClientProgress, ClientSessionLimits};
use crate::Error;
use std::fmt;
use std::io::{Read, Write};
use std::path::Path;

/// Independent exact server/transport binding and lifetime client limits. The
/// caller still authenticates the supplied connection to this SAME domain.
#[derive(Clone, PartialEq, Eq)]
pub struct FileClientProfile { binding: Vec<u8>, limits: ClientSessionLimits, image_bytes: usize }
impl FileClientProfile {
    pub fn new(binding: Vec<u8>, limits: ClientSessionLimits, image_bytes: usize) -> Result<Self, FileClientError> {
        if image_bytes == 0 || image_bytes > MAX_JOURNAL_BYTES { return Err(Error::Limit.into()); }
        let state = ActorClientState::new(limits)?;
        if state.encode_recovery(&binding)?.len() > image_bytes { return Err(Error::Limit.into()); }
        Ok(Self { binding, limits, image_bytes })
    }
    pub fn binding(&self) -> &[u8] { &self.binding }
    pub fn limits(&self) -> ClientSessionLimits { self.limits }
    pub fn image_bytes(&self) -> usize { self.image_bytes }
}
impl fmt::Debug for FileClientProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileClientProfile").field("limits", &self.limits).field("image_bytes", &self.image_bytes).finish_non_exhaustive()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileClientError { Storage(JournalError), Client(ClientError) }
impl From<JournalError> for FileClientError { fn from(error: JournalError) -> Self { Self::Storage(error) } }
impl From<Error> for FileClientError { fn from(error: Error) -> Self { Self::Storage(error.into()) } }
impl From<ClientError> for FileClientError { fn from(error: ClientError) -> Self { Self::Client(error) } }
impl fmt::Display for FileClientError { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{self:?}") } }
impl std::error::Error for FileClientError {}

struct Owner { store: storage::Store, profile: FileClientProfile, revision: u64, unavailable: bool }
impl Owner {
    fn check(&self, expected: u64) -> Result<(), FileClientError> {
        if self.unavailable { return Err(JournalError::Unavailable.into()); }
        if expected != self.revision { return Err(Error::Stale.into()); } Ok(())
    }
    fn commit(&mut self, archive: &[u8], reserved: Option<ClientIoWork>) -> Result<(), FileClientError> {
        self.unavailable = true;
        let next = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        let bytes = image::encode(self.store.identity(), &self.profile, next, archive, reserved)?;
        self.store.replace(&bytes)?;
        self.revision = next; self.unavailable = false; Ok(())
    }
}

/// Read-only selected canonical cut. Reservations are separate from completed
/// work. The saved response is historical; this value cannot connect or resend.
pub struct FileClientInspection {
    revision: u64, state: ActorClientState, reserved: Option<ClientIoWork>,
}
impl FileClientInspection {
    pub fn revision(&self) -> u64 { self.revision }
    pub fn work(&self) -> ClientIoWork { self.state.work() }
    pub fn reserved_work(&self) -> Option<ClientIoWork> { self.reserved }
    pub fn request_count(&self) -> usize { self.state.request_count() }
    pub fn original_proposal(&self, request: u64) -> Option<&ActorProposal> { self.state.original_proposal(request) }
    pub fn last_response(&self, request: u64) -> Option<&WireResponse> { self.state.last_response(request) }
    pub fn interrupted_command(&self) -> Option<&Command> { self.state.interrupted_command() }
    pub fn last_failure(&self) -> Option<ClientFailure> { self.state.last_failure() }
}
impl fmt::Debug for FileClientInspection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileClientInspection").field("revision", &self.revision)
            .field("work", &self.work()).field("reserved", &self.reserved).finish_non_exhaustive()
    }
}

/// Exclusive client-side retry custody. Use a directory DISTINCT from every
/// server publication domain. No server owner, policy, permit or clock is here.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::FileActorClient;
/// fn duplicate(client: FileActorClient) { client.clone(); }
/// ```
pub struct FileActorClient { owner: Owner, state: ActorClientState }
impl fmt::Debug for FileActorClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileActorClient").field("revision", &self.owner.revision).field("state", &self.state).finish_non_exhaustive()
    }
}
impl FileActorClient {
    pub fn create(directory: impl AsRef<Path>, profile: FileClientProfile) -> Result<Self, FileClientError> {
        let state = ActorClientState::new(profile.limits)?;
        let archive = state.encode_recovery(&profile.binding)?;
        let store = storage::Store::create(directory.as_ref())?;
        let bytes = image::encode(store.identity(), &profile, 0, &archive, None)?;
        store.replace(&bytes)?;
        Ok(Self { owner: Owner { store, profile, revision: 0, unavailable: false }, state })
    }
    /// Check complete image, exact domain/limits/path and independent revision
    /// floor BEFORE cleanup. An unfinished step consumes its full saved ceiling.
    /// Reopening sends nothing and never restores transport-local ticket rights.
    pub fn open(directory: impl AsRef<Path>, profile: FileClientProfile, minimum_revision: u64) -> Result<Self, FileClientError> {
        let store = storage::Store::open(directory.as_ref())?;
        let bytes = store.read(profile.image_bytes)?;
        let cut = image::decode(store.identity(), &profile, minimum_revision, &bytes)?;
        let mut state = cut.state;
        if let Some(ceiling) = cut.reserved { state.consume_recovery_reservation(ceiling)?; }
        store.confirm_and_cleanup()?;
        Ok(Self { owner: Owner { store, profile, revision: cut.revision, unavailable: false }, state })
    }
    /// Historical inspection beside a locked/faulted client, without cleanup,
    /// writer custody or connection. Staging files are NEVER promoted as results.
    pub fn read(directory: impl AsRef<Path>, profile: &FileClientProfile, minimum_revision: u64) -> Result<FileClientInspection, FileClientError> {
        let identity = storage::identity(directory.as_ref())?;
        let bytes = storage::read(&identity.join(storage::CANONICAL), profile.image_bytes)?;
        image::decode(&identity, profile, minimum_revision, &bytes)
    }
    pub fn revision(&self) -> u64 { self.owner.revision }
    pub fn state(&self) -> &ActorClientState { &self.state }
    /// Caller supplies a bounded/nonblocking connection to the independently
    /// authenticated SAME server. No read, write, retry or hidden task occurs.
    pub fn connect<S>(self, stream: S) -> FileActorConnection<S> {
        FileActorConnection { owner: self.owner, client: Some(self.state.connect(stream)),
            #[cfg(test)] completion_failure: None }
    }
}

/// One original exchange at a time. Persist intent BEFORE any socket operation;
/// persist the resulting original state BEFORE returning responses or errors.
/// Socket progress cannot atomically commit with local disk, so uncertainty and
/// reserved work survive that gap. No alternate protocol parser or I/O loop.
pub struct FileActorConnection<S> {
    owner: Owner, client: Option<ActorClient<S>>,
    #[cfg(test)]
    completion_failure: Option<crate::action::consequence::delivery::persistent::JournalIo>,
}
impl<S> fmt::Debug for FileActorConnection<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileActorConnection").field("revision", &self.owner.revision)
            .field("unavailable", &self.owner.unavailable).finish_non_exhaustive()
    }
}
impl<S> FileActorConnection<S> {
    pub fn revision(&self) -> u64 { self.owner.revision }
    pub fn unavailable(&self) -> bool { self.owner.unavailable }
    pub fn state(&self) -> Result<&ActorClientState, FileClientError> {
        self.owner.check(self.owner.revision)?;
        Ok(self.client.as_ref().ok_or(JournalError::Unavailable)?.state())
    }
    pub fn phase(&self) -> Result<Option<ClientPhase>, FileClientError> {
        self.owner.check(self.owner.revision)?;
        Ok(self.client.as_ref().ok_or(JournalError::Unavailable)?.phase())
    }
    fn check(&mut self, revision: u64) -> Result<(), FileClientError> {
        if self.owner.unavailable { self.client = None; }
        self.owner.check(revision)
    }
    fn persist(&mut self, reserved: Option<ClientIoWork>) -> Result<(), FileClientError> {
        self.owner.unavailable = true;
        let result = (|| {
            let bytes = self.client.as_ref().ok_or(JournalError::Unavailable)?.recovery_image(&self.owner.profile.binding)?;
            self.owner.commit(&bytes, reserved)
        })();
        if result.is_err() { self.client = None; }
        result
    }
    fn prepare(&mut self, revision: u64, begin: impl FnOnce(&mut ActorClient<S>) -> Result<(), ClientError>) -> Result<(), FileClientError> {
        self.check(revision)?;
        self.owner.unavailable = true;
        let result = begin(self.client.as_mut().ok_or(JournalError::Unavailable)?);
        if let Err(error) = result {
            // Original completed preflight failures do not enter I/O or change
            // requests. A panic, unlike an Err, keeps the unavailable latch set.
            self.owner.unavailable = false; return Err(error.into());
        }
        self.persist(None)
    }
    pub fn submit(&mut self, revision: u64, request: u64, proposal: &ActorProposal) -> Result<(), FileClientError> {
        self.prepare(revision, |client| client.submit(request, proposal))
    }
    pub fn retry_submission(&mut self, revision: u64, request: u64) -> Result<(), FileClientError> {
        self.prepare(revision, |client| client.retry_submission(request))
    }
    pub fn poll(&mut self, revision: u64, request: u64) -> Result<(), FileClientError> {
        self.prepare(revision, |client| client.poll(request))
    }
    pub fn cancel(&mut self, revision: u64, request: u64) -> Result<(), FileClientError> {
        self.prepare(revision, |client| client.cancel(request))
    }
    /// Close the connection without cancelling server requests. An unavailable
    /// writer cannot escape its disk-recovery requirement through this operation.
    pub fn into_disconnected(mut self) -> Result<FileActorClient, FileClientError> {
        self.check(self.owner.revision)?;
        let state = self.client.take().ok_or(JournalError::Unavailable)?.into_state();
        Ok(FileActorClient { owner: self.owner, state })
    }
}
impl<S: Read + Write> FileActorConnection<S> {
    pub fn step(&mut self, revision: u64) -> Result<ClientProgress, FileClientError> {
        self.check(revision)?;
        let Some(ceiling) = self.client.as_ref().ok_or(JournalError::Unavailable)?.recovery_step_ceiling()? else {
            return self.client.as_mut().ok_or(JournalError::Unavailable)?.step().map_err(Into::into);
        };
        self.persist(Some(ceiling))?; // acknowledge maximum next I/O BEFORE entry
        self.owner.unavailable = true; // retain across a caught read/write unwind
        let progress = self.client.as_mut().ok_or(JournalError::Unavailable)?.step();
        #[cfg(test)]
        if let Some(stage) = self.completion_failure.take() { self.owner.store.fail_once(stage); }
        self.persist(None)?; // withhold even a received response until this commits
        progress.map_err(Into::into)
    }
}
