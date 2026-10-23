//! One original elapsed budget through authenticated sockets and the original
//! durable stop owner. The virtual scheduler chooses interleavings; native
//! receipt bytes and durable acknowledgment come from StopControlConnection.
use super::linux::request_with;
use crate::peers::PeerProfile;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight,
    driver::FileSupervisedDriver,
    reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
        control::{StopControlConnection, StopControlPhase, StopControlProgress},
    },
};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::strict_json::{self, Json, Limits};
use std::cell::{Cell, RefCell};
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

#[path = "../../../tests/support/file_oversight.rs"]
mod fixture;

const OPERATION: u64 = 7;
const RUNTIME_MS: u64 = 10;

struct Native {
    root: fixture::Directory,
    peer: PeerProfile,
    listener: UnixListener,
    policy: PeerPolicy,
    driver: RefCell<FileSupervisedDriver>,
    reviewer: FileHumanReviewer,
    connection: RefCell<Option<StopControlConnection<UnixStream>>>,
    ticks: Cell<u64>,
    checks: Cell<usize>,
    accepted: Cell<usize>,
    applied: Cell<usize>,
    before: Vec<u8>,
}

impl Native {
    fn new() -> Self {
        let root = fixture::Directory::new();
        let directory = root.0.join("peer");
        DirBuilder::new().mode(0o700).create(&directory).unwrap();
        let mut profile = fixture::profile();
        profile.delivery.clock_domain = crate::config::CLOCK_DOMAIN;
        let (mut host, reviewer) = FileOversight::create(root.store(), profile.clone()).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let keys = fixture::ready(&mut host, &reviewer, 1, b"original publication");
        assert_eq!(
            host.inspect().control.ledger.reserved,
            keys.action.spec().units
        );
        let (a, _b) = UnixStream::pair().unwrap();
        let credentials = PeerCredentials::observe(&a).unwrap();
        let policy = PeerPolicy::new(
            credentials.uid(),
            credentials.gid(),
            Some(credentials.pid()),
        )
        .unwrap();
        let scope = profile.delivery.scope;
        let bytes = format!(
            concat!(
                "{{\"version\":1,\"clock\":\"unix_milliseconds\",",
                "\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{}}},",
                "\"reviewer_id\":77,\"socket_directory\":\"{}\",",
                "\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
                "\"reviewer\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
                "\"candidate_limit\":4,\"runtime_ms\":{},\"poll_ms\":1}}"
            ),
            scope.tenant,
            scope.principal,
            scope.run,
            scope.branch,
            scope.authority,
            directory.display(),
            credentials.uid(),
            credentials.gid(),
            credentials.pid(),
            credentials.uid(),
            credentials.gid(),
            credentials.pid(),
            RUNTIME_MS
        );
        let peer = PeerProfile::decode(bytes.as_bytes()).unwrap();
        let path = crate::workflow::control::socket_path(&peer, OPERATION);
        let listener = UnixListener::bind(&path).unwrap();
        peer.secure_socket(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let before = fs::read(root.store().join("delivery.bin")).unwrap();
        let (_, driver) = host.into_supervised_driver();
        Self {
            root,
            peer,
            listener,
            policy,
            driver: RefCell::new(driver),
            reviewer,
            connection: RefCell::new(None),
            ticks: Cell::new(0),
            checks: Cell::new(0),
            accepted: Cell::new(0),
            applied: Cell::new(0),
            before,
        }
    }

    fn poll(&self) {
        let mut connection = self.connection.borrow_mut();
        if self.accepted.get() == 0 {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
                Err(error) => panic!("native stop accept: {error}"),
            };
            self.accepted.set(1);
            stream.set_nonblocking(true).unwrap();
            let verified = VerifiedReviewerSocket::verify(stream, self.policy).unwrap();
            let driver = self.driver.borrow();
            let host = driver.supervisor().host().unwrap();
            *connection = Some(
                verified
                    .into_stop_connection(&host, &self.reviewer, OPERATION, [7; 32])
                    .unwrap(),
            );
        }
        let Some(connection) = connection.as_mut() else {
            return;
        };
        if matches!(
            connection.phase(),
            StopControlPhase::Complete | StopControlPhase::Failed
        ) {
            return;
        }
        if matches!(
            connection.step(&mut self.driver.borrow_mut(), || ElapsedTick(2)),
            Ok(StopControlProgress::Applied)
        ) {
            self.applied.set(self.applied.get() + 1);
        }
    }

    fn elapsed(&self) -> Duration {
        self.checks.set(self.checks.get() + 1);
        self.poll();
        Duration::from_millis(self.ticks.get())
    }

