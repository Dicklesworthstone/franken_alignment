//! Actual original owners and authenticated Unix sockets, without qualification
//! claims about production deployment or independent model judgments.
#[path = "tests/hold_diagnosis.rs"]
mod hold_diagnosis;
#[path = "tests/policy_replay.rs"]
mod policy_replay;
#[path = "tests/review_explanation.rs"]
mod review_explanation;

use super::super::{Config, PeerProfile};
use super::*;
use fa_reference::Snapshot;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_EXPLANATION_EVENTS, MAX_EXPLANATION_QUERIES, MAX_INSPECTION_QUERIES,
};
use fa_reference::action::consequence::oversight::actor::{ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::MAX_CHANNEL_EXCHANGES;
use fa_reference::action::consequence::oversight::evidence_source::FileEvidenceSource;
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const REGISTRATION: u64 = 41;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = crate::test_directory::create(format!(
            "fa-tower-operator-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ["actors", "reviewers", "operators"] {
            fs::DirBuilder::new()
                .mode(0o750)
                .create(path.join(name))
                .unwrap();
        }
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("operator service fixture cleanup: {error}");
        }
    }
}

struct Fixture {
    driver: FileSupervisedDriver,
    port: Port,
    config: Config,
    actor: Profile,
    reviewer: PeerProfile,
    settings: Settings,
    _root: Directory,
}
impl Fixture {
    fn new(inspection: usize, explanation: usize) -> Self {
        Self::configured(inspection, explanation, |_| {})
    }
    fn configured(
        inspection: usize,
        explanation: usize,
        configure: impl FnOnce(&mut Config),
    ) -> Self {
        let root = Directory::new();
        let mut config = Config::decode(include_bytes!(
            "../../../../../fixtures/supervised_publication.json"
        ))
        .unwrap();
        config.store = root.0.join("store");
        config.profile.delivery.max_attempts = 64;
        configure(&mut config);
        // Deliberately unavailable: a supervisor read cannot silently bootstrap
        // evidence, launch the configured helpers or acquire an admission view.
        config.source = FileEvidenceSource::new(
            root.0.join("absent-source.json"),
            51,
            config.profile.delivery.scope,
            1_048_576,
        )
        .unwrap();
        let (socket, _) = UnixStream::pair().unwrap();
        let credentials = PeerCredentials::observe(&socket).unwrap();
        let identity = PeerPolicy::new(
            credentials.uid(),
            credentials.gid(),
            Some(credentials.pid()),
        )
        .unwrap();
        let actor = Profile {
            request: 7,
            scope: config.profile.delivery.scope,
            socket: root.0.join("actors/actor.sock"),
            supervisor: identity,
            actor: identity,
            candidates: 8,
            connections: 4,
            exchanges: 1_024,
            runtime_ms: 15_000,
            poll_ms: 1,
            reply_ms: 20,
        };
        let scope = actor.scope;
        let reviewer = PeerProfile::decode(format!(
            r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":77,"socket_directory":"{}/reviewers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":8,"runtime_ms":15000,"poll_ms":1}}"#,
            scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
            root.0.display(), credentials.uid(), credentials.gid(), credentials.pid(),
            credentials.uid(), credentials.gid(), credentials.pid()).as_bytes()).unwrap();
        let settings = Settings::decode(
            profile_json(
                &root.0.join("operators/operator.sock"),
                identity,
                inspection,
                explanation,
            )
            .as_bytes(),
        )
        .unwrap();
        // This fixture selects the original native, unconfigured-source host;
        // end-to-end service tests exercise the separate source bootstrap path.
        let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1_000))
            .unwrap();
        let (port, driver) = host.into_supervised_driver();
        Self {
            driver,
            port,
            config,
            actor,
            reviewer,
            settings,
            _root: root,
        }
    }
    fn start(&self) -> Service {
        self.settings
            .check_host(&self.config, &self.actor, &self.reviewer, &[])
            .unwrap();
        Service::start(
            &self.settings,
            &self.actor,
            self.port.clone(),
            &self.driver,
            REGISTRATION,
            &deadline(),
        )
        .unwrap()
    }
    fn snapshot(&mut self) {
        let revision = self.driver.supervisor().host().unwrap().revision();
        self.driver
            .supervisor_mut()
            .set_snapshot(
                revision,
                Some(Snapshot {
                    semantic_epoch: 1,
                    complete: true,
                    values: BTreeMap::from([(7, b"ok".to_vec())]),
                }),
            )
            .unwrap();
    }
    fn proposal(&self) -> ActorProposal {
        ActorProposal {
            target: self.config.profile.delivery.target,
            payload: b"publish".to_vec(),
            units: 8,
            deadline: ElapsedTick(10_000),
            expected_policy_epoch: 0,
        }
    }
    fn submit(&mut self, request: u64) {
        self.snapshot();
        let ticket = self.port.submit(request, &self.proposal()).unwrap();
        assert_eq!(self.port.poll(&ticket), Knowledge::Pending { request });
    }
    fn request(&self, verb: Verb) -> TowerRequest {
        let tower = PublicationTower::new(
            Address::new(
                self.actor.scope.tenant.to_string(),
                Noun::Run,
                self.actor.scope.run.to_string(),
                Some(REGISTRATION),
            )
            .unwrap(),
            self.port.clone(),
            PublicationLimits::default(),
        )
        .unwrap();
        let address = if verb == Verb::Explain {
            tower.request_address(self.actor.request).unwrap()
        } else {
            tower.mount_address().clone()
        };
        TowerRequest::new("operator-fixture", address, verb)
    }
    fn query(&self, service: &mut Service, request: TowerRequest) -> Json {
        query(&self.settings.socket, service, request)
    }
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        files(&self.config.store)
    }
}

