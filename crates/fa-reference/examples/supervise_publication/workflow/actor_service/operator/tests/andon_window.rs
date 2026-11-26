//! A separately authenticated operator can complete original occurrence-window
//! accounting while the independently held evaluator retains all label custody.
use super::*;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileIndependentEvaluator;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileAndonWindowBasis, MAX_ANDON_WINDOW_QUERIES, MAX_ANDON_WINDOW_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::AndonBudget;

#[path = "../../../../../../tests/support/file_andon_window.rs"]
mod native;

fn budget_json(budget: AndonBudget) -> String {
    format!(
        r#"{{"window":{},"start":{},"end":{},"false_stops_allowed":{},"cost_per_stop":{}}}"#,
        budget.window, budget.start, budget.end, budget.false_stops_allowed, budget.cost_per_stop
    )
}

fn selection(budget: AndonBudget, queries: usize, events: usize) -> String {
    format!(
        r#"{{"budget":{},"queries":{},"max_replay_events":{},"authority":"register_original_andon_window"}}"#,
        budget_json(budget),
        queries,
        events
    )
}

fn selected(profile: &str, value: &str) -> String {
    profile.replacen('{', &format!("{{\"andon_window\":{value},"), 1)
}

fn fixture(budget: AndonBudget) -> (Fixture, native::Directory, FileIndependentEvaluator) {
    let mut fixture = Fixture::new(1, 0);
    let storage = native::Directory::new();
    let mut profile = native::profile(0);
    // Bind the original owner to the live service's clock domain at creation.
    // Bootstrap remains clock-free so registration cannot acquire clock custody.
    profile.delivery.clock_domain = crate::config::CLOCK_DOMAIN;
    let (mut host, human) = FileOversight::create(&storage.0, profile.clone()).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
        .unwrap();
    let evaluator = host
        .enable_credibility(host.revision(), native::protocol())
        .unwrap();
    host.enable_andon(host.revision(), &native::config()).unwrap();
    let (port, driver) = host.into_supervised_driver();
    fixture.config.store = storage.0.clone();
    fixture.actor.scope = profile.delivery.scope;
    fixture.config.profile = profile;
    fixture.port = port;
    fixture.driver = driver;
    fixture.human = human;
    fixture.settings = Settings::decode(
        selected(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
            &selection(budget, 4, MAX_ANDON_WINDOW_REPLAY_EVENTS),
        )
        .as_bytes(),
    )
    .unwrap();
    fixture.settings.connection_limit = 8;
    (fixture, storage, evaluator)
}

fn next(fixture: &Fixture, operation: u64) -> TowerRequest {
    let mut request = fixture.request(Verb::Next);
    request.arguments = ReadArguments::AndonWindowNext { operation };
    request
}

fn registration(fixture: &Fixture, operation: u64) -> TowerRequest {
    let host = fixture.driver.supervisor().host().unwrap();
    let state = host.inspect();
    let report = host.credibility_report().unwrap();
    let mut request = fixture.request(Verb::RegisterAndonWindow);
    request.operation_id = format!("register-andon-window-{operation}");
    request.idempotency_key = Some(operation.to_string());
    request.arguments = ReadArguments::RegisterAndonWindow {
        operation,
        expected: FileAndonWindowBasis {
            scope: fixture.actor.scope,
            journal_revision: state.revision,
            control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch,
            evaluation_revision: report.revision,
            congress_generation: 1,
            policy_generation: report.policy_generation,
            recorded_at: state.control.ledger.elapsed,
            accounting: host.andon_ledger().unwrap().identity(),
        },
        expected_budget: fixture.settings.andon_window.as_ref().unwrap().budget,
    };
    request
}