    fn pause(&self, delay: Duration) {
        self.ticks
            .set(self.ticks.get() + u64::try_from(delay.as_millis()).unwrap());
    }

    fn image(&self) -> Vec<u8> {
        fs::read(self.root.store().join("delivery.bin")).unwrap()
    }

    fn assert_stopped(&self, json: &Json) {
        let driver = self.driver.borrow();
        let host = driver.supervisor().host().unwrap();
        let snapshot = host.inspect();
        let stop = snapshot.stop.unwrap();
        assert_eq!(stop.request().operation, OPERATION);
        assert!(host.stop_progress().unwrap().drained());
        assert_eq!(snapshot.executions, 0);
        assert_eq!(snapshot.control.ledger.reserved, 0);
        assert_eq!(snapshot.control.ledger.charged, 0);
        assert_eq!(
            json.get("status").unwrap().as_str(),
            Some("stopped_drained")
        );
        assert_eq!(json.get("acknowledged").unwrap().as_bool(), Some(true));
        assert_eq!(json.get("drained").unwrap().as_bool(), Some(true));
        assert_eq!(
            json.get("control_sequence").unwrap().as_str(),
            Some(stop.control_sequence().to_string().as_str())
        );
        assert_eq!(self.accepted.get(), 1);
        assert_eq!(self.applied.get(), 1);
        assert_ne!(self.image(), self.before);
        let mut profile = fixture::profile();
        profile.delivery.clock_domain = crate::config::CLOCK_DOMAIN;
        assert!(matches!(
            FileOversight::open(self.root.store(), profile),
            Err(JournalError::Busy)
        ));
    }
}

fn parsed(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, Limits::default()).unwrap()
}

#[test]
fn timely_authenticated_stop_preserves_native_receipt_and_original_accounting() {
    let f = Native::new();
    let mut out = Vec::new();
    request_with(
        &f.peer,
        OPERATION,
        &mut out,
        &mut || f.elapsed(),
        || f.peer.connect_stop_client(OPERATION),
        &mut |delay| f.pause(delay),
    )
    .unwrap();
    f.assert_stopped(&parsed(&out));
    let acknowledged = f.image();
    for _ in 0..3 {
        f.poll();
    }
    assert_eq!(f.applied.get(), 1);
    assert_eq!(f.image(), acknowledged);
}

#[test]
fn connection_consuming_original_deadline_never_sends_a_stop_request() {
    let f = Native::new();
    let mut out = Vec::new();
    let error = request_with(
        &f.peer,
        OPERATION,
        &mut out,
        &mut || f.elapsed(),
        || {
            let client = f.peer.connect_stop_client(OPERATION)?;
            f.ticks.set(RUNTIME_MS);
            Ok(client)
        },
        &mut |delay| f.pause(delay),
    )
    .unwrap_err();
    assert!(error.contains("deadline reached"));
    let json = parsed(&out);
    assert_eq!(json.get("status").unwrap().as_str(), Some("unconfirmed"));
    assert_eq!(
        json.get("request_may_have_been_sent").unwrap().as_bool(),
        Some(false)
    );
    assert_eq!(json.get("acknowledged").unwrap().as_bool(), Some(false));
    for _ in 0..3 {
        f.poll();
    }
    assert_eq!(f.accepted.get(), 1);
    assert_eq!(f.applied.get(), 0);
    assert_eq!(f.image(), f.before);
    assert!(
        f.driver
            .borrow()
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .stop
            .is_none()
    );
}

#[test]
fn receipt_crossing_exact_deadline_retains_native_facts_without_timely_success() {
    let control = Native::new();
    request_with(
        &control.peer,
        OPERATION,
        &mut Vec::new(),
        &mut || control.elapsed(),
        || control.peer.connect_stop_client(OPERATION),
        &mut |delay| control.pause(delay),
    )
    .unwrap();
    let final_check = control.checks.get();
    let f = Native::new();
    let mut out = Vec::new();
    let error = request_with(
        &f.peer,
        OPERATION,
        &mut out,
        &mut || {
            let elapsed = f.elapsed();
            if f.checks.get() >= final_check {
                Duration::from_millis(RUNTIME_MS)
            } else {
                elapsed
            }
        },
        || f.peer.connect_stop_client(OPERATION),
        &mut |delay| f.pause(delay),
    )
    .unwrap_err();
    assert!(error.contains("deadline reached"));
    assert!(error.contains("StoppedDrained"));
    f.assert_stopped(&parsed(&out));
    let acknowledged = f.image();
    for _ in 0..3 {
        f.poll();
    }
    assert_eq!(f.applied.get(), 1);
    assert_eq!(f.image(), acknowledged);
}
