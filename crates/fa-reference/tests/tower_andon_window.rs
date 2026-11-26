//! Exact original-window registration through a separately granted Tower route.
//! Native reviews and labels exercise accounting; they do not qualify detector quality.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/file_credibility.rs"]
mod fixture;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, credibility::FileIndependentEvaluator, credibility::andon::FileAndonConfig,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileAndonWindowBasis, FileAndonWindowBlockReason, FileAndonWindowError,
    FileSupervisorAndonWindowGrant, MAX_ANDON_WINDOW_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::Knowledge as NativeKnowledge;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};
use fa_reference::tower::publication::{
    OperatorCapabilityKind, PublicationLimits, PublicationResponse, PublicationResult,
    PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerInstallError, TowerRequest,
    Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use fixture::Directory;
use support::{Port, Supervisor};

const OPERATION: u64 = 7001;

fn identity() -> LedgerIdentity {
    LedgerIdentity {
        deployment: 101,
        evaluator: 202,
    }
}
fn initial_window() -> AndonBudget {
    AndonBudget {
        window: 1,
        start: 0,
        end: 100,
        false_stops_allowed: 1,
        cost_per_stop: 7,
    }
}
fn next_window() -> AndonBudget {
    AndonBudget {
        window: 2,
        start: 100,
        end: 200,
        false_stops_allowed: 1,
        cost_per_stop: u64::MAX,
    }
}
fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(41)).unwrap()
}
fn access(operator: bool, permit: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        if permit {
            vec![PurposeTag::Permit]
        } else {
            Vec::new()
        },
        0,
    )
}
fn query() -> TowerRequest {
    let mut request = TowerRequest::new("inspect-andon-window", mount(), Verb::Next);
    request.arguments = ReadArguments::AndonWindowNext {
        operation: OPERATION,
    };
    request
}
fn image(directory: &Directory) -> Vec<u8> {
    std::fs::read(directory.store().join("delivery.bin")).unwrap()
}
fn ready() -> (Directory, Port, Supervisor, FileIndependentEvaluator) {
    let directory = Directory::new();
    let (mut host, _human) = FileOversight::create(directory.store(), fixture::profile()).unwrap();
    let evaluator = host
        .enable_credibility(host.revision(), fixture::protocol())
        .unwrap();
    host.enable_andon(
        host.revision(),
        &FileAndonConfig::new(identity(), vec![initial_window()]).unwrap(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    (directory, actor, supervisor, evaluator)
}
fn install(
    actor: Port,
    supervisor: &Supervisor,
    budget: AndonBudget,
    queries: usize,
) -> (PublicationTower, FileSupervisorAndonWindowGrant) {
    let (port, grant) = supervisor
        .andon_window_port(budget, queries, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let mut tower =
        PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default())
            .unwrap();
    tower.set_andon_window(Some(port)).unwrap();
    (tower, grant)
}
fn action(response: &PublicationResponse) -> TowerRequest {
    let PublicationResult::AndonWindowNext(linked) = &response.result else {
        panic!("original window plan: {response:?}");
    };
    let Ok(NativeKnowledge::Known { value, basis }) = linked.native() else {
        panic!("known original window plan: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    assert!(value.can_register());
    linked.action().unwrap().expect("original window action")
}
fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
}

#[test]
fn strict_requests_bind_the_complete_predecessor_and_independently_selected_budget() {
    let basis = FileAndonWindowBasis {
        scope: fixture::profile().delivery.scope,
        journal_revision: 5,
        control_sequence: 6,
        authority_epoch: 7,
        evaluation_revision: 8,
        congress_generation: 9,
        policy_generation: 10,
        recorded_at: Some(ElapsedTick(11)),
        accounting: identity(),
    };
    let mut register = TowerRequest::new(
        "register-andon-window-7001",
        mount(),
        Verb::RegisterAndonWindow,
    );
    register.arguments = ReadArguments::RegisterAndonWindow {
        operation: OPERATION,
        expected: basis,
        expected_budget: next_window(),
    };
    register.idempotency_key = Some(OPERATION.to_string());
    for request in [query(), register.clone()] {
        let encoded = encode_request(&request).unwrap();
        assert_eq!(decode_request(encoded.as_bytes()).unwrap(), request);
        for invalid in [
            encoded.replace("\"operation\":\"7001\"", "\"operation\":7001"),
            encoded.replace("\"operation\":\"7001\"", "\"operation\":\"07001\""),
            encoded.replace(
                "\"operation\":\"7001\"",
                "\"operation\":\"7001\",\"window\":\"99\"",
            ),
            encoded.replace(
                "\"operation\":\"7001\"",
                "\"operation\":\"7001\",\"false_stops_allowed\":\"99\"",
            ),
            encoded.replace(
                "\"operation\":\"7001\"",
                "\"operation\":\"7001\",\"cost_per_stop\":\"0\"",
            ),
            encoded.replace(
                "\"operation\":\"7001\"",
                "\"operation\":\"7001\",\"operation\":\"7001\"",
            ),
        ] {
            assert_ne!(invalid, encoded);
            assert!(decode_request(invalid.as_bytes()).is_err(), "{invalid}");
        }
    }
    let encoded = encode_request(&register).unwrap();
    for invalid in [
        encoded.replace("\"evaluation_revision\":\"8\",", ""),
        encoded.replace("\"deployment\":\"101\"", "\"deployment\":\"0\""),
        encoded.replace("\"evaluator\":\"202\"", "\"evaluator\":\"0202\""),
        encoded.replace("\"accounting\":{\"deployment\":\"101\",\"evaluator\":\"202\"}", "\"accounting\":{}"),
        encoded.replace("\"purpose\":\"effect\"", "\"purpose\":\"observe\""),
        encoded.replace("\"verb\":\"register-andon-window\"", "\"verb\":\"preregister\""),
        encoded.replace("\"expected_budget\":{\"window\":\"2\",\"start\":\"100\",\"end\":\"200\",\"false_stops_allowed\":\"1\",\"cost_per_stop\":\"18446744073709551615\"}", "\"expected_budget\":{}"),
        encoded.replace(",\"expected_budget\":{\"window\":\"2\",\"start\":\"100\",\"end\":\"200\",\"false_stops_allowed\":\"1\",\"cost_per_stop\":\"18446744073709551615\"}", ""),
        encoded.replace("\"window\":\"2\"", "\"window\":2"),
        encoded.replace("\"start\":\"100\"", "\"start\":\"0100\""),
        encoded.replace("\"end\":\"200\"", "\"end\":\"100\""),
        encoded.replace("\"cost_per_stop\":\"18446744073709551615\"", "\"cost_per_stop\":\"18446744073709551615\",\"cost_per_stop\":\"0\""),
    ] {
        assert_ne!(invalid, encoded);
        assert!(decode_request(invalid.as_bytes()).is_err(), "{invalid}");
    }
    register.idempotency_key = None;
    assert!(encode_request(&register).is_err());
    assert_eq!(
        Verb::RegisterAndonWindow.class(),
        fa_reference::tower::VerbClass::Mutation
    );
}

#[test]
fn scope_role_generation_and_control_mount_checks_precede_native_spending() {
    let (directory, actor, supervisor, _evaluator) = ready();
    let (port, grant) = supervisor
        .andon_window_port(next_window(), 8, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let mut tower = PublicationTower::new_control_operator(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    tower.set_andon_window(Some(port.clone())).unwrap();
    let bytes = image(&directory);
    let state = supervisor.host().unwrap().inspect();

    refused(
        &tower.dispatch(&access(false, true), query()),
        ReasonCode::DeniedByPolicy,
    );
    refused(
        &tower.dispatch(&access(true, false), query()),
        ReasonCode::PurposeMismatch,
    );
    let mut foreign = query();
    foreign.address = Address::new("2", Noun::Run, "3", Some(41)).unwrap();
    refused(
        &tower.dispatch(&access(true, true), foreign),
        ReasonCode::DeniedByPolicy,
    );
    let mut other_run = query();
    other_run.address = Address::new("1", Noun::Run, "4", Some(41)).unwrap();
    refused(
        &tower.dispatch(&access(true, true), other_run),
        ReasonCode::MissingRequiredObservation,
    );
    let mut stale = query();
    stale.address.generation = Some(42);
    refused(
        &tower.dispatch(&access(true, true), stale),
        ReasonCode::StaleGeneration,
    );
    let mut malformed = query();
    malformed.arguments = ReadArguments::AndonWindowNext { operation: 0 };
    refused(
        &tower.dispatch(&access(true, true), malformed),
        ReasonCode::IntegrityFailure,
    );

    for mut limited in [
        PublicationTower::new(mount(), actor.clone(), PublicationLimits::default()).unwrap(),
        PublicationTower::new_reconciliation_operator(mount(), actor, PublicationLimits::default())
            .unwrap(),
    ] {
        limited.set_andon_window(Some(port.clone())).unwrap();
        refused(
            &limited.dispatch(&access(true, true), query()),
            ReasonCode::CapabilityUnsupported,
        );
    }
    assert_eq!(grant.remaining_queries(), 8);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), state);
    assert_eq!(image(&directory), bytes);
    action(&tower.dispatch(&access(true, true), query()));
    assert_eq!(grant.remaining_queries(), 7);
    assert_eq!(grant.remaining_operations(), 1);
}

#[test]
fn a_registered_next_window_accepts_real_later_false_stops_without_forgiving_the_original_incident()
{
    let (directory, actor, mut supervisor, evaluator) = ready();
    {
        let mut host = supervisor.host_mut().unwrap();
        fixture::review(&mut host, 1, Verdict::Hold);
        assert!(fixture::label(
            &mut host,
            &evaluator,
            1,
            1,
            GroundTruth::Benign
        ));
    }
    let before = supervisor.host().unwrap().inspect();
    let original = supervisor.host().unwrap().andon_ledger().unwrap().windows()[&1].clone();
    assert!(original.incident.is_some());
    let (mut tower, grant) = install(actor, &supervisor, next_window(), 4);
    let proposed = tower.dispatch(&access(true, true), query());
    let command = action(&proposed);
    let bytes = image(&directory);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(grant.remaining_queries(), 3);
    let response = tower.exchange(
        &access(true, true),
        encode_request(&command).unwrap().as_bytes(),
    );
    let PublicationResult::AndonWindowRegistration(linked) = &response.result else {
        panic!("native registration: {response:?}");
    };
    let Ok(NativeKnowledge::Known { value, basis }) = linked.native() else {
        panic!("acknowledged original registration: {response:?}");
    };
    assert!(value.registered());
    assert_eq!(value.budget(), next_window());
    assert_eq!(value.after(), *basis);
    assert_eq!(value.before().journal_revision + 1, basis.journal_revision);
    assert_eq!(value.before().control_sequence, basis.control_sequence);
    assert_eq!(value.before().authority_epoch, basis.authority_epoch);
    assert_eq!(
        value.before().evaluation_revision,
        basis.evaluation_revision
    );
    assert_eq!(value.before().accounting, basis.accounting);
    assert_eq!(
        supervisor.host().unwrap().andon_ledger().unwrap().windows()[&1],
        original
    );
    assert_ne!(image(&directory), bytes);
    assert_eq!(grant.remaining_operations(), 0);
    let encoded = response.to_json().unwrap();
    assert!(encoded.len() <= PublicationLimits::default().max_response_bytes);
    assert!(encoded.contains("\"cost_per_stop\":\"18446744073709551615\""));
    assert!(!response.envelope.retry_safe);

    let acknowledged = image(&directory);
    let retry = tower.dispatch(&access(true, true), command.clone());
    assert_eq!(retry.result, response.result);
    assert_eq!(image(&directory), acknowledged);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(110)).unwrap();
        fixture::review(&mut host, 2, Verdict::Hold);
        assert!(fixture::label(
            &mut host,
            &evaluator,
            2,
            2,
            GroundTruth::Benign
        ));
        let ledger = host.andon_ledger().unwrap();
        assert_eq!(ledger.windows()[&1], original);
        assert_eq!(ledger.windows()[&2].false_stops, 1);
        assert_eq!(ledger.windows()[&2].total_cost, u128::from(u64::MAX));
        assert_eq!(
            ledger.windows()[&2]
                .incident
                .as_ref()
                .unwrap()
                .opened_by_round,
            102
        );
    }
    let later = image(&directory);
    assert_eq!(
        tower.dispatch(&access(true, true), command).result,
        response.result
    );
    assert_eq!(image(&directory), later);
}

#[test]
fn exact_existing_window_is_an_acknowledged_no_op_and_stale_plans_do_not_spend_registration() {
    let (directory, actor, mut supervisor, _evaluator) = ready();
    let (mut tower, grant) = install(actor, &supervisor, initial_window(), 4);
    let plan = tower.dispatch(&access(true, true), query());
    let command = action(&plan);
    let PublicationResult::AndonWindowNext(linked) = &plan.result else {
        unreachable!()
    };
    let Ok(NativeKnowledge::Known { value, .. }) = linked.native() else {
        unreachable!()
    };
    assert!(value.already_registered());
    let bytes = image(&directory);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let changed = image(&directory);
    assert_ne!(changed, bytes);
    let stale = tower.dispatch(&access(true, true), command);
    let PublicationResult::AndonWindowRegistration(linked) = stale.result else {
        panic!("native stale predecessor");
    };
    assert_eq!(
        linked.native(),
        &Err(FileAndonWindowError::StalePredecessor)
    );
    assert_eq!(image(&directory), changed);
    assert_eq!(grant.remaining_operations(), 1);

    let command = action(&tower.dispatch(&access(true, true), query()));
    let completed = tower.dispatch(&access(true, true), command);
    let PublicationResult::AndonWindowRegistration(linked) = &completed.result else {
        panic!("native exact-existing acknowledgment");
    };
    let Ok(NativeKnowledge::Known { value, basis }) = linked.native() else {
        panic!("known no-op")
    };
    assert!(!value.registered());
    assert_eq!(value.budget(), initial_window());
    assert_eq!(value.before(), *basis);
    assert_eq!(value.after(), *basis);
    assert_eq!(image(&directory), changed);
    completed.to_json().unwrap();
}

#[test]
fn an_intervening_overlapping_window_blocks_the_fixed_plan_without_an_action() {
    let (directory, actor, mut supervisor, _evaluator) = ready();
    let (mut tower, grant) = install(actor, &supervisor, next_window(), 3);
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.register_andon_window(
            revision,
            AndonBudget {
                window: 3,
                start: 150,
                end: 250,
                false_stops_allowed: 0,
                cost_per_stop: 0,
            },
        )
        .unwrap();
    }
    let bytes = image(&directory);
    let response = tower.dispatch(&access(true, true), query());
    let PublicationResult::AndonWindowNext(linked) = &response.result else {
        panic!("native plan")
    };
    let Ok(NativeKnowledge::Known { value, .. }) = linked.native() else {
        panic!("known block")
    };
    assert_eq!(
        value.block_reason(),
        Some(FileAndonWindowBlockReason::WindowOverlap)
    );
    assert!(!value.can_register());
    assert_eq!(linked.action().unwrap(), None);
    assert_eq!(value.budget(), next_window());
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(image(&directory), bytes);
    response.to_json().unwrap();
}

