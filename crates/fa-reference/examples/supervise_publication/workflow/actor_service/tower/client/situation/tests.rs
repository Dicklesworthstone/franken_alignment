//! Real native situation captures over the same authenticated client transport.
//! Deliberately altered replies below exercise the wire reader, not native proof.
use super::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorInspectionGrant;
use fa_reference::action::consequence::oversight::actor::{ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use fa_reference::address_resolver::{CallerScope, ResolverPlane};
use fa_reference::observation_algebra::AuthorityKind;
use fa_reference::strict_json::{self, Limits};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{MAX_REQUEST_BYTES, TowerAccess, decode_request, encode_request};
use std::fs;
use std::io::{self, Read};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[path = "../../../../../../../tests/support/file_oversight_actor.rs"]
mod native;

#[path = "tests/continuation.rs"]
mod continuation;
#[path = "tests/validation.rs"]
mod validation;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const GENERATION: u64 = 11;

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(crate::test_directory::create(format!(
            "fa-sit-client-{}-{stamp}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
        )))
    }

    fn socket(&self) -> PathBuf {
        self.0.join("operator.sock")
    }

    fn settings(&self, supervisor_pid: u32, exchanges: u64) -> PathBuf {
        let peer = credentials();
        let scope = native::profile().delivery.scope;
        let document = format!(
            concat!(
                "{{\"schema\":\"fa.tower-operator-service/1\",",
                "\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{}}},",
                "\"socket\":\"{}\",",
                "\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
                "\"operator\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
                "\"candidate_limit\":2,\"connection_limit\":2,\"exchange_limit\":{},",
                "\"runtime_ms\":10000,\"poll_ms\":1,\"inspection_queries\":4,",
                "\"explanation_queries\":0,\"explanation_replay_events\":0,",
                "\"explanation_disclosure\":\"none\"}}"
            ),
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            self.socket().display(),
            peer.uid(),
            peer.gid(),
            supervisor_pid,
            peer.uid(),
            peer.gid(),
            peer.pid(),
            exchanges,
        );
        let path = self.0.join("operator.json");
        fs::write(&path, document).unwrap();
        path
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("situation client fixture cleanup: {error}");
        }
    }
}

fn credentials() -> PeerCredentials {
    let (socket, _peer) = UnixStream::pair().unwrap();
    PeerCredentials::observe(&socket).unwrap()
}

fn mount() -> Address {
    let scope = native::profile().delivery.scope;
    Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(GENERATION),
    )
    .unwrap()
}

fn args(profile: &Path, max_pages: usize) -> Vec<String> {
    vec![
        "tower-situation".into(),
        profile.display().to_string(),
        mount().canonical_string(),
        "--max-pages".into(),
        max_pages.to_string(),
    ]
}

fn bounded_socket(listener: &UnixListener) -> UnixStream {
    listener.set_nonblocking(true).unwrap();
    let started = Instant::now();
    let socket = loop {
        match listener.accept() {
            Ok((socket, _)) => break socket,
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    && started.elapsed() < Duration::from_secs(10) =>
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("bounded fixture accept failed: {error}"),
        }
    };
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    socket
}

fn read_document(socket: &mut UnixStream) -> Vec<u8> {
    let mut document = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        socket.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' {
            return document;
        }
        assert!(document.len() < MAX_REQUEST_BYTES);
        document.push(byte[0]);
    }
}

fn send(socket: &mut UnixStream, response: &str) {
    socket.write_all(response.as_bytes()).unwrap();
    socket.write_all(b"\n").unwrap();
    socket.flush().unwrap();
}

fn disconnected(socket: &mut UnixStream, listener: &UnixListener) {
    assert_eq!(
        socket.read(&mut [0_u8; 1]).unwrap(),
        0,
        "unexpected next request"
    );
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock,
        "client reconnected after its original exchange ended"
    );
}

