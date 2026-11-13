//! Actual native campaigns and kernel-authenticated sockets. These fixtures
//! establish the custodian boundary, not a claim about independent humans.
use super::super::{Config, PeerProfile};
use super::*;
use fa_reference::Snapshot;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::policy_governance::CampaignDisposition;
use fa_reference::action::{ActionSpec, ElapsedTick, VERSION};
use fa_reference::strict_json::{self, Json, Limits};
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[path = "tests/inspection.rs"]
pub(super) mod inspection;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = crate::test_directory::create(format!(
            "fa-gov-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ["actors", "operators", "reviewers", "governors"] {
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
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    service: Service,
    operator: operator::Service,
    driver: FileSupervisedDriver,
    config: Config,
    actor: Profile,
    reviewer: PeerProfile,
    operator_settings: operator::Settings,
    _root: Directory,
}

fn identity() -> PeerPolicy {
    let (socket, _other) = UnixStream::pair().unwrap();
    let peer = PeerCredentials::observe(&socket).unwrap();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap()
}
fn different(peer: PeerPolicy, offset: u32) -> PeerPolicy {
    let pid = peer.pid().unwrap();
    let selected = if pid <= i32::MAX as u32 - offset {
        pid + offset
    } else {
        pid - offset
    };
    PeerPolicy::new(peer.uid(), peer.gid(), Some(selected)).unwrap()
}
fn peer_json(peer: PeerPolicy) -> String {
    format!(
        "{{\"uid\":{},\"gid\":{},\"pid\":{}}}",
        peer.uid(),
        peer.gid(),
        peer.pid()
            .map_or_else(|| "null".into(), |pid| pid.to_string())
    )
}
fn scope_numbers() -> &'static str {
    "{\"tenant\":1,\"principal\":2,\"run\":3,\"branch\":4,\"authority\":5}"
}
fn settings_json(socket: &Path, supervisor: PeerPolicy, governor: PeerPolicy) -> String {
    format!(
        concat!(
            "{{\"schema\":\"fa.policy-governor-service/1\",\"scope\":{},",
            "\"socket\":{},\"supervisor\":{},\"governor\":{},\"candidate_limit\":8,",
            "\"connection_limit\":6,\"exchange_limit\":24,\"runtime_ms\":15000,\"poll_ms\":1,",
            "\"campaigns\":{{\"cases\":16,\"input_bytes\":1048576,\"max_campaigns\":8}},",
            "\"campaign_limit\":8,\"report_bytes\":1048576}}"
        ),
        scope_numbers(),
        protocol::quote(socket.to_str().unwrap()),
        peer_json(supervisor),
        peer_json(governor)
    )
}
fn deadline() -> Deadline {
    Deadline {
        logical: ElapsedTick(20_000),
        started: Instant::now(),
        wall: Duration::from_secs(15),
    }
}

