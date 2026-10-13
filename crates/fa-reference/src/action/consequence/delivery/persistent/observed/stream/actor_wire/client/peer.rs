//! Opt-in outgoing peer verification around the ORIGINAL durable client.
//! Kernel connection credentials are not executable or application attestation.
use super::{ActorClientState, ActorProposal, ClientDriveBudget, ClientDriveReport,
    ClientPhase, ClientProgress, ClientSessionLimits, FileActorClient, FileActorConnection,
    FileClientError, FileClientInspection, FileClientProfile};
use crate::action::consequence::delivery::persistent::codec::shared::Writer;
use crate::action::consequence::oversight::actor_peer::{
    PeerCredentials, PeerPolicy, PeerRefusal, PeerSetupStage,
};
use std::{fmt, io, os::unix::net::UnixStream, path::Path};

const BINDING_DOMAIN: &[u8; 8] = b"FACPEER\x01";
const MAX_BINDING_BYTES: usize = 4096;

/// Independently provisioned server/transport identity AND its required kernel
/// credentials. Fixed before client creation; there is no live policy switch or
/// public raw-profile unwrap. No peer identity is learned from a candidate socket.
#[derive(Clone, PartialEq, Eq)]
pub struct FilePeerClientProfile {
    inner: FileClientProfile,
    policy: PeerPolicy,
}
impl fmt::Debug for FilePeerClientProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePeerClientProfile").field("policy", &self.policy)
            .field("limits", &self.inner.limits()).finish_non_exhaustive()
    }
}
impl FilePeerClientProfile {
    pub fn policy(&self) -> PeerPolicy { self.policy }
    pub fn limits(&self) -> ClientSessionLimits { self.inner.limits() }
    pub fn image_bytes(&self) -> usize { self.inner.image_bytes() }
}
impl FileClientProfile {
    /// Select the verified-peer mode before creating storage. Domain separation
    /// pins the entire original opaque binding and exact UID/GID/optional PID in
    /// the ORIGINAL recovery image. Old raw-mode stores are not migrated here.
    pub fn with_peer_policy(self, policy: PeerPolicy) -> Result<FilePeerClientProfile, FileClientError> {
        let mut w = Writer::new(MAX_BINDING_BYTES);
        w.raw(BINDING_DOMAIN)?; w.blob(&self.binding)?;
        w.u32(policy.uid())?; w.u32(policy.gid())?;
        w.u8(u8::from(policy.pid().is_some()))?;
        if let Some(pid) = policy.pid() { w.u32(pid)?; }
        let inner = Self::new(w.finish(), self.limits, self.image_bytes)?;
        Ok(FilePeerClientProfile { inner, policy })
    }
}

/// Same exclusive disk owner and exact requests. The selected mode exposes only
/// credential-verified connections; an error retains this original client.
/// A refused candidate closes without sending bytes or acquiring a new ticket.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::peer::FilePeerClient;
/// fn bypass(client: FilePeerClient) { client.into_raw(); }
/// ```
pub struct FilePeerClient {
    client: FileActorClient,
    profile: FilePeerClientProfile,
}
impl fmt::Debug for FilePeerClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePeerClient").field("revision", &self.revision())
            .field("policy", &self.profile.policy).finish_non_exhaustive()
    }
}
impl FilePeerClient {
    pub fn create(directory: impl AsRef<Path>, profile: FilePeerClientProfile) -> Result<Self, FileClientError> {
        let client = FileActorClient::create(directory, profile.inner.clone())?;
        Ok(Self { client, profile })
    }
    /// Original full-image checks, work-reservation handling and revision floor
    /// precede staging cleanup. Changing the peer policy is a binding mismatch.
    /// No socket is created, and saved credentials never stand in for a new check.
    pub fn open(directory: impl AsRef<Path>, profile: FilePeerClientProfile,
        minimum_revision: u64) -> Result<Self, FileClientError>
    {
        let client = FileActorClient::open(directory, profile.inner.clone(), minimum_revision)?;
        Ok(Self { client, profile })
    }
    pub fn read(directory: impl AsRef<Path>, profile: &FilePeerClientProfile,
        minimum_revision: u64) -> Result<FileClientInspection, FileClientError>
    {
        FileActorClient::read(directory, &profile.inner, minimum_revision)
    }
    pub fn revision(&self) -> u64 { self.client.revision() }
    pub fn state(&self) -> &ActorClientState { self.client.state() }
    pub fn profile(&self) -> &FilePeerClientProfile { &self.profile }