fn profile_json(
    socket: &Path,
    identity: PeerPolicy,
    inspection: usize,
    explanation: usize,
) -> String {
    let disclosure = if explanation == 0 {
        "none"
    } else {
        "policy_and_selected_witnesses"
    };
    let events = if explanation == 0 {
        0
    } else {
        MAX_EXPLANATION_EVENTS
    };
    let pid = identity
        .pid()
        .map_or_else(|| "null".to_owned(), |pid| pid.to_string());
    format!(
        r#"{{"schema":"fa.tower-operator-service/1","scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"operator":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":8,"connection_limit":4,"exchange_limit":16,"runtime_ms":15000,"poll_ms":1,"inspection_queries":{},"explanation_queries":{},"explanation_replay_events":{},"explanation_disclosure":"{}"}}"#,
        socket.display(),
        identity.uid(),
        identity.gid(),
        pid,
        identity.uid(),
        identity.gid(),
        pid,
        inspection,
        explanation,
        events,
        disclosure
    )
}
fn deadline() -> Deadline {
    Deadline {
        logical: ElapsedTick(20_000),
        started: Instant::now(),
        wall: Duration::from_secs(15),
    }
}
fn files(store: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(store)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}
fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| &value.as_object().unwrap()[*key])
}
fn query(path: &Path, service: &mut Service, request: TowerRequest) -> Json {
    // Complete the prior connection's EOF without replacing its session,
    // retained capture or native grants before making an explicit reconnect.
    service.checkpoint();
    let mut socket = UnixStream::connect(path).unwrap();
    let mut document = encode_request(&request).unwrap().into_bytes();
    document.push(b'\n');
    socket.write_all(&document).unwrap();
    let response = read_reply(&mut socket, service);
    strict_json::parse(&response, KNOWLEDGE_JSON_LIMITS).unwrap()
}
fn read_reply(socket: &mut UnixStream, service: &mut Service) -> Vec<u8> {
    socket.set_nonblocking(true).unwrap();
    let mut response = Vec::new();
    for _ in 0..4_096 {
        service.checkpoint();
        let mut buffer = [0_u8; 4_096];
        match socket.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => response.extend_from_slice(&buffer[..count]),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => panic!("operator response failed: {error}"),
        }
        assert!(response.len() <= PublicationLimits::default().max_response_bytes + 1);
        if response.ends_with(b"\n") {
            break;
        }
    }
    assert!(
        response.ends_with(b"\n"),
        "missing complete operator response; retirement={:?}",
        service.retirement
    );
    response
}
fn known(response: &Json, kind: &str) {
    assert_eq!(field(response, &["result", "kind"]).as_str(), Some(kind));
    assert_eq!(
        field(response, &["result", "value", "knowledge", "state"]).as_str(),
        Some("known")
    );
}