impl Fixture {
    fn new() -> Self {
        let root = Directory::new();
        let mut config = Config::decode(include_bytes!(
            "../../../../../fixtures/supervised_publication.json"
        ))
        .unwrap();
        config.store = root.0.join("store");
        let server = identity();
        let actor = Profile {
            request: 7,
            scope: config.profile.delivery.scope,
            socket: root.0.join("actors/actor.sock"),
            supervisor: server,
            actor: different(server, 1),
            candidates: 8,
            connections: 4,
            exchanges: 32,
            runtime_ms: 15_000,
            poll_ms: 1,
            reply_ms: 20,
        };
        let reviewer = PeerProfile::decode(format!(concat!("{{\"version\":1,\"clock\":\"unix_milliseconds\",",
            "\"scope\":{},\"reviewer_id\":77,\"socket_directory\":{},\"supervisor\":{},\"reviewer\":{},",
            "\"candidate_limit\":8,\"runtime_ms\":15000,\"poll_ms\":1}}"), scope_numbers(),
            protocol::quote(root.0.join("reviewers").to_str().unwrap()), peer_json(server),
            peer_json(different(server, 3))).as_bytes()).unwrap();
        let operator_settings = operator::Settings::decode(format!(concat!(
            "{{\"schema\":\"fa.tower-operator-service/1\",\"scope\":{},\"socket\":{},",
            "\"supervisor\":{},\"operator\":{},\"candidate_limit\":8,\"connection_limit\":4,",
            "\"exchange_limit\":24,\"runtime_ms\":15000,\"poll_ms\":1,\"inspection_queries\":0,",
            "\"explanation_queries\":0,\"explanation_replay_events\":0,\"explanation_disclosure\":\"none\",",
            "\"policy_promotion\":{{\"queries\":8,\"max_replay_events\":{},\"authority\":\"promote_approved_policy\"}}}}"),
            scope_numbers(), protocol::quote(root.0.join("operators/operator.sock").to_str().unwrap()),
            peer_json(server), peer_json(different(server, 2)),
            fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_POLICY_PROMOTION_REPLAY_EVENTS).as_bytes()).unwrap();
        let settings = Settings::decode(
            settings_json(&root.0.join("governors/governor.sock"), server, server).as_bytes(),
        )
        .unwrap();
        settings
            .check_host(&config, &actor, &reviewer, &operator_settings, &[7, 8])
            .unwrap();
        operator_settings
            .check_host(&config, &actor, &reviewer, &[&settings.socket])
            .unwrap();
        // The ordinary service's first-image bootstrap is covered separately.
        // This native fixture opts in BEFORE the first actual proposal.
        let (mut host, _human) =
            FileOversight::create(&config.store, config.profile.clone()).unwrap();
        let campaigns = settings.campaign_requirement();
        let governor = host
            .enable_policy_campaigns(host.revision(), campaigns.limits, campaigns.max_campaigns)
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1000))
            .unwrap();
        let spec = ActionSpec {
            version: VERSION,
            scope: config.profile.delivery.scope,
            target: Some(config.profile.delivery.target),
            payload: b"original".to_vec(),
            required_witnesses: Vec::new(),
            policy_epoch: 0,
            deadline: ElapsedTick(10_000),
            units: 8,
        };
        host.propose(
            host.revision(),
            1,
            spec,
            Snapshot {
                semantic_epoch: 1,
                complete: true,
                values: BTreeMap::from([(7, b"ok".to_vec())]),
            },
        )
        .unwrap();
        let (port, driver) = host.into_supervised_driver();
        let operator =
            operator::Service::start(&operator_settings, &actor, port, &driver, 11, &deadline())
                .unwrap();
        let service = Service::start(&settings, &actor, governor, &deadline()).unwrap();
        Self {
            service,
            operator,
            driver,
            config,
            actor,
            reviewer,
            operator_settings,
            _root: root,
        }
    }

    fn review_request(&self, operation: u64) -> Vec<u8> {
        let host = self.driver.supervisor().host().unwrap();
        let cut = host.inspect();
        format!(concat!("{{\"schema\":\"fa.policy-governor-request/1\",\"scope\":{},\"verb\":\"policy-replay\",",
            "\"operation\":\"{}\",\"expected\":{{\"journal_revision\":\"{}\",\"control_sequence\":\"{}\",",
            "\"authority_epoch\":\"{}\",\"policy_generation\":\"1\"}},",
            "\"candidate\":{{\"schema_version\":\"fa.policy-replay-request/1\",\"expected_policy_generation\":1,",
            "\"limits\":{{\"cases\":16,\"input_bytes\":1048576,\"output_bytes\":1048576}},",
            "\"candidate\":{{\"generation\":2,\"nodes\":[{{\"op\":\"payload_at_most\",\"maximum\":3}}]}}}}}}"),
            protocol::scope_json(self.actor.scope), operation, cut.revision, cut.control.sequence,
            cut.control.ledger.epoch).into_bytes()
    }

    fn approve_request(&self, response: &[u8]) -> Vec<u8> {
        let reply = parsed(response);
        let value = reply.get("knowledge").unwrap().get("value").unwrap();
        format!(
            concat!(
                "{{\"schema\":\"fa.policy-governor-request/1\",\"scope\":{},\"verb\":\"promote\",",
                "\"phase\":\"approve\",\"campaign\":{},\"expected_journal_revision\":{},",
                "\"reviewed_report\":{},\"accept_newly_reviewable\":false}}"
            ),
            protocol::scope_json(self.actor.scope),
            protocol::json(reply.get("campaign").unwrap()),
            protocol::json(value.get("journal_revision").unwrap()),
            protocol::json(value.get("reviewed_report").unwrap())
        )
        .into_bytes()
    }

    fn respond(&mut self, bytes: &[u8]) -> Vec<u8> {
        let request = protocol::decode(bytes, self.service.settings.report_bytes).unwrap();
        let reply = self
            .service
            .respond(&request, &mut self.driver, &mut self.operator);
        reply[4..].to_vec()
    }
    fn image(&self) -> Vec<u8> {
        fs::read(self.config.store.join("delivery.bin")).unwrap()
    }
    fn connect(&self) -> UnixStream {
        let socket = UnixStream::connect(&self.service.settings.socket).unwrap();
        socket.set_nonblocking(true).unwrap();
        socket
    }
    fn turn(&mut self) {
        self.service
            .checkpoint(&mut self.driver, &mut self.operator);
    }
    fn exchange(&mut self, socket: &mut UnixStream, request: &[u8]) -> Vec<u8> {
        socket.write_all(&frame(request.to_vec())).unwrap();
        let mut response = Vec::new();
        for _ in 0..4096 {
            self.turn();
            let mut bytes = [0_u8; 8192];
            loop {
                match socket.read(&mut bytes) {
                    Ok(0) => panic!("governor closed before bounded reply"),
                    Ok(count) => response.extend_from_slice(&bytes[..count]),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => panic!("governor response failed: {error}"),
                }
                assert!(response.len() <= protocol::MAX_RESPONSE_BYTES + 4);
                if response.len() >= 4 {
                    let size = u32::from_be_bytes(response[..4].try_into().unwrap()) as usize;
                    if response.len() == size + 4 {
                        return response[4..].to_vec();
                    }
                    assert!(response.len() < size + 4, "unsolicited governor suffix");
                }
            }
        }
        panic!("bounded governor exchange made no complete reply");
    }
}

