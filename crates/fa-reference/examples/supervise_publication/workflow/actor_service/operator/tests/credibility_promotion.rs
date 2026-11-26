//! Original independent labels reach the same live, authenticated operator.
//! Fixture verdicts establish composition, not empirical helper quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileIndependentEvaluator;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileCredibilityPromotionBasis, MAX_CREDIBILITY_PROMOTION_QUERIES,
    MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::joint_credibility::JointReplayBudget;

#[path = "../../../../../../tests/support/file_credibility_promotion.rs"]
mod native;

fn selection(queries: usize, events: usize) -> String {
    format!(r#"{{"queries":{queries},"max_replay_events":{events},"authority":"promote_qualified_credibility"}}"#)
}

fn selected(profile: &str, value: &str) -> String {
    profile.replacen('{', &format!("{{\"credibility_promotion\":{value},"), 1)
}

fn fixture(hold: u64) -> (Fixture, native::Directory, FileIndependentEvaluator) {
    let mut fixture = Fixture::new(1, 0);
    let (storage, mut host, human, evaluator) = native::ready(hold, JointReplayBudget::default());
    host.observe_time(host.revision(), ElapsedTick(0)).unwrap();
    let (port, driver) = host.into_supervised_driver();
    fixture.config.store = storage.0.clone();
    fixture.config.profile = native::profile(hold);
    fixture.port = port;
    fixture.driver = driver;
    fixture.human = human;
    fixture.settings = Settings::decode(selected(
        &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
        &selection(4, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS),
    ).as_bytes()).unwrap();
    fixture.settings.connection_limit = 8;
    fixture.settings.response_bytes = 65_536;
    (fixture, storage, evaluator)
}

fn next(fixture: &Fixture, operation: u64) -> TowerRequest {
    let mut request = fixture.request(Verb::Next);
    request.arguments = ReadArguments::CredibilityPromotionNext { operation };
    request
}

fn promotion(fixture: &Fixture, operation: u64) -> TowerRequest {
    let host = fixture.driver.supervisor().host().unwrap();
    let state = host.inspect();
    let report = host.credibility_report().unwrap();
    let mut request = fixture.request(Verb::Promote);
    request.idempotency_key = Some(operation.to_string());
    request.arguments = ReadArguments::PromoteCredibility {
        operation,
        expected: FileCredibilityPromotionBasis {
            scope: fixture.actor.scope,
            journal_revision: state.revision,
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
            evaluation_revision: report.revision,
            congress_generation: 1,
            policy_generation: report.policy_generation,
            recorded_at: state.control.ledger.elapsed,
        },
    };
    request
}

#[test]
fn credibility_profile_is_explicit_closed_and_has_separate_numerical_work() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let profile = profile_json(Path::new("/operator/credibility.sock"), identity, 1, 0);
    for compatible in [profile.clone(), selected(&profile, "null")] {
        assert!(Settings::decode(compatible.as_bytes()).unwrap().credibility_promotion.is_none());
    }
    let valid = selection(2, 1024);
    let only = profile_json(Path::new("/operator/credibility.sock"), identity, 0, 0);
    let settings = Settings::decode(selected(&only, &valid).as_bytes()).unwrap();
    assert!(settings.policy_promotion.is_none());
    assert!(settings.governance_ledger.is_none());
    assert!(settings.check_generated_operator().is_err());
    assert!(settings.check_learned_operator().is_err());
    let paid = valid.replacen('{', "{\"numerical_replay_events\":1024,", 1);
    let settings = Settings::decode(selected(&only, &paid).as_bytes()).unwrap();
    settings.check_generated_operator().unwrap();
    settings.check_learned_operator().unwrap();
    for invalid in [
        "{}".to_owned(), "[]".into(), "false".into(),
        selection(0, 1024), selection(MAX_CREDIBILITY_PROMOTION_QUERIES + 1, 1024),
        selection(2, 0), selection(2, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS + 1),
        valid.replace("promote_qualified_credibility", "promote_approved_policy"),
        valid.replace("\"queries\":2", "\"queries\":\"2\""),
        valid.replacen('{', "{\"operations\":2,", 1),
        valid.replacen('{', "{\"weights\":{},", 1),
        valid.replacen('{', "{\"queries\":2,", 1),
        valid.replacen('{', "{\"numerical_replay_events\":0,", 1),
        valid.replacen('{', "{\"numerical_replay_events\":1025,", 1),
    ] {
        assert!(Settings::decode(selected(&profile, &invalid).as_bytes()).is_err(), "accepted {invalid}");
    }
    assert!(Settings::decode(selected(&only, "null").as_bytes()).is_err());
}