#[test]
fn closed_operator_profile_requires_explicit_disclosure_and_original_work_bounds() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let original = profile_json(Path::new("/operator/inspection.sock"), identity, 2, 2);
    let accepted = Settings::decode(original.as_bytes()).unwrap();
    assert_eq!(
        accepted.explanation_disclosure,
        "policy_and_selected_witnesses"
    );
    for (before, after) in [
        ("fa.tower-operator-service/1", "fa.tower-operator-service/2"),
        ("\"tenant\":1", "\"tenant\":0"),
        ("\"connection_limit\":4", "\"connection_limit\":9"),
        ("\"connection_limit\":4", "\"connection_limit\":0"),
        ("\"candidate_limit\":8", "\"candidate_limit\":0"),
        ("\"candidate_limit\":8", "\"candidate_limit\":1025"),
        ("\"exchange_limit\":16", "\"exchange_limit\":0"),
        ("\"runtime_ms\":15000", "\"runtime_ms\":0"),
        ("\"runtime_ms\":15000", "\"runtime_ms\":3600001"),
        ("\"runtime_ms\":15000", "\"runtime_ms\":\"15000\""),
        ("\"poll_ms\":1", "\"poll_ms\":0"),
        ("\"poll_ms\":1", "\"poll_ms\":1001"),
        ("\"pid\":3", "\"pid\":0"),
        ("\"uid\":1", "\"uid\":4294967295"),
        ("policy_and_selected_witnesses", "none"),
        ("policy_and_selected_witnesses", "exact_predicates"),
        ("/operator/inspection.sock", "relative.sock"),
        ("/operator/inspection.sock", "/operator/../inspection.sock"),
        ("/operator/inspection.sock", "/operator/./inspection.sock"),
        ("/operator/inspection.sock", "/operator//inspection.sock"),
    ] {
        assert!(
            Settings::decode(original.replace(before, after).as_bytes()).is_err(),
            "accepted {after}"
        );
    }
    for invalid in [
        original.replace(
            "\"exchange_limit\":16",
            &format!("\"exchange_limit\":{}", MAX_CHANNEL_EXCHANGES + 1),
        ),
        original.replace(
            "\"inspection_queries\":2",
            &format!("\"inspection_queries\":{}", MAX_INSPECTION_QUERIES + 1),
        ),
        original.replace(
            "\"explanation_queries\":2",
            &format!("\"explanation_queries\":{}", MAX_EXPLANATION_QUERIES + 1),
        ),
        original.replace(
            &format!("\"explanation_replay_events\":{MAX_EXPLANATION_EVENTS}"),
            "\"explanation_replay_events\":0",
        ),
        original.replace(
            &format!("\"explanation_replay_events\":{MAX_EXPLANATION_EVENTS}"),
            &format!(
                "\"explanation_replay_events\":{}",
                MAX_EXPLANATION_EVENTS + 1
            ),
        ),
        original.replacen('{', "{\"unexpected\":1,", 1),
        original.replacen('{', "{\"inspection_queries\":1,", 1),
        original.replace("\"principal\":2,", ""),
        original.replace(
            "/operator/inspection.sock",
            &format!("/{}", "x".repeat(100)),
        ),
        profile_json(Path::new("/operator/inspection.sock"), identity, 0, 0),
    ] {
        assert!(
            Settings::decode(invalid.as_bytes()).is_err(),
            "accepted {invalid}"
        );
    }
    for (inspection, explanation) in [(2, 0), (0, 2)] {
        let document = profile_json(
            Path::new("/operator/inspection.sock"),
            identity,
            inspection,
            explanation,
        );
        assert!(Settings::decode(document.as_bytes()).is_ok());
    }
    let none = profile_json(Path::new("/operator/inspection.sock"), identity, 2, 0);
    assert!(
        Settings::decode(
            none.replace(
                "\"explanation_replay_events\":0",
                "\"explanation_replay_events\":1"
            )
            .as_bytes()
        )
        .is_err()
    );
    assert!(
        Settings::decode(
            none.replace(
                "\"explanation_disclosure\":\"none\"",
                "\"explanation_disclosure\":\"policy_and_selected_witnesses\""
            )
            .as_bytes()
        )
        .is_err()
    );
}

