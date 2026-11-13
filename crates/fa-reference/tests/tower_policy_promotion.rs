//! A separately approved policy campaign reaches the ORIGINAL durable owner.
//! Synthetic helper verdicts exercise custody and effect gates, not model safety.
#![cfg(unix)]

#[cfg(target_os = "linux")]
#[path = "tower_policy_promotion/peer.rs"]
mod peer;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::Error;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        JournalError,
        governance::PolicyUpdate,
        observed::{
            FileHumanReviewer,
            governance::campaigns::{
                FilePolicyCampaignReview, FilePolicyGovernor, FilePolicyPromotionPermit,
            },
        },
        requests::actor::{
            FilePolicyPromotionBlockReason, FilePolicyPromotionError, FilePolicyPromotionPlan,
            FilePolicyPromotionPlanResult, FilePolicyPromotionReceipt, FilePolicyPromotionResult,
            FileSupervisorPolicyPromotionGrant, MAX_POLICY_PROMOTION_REPLAY_EVENTS,
        },
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::actor::{
    ActorProposal, Knowledge as ActorKnowledge, UnknownReason as ActorUnknown,
};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::action::{ActionState, ElapsedTick, Purpose};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::knowledge_surface::KNOWLEDGE_JSON_LIMITS;
use fa_reference::strict_json::{self, Json};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ENVELOPE_FIELDS, ReadArguments, ReasonCode, TowerAccess, TowerEncodingError, TowerInstallError,
    TowerRequest, Verb, decode_request, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use support::{Directory, Port, Supervisor, observe, oversight, ready, snapshot};

const REGISTRATION: u64 = 41;
const CAMPAIGN: u64 = 8001;
const OPERATION: u64 = 9001;
const SEED_ATTEMPT: u64 = 900001;
const REPLAY_EVENTS: usize = MAX_POLICY_PROMOTION_REPLAY_EVENTS;

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(REGISTRATION)).unwrap()
}

fn access(operator: bool, permit: bool) -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        operator,
        if permit {
            vec![PurposeTag::Permit, PurposeTag::Experiment]
        } else {
            Vec::new()
        },
        0,
    )
}

fn limits() -> ReplayLimits {
    ReplayLimits {
        cases: 16,
        input_bytes: 1_048_576,
    }
}

fn guarded(
    root: &Directory,
) -> (
    Port,
    Supervisor,
    FileHumanReviewer,
    FilePolicyGovernor,
    ActorProposal,
) {
    let (mut host, human) = oversight::create(root);
    let governor = host
        .enable_policy_campaigns(host.revision(), limits(), 8)
        .unwrap();
    // Campaigns require real retained cases, including exact denials. This
    // original proposal fails the known key-value predicate, and its payload
    // also stays denied under the later three-byte candidate. It owns no
    // reservation or effect key, so promotion cannot cancel or refund it.
    let mut denied = snapshot();
    denied.values.insert(7, b"denied".to_vec());
    let seed = oversight::spec(&host, b"seed denial");
    host.propose(host.revision(), SEED_ATTEMPT, seed, denied)
        .unwrap();
    assert_eq!(
        host.inspect().control.ledger.stages[&SEED_ATTEMPT],
        ActionState::Denied
    );
    let proposal = support::proposal(&oversight::spec(&host, b"old publication"));
    let (actor, supervisor) = host.into_actor_gateway();
    (actor, supervisor, human, governor, proposal)
}

fn same_policy(generation: u64) -> Policy {
    Policy::new(
        generation,
        support::profile().delivery.policy.nodes().to_vec(),
    )
    .unwrap()
}

fn approve(
    supervisor: &mut Supervisor,
    governor: &FilePolicyGovernor,
    candidate: Policy,
) -> (FilePolicyPromotionPermit, FilePolicyCampaignReview) {
    let mut host = supervisor.host_mut().unwrap();
    let control = host.inspect().control;
    let update =
        PolicyUpdate::new(CAMPAIGN, control.sequence, control.ledger.epoch, candidate).unwrap();
    let revision = host.revision();
    let review = host.request_policy_campaign(revision, &update).unwrap();
    let revision = host.revision();
    let key = governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    (key, review)
}

