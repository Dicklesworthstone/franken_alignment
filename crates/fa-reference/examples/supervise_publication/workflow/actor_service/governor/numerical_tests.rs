//! Paid campaign work enters an original numerical owner, with actual native
//! decoder history and a retained proposal rather than fabricated report bytes.
use super::*;
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorPort, MAX_POLICY_PROMOTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;

mod data {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/decoder_inputs.rs"
    ));
}

fn selected_profile(fixture: &Fixture, selected: Option<(usize, usize)>) -> Settings {
    let ordinary = settings_json(
        &fixture._root.0.join("governors/governor.sock"),
        fixture.actor.supervisor,
        fixture.actor.supervisor,
    );
    let document = match selected {
        Some((maximum, events)) => ordinary.replacen(
            '{',
            &format!(
                "{{\"numerical_replay\":{{\"max_replay_events\":{maximum},\"events\":{events}}},"
            ),
            1,
        ),
        None => ordinary,
    };
    Settings::decode(document.as_bytes()).unwrap()
}

fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]),
    }
}

fn numerical_fixture(
    allowance: impl FnOnce(usize) -> Option<(usize, usize)>,
) -> (Fixture, FileActorPort<FileOversight>) {
    // Reuse only the independent profiles and socket layout. The numerical
    // host, campaign role and actor port below are newly created together.
    let mut fixture = Fixture::new();
    fixture.service.retire();
    fixture.operator.retire();
    fixture.config.store = fixture._root.0.join("numerical-store");
    let (mut host, _human) =
        FileOversight::create(&fixture.config.store, fixture.config.profile.clone()).unwrap();
    let campaigns = fixture.service.settings.campaign_requirement();
    let governor = host
        .enable_policy_campaigns(host.revision(), campaigns.limits, campaigns.max_campaigns)
        .unwrap();
    let decoder = DecoderProfile::new(
        DecoderIdentity {
            tenant: 1,
            model: 9,
            model_generation: 1,
            tokenizer_generation: 1,
            profile_generation: 1,
        },
        DecoderShape {
            vocabulary: 2,
            hidden: 2,
            intermediate: 2,
            layers: 1,
            query_heads: 1,
            cache_heads: 1,
            context: 4,
        },
        1e-5,
        10000.0,
    )
    .unwrap();
    let configuration = FileDecoderConfig::new(
        decoder,
        data::weights(),
        data::monitor(3.0),
        data::sampling(),
        5,
        DecoderBindingLimits::default(),
    )
    .unwrap();
    host.enable_decoder(host.revision(), configuration).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000))
        .unwrap();
    let before = host.decoder_inspection().unwrap().numerical;
    let stepped = host
        .advance_decoder_forced(
            host.revision(),
            before.actor_revision,
            before.position,
            0,
            DecoderBudget {
                scalar_products: MAX_DECODER_PRODUCTS,
            },
        )
        .unwrap()
        .unwrap();
    assert!(matches!(stepped, MonitoredStep::Released(_)));
    host.propose(
        host.revision(),
        1,
        ActionSpec {
            version: VERSION,
            scope: fixture.config.profile.delivery.scope,
            target: Some(fixture.config.profile.delivery.target),
            payload: b"original".to_vec(),
            required_witnesses: Vec::new(),
            policy_epoch: 0,
            deadline: ElapsedTick(10_000),
            units: 8,
        },
        snapshot(),
    )
    .unwrap();
    let work = usize::try_from(host.revision()).unwrap() + 1;
    let settings = selected_profile(&fixture, allowance(work));
    fixture
        .operator_settings
        .policy_promotion
        .as_mut()
        .unwrap()
        .numerical_replay_events = Some(MAX_POLICY_PROMOTION_REPLAY_EVENTS);
    let (actor, driver) = host.into_supervised_driver();
    fixture.driver = driver;
    fixture.operator = operator::Service::start(
        &fixture.operator_settings,
        &fixture.actor,
        actor.clone(),
        &fixture.driver,
        11,
        &deadline(),
    )
    .unwrap();
    fixture.service = Service::start(&settings, &fixture.actor, governor, &deadline()).unwrap();
    (fixture, actor)
}