#[test]
fn foreign_replacement_and_revocation_withdraw_the_route_without_copying_authority() {
    let (directory, actor, supervisor, _evaluator) = ready();
    let (_other_directory, _other_actor, other_supervisor, _other_evaluator) = ready();
    let (port, grant) = supervisor
        .andon_window_port(next_window(), 4, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let (foreign, foreign_grant) = other_supervisor
        .andon_window_port(next_window(), 4, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let mut tower =
        PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default())
            .unwrap();
    tower.set_andon_window(Some(port.clone())).unwrap();
    let bytes = image(&directory);
    assert_eq!(
        tower.set_andon_window(Some(foreign)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    refused(
        &tower.dispatch(&access(true, true), query()),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 4);
    assert_eq!(foreign_grant.remaining_queries(), 4);
    tower.set_andon_window(Some(port)).unwrap();
    let command = action(&tower.dispatch(&access(true, true), query()));
    grant.revoke();
    let response = tower.dispatch(&access(true, true), command);
    let PublicationResult::AndonWindowRegistration(linked) = &response.result else {
        panic!("revoked original grant");
    };
    assert!(matches!(
        linked.native(),
        Ok(NativeKnowledge::Withheld {
            authority_required: "supervisor_andon_window",
        })
    ));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    response.to_json().unwrap();
    assert_eq!(image(&directory), bytes);
}

#[test]
fn receipt_and_plan_envelopes_cannot_relabel_the_original_registration() {
    let (_directory, actor, supervisor, _evaluator) = ready();
    let (mut tower, _grant) = install(actor, &supervisor, next_window(), 3);
    let plan = tower.dispatch(&access(true, true), query());
    let command = action(&plan);
    let response = tower.dispatch(&access(true, true), command);
    response.to_json().unwrap();
    for original in [plan, response] {
        let mut altered = original.clone();
        altered.envelope.operation_id.push_str("-different");
        assert_eq!(altered.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut altered = original.clone();
        altered.envelope.address.generation = Some(42);
        assert_eq!(altered.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut altered = original;
        altered.envelope.idempotency_key = Some("7002".into());
        assert_eq!(altered.to_json(), Err(TowerEncodingError::InvalidKnowledge));
    }
}

#[test]
fn discovery_exposes_the_shared_registration_grant_without_consuming_it() {
    let (directory, actor, supervisor, _evaluator) = ready();
    let (mut tower, grant) = install(actor, &supervisor, next_window(), 4);
    let mut request = TowerRequest::new("discover-andon-window", mount(), Verb::Capabilities);
    request.arguments = ReadArguments::OperatorCapabilities;
    let bytes = image(&directory);
    for _ in 0..3 {
        let response = tower.dispatch(&access(true, true), request.clone());
        let PublicationResult::OperatorCapabilities(linked) = &response.result else {
            panic!("discovery")
        };
        assert_eq!(linked.entries().len(), OperatorCapabilityKind::ALL.len());
        let entry = linked
            .entries()
            .iter()
            .find(|entry| entry.kind() == OperatorCapabilityKind::AndonWindow)
            .unwrap();
        assert!(entry.configured() && entry.route_connected() && entry.purpose_allowed());
        let Some(NativeKnowledge::Known { value, .. }) = entry.native() else {
            panic!("live original grant")
        };
        assert_eq!(value.limits()["remaining_queries"], 4);
        assert_eq!(value.limits()["remaining_operations"], 1);
        assert_eq!(
            entry.kind().verbs(),
            &[Verb::Next, Verb::RegisterAndonWindow]
        );
        response.to_json().unwrap();
    }
    assert_eq!(grant.remaining_queries(), 4);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(image(&directory), bytes);
}

#[test]
fn replacing_the_selected_grant_cannot_execute_an_old_plan_with_a_different_budget() {
    let (directory, actor, supervisor, _evaluator) = ready();
    let (mut tower, original_grant) = install(actor, &supervisor, next_window(), 3);
    let old_plan = tower.dispatch(&access(true, true), query());
    let old_action = action(&old_plan);
    let replacement = AndonBudget {
        window: 3,
        start: 200,
        end: 300,
        false_stops_allowed: 4,
        cost_per_stop: 13,
    };
    let (port, replacement_grant) = supervisor
        .andon_window_port(replacement, 3, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    tower.set_andon_window(Some(port)).unwrap();
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(&directory);
    let refused = tower.dispatch(&access(true, true), old_action);
    let PublicationResult::AndonWindowRegistration(linked) = &refused.result else {
        panic!("native predecessor assertion refusal: {refused:?}");
    };
    assert_eq!(linked.attempted().expected_budget(), next_window());
    assert_eq!(
        linked.native(),
        &Err(FileAndonWindowError::IdempotencyConflict)
    );
    assert_eq!(
        refused.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    refused.to_json().unwrap();
    assert_eq!(replacement_grant.remaining_operations(), 1);
    assert_eq!(replacement_grant.remaining_queries(), 3);
    assert_eq!(original_grant.remaining_operations(), 1);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(&directory), bytes);

    let current_plan = tower.dispatch(&access(true, true), query());
    let current_action = action(&current_plan);
    let ReadArguments::RegisterAndonWindow {
        expected_budget, ..
    } = &current_action.arguments
    else {
        panic!("exact asserted window");
    };
    assert_eq!(*expected_budget, replacement);
    let response = tower.dispatch(&access(true, true), current_action);
    let PublicationResult::AndonWindowRegistration(linked) = &response.result else {
        panic!("current independently selected registration");
    };
    let Ok(NativeKnowledge::Known { value, basis }) = linked.native() else {
        panic!("known original registration");
    };
    assert!(value.registered());
    assert_eq!(value.budget(), replacement);
    assert_eq!(linked.attempted().expected_budget(), replacement);
    assert_eq!(basis.journal_revision, before.revision + 1);
    let host = supervisor.host().unwrap();
    assert!(!host.andon_ledger().unwrap().windows().contains_key(&2));
    assert_eq!(
        host.andon_ledger().unwrap().windows()[&3].budget,
        replacement
    );
    assert_eq!(replacement_grant.remaining_operations(), 0);
    assert_eq!(original_grant.remaining_operations(), 1);
    response.to_json().unwrap();
}
