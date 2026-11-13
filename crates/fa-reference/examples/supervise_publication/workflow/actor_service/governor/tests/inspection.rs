//! Original native cuts and the production response cache, without a listener.
//! Socket credentials and transport are exercised by the separate socket tests.
use super::*;
use fa_reference::action::consequence::delivery::StopRequest;

struct Native {
    service: Service,
    driver: FileSupervisedDriver,
    config: Config,
    _root: Directory,
}

impl Native {
    fn new(source: bool) -> Self {
        let root = Directory::new();
        let mut config = Config::decode(include_bytes!(
            "../../../../../../fixtures/supervised_publication.json"
        ))
        .unwrap();
        config.store = root.0.join("store");
        let peer = identity();
        let settings = Settings::decode(
            settings_json(&root.0.join("governors/governor.sock"), peer, peer).as_bytes(),
        )
        .unwrap();
        let (mut host, _human) =
            FileOversight::create(&config.store, config.profile.clone()).unwrap();
        let campaigns = settings.campaign_requirement();
        let role = host
            .enable_policy_campaigns(host.revision(), campaigns.limits, campaigns.max_campaigns)
            .unwrap();
        if source {
            host.enable_file_source(host.revision(), config.source_policy)
                .unwrap();
        }
        host.observe_time(host.revision(), ElapsedTick(1000))
            .unwrap();
        if !source {
            host.propose(
                host.revision(),
                1,
                ActionSpec {
                    version: VERSION,
                    scope: config.profile.delivery.scope,
                    target: Some(config.profile.delivery.target),
                    payload: b"original".to_vec(),
                    required_witnesses: Vec::new(),
                    policy_epoch: 0,
                    deadline: ElapsedTick(10_000),
                    units: 8,
                },
                Snapshot {
                    semantic_epoch: 1,
                    complete: true,
                    values: BTreeMap::from([(7, b"ok".to_vec())]),
                },
            )
            .unwrap();
        }
        let (_port, driver) = host.into_supervised_driver();
        // The genuine native role and driver are retained. There is deliberately
        // no pathname listener or claim that credentials were verified here.
        let service = Service {
            settings,
            socket: None,
            connection: None,
            role: Some(role),
            started: Instant::now(),
            wall: Duration::from_secs(15),
            candidates: 0,
            connections: 0,
            exchanges: 0,
            read: 0,
            written: 0,
            io_calls: 0,
            campaigns_entered: 0,
            approval_entered: false,
            prepared: BTreeMap::new(),
            cache: Vec::new(),
            cache_bytes: 0,
            retired: false,
        };
        Self {
            service,
            driver,
            config,
            _root: root,
        }
    }

    fn request(&self, query: u64) -> Vec<u8> {
        format!(
            "{{\"schema\":\"fa.policy-governor-request/1\",\"scope\":{},\"verb\":\"get\",\"query\":\"{query}\"}}",
            protocol::scope_json(self.service.settings.scope),
        )
        .into_bytes()
    }

    fn respond(&mut self, bytes: &[u8]) -> Vec<u8> {
        let request = protocol::decode(bytes, self.service.settings.report_bytes).unwrap();
        let driver = &mut self.driver;
        let response =
            self.service
                .respond_with(&request, |service, index| match &request.command {
                    Command::Inspect { .. } => service.inspect(&request, driver),
                    Command::Review { .. } => service.review(&request, index, driver),
                    Command::Approve { .. } => panic!("inspection fixture has no operator"),
                });
        response[4..].to_vec()
    }

    fn image(&self) -> Vec<u8> {
        fs::read(self.config.store.join("delivery.bin")).unwrap()
    }

    fn advance(&mut self) {
        let mut host = self.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1001)).unwrap();
    }
}

/// These bytes originate at a real guarded native owner. The client fixtures
/// need neither a pathname socket nor a hand-written successful response.
pub(in super::super) fn original_document() -> (Settings, Vec<u8>, Vec<u8>) {
    let mut native = Native::new(false);
    let request = native.request(501);
    let response = native.respond(&request);
    assert!(parsed(&response).get("knowledge").is_some());
    (native.service.settings.clone(), request, response)
}

