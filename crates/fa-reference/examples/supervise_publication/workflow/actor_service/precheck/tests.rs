//! Real durable owners and local peer sockets. These fixtures test enforcement,
//! not source authenticity, detector quality or a qualified production runtime.
use super::*;
use super::super::prepare_host;
use fa_reference::Snapshot;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorPort, FilePrecheckOutcome,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    PrecheckRequest, PrecheckWireError, PrecheckWireResponse, encode_precheck_request,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::client::{
    PrecheckClientPhase, PrecheckPeerClient,
};
use fa_reference::action::consequence::oversight::actor::{ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource,
};
use fa_reference::action::consequence::oversight::policy_state::StateFreshness;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = crate::test_directory::create(format!("fa-precheck-service-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        for name in ["actors", "reviewers"] {
            fs::DirBuilder::new().mode(0o750).create(path.join(name)).unwrap();
        }
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("precheck service fixture cleanup: {error}");
        }
    }
}

struct Fixture {
    driver: FileSupervisedDriver,
    port: FileActorPort<FileOversight>,
    config: Config,
    actor: Profile,
    reviewer: PeerProfile,
    settings: Settings,
    _root: Directory,
}
impl Fixture {
    fn new(queries: usize, exchanges: usize) -> Self {
        let root = Directory::new();
        let mut config = Config::decode(include_bytes!("../../../../../fixtures/supervised_publication.json")).unwrap();
        config.store = root.0.join("store");
        config.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51,
            config.profile.delivery.scope, 1_048_576).unwrap();
        config.source_policy.freshness = StateFreshness::new(100).unwrap();
        config.timing.runtime_ms = 15_000;
        config.timing.poll_ms = 1;
        let data = EvidenceSnapshot::new(EvidenceIdentity {
            source: 51, generation: 1, scope: config.profile.delivery.scope,
        }, Snapshot { semantic_epoch: 1, complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]) },
            BTreeMap::from([("alpha".into(), b"private-helper-alpha".to_vec()),
                ("beta".into(), b"private-helper-beta".to_vec())])).unwrap();
        fs::write(root.0.join("evidence.json"), data.encode()).unwrap();
        let (socket, _) = UnixStream::pair().unwrap();
        let id = PeerCredentials::observe(&socket).unwrap();
        let policy = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap();
        let actor = Profile { request: 7, scope: config.profile.delivery.scope,
            socket: root.0.join("actors/actor.sock"), supervisor: policy, actor: policy,
            candidates: 8, connections: 4, exchanges: 1_024,
            runtime_ms: 15_000, poll_ms: 1, reply_ms: 20 };
        let scope = actor.scope;
        let reviewer = PeerProfile::decode(format!(
            r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":77,"socket_directory":"{}/reviewers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":8,"runtime_ms":15000,"poll_ms":1}}"#,
            scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
            root.0.display(), id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid()
        ).as_bytes()).unwrap();
        let settings = Settings::decode(profile_json(&root.0.join("actors/precheck.sock"), queries, exchanges).as_bytes()).unwrap();
        let (host, _) = prepare_host(&config, None, false).unwrap();
        let (port, driver) = host.into_supervised_driver();
        Self { driver, port, config, actor, reviewer, settings, _root: root }
    }
    fn start(&mut self) -> Service {
        Service::start(&self.settings, &self.actor, &self.reviewer, &mut self.config,
            &mut self.driver, &deadline(), &mut || ElapsedTick(1_000)).unwrap()
    }
    fn request(&self, query: u64, payload: &[u8]) -> PrecheckRequest {
        PrecheckRequest { query, proposal: ActorProposal {
            target: self.config.profile.delivery.target, payload: payload.to_vec(),
            units: 8, deadline: ElapsedTick(10_000), expected_policy_epoch: 0,
        } }
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(&self.config.store).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| {
                let bytes = fs::read(&path).unwrap(); (path, bytes)
            }).collect()
    }
    fn trace_case(&self, service: Option<&Service>) {
        eprintln!("precheck-service-fixture case={:?}; source_read_attempts={}; frames={:?}; retirement={:?}",
            std::thread::current().name(), self.config.source.status().read_attempts,
            service.map(|value| value.frames), service.and_then(|value| value.retirement));
    }
    fn query(&self, service: &mut Service, request: PrecheckRequest) -> PrecheckWireResponse {
        // Observe EOF from an earlier completed one-shot before accepting a new
        // explicit query. Neither call is allowed to refresh source or quotas.
        service.checkpoint(&self.driver, ElapsedTick(1_000));
        let socket = UnixStream::connect(&self.settings.socket).unwrap();
        let mut client = PrecheckPeerClient::connect(socket, self.actor.supervisor, request,
            self.settings.expected_policy_epoch).unwrap();
        for _ in 0..1_024 {
            let report = client.drive(DriveBudget::default()).unwrap();
            assert!(report.status.failure.is_none(), "{:?}", report.status);
            if report.status.closed() { break; }
            service.checkpoint(&self.driver, ElapsedTick(1_000));
        }
        assert_eq!(client.status().phase, PrecheckClientPhase::Complete);
        self.trace_case(Some(service));
        client.take_response().unwrap()
    }
}

