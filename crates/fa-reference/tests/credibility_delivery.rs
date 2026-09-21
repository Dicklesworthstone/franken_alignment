//! Public API composition with the original receipt-producing endpoints.
use fa_reference::action::consequence::delivery::{
    DeliveryBroker, DispatchEnvelope, EndpointOutcome, EndpointStatus, NonExecutionReason,
    PublicationEndpoint,
};
use fa_reference::action::consequence::congress::{
    CongressPolicy, CredibilityBinding, CredibilityRequirements, MemberPolicy,
};
use fa_reference::action::consequence::congress::credibility::{
    Campaign, CaseSpec, CredibilityLedger, CredibilitySnapshot, EvaluationLabel, EvaluationScope,
    HelperGeneration, LabelSource, LabelVerdict, Observation,
};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::{
    ControllerConfig, PolicyAuthority, PolicyReview, PolicySession,
};
use fa_reference::action::consequence::gate::containment::session::policy::controller::credibility::{CredibilityActivation, CredibilityWithdrawalRequest};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

fn profile() -> RestartProfile {
    RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 1,
        tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::ExactRestart }
}
fn actor(profile: RestartProfile) -> ActorState {
    ActorState::new(profile, vec![1], vec![2], vec![3], 1).unwrap()
}
fn spec(epoch: u64) -> ActionSpec {
    ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1,
            expected_version: 1, generation: 1 }),
        payload: b"hello".to_vec(), required_witnesses: Vec::new(), policy_epoch: epoch,
        deadline: ElapsedTick(100), units: 5 }
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }
}
fn config() -> ControllerConfig {
    ControllerConfig { scope: spec(0).scope, total: 100, max_attempts: 128,
        actor: actor(profile()), suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::Absent { key: 7 }]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([
                ("alice".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 8 }),
                ("bob".to_owned(), MemberPolicy { cohort: "b".to_owned(), weight: 8 }),
            ]), caps: Caps { per_member: 10, per_cohort: 10 },
            continue_minimum: 16, continue_hold_maximum: 0, narrow_at: 16, suspend_at: 20,
            minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: TargetCeiling::new(&[spec(0).target.unwrap()]).unwrap() }
}
fn finish(mut session: PolicySession, bob: Option<Verdict>) -> PolicyReview {
    for (id, verdict) in [("alice", Verdict::Allow), ("bob", bob.unwrap_or(Verdict::Allow))] {
        let commitment = session.commitment(id, verdict, b"salt").unwrap();
        session.commit(id, commitment).unwrap();
    }
    session.open_reveals().unwrap();
    session.reveal("alice", Verdict::Allow, b"salt").unwrap();
    if let Some(verdict) = bob { session.reveal("bob", verdict, b"salt").unwrap(); }
    session.finish().unwrap()
}
fn evidence(campaign: u64, model: u64, label: LabelVerdict) -> CredibilitySnapshot {
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign, model_generation: model, evaluator_generation: 1,
            held_out_manifest: [9; 32] }, label_owner: "evaluator".to_owned(),
        helpers: BTreeMap::from([
            ("alice".to_owned(), HelperGeneration { generation: 1, cohort: "a".to_owned() }),
            ("bob".to_owned(), HelperGeneration { generation: 1, cohort: "b".to_owned() }),
        ]), strata: BTreeSet::from(["publication".to_owned()]),
        cases: (1..=2).map(|id| CaseSpec { id, stratum: "publication".to_owned(),
            evidence_root: [id as u8; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for id in 1..=2 {
        let observation = if id == 1 { Observation::Clear } else { Observation::Hold { first_sequence: 1 } };
        ledger.record_observations(id, BTreeMap::from([
            ("alice".to_owned(), observation), ("bob".to_owned(), observation),
        ])).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "evaluator".to_owned(),
            evaluator_generation: 1, source: LabelSource::IndependentEvaluation,
            evidence_root: [id as u8; 32], recorded_sequence: 2,
            verdict: if id == 1 { LabelVerdict::Safe } else { label } }).unwrap();
    }
    ledger.seal(2).unwrap()
}
fn activation(controller: &PolicyAuthority, operation: u64, generation: u64) -> CredibilityActivation {
    let snapshot = evidence(operation, controller.actor().profile().model_generation, LabelVerdict::Violation);
    CredibilityActivation { operation, expected_control_sequence: controller.inspect().sequence,
        expected_epoch: controller.inspect().ledger.epoch, scope: spec(0).scope,
        policy_generation: controller.policy().generation(), actor_profile: controller.actor().profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().to_owned(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(),
            reducer_generation: generation },
        stratum: "publication".to_owned(),
        requirements: CredibilityRequirements { minimum_safe_cases: 1, minimum_violation_cases: 1,
            minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 10, lead_bonus_weight: 0,
            lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}
fn prepare(broker: &mut DeliveryBroker, endpoint: &PublicationEndpoint, id: u64) -> FrozenAction {
    let mut action = spec(broker.inspect().ledger.epoch);
    action.target = Some(endpoint.target());
    broker.propose(id, action, &snapshot()).unwrap().action
}
fn review(broker: &mut DeliveryBroker, endpoint: &PublicationEndpoint, id: u64) -> FrozenAction {
    let action = prepare(broker, endpoint, id);
    let session = broker.begin_review(id, 100 + id, [1; 32], &snapshot()).unwrap();
    broker.apply_review(finish(session, Some(Verdict::Allow)), &snapshot()).unwrap();
    action
}
fn authorize(broker: &mut DeliveryBroker, endpoint: &PublicationEndpoint, id: u64) -> (FrozenAction, Permit) {
    let action = review(broker, endpoint, id);
    let permit = broker.authorize(id, &snapshot()).unwrap();
    (action, permit)
}
fn attached(endpoint: &mut PublicationEndpoint) -> DeliveryBroker {
    let mut broker = DeliveryBroker::new(config(), endpoint).unwrap();
    broker.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    for id in 1..=2 {
        review(&mut broker, endpoint, id);
        broker.cancel(id).unwrap();
    }
    broker
}
fn pair() -> (DeliveryBroker, PublicationEndpoint) {
    let mut endpoint = PublicationEndpoint::new(spec(0).target.unwrap(), Vec::new(), 200, 128).unwrap();
    let broker = attached(&mut endpoint);
    (broker, endpoint)
}
fn activate(broker: &mut DeliveryBroker, endpoint: &mut PublicationEndpoint) {
    let request = activation(broker.controller(), 1, 2);
    broker.activate_credibility(request).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
}
fn send(broker: &mut DeliveryBroker, endpoint: &PublicationEndpoint, id: u64) -> DispatchEnvelope {
    let (action, permit) = authorize(broker, endpoint, id);
    broker.dispatch(&permit, &action, &snapshot()).unwrap()
}

#[test]
fn qualified_weights_publish_only_after_the_new_endpoint_fence() {
    let (mut broker, mut endpoint) = pair();
    let old_ack = endpoint.install_fence(broker.fence_request()).unwrap();
    let request = activation(broker.controller(), 1, 2);
    let change = broker.activate_credibility(request).unwrap();
    assert_eq!(change.reducer_generation, 2);
    assert_eq!(broker.dispatcher_epoch(), 1);
    assert!(!broker.fence_confirmed());
    let (action, permit) = authorize(&mut broker, &endpoint, 10);
    assert_eq!(broker.dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.confirm_fence(old_ack), Err(Error::Stale));
    assert_eq!(broker.inspect().ledger.reserved, 5);
    assert_eq!(endpoint.execution_count(), 0);
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    let message = broker.dispatch(&permit, &action, &snapshot()).unwrap();
    let receipt = endpoint.deliver(&message).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    assert!(broker.accept_receipt(receipt.clone()).unwrap());
    assert!(!broker.accept_receipt(receipt).unwrap());
    assert_eq!(endpoint.payload(), b"hello");
    assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(broker.inspect().ledger.charged, 5);
}

#[test]
fn delayed_old_messages_are_fenced_without_refunding_unknown_outcomes() {
    for already_executed in [false, true] {
        let (mut broker, mut endpoint) = pair();
        let old = send(&mut broker, &endpoint, 10);
        if already_executed { endpoint.deliver(&old).unwrap(); }
        broker.acknowledgment_lost(10).unwrap();
        let request = activation(broker.controller(), 1, 2);
        broker.activate_credibility(request).unwrap();
        assert_eq!(broker.inspect().ledger.charged, 5);
        assert_eq!(broker.inspect().ledger.stages[&10], ActionState::Unknown);
        assert!(broker.cancel(10).is_err());
        broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
        assert_eq!(endpoint.deliver(&old).unwrap_err(), Error::Stale);
        let query = broker.status_query(10).unwrap();
        if !already_executed {
            assert_eq!(endpoint.status(&query).unwrap(), EndpointStatus::AwaitingResolution);
            assert_eq!(broker.inspect().ledger.available, 95);
        }
        let terminal = endpoint.seal_unexecuted(&query).unwrap();
        let expected = if already_executed { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } };
        assert_eq!(terminal.outcome(), expected);
        broker.accept_receipt(terminal).unwrap();
        assert_eq!(broker.inspect().ledger.charged, if already_executed { 5 } else { 0 });
        let fresh = send(&mut broker, &endpoint, 11);
        let receipt = endpoint.deliver(&fresh).unwrap();
        assert!(matches!(receipt.outcome(), EndpointOutcome::Executed { .. }));
        broker.accept_receipt(receipt).unwrap();
        assert_eq!(endpoint.execution_count(), if already_executed { 2 } else { 1 });
    }
}

#[test]
fn historical_activation_retries_cannot_roll_back_or_acknowledge_a_fence() {
    let (mut broker, mut endpoint) = pair();
    let first_request = activation(broker.controller(), 1, 2);
    let first = broker.activate_credibility(first_request.clone()).unwrap();
    let first_ack = endpoint.install_fence(broker.fence_request()).unwrap();
    broker.confirm_fence(first_ack.clone()).unwrap();
    let second = activation(broker.controller(), 2, 3);
    broker.activate_credibility(second).unwrap();
    let before = broker.inspect();
    assert_eq!(broker.activate_credibility(first_request.clone()).unwrap(), first);
    assert_eq!(broker.inspect(), before);
    assert_eq!(broker.dispatcher_epoch(), 2);
    assert!(!broker.fence_confirmed());
    assert_eq!(broker.confirm_fence(first_ack), Err(Error::Stale));
    let mut conflict = first_request;
    conflict.expected_epoch += 1;
    assert_eq!(broker.activate_credibility(conflict), Err(Error::Binding));
    assert_eq!(broker.dispatcher_epoch(), 2);
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    let message = send(&mut broker, &endpoint, 10);
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn expiry_blocks_a_reserved_send_but_preserves_actual_execution_receipts() {
    let (mut broker, mut endpoint) = pair();
    let mut request = activation(broker.controller(), 1, 2);
    request.requirements.maximum_evidence_age = 3; // valid through sequence 5
    broker.activate_credibility(request).unwrap(); // sequence 3
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    let message = send(&mut broker, &endpoint, 10); // sequence 4
    let receipt = endpoint.deliver(&message).unwrap();
    broker.acknowledgment_lost(10).unwrap();
    let (action, permit) = authorize(&mut broker, &endpoint, 11); // sequence 5
    assert_eq!(broker.inspect().sequence, 5);
    prepare(&mut broker, &endpoint, 12);
    let held = finish(broker.begin_review(12, 112, [1; 32], &snapshot()).unwrap(), None);
    broker.apply_review(held, &snapshot()).unwrap(); // actual control sequence 6
    let before = broker.inspect();
    assert_eq!(broker.dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.inspect(), before);
    assert_eq!(broker.inspect().ledger.reserved, 5);
    assert_eq!(endpoint.execution_count(), 1);
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.inspect().ledger.stages[&10], ActionState::Confirmed);
    broker.cancel(11).unwrap();
    assert_eq!(broker.inspect().ledger.available, 95);
    assert_eq!(broker.inspect().ledger.charged, 5);
}

#[test]
fn rejected_activation_changes_neither_authority_nor_dispatcher() {
    let (mut broker, mut endpoint) = pair();
    let mut request = activation(broker.controller(), 1, 2);
    request.snapshot = evidence(1, 1, LabelVerdict::Censored);
    let before = broker.inspect();
    assert_eq!(broker.activate_credibility(request), Err(Error::Incomplete));
    assert_eq!(broker.inspect(), before);
    assert_eq!(broker.dispatcher_epoch(), 0);
    assert!(broker.fence_confirmed());
    activate(&mut broker, &mut endpoint);
    let message = send(&mut broker, &endpoint, 10);
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn evidence_loss_blocks_old_keys_without_losing_endpoint_settlement() {
    let (mut broker, mut endpoint) = pair();
    activate(&mut broker, &mut endpoint);
    let message = send(&mut broker, &endpoint, 10);
    let receipt = endpoint.deliver(&message).unwrap();
    let (action, permit) = authorize(&mut broker, &endpoint, 11);
    let mut changed = profile();
    changed.tokenizer_generation += 1;
    assert_eq!(broker.replace_actor_state(broker.controller().actor_revision(), actor(changed)), Err(Error::Binding));
    let before = broker.inspect();
    broker.withdraw_credibility(CredibilityWithdrawalRequest {
        operation: 1, expected_control_sequence: before.sequence, expected_epoch: before.ledger.epoch,
    }).unwrap();
    assert!(!broker.fence_confirmed());
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    assert_eq!(broker.dispatch(&permit, &action, &snapshot()).unwrap_err(), Error::Stale);
    broker.accept_receipt(receipt).unwrap();
    broker.replace_actor_state(broker.controller().actor_revision(), actor(profile())).unwrap();
    assert_eq!(broker.controller().check_credibility(), Err(Error::Stale));
    let refresh = activation(broker.controller(), 2, 3);
    broker.activate_credibility(refresh).unwrap();
    assert_eq!(broker.inspect().ledger.reserved, 0);
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    let next = send(&mut broker, &endpoint, 12);
    broker.accept_receipt(endpoint.deliver(&next).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 2);
}

#[cfg(unix)]
#[test]
fn credible_publication_and_fence_are_written_to_the_original_file_endpoint() {
    use fa_reference::action::consequence::delivery::FilePublicationLimits;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let path = std::env::temp_dir().join(format!("fa-credible-delivery-{}-{}",
        std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let (mut endpoint, _recovery) = PublicationEndpoint::create_file_publication(
        &path, spec(0).target.unwrap(), Vec::new(), 200, 128,
        FilePublicationLimits { mutations: 128, bytes: 1_048_576 },
    ).unwrap();
    let directory = Directory(path);
    let mut broker = attached(&mut endpoint);
    let request = activation(broker.controller(), 1, 2);
    broker.activate_credibility(request.clone()).unwrap();
    broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
    let message = send(&mut broker, &endpoint, 10);
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    let published = PublicationEndpoint::read_file_publication(&directory.0).unwrap();
    assert_eq!(published.payload, b"hello");
    assert_eq!(published.execution_count, 1);
    assert_eq!(published.terminal_receipts, 1);
    assert_eq!(published.dispatcher_epoch, 1);
    broker.activate_credibility(request).unwrap();
    let repeated = PublicationEndpoint::read_file_publication(&directory.0).unwrap();
    assert_eq!(repeated, published);
    drop(endpoint);
    drop(directory);
}

#[test]
fn withdrawal_fences_late_messages_and_historical_retry_preserves_fresh_activation() {
    for execution_before_fence in [false, true] {
        let (mut broker, mut endpoint) = pair();
        activate(&mut broker, &mut endpoint);
        let old = send(&mut broker, &endpoint, 10);
        broker.acknowledgment_lost(10).unwrap();
        let before = broker.inspect();
        let request = CredibilityWithdrawalRequest {
            operation: 9, expected_control_sequence: before.sequence, expected_epoch: before.ledger.epoch,
        };
        let receipt = broker.withdraw_credibility(request.clone()).unwrap();
        assert_eq!(broker.dispatcher_epoch(), 2);
        assert!(!broker.fence_confirmed());
        assert_eq!(broker.inspect().ledger.charged, 5);
        // The endpoint has not yet installed the fence: explicitly test the
        // nonclaim that local withdrawal alone stops an already sent message.
        if execution_before_fence { endpoint.deliver(&old).unwrap(); }
        broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
        assert_eq!(endpoint.deliver(&old).unwrap_err(), Error::Stale);
        let query = broker.status_query(10).unwrap();
        let terminal = endpoint.seal_unexecuted(&query).unwrap();
        assert_eq!(terminal.outcome(), if execution_before_fence {
            EndpointOutcome::Executed { resulting_version: 2 }
        } else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } });
        broker.accept_receipt(terminal).unwrap();
        let refresh = activation(broker.controller(), 2, 3);
        broker.activate_credibility(refresh).unwrap();
        let before = broker.inspect();
        assert_eq!(broker.withdraw_credibility(request).unwrap(), receipt);
        assert_eq!(broker.inspect(), before);
        assert_eq!(broker.dispatcher_epoch(), 3);
        assert!(!broker.fence_confirmed());
        broker.confirm_fence(endpoint.install_fence(broker.fence_request()).unwrap()).unwrap();
        let fresh = send(&mut broker, &endpoint, 11);
        broker.accept_receipt(endpoint.deliver(&fresh).unwrap()).unwrap();
        assert_eq!(endpoint.execution_count(), if execution_before_fence { 2 } else { 1 });
    }
}
