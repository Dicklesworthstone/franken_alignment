//! Actual native evaluation accounting through the authenticated operator
//! command. Counter consistency never certifies label truth or effect authority.
#[path = "governance_ledger/findings.rs"]
mod findings;

use super::super::super::governance_ledger as validation;
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorGovernanceLedgerGrant;
use fa_reference::action::consequence::oversight::credibility::{Fraction, GroundTruth};
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};
use std::collections::BTreeMap;

#[path = "../../../../../../../../tests/support/file_credibility.rs"]
mod evaluated;

#[derive(Clone, Copy)]
enum Fixture {
    Absent,
    Empty,
    Pending,
    Censored,
    Labelled,
    Qualified,
}

struct NativeLedger {
    tower: PublicationTower,
    supervisor: native::Supervisor,
    grant: Option<FileSupervisorGovernanceLedgerGrant>,
    storage: evaluated::Directory,
}

impl NativeLedger {
    fn new(queries: usize, fixture: Fixture) -> Self {
        let storage = evaluated::Directory::new();
        let (mut host, _reviewer) =
            FileOversight::create(storage.store(), evaluated::profile()).unwrap();
        let evaluator = if !matches!(fixture, Fixture::Absent) {
            let mut protocol = evaluated::protocol();
            protocol.false_stop_budget = 2;
            if matches!(fixture, Fixture::Qualified) {
                // Actual two-origin success exercises products beyond u64 and
                // equality at both positive floors. No float approximation.
                protocol.precision_floor = Fraction {
                    numerator: u64::MAX,
                    denominator: u64::MAX,
                };
                protocol.recall_floor = protocol.precision_floor;
                protocol.false_positive_ceiling = Fraction {
                    numerator: 0,
                    denominator: u64::MAX,
                };
            }
            let evaluator = host.enable_credibility(host.revision(), protocol).unwrap();
            let andon = FileAndonConfig::new(
                LedgerIdentity {
                    deployment: 101,
                    evaluator: 202,
                },
                vec![AndonBudget {
                    window: 1,
                    start: 0,
                    end: 100,
                    false_stops_allowed: 2,
                    cost_per_stop: u64::MAX,
                }],
            )
            .unwrap();
            host.enable_andon(host.revision(), &andon).unwrap();
            Some(evaluator)
        } else {
            None
        };
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        if matches!(fixture, Fixture::Labelled) {
            for id in [1, 2] {
                evaluated::review(&mut host, id, Verdict::Hold);
                assert!(evaluated::label(
                    &mut host,
                    evaluator.as_ref().unwrap(),
                    id,
                    id,
                    GroundTruth::Benign,
                ));
            }
        } else if matches!(fixture, Fixture::Qualified) {
            for (id, verdict, truth) in [
                (1, Verdict::Hold, GroundTruth::Violation),
                (2, Verdict::Hold, GroundTruth::Violation),
                (3, Verdict::Allow, GroundTruth::Benign),
            ] {
                evaluated::review(&mut host, id, verdict);
                assert!(evaluated::label(
                    &mut host,
                    evaluator.as_ref().unwrap(),
                    id,
                    id,
                    truth,
                ));
            }
            assert!(host.credibility_report().unwrap().qualified());
        } else if matches!(fixture, Fixture::Pending | Fixture::Censored) {
            evaluated::review(&mut host, 1, Verdict::Hold);
            if matches!(fixture, Fixture::Censored) {
                assert!(evaluated::label(
                    &mut host,
                    evaluator.as_ref().unwrap(),
                    1,
                    1,
                    GroundTruth::Censored,
                ));
            }
        }
        let (actor, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.governance_ledger_port(queries).unwrap();
        let mut tower =
            PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
        tower.set_governance_ledger(Some(port)).unwrap();
        Self {
            tower,
            supervisor,
            grant: Some(grant),
            storage,
        }
    }

    fn response(&mut self, request: &TowerRequest, operator: bool) -> String {
        let before = self.supervisor.host().unwrap().inspect();
        let bytes = self.image();
        let reply = self.tower.exchange(
            &access(operator),
            encode_request(request).unwrap().as_bytes(),
        );
        assert_eq!(self.supervisor.host().unwrap().inspect(), before);
        assert_eq!(self.image(), bytes);
        reply.to_json().unwrap()
    }

    fn remaining(&self) -> usize {
        self.grant.as_ref().unwrap().remaining_queries()
    }

    fn image(&self) -> Vec<u8> {
        fs::read(self.storage.store().join("delivery.bin")).unwrap()
    }
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        Vec::new(),
        0,
    )
}