fn get(fixture: &Fixture, query: u64) -> Vec<u8> {
    format!(
        "{{\"schema\":\"fa.policy-governor-request/1\",\"scope\":{},\"verb\":\"get\",\"query\":\"{query}\"}}",
        protocol::scope_json(fixture.actor.scope)
    )
    .into_bytes()
}

fn assert_known(response: &[u8]) {
    assert_eq!(
        parsed(response)
            .get("knowledge")
            .and_then(|knowledge| knowledge.get("state"))
            .and_then(Json::as_str),
        Some("known"),
        "{}",
        String::from_utf8_lossy(response)
    );
}

fn preserve_snapshot(fixture: &mut Fixture) {
    let revision = fixture.driver.supervisor().host().unwrap().revision();
    fixture
        .driver
        .supervisor_mut()
        .set_snapshot(revision, Some(snapshot()))
        .unwrap();
}

fn consume_original_snapshot(fixture: &Fixture, actor: &FileActorPort<FileOversight>) {
    actor
        .submit(
            999,
            &ActorProposal {
                target: fixture.config.profile.delivery.target,
                payload: b"next".to_vec(),
                units: 4,
                deadline: ElapsedTick(10_000),
                expected_policy_epoch: 0,
            },
        )
        .expect("preflight refusal must preserve the original unused admission snapshot");
}

/// Genuine numerical Get response for the executable client's independent
/// validation tests. These bytes contain no native campaign capability.
pub(in super::super) fn original_numerical_document() -> (Settings, Vec<u8>, Vec<u8>) {
    let (mut fixture, _actor) = numerical_fixture(|work| Some((512, 2 * work + 1)));
    let request = get(&fixture, 501);
    let response = fixture.respond(&request);
    assert_known(&response);
    (fixture.service.settings.clone(), request, response)
}

#[test]
fn numerical_profile_requires_explicit_closed_and_bounded_work() {
    let root = Directory::new();
    let peer = identity();
    let ordinary = settings_json(&root.0.join("governors/governor.sock"), peer, peer);
    assert!(
        Settings::decode(ordinary.as_bytes())
            .unwrap()
            .numerical_replay
            .is_none()
    );
    let decode = |value: &str| {
        Settings::decode(
            ordinary
                .replacen('{', &format!("{{\"numerical_replay\":{value},"), 1)
                .as_bytes(),
        )
    };
    let selected = decode("{\"max_replay_events\":64,\"events\":128}")
        .unwrap()
        .numerical_replay
        .unwrap();
    assert_eq!(selected.max_replay_events, 64);
    assert_eq!(selected.events, 128);
    for value in [
        "null",
        "{}",
        "{\"max_replay_events\":0,\"events\":1}",
        "{\"max_replay_events\":64,\"events\":0}",
        "{\"max_replay_events\":1,\"events\":25}",
        "{\"max_replay_events\":\"64\",\"events\":128}",
        "{\"max_replay_events\":64,\"events\":\"128\"}",
        "{\"max_replay_events\":64,\"events\":128,\"refill\":true}",
        "{\"max_replay_events\":64,\"events\":128,\"events\":128}",
    ] {
        assert!(
            decode(value).is_err(),
            "accepted numerical allowance {value}"
        );
    }
    assert!(
        decode(&format!(
            "{{\"max_replay_events\":{},\"events\":1}}",
            MAX_POLICY_PROMOTION_REPLAY_EVENTS + 1
        ))
        .is_err()
    );
}