fn tower(actor: Port) -> PublicationTower {
    PublicationTower::new_control_operator(mount(), actor, PublicationLimits::default()).unwrap()
}

fn granted(
    actor: Port,
    supervisor: &Supervisor,
    key: FilePolicyPromotionPermit,
) -> (PublicationTower, FileSupervisorPolicyPromotionGrant) {
    let (port, grant) = supervisor
        .policy_promotion_port(key, 32, REPLAY_EVENTS)
        .unwrap();
    let mut facade = tower(actor);
    facade.set_policy_promotion(Some(port)).unwrap();
    (facade, grant)
}

fn query(operation: u64) -> TowerRequest {
    let mut request = TowerRequest::new("inspect-policy-promotion", mount(), Verb::Next);
    request.arguments = ReadArguments::PolicyPromotionNext {
        campaign: CAMPAIGN,
        operation,
    };
    request
}

fn next(facade: &mut PublicationTower, operation: u64) -> PublicationResponse {
    facade.exchange(
        &access(true, true),
        encode_request(&query(operation)).unwrap().as_bytes(),
    )
}

fn native_next(response: &PublicationResponse) -> &FilePolicyPromotionPlanResult {
    let PublicationResult::PolicyPromotionNext(linked) = &response.result else {
        panic!("expected native campaign plan: {response:?}");
    };
    linked.native()
}

fn plan(response: &PublicationResponse) -> &FilePolicyPromotionPlan {
    let Ok(ActorKnowledge::Known { value, basis }) = native_next(response) else {
        panic!("expected known campaign plan: {response:?}");
    };
    assert_eq!(value.basis(), *basis);
    value
}

fn action(response: &PublicationResponse) -> TowerRequest {
    let PublicationResult::PolicyPromotionNext(linked) = &response.result else {
        panic!("campaign plan");
    };
    linked
        .action()
        .unwrap()
        .expect("independently approved campaign")
}

fn native(response: &PublicationResponse) -> &FilePolicyPromotionResult {
    let PublicationResult::PolicyPromotion(linked) = &response.result else {
        panic!("expected original policy transition: {response:?}");
    };
    linked.native()
}

fn receipt(response: &PublicationResponse) -> &FilePolicyPromotionReceipt {
    let Ok(ActorKnowledge::Known { value, basis }) = native(response) else {
        panic!("expected acknowledged promotion: {response:?}");
    };
    assert_eq!(value.after(), *basis);
    assert_eq!(response.envelope.reason_code, None);
    assert!(!response.envelope.retry_safe);
    value
}

fn refused(response: &PublicationResponse, reason: ReasonCode) {
    assert!(matches!(response.result, PublicationResult::Refused(_)));
    assert_eq!(response.envelope.reason_code, Some(reason));
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn field<'a>(value: &'a Json, name: &str) -> &'a Json {
    value.get(name).unwrap_or_else(|| panic!("missing {name}"))
}

fn document(response: &PublicationResponse) -> Json {
    strict_json::parse(
        response.to_json().unwrap().as_bytes(),
        KNOWLEDGE_JSON_LIMITS,
    )
    .unwrap()
}

