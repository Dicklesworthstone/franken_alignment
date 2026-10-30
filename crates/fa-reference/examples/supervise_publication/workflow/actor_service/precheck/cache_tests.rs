//! Real service/bootstrap and authenticated native clients under explicit v2
//! cache selection. Fixtures establish boundaries, not production qualification.
use super::*;
use super::super::prepare_host;
use fa_reference::{Snapshot, action::ActionState};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{FileActorPort, FilePrecheckOutcome};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    PrecheckRequest, PrecheckWireResponse, PrecheckWireError,
    cache::{PrecheckCacheLimits, MAX_PRECHECK_CACHE_ENTRIES, MAX_PRECHECK_CACHE_BYTES},
    client::{PrecheckPeerClient, PrecheckCapabilitiesClient, PrecheckClientPhase},
    discovery::{PrecheckCapabilitiesRequest, PrecheckCapabilitiesResponse},
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorProposal, Knowledge},
    actor_peer::{PeerCredentials, PeerPolicy},
    actor_wire::{ActorWire, Command, encode_command},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::StateFreshness,
};
use std::{collections::BTreeMap, fs, io::Read, path::{Path, PathBuf},
    os::unix::{fs::DirBuilderExt, net::UnixStream}, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("cached precheck fixture cleanup: {error}"); }
    }
}

