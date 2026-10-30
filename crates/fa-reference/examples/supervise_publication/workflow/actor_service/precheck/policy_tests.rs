//! Real operator-to-service reads with the original source-backed owner.
//! No fixture claims production containment or authentic provider observations.
use super::*;
use super::super::prepare_host;
use fa_reference::{Snapshot, action::ActionState};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorPort;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::policy::decode_policy_response;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    actor::{ActorProposal, Knowledge}, actor_peer::{PeerCredentials, PeerPolicy},
    actor_wire::{ActorWire, Command, encode_command},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::StateFreshness,
};
use std::{collections::BTreeMap, fs, io::Write, path::{Path, PathBuf}, thread,
    os::unix::{fs::DirBuilderExt, net::UnixStream}, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("policy service fixture cleanup: {error}"); }
    }
}
fn profile(socket: &Path) -> String {
    format!(r#"{{"schema":"fa.actor-precheck-service/3","policy_disclosure":"full","socket":"{}","expected_policy_epoch":0,"disclosure_id":81,"query_limit":8,"exchange_limit":16,"candidate_limit":16,"connection_limit":16,"runtime_ms":15000,"cache_entries":4,"cache_bytes":262144,"leakage":{{"policy_structure":"disclosed","policy_literals":"disclosed","source_values":"withheld","helper_feedback":"withheld","consequences":"withheld","timing":"observable","query_usage":"observable"}}}}"#, socket.display())
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
    fn new(large: bool) -> Self {
        let root = Root(crate::test_directory::create(format!("fa-pfull-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed))));
        for name in ["actors", "reviewers"] { fs::DirBuilder::new().mode(0o750).create(root.0.join(name)).unwrap(); }
        let mut config = Config::decode(include_bytes!("../../../../../fixtures/supervised_publication.json")).unwrap();
        config.store = root.0.join("store");
        config.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51,
            config.profile.delivery.scope, 1_048_576).unwrap();
        config.source_policy.freshness = StateFreshness::new(100).unwrap();
        config.timing.runtime_ms = 15_000; config.timing.poll_ms = 1;
        if large { config.profile.delivery.policy = Policy::new(1, vec![Predicate::PayloadIs(vec![255; 65_536])]).unwrap(); }
        let (socket, _) = UnixStream::pair().unwrap(); let id = PeerCredentials::observe(&socket).unwrap();
        let peer = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap();
        let actor = Profile { request: 7, scope: config.profile.delivery.scope,
            socket: root.0.join("actors/actor.sock"), supervisor: peer, actor: peer,
            candidates: 16, connections: 16, exchanges: 1_024, runtime_ms: 15_000, poll_ms: 1, reply_ms: 20 };
        let s = actor.scope;
        let reviewer = PeerProfile::decode(format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":77,"socket_directory":"{}/reviewers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":16,"runtime_ms":15000,"poll_ms":1}}"#,
            s.tenant, s.principal, s.run, s.branch, s.authority, root.0.display(),
            id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid()).as_bytes()).unwrap();
        let settings = Settings::decode(profile(&root.0.join("actors/precheck.sock")).as_bytes()).unwrap();
        let (host, _) = prepare_host(&config, None, false).unwrap(); let (port, driver) = host.into_supervised_driver();
        let fixture = Self { driver, port, config, actor, reviewer, settings, root };
        fixture.source(1, b"ok"); fixture.write_profiles(81, peer); fixture
    }
    fn source(&self, epoch: u64, value: &[u8]) {
        let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
            scope: self.config.profile.delivery.scope }, Snapshot { semantic_epoch: epoch, complete: true,
            values: BTreeMap::from([(7, value.to_vec())]) },
            BTreeMap::from([("alpha".into(), b"private-helper-alpha".to_vec()),
                ("beta".into(), b"private-helper-beta".to_vec())])).unwrap();
        fs::write(self.root.0.join("evidence.json"), data.encode()).unwrap();
    }
    fn write_profiles(&self, disclosure: u64, supervisor: PeerPolicy) {
        fs::write(self.root.0.join("precheck.json"), profile(&self.settings.socket)
            .replace("\"disclosure_id\":81", &format!("\"disclosure_id\":{disclosure}"))).unwrap();
        let s = self.actor.scope; let peer = self.actor.actor;
        let actor = format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"actor":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":16,"connection_limit":16,"exchange_limit":1024,"runtime_ms":15000,"poll_ms":1,"reply_ms":20}}"#,
            s.tenant, s.principal, s.run, s.branch, s.authority, self.actor.socket.display(),
            supervisor.uid(), supervisor.gid(), supervisor.pid().unwrap(), peer.uid(), peer.gid(), peer.pid().unwrap());
        fs::write(self.root.0.join("actor.json"), actor).unwrap();
    }
    fn start(&mut self) -> Service {
        Service::start(&self.settings, &self.actor, &self.reviewer, &mut self.config,
            &mut self.driver, &deadline(), &mut || ElapsedTick(1_000)).unwrap()
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fs::read_dir(&self.config.store).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file()).map(|path| { let bytes = fs::read(&path).unwrap(); (path, bytes) }).collect()
    }
    fn command(&self, service: &mut Service, fail_output: bool) -> (Result<(), String>, Vec<u8>) {
        service.checkpoint(&self.driver, ElapsedTick(1_000));
        let args = vec!["actor-precheck".into(), "--policy".into(),
            self.root.0.join("actor.json").to_str().unwrap().into(),
            self.root.0.join("precheck.json").to_str().unwrap().into(), "9001".into()];
        let handle = thread::spawn(move || {
            let mut out = Output { bytes: Vec::new(), fail: fail_output };
            let result = client::command(&args, &mut out); (result, out.bytes)
        });
        let started = Instant::now();
        while !handle.is_finished() {
            assert!(started.elapsed() < Duration::from_secs(10));
            service.checkpoint(&self.driver, ElapsedTick(1_000));
            thread::sleep(Duration::from_millis(1));
        }
        handle.join().unwrap()
    }
}
fn deadline() -> Deadline {
    Deadline { logical: ElapsedTick(20_000), started: Instant::now(), wall: Duration::from_secs(15) }
}
struct Output { bytes: Vec<u8>, fail: bool }
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail && !self.bytes.is_empty() { return Err(io::Error::from(io::ErrorKind::BrokenPipe)); }
        let count = if self.fail { bytes.len().min(3) } else { bytes.len() };
        self.bytes.extend_from_slice(&bytes[..count]); Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn full_profile_requires_exact_leakage_and_limits_without_upgrading_old_versions() {
    let good = profile(Path::new("/operator/precheck.sock"));
    assert!(Settings::decode(good.as_bytes()).unwrap().full_policy);
    for invalid in [
        good.replace("service/3", "service/1"), good.replace("service/3", "service/2"),
        good.replace("\"full\"", "\"exact_predicates\""),
        good.replace("\"cache_entries\":4,", ""), good.replace("\"cache_bytes\":262144,", ""),
        good.replace("\"cache_entries\":4", "\"cache_entries\":65"),
        good.replace("\"policy_literals\":\"disclosed\"", "\"policy_literals\":\"withheld\""),
        good.replace("\"helper_feedback\":\"withheld\"", "\"helper_feedback\":\"disclosed\""),
        good.replace("\"timing\":\"observable\"", "\"timing\":\"hidden\""),
        good.replace("\"query_usage\":\"observable\"", "\"other\":\"observable\""),
        good.replace("\"leakage\":{", "\"leakage\":{\"policy_literals\":\"disclosed\","),
        good.replace("\"leakage\":{", "\"leakage\":{\"unexpected\":true,"),
    ] { assert!(Settings::decode(invalid.as_bytes()).is_err(), "accepted {invalid}"); }
}