fn review_from_inspection(response: &[u8], operation: u64) -> Vec<u8> {
    let response = parsed(response);
    let value = response.get("knowledge").unwrap().get("value").unwrap();
    let expected = Json::Object(
        [
            "journal_revision",
            "control_sequence",
            "authority_epoch",
            "policy_generation",
        ]
        .into_iter()
        .map(|name| (name.to_owned(), value.get(name).unwrap().clone()))
        .collect(),
    );
    let bounds = value.get("campaigns").unwrap();
    format!(
        concat!(
            "{{\"schema\":\"fa.policy-governor-request/1\",\"scope\":{},",
            "\"verb\":\"policy-replay\",\"operation\":\"{}\",\"expected\":{},",
            "\"candidate\":{{\"schema_version\":\"fa.policy-replay-request/1\",",
            "\"expected_policy_generation\":{},",
            "\"limits\":{{\"cases\":{},\"input_bytes\":{},\"output_bytes\":{}}},",
            "\"candidate\":{{\"generation\":2,\"nodes\":[{{\"op\":\"payload_at_most\",\"maximum\":3}}]}}}}}}"
        ),
        protocol::json(response.get("scope").unwrap()),
        operation,
        protocol::json(&expected),
        protocol::decimal(value.get("policy_generation").unwrap()).unwrap(),
        protocol::decimal(bounds.get("cases").unwrap()).unwrap(),
        protocol::decimal(bounds.get("input_bytes").unwrap()).unwrap(),
        protocol::decimal(value.get("report_bytes").unwrap()).unwrap(),
    )
    .into_bytes()
}