fn request() -> TowerRequest {
    let mut request = TowerRequest::new("original-governance-ledger", mount(), Verb::Ledger);
    request.idempotency_key = Some("observation-label".into());
    request
}

fn settings(root: &Directory, queries: usize, exchanges: u64) -> PathBuf {
    let path = root.settings(credentials().pid(), exchanges);
    let original = fs::read_to_string(&path).unwrap();
    let selected = original
        .replace("\"inspection_queries\":3", "\"inspection_queries\":0")
        .replace("\"explanation_queries\":2", "\"explanation_queries\":0")
        .replace("\"explanation_replay_events\":1024", "\"explanation_replay_events\":0")
        .replace("policy_and_selected_witnesses", "none")
        .replacen(
            '{',
            &format!("{{\"governance_ledger\":{{\"queries\":{queries},\"disclosure\":\"recorded_governance_accounting\"}},"),
            1,
        );
    fs::write(&path, selected).unwrap();
    path
}

fn authenticate(socket: &UnixStream) {
    let peer = credentials();
    PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid()))
        .unwrap()
        .verify(socket)
        .unwrap();
}

fn accepted(bytes: &str, query: &TowerRequest) -> Json {
    let parsed = parse_response(bytes.as_bytes(), query).unwrap();
    validation::check_profile(&parsed, query, native::profile().delivery.scope).unwrap();
    parsed
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).unwrap())
}

fn body(value: &Json) -> &Json {
    field(value, &["result", "value"])
}

fn ledger(value: &Json) -> &Json {
    field(body(value), &["knowledge", "value"])
}

fn set(value: &mut Json, path: &[&str], replacement: Json) {
    let (last, parents) = path.split_last().unwrap();
    let mut value = value;
    for key in parents {
        let Json::Object(fields) = value else {
            panic!("original object");
        };
        value = fields.get_mut(*key).unwrap();
    }
    let Json::Object(fields) = value else {
        panic!("original object");
    };
    assert!(fields.insert((*last).to_owned(), replacement).is_some());
}

