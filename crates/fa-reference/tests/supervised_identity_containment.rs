//! Original learned fingerprints, real helper sockets and actual endpoint effects.
//! Synthetic tiny-model intervals and supplied review ballots are causal controls.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/learned_identity_model.rs"]
mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::activation::identity::ModelManifest;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, EndpointStatus, PublicationEndpoint};
use fa_reference::action::consequence::gate::{TargetCeiling, containment::{ActorState, RestartGrade,
    RestartProfile, session::policy::{Policy, Predicate, controller::ControllerConfig}}};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, DispatchKeys,
    HelperContract, ReviewWindow, action_frame, actor::{ActorError, ActorOutcome, ActorPort, ActorProposal,
        ActorTicket, IntakeLimits, Knowledge}, decoder_monitoring::LearnedDecoderBindingLimits,
    helper_client::{ClientProgress, HelperClient}, helper_workers::HelperLimits,
    human::{HumanReviewPolicy, HumanReviewer}, identity::{IdentityObserver, IdentityPolicy, IdentityStatus},
    learned_host::identity::{HostedIdentityCheckRequest, HostedIdentityCheckStatus as Status, HostedLearnedIdentityCheck},
    supervised::{DriverError, DriverEvent, DriverPhase, SupervisedDriver}};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::io::{Read, ErrorKind};
use std::os::unix::net::UnixStream;

fn target() -> ResolvedTarget { ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 } }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn proposal() -> ActorProposal { ActorProposal { target: target(), payload: b"visible".to_vec(), units: 16,
    deadline: ElapsedTick(100), expected_policy_epoch: 0 } }
fn owner() -> (ActorPort, SupervisedDriver, IdentityObserver, HumanReviewer, ModelManifest) {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let manifest = passport.manifest().clone();
    let contracts = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"owned-identity-test".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap())])).unwrap();
    let config = ControllerConfig {
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 3,
            tokenizer_generation: 4, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1 }, narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    };
    let endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap();
    let (port, mut driver) = SupervisedDriver::new(config, endpoint, contracts, IntakeLimits::default()).unwrap();
    let host = driver.supervisor_mut().broker_mut();
    let observer = host.enable_identity_checks(passport, IdentityPolicy {
        observer_id: 77, timeout_ticks: 10, validity_ticks: 50, max_checks: 8,
    }).unwrap();
    let human = host.enable_human_review(HumanReviewPolicy { reviewer_id: 88, max_validity_ticks: 100, max_requests: 8 }).unwrap();
    let source = fixture::source(&model, false);
    host.own_learned_generation(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    driver.observe_time(ElapsedTick(1)).unwrap(); driver.confirm_dispatcher_fence().unwrap();
    (port, driver, observer, human, manifest)
}
fn begin(driver: &mut SupervisedDriver, observer: IdentityObserver, check: u64, manifest: ModelManifest)
    -> HostedLearnedIdentityCheck
{
    let host = driver.supervisor_mut().broker_mut();
    let request = HostedIdentityCheckRequest { check, expected_control_sequence: host.inspect().sequence,
        expected_actor_revision: host.actor_revision(), observed_manifest: manifest, measurement_sequence: check,
        budget: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } };
    host.begin_hosted_learned_identity_check(observer, request).unwrap()
}
fn initial_match(driver: &mut SupervisedDriver, observer: IdentityObserver, manifest: ModelManifest) -> IdentityObserver {
    let mut check = begin(driver, observer, 1, manifest);
    for _ in 0..5 {
        let revision = check.revision();
        let result = driver.advance_hosted_learned_identity(&mut check, revision, || ElapsedTick(1));
        result.observation.unwrap(); result.synchronization.unwrap(); assert!(result.containment.is_none());
    }
    assert_eq!(check.status(), Status::Installed); assert_eq!(check.work().completed_tokens, 4);
    let n = driver.supervisor().broker().hosted_learned_generation().unwrap();
    let step = driver.advance_hosted_learned(n.actor_revision, n.position, || ElapsedTick(1));
    step.inference.unwrap(); step.synchronization.unwrap(); assert!(step.containment.is_none());
    check.take_observer().unwrap()
}
fn start(driver: &mut SupervisedDriver, port: &ActorPort)
    -> (ActorTicket, CommitteeInput, HelperClient<UnixStream>, UnixStream)
{
    let ticket = port.submit(1, &proposal()).unwrap();
    assert!(driver.accept_next(&snapshot()).unwrap().unwrap().result.unwrap().is_some());
    let supervisor = driver.supervisor(); let action = supervisor.action(1).unwrap();
    let contracts = supervisor.broker().contracts(); let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(action); let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: helper.projection_id(),
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let input = CommitteeInput::capture(action, contracts, BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    let (server, peer) = UnixStream::pair().unwrap(); let inspect = peer.try_clone().unwrap();
    let client = HelperClient::from_unix(peer, helper.profile_at(action.spec().policy_epoch)).unwrap();
    let revision = supervisor.broker().input_revision(supervisor.attempt(1).unwrap()).unwrap();
    driver.start_review(fa_reference::action::consequence::oversight::supervised::ReviewLaunch {
        request: 1, round: 101, evidence_root: [7; 32],
        window: ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) },
        expected_input_revision: revision, inputs: input.clone(),
        streams: BTreeMap::from([("reviewer".to_owned(), server)]), limits: HelperLimits::default(),
    }, &snapshot()).unwrap();
    (ticket, input, client, inspect)
}
fn review(driver: &mut SupervisedDriver, input: &CommitteeInput, client: &mut HelperClient<UnixStream>) {
    for _ in 0..128 {
        if client.step().unwrap() == ClientProgress::NeedsInference {
            assert_eq!(client.input().unwrap().actual_input(), input.views()["reviewer"].actual_input());
            client.respond(Verdict::Allow, b"salt").unwrap();
        }
        match driver.step(ElapsedTick(1), Some(input), &snapshot(), None).unwrap() {
            DriverEvent::Workers { .. } => {}, DriverEvent::ReviewApplied { .. } => return,
            other => panic!("unexpected original review event: {other:?}"),
        }
    }
    panic!("bounded original socket review did not complete");
}