#[test]
fn approved_promotion_cancels_old_reservations_preserves_unknown_charge_and_new_work_publishes() {
    let root = Directory::new();
    let (actor, mut supervisor, human, governor, proposal) = guarded(&root);
    observe(&mut supervisor);
    actor.submit(701, &proposal).unwrap();
    let old = ready(&mut supervisor, &human, 701);
    observe(&mut supervisor);
    let unknown_ticket = actor.submit(702, &proposal).unwrap();
    let unknown = ready(&mut supervisor, &human, 702);
    {
        let mut host = supervisor.host_mut().unwrap();
        oversight::dispatch(&mut host, &unknown);
        let revision = host.revision();
        host.reconcile(revision, unknown.automatic.attempt())
            .unwrap();
    }
    let (key, review) = approve(
        &mut supervisor,
        &governor,
        Policy::new(2, vec![Predicate::PayloadAtMost(3)]).unwrap(),
    );
    assert!(!review.report().newly_blocked().is_empty());
    let (mut facade, grant) = granted(actor.clone(), &supervisor, key);
    let before = supervisor.host().unwrap().inspect();
    let original = image(&root);
    let proposed = next(&mut facade, OPERATION);
    assert!(plan(&proposed).can_promote());
    assert_eq!(plan(&proposed).candidate_policy_generation(), 2);
    assert_eq!(plan(&proposed).basis().campaign, CAMPAIGN);
    assert_eq!(image(&root), original);
    assert_eq!(grant.remaining_operations(), 1);
    let command = action(&proposed);
    assert_eq!(command.verb, Verb::Promote);
    let encoded = encode_request(&command).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), command);
    let completed = facade.exchange(&access(true, true), encoded.as_bytes());
    let result = receipt(&completed);
    assert_eq!(result.before(), plan(&proposed).basis());
    assert_eq!(result.operation(), OPERATION);
    assert_eq!(result.candidate_policy_generation(), 2);
    assert_eq!(result.cancelled(), &[old.automatic.attempt()]);
    assert_eq!(result.refunded_units(), 16);
    assert_eq!(
        result.after().journal_revision,
        result.before().journal_revision + 1
    );
    assert_eq!(
        result.after().control_sequence,
        result.before().control_sequence + 1
    );
    assert_eq!(
        result.after().authority_epoch,
        result.before().authority_epoch + 1
    );
    assert_eq!(result.after().policy_generation, 2);
    assert_eq!(result.after().recorded_at, result.before().recorded_at);
    {
        let mut host = supervisor.host_mut().unwrap();
        let after = host.inspect();
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.control.ledger.charged, 16);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.executions, 0);
        assert_eq!(after.payload, before.payload);
        assert_eq!(
            after.control.ledger.stages[&unknown.automatic.attempt()],
            ActionState::Unknown
        );
        assert_eq!(
            after.control.ledger.stages[&old.automatic.attempt()],
            ActionState::Cancelled
        );
        assert_eq!(
            host.human_status(old.human.request()).unwrap().disposition,
            HumanDisposition::Revoked
        );
        let revision = host.revision();
        assert!(
            host.dispatch(
                revision,
                &old.automatic,
                &old.human,
                &old.action,
                &old.inputs,
                snapshot()
            )
            .is_err()
        );
        assert_eq!(host.revision(), revision);
    }
    // The new policy did not approve an effect. A fresh epoch proposal needs
    // original full-input congress and a new independently held human key.
    let fresh = {
        let host = supervisor.host().unwrap();
        support::proposal(&oversight::spec(&host, b"ok"))
    };
    assert_eq!(
        fresh.expected_policy_epoch,
        proposal.expected_policy_epoch + 1
    );
    observe(&mut supervisor);
    actor.submit(703, &fresh).unwrap();
    let current = ready(&mut supervisor, &human, 703);
    let saved = image(&root);
    let repeated = facade.exchange(&access(true, true), encoded.as_bytes());
    assert_eq!(native(&repeated), native(&completed));
    assert_eq!(image(&root), saved);
    assert_eq!(grant.remaining_operations(), 0);
    {
        let mut host = supervisor.host_mut().unwrap();
        assert_eq!(
            host.inspect().control.ledger.stages[&current.automatic.attempt()],
            ActionState::Authorized
        );
        oversight::dispatch(&mut host, &current);
        let revision = host.revision();
        assert!(matches!(
            host.publish(revision, current.automatic.attempt()).unwrap(),
            EndpointOutcome::Executed { .. }
        ));
        let revision = host.revision();
        host.reconcile(revision, current.automatic.attempt())
            .unwrap();
        let after = host.inspect();
        assert_eq!(after.payload, b"ok");
        assert_eq!(after.executions, 1);
        assert_eq!(after.control.ledger.charged, 32);
        assert_eq!(after.control.ledger.reserved, 0);
    }
    assert!(matches!(
        actor.poll(&unknown_ticket),
        ActorKnowledge::Unknown {
            reason: ActorUnknown::OutcomeUnknown
        }
    ));
}