fn request(page: usize, cursor: Option<u64>) -> TowerRequest {
    let mut request = TowerRequest::new(
        format!("tower-situation-page-{page}"),
        mount(),
        Verb::Situation,
    );
    request.arguments = cursor.map_or(ReadArguments::None, |cursor| ReadArguments::SituationPage {
        cursor,
    });
    request
}

fn access() -> TowerAccess {
    TowerAccess::new(
        CallerScope {
            caller_tenant: native::profile().delivery.scope.tenant.to_string(),
            allowed_planes: BTreeSet::from([ResolverPlane::ControlLedger]),
            granted_authorities: BTreeSet::from([AuthorityKind::Verify]),
        },
        true,
        Vec::new(),
        0,
    )
}

struct Fixture {
    supervisor: Option<native::Supervisor>,
    actor: native::Port,
    proposal: ActorProposal,
    tower: PublicationTower,
    grant: Option<FileSupervisorInspectionGrant>,
    _storage: native::Directory,
}

impl Fixture {
    fn new(attempts: u64, queries: usize) -> Self {
        let storage = native::Directory::new();
        let mut config = native::profile();
        config.delivery.max_attempts = 64;
        let (mut host, _reviewer) = FileOversight::create(storage.store(), config).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let proposal = native::proposal(&native::oversight::spec(&host, b"published"));
        let (actor, mut supervisor) = host.into_actor_gateway();
        for request in 1001..1001 + attempts {
            native::observe(&mut supervisor);
            actor.submit(request, &proposal).unwrap();
        }
        let mut tower =
            PublicationTower::new(mount(), actor.clone(), PublicationLimits::default()).unwrap();
        let grant = if queries == 0 {
            None
        } else {
            let (inspection, grant) = supervisor.inspection_port(queries).unwrap();
            tower.set_inspection(Some(inspection)).unwrap();
            Some(grant)
        };
        Self {
            supervisor: Some(supervisor),
            actor,
            proposal,
            tower,
            grant,
            _storage: storage,
        }
    }

    fn reply(&mut self, document: &[u8]) -> PublicationResponse {
        let before = self
            .supervisor
            .as_ref()
            .map(|supervisor| supervisor.host().unwrap().inspect());
        let response = self.tower.exchange(&access(), document);
        let after = self
            .supervisor
            .as_ref()
            .map(|supervisor| supervisor.host().unwrap().inspect());
        assert_eq!(after, before, "situation query changed the original owner");
        response
    }

    fn advance(&mut self) {
        let supervisor = self.supervisor.as_mut().unwrap();
        {
            let mut host = supervisor.host_mut().unwrap();
            let revision = host.revision();
            host.cancel(revision, 1).unwrap();
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(2)).unwrap();
        }
        native::observe(supervisor);
        self.actor.submit(1034, &self.proposal).unwrap();
        assert_eq!(
            supervisor
                .host()
                .unwrap()
                .inspect()
                .control
                .ledger
                .stages
                .len(),
            34
        );
    }

    fn remaining(&self) -> usize {
        self.grant.as_ref().unwrap().remaining_queries()
    }
}

fn native_page(
    response: &PublicationResponse,
) -> &fa_reference::action::consequence::delivery::persistent::requests::actor::FileInspectionPage {
    let PublicationResult::Situation(Ok(Knowledge::Known { value, .. })) = &response.result else {
        panic!("expected a native Known situation page: {response:?}");
    };
    value
}

fn parse(value: &str) -> Json {
    strict_json::parse(value.as_bytes(), Limits::default()).unwrap()
}

fn replace_once(original: &str, old: &str, new: &str) -> String {
    assert_eq!(
        original.matches(old).count(),
        1,
        "mutation does not select exactly one field"
    );
    original.replacen(old, new, 1)
}

fn emitted(output: &[u8]) -> Vec<Json> {
    std::str::from_utf8(output)
        .unwrap()
        .lines()
        .map(parse)
        .collect()
}
