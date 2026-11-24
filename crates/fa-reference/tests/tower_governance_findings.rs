//! Original incident reads share native Ledger custody and never turn an
//! unimplemented standing query into an all-clear result.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/file_credibility.rs"]
mod fixture;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileGovernanceFindingsSnapshot, FileSupervisorGovernanceLedgerGrant,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as NativeKnowledge,
};
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::governance_assurance::RiskTheaterFamily;
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::{Knowledge, UnknownReason};
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    OperatorCapabilityKind, PublicationLimits, PublicationResponse, PublicationResult,
    PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerRequest, Verb,
};
use fixture::{Directory, ordinary};
use support::{Port, Supervisor};

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(41)).unwrap()
}

fn access(operator: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        Vec::new(),
        0,
    )
}

fn query(verb: Verb) -> TowerRequest {
    TowerRequest::new("original-findings", mount(), verb)
}

fn install(
    actor: Port,
    host: &Supervisor,
    queries: usize,
) -> (PublicationTower, FileSupervisorGovernanceLedgerGrant) {
    let (port, grant) = host.governance_ledger_port(queries).unwrap();
    let mut tower =
        PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default())
            .unwrap();
    tower.set_governance_ledger(Some(port)).unwrap();
    (tower, grant)
}

fn known(response: &PublicationResponse) -> &FileGovernanceFindingsSnapshot {
    let PublicationResult::GovernanceFindings(linked) = &response.result else {
        panic!("expected native findings: {response:?}");
    };
    let Ok(NativeKnowledge::Known { value, basis }) = linked.native() else {
        panic!("expected acknowledged findings: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(linked.registration(), &mount());
    assert_eq!(response.envelope.reason_code, None);
    for family in RiskTheaterFamily::ALL {
        assert!(matches!(
            value.risk_theater()[&family],
            Knowledge::Unknown {
                reason: UnknownReason::LowerLayerUnverified,
            }
        ));
    }
    value
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).unwrap())
}