fn profile_json(socket: &Path, queries: usize, exchanges: usize) -> String {
    format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":0,"disclosure_id":81,"query_limit":{},"exchange_limit":{},"candidate_limit":8,"connection_limit":4,"runtime_ms":15000}}"#,
        socket.display(), queries, exchanges)
}
fn deadline() -> Deadline {
    Deadline { logical: ElapsedTick(20_000), started: Instant::now(), wall: Duration::from_secs(15) }
}
fn known(response: &PrecheckWireResponse, expected: FilePrecheckOutcome, query: u64) {
    assert_eq!(response.query, Some(query));
    let Knowledge::Known { value, basis } = response.result.as_ref().unwrap() else {
        panic!("expected a native exact-precheck observation");
    };
    assert_eq!(*value, expected);
    assert_eq!((basis.disclosure, basis.query, basis.policy_epoch, basis.observed_at),
        (81, query, 0, ElapsedTick(1_000)));
    let bytes = String::from_utf8(response.encode().unwrap()).unwrap();
    assert!(bytes.contains("\"precheck\":true"));
    match expected {
        FilePrecheckOutcome::PrecheckPassed => assert!(bytes.contains("\"review_required\":true")),
        FilePrecheckOutcome::DeniedByPolicy => assert!(!bytes.contains("review_required")),
    }
    for secret in ["private-helper", "alpha", "beta", "permit", "source", "predicate"] {
        assert!(!bytes.contains(secret), "unexpected disclosure {secret}");
    }
}

fn raw_reply(socket: &mut UnixStream, service: &mut Service, driver: &FileSupervisedDriver) -> Vec<u8> {
    socket.set_nonblocking(true).unwrap();
    let mut response = Vec::new();
    for _ in 0..2_048 {
        service.checkpoint(driver, ElapsedTick(1_000));
        let mut bytes = [0_u8; 128];
        match socket.read(&mut bytes) {
            Ok(0) => break,
            Ok(count) => response.extend_from_slice(&bytes[..count]),
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => {}
            Err(error) => panic!("raw response failed: {error}"),
        }
        assert!(response.len() <= 1_025);
        if response.ends_with(b"\n") { break; }
    }
    assert!(response.ends_with(b"\n"), "missing complete raw response");
    response
}