fn parsed(bytes: &[u8]) -> Json {
    strict_json::parse(
        bytes,
        Limits {
            max_bytes: protocol::MAX_RESPONSE_BYTES,
            max_depth: 16,
            max_items: 16_512,
            max_string_bytes: protocol::MAX_REPORT_BYTES,
        },
    )
    .unwrap()
}
fn reason(bytes: &[u8]) -> String {
    let reply = parsed(bytes);
    assert!(reply.get("knowledge").is_none());
    reply
        .get("error")
        .unwrap()
        .get("code")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

/// Original native documents for the independent client validator's tests.
/// Returned bytes carry historical protocol evidence and no live capability.
pub(super) fn original_documents() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut fixture = Fixture::new();
    let review = fixture.review_request(801);
    let report = fixture.respond(&review);
    let approve = fixture.approve_request(&report);
    let acknowledged = fixture.respond(&approve);
    assert_eq!(
        parsed(&acknowledged)
            .get("knowledge")
            .unwrap()
            .get("state")
            .unwrap()
            .as_str(),
        Some("known")
    );
    (review, report, approve, acknowledged)
}

#[test]
fn complete_original_report_and_explicit_approval_delegate_only_one_original_key() {
    let mut fixture = Fixture::new();
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    let review = fixture.review_request(801);
    let report = fixture.respond(&review);
    let after_review = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after_review.revision, before.revision + 1);
    assert_eq!(after_review.control, before.control);
    let document = parsed(&report);
    let report_text = document
        .get("knowledge")
        .unwrap()
        .get("value")
        .unwrap()
        .get("reviewed_report")
        .unwrap()
        .as_str()
        .unwrap();
    let full = parsed(report_text.as_bytes());
    let details = full.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(details.get("case_count").unwrap().as_u64(), Some(1));
    assert_eq!(
        details
            .get("cut")
            .unwrap()
            .get("journal_revision")
            .unwrap()
            .as_u64(),
        Some(before.revision)
    );
    let saved = fixture.image();
    assert_eq!(fixture.respond(&review), report);
    assert_eq!(fixture.image(), saved);
    assert_eq!(fixture.service.campaigns_entered, 1);
    let approve = fixture.approve_request(&report);
    let ack = fixture.respond(&approve);
    assert_eq!(
        parsed(&ack)
            .get("knowledge")
            .unwrap()
            .get("state")
            .unwrap()
            .as_str(),
        Some("known")
    );
    let after = fixture.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.revision, before.revision + 2);
    assert_eq!(
        after.control, before.control,
        "approval cannot promote or refund"
    );
    assert!(!fixture.operator.promotion_available());
    assert_eq!(
        fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .current_policy()
            .unwrap()
            .generation(),
        1
    );
    assert_eq!(
        fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .policy_campaign(801)
            .unwrap()
            .observed_disposition(),
        CampaignDisposition::Approved
    );
    let saved = fixture.image();
    assert_eq!(fixture.respond(&approve), ack);
    assert_eq!(fixture.image(), saved);
    let another = fixture.review_request(802);
    assert_eq!(reason(&fixture.respond(&another)), "operation_limit");
}

