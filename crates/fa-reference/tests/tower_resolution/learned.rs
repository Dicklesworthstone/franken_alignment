//! Actual learned source release through the separately granted operator path.
//! Numerical weights and synthetic ballots exercise custody, not detection quality.
use super::{
    OPERATION, REQUEST, Directory, Supervisor, access, action, document, field, image,
    native_next, native_resolution, next, next_request, plan, receipt, refused, support,
};
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        Reconciliation,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            decoder::learned::FileLearnedConfig,
        },
        requests::actor::{
            FileActorTicket, FileLearnedTextStreamActorPort, FileResolutionBlockReason,
            FileResolutionError, FileSupervisorResolutionGrant, LearnedTextRelease,
            MAX_RESOLUTION_REPLAY_EVENTS, MAX_REVIEW_EXPLANATION_EVENTS,
        },
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::containment::{
    ActorState, RestartGrade, RestartProfile,
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorOutcome, ActorProposal, Knowledge as ActorKnowledge},
    decoder_monitoring::LearnedDecoderBindingLimits,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::tower::publication::{PublicationLimits, PublicationTower};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest, Verb,
    decode_request, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;

#[path = "../support/learned_text_model.rs"]
mod numerical;

fn profile() -> FileOversightProfile {
    let mut profile = support::profile();
    profile.delivery.total = 4096;
    profile.delivery.initial_payload.clear();
    profile.delivery.actor = ActorState::new(
        RestartProfile {
            id: 1,
            generation: 1,
            host_generation: 1,
            model_generation: 3,
            tokenizer_generation: 4,
            state_schema_generation: 1,
            grade: RestartGrade::AuditOnly,
        },
        Vec::new(),
        vec![0],
        vec![0],
        0,
    )
    .unwrap();
    profile
}

fn recipe() -> FileLearnedConfig {
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    FileLearnedConfig::new_text_stream(
        model.clone(),
        numerical::tokenizer(&model),
        numerical::config(&model),
        LearnedDecoderBindingLimits::default(),
        StreamProfile::new(1, 1, 4, 64, 128).unwrap(),
    )
    .unwrap()
}

fn create(root: &Directory) -> (FileLearnedTextStreamActorPort, Supervisor, FileHumanReviewer) {
    let (mut host, reviewer) =
        FileOversight::create_with_learned_text_stream(root.store(), profile(), recipe()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    (actor, supervisor, reviewer)
}

fn generate(supervisor: &mut Supervisor) {
    let mut host = supervisor.host_mut().unwrap();
    for _ in 0..4 {
        let state = host.learned_generation_inspection().unwrap().numerical;
        if !state.status.is_active() {
            break;
        }
        let revision = host.revision();
        host.advance_learned_generation(revision, state.actor_revision, state.position)
            .unwrap()
            .unwrap();
    }
    let generated = host.learned_generation_inspection().unwrap();
    assert!(!generated.numerical.status.is_active());
    assert!(generated.numerical.work.admitted_tokens > 0);
    assert!(generated.numerical.work.reserved_decoder_products > 0);
    assert!(generated.numerical.telemetry.compression_work_units > 0);
}

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(super::REGISTRATION)).unwrap()
}

fn tower(actor: FileLearnedTextStreamActorPort) -> PublicationTower {
    PublicationTower::new_learned_reconciliation_operator(
        mount(), actor, PublicationLimits::default(),
    )
    .unwrap()
}

struct Learned {
    root: Directory,
    actor: FileLearnedTextStreamActorPort,
    supervisor: Supervisor,
    ticket: FileActorTicket<FileOversight>,
}

fn dispatched(published: bool) -> Learned {
    let root = Directory::new();
    let (actor, mut supervisor, reviewer) = create(&root);
    generate(&mut supervisor);
    support::observe(&mut supervisor);
    let ticket = actor
        .submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100))
        .unwrap();
    assert_eq!(actor.poll(&ticket), ActorKnowledge::Pending { request: REQUEST });
    let keys = support::ready(&mut supervisor, &reviewer, REQUEST);
    {
        let mut host = supervisor.host_mut().unwrap();
        support::oversight::dispatch(&mut host, &keys);
        if published {
            let revision = host.revision();
            let result = host
                .publish_checked(
                    revision,
                    keys.automatic.attempt(),
                    Some(&keys.inputs),
                    support::snapshot(),
                    ElapsedTick(2),
                )
                .unwrap();
            assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
        }
    }
    Learned { root, actor, supervisor, ticket }
}

