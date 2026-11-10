//! Native original journal pages over an authenticated Unix connection. Reply
//! mutations test rejection of misleading wire data, not native event evidence.

use super::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorJournalTailGrant;
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use fa_reference::address_resolver::{CallerScope, ResolverPlane};
use fa_reference::observation_algebra::AuthorityKind;
use fa_reference::strict_json::{self, Limits};
use fa_reference::tower::publication::{PublicationLimits, PublicationTower};
use fa_reference::tower::{MAX_REQUEST_BYTES, TowerAccess, decode_request, encode_request};
use std::fs;
use std::io::{self, Read};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[path = "../../../../../../../tests/support/file_oversight_actor.rs"]
mod native;
#[path = "tests/pagination.rs"]
mod pagination;
#[path = "tests/validation.rs"]
mod validation;

const GENERATION: u64 = 11;
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(crate::test_directory::create(format!(
            "fa-tail-client-{}-{stamp}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
        )))
    }

    fn socket(&self) -> PathBuf {
        self.0.join("operator.sock")
    }

    fn settings(
        &self,
        supervisor_pid: u32,
        exchanges: u64,
        queries: usize,
        events: usize,
    ) -> PathBuf {
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
                "\"runtime_ms\":10000,\"poll_ms\":1,\"inspection_queries\":0,",
                "\"explanation_queries\":0,\"explanation_replay_events\":0,",
                "\"explanation_disclosure\":\"none\",",
                "\"journal_tail\":{{\"queries\":{},\"events_per_page\":{},",
                "\"disclosure\":\"redacted_journal_events\"}}}}"
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
            queries,
            events
        );
        let path = self.0.join("operator.json");
        fs::write(&path, document).unwrap();
        path
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("journal tail client fixture cleanup: {error}");
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

fn args(profile: &Path, after_revision: u64, max_pages: usize) -> Vec<String> {
    vec![
        "tower-tail".into(),
        profile.display().to_string(),
        mount().canonical_string(),
        "--after-revision".into(),
        after_revision.to_string(),
        "--max-pages".into(),
        max_pages.to_string(),
    ]
}

fn request(page: usize, after_revision: u64, cursor: Option<u64>) -> TowerRequest {
    let mut request = TowerRequest::new(format!("tower-tail-page-{page}"), mount(), Verb::Tail);
    request.arguments = cursor.map_or(ReadArguments::JournalTail { after_revision }, |cursor| {
        ReadArguments::JournalTailPage { cursor }
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
    tower: PublicationTower,
    grant: Option<FileSupervisorJournalTailGrant>,
    _storage: native::Directory,
}

impl Fixture {
    fn new(ticks: u64, queries: usize, events: usize) -> Self {
        let storage = native::Directory::new();
        let (mut host, _reviewer) =
            FileOversight::create(storage.store(), native::profile()).unwrap();
        for tick in 1..=ticks {
            host.observe_time(host.revision(), ElapsedTick(tick))
                .unwrap();
        }
        assert_eq!(host.revision(), ticks);
        let (actor, supervisor) = host.into_actor_gateway();
        let mut tower =
            PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
        let grant = if queries == 0 {
            None
        } else {
            let (port, grant) = supervisor.journal_tail_port(queries, events).unwrap();
            tower.set_journal_tail(Some(port)).unwrap();
            Some(grant)
        };
        Self {
            supervisor: Some(supervisor),
            tower,
            grant,
            _storage: storage,
        }
    }

    fn reply(&mut self, bytes: &[u8]) -> String {
        let before = self
            .supervisor
            .as_ref()
            .map(|owner| owner.host().unwrap().inspect());
        let response = self.tower.exchange(&access(), bytes).to_json().unwrap();
        let after = self
            .supervisor
            .as_ref()
            .map(|owner| owner.host().unwrap().inspect());
        assert_eq!(after, before, "tail changed the original owner");
        response
    }

    fn advance(&mut self) {
        let mut host = self.supervisor.as_mut().unwrap().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(revision + 1))
            .unwrap();
    }

    fn remaining(&self) -> usize {
        self.grant.as_ref().unwrap().remaining_queries()
    }
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

fn parse(value: &str) -> Json {
    strict_json::parse(value.as_bytes(), Limits::default()).unwrap()
}

fn page(value: &Json) -> &Json {
    value
        .get("result")
        .unwrap()
        .get("value")
        .unwrap()
        .get("knowledge")
        .unwrap()
        .get("value")
        .unwrap()
}

fn basis(value: &Json) -> &Json {
    value
        .get("result")
        .unwrap()
        .get("value")
        .unwrap()
        .get("knowledge")
        .unwrap()
        .get("basis")
        .unwrap()
}

fn replace_once(original: &str, old: &str, new: &str) -> String {
    assert_eq!(
        original.matches(old).count(),
        1,
        "mutation must select exactly one field"
    );
    original.replacen(old, new, 1)
}