#[test]
fn profile_preflight_binds_server_scope_and_preserves_colliding_or_preexisting_paths() {
    let fixture = Fixture::new(2, 0);
    let before = fixture.files();
    fixture
        .settings
        .check_host(&fixture.config, &fixture.actor, &fixture.reviewer, &[])
        .unwrap();
    for path in [
        fixture.actor.socket.clone(),
        fixture.reviewer.socket(7),
        super::super::super::control::socket_path(&fixture.reviewer, 7),
    ] {
        let mut settings = fixture.settings.clone();
        settings.socket = path;
        assert!(
            settings
                .check_host(&fixture.config, &fixture.actor, &fixture.reviewer, &[])
                .is_err()
        );
    }
    assert!(
        fixture
            .settings
            .check_host(
                &fixture.config,
                &fixture.actor,
                &fixture.reviewer,
                &[fixture.settings.socket.as_path()]
            )
            .is_err()
    );
    let mut wrong = fixture.settings.clone();
    wrong.scope.principal += 1;
    assert!(
        wrong
            .check_host(&fixture.config, &fixture.actor, &fixture.reviewer, &[])
            .is_err()
    );
    wrong = fixture.settings.clone();
    wrong.supervisor =
        PeerPolicy::new(wrong.supervisor.uid(), wrong.supervisor.gid(), None).unwrap();
    assert!(
        wrong
            .check_host(&fixture.config, &fixture.actor, &fixture.reviewer, &[])
            .is_err()
    );
    let directory = fixture.settings.socket.parent().unwrap();
    fs::set_permissions(directory, fs::Permissions::from_mode(0o770)).unwrap();
    assert!(fixture.settings.check_directory().is_err());
    fs::set_permissions(directory, fs::Permissions::from_mode(0o750)).unwrap();
    fs::write(&fixture.settings.socket, b"preserved operator path").unwrap();
    assert!(
        fixture
            .settings
            .check_host(&fixture.config, &fixture.actor, &fixture.reviewer, &[])
            .is_err()
    );
    assert_eq!(
        fs::read(&fixture.settings.socket).unwrap(),
        b"preserved operator path"
    );
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn real_native_situation_and_opted_in_explanation_do_not_read_source_or_mutate_history() {
    let mut fixture = Fixture::new(3, 2);
    fixture.submit(7);
    let before = fixture.files();
    let owner = fixture.driver.supervisor().host().unwrap().inspect();
    let mut service = fixture.start();
    let situation = fixture.query(&mut service, fixture.request(Verb::Situation));
    known(&situation, "situation");
    assert_eq!(
        field(
            &situation,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "summary",
                "attempts"
            ]
        )
        .as_str(),
        Some("1")
    );
    let explanation = fixture.query(&mut service, fixture.request(Verb::Explain));
    known(&explanation, "explanation");
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .inspection
            .as_ref()
            .unwrap()
            .remaining_queries(),
        2
    );
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .explanation
            .as_ref()
            .unwrap()
            .remaining_queries(),
        1
    );
    assert_eq!(fixture.driver.supervisor().host().unwrap().inspect(), owner);
    assert_eq!(fixture.files(), before);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    service.retire();
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn inspection_only_refuses_policy_disclosure_while_preserving_its_real_read() {
    let mut fixture = Fixture::new(2, 0);
    fixture.submit(7);
    let before = fixture.files();
    let mut service = fixture.start();
    let refused = fixture.query(&mut service, fixture.request(Verb::Explain));
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .inspection
            .as_ref()
            .unwrap()
            .remaining_queries(),
        2
    );
    assert!(service.active.as_ref().unwrap().explanation.is_none());
    known(
        &fixture.query(&mut service, fixture.request(Verb::Situation)),
        "situation",
    );
    assert_eq!(fixture.files(), before);
}

