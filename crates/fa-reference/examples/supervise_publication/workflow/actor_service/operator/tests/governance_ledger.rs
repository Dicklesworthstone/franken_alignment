//! Original accounting observed through real authenticated operator reconnects.
//! These reads do not bootstrap the deliberately missing publication source.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileIndependentEvaluator;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_GOVERNANCE_LEDGER_QUERIES;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};

#[path = "../../../../../../tests/support/file_credibility.rs"]
mod credibility;

fn with_ledger(profile: &str, selected: &str) -> String {
    profile.replacen('{', &format!("{{\"governance_ledger\":{selected},"), 1)
}

fn selection(queries: usize) -> String {
    format!(r#"{{"queries":{queries},"disclosure":"recorded_governance_accounting"}}"#)
}

fn fixture(queries: usize) -> Fixture {
    let mut fixture = Fixture::configured(1, 0, |config| {
        let clock = config.profile.delivery.clock_domain;
        config.profile = credibility::profile();
        config.profile.delivery.clock_domain = clock;
    });
    fixture.settings = Settings::decode(
        with_ledger(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
            &selection(queries),
        )
        .as_bytes(),
    )
    .unwrap();
    fixture.settings.connection_limit = 8;
    fixture
}

fn enable(fixture: &mut Fixture) -> FileIndependentEvaluator {
    let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
    let mut protocol = credibility::protocol();
    protocol.false_stop_budget = 2;
    let revision = host.revision();
    let evaluator = host.enable_credibility(revision, protocol).unwrap();
    let config = FileAndonConfig::new(
        LedgerIdentity {
            deployment: 101,
            evaluator: 202,
        },
        vec![AndonBudget {
            window: 1,
            start: 0,
            end: 10_000,
            false_stops_allowed: 2,
            cost_per_stop: u64::MAX,
        }],
    )
    .unwrap();
    let revision = host.revision();
    host.enable_andon(revision, &config).unwrap();
    evaluator
}

fn remaining(service: &Service) -> usize {
    service
        .active
        .as_ref()
        .unwrap()
        .governance_ledger
        .as_ref()
        .unwrap()
        .remaining_queries()
}

fn read(fixture: &Fixture, service: &mut Service) -> Json {
    let before = fixture.files();
    let original = fixture.driver.supervisor().host().unwrap().inspect();
    let reply = fixture.query(service, fixture.request(Verb::Ledger));
    assert_eq!(fixture.files(), before);
    assert_eq!(
        fixture.driver.supervisor().host().unwrap().inspect(),
        original
    );
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    reply
}

#[test]
fn ledger_profile_requires_explicit_finite_accounting_disclosure() {
    let identity = PeerPolicy::new(1, 2, Some(3)).unwrap();
    let path = Path::new("/operator/ledger.sock");
    let original = profile_json(path, identity, 1, 1);
    for compatible in [original.clone(), with_ledger(&original, "null")] {
        let decoded = Settings::decode(compatible.as_bytes()).unwrap();
        assert!(decoded.governance_ledger.is_none());
        assert_eq!(
            (decoded.inspection_queries, decoded.explanation_queries),
            (1, 1)
        );
    }
    let only = with_ledger(
        &profile_json(path, identity, 0, 0),
        &selection(MAX_GOVERNANCE_LEDGER_QUERIES),
    );
    let decoded = Settings::decode(only.as_bytes()).unwrap();
    decoded.check_learned_operator().unwrap();
    assert_eq!(
        decoded.governance_ledger.unwrap().queries,
        MAX_GOVERNANCE_LEDGER_QUERIES
    );
    for invalid in [
        "{}".to_owned(),
        "[]".into(),
        "false".into(),
        selection(0),
        selection(MAX_GOVERNANCE_LEDGER_QUERIES + 1),
        selection(1).replace("recorded_governance_accounting", "none"),
        selection(1).replace("recorded_governance_accounting", "raw_labels"),
        selection(1).replace("\"queries\":1", "\"queries\":\"1\""),
        selection(1).replacen('{', "{\"queries\":1,", 1),
        selection(1).replacen('{', "{\"effect_authority\":true,", 1),
    ] {
        assert!(
            Settings::decode(with_ledger(&original, &invalid).as_bytes()).is_err(),
            "accepted {invalid}"
        );
    }
    assert!(
        Settings::decode(with_ledger(&profile_json(path, identity, 0, 0), "null").as_bytes())
            .is_err()
    );
}

#[test]
fn real_operator_observes_assessments_and_wide_andon_cost_without_source_or_authority_changes() {
    let mut fixture = fixture(4);
    let evaluator = enable(&mut fixture);
    let mut service = fixture.start();
    let initial = read(&fixture, &mut service);
    known(&initial, "governance-ledger");
    assert_eq!(
        field(
            &initial,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "credibility",
                "value",
                "retained_cases"
            ]
        )
        .as_str(),
        Some("0")
    );
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        credibility::review(&mut host, 1, Verdict::Hold);
    }
    let pending = read(&fixture, &mut service);
    assert_eq!(
        field(
            &pending,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "credibility",
                "value",
                "pending_cases"
            ]
        )
        .as_str(),
        Some("1")
    );
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        assert!(credibility::label(
            &mut host,
            &evaluator,
            1,
            11,
            GroundTruth::Benign
        ));
    }
    let labelled = read(&fixture, &mut service);
    assert_eq!(
        field(
            &labelled,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "credibility",
                "value",
                "lifetime_false_stops"
            ]
        )
        .as_str(),
        Some("1")
    );
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        credibility::review(&mut host, 2, Verdict::Hold);
        assert!(credibility::label(
            &mut host,
            &evaluator,
            2,
            12,
            GroundTruth::Benign
        ));
    }
    // Direct native mutation invalidates the admission snapshot. Install a
    // current one before proving these final reads leave it available.
    fixture.snapshot();
    let charged = read(&fixture, &mut service);
    known(&charged, "governance-ledger");
    let accounting = field(&charged, &["result", "value", "knowledge", "value"]);
    assert_eq!(
        field(accounting, &["credibility", "value", "benign_origins"]).as_str(),
        Some("2")
    );
    assert_eq!(
        field(
            accounting,
            &["credibility", "value", "calibration_incident_open"]
        )
        .as_bool(),
        Some(true)
    );
    let windows = field(accounting, &["andon", "value", "windows"])
        .as_array()
        .unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(
        field(&windows[0], &["total_cost"]).as_str(),
        Some("36893488147419103230")
    );
    assert_eq!(
        field(&windows[0], &["calibration_incident", "opened_by_round"]).as_str(),
        Some("102")
    );
    assert_eq!(remaining(&service), 0);
    let exhausted = read(&fixture, &mut service);
    assert_eq!(
        field(&exhausted, &["envelope", "reason_code"]).as_str(),
        Some("BudgetExhausted")
    );
    assert_eq!(remaining(&service), 0);
    // Observing accounting neither consumes the current one-use admission
    // view nor approves the actor request.
    let ticket = fixture.port.submit(8, &fixture.proposal()).unwrap();
    assert_eq!(
        fixture.port.poll(&ticket),
        Knowledge::Pending { request: 8 }
    );
    assert_eq!(
        fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .executions,
        0
    );
    service.retire();
    assert!(!fixture.settings.socket.exists());
}