#[test]
fn scope_purpose_operator_and_exact_predecessor_are_required_before_any_mutation() {
    let root = Directory::new();
    let (actor, mut supervisor, _, governor, _) = guarded(&root);
    let (key, _) = approve(&mut supervisor, &governor, same_policy(2));
    let (mut facade, grant) = granted(actor, &supervisor, key);
    let proposed = next(&mut facade, OPERATION);
    let command = action(&proposed);
    let saved = image(&root);
    refused(
        &facade.dispatch(&access(false, true), command.clone()),
        ReasonCode::DeniedByPolicy,
    );
    refused(
        &facade.dispatch(&access(true, false), command.clone()),
        ReasonCode::PurposeMismatch,
    );
    let foreign = TowerAccess::new(
        CallerScope::full_tenant_access("2"),
        true,
        vec![PurposeTag::Permit],
        0,
    );
    refused(
        &facade.dispatch(&foreign, command.clone()),
        ReasonCode::DeniedByPolicy,
    );
    for generation in [None, Some(REGISTRATION + 1)] {
        let mut changed = command.clone();
        changed.address.generation = generation;
        refused(
            &facade.dispatch(&access(true, true), changed),
            ReasonCode::StaleGeneration,
        );
    }
    for coordinate in 0..6 {
        let mut changed = command.clone();
        let ReadArguments::Promote { expected, .. } = &mut changed.arguments else {
            panic!("promotion");
        };
        match coordinate {
            0 => expected.scope.tenant += 1,
            1 => expected.scope.principal += 1,
            2 => expected.scope.run += 1,
            3 => expected.scope.branch += 1,
            4 => expected.scope.authority += 1,
            _ => expected.scope.purpose = Purpose::Experiment,
        }
        refused(
            &facade.dispatch(&access(true, true), changed),
            ReasonCode::IntegrityFailure,
        );
    }
    for coordinate in 0..6 {
        let mut changed = command.clone();
        let ReadArguments::Promote { expected, .. } = &mut changed.arguments else {
            panic!("promotion");
        };
        match coordinate {
            0 => expected.journal_revision += 1,
            1 => expected.control_sequence += 1,
            2 => expected.authority_epoch += 1,
            3 => expected.policy_generation += 1,
            4 => expected.recorded_at = Some(ElapsedTick(2)),
            _ => expected.recorded_at = None,
        }
        assert_eq!(
            native(&facade.dispatch(&access(true, true), changed)),
            &Err(FilePolicyPromotionError::StalePredecessor)
        );
    }
    let mut changed = command.clone();
    changed.idempotency_key = Some((OPERATION + 1).to_string());
    refused(
        &facade.dispatch(&access(true, true), changed),
        ReasonCode::IntegrityFailure,
    );
    assert_eq!(image(&root), saved);
    assert_eq!(
        (grant.remaining_queries(), grant.remaining_operations()),
        (31, 1)
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    assert_eq!(
        native(&facade.dispatch(&access(true, true), command)),
        &Err(FilePolicyPromotionError::StalePredecessor)
    );
    let current = next(&mut facade, OPERATION);
    assert!(plan(&current).can_promote());
    receipt(&facade.dispatch(&access(true, true), action(&current)));
}

#[test]
fn replay_or_noncontrol_mounts_cannot_promote_and_foreign_or_revoked_custody_cannot_substitute() {
    let root = Directory::new();
    let (actor, mut supervisor, _, governor, _) = guarded(&root);
    let saved = image(&root);
    supervisor
        .host()
        .unwrap()
        .replay_candidate_policy(same_policy(2), limits())
        .unwrap();
    let mut unselected = tower(actor.clone());
    refused(
        &next(&mut unselected, OPERATION),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(image(&root), saved);

    let (key, _) = approve(&mut supervisor, &governor, same_policy(2));
    let (port, grant) = supervisor
        .policy_promotion_port(key, 8, REPLAY_EVENTS)
        .unwrap();
    let mut actor_tower =
        PublicationTower::new(mount(), actor.clone(), PublicationLimits::default()).unwrap();
    actor_tower
        .set_policy_promotion(Some(port.clone()))
        .unwrap();
    refused(
        &next(&mut actor_tower, OPERATION),
        ReasonCode::CapabilityUnsupported,
    );
    let mut reconciliation = PublicationTower::new_reconciliation_operator(
        mount(),
        actor.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    reconciliation
        .set_policy_promotion(Some(port.clone()))
        .unwrap();
    refused(
        &next(&mut reconciliation, OPERATION),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(
        (grant.remaining_queries(), grant.remaining_operations()),
        (8, 1)
    );
    let mut control = tower(actor.clone());
    control.set_policy_promotion(Some(port)).unwrap();
    let proposed = next(&mut control, OPERATION);
    let command = action(&proposed);
    let saved = image(&root);
    let mut intake = TowerRequest::new("operator-cannot-propose", mount(), Verb::Propose);
    let proposal = support::proposal(&oversight::spec(&supervisor.host().unwrap(), b"actor-only"));
    intake.idempotency_key = Some("701".into());
    intake.arguments = ReadArguments::Propose {
        request: 701,
        proposal,
    };
    refused(
        &control.dispatch(&access(true, true), intake),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(image(&root), saved);

    let other_root = Directory::new();
    let (other_actor, mut other, _, other_governor, _) = guarded(&other_root);
    let (foreign_key, _) = approve(&mut other, &other_governor, same_policy(2));
    let (foreign_port, foreign_grant) = other
        .policy_promotion_port(foreign_key, 8, REPLAY_EVENTS)
        .unwrap();
    assert_eq!(
        control.set_policy_promotion(Some(foreign_port)),
        Err(TowerInstallError::SnapshotMismatch)
    );
    refused(
        &control.dispatch(&access(true, true), command.clone()),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(foreign_grant.remaining_operations(), 1);
    // Grant removal withdraws the existing key instead of reconstructing it
    // from a campaign id, review report or the approved journal disposition.
    let mut selected = tower(other_actor);
    let key = {
        let mut host = other.host_mut().unwrap();
        let control = host.inspect().control;
        let update = PolicyUpdate::new(
            CAMPAIGN + 1,
            control.sequence,
            control.ledger.epoch,
            same_policy(3),
        )
        .unwrap();
        let revision = host.revision();
        let review = host.request_policy_campaign(revision, &update).unwrap();
        let revision = host.revision();
        other_governor
            .approve(&mut host, revision, &review, false)
            .unwrap()
    };
    let (port, retained) = other.policy_promotion_port(key, 4, REPLAY_EVENTS).unwrap();
    selected.set_policy_promotion(Some(port)).unwrap();
    retained.revoke();
    let mut revoked_query = query(OPERATION);
    revoked_query.arguments = ReadArguments::PolicyPromotionNext {
        campaign: CAMPAIGN + 1,
        operation: OPERATION,
    };
    let revoked = selected.dispatch(&access(true, true), revoked_query);
    assert!(matches!(
        native_next(&revoked),
        Ok(ActorKnowledge::Withheld { .. })
    ));
    assert_eq!(image(&root), saved);
}

#[test]
fn closed_wire_and_response_binding_preserve_campaign_and_operation_identity() {
    let root = Directory::new();
    let (actor, mut supervisor, _, governor, _) = guarded(&root);
    let (key, _) = approve(&mut supervisor, &governor, same_policy(2));
    let (mut facade, grant) = granted(actor, &supervisor, key);
    let proposed = next(&mut facade, OPERATION);
    let command = action(&proposed);
    let encoded_next = encode_request(&query(OPERATION)).unwrap();
    let encoded_command = encode_request(&command).unwrap();
    let saved = image(&root);
    for malformed in [
        encoded_next.replace("\"action\":\"promote_policy\",", ""),
        encoded_next.replace("\"campaign\":\"8001\"", "\"campaign\":8001"),
        encoded_next.replace("\"operation\":\"9001\"", "\"operation\":\"09001\""),
        encoded_command.replace("\"policy_generation\":\"1\"", "\"policy_generation\":1"),
        encoded_command.replace(
            "\"recorded_at\":\"1\"",
            "\"recorded_at\":\"1\",\"approved\":true",
        ),
        encoded_command.replace("\"verb\":\"promote\"", "\"verb\":\"resolve\""),
    ] {
        assert_ne!(malformed, encoded_next);
        assert_ne!(malformed, encoded_command);
        assert!(decode_request(malformed.as_bytes()).is_err());
        refused(
            &facade.exchange(&access(true, true), malformed.as_bytes()),
            ReasonCode::IntegrityFailure,
        );
    }
    assert_eq!(image(&root), saved);
    assert_eq!(
        (grant.remaining_queries(), grant.remaining_operations()),
        (31, 1)
    );
    let completed = facade.dispatch(&access(true, true), command.clone());
    receipt(&completed);
    let parsed = document(&completed);
    assert_eq!(
        field(&parsed, "envelope").as_object().unwrap().len(),
        ENVELOPE_FIELDS.len()
    );
    for response in [proposed, completed] {
        response.to_json().unwrap();
        let mut changed = response.clone();
        changed.envelope.address = Address::new("1", Noun::Run, "4", Some(REGISTRATION)).unwrap();
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.address.generation = Some(REGISTRATION + 1);
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.operation_id.push_str("-different");
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut changed = response.clone();
        changed.envelope.idempotency_key = Some((OPERATION + 1).to_string());
        assert_eq!(changed.to_json(), Err(TowerEncodingError::InvalidKnowledge));
        let mut tiny = response;
        tiny.envelope.limits.max_response_bytes = 1;
        assert_eq!(tiny.to_json(), Err(TowerEncodingError::OutputLimit));
    }
    let saved = image(&root);
    let mut conflict = command;
    let ReadArguments::Promote { expected, .. } = &mut conflict.arguments else {
        panic!("promotion");
    };
    expected.policy_generation += 1;
    assert_eq!(
        native(&facade.dispatch(&access(true, true), conflict)),
        &Err(FilePolicyPromotionError::IdempotencyConflict)
    );
    assert_eq!(image(&root), saved);
}

#[test]
fn exhausted_replay_or_queries_cannot_spend_the_original_approval() {
    let root = Directory::new();
    let (actor, mut supervisor, _, governor, _) = guarded(&root);
    let (key, _) = approve(&mut supervisor, &governor, same_policy(2));
    let (port, grant) = supervisor.policy_promotion_port(key, 1, 1).unwrap();
    let mut facade = tower(actor);
    facade.set_policy_promotion(Some(port)).unwrap();
    let saved = image(&root);
    let blocked = next(&mut facade, OPERATION);
    assert_eq!(
        plan(&blocked).block_reason(),
        Some(FilePolicyPromotionBlockReason::ReplayLimit)
    );
    assert!(!plan(&blocked).can_promote());
    assert!(plan(&blocked).request().is_none());
    let PublicationResult::PolicyPromotionNext(linked) = &blocked.result else {
        panic!("promotion plan");
    };
    assert!(linked.action().unwrap().is_none());
    let mut command = TowerRequest::new("cannot-spend-short-replay", mount(), Verb::Promote);
    command.idempotency_key = Some(OPERATION.to_string());
    command.arguments = ReadArguments::Promote {
        operation: OPERATION,
        expected: plan(&blocked).basis(),
    };
    assert_eq!(
        native(&facade.dispatch(&access(true, true), command)),
        &Err(FilePolicyPromotionError::Capacity)
    );
    assert_eq!(
        native_next(&next(&mut facade, OPERATION)),
        &Err(FilePolicyPromotionError::Capacity)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(image(&root), saved);
}

#[test]
fn a_foreign_approval_key_is_not_a_port_even_when_every_visible_identifier_matches() {
    let first_root = Directory::new();
    let second_root = Directory::new();
    let (_, mut first, _, first_governor, _) = guarded(&first_root);
    let (_, mut second, _, second_governor, _) = guarded(&second_root);
    let (key, _) = approve(&mut first, &first_governor, same_policy(2));
    let (_matching, _) = approve(&mut second, &second_governor, same_policy(2));
    assert_eq!(
        first.host().unwrap().inspect(),
        second.host().unwrap().inspect()
    );
    let saved_first = image(&first_root);
    let saved_second = image(&second_root);
    assert_eq!(
        second.policy_promotion_port(key, 2, REPLAY_EVENTS).err(),
        Some(JournalError::Contract(Error::Binding))
    );
    assert_eq!(image(&first_root), saved_first);
    assert_eq!(image(&second_root), saved_second);
}