#[test]
fn authenticated_labelled_accounting_preserves_u128_cost_and_spends_only_explicit_reads() {
    let root = Directory::new();
    let profile = settings(&root, 2, 4);
    let query = request();
    let mut exhausted = query.clone();
    exhausted.operation_id = "explicit-exhausted-query".into();
    // Repeating the same observational idempotency label does not reuse a
    // captured result or replenish the original finite grant.
    let expected = [query.clone(), query, exhausted];
    let documents: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, query)| root.document(&format!("ledger-{index}.json"), query))
        .collect();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut native = NativeLedger::new(2, Fixture::Labelled);
        let before = native.supervisor.host().unwrap().inspect();
        let bytes = native.image();
        let mut socket = bounded_socket(&listener);
        authenticate(&socket);
        let mut replies = Vec::new();
        for (index, query) in expected.iter().enumerate() {
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), *query);
            let reply = native.response(query, true);
            let parsed = accepted(&reply, query);
            if index < 2 {
                let accounting = ledger(&parsed);
                let credibility = field(accounting, &["credibility", "value"]);
                assert_eq!(field(credibility, &["scoped_cases"]).as_str(), Some("2"));
                assert_eq!(field(credibility, &["benign_origins"]).as_str(), Some("2"));
                assert_eq!(
                    field(credibility, &["violation_origins"]).as_str(),
                    Some("0")
                );
                assert_eq!(
                    field(credibility, &["calibration_incident_open"]).as_bool(),
                    Some(true)
                );
                assert_eq!(
                    field(credibility, &["precision_floor", "numerator"]).as_str(),
                    Some("1")
                );
                assert_eq!(
                    field(credibility, &["precision_floor", "denominator"]).as_str(),
                    Some("2")
                );
                let members = field(credibility, &["members"]).as_array().unwrap();
                assert_eq!(members.len(), 2);
                for member in members {
                    assert_eq!(field(member, &["false_positives"]).as_str(), Some("2"));
                }
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
                assert_eq!(
                    field(body(&parsed), &["effect_authority"]).as_bool(),
                    Some(false)
                );
                assert_eq!(
                    field(body(&parsed), &["label_truth"]).as_str(),
                    Some("not_verified")
                );
                assert_eq!(
                    field(body(&parsed), &["population_calibration"]).as_str(),
                    Some("not_established")
                );
            } else {
                assert_eq!(
                    field(body(&parsed), &["error", "code"]).as_str(),
                    Some("capacity")
                );
                assert_eq!(
                    field(&parsed, &["envelope", "reason_code"]).as_str(),
                    Some("BudgetExhausted")
                );
            }
            assert_eq!(native.remaining(), usize::from(index == 0));
            socket.write_all(format!("{reply}\n").as_bytes()).unwrap();
            replies.push(reply);
        }
        assert_eq!(
            socket.read(&mut [0_u8; 1]).unwrap(),
            0,
            "unrequested follow-up"
        );
        assert_eq!(native.image(), bytes);
        assert_eq!(native.supervisor.host().unwrap().inspect(), before);
        assert_eq!(before.executions, 0);
        assert_eq!(before.control.ledger.charged, 0);
        replies
    });
    let mut output = Vec::new();
    operator_command(&args(&profile, &documents), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn authenticated_absence_is_distinct_from_installed_empty_accounting() {
    for fixture in [Fixture::Absent, Fixture::Empty] {
        let root = Directory::new();
        let profile = settings(&root, 1, 2);
        let query = request();
        let document = root.document("ledger.json", &query);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut native = NativeLedger::new(1, fixture);
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
            let reply = native.response(&query, true);
            let parsed = accepted(&reply, &query);
            let accounting = ledger(&parsed);
            if matches!(fixture, Fixture::Absent) {
                for (name, domain) in [
                    ("credibility", "original_owner_credibility_ledger"),
                    ("andon", "original_owner_andon_ledger"),
                ] {
                    assert_eq!(field(accounting, &[name, "state"]).as_str(), Some("absent"));
                    assert_eq!(
                        field(accounting, &[name, "closed_domain"]).as_str(),
                        Some(domain)
                    );
                }
            } else {
                assert_eq!(
                    field(accounting, &["credibility", "state"]).as_str(),
                    Some("known")
                );
                assert_eq!(
                    field(accounting, &["credibility", "value", "retained_cases"]).as_str(),
                    Some("0")
                );
                assert_eq!(
                    field(accounting, &["andon", "state"]).as_str(),
                    Some("known")
                );
            }
            socket.write_all(format!("{reply}\n").as_bytes()).unwrap();
            assert_eq!(socket.read(&mut [0_u8; 1]).unwrap(), 0);
            assert_eq!(native.remaining(), 0);
            reply
        });
        let mut output = Vec::new();
        operator_command(&args(&profile, &[document]), &mut output).unwrap();
        assert_eq!(output, format!("{}\n", server.join().unwrap()).as_bytes());
    }
}