#[test]
fn ledger_is_separately_granted_and_absence_is_not_an_empty_installed_ledger() {
    let ordinary = Fixture::new(1, 0);
    let mut service = ordinary.start();
    let denied = read(&ordinary, &mut service);
    assert_eq!(
        field(&denied, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    known(
        &ordinary.query(&mut service, ordinary.request(Verb::Situation)),
        "situation",
    );
    service.retire();

    let fixture = fixture(2);
    let mut service = fixture.start();
    let absent = read(&fixture, &mut service);
    known(&absent, "governance-ledger");
    for subsystem in ["credibility", "andon"] {
        assert_eq!(
            field(
                &absent,
                &["result", "value", "knowledge", "value", subsystem, "state"]
            )
            .as_str(),
            Some("absent")
        );
    }
    let denied = fixture.query(&mut service, fixture.request(Verb::Situation));
    assert_eq!(
        field(&denied, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    assert_eq!(remaining(&service), 1);
    service
        .active
        .as_mut()
        .unwrap()
        .governance_ledger
        .take()
        .unwrap()
        .revoke();
    let withdrawn = read(&fixture, &mut service);
    assert_eq!(
        field(&withdrawn, &["result", "value", "knowledge", "state"]).as_str(),
        Some("withheld")
    );
    assert_eq!(
        field(&withdrawn, &["envelope", "reason_code"]).as_str(),
        Some("DeniedByPolicy")
    );
}