#[test]
fn matching_identity_preserves_the_original_socket_review_and_two_key_publication() {
    let (port, mut driver, observer, human, manifest) = owner();
    let _observer = initial_match(&mut driver, observer, manifest);
    let (ticket, input, mut client, _) = start(&mut driver, &port);
    review(&mut driver, &input, &mut client);
    assert!(matches!(driver.step(ElapsedTick(1), Some(&input), &snapshot(), None).unwrap(), DriverEvent::AwaitingHuman { .. }));
    let request = driver.request_human_approval(500, Some(&input), ElapsedTick(40)).unwrap();
    let key = human.approve(&request, ElapsedTick(1)).unwrap();
    assert!(matches!(driver.step(ElapsedTick(1), Some(&input), &snapshot(), Some(&key)).unwrap(), DriverEvent::PublicationResolved { .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(driver.endpoint().execution_count(), 1); assert_eq!(driver.endpoint().payload(), b"visible");
    assert_eq!(driver.supervisor().broker().inspect().ledger.charged, 16);
}

#[test]
fn unapplied_mismatch_stops_helper_io_then_installed_fence_closes_original_actor_tickets() {
    let (port, mut driver, observer, _, mut manifest) = owner();
    let observer = initial_match(&mut driver, observer, manifest.clone());
    let (ticket, _, _client, mut wire) = start(&mut driver, &port);
    let queued = port.submit(2, &proposal()).unwrap();
    manifest.weights[0] ^= 1;
    let mut check = begin(&mut driver, observer, 2, manifest);
    assert_eq!(check.status(), Status::ReadyToApply);
    let mut missing = snapshot(); missing.complete = false;
    assert_eq!(driver.step(ElapsedTick(1), None, &missing, None).unwrap_err(), DriverError::Control(Error::Incomplete));
    assert_eq!(wire.read(&mut [0; 1]).unwrap_err().kind(), ErrorKind::WouldBlock);
    assert_eq!(driver.phase(), DriverPhase::Reviewing { request: 1 });
    assert_eq!(driver.accept_next(&snapshot()).err(), Some(Error::Incomplete));
    assert!(matches!(port.poll(&queued), Knowledge::Pending { .. }));
    let result = driver.advance_hosted_learned_identity(&mut check, 0, || ElapsedTick(1));
    assert_eq!(result.observation.unwrap(), Status::Installed); result.synchronization.unwrap();
    assert!(result.containment.unwrap().unwrap().outcomes.is_empty());
    assert_eq!(driver.phase(), DriverPhase::Idle);
    for ticket in [&ticket, &queued] {
        assert!(matches!(port.poll(ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    }
    assert_eq!(port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
    assert_eq!(port.submit(1, &proposal()).unwrap().request(), 1);
    assert_eq!(port.submit(2, &proposal()).unwrap().request(), 2);
    assert_eq!(driver.endpoint().execution_count(), 0);
    assert!(driver.supervisor().broker().stop_receipt().is_none(), "do not invent a numerical stop");
}

#[test]
fn post_installation_clock_failure_preserves_unknown_charge_until_real_endpoint_resolution() {
    for executes_in_gap in [false, true] {
        let (port, mut driver, observer, human, mut manifest) = owner();
        let observer = initial_match(&mut driver, observer, manifest.clone());
        let (ticket, input, mut client, _) = start(&mut driver, &port);
        review(&mut driver, &input, &mut client);
        let automatic = driver.supervisor_mut().authorize_request(1, Some(&input), &snapshot()).unwrap();
        let request = driver.request_human_approval(500, Some(&input), ElapsedTick(40)).unwrap();
        let key = human.approve(&request, ElapsedTick(1)).unwrap();
        let envelope = driver.supervisor_mut().dispatch_request(1, DispatchKeys { automatic: &automatic, human: Some(&key) },
            Some(&input), &snapshot()).unwrap();
        driver.supervisor_mut().acknowledgment_lost(1).unwrap();
        let queued = port.submit(2, &proposal()).unwrap();
        let numerical = driver.supervisor().broker().hosted_learned_generation().unwrap();
        manifest.weights[0] ^= 1;
        let mut check = begin(&mut driver, observer, 2, manifest);
        let mut calls = 0;
        let result = driver.advance_hosted_learned_identity(&mut check, 0, || {
            calls += 1; ElapsedTick(if calls == 1 { 1 } else { 0 })
        });
        assert_eq!(calls, 2); assert_eq!(result.observation.unwrap(), Status::Installed);
        result.synchronization.unwrap(); assert_eq!(result.containment.unwrap().err(), Some(Error::Stale));
        assert_eq!(driver.phase(), DriverPhase::Idle);
        assert_eq!(driver.supervisor().broker().inspect().ledger.charged, 16);
        assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
        assert!(matches!(port.poll(&queued), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        assert_eq!(port.submit(3, &proposal()).unwrap_err(), ActorError::Unavailable);
        // This is an actual external gap, not an inferred nonexecution outcome.
        if executes_in_gap { let _lost = driver.endpoint_mut().deliver(&envelope).unwrap(); }
        let installed = driver.supervisor().broker().identity_installation(2).unwrap();
        let mut unavailable = snapshot(); unavailable.complete = false;
        let event = driver.step(ElapsedTick(2), None, &unavailable, None).unwrap();
        let DriverEvent::IdentityFence { sweep } = event else { panic!("identity fence did not precede missing evidence"); };
        assert_eq!(sweep.check, 2); assert_eq!(sweep.outcomes.len(), 1);
        assert_eq!(driver.endpoint_mut().deliver(&envelope).err(), Some(Error::Stale));
        if executes_in_gap {
            let Ok(EndpointStatus::Resolved(receipt)) = &sweep.outcomes[&1] else { panic!("executed outcome missing"); };
            assert!(matches!(receipt.outcome(), EndpointOutcome::Executed { .. }));
            assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        } else {
            assert_eq!(sweep.outcomes[&1], Ok(EndpointStatus::AwaitingResolution));
            assert_eq!(driver.supervisor().broker().inspect().ledger.charged, 16);
            assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
            let deadline = envelope.request().execution_deadline();
            let event = driver.step(deadline, None, &unavailable, None).unwrap();
            let DriverEvent::IdentityFence { sweep } = event else { panic!("original expiry did not run"); };
            let Ok(EndpointStatus::Resolved(receipt)) = &sweep.outcomes[&1] else { panic!("terminal nonexecution missing"); };
            assert!(matches!(receipt.outcome(), EndpointOutcome::NotExecuted { .. }));
            assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
        }
        assert_eq!(driver.supervisor().broker().inspect().ledger.charged, if executes_in_gap { 16 } else { 0 });
        assert_eq!(driver.endpoint().execution_count(), u64::from(executes_in_gap));
        assert_eq!(driver.supervisor().broker().identity_installation(2).unwrap(), installed);
        assert_eq!(driver.supervisor().broker().hosted_learned_generation().unwrap(), numerical);
        assert_eq!(driver.supervisor().broker().identity_status().unwrap(), IdentityStatus::Mismatch { check: 2 });
        assert_eq!(check.work().entered_tokens, 0);
        // Reconnection moves the same restart marker. Neither it nor repeated
        // containment should turn this installed identity into another restart.
        let settled_at = if executes_in_gap { ElapsedTick(2) } else { envelope.request().execution_deadline() };
        let (offline, endpoint) = driver.detach_endpoint();
        let mut driver = offline.reconnect(endpoint, settled_at).unwrap();
        let fence = driver.supervisor().broker().fence_request();
        let acknowledgement = driver.endpoint_mut().install_fence(fence).unwrap();
        assert!(driver.service_identity_containment(|| settled_at).unwrap().unwrap().outcomes.is_empty());
        driver.supervisor_mut().broker_mut().confirm_fence(acknowledgement).unwrap();
        assert_eq!(driver.supervisor().broker().identity_installation(2).unwrap(), installed);
    }
}