#[test]
fn operator_cannot_spend_a_preinstalled_admission_snapshot_or_acquire_a_permit_purpose() {
    let mut fixture = Fixture::new(2, 0);
    fixture.snapshot();
    let mut request = fixture.request(Verb::Propose);
    request.arguments = ReadArguments::Propose {
        request: 7,
        proposal: fixture.proposal(),
    };
    request.idempotency_key = Some("7".into());
    let before = fixture.files();
    let mut service = fixture.start();
    let refused = fixture.query(&mut service, request);
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("PurposeMismatch")
    );
    assert_eq!(fixture.files(), before);
    // The positive actor control uses the SAME still-unconsumed snapshot and
    // complete proposal. No fresh set_snapshot or source read repairs the case.
    let ticket = fixture.port.submit(7, &fixture.proposal()).unwrap();
    assert_eq!(
        fixture.port.poll(&ticket),
        Knowledge::Pending { request: 7 }
    );
    known(
        &fixture.query(&mut service, fixture.request(Verb::Situation)),
        "situation",
    );
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn explicit_reconnect_continues_the_original_capture_and_never_renews_query_work() {
    let mut fixture = Fixture::new(2, 0);
    for request in 1..=17 {
        fixture.submit(request);
    }
    let before = fixture.files();
    let mut service = fixture.start();
    let first = fixture.query(&mut service, fixture.request(Verb::Situation));
    known(&first, "situation");
    let cursor = field(
        &first,
        &["result", "value", "knowledge", "value", "continuation"],
    )
    .as_str()
    .unwrap()
    .parse()
    .unwrap();
    let mut next = fixture.request(Verb::Situation);
    next.arguments = ReadArguments::SituationPage { cursor };
    let second = fixture.query(&mut service, next);
    known(&second, "situation");
    assert_eq!(
        field(
            &second,
            &["result", "value", "knowledge", "value", "offset"]
        )
        .as_str(),
        Some("16")
    );
    assert_eq!(
        field(
            &second,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "enumeration_complete"
            ]
        )
        .as_bool(),
        Some(true)
    );
    assert_eq!(
        field(&second, &["result", "value", "knowledge", "basis"]),
        field(&first, &["result", "value", "knowledge", "basis"])
    );
    let exhausted = fixture.query(&mut service, fixture.request(Verb::Situation));
    assert_eq!(
        field(&exhausted, &["envelope", "reason_code"]).as_str(),
        Some("BudgetExhausted")
    );
    let active = service.active.as_ref().unwrap();
    assert_eq!(active.inspection.as_ref().unwrap().remaining_queries(), 0);
    assert_eq!(active.session.status().connections_admitted, 3);
    assert_eq!(
        active.session.remaining_exchanges(),
        fixture.settings.exchange_limit - 3
    );
    assert_eq!(fixture.files(), before);
}