#[test]
fn andon_profile_freezes_one_closed_budget_with_independent_finite_work() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let profile = profile_json(Path::new("/operator/andon.sock"), identity, 1, 0);
    for compatible in [profile.clone(), selected(&profile, "null")] {
        assert!(
            Settings::decode(compatible.as_bytes())
                .unwrap()
                .andon_window
                .is_none()
        );
    }
    let only = profile_json(Path::new("/operator/andon.sock"), identity, 0, 0);
    let budget = native::later_window();
    let valid = selection(budget, 2, 1024);
    let settings = Settings::decode(selected(&only, &valid).as_bytes()).unwrap();
    assert_eq!(settings.andon_window.as_ref().unwrap().budget, budget);
    assert!(settings.governance_ledger.is_none());
    assert!(settings.credibility_promotion.is_none());
    assert!(settings.check_generated_operator().is_err());
    assert!(settings.check_learned_operator().is_err());
    let paid = valid.replacen('{', "{\"numerical_replay_events\":1024,", 1);
    let settings = Settings::decode(selected(&only, &paid).as_bytes()).unwrap();
    settings.check_generated_operator().unwrap();
    settings.check_learned_operator().unwrap();
    for budget in [
        AndonBudget {
            false_stops_allowed: 0,
            cost_per_stop: 0,
            ..budget
        },
        AndonBudget {
            start: u64::MAX - 1,
            end: u64::MAX,
            ..budget
        },
    ] {
        let selected = selected(&only, &selection(budget, 1, 128));
        assert_eq!(
            Settings::decode(selected.as_bytes())
                .unwrap()
                .andon_window
                .unwrap()
                .budget,
            budget
        );
    }
    for invalid in [
        "{}".to_owned(),
        "[]".into(),
        "false".into(),
        selection(budget, 0, 1024),
        selection(budget, MAX_ANDON_WINDOW_QUERIES + 1, 1024),
        selection(budget, 2, 0),
        selection(budget, 2, MAX_ANDON_WINDOW_REPLAY_EVENTS + 1),
        selection(
            AndonBudget {
                window: 0,
                ..budget
            },
            2,
            1024,
        ),
        selection(
            AndonBudget {
                end: budget.start,
                ..budget
            },
            2,
            1024,
        ),
        valid.replace(
            "register_original_andon_window",
            "promote_qualified_credibility",
        ),
        valid.replace("\"queries\":2", "\"queries\":\"2\""),
        valid.replacen('{', "{\"operations\":2,", 1),
        valid.replacen('{', "{\"queries\":2,", 1),
        valid.replacen('{', "{\"numerical_replay_events\":0,", 1),
        valid.replacen('{', "{\"numerical_replay_events\":1025,", 1),
        valid.replace("\"window\":2", "\"window\":2,\"replacement\":true"),
        valid.replace("\"window\":2", "\"window\":2,\"window\":2"),
    ] {
        assert!(
            Settings::decode(selected(&profile, &invalid).as_bytes()).is_err(),
            "accepted {invalid}"
        );
    }
    assert!(Settings::decode(selected(&only, "null").as_bytes()).is_err());
}