    /// Reuse PeerPolicy::verify on THIS socket before mode changes, protocol work
    /// or moving the original client. An error drops only the candidate socket
    /// and returns the still-locked client. No auto-retry, DNS or path lookup.
    ///
    /// UID/GID-only policies admit every process with those credentials. Optional
    /// PID matches connection-time PID, not executable identity or an FD recipient.
    pub fn connect(self, socket: UnixStream) -> Result<FilePeerConnection, FilePeerConnectFailure> {
        let credentials = match self.profile.policy.verify(&socket) {
            Ok(credentials) => credentials,
            Err(error) => {
                let refusal = if error.kind() == io::ErrorKind::PermissionDenied {
                    PeerRefusal::CredentialsRejected
                } else { PeerRefusal::Io { stage: PeerSetupStage::Credentials, kind: error.kind() } };
                return Err(FilePeerConnectFailure { refusal, client: self });
            }
        };
        if let Err(error) = socket.set_nonblocking(true) {
            return Err(FilePeerConnectFailure { refusal: PeerRefusal::Io {
                stage: PeerSetupStage::Nonblocking, kind: error.kind(),
            }, client: self });
        }
        Ok(FilePeerConnection { connection: self.client.connect(socket),
            profile: self.profile, credentials })
    }
}

/// Candidate refusal is not a request outcome. The original owner, proposals,
/// journal revision and work counters remain available for a corrected connect.
#[derive(Debug)]
#[must_use = "retain the original locked client after a refused candidate"]
pub struct FilePeerConnectFailure {
    pub refusal: PeerRefusal,
    pub client: FilePeerClient,
}

/// No raw stream, unchecked reconnect or mutable peer-policy accessor. Every
/// command, write-ahead step, bounded drive, response and failure uses the
/// ORIGINAL durable client; these credentials grant no effect permission.
pub struct FilePeerConnection {
    connection: FileActorConnection<UnixStream>,
    profile: FilePeerClientProfile,
    credentials: PeerCredentials,
}
impl fmt::Debug for FilePeerConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilePeerConnection").field("revision", &self.revision())
            .field("credentials", &self.credentials).field("unavailable", &self.unavailable()).finish()
    }
}
impl FilePeerConnection {
    /// Historical credentials of this socket's connection, not of each sender.
    pub fn credentials(&self) -> PeerCredentials { self.credentials }
    pub fn revision(&self) -> u64 { self.connection.revision() }
    pub fn unavailable(&self) -> bool { self.connection.unavailable() }
    pub fn state(&self) -> Result<&ActorClientState, FileClientError> { self.connection.state() }
    pub fn phase(&self) -> Result<Option<ClientPhase>, FileClientError> { self.connection.phase() }
    pub fn submit(&mut self, revision: u64, request: u64, proposal: &ActorProposal) -> Result<(), FileClientError> {
        self.connection.submit(revision, request, proposal)
    }
    pub fn retry_submission(&mut self, revision: u64, request: u64) -> Result<(), FileClientError> {
        self.connection.retry_submission(revision, request)
    }
    pub fn poll(&mut self, revision: u64, request: u64) -> Result<(), FileClientError> {
        self.connection.poll(revision, request)
    }
    pub fn cancel(&mut self, revision: u64, request: u64) -> Result<(), FileClientError> {
        self.connection.cancel(revision, request)
    }
    pub fn step(&mut self, revision: u64) -> Result<ClientProgress, FileClientError> {
        self.connection.step(revision)
    }
    pub fn drive(&mut self, revision: u64, budget: ClientDriveBudget) -> Result<ClientDriveReport, FileClientError> {
        self.connection.drive(revision, budget)
    }
    /// Preserve the same immutable policy and original client. The next socket
    /// must verify afresh; no accepted credentials or ticket visibility transfer.
    pub fn into_disconnected(self) -> Result<FilePeerClient, FileClientError> {
        Ok(FilePeerClient { client: self.connection.into_disconnected()?, profile: self.profile })
    }
}

#[cfg(test)]
mod tests;
