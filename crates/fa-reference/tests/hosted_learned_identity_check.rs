//! Real original anchor inference and liveness authority; tiny fixed intervals
//! are causal controls, not calibrated identity or cryptographic attestation.
#![forbid(unsafe_code)]
#[path = "support/learned_identity_model.rs"]
mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit,
    Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::activation::identity::{ModelManifest, ModelPassport};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget,
    DecoderModel, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{TargetCeiling, containment::{ActorState, RestartGrade,
    RestartProfile, session::policy::{Policy, Predicate, controller::ControllerConfig}}};
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    OversightBroker, ReviewWindow, action_frame, decoder_monitoring::LearnedDecoderBindingLimits,
    human::{HumanReviewer, HumanReviewPolicy}, identity::{IdentityObserver, IdentityOutcome,
        IdentityPolicy, IdentityStatus}, learned_host::identity::{HostedIdentityCheckRequest,
        HostedIdentityCheckStatus as Status, HostedLearnedIdentityCheck}};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::BTreeMap;

fn scope() -> Scope { Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
fn target() -> ResolvedTarget { ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 } }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn owner(model: DecoderModel, passport: ModelPassport, alarm: bool)
    -> (OversightBroker, PublicationEndpoint, IdentityObserver, HumanReviewer)
{
    let mut endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap();
    let contracts = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"owned-identity-test".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap())])).unwrap();
    let mut host = OversightBroker::new(ControllerConfig {
        scope: scope(), total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 3,
            tokenizer_generation: 4, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1 }, narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    }, &mut endpoint, contracts).unwrap();
    let observer = host.enable_identity_checks(passport, IdentityPolicy {
        observer_id: 77, timeout_ticks: 10, validity_ticks: 50, max_checks: 8,
    }).unwrap();
    let human = host.enable_human_review(HumanReviewPolicy {
        reviewer_id: 88, max_validity_ticks: 100, max_requests: 8,
    }).unwrap();
    let source = fixture::source(&model, alarm);
    host.own_learned_generation(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
    host.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    (host, endpoint, observer, human)
}
fn request(host: &OversightBroker, id: u64, manifest: ModelManifest) -> HostedIdentityCheckRequest {
    HostedIdentityCheckRequest { check: id, expected_control_sequence: host.inspect().sequence,
        expected_actor_revision: host.actor_revision(), observed_manifest: manifest,
        measurement_sequence: id, budget: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
}
fn advance(host: &mut OversightBroker, run: &mut HostedLearnedIdentityCheck) -> Result<Status, Error> {
    run.advance(host, run.revision(), ElapsedTick(1))
}
fn measure(host: &mut OversightBroker, run: &mut HostedLearnedIdentityCheck) {
    let bound = run.work().planned_tokens;
    for _ in 0..bound {
        if run.status() != Status::Measuring { break; }
        advance(host, run).unwrap();
    }
    assert_eq!(run.status(), Status::ReadyToApply);
}
fn match_identity(host: &mut OversightBroker, observer: IdentityObserver, manifest: ModelManifest) -> IdentityObserver {
    let req = request(host, 1, manifest);
    let mut run = host.begin_hosted_learned_identity_check(observer, req).unwrap();
    measure(host, &mut run); assert_eq!(advance(host, &mut run).unwrap(), Status::Installed);
    run.take_observer().unwrap()
}
fn step(host: &mut OversightBroker) {
    let n = host.hosted_learned_generation().unwrap();
    host.advance_hosted_learned(n.actor_revision, n.position).unwrap();
}
fn prepared(host: &mut OversightBroker) -> (FrozenAction, CommitteeInput, Permit) {
    let action = host.propose(1, ActionSpec { version: VERSION, scope: scope(), target: Some(target()),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }, &snapshot()).unwrap().action;
    let helper = &host.contracts().members()["reviewer"];
    let mut bytes = action_frame(&action); let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection { projection_id: helper.projection_id(),
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let input = CommitteeInput::capture(&action, host.contracts(), BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    host.record_inputs(1, host.input_revision(1).unwrap(), input.clone()).unwrap();
    let mut session = host.begin_review(1, 101, [7; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
    }, &snapshot()).unwrap();
    let digest = session.commitment("reviewer", Verdict::Allow, b"salt").unwrap();
    session.commit("reviewer", digest, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("reviewer", Verdict::Allow, b"salt", ElapsedTick(1)).unwrap();
    host.apply_review(session.finish(ElapsedTick(1)).unwrap(), Some(&input), &snapshot()).unwrap();
    let permit = host.authorize(1, Some(&input), &snapshot()).unwrap();
    (action, input, permit)
}

#[test]
fn actual_complete_roster_yields_before_installation_then_original_two_keys_publish() {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let (mut host, mut endpoint, observer, human) = owner(model, passport.clone(), false);
    let before = host.hosted_learned_generation().unwrap();
    let req = request(&host, 1, passport.manifest().clone());
    let mut run = host.begin_hosted_learned_identity_check(observer, req).unwrap();
    assert_eq!(run.work().entered_tokens, 0); assert_eq!(run.work().planned_tokens, 4);
    for completed in 1..=4 {
        advance(&mut host, &mut run).unwrap();
        assert_eq!(run.work().completed_tokens, completed);
        assert!(run.installation().is_none());
        assert_eq!(host.hosted_learned_generation().unwrap(), before);
        assert_eq!(endpoint.execution_count(), 0);
    }
    assert_eq!(run.measurements().len(), 2);
    assert_eq!(run.report().outcome, IdentityOutcome::Matched);
    assert_eq!(run.status(), Status::ReadyToApply);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(host.authorize(1, None, &snapshot()).err(), Some(Error::Incomplete));
    assert_eq!(advance(&mut host, &mut run).unwrap(), Status::Installed);
    assert_eq!(run.work().completed_tokens, 4);
    assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Matching { check: 1, .. }));
    assert!(!host.inspect().suspended);
    step(&mut host);
    let (action, input, permit) = prepared(&mut host);
    assert_eq!(host.dispatch(&permit, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    let approval = host.request_human_approval(500, 1, Some(&input), ElapsedTick(40)).unwrap();
    let key = human.approve(&approval, ElapsedTick(1)).unwrap();
    let envelope = host.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).unwrap();
    host.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"visible"); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(host.inspect().ledger.charged, 16);
    let _observer = run.take_observer().unwrap();
    assert_eq!(run.take_observer().err(), Some(Error::Missing));
    assert_eq!(advance(&mut host, &mut run).err(), Some(Error::WrongState));
}

#[test]
fn byte_distinct_owned_model_latches_real_anchor_mismatch_then_original_fence() {
    let reference = fixture::model(1.0); let actual = fixture::model(2.0);
    assert_eq!(reference.profile(), actual.profile());
    let passport = fixture::passport(&reference);
    let (mut host, endpoint, observer, _) = owner(actual, passport.clone(), false);
    let before = host.hosted_learned_generation().unwrap();
    let req = request(&host, 1, passport.manifest().clone());
    let mut run = host.begin_hosted_learned_identity_check(observer, req).unwrap();
    measure(&mut host, &mut run);
    assert_eq!(run.work().completed_tokens, 2);
    assert_eq!(run.work().measured_anchors, 1);
    assert!(matches!(run.report().outcome, IdentityOutcome::Mismatch(_)));
    assert_eq!(run.report().observations[&10].first_outlier().unwrap().observed_bits, 2.0_f32.to_bits());
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Mismatch { check: 1 });
    assert_eq!(host.authorize(1, None, &snapshot()).err(), Some(Error::Binding));
    assert!(!host.inspect().suspended);
    advance(&mut host, &mut run).unwrap();
    assert!(host.inspect().suspended);
    assert_eq!(run.installation().unwrap().report, *run.report());
    assert_eq!(host.hosted_learned_generation().unwrap(), before);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn manifest_mismatch_refunds_only_unsent_work_and_preserves_unknown_execution() {
    for sent in [false, true] {
        let model = fixture::model(1.0); let passport = fixture::passport(&model);
        let (mut host, mut endpoint, observer, human) = owner(model, passport.clone(), false);
        let observer = match_identity(&mut host, observer, passport.manifest().clone());
        step(&mut host);
        let (action, input, permit) = prepared(&mut host);
        let request_key = host.request_human_approval(500, 1, Some(&input), ElapsedTick(40)).unwrap();
        let key = human.approve(&request_key, ElapsedTick(1)).unwrap();
        let mut receipt = None;
        if sent {
            let envelope = host.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).unwrap();
            receipt = Some(endpoint.deliver(&envelope).unwrap()); host.acknowledgment_lost(1).unwrap();
        }
        let mut changed = passport.manifest().clone(); changed.weights[0] ^= 1;
        let req = request(&host, 2, changed);
        let mut run = host.begin_hosted_learned_identity_check(observer, req).unwrap();
        assert_eq!(run.status(), Status::ReadyToApply); assert_eq!(run.work().entered_tokens, 0);
        advance(&mut host, &mut run).unwrap();
        assert_eq!(run.installation().unwrap().refunded_units, if sent { 0 } else { 16 });
        assert_eq!(host.inspect().ledger.charged, if sent { 16 } else { 0 });
        assert_eq!(host.inspect().ledger.stages[&1], if sent { ActionState::Unknown } else { ActionState::Cancelled });
        assert!(host.dispatch_with_human(&permit, &key, &action, Some(&input), &snapshot()).is_err());
        if let Some(receipt) = receipt { host.accept_receipt(receipt).unwrap(); }
        assert_eq!(endpoint.execution_count(), u64::from(sent));
        assert_eq!(host.inspect().ledger.charged, if sent { 16 } else { 0 });
    }
}

#[test]
fn cancellation_keeps_partial_measurements_and_reuses_only_the_original_observer() {
    for tokens in [1, 2, 4] {
        let model = fixture::model(1.0); let passport = fixture::passport(&model);
        let (mut host, endpoint, observer, _) = owner(model, passport.clone(), false);
        let req = request(&host, 1, passport.manifest().clone());
        let mut run = host.begin_hosted_learned_identity_check(observer, req).unwrap();
        assert_eq!(run.take_observer().err(), Some(Error::WrongState));
        for _ in 0..tokens { advance(&mut host, &mut run).unwrap(); }
        let work = run.work(); let measurements = run.measurements().len();
        run.cancel(&mut host, run.revision()).unwrap();
        assert_eq!(run.status(), Status::Cancelled); assert_eq!(run.work(), work);
        assert_eq!(run.measurements().len(), measurements); assert!(run.installation().is_none());
        assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
        let observer = run.take_observer().unwrap();
        let req = request(&host, 2, passport.manifest().clone());
        let mut retry = host.begin_hosted_learned_identity_check(observer, req).unwrap();
        measure(&mut host, &mut retry); advance(&mut host, &mut retry).unwrap();
        assert_eq!(retry.work().completed_tokens, 4); assert_eq!(run.work(), work);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn stale_revisions_and_foreign_owners_are_free_but_expiry_and_source_loss_close_the_check() {
    for changed_source in [false, true] {
        let model = fixture::model(1.0); let passport = fixture::passport(&model);
        let (mut host, endpoint, observer, _) = owner(model.clone(), passport.clone(), false);
        let (mut foreign, _, _, _) = owner(model, passport.clone(), false);
        let req = request(&host, 1, passport.manifest().clone());
        let mut run = host.begin_hosted_learned_identity_check(observer, req).unwrap();
        assert_eq!(run.advance(&mut host, 1, ElapsedTick(1)).err(), Some(Error::Stale));
        assert_eq!(run.advance(&mut foreign, 0, ElapsedTick(1)).err(), Some(Error::Binding));
        assert_eq!(run.advance(&mut host, 0, ElapsedTick(0)).err(), Some(Error::Stale));
        assert_eq!(run.revision(), 0); assert_eq!(run.work().entered_tokens, 0);
        advance(&mut host, &mut run).unwrap(); let work = run.work();
        let now = if changed_source { step(&mut host); ElapsedTick(1) } else { run.challenge().deadline() };
        assert_eq!(run.advance(&mut host, run.revision(), now).err(), Some(Error::Stale));
        assert_eq!(run.status(), Status::Failed); assert_eq!(run.work(), work);
        assert!(run.installation().is_none()); assert!(run.cleanup_failure().is_none());
        assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn setup_failure_returns_role_without_restoring_old_live_identity_or_spending_tokens() {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let (mut host, endpoint, observer, _) = owner(model.clone(), passport.clone(), false);
    let (foreign, _, foreign_observer, _) = owner(model, passport.clone(), false);
    let foreign_before = foreign.inspect();
    let req = request(&host, 1, passport.manifest().clone());
    let failure = host.begin_hosted_learned_identity_check(foreign_observer, req).unwrap_err();
    assert_eq!(failure.error, Error::Binding); assert!(failure.cleanup_failure.is_none());
    assert_eq!(foreign.inspect(), foreign_before);
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
    let req = request(&host, 2, passport.manifest().clone());
    let mut run = host.begin_hosted_learned_identity_check(observer, req).unwrap();
    measure(&mut host, &mut run); advance(&mut host, &mut run).unwrap();
    let observer = run.take_observer().unwrap(); let numerical = host.hosted_learned_generation().unwrap();
    let mut req = request(&host, 3, passport.manifest().clone()); req.budget.scalar_products = 1;
    let failure = host.begin_hosted_learned_identity_check(observer, req).unwrap_err();
    assert_eq!(failure.error, Error::Limit); assert!(failure.challenge.is_some());
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(host.identity_report(3).unwrap().outcome, IdentityOutcome::Unavailable);
    assert_eq!(host.hosted_learned_generation().unwrap(), numerical);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn reused_measurement_sequence_cannot_refresh_identity_and_matching_does_not_clear_a_hold() {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let (mut host, endpoint, observer, _) = owner(model, passport.clone(), true);
    let observer = match_identity(&mut host, observer, passport.manifest().clone());
    let mut req = request(&host, 2, passport.manifest().clone()); req.measurement_sequence = 1;
    let mut stale = host.begin_hosted_learned_identity_check(observer, req).unwrap();
    advance(&mut host, &mut stale).unwrap();
    assert_eq!(advance(&mut host, &mut stale).err(), Some(Error::Stale));
    assert_eq!(stale.work().completed_tokens, 2); assert_eq!(stale.measurements().len(), 1);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    let observer = stale.take_observer().unwrap();
    step(&mut host); step(&mut host);
    let held = host.hosted_learned_generation().unwrap();
    assert!(!held.status.is_active());
    let req = request(&host, 3, passport.manifest().clone());
    let mut diagnostic = host.begin_hosted_learned_identity_check(observer, req).unwrap();
    measure(&mut host, &mut diagnostic); advance(&mut host, &mut diagnostic).unwrap();
    assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Matching { check: 3, .. }));
    assert_eq!(host.hosted_learned_generation().unwrap(), held);
    assert!(host.advance_hosted_learned(held.actor_revision, held.position).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}
