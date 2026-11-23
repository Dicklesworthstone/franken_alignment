//! Observe actual native helper evaluation and false-stop accounting through
//! an explicitly retained operator grant. These fixtures exercise original
//! reviews and independent labels, not caller-supplied telemetry.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/file_credibility.rs"]
mod fixture;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileGovernanceLedgerResult, FileGovernanceLedgerSnapshot, FileSupervisorGovernanceLedgerGrant,
};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::consequence::oversight::credibility::{
    GroundTruth, QualificationFailure,
};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::observation_algebra::Knowledge;
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity, MAX_WINDOWS};
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerInstallError,
    TowerRequest, Verb, decode_request, encode_request,
};
use fixture::{Directory, ordinary};
use std::collections::BTreeSet;
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

fn request() -> TowerRequest {
    let mut request = TowerRequest::new("inspect-original-governance", mount(), Verb::Ledger);
    request.idempotency_key = Some("observation-label".to_owned());
    request
}

fn tower(actor: Port) -> PublicationTower {
    PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap()
}

fn install(
    actor: Port,
    supervisor: &Supervisor,
    queries: usize,
) -> (PublicationTower, FileSupervisorGovernanceLedgerGrant) {
    let (port, grant) = supervisor.governance_ledger_port(queries).unwrap();
    let mut facade = tower(actor);
    facade.set_governance_ledger(Some(port)).unwrap();
    (facade, grant)
}

fn read(facade: &mut PublicationTower) -> PublicationResponse {
    let request = request();
    let encoded = encode_request(&request).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), request);
    facade.exchange(&access(true), encoded.as_bytes())
}

fn native(response: &PublicationResponse) -> &FileGovernanceLedgerResult {
    let PublicationResult::GovernanceLedger(linked) = &response.result else {
        panic!("expected original governance ledger observation: {response:?}");
    };
    assert_eq!(linked.registration(), &mount());
    assert_eq!(linked.query_address(), &response.envelope.address);
    assert_eq!(linked.query_operation_id(), response.envelope.operation_id);
    assert_eq!(
        linked.query_idempotency_key(),
        response.envelope.idempotency_key.as_deref()
    );
    no_global_claims(response);
    linked.native()
}

fn known(response: &PublicationResponse) -> &FileGovernanceLedgerSnapshot {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected known original governance accounting: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert_eq!(basis.scope, ordinary::profile().delivery.scope);
    assert_eq!(response.envelope.reason_code, None);
    value
}

fn unavailable(response: &PublicationResponse) {
    assert_eq!(
        native(response),
        &Ok(ActorKnowledge::Unknown {
            reason: ActorUnknown::ControllerUnavailable,
        })
    );
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::LowerLayerUnverified)
    );
    response.to_json().unwrap();
}

fn capacity(response: &PublicationResponse) {
    assert_eq!(native(response), &Err(ActorError::Capacity));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::BudgetExhausted)
    );
    response.to_json().unwrap();
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
    no_global_claims(response);
}

fn no_global_claims(response: &PublicationResponse) {
    fn unobserved<T>(value: &Knowledge<T>) {
        assert!(matches!(
            value,
            Knowledge::Unknown { .. } | Knowledge::Withheld { .. }
        ));
    }
    unobserved(&response.envelope.generation);
    unobserved(&response.envelope.control_seq);
    unobserved(&response.envelope.source_frontiers);
    unobserved(&response.envelope.budgets_remaining);
    unobserved(&response.envelope.realized_cost);
    unobserved(&response.envelope.affordances);
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter().fold(value, |value, name| {
        value
            .get(name)
            .unwrap_or_else(|| panic!("missing field {name}"))
    })
}