#[test]
fn original_qualification_failures_cannot_be_erased_inserted_or_reordered() {
    for fixture in [
        Fixture::Empty,
        Fixture::Pending,
        Fixture::Censored,
        Fixture::Labelled,
        Fixture::Qualified,
    ] {
        let mut native = NativeLedger::new(1, fixture);
        let query = request();
        let original = native.response(&query, true);
        let parsed = accepted(&original, &query);
        let credibility = field(ledger(&parsed), &["credibility", "value"]);
        let failures = field(credibility, &["qualification_failures"])
            .as_array()
            .unwrap();
        assert_eq!(failures.is_empty(), matches!(fixture, Fixture::Qualified));
        if matches!(fixture, Fixture::Pending | Fixture::Censored) {
            assert_eq!(
                field(&failures[0], &["reason"]).as_str(),
                Some(if matches!(fixture, Fixture::Pending) {
                    "pending_labels"
                } else {
                    "censored_labels"
                })
            );
        }
        // All nonqualified fixtures have zero violation observations. Their
        // exact native report therefore keeps the empty-denominator recall
        // failure, including when all observed benign cases were stopped.
        if !matches!(fixture, Fixture::Qualified) {
            assert!(failures.iter().any(|failure| {
                field(failure, &["reason"]).as_str() == Some("recall")
                    && field(failure, &["member"]).as_str() == Some("alpha")
            }));
        }
        let mut invented = failures.to_vec();
        invented.push(Json::Object(BTreeMap::from([
            (
                "reason".into(),
                Json::String(
                    if matches!(fixture, Fixture::Labelled) {
                        "insufficient_benign_origins"
                    } else {
                        "false_stop_budget_exhausted"
                    }
                    .into(),
                ),
            ),
            ("member".into(), Json::Null),
        ])));
        let mut mutations = vec![invented];
        if !failures.is_empty() {
            let mut missing = failures.to_vec();
            missing.remove(0);
            mutations.push(missing);
            mutations.push(Vec::new());
            let mut reordered = failures.to_vec();
            assert!(reordered.len() >= 2 && reordered[0] != reordered[1]);
            reordered.swap(0, 1);
            mutations.push(reordered);
        }
        for failures in mutations {
            let mut changed = parsed.clone();
            set(
                &mut changed,
                &[
                    "result",
                    "value",
                    "knowledge",
                    "value",
                    "credibility",
                    "value",
                    "qualification_failures",
                ],
                Json::Array(failures),
            );
            assert!(!validation::matches(
                body(&changed),
                &query,
                field(&changed, &["envelope"]),
            ));
        }
        // Local validation neither re-reads accounting nor verifies label
        // truth. Every original response remains the same native observation.
        accepted(&original, &query);
        assert_eq!(native.remaining(), 0);
    }
}

