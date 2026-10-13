//! Actual Linux socket credentials and original client/disk behavior.
use super::*;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::persistent::{JournalIo, MAX_JOURNAL_BYTES};
use crate::action::consequence::oversight::actor::Knowledge;
use crate::action::consequence::oversight::actor_wire::{Command, WireResponse, encode_command};
use crate::action::consequence::oversight::actor_wire::client::ClientError;
use std::{fs, io::{Read, Write}, os::fd::OwnedFd, path::PathBuf,
    sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-peer-client-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("client") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("peer client cleanup: {error}"); } }
}
fn base() -> FileClientProfile {
    FileClientProfile::new(b"independent-server/raw-v1".to_vec(),
        ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap()
}
fn policy(socket: &UnixStream) -> PeerPolicy {
    let observed = PeerCredentials::observe(socket).unwrap();
    PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap()
}
fn proposal() -> ActorProposal {
    ActorProposal { target: ResolvedTarget { adapter: 1, object: 2, contract_version: 1,
        expected_version: 1, generation: 1 }, payload: b"exact peer-checked request".to_vec(),
        expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }
}
fn response() -> WireResponse { WireResponse { request: Some(7), result: Ok(Knowledge::Pending { request: 7 }) } }
fn exchange(client: &mut FilePeerConnection, server: &mut UnixStream) {
    let r = client.revision(); let sent = client.drive(r, ClientDriveBudget::default()).unwrap();
    assert_eq!(sent.progress, ClientProgress::Blocked);
    let mut expected = encode_command(&Command::Submit { request: 7, proposal: proposal() }).unwrap();
    expected.push(b'\n'); let mut actual = vec![0; expected.len()];
    server.read_exact(&mut actual).unwrap(); assert_eq!(actual, expected);
    let mut reply = response().encode(); reply.push(b'\n'); server.write_all(&reply).unwrap();
    let r = client.revision();
    assert_eq!(client.drive(r, ClientDriveBudget::default()).unwrap().progress, ClientProgress::Response(response()));
}
fn nonsocket() -> UnixStream {
    let descriptor: OwnedFd = fs::File::open("/dev/null").unwrap().into();
    UnixStream::from(descriptor)
}

#[test]
fn peer_binding_is_canonical_exact_and_keeps_original_limits() {
    let (socket, _) = UnixStream::pair().unwrap(); let policy = policy(&socket);
    let raw = base(); let selected = raw.clone().with_peer_policy(policy).unwrap();
    let mut vector = b"FACPEER\x01".to_vec();
    vector.extend_from_slice(&(raw.binding().len() as u32).to_be_bytes()); vector.extend_from_slice(raw.binding());
    vector.extend_from_slice(&policy.uid().to_be_bytes()); vector.extend_from_slice(&policy.gid().to_be_bytes());
    vector.push(1); vector.extend_from_slice(&policy.pid().unwrap().to_be_bytes());
    assert_eq!(selected.inner.binding(), vector);
    assert_eq!(selected, raw.clone().with_peer_policy(policy).unwrap());
    assert_eq!(selected.limits(), raw.limits()); assert_eq!(selected.image_bytes(), raw.image_bytes());
    let no_pid = raw.with_peer_policy(PeerPolicy::new(policy.uid(), policy.gid(), None).unwrap()).unwrap();
    assert_ne!(selected, no_pid);
    assert!(!format!("{selected:?}").contains("independent-server"));
    let maximum = FileClientProfile::new(vec![b'x'; MAX_BINDING_BYTES], ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap();
    assert!(maximum.with_peer_policy(policy).is_err());
}

#[test]
fn each_wrong_uid_gid_or_pid_closes_candidate_without_bytes_or_disk_changes() {
    for field in 0..3 {
        let root = Directory::new(); let (socket, mut server) = UnixStream::pair().unwrap();
        server.set_nonblocking(true).unwrap(); let p = policy(&socket);
        let wrong = match field {
            0 => PeerPolicy::new(if p.uid() == 0 { 1 } else { 0 }, p.gid(), p.pid()),
            1 => PeerPolicy::new(p.uid(), if p.gid() == 0 { 1 } else { 0 }, p.pid()),
            _ => PeerPolicy::new(p.uid(), p.gid(), Some(if p.pid() == Some(1) { 2 } else { 1 })),
        }.unwrap();
        let profile = base().with_peer_policy(wrong).unwrap();
        let client = FilePeerClient::create(root.store(), profile.clone()).unwrap();
        let before = root.bytes(); let work = client.state().work();
        let refusal = client.connect(socket).unwrap_err();
        assert_eq!(refusal.refusal, PeerRefusal::CredentialsRejected);
        assert_eq!(refusal.client.revision(), 0); assert_eq!(refusal.client.state().work(), work);
        assert_eq!(refusal.client.state().request_count(), 0); assert_eq!(root.bytes(), before);
        assert_eq!(server.read(&mut [0; 1]).unwrap(), 0, "refused socket was not closed");
        assert!(FilePeerClient::open(root.store(), profile, 0).is_err(), "refusal lost original exclusive custody");
        // Positive control uses the same request and actual server, with the
        // independently selected matching contract in a different client domain.
        let good_root = Directory::new(); let (socket, mut server) = UnixStream::pair().unwrap();
        let client = FilePeerClient::create(good_root.store(), base().with_peer_policy(p).unwrap()).unwrap();
        let mut client = client.connect(socket).unwrap(); client.submit(0, 7, &proposal()).unwrap();
        exchange(&mut client, &mut server);
    }
}

#[test]
fn nonsocket_refusal_returns_original_client_for_successful_corrected_connection() {
    let root = Directory::new(); let (socket, mut server) = UnixStream::pair().unwrap();
    let expected = policy(&socket); let profile = base().with_peer_policy(expected).unwrap();
    let client = FilePeerClient::create(root.store(), profile).unwrap(); let before = root.bytes();
    let failure = client.connect(nonsocket()).unwrap_err();
    assert!(matches!(failure.refusal, PeerRefusal::Io { stage: PeerSetupStage::Credentials, .. }));
    assert_eq!(root.bytes(), before); assert_eq!(failure.client.revision(), 0);
    let mut client = failure.client.connect(socket).unwrap();
    assert_eq!(client.credentials().pid(), expected.pid().unwrap());
    assert_eq!(client.credentials().uid(), expected.uid());
    assert_eq!(root.bytes(), before, "attachment is not a journal transaction");
    client.submit(0, 7, &proposal()).unwrap(); exchange(&mut client, &mut server);
    let work = client.state().unwrap().work(); let revision = client.revision();
    let client = client.into_disconnected().unwrap();
    let failure = client.connect(nonsocket()).unwrap_err();
    assert_eq!(failure.client.state().work(), work); assert_eq!(failure.client.revision(), revision);
    let (socket, mut server) = UnixStream::pair().unwrap();
    let mut client = failure.client.connect(socket).unwrap();
    assert_eq!(client.poll(revision, 7), Err(FileClientError::Client(ClientError::TicketUnavailable)));
    client.retry_submission(revision, 7).unwrap(); exchange(&mut client, &mut server);
    assert_eq!(client.state().unwrap().request_count(), 1);
    assert!(client.state().unwrap().work().exchanges > work.exchanges);
}

#[test]
fn changing_peer_requirements_or_transport_profile_cannot_clean_staging_or_restore_a_client() {
    let root = Directory::new(); let (socket, mut server) = UnixStream::pair().unwrap();
    let expected = policy(&socket); let profile = base().with_peer_policy(expected).unwrap();
    let mut client = FilePeerClient::create(root.store(), profile.clone()).unwrap().connect(socket).unwrap();
    client.submit(0, 7, &proposal()).unwrap(); exchange(&mut client, &mut server);
    let revision = client.revision(); let work = client.state().unwrap().work(); drop(client);
    let before = root.bytes(); let staging = root.store().join("delivery.pending");
    fs::write(&staging, b"unacknowledged staged evidence").unwrap();
    let weaker = base().with_peer_policy(PeerPolicy::new(expected.uid(), expected.gid(), None).unwrap()).unwrap();
    assert!(FilePeerClient::open(root.store(), weaker, 0).is_err());
    let other_transport = FileClientProfile::new(b"different-server/stream-v1".to_vec(),
        ClientSessionLimits::default(), MAX_JOURNAL_BYTES).unwrap().with_peer_policy(expected).unwrap();
    assert!(FilePeerClient::open(root.store(), other_transport, 0).is_err());
    assert!(FileActorClient::open(root.store(), base(), 0).is_err());
    assert!(FilePeerClient::open(root.store(), profile.clone(), revision + 1).is_err());
    assert_eq!(root.bytes(), before); assert_eq!(fs::read(&staging).unwrap(), b"unacknowledged staged evidence");
    let selected = FilePeerClient::read(root.store(), &profile, revision).unwrap();
    assert_eq!(selected.last_response(7), Some(&response())); assert!(staging.exists());
    let client = FilePeerClient::open(root.store(), profile, revision).unwrap();
    assert_eq!(client.state().work(), work); assert!(!staging.exists());
    assert_eq!(client.state().original_proposal(7), Some(&proposal()));
    let (socket, mut server) = UnixStream::pair().unwrap(); let mut client = client.connect(socket).unwrap();
    assert_eq!(client.cancel(revision, 7), Err(FileClientError::Client(ClientError::TicketUnavailable)));
    client.retry_submission(revision, 7).unwrap(); exchange(&mut client, &mut server);
}

#[test]
fn verified_peer_cannot_bypass_the_original_failed_intent_write() {
    let root = Directory::new(); let (socket, mut server) = UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap(); let profile = base().with_peer_policy(policy(&socket)).unwrap();
    let mut client = FilePeerClient::create(root.store(), profile.clone()).unwrap().connect(socket).unwrap();
    client.submit(0, 7, &proposal()).unwrap();
    client.connection.owner.store.fail_once(JournalIo::Stage);
    assert!(client.drive(1, ClientDriveBudget::default()).is_err()); assert!(client.unavailable());
    assert!(client.state().is_err()); assert_eq!(server.read(&mut [0; 1]).unwrap(), 0);
    assert!(client.into_disconnected().is_err());
    let selected = FilePeerClient::read(root.store(), &profile, 1).unwrap();
    assert!(selected.last_response(7).is_none()); assert!(selected.reserved_work().is_none());
    let client = FilePeerClient::open(root.store(), profile, 1).unwrap();
    let (socket, mut server) = UnixStream::pair().unwrap(); let mut client = client.connect(socket).unwrap();
    assert_eq!(client.poll(1, 7), Err(FileClientError::Client(ClientError::TicketUnavailable)));
    client.retry_submission(1, 7).unwrap(); exchange(&mut client, &mut server);
}
