//! Actual original learned inference and congress; ballots are synthetic controls.
#[path = "support/restart_model.rs"]
pub mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedReview, OversightBroker, ReviewWindow, action_frame,
    decoder_monitoring::LearnedDecoderBindingLimits,
    human::{HumanReviewer, HumanReviewPolicy},
    learned_host::sidecar::{LearnedSidecar, LearnedSidecarRequest},
    learned_source::LearnedSourceConfig,
    sidecar::{SidecarCongressBudget, SidecarCongressPlan, SidecarIdentity},
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationSpec, GenerationTelemetryBudget,
};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};

fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn empty_owner(human: bool) -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>) {
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
        grade: RestartGrade::AuditOnly }, Vec::new(), vec![0], vec![0], 0).unwrap();
    let contract = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1, b"approve?".to_vec()).unwrap(),
    )])).unwrap();
    let mut endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap();
    let mut owner = OversightBroker::new(ControllerConfig {
        scope: scope(), total: 100, max_attempts: 8, actor, suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        }, narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    }, &mut endpoint, contract).unwrap();
    let reviewer = if human { Some(owner.enable_human_review(HumanReviewPolicy {
        reviewer_id: 55, max_validity_ticks: 100, max_requests: 8,
    }).unwrap()) } else { None };
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    (owner, endpoint, reviewer)
}
fn owner(mode: u8, required: bool, human: bool) -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>) {
    let (mut owner, endpoint, reviewer) = empty_owner(human);
    let model = fixture::model();
    let config = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: fixture::policy(&model, mode, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
    owner.own_learned_generation(model, config, LearnedDecoderBindingLimits::default()).unwrap();
    if required { owner.enable_learned_sidecar_requirement().unwrap(); }
    (owner, endpoint, reviewer)
}
fn step(owner: &mut OversightBroker) {
    let current = owner.hosted_learned_generation().unwrap();
    owner.advance_hosted_learned(current.actor_revision, current.position).unwrap();
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn propose(owner: &mut OversightBroker, id: u64) -> FrozenAction {
    let spec = ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"visible".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch, deadline: ElapsedTick(100), units: 16 };
    owner.propose(id, spec, &snapshot()).unwrap().action
}
fn request(owner: &OversightBroker, attempt: u64) -> LearnedSidecarRequest {
    let evidence = owner.learned_decoder_evidence(attempt).unwrap().unwrap();
    LearnedSidecarRequest { identity: SidecarIdentity { object_id: attempt + 1000, generation: 1, transform_id: 7 },
        priority: evidence.audit().source().groups().collect(), budget: SidecarCongressBudget::default() }
}
fn begin(owner: &mut OversightBroker, attempt: u64) -> LearnedSidecar {
    let request = request(owner, attempt);
    owner.begin_learned_sidecar(attempt, owner.actor_revision(), request).unwrap()
}
fn window() -> ReviewWindow { ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) } }
fn ballot(mut session: fa_reference::action::consequence::oversight::ObservedSession, verdict: Verdict) -> ObservedReview {
    let salt = [11; 32];
    let commitment = session.commitment("reviewer", verdict, &salt).unwrap();
    session.commit("reviewer", commitment, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("reviewer", verdict, &salt, ElapsedTick(1)).unwrap();
    session.finish(ElapsedTick(1)).unwrap()
}
fn review(owner: &mut OversightBroker, sidecar: &LearnedSidecar, round: u64, verdict: Verdict) -> ObservedReview {
    ballot(owner.begin_learned_sidecar_review(sidecar, round, [7; 32], window(), &snapshot()).unwrap(), verdict)
}
fn text_only(owner: &OversightBroker, action: &FrozenAction) -> CommitteeInput {
    let helper = &owner.contracts().members()["reviewer"];
    let mut bytes = action_frame(action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection { projection_id: helper.projection_id(),
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    CommitteeInput::capture(action, owner.contracts(), BTreeMap::from([("reviewer".to_owned(), view)])).unwrap()
}

#[test]
fn original_checked_source_reaches_every_helper_and_two_keys_still_decide_publication() {
    let (mut owner, mut endpoint, human) = owner(0, true, true); step(&mut owner);
    let action = propose(&mut owner, 1);
    let numerical = owner.hosted_learned_generation().unwrap();
    let sidecar = begin(&mut owner, 1);
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
    assert_eq!(sidecar.evidence().tokens(), &[0]);
    assert_eq!(sidecar.evidence().evaluation_origin(), 201);
    assert_eq!(sidecar.source().encode().unwrap(), sidecar.evidence().audit().source().encode().unwrap());
    let base = sidecar.source().encode_base().unwrap();
    assert_eq!(&sidecar.round().payload()[24..], base.as_slice());
    assert!(sidecar.round().selected_groups().is_empty());
    let input = owner.current_learned_sidecar(&sidecar).unwrap().clone();
    for view in input.views().values() {
        assert_eq!(view.actual_input().part_bytes(1).unwrap(), sidecar.round().payload());
    }
    assert_eq!(sidecar.round().work().committee_bytes, input.logical_bytes());
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
    let reviewed = review(&mut owner, &sidecar, 101, Verdict::Allow);
    owner.apply_review(reviewed, Some(&input), &snapshot()).unwrap();
    let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&permit, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    let human_request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    let human = human.unwrap().approve(&human_request, ElapsedTick(1)).unwrap();
    let envelope = owner.dispatch_with_human(&permit, &human, &action, Some(&input), &snapshot()).unwrap();
    let receipt = endpoint.deliver(&envelope).unwrap(); owner.accept_receipt(receipt).unwrap();
    assert_eq!(endpoint.execution_count(), 1); assert_eq!(endpoint.payload(), b"visible");
    assert_eq!(owner.inspect().ledger.charged, 16);
    assert!(owner.dispatch_with_human(&permit, &human, &action, Some(&input), &snapshot()).is_err());
}

#[test]
fn manual_text_or_even_byte_identical_sidecar_cannot_register_required_provenance() {
    for encoded in [false, true] {
        let (mut owner, endpoint, _) = owner(0, true, false); step(&mut owner);
        let action = propose(&mut owner, 1);
        let input = if encoded {
            let choice = request(&owner, 1);
            let source = owner.learned_decoder_evidence(1).unwrap().unwrap().audit().source().clone();
            let mut plan = SidecarCongressPlan::new(source, choice.identity, choice.priority, choice.budget).unwrap();
            plan.initial(&action, owner.contracts()).unwrap().input().clone()
        } else { text_only(&owner, &action) };
        owner.record_inputs(1, 0, input).unwrap();
        assert_eq!(owner.begin_review(1, 101, [7; 32], window(), &snapshot()).err(), Some(Error::Incomplete));
        let choice = request(&owner, 1);
        assert_eq!(owner.begin_learned_sidecar(1, owner.actor_revision(), choice).err(), Some(Error::WrongState));
        // Near-identical positive path: same numerical source, genuine registration.
        propose(&mut owner, 2); let sidecar = begin(&mut owner, 2);
        let input = sidecar.round().input().clone();
        let reviewed = review(&mut owner, &sidecar, 102, Verdict::Allow);
        owner.apply_review(reviewed, Some(&input), &snapshot()).unwrap();
        assert!(owner.authorize(2, Some(&input), &snapshot()).is_ok());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn replacing_required_input_invalidates_all_existing_permitting_boundaries() {
    for phase in 0..4 {
        let (mut owner, endpoint, _) = owner(0, true, false); step(&mut owner);
        let action = propose(&mut owner, 1); let sidecar = begin(&mut owner, 1);
        let original = sidecar.round().input().clone();
        let mut completed = if phase > 0 { Some(review(&mut owner, &sidecar, 101, Verdict::Allow)) } else { None };
        if phase > 1 { owner.apply_review(completed.take().unwrap(), Some(&original), &snapshot()).unwrap(); }
        let permit = if phase == 3 { Some(owner.authorize(1, Some(&original), &snapshot()).unwrap()) } else { None };
        let replacement = text_only(&owner, &action);
        owner.record_inputs(1, sidecar.input_revision(), replacement.clone()).unwrap();
        assert_eq!(owner.current_learned_sidecar(&sidecar).err(), Some(Error::Stale));
        match phase {
            0 => assert_eq!(owner.begin_review(1, 101, [7; 32], window(), &snapshot()).err(), Some(Error::Stale)),
            1 => assert_eq!(owner.apply_review(completed.unwrap(), Some(&original), &snapshot()).err(), Some(Error::Stale)),
            2 => assert_eq!(owner.authorize(1, Some(&replacement), &snapshot()).err(), Some(Error::Stale)),
            3 => assert_eq!(owner.dispatch(&permit.unwrap(), &action, Some(&replacement), &snapshot()).err(), Some(Error::Stale)),
            _ => unreachable!(),
        }
        assert_eq!(endpoint.execution_count(), 0);
        assert_eq!(owner.inspect().ledger.reserved, if phase == 3 { 16 } else { 0 });
    }
}

#[test]
fn advancing_or_holding_original_source_invalidates_retained_sidecar_without_rewriting_it() {
    for mode in [0, 2] {
        let (mut owner, endpoint, _) = owner(mode, true, false); step(&mut owner);
        propose(&mut owner, 1); let sidecar = begin(&mut owner, 1);
        let old = sidecar.round().clone(); let old_source = sidecar.source().encode().unwrap();
        let completed = review(&mut owner, &sidecar, 101, Verdict::Allow);
        step(&mut owner);
        assert!(owner.current_learned_sidecar(&sidecar).is_err());
        assert!(owner.apply_review(completed, Some(old.input()), &snapshot()).is_err());
        assert_eq!(sidecar.round(), &old); assert_eq!(sidecar.source().encode().unwrap(), old_source);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn failed_preparation_changes_no_input_but_success_cannot_be_restarted_after_withdrawal() {
    let (mut owner, _, _) = owner(0, true, false); step(&mut owner); propose(&mut owner, 1);
    let before = owner.hosted_learned_generation().unwrap(); let bytes = owner.captured_input_bytes();
    let good = request(&owner, 1);
    let mut short = good.clone(); short.budget.committee_bytes = 1;
    assert_eq!(owner.begin_learned_sidecar(1, owner.actor_revision(), short).err(), Some(Error::Limit));
    let mut wrong = good.clone(); wrong.priority[0].row.position += 1;
    assert_eq!(owner.begin_learned_sidecar(1, owner.actor_revision(), wrong).err(), Some(Error::Missing));
    assert_eq!(owner.input_revision(1).unwrap(), 0); assert!(owner.current_inputs(1).unwrap().is_none());
    assert_eq!(owner.captured_input_bytes(), bytes); assert_eq!(owner.hosted_learned_generation().unwrap(), before);
    let sidecar = owner.begin_learned_sidecar(1, owner.actor_revision(), good.clone()).unwrap();
    assert_eq!(owner.begin_learned_sidecar(1, owner.actor_revision(), good.clone()).err(), Some(Error::Duplicate));
    owner.inputs_unavailable(1, sidecar.input_revision()).unwrap();
    assert_eq!(owner.begin_learned_sidecar(1, owner.actor_revision(), good).err(), Some(Error::Duplicate));
    assert!(owner.current_learned_sidecar(&sidecar).is_err());
    assert_eq!(owner.hosted_learned_generation().unwrap(), before);
}

#[test]
fn requirement_is_bootstrap_only_and_optional_mode_retains_original_text_review() {
    let (mut unattached, _, _) = empty_owner(false);
    assert_eq!(unattached.enable_learned_sidecar_requirement(), Err(Error::Incomplete));
    let (mut required, _, _) = owner(0, false, false);
    required.enable_learned_sidecar_requirement().unwrap();
    assert_eq!(required.enable_learned_sidecar_requirement(), Err(Error::Duplicate));
    let (mut optional, mut endpoint, _) = owner(0, false, false); step(&mut optional);
    assert_eq!(optional.enable_learned_sidecar_requirement(), Err(Error::WrongState));
    assert!(!optional.learned_sidecar_required());
    let action = propose(&mut optional, 1); let input = text_only(&optional, &action);
    optional.record_inputs(1, 0, input.clone()).unwrap();
    let session = optional.begin_review(1, 101, [7; 32], window(), &snapshot()).unwrap();
    optional.apply_review(ballot(session, Verdict::Allow), Some(&input), &snapshot()).unwrap();
    let permit = optional.authorize(1, Some(&input), &snapshot()).unwrap();
    let message = optional.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
    optional.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn terminal_latest_position_coverage_is_not_relabelled_as_the_entire_prefix() {
    let (mut owner, _, _) = owner(0, true, false);
    for _ in 0..4 { step(&mut owner); }
    propose(&mut owner, 1); let before = owner.hosted_learned_generation().unwrap();
    let sidecar = begin(&mut owner, 1);
    assert_eq!(sidecar.evidence().tokens().len(), 4);
    assert_eq!(sidecar.evidence().audit().first_position(), 3);
    assert_eq!(sidecar.evidence().audit().end_position(), 4);
    assert!(sidecar.source().descriptor().layers().values().all(|layer| layer.first_position == 3 && layer.token_count == 1));
    let input = sidecar.round().input().clone(); let reviewed = review(&mut owner, &sidecar, 101, Verdict::Allow);
    owner.apply_review(reviewed, Some(&input), &snapshot()).unwrap();
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_ok());
    assert_eq!(owner.hosted_learned_generation().unwrap(), before);
}

#[test]
fn foreign_handle_and_expired_source_refuse_without_another_packet_or_numerical_step() {
    let (mut first, _, _) = owner(0, true, false); step(&mut first); propose(&mut first, 1);
    let sidecar = begin(&mut first, 1);
    let (mut second, _, _) = owner(0, true, false); step(&mut second); propose(&mut second, 1);
    assert_eq!(second.current_learned_sidecar(&sidecar).err(), Some(Error::Binding));
    assert_eq!(second.begin_learned_sidecar_review(&sidecar, 101, [7; 32], window(), &snapshot()).err(), Some(Error::Binding));
    assert_eq!(second.input_revision(1).unwrap(), 0);
    let before = second.hosted_learned_generation().unwrap(); second.observe_time(ElapsedTick(100)).unwrap();
    let choice = request(&second, 1);
    assert_eq!(second.begin_learned_sidecar(1, second.actor_revision(), choice).err(), Some(Error::Stale));
    assert_eq!(second.hosted_learned_generation().unwrap(), before);
    assert_eq!(second.input_revision(1).unwrap(), 0);
}

#[path = "learned_host_sidecar/refinement.rs"]
mod refinement;