#[test]
fn live_registration_recovers_late_original_labels_without_reopening_or_new_authority() {
    let (mut fixture, _storage, evaluator) = fixture(native::later_window());
    let mut service = fixture.start();
    let original = fixture.files();
    let stale = registration(&fixture, 901);
    let planned = fixture.query(&mut service, next(&fixture, 901));
    known(&planned, "andon-window-next");
    assert_eq!(
        field(
            &planned,
            &["result", "value", "knowledge", "value", "can_register"]
        )
        .as_bool(),
        Some(true)
    );
    assert!(
        field(
            &planned,
            &["result", "value", "knowledge", "basis", "recorded_at"]
        )
        .is_null()
    );
    assert_eq!(fixture.files(), original);
    let ticket;
    let assessment = Assessment {
        origin: 3,
        evidence_id: [2; 32],
        truth: GroundTruth::Benign,
    };
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        native::review(&mut host, 3, Verdict::Hold, Verdict::Allow);
        ticket = host.evaluation_ticket(103).unwrap();
        let before = host.inspect();
        let revision = host.revision();
        assert!(
            evaluator
                .assess(&mut host, revision, &ticket, assessment.clone())
                .is_err()
        );
        assert_eq!(host.inspect(), before);
        assert!(host.evaluation_history(103).unwrap().is_empty());
        assert!(!host.andon_ledger().unwrap().windows().contains_key(&2));
    }
    let original = fixture.files();
    let rejected = fixture.query(&mut service, stale);
    assert_eq!(
        field(&rejected, &["envelope", "reason_code"]).as_str(),
        Some("StaleGeneration")
    );
    assert_eq!(fixture.files(), original);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .andon_window
            .as_ref()
            .unwrap()
            .remaining_operations(),
        1
    );
    let planned = fixture.query(&mut service, next(&fixture, 901));
    known(&planned, "andon-window-next");
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    let request = registration(&fixture, 901);
    let mut substituted = request.clone();
    if let ReadArguments::RegisterAndonWindow {
        expected_budget, ..
    } = &mut substituted.arguments
    {
        expected_budget.cost_per_stop = 7;
    } else {
        unreachable!();
    }
    let refused = fixture.query(&mut service, substituted);
    assert_eq!(
        field(&refused, &["envelope", "reason_code"]).as_str(),
        Some("IntegrityFailure"),
    );
    assert_eq!(fixture.files(), original);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .andon_window
            .as_ref()
            .unwrap()
            .remaining_operations(),
        1
    );
    let registered = fixture.query(&mut service, request.clone());
    known(&registered, "andon-window-registration");
    assert_eq!(
        field(
            &registered,
            &["result", "value", "knowledge", "value", "registered"]
        )
        .as_bool(),
        Some(true)
    );
    assert_eq!(
        field(&registered, &["result", "value", "effect_authority"]).as_bool(),
        Some(false)
    );
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let after = host.inspect();
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(after.control, before.control);
        assert_eq!(after.executions, before.executions);
        assert_eq!(
            host.andon_ledger().unwrap().windows()[&2].budget,
            native::later_window()
        );
        assert_eq!(host.andon_ledger().unwrap().windows()[&2].false_stops, 0);
        let revision = host.revision();
        assert!(
            evaluator
                .assess(&mut host, revision, &ticket, assessment.clone())
                .unwrap()
        );
        let window = &host.andon_ledger().unwrap().windows()[&2];
        assert_eq!(window.false_stops, 1);
        assert_eq!(window.total_cost, u128::from(u64::MAX));
        assert_eq!(window.incident.as_ref().unwrap().opened_by_round, 103);
        let revision = host.revision();
        assert!(
            !evaluator
                .assess(&mut host, revision, &ticket, assessment)
                .unwrap()
        );
        assert_eq!(host.revision(), revision);
    }
    let committed = fixture.files();
    let repeated = fixture.query(&mut service, request);
    assert_eq!(
        field(&repeated, &["result"]),
        field(&registered, &["result"])
    );
    assert_eq!(fixture.files(), committed);
    let refused = fixture.query(&mut service, registration(&fixture, 902));
    assert_eq!(
        field(&refused, &["envelope", "status"]).as_str(),
        Some("error")
    );
    assert_eq!(fixture.files(), committed);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .andon_window
            .as_ref()
            .unwrap()
            .remaining_operations(),
        0
    );
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        native::review(&mut host, 4, Verdict::Hold, Verdict::Allow);
        native::assess(&mut host, &evaluator, 4, 4, GroundTruth::Benign);
        let window = &host.andon_ledger().unwrap().windows()[&2];
        assert_eq!(window.false_stops, 2);
        assert_eq!(window.total_cost, 2 * u128::from(u64::MAX));
        assert_eq!(window.incident.as_ref().unwrap().opened_by_round, 103);
    }
}

#[test]
fn conflicting_and_overlapping_selections_remain_blocked_without_spending_registration() {
    for (budget, reason) in [
        (
            AndonBudget {
                cost_per_stop: 8,
                ..native::initial_window()
            },
            "window_conflict",
        ),
        (
            AndonBudget {
                start: 5,
                ..native::later_window()
            },
            "window_overlap",
        ),
    ] {
        let (fixture, _storage, _evaluator) = fixture(budget);
        let mut service = fixture.start();
        let original = fixture.files();
        let plan = fixture.query(&mut service, next(&fixture, 903));
        known(&plan, "andon-window-next");
        assert_eq!(
            field(
                &plan,
                &["result", "value", "knowledge", "value", "can_register"]
            )
            .as_bool(),
            Some(false)
        );
        assert_eq!(
            field(
                &plan,
                &["result", "value", "knowledge", "value", "block_reason"]
            )
            .as_str(),
            Some(reason)
        );
        assert!(field(&plan, &["result", "value", "knowledge", "value", "action"]).is_null());
        let refusal = fixture.query(&mut service, registration(&fixture, 903));
        assert_eq!(
            field(&refusal, &["envelope", "status"]).as_str(),
            Some("error")
        );
        assert_eq!(fixture.files(), original);
        assert_eq!(
            service
                .active
                .as_ref()
                .unwrap()
                .andon_window
                .as_ref()
                .unwrap()
                .remaining_operations(),
            1
        );
    }
}