fn install(
    facade: &mut PublicationTower,
    supervisor: &Supervisor,
    numerical_events: usize,
) -> FileSupervisorResolutionGrant {
    let (port, grant) = supervisor
        .resolution_port_with_numerical_budget(
            16, 4, MAX_RESOLUTION_REPLAY_EVENTS, numerical_events,
        )
        .unwrap();
    facade.set_resolution(Some(port)).unwrap();
    grant
}

#[test]
fn learned_next_executes_original_receipt_reconciliation_and_preserves_exact_retry_across_facades() {
    let f = dispatched(true);
    let mut facade = tower(f.actor.clone());
    let total = 4 * MAX_RESOLUTION_REPLAY_EVENTS;
    let (port, grant) = f.supervisor
        .resolution_port_with_numerical_budget(8, 4, MAX_RESOLUTION_REPLAY_EVENTS, total)
        .unwrap();
    facade.set_resolution(Some(port.clone())).unwrap();
    let before = f.supervisor.host().unwrap().inspect();
    let generation = f.supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let original = image(&f.root);
    let response = next(&mut facade, OPERATION);
    assert!(plan(&response).can_reconcile());
    assert_eq!(plan(&response).stage(), ActionState::Dispatching);
    assert_eq!(plan(&response).basis().request, REQUEST);
    assert_ne!(plan(&response).basis().attempt, REQUEST);
    assert_eq!(image(&f.root), original);
    assert_eq!(f.supervisor.host().unwrap().inspect(), before);
    assert_eq!(f.supervisor.host().unwrap().learned_generation_inspection().unwrap(), generation);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total));
    assert_eq!(grant.remaining_operations(), 4);

    let command = action(&response);
    let encoded = encode_request(&command).unwrap();
    assert_eq!(decode_request(encoded.as_bytes()).unwrap(), command);
    let parsed = document(&response);
    let value = field(field(field(field(&parsed, "result"), "value"), "knowledge"), "value");
    for name in ["cost_estimate", "expected_information_gain", "ranking", "current_eligibility"] {
        assert_eq!(field(field(value, name), "state").as_str(), Some("unknown"));
    }
    let completed = facade.exchange(&access(true, true), encoded.as_bytes());
    let resolved = receipt(&completed);
    assert_eq!(resolved.before(), plan(&response).basis());
    assert_eq!(resolved.after().journal_revision, before.revision + 1);
    assert_eq!(resolved.before().recorded_at, resolved.after().recorded_at);
    assert_eq!(resolved.stage(), ActionState::Confirmed);
    assert!(matches!(resolved.outcome(), Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
    let after = f.supervisor.host().unwrap().inspect();
    let after_generation = f.supervisor.host().unwrap().learned_generation_inspection().unwrap();
    assert_eq!(after_generation.numerical, generation.numerical);
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert!(matches!(f.actor.poll(&f.ticket), ActorKnowledge::Known { value: ActorOutcome::Executed, .. }));
    let reserved = before.revision as usize + 1;
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total - reserved));
    assert_eq!(grant.remaining_operations(), 3);

    let committed = image(&f.root);
    let mut reconnected = tower(f.actor.clone());
    reconnected.set_resolution(Some(port)).unwrap();
    let repeated = reconnected.exchange(&access(true, true), encoded.as_bytes());
    assert_eq!(native_resolution(&repeated), native_resolution(&completed));
    assert_eq!(image(&f.root), committed);
    assert_eq!(f.supervisor.host().unwrap().inspect(), after);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total - reserved));
    assert_eq!(grant.remaining_operations(), 3);
    let terminal = next(&mut reconnected, OPERATION + 1);
    assert_eq!(plan(&terminal).block_reason(), Some(FileResolutionBlockReason::Terminal));
    assert!(!plan(&terminal).can_reconcile());
}

