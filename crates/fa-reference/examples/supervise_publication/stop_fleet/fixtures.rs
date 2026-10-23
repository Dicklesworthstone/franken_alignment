//! Real locked file owners and authenticated named Unix sockets. The test
//! scheduler only chooses interleavings; the original StopControlConnection
//! applies every stop and drain, with no asserted wire success in these fixtures.
use super::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight, FileOversightProfile,
    driver::FileSupervisedDriver,
    reviewer::peer::{PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
        control::{StopControlConnection, StopControlPhase, StopControlProgress}},
};
use std::cell::{Cell, RefCell};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) struct Root(pub PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-live-fleet-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("live fleet fixture cleanup: {error}"); }
    }
}
pub(super) fn private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(path).unwrap();
    file.write_all(bytes).unwrap();
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Behavior { Normal, Silent, Malformed, LostReceipt }
#[derive(Clone, Copy, Debug)]
pub(super) enum Seed { None, Reserved, Sent, Published }

pub(super) struct Server {
    pub id: u64,
    pub operation: u64,
    pub store: PathBuf,
    pub peer_path: PathBuf,
    pub profile: FileOversightProfile,
    pub driver: FileSupervisedDriver,
    pub reviewer: FileHumanReviewer,
    pub tick: ElapsedTick,
    pub accepted: usize,
    pub applied: usize,
    pub before: Vec<u8>,
    pub applied_image: Option<Vec<u8>>,
    listener: UnixListener,
    client_policy: PeerPolicy,
    connection: Option<StopControlConnection<UnixStream>>,
    silent: Option<UnixStream>,
    behavior: Behavior,
}
impl Server {
    fn new(root: &Root, id: u64, behavior: Behavior, seed: Seed) -> Self {
        let directory = root.0.join(format!("p{id}"));
        DirBuilder::new().mode(0o700).create(&directory).unwrap();
        let store = root.0.join(format!("domain-{id}"));
        let mut profile = fixture::profile();
        profile.delivery.scope.run = id;
        profile.delivery.scope.authority = 100 + id;
        profile.delivery.clock_domain = crate::config::CLOCK_DOMAIN;
        profile.delivery.limits.events = 128;
        let (mut host, reviewer) = FileOversight::create(&store, profile.clone()).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        if !matches!(seed, Seed::None) { seed_effect(&mut host, &reviewer, &profile, seed); }
        let operation = 1000 + id;
        let (a, _b) = UnixStream::pair().unwrap();
        let identity = PeerCredentials::observe(&a).unwrap();
        let client_policy = PeerPolicy::new(identity.uid(), identity.gid(), Some(identity.pid())).unwrap();
        let scope = profile.delivery.scope;
        let peer = format!(concat!(
            "{{\"version\":1,\"clock\":\"unix_milliseconds\",",
            "\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{}}},",
            "\"reviewer_id\":77,\"socket_directory\":\"{}\",",
            "\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
            "\"reviewer\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
            "\"candidate_limit\":4,\"runtime_ms\":1000,\"poll_ms\":1}}"),
            scope.tenant, scope.principal, scope.run, scope.branch, scope.authority, directory.display(),
            identity.uid(), identity.gid(), identity.pid(), identity.uid(), identity.gid(), identity.pid());
        let peer_path = root.0.join(format!("peer-{id}.json"));
        private(&peer_path, peer.as_bytes());
        let p = crate::peers::PeerProfile::decode(peer.as_bytes()).unwrap();
        let path = crate::workflow::control::socket_path(&p, operation);
        let listener = UnixListener::bind(&path).unwrap();
        p.secure_socket(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let before = fs::read(store.join("delivery.bin")).unwrap();
        let (_, driver) = host.into_supervised_driver();
        Self { id, operation, store, peer_path, profile, driver, reviewer, tick: ElapsedTick(2),
            accepted: 0, applied: 0, before, applied_image: None, listener, client_policy, connection: None,
            silent: None, behavior }
    }
    pub fn image(&self) -> Vec<u8> { fs::read(self.store.join("delivery.bin")).unwrap() }
    pub fn assert_locked(&self) {
        assert!(matches!(FileOversight::open(&self.store, self.profile.clone()), Err(JournalError::Busy)));
    }
    fn poll(&mut self) {
        if self.accepted == 0 {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
                Err(error) => panic!("fixture accept: {error}"),
            };
            self.accepted += 1;
            stream.set_nonblocking(true).unwrap();
            match self.behavior {
                Behavior::Silent => { self.silent = Some(stream); return; }
                Behavior::Malformed => {
                    let mut stream = stream;
                    stream.write_all(&[0xff; 104]).unwrap();
                    self.silent = Some(stream); return;
                }
                Behavior::Normal | Behavior::LostReceipt => {}
            }
            // The independently provisioned client identity is fixed before
            // the listener accepts. Incoming credentials cannot choose policy.
            let verified = VerifiedReviewerSocket::verify(stream, self.client_policy).unwrap();
            let host = self.driver.supervisor().host().unwrap();
            self.connection = Some(verified.into_stop_connection(&host, &self.reviewer,
                self.operation, [self.id as u8; 32]).unwrap());
        }
        let Some(connection) = &mut self.connection else { return; };
        if matches!(connection.phase(), StopControlPhase::Complete | StopControlPhase::Failed) { return; }
        let tick = self.tick;
        match connection.step(&mut self.driver, || tick) {
            Ok(StopControlProgress::Applied) => {
                self.applied += 1;
                self.applied_image = Some(self.image());
                if self.behavior == Behavior::LostReceipt { self.connection = None; }
            }
            Ok(_) => {}
            Err(_) => {
                // A closed client can lose either an offer or a receipt. The
                // native connection retains any original application separately.
            }
        }
    }
}