#[test]
fn real_operator_gets_complete_native_definition_without_source_or_authority_changes() {
    for large in [false, true] {
        let mut fixture = Fixture::new(large); let mut service = fixture.start();
        let before = fixture.files(); let state = fixture.driver.supervisor().host().unwrap().inspect();
        let (result, output) = fixture.command(&mut service, false); result.unwrap();
        let response = decode_policy_response(output.strip_suffix(b"\n").unwrap(), 9001, 0).unwrap();
        let Ok(Knowledge::Known { value, basis }) = &response.result else { panic!("policy") };
        assert_eq!(value.definition(), &fixture.config.profile.delivery.policy);
        assert_eq!((basis.disclosure, basis.observed_at), (81, ElapsedTick(1_000)));
        assert_eq!(output, [response.encode().unwrap(), b"\n".to_vec()].concat());
        if large { assert!(output.len() > 131_072); }
        for secret in ["private-helper-alpha", "private-helper-beta"] {
            assert!(!std::str::from_utf8(&output).unwrap().contains(secret));
        }
        assert_eq!(fixture.files(), before); assert_eq!(fixture.config.source.status().read_attempts, 1);
        assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), state);
        assert_eq!(service.frames, 1); assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 7);
    }
}

#[test]
fn public_full_profile_cannot_upgrade_an_independently_exact_only_server() {
    let mut fixture = Fixture::new(false); fixture.settings.full_policy = false;
    let mut service = fixture.start(); let before = fixture.files();
    let (result, output) = fixture.command(&mut service, false);
    assert!(result.unwrap_err().contains("unavailable"));
    assert!(matches!(decode_policy_response(output.strip_suffix(b"\n").unwrap(), 9001, 0).unwrap().result,
        Ok(Knowledge::Withheld { .. })));
    assert_eq!(fixture.files(), before); assert_eq!(service.frames, 1);
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 7);
}