#[test]
fn canonical_schema_thresholds_and_counts_cannot_be_changed_into_authority_or_better_results() {
    let mut native = NativeLedger::new(1, Fixture::Labelled);
    let query = request();
    let original = native.response(&query, true);
    accepted(&original, &query);
    for (from, to) in [
        ("fa.governance-ledger/1", "fa.governance-ledger/2"),
        ("\"kind\":\"governance-ledger\"", "\"kind\":\"refused\""),
        ("\"operation\":\"ledger\"", "\"operation\":\"verify\""),
        (
            "recorded_governance_accounting",
            "global_governance_authority",
        ),
        (
            "\"current_eligibility\":\"not_observed\"",
            "\"current_eligibility\":\"known\"",
        ),
        (
            "\"label_truth\":\"not_verified\"",
            "\"label_truth\":\"verified\"",
        ),
        (
            "\"population_calibration\":\"not_established\"",
            "\"population_calibration\":\"established\"",
        ),
        ("\"effect_authority\":false", "\"effect_authority\":true"),
        ("\"status\":\"ok\"", "\"status\":\"error\""),
        (
            "\"reason_code\":null",
            "\"reason_code\":\"BudgetExhausted\"",
        ),
        ("\"retry_safe\":true", "\"retry_safe\":false"),
        (
            "\"registration\":\"fa://1/run/3@11\"",
            "\"registration\":\"fa://1/run/3@12\"",
        ),
        (
            "\"registration\":\"fa://1/run/3@11\"",
            "\"registration\":\"fa://1/run/3@011\"",
        ),
        (
            "\"query_address\":\"fa://1/run/3@11\"",
            "\"query_address\":\"fa://1/run/9@11\"",
        ),
        (
            "\"origin\":\"acknowledged_owner\"",
            "\"origin\":\"verified_global_state\"",
        ),
        (
            "\"minimum_violation_origins\":\"1\"",
            "\"minimum_violation_origins\":\"0\"",
        ),
        (
            "\"minimum_violation_origins\":\"1\"",
            "\"minimum_violation_origins\":\"513\"",
        ),
        (
            "\"minimum_benign_origins\":\"1\"",
            "\"minimum_benign_origins\":1",
        ),
        (
            "\"precision_floor\":{\"numerator\":\"1\",\"denominator\":\"2\"}",
            "\"precision_floor\":{\"numerator\":\"0\",\"denominator\":\"2\"}",
        ),
        (
            "\"recall_floor\":{\"numerator\":\"1\",\"denominator\":\"2\"}",
            "\"recall_floor\":{\"numerator\":\"1\",\"denominator\":\"0\"}",
        ),
        (
            "\"false_positive_ceiling\":{\"numerator\":\"1\",\"denominator\":\"2\"}",
            "\"false_positive_ceiling\":{\"numerator\":\"3\",\"denominator\":\"2\"}",
        ),
        ("\"retained_cases\":\"2\"", "\"retained_cases\":\"1\""),
        ("\"retained_cases\":\"2\"", "\"retained_cases\":\"02\""),
        (
            "\"retained_cases\":\"2\"",
            "\"retained_cases\":\"18446744073709551616\"",
        ),
        (
            "\"retained_cases\":\"2\"",
            "\"retained_cases\":\"2\",\"qualified\":true",
        ),
        ("\"false_positives\":\"2\"", "\"false_positives\":\"1\""),
        (
            "\"false_positives\":\"2\"",
            "\"false_positives\":\"2\",\"false_positives\":\"0\"",
        ),
        (
            "\"lifetime_false_stops\":\"2\"",
            "\"lifetime_false_stops\":\"1\"",
        ),
        ("\"false_stop_budget\":\"2\"", "\"false_stop_budget\":\"0\""),
        (
            "\"calibration_incident_open\":true",
            "\"calibration_incident_open\":false",
        ),
        ("\"false_stops\":\"2\"", "\"false_stops\":\"1\""),
        (
            "\"total_cost\":\"36893488147419103230\"",
            "\"total_cost\":\"36893488147419103229\"",
        ),
        (
            "\"total_cost\":\"36893488147419103230\"",
            "\"total_cost\":36893488147419103230",
        ),
        (
            "\"total_cost\":\"36893488147419103230\"",
            "\"total_cost\":\"036893488147419103230\"",
        ),
        (
            "\"total_cost\":\"36893488147419103230\"",
            "\"total_cost\":\"340282366920938463463374607431768211456\"",
        ),
        (
            "\"contributing_rounds\":\"2\"",
            "\"contributing_rounds\":\"1\"",
        ),
    ] {
        let damaged = original.replacen(from, to, 1);
        assert_ne!(damaged, original, "mutation missed {from}");
        assert!(
            validate_response(damaged.as_bytes(), &query).is_err(),
            "accepted {to}"
        );
    }
    assert_eq!(native.remaining(), 0);
}

#[test]
fn complete_scope_replacement_cannot_bypass_the_configured_operator_audience() {
    let mut native = NativeLedger::new(2, Fixture::Labelled);
    let query = request();
    let original = native.response(&query, true);
    let parsed = accepted(&original, &query);
    for (name, number) in [
        ("tenant", 1),
        ("principal", 2),
        ("run", 3),
        ("branch", 4),
        ("authority", 5),
    ] {
        let zero = original.replace(
            &format!("\"{name}\":\"{number}\""),
            &format!("\"{name}\":\"0\""),
        );
        assert_ne!(zero, original);
        assert!(validate_response(zero.as_bytes(), &query).is_err());
        // Replace one basis only: outer and subsystem observations must retain
        // the same original cut even before comparing the endpoint's audience.
        let partial = original.replacen(
            &format!("\"{name}\":\"{number}\""),
            &format!("\"{name}\":\"{}\"", number + 10),
            1,
        );
        assert_ne!(partial, original);
        assert!(validate_response(partial.as_bytes(), &query).is_err());
    }
    for (name, number) in [("principal", 2), ("branch", 4), ("authority", 5)] {
        // All three basis copies agree, and tenant/run still match the request.
        // Only the independently configured full scope exposes this substitution.
        let foreign = original.replace(
            &format!("\"{name}\":\"{number}\""),
            &format!("\"{name}\":\"{}\"", number + 10),
        );
        assert_ne!(foreign, original);
        let foreign = parse_response(foreign.as_bytes(), &query).unwrap();
        assert!(
            validation::check_profile(&foreign, &query, native::profile().delivery.scope).is_err()
        );
    }
    let mut changed = parsed.clone();
    set(
        &mut changed,
        &[
            "result",
            "value",
            "knowledge",
            "value",
            "andon",
            "basis",
            "control_sequence",
        ],
        Json::String("999".into()),
    );
    assert!(!validation::matches(
        body(&changed),
        &query,
        field(&changed, &["envelope"])
    ));
    let mut changed = parsed;
    set(
        &mut changed,
        &[
            "result",
            "value",
            "knowledge",
            "value",
            "credibility",
            "state",
        ],
        Json::String("absent".into()),
    );
    assert!(!validation::matches(
        body(&changed),
        &query,
        field(&changed, &["envelope"])
    ));
    let mut unversioned = query;
    unversioned.address.generation = None;
    accepted(&native.response(&unversioned, true), &unversioned);
    assert_eq!(native.remaining(), 0);
}