#[test]
fn exact_profile_has_closed_grammar_and_native_resource_bounds() {
    let original = profile_json(Path::new("/operator/precheck.sock"), 2, 4);
    let accepted = Settings::decode(original.as_bytes()).unwrap();
    assert_eq!(accepted.expected_policy_epoch, 0);
    for replacement in [
        original.replace("exact_predicates", "none"),
        original.replace("exact_predicates", "full"),
        original.replace("fa.actor-precheck-service/1", "fa.actor-precheck-service/2"),
        original.replace("\"disclosure_id\":81", "\"disclosure_id\":0"),
        original.replace("\"query_limit\":2", "\"query_limit\":0"),
        original.replace("\"query_limit\":2", "\"query_limit\":129"),
        original.replace("\"exchange_limit\":4", "\"exchange_limit\":0"),
        original.replace("\"exchange_limit\":4", "\"exchange_limit\":129"),
        original.replace("\"candidate_limit\":8", "\"candidate_limit\":0"),
        original.replace("\"candidate_limit\":8", "\"candidate_limit\":1025"),
        original.replace("\"connection_limit\":4", "\"connection_limit\":9"),
        original.replace("\"connection_limit\":4", "\"connection_limit\":0"),
        original.replace("\"runtime_ms\":15000", "\"runtime_ms\":0"),
        original.replace("\"runtime_ms\":15000", "\"runtime_ms\":3600001"),
        original.replace("\"expected_policy_epoch\":0,", ""),
        original.replacen("{", "{\"unexpected\":1,", 1),
        original.replacen("{", "{\"query_limit\":1,", 1),
        original.replace("/operator/precheck.sock", "relative.sock"),
        original.replace("/operator/precheck.sock", "/operator/../precheck.sock"),
        original.replace("/operator/precheck.sock", &format!("/{}", "x".repeat(100))),
    ] {
        assert!(Settings::decode(replacement.as_bytes()).is_err(), "accepted {replacement}");
    }
}

#[test]
fn profile_preflight_refuses_collisions_and_preserves_preexisting_paths() {
    let fixture = Fixture::new(2, 4);
    let before = fixture.files();
    let reads = fixture.config.source.status().read_attempts;
    for path in [&fixture.actor.socket, &fixture.reviewer.socket(7),
        &super::super::super::control::socket_path(&fixture.reviewer, 7)] {
        let mut settings = fixture.settings.clone(); settings.socket = path.clone();
        assert!(settings.check_host(&fixture.config, &fixture.actor, &fixture.reviewer).is_err());
    }
    fs::write(&fixture.settings.socket, b"must remain").unwrap();
    assert!(fixture.settings.check_host(&fixture.config, &fixture.actor, &fixture.reviewer).is_err());
    assert_eq!(fs::read(&fixture.settings.socket).unwrap(), b"must remain");
    // The actor client validates a serving directory, not an absent endpoint.
    fixture.settings.check_actor(&fixture.actor).unwrap();
    fs::set_permissions(fixture.settings.socket.parent().unwrap(), fs::Permissions::from_mode(0o770)).unwrap();
    assert!(fixture.settings.check_actor(&fixture.actor).is_err());
    assert_eq!(fixture.config.source.status().read_attempts, reads);
    assert_eq!(fixture.files(), before);
}

#[test]
fn real_prechecks_leave_owner_unchanged_and_submit_still_enters_review() {
    let mut fixture = Fixture::new(4, 6);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let mut service = fixture.start();
    assert_eq!(fixture.config.source.status().read_attempts, 1);
    let before = fixture.files();
    let state = fixture.driver.supervisor().host().unwrap().inspect();
    known(&fixture.query(&mut service, fixture.request(7, b"network")), FilePrecheckOutcome::PrecheckPassed, 7);
    let mut denied = fixture.request(8, b"network"); denied.proposal.units = 513;
    known(&fixture.query(&mut service, denied), FilePrecheckOutcome::DeniedByPolicy, 8);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(fixture.config.source.status().read_attempts, 1);
    assert_eq!(fixture.driver.supervisor().host().unwrap().retained_requests(), 0);

    let proposal = fixture.request(7, b"network").proposal;
    let mut wire = ActorWire::new(fixture.port.clone());
    let submitted = fixture.driver.exchange_actor_from_file(&mut wire,
        &encode_command(&Command::Submit { request: 7, proposal }).unwrap(),
        &mut fixture.config.source, || ElapsedTick(1_001)).unwrap();
    assert!(matches!(submitted.response.result, Ok(Knowledge::Pending { request: 7 })));
    assert_eq!(fixture.config.source.status().read_attempts, 2);
    let host = fixture.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!((host.inspect().executions, host.inspect().control.ledger.reserved), (0, 0));
    drop(host);
    let admitted = fixture.files();
    service.checkpoint(&fixture.driver, ElapsedTick(1_001));
    assert_eq!(service.retirement, Some(Retirement::OwnerChanged));
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.files(), admitted);
}