#[test]
fn policy_read_does_not_replace_fresh_source_admission_or_ordinary_review() {
    for changed in [false, true] {
        let mut fixture = Fixture::new(false); let mut service = fixture.start();
        fixture.command(&mut service, false).0.unwrap();
        if changed { fixture.source(2, b"no"); }
        let proposal = ActorProposal { target: fixture.config.profile.delivery.target,
            payload: b"network".to_vec(), units: 8, deadline: ElapsedTick(10_000), expected_policy_epoch: 0 };
        // Use the original public port and source-aware submission entry.
        let mut wire = ActorWire::new(fixture.port.clone());
        let submitted = fixture.driver.exchange_actor_from_file(&mut wire,
            &encode_command(&Command::Submit { request: 7, proposal }).unwrap(),
            &mut fixture.config.source, || ElapsedTick(1_001)).unwrap();
        assert!(submitted.response.result.is_ok());
        assert_eq!(fixture.config.source.status().read_attempts, 2);
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().control.ledger.stages[&1],
            if changed { ActionState::Denied } else { ActionState::Reviewing });
        assert_eq!(host.inspect().executions, 0); drop(host);
        service.checkpoint(&fixture.driver, ElapsedTick(1_001));
        assert_eq!(service.retirement, Some(Retirement::OwnerChanged));
    }
}

#[test]
fn missing_programmatic_limits_refuse_before_clock_source_or_listener() {
    let mut fixture = Fixture::new(false); fixture.settings.cache = None;
    let files = fixture.files(); let mut calls = 0;
    assert!(Service::start(&fixture.settings, &fixture.actor, &fixture.reviewer,
        &mut fixture.config, &mut fixture.driver, &deadline(), &mut || {
            calls += 1; ElapsedTick(1_000)
        }).is_err());
    assert_eq!(calls, 0); assert_eq!(fixture.config.source.status().read_attempts, 0);
    assert!(!fixture.settings.socket.exists()); assert_eq!(fixture.files(), files);
}

#[test]
fn full_endpoint_retains_clock_lease_wall_and_supervisor_retirement() {
    for variant in 0..4 {
        let mut fixture = Fixture::new(false); let mut service = fixture.start();
        fixture.command(&mut service, false).0.unwrap(); let files = fixture.files();
        match variant {
            0 => service.checkpoint(&fixture.driver, ElapsedTick(1_100)),
            1 => service.checkpoint(&fixture.driver, ElapsedTick(999)),
            2 => { service.wall = Duration::ZERO; service.checkpoint(&fixture.driver, ElapsedTick(1_000)); }
            _ => service.retire(),
        }
        assert!(service.active.is_none()); assert!(!fixture.settings.socket.exists());
        service.checkpoint(&fixture.driver, ElapsedTick(1_000));
        assert!(UnixStream::connect(&fixture.settings.socket).is_err());
        assert_eq!(fixture.files(), files); assert_eq!(fixture.config.source.status().read_attempts, 1);
    }
}

#[test]
fn partial_output_failure_never_resends_a_policy_request() {
    let mut fixture = Fixture::new(false); let mut service = fixture.start(); let files = fixture.files();
    let (result, output) = fixture.command(&mut service, true);
    assert!(result.unwrap_err().contains("output failed")); assert_eq!(output.len(), 3);
    service.checkpoint(&fixture.driver, ElapsedTick(1_000));
    assert_eq!((service.frames, service.candidates), (1, 1));
    assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), 7);
    assert_eq!(fixture.files(), files);
}

#[test]
fn mismatched_disclosure_or_server_credentials_never_print_policy_bytes() {
    for auth in [false, true] {
        let mut fixture = Fixture::new(false); let mut service = fixture.start();
        let original = fixture.actor.supervisor;
        let peer = if auth { PeerPolicy::new(original.uid(), original.gid(),
            Some(if original.pid() == Some(1) { 2 } else { 1 })).unwrap() } else { original };
        fixture.write_profiles(if auth { 81 } else { 82 }, peer);
        let (result, output) = fixture.command(&mut service, false);
        assert!(result.is_err()); assert!(output.is_empty());
        assert_eq!(service.frames, if auth { 0 } else { 1 });
        assert_eq!(service.active.as_ref().unwrap().disclosure.remaining_queries(), if auth { 8 } else { 7 });
    }
}
