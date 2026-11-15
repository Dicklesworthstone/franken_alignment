//! Both authenticated protocols consume one original disclosure. Fixture helper
//! decisions exercise publication plumbing, not empirical detector quality.

use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::discovery::{
    PrecheckCapabilitiesRequest, PrecheckCapabilitiesResponse,
};
use fa_reference::address_resolver::{Address, Noun};
use fa_reference::tower::{ReadArguments, TowerRequest, Verb, encode_request};

fn request(config: &Config, verb: Verb, arguments: ReadArguments) -> String {
    let scope = config.profile.delivery.scope;
    let address = Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(GENERATION),
    )
    .unwrap();
    let mut request = TowerRequest::new("shared-disclosure", address, verb);
    request.arguments = arguments;
    encode_request(&request).unwrap()
}

pub(super) fn check_document(config: &Config) -> String {
    request(
        config,
        Verb::Precheck,
        ReadArguments::Precheck {
            query: 9001,
            proposal: candidate(config),
        },
    )
}

fn capabilities_document(config: &Config) -> String {
    request(
        config,
        Verb::Capabilities,
        ReadArguments::Capabilities {
            query: 9001,
            expected_policy_epoch: 0,
        },
    )
}

fn native_value<'a>(response: &'a Json, kind: &str) -> &'a Json {
    let result = field(response, "result");
    assert_eq!(field(result, "kind").as_str(), Some(kind));
    let value = field(result, "value");
    assert_eq!(field(value, "query").as_str(), Some("9001"));
    value
}

fn assert_basis(knowledge: &Json) {
    let basis = field(knowledge, "basis");
    for (name, expected) in [
        ("query", "9001"),
        ("disclosure", "81"),
        ("policy_epoch", "0"),
        ("observed_at", "1000"),
    ] {
        assert_eq!(field(basis, name).as_str(), Some(expected));
    }
}