#[test]
fn malformed_query_and_reconnect_share_original_work_without_source_refresh() {
    let mut fixture = Fixture::new(1, 4);
    let mut service = fixture.start();
    let before = fixture.files();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket.write_all(b"{}\n").unwrap();
    let bytes = raw_reply(&mut socket, &mut service, &fixture.driver);
    assert!(std::str::from_utf8(&bytes).unwrap().contains("malformed_request"));
    assert_eq!(service.frames, 1);
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 1);
    drop(socket);
    known(&fixture.query(&mut service, fixture.request(7, b"network")), FilePrecheckOutcome::PrecheckPassed, 7);
    let exhausted = fixture.query(&mut service, fixture.request(7, b"network"));
    assert_eq!(exhausted.result, Err(PrecheckWireError::Capacity));
    assert_eq!(service.frames, 3);
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 0);
    assert_eq!(fixture.config.source.status().read_attempts, 1);
    assert_eq!(fixture.files(), before);
}

#[test]
fn lost_reply_is_charged_and_an_explicit_new_query_cannot_refund_it() {
    let mut fixture = Fixture::new(1, 3);
    let mut service = fixture.start();
    let before = fixture.files();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    let mut document = encode_precheck_request(&fixture.request(7, b"network")).unwrap();
    document.push(b'\n'); socket.write_all(&document).unwrap();
    for _ in 0..1_024 {
        service.checkpoint(&fixture.driver, ElapsedTick(1_000));
        if service.active.as_ref().unwrap().disclosure.remaining_queries() == 0 { break; }
    }
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 0);
    assert_eq!(service.frames, 1);
    // Deliberately never receive the already-computed result.
    drop(socket);
    for _ in 0..1_024 {
        service.checkpoint(&fixture.driver, ElapsedTick(1_000));
        if service.active.as_ref().is_none_or(|active| active.session.status().active.is_none()) { break; }
    }
    if service.active.is_some() {
        assert!(service.active.as_ref().unwrap().session.status().active.is_none());
        let result = fixture.query(&mut service, fixture.request(7, b"network"));
        assert_eq!(result.result, Err(PrecheckWireError::Capacity));
    } else {
        // A kernel may report a reset when the peer closes with an unread
        // response. That retires this sidecar; it cannot reopen a fresh grant.
        assert_eq!(service.retirement, Some(Retirement::TransportFailure));
        assert!(UnixStream::connect(&fixture.settings.socket).is_err());
    }
    assert_eq!(fixture.config.source.status().read_attempts, 1);
    assert_eq!(fixture.files(), before);
    fixture.trace_case(Some(&service));
}

#[test]
fn partial_frame_failure_retires_only_the_disclosure_not_the_owner() {
    let mut fixture = Fixture::new(2, 3);
    let mut service = fixture.start();
    let before = fixture.files();
    let state = fixture.driver.supervisor().host().unwrap().inspect();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket.write_all(b"{\"query\":7").unwrap();
    socket.shutdown(std::net::Shutdown::Write).unwrap();
    for _ in 0..16 {
        service.checkpoint(&fixture.driver, ElapsedTick(1_000));
        if service.active.is_none() { break; }
    }
    assert_eq!(service.retirement, Some(Retirement::TransportFailure));
    assert_eq!(service.frames, 0);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(fixture.config.source.status().read_attempts, 1);
    assert!(!fixture.settings.socket.exists());
    fixture.trace_case(Some(&service));
}