#[test]
fn live_independent_labels_enable_one_original_weight_promotion_without_reopening() {
    let (mut fixture, _storage, evaluator) = fixture(0);
    let mut service = fixture.start();
    let original = fixture.files();
    let pending = fixture.query(&mut service, next(&fixture, 901));
    assert_eq!(field(&pending, &["result", "value", "knowledge", "value", "can_promote"]).as_bool(), Some(false));
    assert_eq!(fixture.files(), original);
    assert_eq!(service.active.as_ref().unwrap().credibility_promotion.as_ref().unwrap().remaining_operations(), 1);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        native::calibrate(&mut host, &evaluator);
        assert!(host.joint_credibility_report(host.credibility_report().unwrap().revision).unwrap().qualified());
    }
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    let original = fixture.files();
    let planned = fixture.query(&mut service, next(&fixture, 901));
    assert_eq!(field(&planned, &["result", "kind"]).as_str(), Some("credibility-promotion-next"));
    assert_eq!(field(&planned, &["result", "value", "knowledge", "value", "can_promote"]).as_bool(), Some(true));
    assert_eq!(field(&planned, &["result", "value", "knowledge", "value", "action", "arguments", "kind"]).as_str(), Some("credibility_promotion"));
    assert_eq!(fixture.files(), original);
    let request = promotion(&fixture, 901);
    let applied = fixture.query(&mut service, request.clone());
    assert_eq!(field(&applied, &["envelope", "status"]).as_str(), Some("ok"));
    assert_eq!(field(&applied, &["result", "kind"]).as_str(), Some("credibility-promotion"));
    {
        let host = fixture.driver.supervisor().host().unwrap();
        let receipt = host.credibility_promotion(901).unwrap();
        assert_eq!(receipt.change.previous.generation, 1);
        assert_eq!(receipt.change.current.generation, 2);
        assert_eq!(receipt.change.current.members["a"].weight, 5);
        assert_eq!(receipt.change.current.members["b"].weight, 5);
        assert_eq!(host.inspect().revision, before.revision + 1);
        assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged);
        assert_eq!(host.inspect().executions, before.executions);
        evaluator.check_owner(&host).unwrap();
    }
    let committed = fixture.files();
    let repeated = fixture.query(&mut service, request);
    assert_eq!(field(&repeated, &["result"]), field(&applied, &["result"]));
    assert_eq!(fixture.files(), committed);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    assert_eq!(service.active.as_ref().unwrap().credibility_promotion.as_ref().unwrap().remaining_operations(), 0);
    assert!(service.active.as_ref().unwrap().policy_promotion.is_none());
    assert!(service.governor.is_none());
}

#[test]
fn live_joint_regression_refuses_marginally_qualified_weights_and_stale_cuts_do_not_enter() {
    for hold in [0, 5] {
        let (mut fixture, _storage, evaluator) = fixture(hold);
        {
            let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
            native::calibrate(&mut host, &evaluator);
        }
        let mut service = fixture.start();
        let original = fixture.files();
        let planned = fixture.query(&mut service, next(&fixture, 902));
        assert_eq!(field(&planned, &["result", "value", "knowledge", "value", "can_promote"]).as_bool(), Some(hold == 0));
        if hold == 5 {
            assert_eq!(field(&planned, &["result", "value", "knowledge", "value", "block_reason"]).as_str(), Some("joint_unqualified"));
            let denied = fixture.query(&mut service, promotion(&fixture, 902));
            assert_eq!(field(&denied, &["envelope", "status"]).as_str(), Some("error"));
        } else {
            let mut stale = promotion(&fixture, 902);
            if let ReadArguments::PromoteCredibility { expected, .. } = &mut stale.arguments {
                expected.evaluation_revision += 1;
            } else {
                unreachable!();
            }
            let refused = fixture.query(&mut service, stale);
            assert_eq!(field(&refused, &["envelope", "reason_code"]).as_str(), Some("StaleGeneration"));
            assert_eq!(service.active.as_ref().unwrap().credibility_promotion.as_ref().unwrap().remaining_operations(), 1);
        }
        assert_eq!(fixture.files(), original);
        assert!(fixture.driver.supervisor().host().unwrap().credibility_promotion(902).is_err());
    }
}

#[test]
fn omitted_grant_and_uninstalled_joint_contract_never_create_promotion_authority() {
    let fixture = Fixture::new(1, 0);
    let mut service = fixture.start();
    let original = fixture.files();
    let reply = fixture.query(&mut service, next(&fixture, 903));
    assert_eq!(field(&reply, &["envelope", "status"]).as_str(), Some("error"));
    assert!(service.active.as_ref().unwrap().credibility_promotion.is_none());
    assert_eq!(fixture.files(), original);
    drop(service);

    let mut fixture = Fixture::new(1, 0);
    fixture.settings = Settings::decode(selected(
        &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
        &selection(1, MAX_CREDIBILITY_PROMOTION_REPLAY_EVENTS),
    ).as_bytes()).unwrap();
    let original = fixture.files();
    assert!(Service::start(&fixture.settings, &fixture.actor, fixture.port.clone(),
        &fixture.driver, REGISTRATION, &deadline()).is_err());
    assert_eq!(fixture.files(), original);
    assert!(!fixture.settings.socket.exists());
}