#[test]
fn unknown_revoked_exhausted_and_outer_refused_results_remain_closed_non_authority_states() {
    let query = request();
    let mut native = NativeLedger::new(1, Fixture::Empty);
    let unknown = {
        let _borrow = native.supervisor.host_mut().unwrap();
        native
            .tower
            .exchange(&access(true), encode_request(&query).unwrap().as_bytes())
            .to_json()
            .unwrap()
    };
    let parsed = accepted(&unknown, &query);
    assert_eq!(
        field(body(&parsed), &["knowledge", "state"]).as_str(),
        Some("unknown")
    );
    assert_eq!(native.remaining(), 1);
    let false_known = unknown.replace("\"state\":\"unknown\"", "\"state\":\"known\"");
    assert_ne!(false_known, unknown);
    assert!(validate_response(false_known.as_bytes(), &query).is_err());
    accepted(&native.response(&query, true), &query);
    let exhausted = native.response(&query, true);
    assert_eq!(
        field(body(&accepted(&exhausted, &query)), &["error", "code"]).as_str(),
        Some("capacity")
    );
    assert!(
        validate_response(
            exhausted
                .replace("\"code\":\"capacity\"", "\"code\":\"success\"")
                .as_bytes(),
            &query,
        )
        .is_err()
    );
    native.grant.take().unwrap().revoke();
    let revoked = native.response(&query, true);
    assert_eq!(
        field(body(&accepted(&revoked, &query)), &["knowledge", "state"]).as_str(),
        Some("withheld")
    );
    assert!(
        validate_response(
            revoked
                .replace("supervisor_governance_ledger", "actor_publication")
                .as_bytes(),
            &query,
        )
        .is_err()
    );
    let withheld = native.response(&query, false);
    accepted(&withheld, &query);
    assert!(withheld.contains("\"kind\":\"refused\""));
    for (from, to) in [
        ("\"kind\":\"refused\"", "\"kind\":\"governance-ledger\""),
        ("\"status\":\"error\"", "\"status\":\"ok\""),
        (
            "\"authority_required\":\"Verify\"",
            "\"authority_required\":\"Permit\"",
        ),
        ("\"retry_safe\":false", "\"retry_safe\":true"),
        (
            "\"type\":\"PublicationRefusal\"",
            "\"type\":\"PublicationRefusal\",\"qualified\":true",
        ),
    ] {
        let damaged = withheld.replace(from, to);
        assert_ne!(damaged, withheld);
        assert!(
            validate_response(damaged.as_bytes(), &query).is_err(),
            "accepted {to}"
        );
    }
    native.tower.set_governance_ledger(None).unwrap();
    let absent_grant = native.response(&query, true);
    accepted(&absent_grant, &query);
    assert!(absent_grant.contains("\"reason_code\":\"CapabilityUnsupported\""));
}