#[test]
fn original_decoder_get_review_and_approval_spend_each_complete_cut_once() {
    let (mut fixture, _actor) = numerical_fixture(|work| Some((512, 2 * work + 1)));
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    let numerical = fixture
        .driver
        .supervisor()
        .host()
        .unwrap()
        .decoder_inspection()
        .unwrap()
        .numerical;
    let work = usize::try_from(before.revision).unwrap() + 1;
    let inspection = get(&fixture, 501);
    let status = fixture.respond(&inspection);
    assert_known(&status);
    assert_eq!(fixture.respond(&inspection), status);
    assert_eq!(
        fixture.service.remaining_numerical_replay_events,
        Some(2 * work + 1)
    );
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        before
    );

    let review = fixture.review_request(801);
    let report = fixture.respond(&review);
    assert_known(&report);
    let response = parsed(&report);
    let text = response
        .get("knowledge")
        .unwrap()
        .get("value")
        .unwrap()
        .get("reviewed_report")
        .unwrap()
        .as_str()
        .unwrap();
    let original_report = parsed(text.as_bytes());
    assert_eq!(
        original_report
            .get("knowledge")
            .unwrap()
            .get("value")
            .unwrap()
            .get("case_count")
            .unwrap()
            .as_u64(),
        Some(1)
    );
    assert_eq!(
        fixture.service.remaining_numerical_replay_events,
        Some(work + 1)
    );
    let after_review = fixture.image();
    assert_eq!(fixture.respond(&review), report);
    assert_eq!(fixture.image(), after_review);
    assert_eq!(
        fixture.service.remaining_numerical_replay_events,
        Some(work + 1)
    );

    let approval = fixture.approve_request(&report);
    let acknowledged = fixture.respond(&approval);
    assert_known(&acknowledged);
    assert_eq!(fixture.service.remaining_numerical_replay_events, Some(0));
    assert!(fixture.service.approval_entered);
    assert_eq!(fixture.service.campaigns_entered, 1);
    assert!(!fixture.operator.promotion_available());
    {
        let host = fixture.driver.supervisor().host().unwrap();
        let after = host.inspect();
        assert_eq!(after.revision, before.revision + 2);
        assert_eq!(after.control, before.control);
        assert_eq!(host.current_policy().unwrap().generation(), 1);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(
            host.policy_campaign(801).unwrap().observed_disposition(),
            CampaignDisposition::Approved
        );
    }
    let saved = fixture.image();
    assert_eq!(fixture.respond(&approval), acknowledged);
    assert_eq!(fixture.respond(&review), report);
    assert_known(&fixture.respond(&get(&fixture, 502)));
    assert_eq!(fixture.service.remaining_numerical_replay_events, Some(0));
    assert_eq!(fixture.image(), saved);
}

#[test]
fn omitted_or_short_numerical_work_preserves_the_original_snapshot_and_owner() {
    for selection in 0..3 {
        let (mut fixture, actor) = numerical_fixture(|work| match selection {
            0 => None,
            1 => Some((512, work - 1)),
            2 => Some((work - 1, work)),
            _ => unreachable!(),
        });
        let before = fixture.driver.supervisor().host().unwrap().inspect();
        let numerical = fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap();
        let image = fixture.image();
        let remaining = fixture.service.remaining_numerical_replay_events;
        preserve_snapshot(&mut fixture);
        let review = fixture.review_request(801);
        let refused = fixture.respond(&review);
        assert_eq!(
            reason(&refused),
            if selection == 0 {
                "unsupported"
            } else {
                "capacity"
            }
        );
        assert_eq!(fixture.respond(&review), refused);
        assert_eq!(
            fixture.driver.supervisor().host().unwrap().inspect(),
            before
        );
        assert_eq!(
            fixture
                .driver
                .supervisor()
                .host()
                .unwrap()
                .decoder_inspection()
                .unwrap(),
            numerical
        );
        assert_eq!(fixture.image(), image);
        assert_eq!(fixture.service.remaining_numerical_replay_events, remaining);
        assert_eq!(fixture.service.campaigns_entered, 0);
        assert!(!fixture.service.approval_entered);
        assert!(fixture.service.prepared.is_empty());
        assert!(fixture.service.cache.iter().all(|cached| !cached.entered));
        consume_original_snapshot(&fixture, &actor);
    }
}