fn document(response: &PublicationResponse) -> Json {
    strict_json::parse(
        response.to_json().unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap()
}

fn andon(windows: usize) -> FileAndonConfig {
    FileAndonConfig::new(
        LedgerIdentity {
            deployment: 101,
            evaluator: 202,
        },
        (0..windows)
            .map(|index| AndonBudget {
                window: index as u64 + 1,
                start: index as u64 * 100,
                end: (index as u64 + 1) * 100,
                false_stops_allowed: 2,
                cost_per_stop: u64::MAX,
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn real_reviews_and_independent_labels_drive_exact_helper_and_false_stop_accounting() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create(root.store(), fixture::profile()).unwrap();
    let evaluator = host
        .enable_credibility(host.revision(), fixture::protocol())
        .unwrap();
    host.enable_andon(host.revision(), &andon(1)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (mut facade, grant) = install(actor, &supervisor, 3);
    let initial = read(&mut facade);
    assert_eq!(known(&initial).credibility().unwrap().scoped_cases(), 0);
    assert_eq!(
        known(&initial).andon().unwrap().windows()[&1].false_stops,
        0
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
    let bytes = image(&root);
    let response = read(&mut facade);
    let snapshot = known(&response);
    let credibility = snapshot.credibility().unwrap();
    assert_eq!(
        (credibility.scoped_cases(), credibility.retained_cases()),
        (2, 2)
    );
    assert_eq!(
        (credibility.pending_cases(), credibility.censored_cases()),
        (0, 0)
    );
    assert_eq!(
        (
            credibility.violation_origins(),
            credibility.benign_origins()
        ),
        (0, 2)
    );
    assert_eq!(credibility.lifetime_false_stops(), 2);
    assert!(
        credibility
            .failures()
            .contains(&QualificationFailure::InsufficientViolationOrigins)
    );
    for member in ["alpha", "beta"] {
        let metrics = &credibility.members()[member];
        assert_eq!(metrics.false_positives, 2);
        assert_eq!(
            (
                metrics.true_positives,
                metrics.false_negatives,
                metrics.true_negatives
            ),
            (0, 0, 0)
        );
        assert_eq!(
            (metrics.missing_origins, metrics.abstaining_origins),
            (0, 0)
        );
    }
    let window = &snapshot.andon().unwrap().windows()[&1];
    assert_eq!(window.false_stops, 2);
    assert_eq!(window.total_cost, 2 * u128::from(u64::MAX));
    let incident = window.incident.as_ref().unwrap();
    assert_eq!(incident.opened_by_round, 102);
    assert_eq!(incident.rounds, BTreeSet::from([101, 102]));
    assert_eq!(snapshot.basis().journal_revision, before.revision);
    assert_eq!(snapshot.basis().control_sequence, before.control.sequence);
    assert_eq!(
        snapshot.basis().authority_epoch,
        before.control.ledger.epoch
    );
    assert_eq!(snapshot.basis().recorded_at, before.control.ledger.elapsed);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
    assert_eq!(before.executions, 0);
    assert_eq!(known(&initial).credibility().unwrap().retained_cases(), 0);
    assert!(snapshot.basis().journal_revision > known(&initial).basis().journal_revision);

    let encoded = response.to_json().unwrap();
    let parsed = document(&response);
    assert_eq!(
        field(&parsed, &["envelope"])
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        ENVELOPE_FIELDS.into_iter().collect(),
    );
    assert_eq!(
        field(&parsed, &["result", "kind"]).as_str(),
        Some("governance-ledger")
    );
    let body = field(&parsed, &["result", "value"]);
    assert_eq!(
        field(body, &["schema"]).as_str(),
        Some("fa.governance-ledger/1")
    );
    assert_eq!(field(body, &["operation"]).as_str(), Some("ledger"));
    assert_eq!(
        field(body, &["disclosure"]).as_str(),
        Some("recorded_governance_accounting")
    );
    assert_eq!(
        field(body, &["current_eligibility"]).as_str(),
        Some("not_observed")
    );
    assert_eq!(field(body, &["label_truth"]).as_str(), Some("not_verified"));
    assert_eq!(
        field(body, &["population_calibration"]).as_str(),
        Some("not_established")
    );
    assert_eq!(field(body, &["effect_authority"]).as_bool(), Some(false));
    assert_eq!(
        field(body, &["knowledge", "basis", "origin"]).as_str(),
        Some("acknowledged_owner")
    );
    let values = field(body, &["knowledge", "value"]);
    for minimum in ["minimum_violation_origins", "minimum_benign_origins"] {
        assert_eq!(
            field(values, &["credibility", "value", minimum]).as_str(),
            Some("1")
        );
    }
    for threshold in ["precision_floor", "recall_floor", "false_positive_ceiling"] {
        let fraction = field(values, &["credibility", "value", threshold]);
        assert_eq!(field(fraction, &["numerator"]).as_str(), Some("1"));
        assert_eq!(field(fraction, &["denominator"]).as_str(), Some("2"));
    }
    let wire_members = field(values, &["credibility", "value", "members"])
        .as_array()
        .unwrap();
    assert_eq!(wire_members.len(), 2);
    assert_eq!(
        field(&wire_members[0], &["false_positives"]).as_str(),
        Some("2")
    );
    let wire_windows = field(values, &["andon", "value", "windows"])
        .as_array()
        .unwrap();
    assert_eq!(wire_windows.len(), 1);
    assert_eq!(
        field(&wire_windows[0], &["cost_per_stop"]).as_str(),
        Some("18446744073709551615")
    );
    assert_eq!(
        field(&wire_windows[0], &["total_cost"]).as_str(),
        Some("36893488147419103230")
    );
    assert_eq!(
        field(
            &wire_windows[0],
            &["calibration_incident", "contributing_rounds"]
        )
        .as_str(),
        Some("2")
    );
    for private in [
        "actual frozen helper context",
        "Approve this exact publication?",
        "exact-view-file-v1",
        "\"evidence_id\"",
        "\"ground_truth\"",
        "\"payload\"",
        "\"candidate_policy\"",
    ] {
        assert!(!encoded.contains(private));
    }
    assert_eq!(response.to_json().unwrap(), encoded);
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn complete_absence_preserves_the_original_admission_snapshot_and_two_key_effect() {
    let root = Directory::new();
    let (host, reviewer) = ordinary::create(&root);
    let proposal = support::proposal(&ordinary::spec(&host, b"fresh effect"));
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (mut facade, grant) = install(actor.clone(), &supervisor, 2);
    support::observe(&mut supervisor);
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&root);
    let response = read(&mut facade);
    let snapshot = known(&response);
    assert!(snapshot.credibility().is_none());
    assert!(snapshot.andon().is_none());
    let parsed = document(&response);
    let value = field(&parsed, &["result", "value", "knowledge", "value"]);
    for (section, domain) in [
        ("credibility", "original_owner_credibility_ledger"),
        ("andon", "original_owner_andon_ledger"),
    ] {
        assert_eq!(field(value, &[section, "state"]).as_str(), Some("absent"));
        assert_eq!(
            field(value, &[section, "closed_domain"]).as_str(),
            Some(domain)
        );
    }
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&root), bytes);
    let ticket = actor.submit(701, &proposal).unwrap();
    assert_eq!(
        actor.poll(&ticket),
        ActorKnowledge::Pending { request: 701 }
    );
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    let keys = support::ready(&mut supervisor, &reviewer, 701);
    {
        let mut host = supervisor.host_mut().unwrap();
        support::oversight::dispatch(&mut host, &keys);
        let revision = host.revision();
        assert!(matches!(
            host.publish(revision, keys.automatic.attempt()).unwrap(),
            EndpointOutcome::Executed { .. }
        ));
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    let after = read(&mut facade);
    assert!(known(&after).basis().journal_revision > snapshot.basis().journal_revision);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
    assert_eq!(
        supervisor.host().unwrap().inspect().control.ledger.charged,
        proposal.units
    );
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn operator_scope_registration_and_argument_checks_precede_native_spending() {
    let root = Directory::new();
    let (host, _) = ordinary::create(&root);
    let (actor, supervisor) = host.into_actor_gateway();
    let mut facade = tower(actor);
    refused(&read(&mut facade), ReasonCode::CapabilityUnsupported);
    let (port, grant) = supervisor.governance_ledger_port(2).unwrap();
    facade.set_governance_ledger(Some(port)).unwrap();
    let bytes = image(&root);
    for caller in [
        access(false),
        TowerAccess::new(
            CallerScope::full_tenant_access("other"),
            true,
            Vec::new(),
            0,
        ),
    ] {
        let response = facade.dispatch(&caller, request());
        assert!(matches!(
            response.result,
            PublicationResult::Refused(Knowledge::Withheld { .. })
        ));
    }
    for (address, reason) in [
        (
            Address::new("1", Noun::Run, "3", Some(40)).unwrap(),
            ReasonCode::StaleGeneration,
        ),
        (
            Address::new("1", Noun::Run, "9", Some(41)).unwrap(),
            ReasonCode::MissingRequiredObservation,
        ),
        (
            Address::new("1", Noun::Request, "3", Some(41)).unwrap(),
            ReasonCode::MissingRequiredObservation,
        ),
    ] {
        let mut query = request();
        query.address = address;
        refused(&facade.dispatch(&access(true), query), reason);
    }
    let mut malformed = request();
    malformed.arguments = ReadArguments::OperatorCapabilities;
    refused(
        &facade.dispatch(&access(true), malformed),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(image(&root), bytes);
    // This read needs no mutation purpose, and an unversioned exact run read
    // remains linked to the original versioned registration in its receipt.
    let mut unversioned = request();
    unversioned.address = mount().without_generation();
    known(&facade.dispatch(&access(true), unversioned));
    known(&read(&mut facade));
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn same_scope_and_different_scope_foreign_owners_cannot_replace_the_original_grant() {
    let root = Directory::new();
    let (host, _) = ordinary::create(&root);
    let (actor, supervisor) = host.into_actor_gateway();
    let mut facade = tower(actor);
    let (original, grant) = supervisor.governance_ledger_port(2).unwrap();
    for field in 0..6 {
        let foreign_root = Directory::new();
        let mut profile = ordinary::profile();
        let scope = &mut profile.delivery.scope;
        match field {
            0 => {} // An equal complete scope still identifies a different owner.
            1 => scope.tenant += 1,
            2 => scope.principal += 1,
            3 => scope.run += 1,
            4 => scope.branch += 1,
            _ => scope.authority += 1,
        }
        let (foreign, _) = FileOversight::create(foreign_root.store(), profile).unwrap();
        let (_, foreign) = foreign.into_actor_gateway();
        let (foreign_port, foreign_grant) = foreign.governance_ledger_port(2).unwrap();
        facade
            .set_governance_ledger(Some(original.clone()))
            .unwrap();
        assert_eq!(
            facade.set_governance_ledger(Some(foreign_port)),
            Err(TowerInstallError::SnapshotMismatch)
        );
        refused(&read(&mut facade), ReasonCode::CapabilityUnsupported);
        assert_eq!(grant.remaining_queries(), 2);
        assert_eq!(foreign_grant.remaining_queries(), 2);
    }
    facade.set_governance_ledger(Some(original)).unwrap();
    known(&read(&mut facade));
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn shared_budget_borrow_conflict_owner_loss_and_revocation_remain_distinct() {
    let root = Directory::new();
    let (host, _) = ordinary::create(&root);
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(2).unwrap();
    let mut facade = tower(actor.clone());
    facade.set_governance_ledger(Some(port.clone())).unwrap();
    let mut reconnected = tower(actor);
    reconnected.set_governance_ledger(Some(port)).unwrap();
    {
        let _borrow = supervisor.host_mut().unwrap();
        unavailable(&read(&mut facade));
        assert_eq!(grant.remaining_queries(), 2);
    }
    known(&read(&mut facade));
    assert_eq!(grant.remaining_queries(), 1);
    known(&read(&mut reconnected));
    assert_eq!(grant.remaining_queries(), 0);
    capacity(&read(&mut facade));
    drop(supervisor);
    unavailable(&read(&mut facade));
    let (_reopened, _) = FileOversight::open(root.store(), ordinary::profile()).unwrap();
    unavailable(&read(&mut reconnected));
    grant.revoke();
    for facade in [&mut facade, &mut reconnected] {
        let response = read(facade);
        assert_eq!(
            native(&response),
            &Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_governance_ledger"
            })
        );
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
        response.to_json().unwrap();
    }
}

#[test]
fn the_serializer_rejects_relabelled_queries_for_known_and_exhausted_results() {
    let root = Directory::new();
    let (host, _) = ordinary::create(&root);
    let (actor, supervisor) = host.into_actor_gateway();
    let (mut facade, grant) = install(actor, &supervisor, 1);
    let observed = read(&mut facade);
    known(&observed);
    let exhausted = read(&mut facade);
    capacity(&exhausted);
    for response in [observed, exhausted] {
        for change in 0..6 {
            let mut forged = response.clone();
            match change {
                0 => forged.envelope.address.tenant = "2".to_owned(),
                1 => forged.envelope.address.id = "9".to_owned(),
                2 => forged.envelope.address.generation = Some(42),
                3 => forged.envelope.address.kind = Noun::Request,
                4 => forged.envelope.operation_id = "another-operation".to_owned(),
                _ => forged.envelope.idempotency_key = Some("another-label".to_owned()),
            }
            assert_eq!(forged.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        }
        let mut too_small = response;
        too_small.envelope.limits.max_response_bytes = 64;
        assert_eq!(too_small.to_json(), Err(TowerEncodingError::OutputLimit));
    }
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn complete_output_overflow_is_charged_once_and_larger_output_never_refills_the_grant() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create(root.store(), fixture::profile()).unwrap();
    let _evaluator = host
        .enable_credibility(host.revision(), fixture::protocol())
        .unwrap();
    host.enable_andon(host.revision(), &andon(MAX_WINDOWS))
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(2).unwrap();
    let mut facade = tower(actor.clone());
    facade.set_governance_ledger(Some(port.clone())).unwrap();
    let bytes = image(&root);
    let before = supervisor.host().unwrap().inspect();
    let overflow = read(&mut facade);
    refused(&overflow, ReasonCode::BudgetExhausted);
    assert!(overflow.to_json().unwrap().len() <= PublicationLimits::default().max_response_bytes);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(image(&root), bytes);
    assert_eq!(supervisor.host().unwrap().inspect(), before);

    let mut complete = PublicationTower::new(
        mount(),
        actor,
        PublicationLimits {
            max_response_bytes: 64 * 1024,
        },
    )
    .unwrap();
    complete.set_governance_ledger(Some(port)).unwrap();
    let response = read(&mut complete);
    assert_eq!(
        known(&response).andon().unwrap().windows().len(),
        MAX_WINDOWS
    );
    assert!(response.to_json().unwrap().len() > PublicationLimits::default().max_response_bytes);
    let parsed = document(&response);
    assert_eq!(
        field(
            &parsed,
            &[
                "result",
                "value",
                "knowledge",
                "value",
                "andon",
                "value",
                "windows"
            ]
        )
        .as_array()
        .unwrap()
        .len(),
        MAX_WINDOWS
    );
    assert_eq!(grant.remaining_queries(), 0);
    capacity(&read(&mut complete));
    assert_eq!(image(&root), bytes);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
}