#[test]
fn registering_the_exact_existing_window_is_a_bounded_acknowledged_noop() {
    let (fixture, _storage, _evaluator) = fixture(native::initial_window());
    let mut service = fixture.start();
    let original = fixture.files();
    let plan = fixture.query(&mut service, next(&fixture, 904));
    known(&plan, "andon-window-next");
    assert_eq!(
        field(
            &plan,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "already_registered"
            ]
        )
        .as_bool(),
        Some(true)
    );
    let request = registration(&fixture, 904);
    let receipt = fixture.query(&mut service, request.clone());
    known(&receipt, "andon-window-registration");
    assert_eq!(
        field(
            &receipt,
            &["result", "value", "knowledge", "value", "registered"]
        )
        .as_bool(),
        Some(false)
    );
    assert_eq!(fixture.files(), original);
    let repeated = fixture.query(&mut service, request);
    assert_eq!(field(&repeated, &["result"]), field(&receipt, &["result"]));
    assert_eq!(fixture.files(), original);
    assert_eq!(
        service
            .active
            .as_ref()
            .unwrap()
            .andon_window
            .as_ref()
            .unwrap()
            .remaining_operations(),
        0
    );
}

#[test]
fn missing_grant_or_original_accounting_never_creates_a_window_capability() {
    let fixture = Fixture::new(1, 0);
    let mut service = fixture.start();
    let original = fixture.files();
    let reply = fixture.query(&mut service, next(&fixture, 905));
    assert_eq!(
        field(&reply, &["envelope", "status"]).as_str(),
        Some("error")
    );
    assert!(service.active.as_ref().unwrap().andon_window.is_none());
    assert_eq!(fixture.files(), original);
    drop(service);

    let mut fixture = Fixture::new(1, 0);
    fixture.settings = Settings::decode(
        selected(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
            &selection(native::later_window(), 1, MAX_ANDON_WINDOW_REPLAY_EVENTS),
        )
        .as_bytes(),
    )
    .unwrap();
    let original = fixture.files();
    assert!(
        Service::start(
            &fixture.settings,
            &fixture.actor,
            fixture.port.clone(),
            &fixture.driver,
            REGISTRATION,
            &deadline()
        )
        .is_err()
    );
    assert_eq!(fixture.files(), original);
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn response_capacity_is_validated_before_live_owner_entry() {
    let (mut fixture, _storage, _evaluator) = fixture(native::later_window());
    let original = fixture.files();
    let before = fixture.driver.supervisor().host().unwrap().inspect();
    fixture.settings.response_bytes =
        fa_reference::tower::publication::MIN_PUBLICATION_RESPONSE_BYTES - 1;
    assert!(
        Service::start(
            &fixture.settings,
            &fixture.actor,
            fixture.port.clone(),
            &fixture.driver,
            REGISTRATION,
            &deadline(),
        )
        .is_err()
    );
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        before
    );
    assert_eq!(fixture.files(), original);
    assert!(!fixture.settings.socket.exists());
    fixture.settings.response_bytes = PublicationLimits::default().max_response_bytes;
    let mut service = fixture.start();
    let request = registration(&fixture, 906);
    let registered = fixture.query(&mut service, request);
    known(&registered, "andon-window-registration");
    assert_eq!(
        field(
            &registered,
            &["result", "value", "knowledge", "value", "registered"]
        )
        .as_bool(),
        Some(true),
    );
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().revision(),
        before.revision + 1
    );
}