fn profile(socket: &Path, cached: bool, queries: usize) -> String {
    let (version, extra) = if cached { (2, ",\"cache_entries\":4,\"cache_bytes\":65536") } else { (1, "") };
    format!(r#"{{"schema":"fa.actor-precheck-service/{version}","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":0,"disclosure_id":81,"query_limit":{queries},"exchange_limit":8,"candidate_limit":16,"connection_limit":16,"runtime_ms":15000{extra}}}"#, socket.display())
}

struct Fixture {
    driver: FileSupervisedDriver,
    port: FileActorPort<FileOversight>,
    config: Config,
    actor: Profile,
    reviewer: PeerProfile,
    settings: Settings,
    root: Root,
}
impl Fixture {
    fn new(cached: bool, queries: usize) -> Self {
        let root = Root(crate::test_directory::create(format!("fa-pcache-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
        for name in ["actors", "reviewers"] {
            fs::DirBuilder::new().mode(0o750).create(root.0.join(name)).unwrap();
        }
        let mut config = Config::decode(include_bytes!("../../../../../fixtures/supervised_publication.json")).unwrap();
        config.store = root.0.join("store");
        config.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51,
            config.profile.delivery.scope, 1_048_576).unwrap();
        config.source_policy.freshness = StateFreshness::new(100).unwrap();
        config.timing.runtime_ms = 15_000; config.timing.poll_ms = 1;
        let (socket, _) = UnixStream::pair().unwrap();
        let id = PeerCredentials::observe(&socket).unwrap();
        let policy = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap();
        let actor = Profile { request: 7, scope: config.profile.delivery.scope,
            socket: root.0.join("actors/actor.sock"), supervisor: policy, actor: policy,
            candidates: 16, connections: 16, exchanges: 1_024,
            runtime_ms: 15_000, poll_ms: 1, reply_ms: 20 };
        let scope = actor.scope;
        let reviewer = PeerProfile::decode(format!(
            r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":77,"socket_directory":"{}/reviewers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":16,"runtime_ms":15000,"poll_ms":1}}"#,
            scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
            root.0.display(), id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid()
        ).as_bytes()).unwrap();
        let settings = Settings::decode(profile(&root.0.join("actors/precheck.sock"), cached, queries).as_bytes()).unwrap();
        let (host, _) = prepare_host(&config, None, false).unwrap();
        let (port, driver) = host.into_supervised_driver();
        let fixture = Self { driver, port, config, actor, reviewer, settings, root };
        fixture.source(1, b"ok"); fixture
    }
    fn source(&self, epoch: u64, value: &[u8]) {
        let data = EvidenceSnapshot::new(EvidenceIdentity {
            source: 51, generation: 1, scope: self.config.profile.delivery.scope,
        }, Snapshot { semantic_epoch: epoch, complete: true,
            values: BTreeMap::from([(7, value.to_vec())]) },
            BTreeMap::from([("alpha".into(), b"private-helper-alpha".to_vec()),
                ("beta".into(), b"private-helper-beta".to_vec())])).unwrap();
        fs::write(self.root.0.join("evidence.json"), data.encode()).unwrap();
    }
    fn start(&mut self) -> Service {
        let deadline = Deadline { logical: ElapsedTick(20_000), started: Instant::now(), wall: Duration::from_secs(15) };
        Service::start(&self.settings, &self.actor, &self.reviewer, &mut self.config,
            &mut self.driver, &deadline, &mut || ElapsedTick(1_000)).unwrap()
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(&self.config.store).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
    }
    fn request(&self, query: u64) -> PrecheckRequest {
        PrecheckRequest { query, proposal: ActorProposal {
            target: self.config.profile.delivery.target, payload: b"network".to_vec(),
            units: 8, deadline: ElapsedTick(10_000), expected_policy_epoch: 0,
        } }
    }
    fn connect(&self, service: &mut Service) -> UnixStream {
        service.checkpoint(&self.driver, ElapsedTick(1_000));
        UnixStream::connect(&self.settings.socket).unwrap()
    }
    fn drive<R>(&self, service: &mut Service, mut client: PrecheckPeerClient<R>) -> R {
        for _ in 0..2_048 {
            let report = client.drive(DriveBudget::default()).unwrap();
            assert!(report.status.failure.is_none(), "{:?}", report.status);
            if report.status.closed() { break; }
            service.checkpoint(&self.driver, ElapsedTick(1_000));
        }
        assert_eq!(client.status().phase, PrecheckClientPhase::Complete);
        client.take_response().unwrap()
    }
    fn query(&self, service: &mut Service, request: PrecheckRequest) -> PrecheckWireResponse {
        let socket = self.connect(service);
        self.drive(service, PrecheckPeerClient::connect(socket, self.actor.supervisor, request, 0).unwrap())
    }
    fn capabilities(&self, service: &mut Service, query: u64) -> PrecheckCapabilitiesResponse {
        let socket = self.connect(service);
        self.drive(service, PrecheckCapabilitiesClient::connect_capabilities(socket, self.actor.supervisor,
            PrecheckCapabilitiesRequest { query, expected_policy_epoch: 0 }, 0).unwrap())
    }
}
fn known(response: &PrecheckWireResponse, expected: FilePrecheckOutcome, query: u64) {
    assert_eq!(response.query, Some(query));
    let Ok(Knowledge::Known { value, basis }) = &response.result else { panic!("known precheck") };
    assert_eq!(*value, expected);
    assert_eq!((basis.disclosure, basis.query, basis.policy_epoch, basis.observed_at), (81, query, 0, ElapsedTick(1_000)));
}

#[test]
fn v1_remains_uncached_and_v2_requires_its_explicit_native_bounds() {
    let legacy = profile(Path::new("/operator/precheck.sock"), false, 4);
    assert_eq!(Settings::decode(legacy.as_bytes()).unwrap().cache, None);
    let explicit = profile(Path::new("/operator/precheck.sock"), true, 4);
    assert_eq!(Settings::decode(explicit.as_bytes()).unwrap().cache,
        Some(PrecheckCacheLimits { entries: 4, retained_bytes: 65_536 }));
    assert!(Settings::decode(legacy.replace("service/1", "service/2").as_bytes()).is_err());
    assert!(Settings::decode(explicit.replace("service/2", "service/1").as_bytes()).is_err());
    for (entries, bytes) in [(1, 1), (MAX_PRECHECK_CACHE_ENTRIES, MAX_PRECHECK_CACHE_BYTES)] {
        let json = explicit.replace("\"cache_entries\":4", &format!("\"cache_entries\":{entries}"))
            .replace("\"cache_bytes\":65536", &format!("\"cache_bytes\":{bytes}"));
        assert_eq!(Settings::decode(json.as_bytes()).unwrap().cache,
            Some(PrecheckCacheLimits { entries, retained_bytes: bytes }));
    }
}

#[test]
fn cached_profile_rejects_missing_duplicate_unknown_and_unbounded_cache_fields() {
    let valid = profile(Path::new("/operator/precheck.sock"), true, 4);
    for invalid in [
        valid.replace("\"cache_entries\":4,", ""), valid.replace(",\"cache_bytes\":65536", ""),
        valid.replace("\"cache_entries\":4", "\"cache_entries\":0"),
        valid.replace("\"cache_entries\":4", "\"cache_entries\":65"),
        valid.replace("\"cache_bytes\":65536", "\"cache_bytes\":0"),
        valid.replace("\"cache_bytes\":65536", "\"cache_bytes\":1048577"),
        valid.replace("\"cache_bytes\":65536", "\"cache_bytes\":\"65536\""),
        valid.replace("\"cache_bytes\":65536", "\"cache_bytes\":-1"),
        valid.replacen('{', "{\"cache_entries\":4,", 1),
        valid.replacen('{', "{\"cache_mode\":\"full\",", 1),
        valid.replace("exact_predicates", "full"), valid.replace("service/2", "service/3"),
    ] { assert!(Settings::decode(invalid.as_bytes()).is_err(), "accepted {invalid}"); }
}

#[test]
fn cached_and_uncached_services_preserve_results_and_cumulative_query_work() {
    let mut original = None;
    for cached in [false, true] {
        let mut fixture = Fixture::new(cached, 4); let mut service = fixture.start();
        let before = fixture.files(); let state = fixture.driver.supervisor().host().unwrap().inspect();
        let result = fixture.capabilities(&mut service, 90);
        let Ok(Knowledge::Known { value, .. }) = result.result else { panic!("capabilities") };
        assert_eq!(value.witness_cache_enabled(), cached); assert_eq!(value.remaining_queries(), 3);
        let first = fixture.query(&mut service, fixture.request(7));
        known(&first, FilePrecheckOutcome::PrecheckPassed, 7);
        let second = fixture.query(&mut service, fixture.request(7));
        assert_eq!(first, second);
        if let Some(original) = &original { assert_eq!(&first, original); } else { original = Some(first); }
        let mut denied = fixture.request(8); denied.proposal.units = 513;
        known(&fixture.query(&mut service, denied), FilePrecheckOutcome::DeniedByPolicy, 8);
        assert_eq!(fixture.query(&mut service, fixture.request(7)).result, Err(PrecheckWireError::Capacity));
        assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 0);
        assert_eq!(service.frames, 5); assert_eq!(service.candidates, 5);
        assert_eq!(fixture.config.source.status().read_attempts, 1);
        assert_eq!(fixture.files(), before); assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
    }
}

#[test]
fn warm_cache_never_replaces_fresh_source_at_actual_submit() {
    for changed in [false, true] {
        let mut fixture = Fixture::new(true, 4); let mut service = fixture.start();
        let candidate = fixture.request(7);
        for _ in 0..2 {
            known(&fixture.query(&mut service, candidate.clone()), FilePrecheckOutcome::PrecheckPassed, 7);
        }
        if changed { fixture.source(2, b"no"); }
        let mut wire = ActorWire::new(fixture.port.clone());
        let report = fixture.driver.exchange_actor_from_file(&mut wire,
            &encode_command(&Command::Submit { request: 7, proposal: candidate.proposal }).unwrap(),
            &mut fixture.config.source, || ElapsedTick(1_001)).unwrap();
        assert!(report.response.result.is_ok());
        assert_eq!(fixture.config.source.status().read_attempts, 2);
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&1],
            if changed { ActionState::Denied } else { ActionState::Reviewing });
        assert_eq!((host.inspect().executions, host.inspect().control.ledger.reserved), (0, 0));
        drop(host);
        let files = fixture.files(); service.checkpoint(&fixture.driver, ElapsedTick(1_001));
        assert_eq!(service.retirement, Some(Retirement::OwnerChanged));
        assert!(!fixture.settings.socket.exists()); assert_eq!(fixture.files(), files);
    }
}