#[test]
fn native_inspection_preserves_cut_and_provides_an_independently_usable_review_predecessor() {
    let mut native = Native::new(false);
    let before = native.driver.supervisor().host().unwrap().inspect();
    let image = native.image();
    let query = native.request(501);
    let response = native.respond(&query);
    let document = parsed(&response);
    let knowledge = document.get("knowledge").unwrap();
    let value = knowledge.get("value").unwrap();
    let basis = knowledge.get("basis").unwrap();
    assert_eq!(document.get("verb").unwrap().as_str(), Some("get"));
    assert_eq!(document.get("phase").unwrap().as_str(), Some("inspect"));
    assert_eq!(document.get("query").unwrap().as_str(), Some("501"));
    assert!(document.get("campaign").is_none());
    assert_eq!(knowledge.get("state").unwrap().as_str(), Some("known"));
    assert_eq!(
        basis.get("origin").unwrap().as_str(),
        Some("acknowledged_owner")
    );
    assert_eq!(basis.get("scope"), document.get("scope"));
    assert_eq!(basis.get("query"), document.get("query"));
    assert!(basis.get("campaign").is_none());
    assert_eq!(basis.get("journal_revision"), value.get("journal_revision"));
    assert_eq!(
        protocol::decimal(value.get("journal_revision").unwrap()),
        Some(before.revision)
    );
    assert_eq!(
        protocol::decimal(value.get("control_sequence").unwrap()),
        Some(before.control.sequence)
    );
    assert_eq!(
        protocol::decimal(value.get("authority_epoch").unwrap()),
        Some(before.control.ledger.epoch)
    );
    assert_eq!(
        protocol::decimal(value.get("policy_generation").unwrap()),
        Some(1)
    );
    assert_eq!(value.get("stopped"), Some(&Json::Bool(false)));
    assert_eq!(value.get("source_interrupted"), Some(&Json::Null));
    assert_eq!(native.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(native.image(), image);
    assert_eq!(native.service.campaigns_entered, 0);
    assert!(!native.service.approval_entered);
    assert!(native.service.prepared.is_empty());
    assert!(matches!(
        native
            .driver
            .supervisor()
            .host()
            .unwrap()
            .policy_campaign(501),
        Err(JournalError::Contract(Error::Missing))
    ));

    // No journal access is used to construct this Review. The same numeric ID
    // remains a distinct campaign identity, not an inspection collision.
    let review = review_from_inspection(&response, 501);
    let reviewed = native.respond(&review);
    assert!(parsed(&reviewed).get("knowledge").is_some());
    assert_eq!(native.service.campaigns_entered, 1);
    assert_eq!(
        native.driver.supervisor().host().unwrap().revision(),
        before.revision + 1
    );
    assert_eq!(native.respond(&query), response);
    assert_eq!(native.respond(&review), reviewed);
    assert!(!native.service.approval_entered);
}

#[test]
fn native_inspection_retry_is_historical_and_a_new_query_observes_the_next_cut() {
    let mut native = Native::new(false);
    let first = native.request(601);
    let original = native.respond(&first);
    let cached_bytes = native.service.cache_bytes;
    native.advance();
    let image = native.image();
    assert_eq!(native.respond(&first), original);
    assert_eq!(native.service.cache_bytes, cached_bytes);
    assert_eq!(native.service.cache.len(), 1);
    assert_eq!(native.image(), image);
    let stale = review_from_inspection(&original, 701);
    assert_eq!(reason(&native.respond(&stale)), "stale_predecessor");
    assert_eq!(native.service.campaigns_entered, 0);
    assert_eq!(native.image(), image);

    let fresh = native.respond(&native.request(602));
    let new = parsed(&fresh);
    let old = parsed(&original);
    assert_eq!(
        protocol::decimal(
            new.get("knowledge")
                .unwrap()
                .get("value")
                .unwrap()
                .get("journal_revision")
                .unwrap()
        ),
        protocol::decimal(
            old.get("knowledge")
                .unwrap()
                .get("value")
                .unwrap()
                .get("journal_revision")
                .unwrap()
        )
        .map(|revision| revision + 1),
    );
    assert_eq!(native.image(), image);
    let reviewed = native.respond(&review_from_inspection(&fresh, 702));
    assert!(parsed(&reviewed).get("knowledge").is_some());
    assert_eq!(native.service.campaigns_entered, 1);
}

#[test]
fn native_inspection_preserves_source_latch_and_observes_terminal_stop_without_authorizing_work() {
    let mut native = Native::new(true);
    let before = native
        .driver
        .supervisor()
        .host()
        .unwrap()
        .file_source_status()
        .unwrap();
    let first = native.respond(&native.request(801));
    assert_eq!(
        parsed(&first)
            .get("knowledge")
            .unwrap()
            .get("value")
            .unwrap()
            .get("source_interrupted"),
        Some(&Json::Bool(before.interrupted)),
    );
    {
        let mut host = native.driver.supervisor_mut().host_mut().unwrap();
        let cut = host.inspect();
        host.request_stop(
            cut.revision,
            StopRequest {
                operation: 77,
                expected_control_sequence: cut.control.sequence,
                expected_authority_epoch: cut.control.ledger.epoch,
            },
        )
        .unwrap();
    }
    let stopped = native.driver.supervisor().host().unwrap().inspect();
    let source = native
        .driver
        .supervisor()
        .host()
        .unwrap()
        .file_source_status();
    let image = native.image();
    let response = native.respond(&native.request(802));
    assert_eq!(
        parsed(&response)
            .get("knowledge")
            .unwrap()
            .get("value")
            .unwrap()
            .get("stopped"),
        Some(&Json::Bool(true)),
    );
    assert_eq!(
        native.driver.supervisor().host().unwrap().inspect(),
        stopped
    );
    assert_eq!(
        native
            .driver
            .supervisor()
            .host()
            .unwrap()
            .file_source_status(),
        source
    );
    assert_eq!(native.image(), image);
    assert_eq!(native.service.campaigns_entered, 0);
    assert!(!native.service.approval_entered);
    assert_eq!(native.respond(&native.request(801)), first);
}

#[test]
fn inspection_cache_cannot_expand_or_borrow_a_different_audience() {
    let mut native = Native::new(false);
    let image = native.image();
    let request = native.request(901);
    let foreign = String::from_utf8(request.clone())
        .unwrap()
        .replace("\"branch\":\"4\"", "\"branch\":\"6\"");
    assert_eq!(reason(&native.respond(foreign.as_bytes())), "audience");
    assert!(native.service.cache.is_empty());
    let first = native.respond(&request);
    for query in 902..901 + MAX_CACHE_ENTRIES as u64 {
        assert!(
            parsed(&native.respond(&native.request(query)))
                .get("knowledge")
                .is_some()
        );
    }
    assert_eq!(native.service.cache.len(), MAX_CACHE_ENTRIES);
    let cache_bytes = native.service.cache_bytes;
    assert_eq!(reason(&native.respond(&native.request(999))), "capacity");
    assert_eq!(native.respond(&request), first);
    assert_eq!(native.service.cache_bytes, cache_bytes);
    assert_eq!(native.service.cache.len(), MAX_CACHE_ENTRIES);
    assert_eq!(native.image(), image);
    assert_eq!(native.service.campaigns_entered, 0);
    assert!(!native.service.approval_entered);
}