#[test]
fn rejected_kernel_peer_spends_only_candidate_allowance_and_receives_no_response() {
    let fixture = Fixture::new(2, 0);
    let mut rejected = fixture.settings.clone();
    let pid = rejected.operator.pid().unwrap();
    rejected.operator = PeerPolicy::new(
        rejected.operator.uid(),
        rejected.operator.gid(),
        Some(if pid == i32::MAX as u32 {
            pid - 1
        } else {
            pid + 1
        }),
    )
    .unwrap();
    let mut service = Service::start(
        &rejected,
        &fixture.actor,
        fixture.port.clone(),
        &fixture.driver,
        REGISTRATION,
        &deadline(),
    )
    .unwrap();
    let before = fixture.files();
    let mut socket = UnixStream::connect(&rejected.socket).unwrap();
    socket.write_all(b"malformed untrusted bytes\n").unwrap();
    service.checkpoint();
    assert_eq!(service.candidates, 1);
    assert_eq!(service.frames, 0);
    let active = service.active.as_ref().unwrap();
    assert_eq!(active.inspection.as_ref().unwrap().remaining_queries(), 2);
    assert_eq!(active.session.status().connections_admitted, 0);
    assert_eq!(
        active.session.remaining_exchanges(),
        fixture.settings.exchange_limit
    );
    socket.set_nonblocking(true).unwrap();
    let mut byte = [0];
    match socket.read(&mut byte) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
            ) => {}
        other => panic!("rejected operator socket was not closed unread: {other:?}"),
    }
    assert_eq!(fixture.files(), before);
    service.retire();
    let mut accepted = fixture.start();
    known(
        &fixture.query(&mut accepted, fixture.request(Verb::Situation)),
        "situation",
    );
}

#[test]
fn truncated_transport_retires_only_the_operator_without_consuming_actor_admission() {
    let mut fixture = Fixture::new(2, 0);
    fixture.snapshot();
    let before = fixture.files();
    let mut service = fixture.start();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket.write_all(b"{").unwrap();
    service.checkpoint();
    assert_eq!(service.frames, 0);
    assert!(service.active.is_some());
    socket.shutdown(Shutdown::Write).unwrap();
    for _ in 0..16 {
        service.checkpoint();
        if service.active.is_none() {
            break;
        }
    }
    assert_eq!(service.retirement, Some(Retirement::TransportFailure));
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.files(), before);
    let ticket = fixture.port.submit(7, &fixture.proposal()).unwrap();
    assert_eq!(
        fixture.port.poll(&ticket),
        Knowledge::Pending { request: 7 }
    );
    assert_eq!(fixture.config.source.status().read_attempts, 0);
}

#[test]
fn original_deadline_caps_sidecar_lifetime_and_retirement_releases_both_custodians() {
    let fixture = Fixture::new(2, 2);
    let expired = Deadline {
        started: Instant::now() - Duration::from_secs(1),
        wall: Duration::from_millis(1),
        ..deadline()
    };
    assert!(
        Service::start(
            &fixture.settings,
            &fixture.actor,
            fixture.port.clone(),
            &fixture.driver,
            REGISTRATION,
            &expired
        )
        .is_err()
    );
    assert!(!fixture.settings.socket.exists());
    let shorter = Deadline {
        wall: Duration::from_secs(2),
        ..deadline()
    };
    let mut service = Service::start(
        &fixture.settings,
        &fixture.actor,
        fixture.port.clone(),
        &fixture.driver,
        REGISTRATION,
        &shorter,
    )
    .unwrap();
    assert!(service.wall <= Duration::from_secs(2));
    known(
        &fixture.query(&mut service, fixture.request(Verb::Situation)),
        "situation",
    );
    let before = fixture.files();
    service.started = Instant::now() - Duration::from_secs(3);
    service.checkpoint();
    assert_eq!(service.retirement, Some(Retirement::Expired));
    assert!(service.active.is_none());
    assert!(!fixture.settings.socket.exists());
    assert_eq!(fixture.files(), before);
}