#[test]
fn warmed_service_retires_on_original_lease_clock_wall_or_explicit_stop() {
    for dimension in 0..4 {
        let mut fixture = Fixture::new(true, 4); let mut service = fixture.start();
        for _ in 0..2 {
            known(&fixture.query(&mut service, fixture.request(7)), FilePrecheckOutcome::PrecheckPassed, 7);
        }
        let files = fixture.files(); let state = fixture.driver.supervisor().host().unwrap().inspect();
        let reason = match dimension {
            0 => { service.checkpoint(&fixture.driver, ElapsedTick(1_100)); Retirement::Expired }
            1 => { service.checkpoint(&fixture.driver, ElapsedTick(999)); Retirement::ClockMovedBack }
            2 => { service.wall = Duration::ZERO; service.checkpoint(&fixture.driver, ElapsedTick(1_000)); Retirement::Expired }
            _ => { service.retire(); Retirement::SupervisorEnded }
        };
        assert_eq!(service.retirement, Some(reason)); assert!(service.active.is_none());
        service.checkpoint(&fixture.driver, ElapsedTick(1_000));
        assert_eq!(service.retirement, Some(reason));
        assert!(UnixStream::connect(&fixture.settings.socket).is_err());
        assert_eq!(fixture.files(), files); assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
        assert_eq!(fixture.config.source.status().read_attempts, 1);
    }
}