#[test]
fn forged_or_stale_review_never_approves_and_fresh_explicit_campaign_remains_possible() {
    let mut fixture = Fixture::new();
    let review = fixture.review_request(801);
    let report = fixture.respond(&review);
    let approve = fixture.approve_request(&report);
    let forged = String::from_utf8(approve.clone())
        .unwrap()
        .replace("retained_exact_policy_corpus", "selected_favorable_subset");
    assert_ne!(forged.as_bytes(), approve);
    let saved = fixture.image();
    assert_eq!(
        reason(&fixture.respond(forged.as_bytes())),
        "report_mismatch"
    );
    assert_eq!(fixture.image(), saved);
    assert!(!fixture.service.approval_entered && fixture.operator.promotion_available());
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1001)).unwrap();
    }
    let saved = fixture.image();
    assert_eq!(reason(&fixture.respond(&approve)), "stale_predecessor");
    assert_eq!(fixture.image(), saved);
    assert!(!fixture.service.approval_entered);
    let next = fixture.review_request(802);
    let report = fixture.respond(&next);
    let approve = fixture.approve_request(&report);
    let ack = fixture.respond(&approve);
    assert_eq!(
        parsed(&ack)
            .get("knowledge")
            .unwrap()
            .get("state")
            .unwrap()
            .as_str(),
        Some("known")
    );
}

#[test]
fn kernel_verified_lost_acknowledgment_reconnect_returns_original_approval_without_reminting() {
    let mut fixture = Fixture::new();
    let review = fixture.review_request(801);
    let mut socket = fixture.connect();
    let report = fixture.exchange(&mut socket, &review);
    let approve = fixture.approve_request(&report);
    socket.write_all(&frame(approve.clone())).unwrap();
    // Lose the application acknowledgment, even if the kernel had accepted
    // its bytes. The original role, native approval and grant are not retried.
    drop(socket);
    for _ in 0..64 {
        fixture.turn();
        if fixture.service.approval_entered {
            break;
        }
    }
    assert!(fixture.service.approval_entered);
    assert!(!fixture.operator.promotion_available());
    let saved = fixture.image();
    for _ in 0..4 {
        fixture.turn();
    }
    let mut reconnect = fixture.connect();
    let ack = fixture.exchange(&mut reconnect, &approve);
    assert_eq!(
        parsed(&ack)
            .get("knowledge")
            .unwrap()
            .get("state")
            .unwrap()
            .as_str(),
        Some("known")
    );
    assert_eq!(fixture.image(), saved);
    assert_eq!(fixture.service.connections, 2);
    assert_eq!(fixture.service.exchanges, 3);
    fixture.service.retire();
    assert!(
        !fixture.operator.promotion_available(),
        "governor retirement must not undo delegation"
    );
    assert_eq!(fixture.image(), saved);
}