#[test]
fn wrong_peer_candidates_spend_only_the_original_connection_cap() {
    for dimension in 0..3 {
        let mut fixture = Fixture::new(2, 3);
        let exact = fixture.actor.actor;
        fixture.actor.actor = match dimension {
            0 => PeerPolicy::new(if exact.uid() == 0 { 1 } else { 0 }, exact.gid(), exact.pid()).unwrap(),
            1 => PeerPolicy::new(exact.uid(), if exact.gid() == 0 { 1 } else { 0 }, exact.pid()).unwrap(),
            _ => PeerPolicy::new(exact.uid(), exact.gid(), Some(if exact.pid() == Some(1) { 2 } else { 1 })).unwrap(),
        };
        fixture.settings.candidate_limit = 2;
        fixture.settings.connection_limit = 2;
        let mut service = fixture.start();
        let before = fixture.files();
        for candidate in 1..=2 {
            let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
            service.checkpoint(&fixture.driver, ElapsedTick(1_000));
            socket.set_nonblocking(true).unwrap();
            assert_eq!(socket.read(&mut [0_u8; 1]).unwrap(), 0);
            assert_eq!(service.candidates, candidate);
            if candidate == 1 {
                assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 2);
            }
        }
        assert_eq!(service.retirement, Some(Retirement::CandidateLimit));
        assert_eq!(service.frames, 0);
        assert_eq!(fixture.config.source.status().read_attempts, 1);
        assert_eq!(fixture.files(), before);
        fixture.trace_case(Some(&service));
    }
}

#[test]
fn original_lease_backward_clock_and_wall_bound_retire_without_journal_work() {
    for dimension in 0..3 {
        let mut fixture = Fixture::new(2, 3);
        let mut service = fixture.start();
        let before = fixture.files();
        let state = fixture.driver.supervisor().host().unwrap().inspect();
        let (tick, reason) = match dimension {
            0 => (1_100, Retirement::Expired),
            1 => (999, Retirement::ClockMovedBack),
            _ => { service.wall = Duration::ZERO; (1_000, Retirement::Expired) }
        };
        service.checkpoint(&fixture.driver, ElapsedTick(tick));
        assert_eq!(service.retirement, Some(reason));
        assert!(!fixture.settings.socket.exists());
        // A later earlier tick cannot resurrect the original grant or endpoint.
        service.checkpoint(&fixture.driver, ElapsedTick(1_000));
        assert_eq!(service.retirement, Some(reason));
        assert_eq!(fixture.config.source.status().read_attempts, 1);
        assert_eq!(fixture.files(), before);
        assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
        fixture.trace_case(Some(&service));
    }
}

#[test]
fn expected_epoch_refusal_precedes_clock_source_and_listener_work() {
    let mut fixture = Fixture::new(2, 3);
    fixture.settings.expected_policy_epoch = 1;
    let before = fixture.files();
    let mut calls = 0;
    let result = Service::start(&fixture.settings, &fixture.actor, &fixture.reviewer,
        &mut fixture.config, &mut fixture.driver, &deadline(), &mut || {
            calls += 1; ElapsedTick(1_000)
        });
    assert!(matches!(result, Err(error) if error.contains("expected policy epoch")));
    assert_eq!(calls, 0);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.files(), before);
    fixture.trace_case(None);
}

#[test]
fn bootstrap_clock_cannot_regress_behind_its_acknowledged_source_tick() {
    let mut fixture = Fixture::new(2, 3);
    let mut ticks = [ElapsedTick(1), ElapsedTick(2), ElapsedTick(100), ElapsedTick(50)].into_iter();
    let result = Service::start(&fixture.settings, &fixture.actor, &fixture.reviewer,
        &mut fixture.config, &mut fixture.driver, &deadline(), &mut || ticks.next().unwrap());
    assert!(matches!(result, Err(error) if error.contains("acknowledged source capture")));
    assert_eq!(fixture.config.source.status().read_attempts, 1);
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.driver.supervisor().host().unwrap().retained_requests(), 0);
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect().executions, 0);
    fixture.trace_case(None);
}