#[test]
fn one_short_approval_preserves_the_pending_campaign_and_unused_snapshot() {
    let (mut fixture, actor) = numerical_fixture(|work| Some((512, 2 * work)));
    let review = fixture.review_request(801);
    let report = fixture.respond(&review);
    assert_known(&report);
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    let remaining = fixture.service.remaining_numerical_replay_events;
    assert_eq!(remaining, Some(usize::try_from(before.revision).unwrap()));
    let image = fixture.image();
    preserve_snapshot(&mut fixture);
    let approval = fixture.approve_request(&report);
    let refused = fixture.respond(&approval);
    assert_eq!(reason(&refused), "capacity");
    assert_eq!(fixture.respond(&approval), refused);
    assert_eq!(fixture.service.remaining_numerical_replay_events, remaining);
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        before
    );
    assert_eq!(fixture.image(), image);
    assert_eq!(fixture.service.campaigns_entered, 1);
    assert!(!fixture.service.approval_entered);
    assert!(fixture.operator.promotion_available());
    assert_eq!(
        fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .policy_campaign(801)
            .unwrap()
            .observed_disposition(),
        CampaignDisposition::Pending
    );
    consume_original_snapshot(&fixture, &actor);
}

#[test]
fn entered_numerical_storage_failure_stays_charged_and_never_replays_on_retry() {
    let (mut fixture, _actor) = numerical_fixture(|work| Some((512, 2 * work + 1)));
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    let work = usize::try_from(before.revision).unwrap() + 1;
    let image = fixture.image();
    // The production store refuses a pre-existing stage file through its actual
    // create_new path. This is an observed I/O failure, not a mocked outcome.
    let pending = fixture.config.store.join("delivery.pending");
    fs::write(&pending, b"existing unacknowledged stage").unwrap();
    let review = fixture.review_request(801);
    let refused = fixture.respond(&review);
    assert_eq!(reason(&refused), "unconfirmed");
    assert_eq!(
        fixture.service.remaining_numerical_replay_events,
        Some(work + 1)
    );
    assert_eq!(fixture.service.campaigns_entered, 1);
    assert!(fixture.service.cache.iter().any(|cached| cached.entered));
    assert!(fixture.service.prepared.is_empty());
    assert!(!fixture.service.approval_entered);
    {
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.revision(), before.revision);
        assert!(host.storage_failure().is_some());
    }
    assert_eq!(fixture.image(), image);
    assert_eq!(
        fs::read(&pending).unwrap(),
        b"existing unacknowledged stage"
    );
    assert_eq!(fixture.respond(&review), refused);
    assert_eq!(
        fixture.service.remaining_numerical_replay_events,
        Some(work + 1)
    );
    assert_eq!(fixture.image(), image);
    assert_eq!(
        fs::read(&pending).unwrap(),
        b"existing unacknowledged stage"
    );
}

#[test]
fn failed_complete_report_keeps_the_native_request_and_numerical_charge() {
    let (mut fixture, _actor) = numerical_fixture(|work| Some((512, 2 * work + 1)));
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    let work = usize::try_from(before.revision).unwrap() + 1;
    let complete = fixture.review_request(801);
    let limited = String::from_utf8(complete.clone())
        .unwrap()
        .replace("\"output_bytes\":1048576", "\"output_bytes\":1")
        .into_bytes();
    assert_ne!(limited, complete);
    let refused = fixture.respond(&limited);
    assert_eq!(reason(&refused), "capacity");
    assert_eq!(
        fixture.service.remaining_numerical_replay_events,
        Some(work + 1)
    );
    assert_eq!(fixture.service.campaigns_entered, 1);
    assert!(fixture.service.prepared.is_empty());
    assert!(!fixture.service.approval_entered);
    {
        let host = fixture.driver.supervisor().host().unwrap();
        assert_eq!(host.revision(), before.revision + 1);
        assert_eq!(
            host.policy_campaign(801).unwrap().observed_disposition(),
            CampaignDisposition::Pending
        );
    }
    let image = fixture.image();
    assert_eq!(fixture.respond(&limited), refused);
    assert_eq!(fixture.image(), image);
    assert_eq!(
        fixture.service.remaining_numerical_replay_events,
        Some(work + 1)
    );
    assert_eq!(reason(&fixture.respond(&complete)), "idempotency_conflict");
    assert_eq!(fixture.image(), image);
}