#[test]
fn weak_operator_service_cannot_keep_the_authority_lock_or_a_known_page_alive() {
    let mut fixture = Fixture::new(3, 0);
    for request in 1..=17 {
        fixture.submit(request);
    }
    let mut service = fixture.start();
    let first = fixture.query(&mut service, fixture.request(Verb::Situation));
    known(&first, "situation");
    let cursor = field(
        &first,
        &["result", "value", "knowledge", "value", "continuation"],
    )
    .as_str()
    .unwrap()
    .parse()
    .unwrap();
    let mut request = fixture.request(Verb::Situation);
    request.arguments = ReadArguments::SituationPage { cursor };
    drop(fixture.driver);
    // Opening the same original store succeeds while the operator sidecar and
    // its read custodians survive, proving that neither retains host authority.
    let (_reopened, _) =
        FileOversight::open(&fixture.config.store, fixture.config.profile.clone()).unwrap();
    let image = files(&fixture.config.store);
    let lost = query(&fixture.settings.socket, &mut service, request);
    assert_eq!(
        field(&lost, &["result", "value", "knowledge", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(
        field(&lost, &["result", "value", "knowledge", "reason"]).as_str(),
        Some("controller_unavailable")
    );
    assert_eq!(files(&fixture.config.store), image);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .inspection
            .as_ref()
            .unwrap()
            .remaining_queries(),
        2
    );
}

#[test]
fn malformed_complete_document_is_an_original_tower_refusal_not_sidecar_replacement() {
    let fixture = Fixture::new(2, 0);
    let mut service = fixture.start();
    let before = fixture.files();
    let mut socket = UnixStream::connect(&fixture.settings.socket).unwrap();
    socket.write_all(b"{\"operator\":true}\n").unwrap();
    let refused = strict_json::parse(
        &read_reply(&mut socket, &mut service),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap();
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("IntegrityFailure")
    );
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .inspection
            .as_ref()
            .unwrap()
            .remaining_queries(),
        2
    );
    drop(socket);
    known(
        &fixture.query(&mut service, fixture.request(Verb::Situation)),
        "situation",
    );
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .session
            .remaining_exchanges(),
        fixture.settings.exchange_limit - 2
    );
    assert_eq!(fixture.files(), before);
}

#[test]
fn final_permitted_exchange_or_connection_completes_before_original_limits_retire_the_listener() {
    for (candidates, connections, exchanges, expected) in [
        (8, 4, 1, Retirement::ExchangeLimit),
        (8, 1, 16, Retirement::ConnectionLimit),
        (1, 1, 16, Retirement::CandidateLimit),
    ] {
        let mut fixture = Fixture::new(2, 0);
        fixture.settings.candidate_limit = candidates;
        fixture.settings.connection_limit = connections;
        fixture.settings.exchange_limit = exchanges;
        let mut service = fixture.start();
        let before = fixture.files();
        // The last admitted connection/query still receives its complete real
        // observation; reaching a cap is never treated as permission to renew.
        known(
            &fixture.query(&mut service, fixture.request(Verb::Situation)),
            "situation",
        );
        for _ in 0..4 {
            service.checkpoint();
        }
        assert_eq!(service.retirement, Some(expected));
        assert!(service.active.is_none());
        assert!(!fixture.settings.socket.exists());
        assert_eq!(service.candidates, 1);
        assert_eq!(service.frames, 1);
        assert_eq!(fixture.files(), before);
    }
}

#[test]
fn numerically_identical_foreign_owner_cannot_supply_the_operator_tower_port() {
    for (inspection, explanation) in [(2, 0), (0, 2)] {
        let mut fixture = Fixture::new(inspection, explanation);
        if explanation != 0 {
            fixture.submit(7);
        }
        let foreign = Fixture::new(inspection, explanation);
        let before = fixture.files();
        let foreign_before = foreign.files();
        assert_eq!(fixture.actor.scope, foreign.actor.scope);
        assert!(
            Service::start(
                &fixture.settings,
                &fixture.actor,
                foreign.port.clone(),
                &fixture.driver,
                REGISTRATION,
                &deadline()
            )
            .is_err()
        );
        assert!(!fixture.settings.socket.exists());
        assert_eq!(fixture.files(), before);
        assert_eq!(foreign.files(), foreign_before);
        // An equal scope cannot substitute the original allocation, while the
        // same-owner positive service still obtains its selected native grant.
        let mut valid = fixture.start();
        if inspection != 0 {
            known(
                &fixture.query(&mut valid, fixture.request(Verb::Situation)),
                "situation",
            );
        } else {
            known(
                &fixture.query(&mut valid, fixture.request(Verb::Explain)),
                "explanation",
            );
        }
    }
}