#[test]
fn programmatically_invalid_cache_limits_fail_before_clock_source_or_listener() {
    for limits in [PrecheckCacheLimits { entries: 0, retained_bytes: 1 },
        PrecheckCacheLimits { entries: 1, retained_bytes: MAX_PRECHECK_CACHE_BYTES + 1 }] {
        let mut fixture = Fixture::new(true, 4); fixture.settings.cache = Some(limits);
        let files = fixture.files(); let mut calls = 0;
        let deadline = Deadline { logical: ElapsedTick(20_000), started: Instant::now(), wall: Duration::from_secs(15) };
        let result = Service::start(&fixture.settings, &fixture.actor, &fixture.reviewer,
            &mut fixture.config, &mut fixture.driver, &deadline, &mut || { calls += 1; ElapsedTick(1_000) });
        assert!(result.is_err()); assert_eq!(calls, 0);
        assert_eq!(fixture.config.source.status().read_attempts, 0);
        assert!(!fixture.settings.socket.exists()); assert_eq!(fixture.files(), files);
    }
}

#[test]
fn cache_opt_in_does_not_bypass_server_side_peer_authentication() {
    let mut fixture = Fixture::new(true, 4);
    let expected = fixture.actor.actor;
    fixture.actor.actor = PeerPolicy::new(expected.uid(), expected.gid(),
        Some(if expected.pid() == Some(1) { 2 } else { 1 })).unwrap();
    let mut service = fixture.start(); let before = fixture.files();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    service.checkpoint(&fixture.driver, ElapsedTick(1_000));
    socket.set_nonblocking(true).unwrap(); assert_eq!(socket.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(service.frames, 0); assert_eq!(service.candidates, 1);
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 4);
    assert_eq!(fixture.config.source.status().read_attempts, 1); assert_eq!(fixture.files(), before);
}

#[test]
fn cache_too_small_to_retain_still_executes_original_policy_and_spends_queries() {
    let mut fixture = Fixture::new(true, 4);
    fixture.settings.cache = Some(PrecheckCacheLimits { entries: 1, retained_bytes: 1 });
    let mut service = fixture.start();
    for query in [7, 8] {
        known(&fixture.query(&mut service, fixture.request(query)), FilePrecheckOutcome::PrecheckPassed, query);
    }
    let response = fixture.capabilities(&mut service, 9);
    let Ok(Knowledge::Known { value, .. }) = response.result else { panic!("capabilities") };
    assert!(value.witness_cache_enabled()); assert_eq!(value.remaining_queries(), 1);
    assert_eq!(fixture.config.source.status().read_attempts, 1);
}