fn seed_effect(host: &mut FileOversight, reviewer: &FileHumanReviewer,
    profile: &FileOversightProfile, seed: Seed)
{
    let mut spec = fixture::spec(host, b"fleet publish");
    spec.scope = profile.delivery.scope;
    let action = host.propose(host.revision(), 1, spec, fixture::snapshot()).unwrap();
    let inputs = fixture::inputs(&action, b"original complete evidence");
    fixture::review_existing(host, 1, 101, &inputs);
    let automatic = host.authorize(host.revision(), 1, &inputs, fixture::snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    if matches!(seed, Seed::Sent | Seed::Published) {
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, fixture::snapshot()).unwrap();
    }
    if matches!(seed, Seed::Published) { host.publish(host.revision(), 1).unwrap(); }
}

pub(super) struct Fixture {
    pub servers: RefCell<Vec<Server>>,
    pub root: Root,
    pub ticks: Cell<u64>,
    pub checks: Cell<usize>,
}
impl Fixture {
    pub fn new(cases: &[(Behavior, Seed)]) -> Self {
        let root = Root::new();
        let servers = cases.iter().enumerate().map(|(index, &(behavior, seed))|
            Server::new(&root, index as u64 + 1, behavior, seed)).collect();
        let fixture = Self { servers: RefCell::new(servers), root, ticks: Cell::new(0), checks: Cell::new(0) };
        private(&fixture.manifest(), fixture.document().as_bytes());
        fixture
    }
    pub fn manifest(&self) -> PathBuf { self.root.0.join("fleet.json") }
    pub fn document(&self) -> String {
        let members = self.servers.borrow().iter().map(|server| format!(
            "{{\"id\":\"{}\",\"operation\":\"{}\",\"peer_profile\":\"{}\"}}",
            server.id, server.operation, server.peer_path.display())).collect::<Vec<_>>().join(",");
        format!("{{\"schema_version\":\"fa.live-fleet-stop/1\",\"runtime_ms\":1000,\"poll_ms\":1,\"steps_per_peer\":2048,\"members\":[{members}]}}")
    }
    pub fn args(&self) -> Vec<String> { vec!["stop-fleet".into(), self.manifest().display().to_string()] }
    pub fn elapsed(&self) -> Duration {
        self.checks.set(self.checks.get() + 1);
        self.servers.borrow_mut().iter_mut().for_each(Server::poll);
        Duration::from_millis(self.ticks.get())
    }
    pub fn pause(&self, delay: Duration) { self.ticks.set(self.ticks.get() + delay.as_millis() as u64); }
    pub fn plan(&self) -> Plan { Plan::read(&self.manifest(), &mut || Duration::ZERO).unwrap() }
    pub fn drive(&self, plan: &Plan) -> Report {
        drive(plan, &mut || self.elapsed(), &mut |member: &Member|
            member.peer.connect_stop_client(member.operation), &mut |delay| self.pause(delay)).unwrap()
    }
    pub fn command(&self, out: &mut impl Write) -> Result<(), String> {
        command_with(&self.args(), out, &mut || self.elapsed(), &mut |delay| self.pause(delay))
    }
}