#[test]
fn learned_constructor_and_review_grant_supply_no_resolution_work_and_one_short_remains_closed() {
    let f = dispatched(false);
    let mut facade = tower(f.actor.clone());
    let before = f.supervisor.host().unwrap().inspect();
    let original = image(&f.root);
    let cut = before.revision as usize + 1;
    refused(&next(&mut facade, OPERATION), ReasonCode::CapabilityUnsupported);

    // This original owner already has a completed congress. Its independently
    // granted historical explanation cannot be spent as mutation authority.
    let (review, review_grant) = f.supervisor
        .review_explanation_port_with_numerical_budget(
            1, MAX_REVIEW_EXPLANATION_EVENTS, MAX_REVIEW_EXPLANATION_EVENTS,
        )
        .unwrap();
    facade.set_review_explanation(Some(review)).unwrap();
    refused(&next(&mut facade, OPERATION), ReasonCode::CapabilityUnsupported);
    assert_eq!(review_grant.remaining_queries(), 1);
    assert_eq!(review_grant.remaining_numerical_replay_events(), Some(MAX_REVIEW_EXPLANATION_EVENTS));

    let mut capable = tower(f.actor.clone());
    let capable_grant = install(&mut capable, &f.supervisor, cut);
    let possible = next(&mut capable, OPERATION);
    let command = action(&possible);
    let (default, default_grant) = f.supervisor
        .resolution_port(4, 2, MAX_RESOLUTION_REPLAY_EVENTS)
        .unwrap();
    facade.set_resolution(Some(default)).unwrap();
    let blocked = next(&mut facade, OPERATION);
    assert_eq!(plan(&blocked).block_reason(), Some(FileResolutionBlockReason::NumericalReplayUnsupported));
    assert!(!plan(&blocked).can_reconcile());
    assert!(blocked.to_json().unwrap().contains("\"block_reason\":\"numerical_replay_unsupported\""));
    assert_eq!(
        native_resolution(&facade.dispatch(&access(true, true), command.clone())),
        &Err(FileResolutionError::Unsupported),
    );
    assert_eq!(default_grant.remaining_numerical_replay_events(), None);
    assert_eq!(default_grant.remaining_operations(), 2);

    let short_grant = install(&mut facade, &f.supervisor, cut - 1);
    let short = next(&mut facade, OPERATION);
    assert_eq!(plan(&short).block_reason(), Some(FileResolutionBlockReason::NumericalReplayLimit));
    assert!(!plan(&short).can_reconcile());
    let parsed = document(&short);
    let value = field(field(field(field(&parsed, "result"), "value"), "knowledge"), "value");
    assert_eq!(field(value, "block_reason").as_str(), Some("numerical_replay_limit"));
    assert_eq!(
        native_resolution(&facade.dispatch(&access(true, true), command.clone())),
        &Err(FileResolutionError::Capacity),
    );
    assert_eq!(short_grant.remaining_numerical_replay_events(), Some(cut - 1));
    assert_eq!(short_grant.remaining_operations(), 4);
    assert_eq!(image(&f.root), original);
    assert_eq!(f.supervisor.host().unwrap().inspect(), before);
    assert_eq!(capable_grant.remaining_numerical_replay_events(), Some(cut));

    // An exact complete-cut allowance independently enables the native work.
    let completed = capable.dispatch(&access(true, true), command);
    assert_eq!(receipt(&completed).outcome(), Reconciliation::AwaitingResolution);
    assert_eq!(capable_grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(capable_grant.remaining_operations(), 3);
}

#[test]
fn original_learned_observer_refuses_next_and_resolve_even_with_the_actual_numerical_grant() {
    let f = dispatched(true);
    let mut capable = tower(f.actor.clone());
    let (port, grant) = f.supervisor
        .resolution_port_with_numerical_budget(
            4, 2, MAX_RESOLUTION_REPLAY_EVENTS, 2 * MAX_RESOLUTION_REPLAY_EVENTS,
        )
        .unwrap();
    capable.set_resolution(Some(port.clone())).unwrap();
    let proposed = next(&mut capable, OPERATION);
    let command = action(&proposed);
    let mut observer = PublicationTower::new_learned_observer(
        mount(), f.actor.clone(), PublicationLimits::default(),
    )
    .unwrap();
    observer.set_resolution(Some(port)).unwrap();
    let remaining = grant.remaining_queries();
    let work = grant.remaining_numerical_replay_events();
    let before = image(&f.root);
    refused(&next(&mut observer, OPERATION), ReasonCode::CapabilityUnsupported);
    refused(
        &observer.exchange(&access(true, true), encode_request(&command).unwrap().as_bytes()),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), remaining);
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(grant.remaining_numerical_replay_events(), work);
    assert_eq!(image(&f.root), before);
    let resolved = capable.dispatch(&access(true, true), command);
    assert_eq!(receipt(&resolved).stage(), ActionState::Confirmed);
}