#[test]
fn rejected_kernel_identity_reads_no_frame_and_enters_no_campaign() {
    let mut fixture = Fixture::new();
    // Select a different real-policy expectation; no claimed credentials are
    // ever supplied to verify. The positive test uses this same kernel gate.
    fixture.service.settings.governor = different(identity(), 99);
    let request = fixture.review_request(801);
    let saved = fixture.image();
    let mut socket = fixture.connect();
    socket.write_all(&frame(request)).unwrap();
    fixture.turn();
    assert_eq!(fixture.service.candidates, 1);
    assert_eq!(fixture.service.connections, 0);
    assert_eq!(fixture.service.read, 0);
    assert_eq!(fixture.service.exchanges, 0);
    assert_eq!(fixture.service.campaigns_entered, 0);
    assert_eq!(fixture.image(), saved);
    assert!(fixture.operator.promotion_available());
}

#[test]
fn scope_and_frame_refusals_preserve_native_work_and_original_budget() {
    let mut fixture = Fixture::new();
    let request = fixture.review_request(801);
    let foreign = String::from_utf8(request)
        .unwrap()
        .replace("\"principal\":\"2\"", "\"principal\":\"22\"");
    let saved = fixture.image();
    assert_eq!(reason(&fixture.respond(foreign.as_bytes())), "audience");
    let mut socket = fixture.connect();
    socket
        .write_all(&((protocol::MAX_REQUEST_BYTES + 1) as u32).to_be_bytes())
        .unwrap();
    fixture.turn();
    assert_eq!(fixture.service.read, 4);
    assert_eq!(fixture.service.exchanges, 1);
    assert_eq!(fixture.service.campaigns_entered, 0);
    assert_eq!(fixture.image(), saved);
    assert!(fixture.operator.promotion_available());
}

#[test]
fn governor_profile_requires_disjoint_roles_and_all_scheduled_paths() {
    let fixture = Fixture::new();
    let selected = fixture.service.settings.clone();
    for other in [
        fixture.actor.actor,
        fixture.operator_settings.operator,
        fixture.reviewer.reviewer_policy().unwrap(),
    ] {
        let mut overlapping = selected.clone();
        overlapping.governor = other;
        assert!(
            overlapping
                .check_host(
                    &fixture.config,
                    &fixture.actor,
                    &fixture.reviewer,
                    &fixture.operator_settings,
                    &[7, 8]
                )
                .unwrap_err()
                .contains("credential")
        );
    }
    for path in [
        fixture.reviewer.socket(8),
        super::super::super::control::socket_path(&fixture.reviewer, 8),
        fixture.config.store.join("governor.sock"),
    ] {
        let mut collision = selected.clone();
        collision.socket = path;
        assert!(
            collision
                .check_host(
                    &fixture.config,
                    &fixture.actor,
                    &fixture.reviewer,
                    &fixture.operator_settings,
                    &[7, 8]
                )
                .is_err()
        );
    }
    assert!(!profile::disjoint(
        identity(),
        PeerPolicy::new(identity().uid(), identity().gid(), None).unwrap()
    ));
    let original = settings_json(&selected.socket, selected.supervisor, selected.governor);
    for invalid in [
        original.replace("\"campaign_limit\":8", "\"campaign_limit\":9"),
        original.replace("\"report_bytes\":1048576", "\"report_bytes\":1048577"),
        original.replace("\"poll_ms\":1", "\"poll_ms\":0"),
        original.replace("/governors/", "/governors//"),
        original.replace(
            "\"campaign_limit\":8",
            "\"campaign_limit\":8,\"approve\":true",
        ),
    ] {
        assert_ne!(invalid, original);
        assert!(Settings::decode(invalid.as_bytes()).is_err());
    }
}