pub(super) fn passed(response: &Json) {
    let native = native_value(response, "precheck");
    assert_eq!(field(native, "precheck").as_bool(), Some(true));
    assert_eq!(field(native, "status").as_str(), Some("ok"));
    let knowledge = field(native, "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    assert_eq!(field(knowledge, "value").as_str(), Some("precheck_passed"));
    assert_eq!(field(knowledge, "review_required").as_bool(), Some(true));
    assert_basis(knowledge);
}

pub(super) fn withheld(response: &Json) {
    let native = native_value(response, "precheck");
    let knowledge = field(native, "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("withheld"));
    assert_eq!(
        field(knowledge, "authority_required").as_str(),
        Some("exact_policy_disclosure")
    );
}

fn capabilities(response: &Json, remaining: usize, cached: bool) {
    let native = native_value(response, "capabilities");
    assert_eq!(field(native, "informational").as_bool(), Some(true));
    let knowledge = field(native, "knowledge");
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    assert_basis(knowledge);
    let value = field(knowledge, "value");
    assert_eq!(
        field(value, "remaining_queries").as_str(),
        Some(remaining.to_string().as_str())
    );
    assert_eq!(
        field(value, "policy_disclosure").as_str(),
        Some("exact_predicates")
    );
    assert_eq!(field(value, "policy_contents").as_str(), Some("withheld"));
    assert_eq!(field(value, "source_refresh").as_str(), Some("unavailable"));
    assert_eq!(field(value, "effect_authority").as_bool(), Some(false));
    assert_eq!(field(value, "review_required").as_bool(), Some(true));
    assert_eq!(
        field(value, "witness_cache").as_str(),
        Some(if cached {
            "exact_witnesses"
        } else {
            "unavailable"
        })
    );
}

fn native_capabilities(profile: &Profile, socket: &Path) -> PrecheckCapabilitiesResponse {
    let mut client = PrecheckPeerClient::connect_capabilities(
        UnixStream::connect(socket).unwrap(),
        profile.supervisor,
        PrecheckCapabilitiesRequest {
            query: 9001,
            expected_policy_epoch: 0,
        },
        0,
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() {
            return response;
        }
        assert!(!report.status.closed());
        pause(1);
    }
}

#[test]
fn both_protocols_share_one_grant_without_source_reads_or_full_policy_leakage() {
    for version in 1..=3 {
        let root = Directory::new();
        let config = configured(&root);
        evidence(&root, &config);
        let (actor, reviewer) = profiles(&root, &config);
        let precheck_socket = root.0.join("actors/precheck.sock");
        let settings =
            Settings::decode(settings_document(&precheck_socket, version, 6).as_bytes()).unwrap();
        let check = check_document(&config);
        let mut denied = candidate(&config);
        denied.units = 513;
        let denied = request(
            &config,
            Verb::Precheck,
            ReadArguments::Precheck {
                query: 9001,
                proposal: denied,
            },
        );
        let discover = capabilities_document(&config);
        let proposed = proposal_document(&config, 7);
        let candidate = candidate(&config);
        let source = root.0.join("evidence.json");
        let store = config.store.clone();
        let profile = actor.clone();
        let review_socket = reviewer.socket(7);
        let peer = std::thread::spawn(move || {
            wait(&precheck_socket);
            let before = store_files(&store);
            let source_bytes = fs::read(&source).unwrap();
            fs::remove_file(&source).unwrap();

            let native = native_capabilities(&profile, &precheck_socket);
            let Ok(Knowledge::Known { value, basis }) = native.result else {
                panic!("expected the original native discovery");
            };
            assert_eq!(value.remaining_queries(), 5);
            assert_eq!(value.policy_contents_available(), version == 3);
            assert_eq!(value.witness_cache_enabled(), version != 1);
            assert_eq!(
                (basis.disclosure, basis.query, basis.policy_epoch),
                (81, 9001, 0)
            );

            let mut socket = connect(&profile);
            capabilities(&raw_exchange(&mut socket, &discover), 4, version != 1);
            let refused = raw_exchange(&mut socket, &denied);
            let refused_knowledge = field(native_value(&refused, "precheck"), "knowledge");
            assert_eq!(field(refused_knowledge, "state").as_str(), Some("known"));
            assert_eq!(
                field(refused_knowledge, "value").as_str(),
                Some("denied_by_policy")
            );
            assert_basis(refused_knowledge);
            super::passed(query(&profile, &precheck_socket, candidate.clone()));
            passed(&raw_exchange(&mut socket, &check));
            capabilities(&raw_exchange(&mut socket, &discover), 0, version != 1);
            let exhausted = raw_exchange(&mut socket, &check);
            let exhausted = native_value(&exhausted, "precheck");
            assert_eq!(field(exhausted, "reason").as_str(), Some("capacity"));
            assert_eq!(
                query(&profile, &precheck_socket, candidate).result,
                Err(PrecheckWireError::Capacity)
            );
            assert_eq!(store_files(&store), before, "queries mutated the owner");
            assert!(
                !review_socket.exists(),
                "queries launched helper/human review"
            );
            assert!(
                !source.exists(),
                "queries reconstructed or refreshed the source"
            );

            // Reconnection retains the same host-installed weak port and grant.
            drop(socket);
            let mut socket = connect(&profile);
            let exhausted = raw_exchange(&mut socket, &discover);
            assert_eq!(
                field(native_value(&exhausted, "capabilities"), "reason").as_str(),
                Some("capacity")
            );
            fs::write(source, source_bytes).unwrap();
            let accepted = raw_exchange(&mut socket, &proposed);
            assert_eq!(
                field(knowledge(&accepted), "state").as_str(),
                Some("pending")
            );
            // Recording the request retires the shared admission-time grant,
            // while the same socket remains available for original observations.
            withheld(&raw_exchange(&mut socket, &check));
        });
        let human = human(reviewer.clone());
        serve_with_precheck(config, &actor, &reviewer, &settings).unwrap();
        peer.join().unwrap();
        human.join().unwrap();
        let config = configured(&root);
        let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(disk.executions, 1);
        assert_eq!(disk.payload, b"network");
        assert_eq!(
            (disk.control.ledger.charged, disk.control.ledger.reserved),
            (7, 0)
        );
        assert!(disk.stop.is_none());
    }
}

#[test]
fn invalid_tower_reads_preserve_work_and_stale_policy_queries_spend_the_same_grant() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let precheck_socket = root.0.join("actors/precheck.sock");
    let settings = Settings::decode(settings_document(&precheck_socket, 1, 3).as_bytes()).unwrap();
    let check = check_document(&config);
    let discover = capabilities_document(&config);
    let proposed = proposal_document(&config, 7);
    let store = config.store.clone();
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        wait(&precheck_socket);
        let before = store_files(&store);
        let mut socket = connect(&profile);
        for invalid in [
            check.replace("@11", "@12"),
            check.replace("fa://1/", "fa://99/"),
            check.replace("\"query\":\"9001\"", "\"query\":\"0\""),
        ] {
            let refused = raw_exchange(&mut socket, &invalid);
            assert_eq!(
                field(field(&refused, "result"), "kind").as_str(),
                Some("refused")
            );
        }
        let stale = check.replace(
            "\"expected_policy_epoch\":\"0\"",
            "\"expected_policy_epoch\":\"1\"",
        );
        let stale = raw_exchange(&mut socket, &stale);
        let stale = field(native_value(&stale, "precheck"), "knowledge");
        assert_eq!(field(stale, "state").as_str(), Some("stale"));
        assert_eq!(field(stale, "generation").as_str(), Some("1"));
        assert_eq!(field(stale, "current").as_str(), Some("0"));
        capabilities(&raw_exchange(&mut socket, &discover), 1, false);
        passed(&raw_exchange(&mut socket, &check));
        assert_eq!(store_files(&store), before);
        raw_exchange(&mut socket, &proposed)
    });
    let human = human(reviewer.clone());
    serve_with_precheck(config, &actor, &reviewer, &settings).unwrap();
    let response = peer.join().unwrap();
    human.join().unwrap();
    assert_eq!(
        field(knowledge(&response), "state").as_str(),
        Some("pending")
    );
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert!(disk.stop.is_none());
}