#[test]
fn learned_reconciliation_operator_never_accepts_payload_intake_or_hold_under_broad_purposes() {
    let root = Directory::new();
    let (actor, mut supervisor, _reviewer) = create(&root);
    generate(&mut supervisor);
    support::observe(&mut supervisor);
    let mut facade = tower(actor.clone());
    let total = MAX_RESOLUTION_REPLAY_EVENTS;
    let grant = install(&mut facade, &supervisor, total);
    let before = supervisor.host().unwrap().inspect();
    let generation = supervisor.host().unwrap().learned_generation_inspection().unwrap();
    let original = image(&root);
    let broad = access(true, true);
    for verb in Verb::ALL {
        if !matches!(verb, Verb::Situation | Verb::Explain | Verb::WhyHeld | Verb::Next | Verb::Resolve | Verb::PolicyReplay) {
            let request = TowerRequest::new("closed-learned-operator", mount(), verb);
            refused(&facade.dispatch(&broad, request), ReasonCode::CapabilityUnsupported);
        }
    }
    let proposal = ActorProposal {
        target: before.target,
        payload: b"actor-selected injection".to_vec(),
        units: 32,
        deadline: ElapsedTick(100),
        expected_policy_epoch: before.control.ledger.epoch,
    };
    let mut injected = TowerRequest::new("no-operator-intake", mount(), Verb::Propose);
    injected.idempotency_key = Some(REQUEST.to_string());
    injected.arguments = ReadArguments::Propose { request: REQUEST, proposal: proposal.clone() };
    refused(
        &facade.exchange(&broad, encode_request(&injected).unwrap().as_bytes()),
        ReasonCode::CapabilityUnsupported,
    );
    let mut precheck = TowerRequest::new("no-operator-precheck", mount(), Verb::Precheck);
    precheck.arguments = ReadArguments::Precheck { query: 17, proposal };
    refused(
        &facade.exchange(&broad, encode_request(&precheck).unwrap().as_bytes()),
        ReasonCode::CapabilityUnsupported,
    );
    assert_eq!(grant.remaining_queries(), 16);
    assert_eq!(grant.remaining_operations(), 4);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total));
    assert_eq!(image(&root), original);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().learned_generation_inspection().unwrap(), generation);

    // Every refusal left the source's one-use admission snapshot untouched.
    // Only the actual source release derives and admits these cumulative bytes.
    let ticket = actor.submit(REQUEST, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert_eq!(actor.poll(&ticket), ActorKnowledge::Pending { request: REQUEST });
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    let pending = next(&mut facade, OPERATION);
    assert_eq!(plan(&pending).block_reason(), Some(FileResolutionBlockReason::NotDispatched));
    assert_eq!(plan(&pending).basis().request_generation, 0);
}