#[test]
fn invalid_or_foreign_accounting_emits_nothing_and_blocks_the_next_document() {
    for mutation in [
        "schema",
        "principal",
        "branch",
        "authority",
        "partial_basis",
        "authority_gain",
        "qualification",
        "cost",
        "u128_overflow",
        "truncated_denominator",
        "truncated_member_count",
        "missing_failure",
        "erased_failures",
        "refused",
    ] {
        let root = Directory::new();
        let profile = settings(&root, 2, 4);
        let query = request();
        let mut later = query.clone();
        later.operation_id = "must-not-spend-another-query".into();
        let documents = [
            root.document("ledger.json", &query),
            root.document("later.json", &later),
        ];
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut native = NativeLedger::new(2, Fixture::Labelled);
            let before = native.supervisor.host().unwrap().inspect();
            let bytes = native.image();
            let mut socket = bounded_socket(&listener);
            authenticate(&socket);
            assert_eq!(decode_request(&read_document(&mut socket)).unwrap(), query);
            let original = native.response(&query, true);
            accepted(&original, &query);
            let forged = match mutation {
                "schema" => original.replace("fa.governance-ledger/1", "fa.governance-ledger/2"),
                "principal" => original.replace("\"principal\":\"2\"", "\"principal\":\"22\""),
                "branch" => original.replace("\"branch\":\"4\"", "\"branch\":\"44\""),
                "authority" => original.replace("\"authority\":\"5\"", "\"authority\":\"55\""),
                "partial_basis" => {
                    original.replacen("\"principal\":\"2\"", "\"principal\":\"22\"", 1)
                }
                "authority_gain" => {
                    original.replace("\"effect_authority\":false", "\"effect_authority\":true")
                }
                "qualification" => original.replace(
                    "\"retained_cases\":\"2\"",
                    "\"retained_cases\":\"2\",\"qualified\":true",
                ),
                "cost" => original.replace(
                    "\"total_cost\":\"36893488147419103230\"",
                    "\"total_cost\":\"2\"",
                ),
                "u128_overflow" => original.replace(
                    "\"total_cost\":\"36893488147419103230\"",
                    "\"total_cost\":\"340282366920938463463374607431768211456\"",
                ),
                "truncated_denominator" => {
                    original.replace("\"scoped_cases\":\"2\"", "\"scoped_cases\":\"1\"")
                }
                "truncated_member_count" => {
                    original.replacen("\"false_positives\":\"2\"", "\"false_positives\":\"1\"", 1)
                }
                "missing_failure" => original.replacen(
                    "{\"reason\":\"insufficient_violation_origins\",\"member\":null},",
                    "",
                    1,
                ),
                "erased_failures" => {
                    let prefix = "\"qualification_failures\":[";
                    let start = original.find(prefix).unwrap() + prefix.len();
                    let end = start + original[start..].find(']').unwrap();
                    format!("{}{}", &original[..start], &original[end..])
                }
                _ => original.replace("\"kind\":\"governance-ledger\"", "\"kind\":\"refused\""),
            };
            assert_ne!(forged, original, "mutation missed {mutation}");
            if matches!(mutation, "principal" | "branch" | "authority") {
                let parsed = parse_response(forged.as_bytes(), &query).unwrap();
                assert!(
                    validation::check_profile(&parsed, &query, native::profile().delivery.scope)
                        .is_err()
                );
            } else {
                assert!(
                    validate_response(forged.as_bytes(), &query).is_err(),
                    "{mutation}"
                );
            }
            socket.write_all(format!("{forged}\n").as_bytes()).unwrap();
            // A nonempty read proves EOF. A zero-length read cannot establish
            // whether the client improperly sent the second document.
            assert_eq!(
                socket.read(&mut [0_u8; 1]).unwrap(),
                0,
                "later document followed {mutation}"
            );
            assert_eq!(native.remaining(), 1);
            assert_eq!(native.image(), bytes);
            assert_eq!(native.supervisor.host().unwrap().inspect(), before);
        });
        let mut output = Vec::new();
        assert!(
            operator_command(&args(&profile, &documents), &mut output).is_err(),
            "{mutation}"
        );
        assert!(output.is_empty());
        server.join().unwrap();
    }
}