#[test]
fn ordinary_tower_ingress_does_not_invent_an_unselected_disclosure() {
    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let source = root.0.join("evidence.json");
    let source_bytes = fs::read(&source).unwrap();
    fs::remove_file(&source).unwrap();
    let check = check_document(&config);
    let discover = capabilities_document(&config);
    let proposed = proposal_document(&config, 7);
    let profile = actor.clone();
    let peer = std::thread::spawn(move || {
        let mut socket = connect(&profile);
        for document in [&discover, &check] {
            let response = raw_exchange(&mut socket, document);
            assert_eq!(
                field(field(&response, "envelope"), "reason_code").as_str(),
                Some("CapabilityUnsupported")
            );
            assert_eq!(
                field(field(&response, "result"), "kind").as_str(),
                Some("refused")
            );
        }
        assert!(!source.exists());
        fs::write(source, source_bytes).unwrap();
        raw_exchange(&mut socket, &proposed)
    });
    let human = human(reviewer.clone());
    serve_tower(config, &actor, &reviewer, false, 1000).unwrap();
    let response = peer.join().unwrap();
    human.join().unwrap();
    assert_eq!(
        field(knowledge(&response), "state").as_str(),
        Some("pending")
    );
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert!(disk.stop.is_none());
}

#[test]
fn original_precheck_deadline_revokes_tower_without_ending_publication_lifetime() {
    use std::sync::Arc;

    let root = Directory::new();
    let config = configured(&root);
    evidence(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    let precheck_socket = root.0.join("actors/precheck.sock");
    let settings = Settings::decode(
        settings_document(&precheck_socket, 1, 3)
            .replace("\"runtime_ms\":15000", "\"runtime_ms\":5000")
            .as_bytes(),
    )
    .unwrap();
    let check = check_document(&config);
    let proposed = proposal_document(&config, 7);
    let profile = actor.clone();
    let tick = Arc::new(AtomicU64::new(1000));
    let peer_tick = tick.clone();
    let peer = std::thread::spawn(move || {
        wait(&precheck_socket);
        let mut socket = connect(&profile);
        passed(&raw_exchange(&mut socket, &check));
        peer_tick.store(6001, Ordering::SeqCst);
        let started = Instant::now();
        while precheck_socket.exists() {
            assert!(started.elapsed() < Duration::from_secs(10));
            pause(1);
        }
        withheld(&raw_exchange(&mut socket, &check));
        raw_exchange(&mut socket, &proposed)
    });
    let human = human(reviewer.clone());
    serve_selected_ingress(
        config,
        &actor,
        &reviewer,
        None,
        false,
        None,
        None,
        Some(&settings),
        Some(GENERATION),
        None,
        || ElapsedTick(tick.load(Ordering::SeqCst)),
    )
    .unwrap();
    let response = peer.join().unwrap();
    human.join().unwrap();
    assert_eq!(
        field(knowledge(&response), "state").as_str(),
        Some("pending")
    );
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1);
    assert_eq!(disk.control.ledger.charged, 7);
    assert!(disk.stop.is_none());
}