fn json(response: &PublicationResponse) -> Json {
    strict_json::parse(
        response.to_json().unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap()
}

#[test]
fn independently_labeled_false_holds_expose_original_incident_support_without_mutating_authority() {
    let directory = Directory::new();
    let (mut host, _) = FileOversight::create(directory.store(), fixture::profile()).unwrap();
    let mut protocol = fixture::protocol();
    protocol.false_stop_budget = 2;
    let evaluator = host.enable_credibility(host.revision(), protocol).unwrap();
    host.enable_andon(
        host.revision(),
        &FileAndonConfig::new(
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
        .unwrap(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (mut tower, grant) = install(actor, &supervisor, 4);
    let initial = tower.dispatch(&access(true), query(Verb::Findings));
    assert!(known(&initial).andon().unwrap().incidents().is_empty());
    assert!(
        !known(&initial)
            .credibility()
            .unwrap()
            .calibration_incident_open()
    );

    for id in [1, 2] {
        let mut host = supervisor.host_mut().unwrap();
        fixture::review(&mut host, id, Verdict::Hold);
        assert!(fixture::label(
            &mut host,
            &evaluator,
            id,
            id,
            GroundTruth::Benign
        ));
    }
    let before = supervisor.host().unwrap().inspect();
    let bytes = std::fs::read(directory.store().join("delivery.bin")).unwrap();
    let response = tower.dispatch(&access(true), query(Verb::Findings));
    let snapshot = known(&response);
    assert!(snapshot.credibility().unwrap().calibration_incident_open());
    assert_eq!(snapshot.basis().journal_revision, before.revision);
    let accounting = &snapshot.andon().unwrap().incidents()[&1];
    assert_eq!(accounting.total_cost, 2 * u128::from(u64::MAX));
    assert_eq!(
        accounting.incident.as_ref().unwrap().rounds,
        std::collections::BTreeSet::from([101, 102])
    );
    let document = json(&response);
    let body = field(&document, &["result", "value"]);
    assert_eq!(
        field(body, &["schema"]).as_str(),
        Some("fa.governance-findings/1")
    );
    assert_eq!(field(body, &["effect_authority"]).as_bool(), Some(false));
    let value = field(body, &["knowledge", "value"]);
    let incident = &field(value, &["andon_incidents", "value", "incidents"])
        .as_array()
        .unwrap()[0];
    assert_eq!(field(incident, &["opened_by_round"]).as_str(), Some("102"));
    assert_eq!(
        field(incident, &["support_rounds"]),
        &Json::Array(vec![Json::String("101".into()), Json::String("102".into())])
    );
    assert_eq!(
        field(incident, &["total_cost"]).as_str(),
        Some("36893488147419103230")
    );
    for status in field(value, &["risk_theater"])
        .as_object()
        .unwrap()
        .values()
    {
        assert_eq!(field(status, &["state"]).as_str(), Some("unknown"));
        assert_eq!(
            field(status, &["reason"]).as_str(),
            Some("lower_layer_unverified")
        );
    }
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        std::fs::read(directory.store().join("delivery.bin")).unwrap(),
        bytes
    );
    assert_eq!(before.executions, 0);
    assert_eq!(grant.remaining_queries(), 2);
    assert!(matches!(
        tower.dispatch(&access(true), query(Verb::Ledger)).result,
        PublicationResult::GovernanceLedger(_)
    ));
    known(&tower.dispatch(&access(true), query(Verb::Findings)));
    let exhausted = tower.dispatch(&access(true), query(Verb::Findings));
    let PublicationResult::GovernanceFindings(linked) = &exhausted.result else {
        panic!("native capacity");
    };
    assert_eq!(linked.native(), &Err(ActorError::Capacity));
    assert_eq!(
        exhausted.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    exhausted.to_json().unwrap();
}

#[test]
fn uninstalled_sources_stay_unknown_and_discovery_exposes_the_same_shared_read_grant() {
    let directory = Directory::new();
    let (host, _) = ordinary::create(&directory);
    let (actor, supervisor) = host.into_actor_gateway();
    let (mut tower, grant) = install(actor, &supervisor, 1);
    let mut discover = query(Verb::Capabilities);
    discover.arguments = ReadArguments::OperatorCapabilities;
    let discovery = tower.dispatch(&access(true), discover);
    let PublicationResult::OperatorCapabilities(linked) = &discovery.result else {
        panic!("discovery");
    };
    let entry = linked
        .entries()
        .iter()
        .find(|entry| entry.kind() == OperatorCapabilityKind::GovernanceLedger)
        .unwrap();
    assert_eq!(entry.kind().verbs(), &[Verb::Ledger, Verb::Findings]);
    assert_eq!(grant.remaining_queries(), 1);
    discovery.to_json().unwrap();
    let response = tower.dispatch(&access(true), query(Verb::Findings));
    assert!(known(&response).andon().is_none());
    assert!(known(&response).credibility().is_none());
    let document = json(&response);
    let value = field(&document, &["result", "value", "knowledge", "value"]);
    for name in ["andon_incidents", "credibility_incident"] {
        assert_eq!(field(value, &[name, "state"]).as_str(), Some("unknown"));
        assert_eq!(
            field(value, &[name, "reason"]).as_str(),
            Some("source_not_installed")
        );
    }
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn actor_and_misbound_queries_cannot_read_or_spend_native_findings() {
    let directory = Directory::new();
    let (host, _) = ordinary::create(&directory);
    let (actor, supervisor) = host.into_actor_gateway();
    let (mut tower, grant) = install(actor, &supervisor, 1);
    let actor = tower.dispatch(&access(false), query(Verb::Findings));
    assert!(matches!(
        actor.result,
        PublicationResult::Refused(Knowledge::Withheld { .. })
    ));
    for (address, reason) in [
        (
            Address::new("1", Noun::Run, "3", Some(40)).unwrap(),
            ReasonCode::StaleGeneration,
        ),
        (
            Address::new("1", Noun::Run, "9", Some(41)).unwrap(),
            ReasonCode::MissingRequiredObservation,
        ),
    ] {
        let mut request = query(Verb::Findings);
        request.address = address;
        assert_eq!(
            tower.dispatch(&access(true), request).envelope.reason_code,
            Some(reason)
        );
    }
    let mut malformed = query(Verb::Findings);
    malformed.arguments = ReadArguments::OperatorCapabilities;
    assert_eq!(
        tower
            .dispatch(&access(true), malformed)
            .envelope
            .reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert_eq!(grant.remaining_queries(), 1);
    known(&tower.dispatch(&access(true), query(Verb::Findings)));
    grant.revoke();
    let revoked = tower.dispatch(&access(true), query(Verb::Findings));
    assert_eq!(
        revoked.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    revoked.to_json().unwrap();
}

#[test]
fn findings_receipts_cannot_be_relabelled_or_truncated_into_a_different_claim() {
    let directory = Directory::new();
    let (host, _) = ordinary::create(&directory);
    let (actor, supervisor) = host.into_actor_gateway();
    let (mut tower, grant) = install(actor, &supervisor, 1);
    let response = tower.dispatch(&access(true), query(Verb::Findings));
    known(&response);
    for change in 0..6 {
        let mut forged = response.clone();
        match change {
            0 => forged.envelope.address.tenant = "2".into(),
            1 => forged.envelope.address.id = "9".into(),
            2 => forged.envelope.address.generation = Some(42),
            3 => forged.envelope.address.kind = Noun::Request,
            4 => forged.envelope.operation_id = "different-operation".into(),
            _ => forged.envelope.idempotency_key = Some("different-label".into()),
        }
        assert_eq!(forged.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    }
    for change in 0..8 {
        let mut forged = response.clone();
        match change {
            0 => forged.envelope.generation = Knowledge::unknown(UnknownReason::Incomplete),
            1 => forged.envelope.control_seq = Knowledge::unknown(UnknownReason::Incomplete),
            2 => forged.envelope.source_frontiers = Knowledge::unknown(UnknownReason::Incomplete),
            3 => forged.envelope.budgets_remaining = Knowledge::unknown(UnknownReason::Incomplete),
            4 => forged.envelope.realized_cost = Knowledge::unknown(UnknownReason::Incomplete),
            5 => forged.envelope.affordances = Knowledge::unknown(UnknownReason::Incomplete),
            6 => forged.envelope.reason_code = Some(ReasonCode::BudgetExhausted),
            _ => forged.envelope.retry_safe = false,
        }
        assert_eq!(forged.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    }
    let mut too_small = response;
    too_small.envelope.limits.max_response_bytes = 64;
    assert_eq!(too_small.to_json(), Err(TowerEncodingError::OutputLimit));
    assert_eq!(grant.remaining_queries(), 0);
}