#[test]
fn learned_owner_scope_purpose_foreign_grant_and_revocation_are_checked_before_cached_work() {
    let f = dispatched(true);
    let mut facade = tower(f.actor.clone());
    let total = 2 * MAX_RESOLUTION_REPLAY_EVENTS;
    let (port, grant) = f.supervisor
        .resolution_port_with_numerical_budget(8, 2, MAX_RESOLUTION_REPLAY_EVENTS, total)
        .unwrap();
    facade.set_resolution(Some(port.clone())).unwrap();
    let proposed = next(&mut facade, OPERATION);
    let command = action(&proposed);
    let original = image(&f.root);
    let remaining = grant.remaining_queries();
    for (denied, reason) in [
        (access(false, true), ReasonCode::DeniedByPolicy),
        (access(true, false), ReasonCode::PurposeMismatch),
        (
            TowerAccess::new(
                CallerScope::full_tenant_access("2"), true, vec![PurposeTag::Permit], 0,
            ),
            ReasonCode::DeniedByPolicy,
        ),
    ] {
        let query = next_request(&facade, REQUEST, OPERATION);
        refused(&facade.dispatch(&denied, query), reason);
        refused(&facade.dispatch(&denied, command.clone()), reason);
    }
    let mut stale = command.clone();
    stale.address.generation = Some(super::REGISTRATION + 1);
    refused(&facade.dispatch(&access(true, true), stale), ReasonCode::StaleGeneration);
    assert_eq!(grant.remaining_queries(), remaining);
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(total));
    assert_eq!(image(&f.root), original);

    let foreign_root = Directory::new();
    let (_foreign_actor, foreign, _reviewer) = create(&foreign_root);
    let (foreign_port, foreign_grant) = foreign
        .resolution_port_with_numerical_budget(4, 2, MAX_RESOLUTION_REPLAY_EVENTS, total)
        .unwrap();
    assert_eq!(facade.set_resolution(Some(foreign_port)), Err(TowerInstallError::SnapshotMismatch));
    refused(&next(&mut facade, OPERATION), ReasonCode::CapabilityUnsupported);
    assert_eq!(foreign_grant.remaining_queries(), 4);
    assert_eq!(foreign_grant.remaining_operations(), 2);
    assert_eq!(foreign_grant.remaining_numerical_replay_events(), Some(total));

    facade.set_resolution(Some(port)).unwrap();
    let completed = facade.dispatch(&access(true, true), command.clone());
    assert_eq!(receipt(&completed).stage(), ActionState::Confirmed);
    let committed = image(&f.root);
    grant.revoke();
    assert!(matches!(
        native_resolution(&facade.dispatch(&access(true, true), command)),
        Ok(ActorKnowledge::Withheld { authority_required: "supervisor_resolution" }),
    ));
    assert!(matches!(
        native_next(&next(&mut facade, OPERATION + 1)),
        Ok(ActorKnowledge::Withheld { authority_required: "supervisor_resolution" }),
    ));
    assert_eq!(image(&f.root), committed);
}

#[test]
fn learned_resolved_cache_is_unavailable_after_original_owner_loss() {
    let f = dispatched(true);
    let mut facade = tower(f.actor.clone());
    let grant = install(&mut facade, &f.supervisor, 2 * MAX_RESOLUTION_REPLAY_EVENTS);
    let proposed = next(&mut facade, OPERATION);
    let command = action(&proposed);
    assert_eq!(receipt(&facade.dispatch(&access(true, true), command.clone())).stage(), ActionState::Confirmed);
    let work = grant.remaining_numerical_replay_events();
    let operations = grant.remaining_operations();
    let committed = image(&f.root);
    drop(f.supervisor);
    assert!(matches!(
        native_resolution(&facade.dispatch(&access(true, true), command)),
        Ok(ActorKnowledge::Unknown { .. }),
    ));
    assert_eq!(grant.remaining_numerical_replay_events(), work);
    assert_eq!(grant.remaining_operations(), operations);
    assert_eq!(image(&f.root), committed);
}

#[test]
fn learned_missing_receipt_at_deadline_stays_charged_and_never_resends_or_seals() {
    let mut f = dispatched(false);
    {
        let mut host = f.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(100)).unwrap();
    }
    let before = f.supervisor.host().unwrap().inspect();
    let cut = before.revision as usize + 1;
    let mut facade = tower(f.actor.clone());
    let grant = install(&mut facade, &f.supervisor, cut);
    let proposed = next(&mut facade, OPERATION);
    let command = action(&proposed);
    let completed = facade.dispatch(&access(true, true), command.clone());
    assert_eq!(receipt(&completed).outcome(), Reconciliation::AwaitingResolution);
    assert_eq!(receipt(&completed).stage(), ActionState::Unknown);
    assert_eq!(receipt(&completed).after().recorded_at, Some(ElapsedTick(100)));
    let after = f.supervisor.host().unwrap().inspect();
    assert_eq!(after.executions, 0);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert!(after.control.ledger.charged > 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(grant.remaining_operations(), 3);
    let committed = image(&f.root);
    assert_eq!(
        native_resolution(&facade.dispatch(&access(true, true), command)),
        native_resolution(&completed),
    );
    assert_eq!(image(&f.root), committed);
    let exhausted = next(&mut facade, OPERATION + 1);
    assert_eq!(plan(&exhausted).block_reason(), Some(FileResolutionBlockReason::NumericalReplayLimit));
    assert!(!plan(&exhausted).can_reconcile());
    assert_eq!(f.supervisor.host().unwrap().inspect(), after);
}
