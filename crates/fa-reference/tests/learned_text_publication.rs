//! Exact native learned output through the ORIGINAL reference permission path.
//! Synthetic helper ballots and tiny weights are explicit protocol controls.
#[path = "support/learned_text_model.rs"]
mod fixture;
use fixture::*;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{PublicationEndpoint, StopRequest};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, ObservedReview, OversightBroker, ReviewWindow, action_frame,
    decoder_monitoring::LearnedDecoderBindingLimits,
    human::{HumanReviewer, HumanReviewPolicy},
    learned_host::{text::LearnedTextTarget, sidecar::LearnedSidecarRequest},
    learned_source::{LearnedEvidenceLimits, LearnedSourceConfig, text::LearnedTextCompletion},
    sidecar::{SidecarCongressBudget, SidecarIdentity},
    helper_workers::{HelperLimits, HelperRound},
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::GenerationSpec;
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn actor(tokens: &[u32]) -> ActorState {
    // Deliberately AuditOnly: this test does not qualify external actor cache or
    // sampler bytes for restart. The live source independently owns real KV.
    ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
        grade: RestartGrade::AuditOnly }, tokens.to_vec(), vec![0], vec![0], tokens.len() as u64).unwrap()
}
fn empty_owner(human: bool) -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>) {
    endpoint_owner(human, false)
}
fn endpoint_owner(human: bool, stream: bool) -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>)
{
    let contract = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1, b"approve?".to_vec()).unwrap(),
    )])).unwrap();
    let mut endpoint = if stream {
        use fa_reference::action::consequence::delivery::stream::StreamProfile;
        PublicationEndpoint::new_stream(target(), StreamProfile::new(1, 1, 4, 128, 512).unwrap(), 1000, 8).unwrap()
    } else { PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap() };
    let mut owner = OversightBroker::new(ControllerConfig {
        scope: scope(), total: 100, max_attempts: 8, actor: actor(&[]), suspend_at_incident: 3,
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
    owner.observe_time(ElapsedTick(1)).unwrap();
    endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    (owner, endpoint, reviewer)
}

fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn destination(owner: &OversightBroker) -> LearnedTextTarget {
    LearnedTextTarget { target: target(), required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }
}
fn raw(owner: &OversightBroker, payload: &[u8]) -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: payload.to_vec(),
        required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}
fn configured(human: bool, sidecar: bool) -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>) {
    let (mut owner, endpoint, reviewer) = empty_owner(human);
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let tokenizer = tokenizer(&model); let config = config(&model);
    owner.own_learned_text_generation(model, tokenizer, config, LearnedDecoderBindingLimits::default()).unwrap();
    if sidecar { owner.enable_learned_sidecar_requirement().unwrap(); }
    (owner, endpoint, reviewer)
}
fn complete(owner: &mut OversightBroker) {
    for _ in 0..4 {
        let state = owner.hosted_learned_generation().unwrap();
        if !state.status.is_active() { break; }
        owner.advance_hosted_learned(state.actor_revision, state.position).unwrap();
    }
    assert!(!owner.hosted_learned_generation().unwrap().status.is_active());
}
fn stop(owner: &mut OversightBroker) {
    let view = owner.inspect();
    owner.request_stop(StopRequest { operation: 900, expected_control_sequence: view.sequence,
        expected_authority_epoch: view.ledger.epoch }).unwrap();
}
fn record_input(owner: &mut OversightBroker, id: u64, action: &FrozenAction) -> CommitteeInput {
    let helper = &owner.contracts().members()["reviewer"];
    let mut bytes = action_frame(action);
    let boundary = bytes.len();
    bytes.extend_from_slice(helper.question());
    let end = bytes.len();
    let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(input, AuthorizationProjection {
        projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch,
        projected_originals: Vec::new(),
    }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(action, owner.contracts(),
        BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    owner.record_inputs(id, owner.input_revision(id).unwrap(), inputs.clone()).unwrap();
    inputs
}
fn review(owner: &mut OversightBroker, id: u64, verdict: Verdict) -> ObservedReview {
    let mut session = owner.begin_review(id, id + 100, [7; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
    }, &snapshot()).unwrap();
    let salt = [11; 32];
    let commit = session.commitment("reviewer", verdict, &salt).unwrap();
    session.commit("reviewer", commit, ElapsedTick(1)).unwrap();
    session.open_reveals(ElapsedTick(1)).unwrap();
    session.reveal("reviewer", verdict, &salt, ElapsedTick(1)).unwrap();
    session.finish(ElapsedTick(1)).unwrap()
}
fn approve(owner: &mut OversightBroker, id: u64, inputs: &CommitteeInput) {
    let review = review(owner, id, Verdict::Allow);
    owner.apply_review(review, Some(inputs), &snapshot()).unwrap();
}

#[test]
fn generated_bytes_sidecar_workers_and_two_independent_keys_reach_original_endpoint() {
    let (mut owner, mut endpoint, human) = configured(true, true); complete(&mut owner);
    let message = owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).unwrap();
    let numerical = owner.hosted_learned_generation().unwrap();
    let request = destination(&owner);
    let action = owner.propose_learned_text(1, request, &snapshot()).unwrap().action;
    assert_eq!(action.spec().payload, message.bytes());
    let choice = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
        priority: Vec::new(), budget: SidecarCongressBudget::default() };
    let sidecar = owner.begin_learned_sidecar(1, owner.actor_revision(), choice).unwrap();
    assert_eq!(sidecar.evidence().tokens(), message.evidence().tokens());
    let input = owner.current_learned_sidecar(&sidecar).unwrap().clone();
    let session = owner.begin_learned_sidecar_review(&sidecar, 101, [7; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
    }, &snapshot()).unwrap();
    let (mut round, ports) = HelperRound::new(session, HelperLimits::default()).unwrap();
    let port = &ports["reviewer"]; let salt = b"independent-worker";
    assert_eq!(port.request().view().actual_input().part_bytes(0).unwrap(), action_frame(&action));
    port.submit_commitment(port.request().commitment(Verdict::Allow, salt).unwrap()).unwrap();
    round.advance(ElapsedTick(1)).unwrap(); port.reveal(Verdict::Allow, salt).unwrap();
    let reviewed = round.finish(ElapsedTick(1)).unwrap();
    owner.apply_review(reviewed, Some(&input), &snapshot()).unwrap();
    let automatic = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&automatic, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    let request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    assert_eq!(request.action().spec().payload, message.bytes());
    let human = human.unwrap().approve(&request, ElapsedTick(1)).unwrap();
    let envelope = owner.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot()).unwrap();
    assert_eq!(envelope.request().payload(), b"OK");
    owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"OK"); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.inspect().ledger.charged, 16);
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
    assert!(owner.dispatch_with_human(&automatic, &human, &action, Some(&input), &snapshot()).is_err());
}

#[test]
fn guessed_prefixes_and_altered_bytes_refuse_before_proposal_or_evidence_retention() {
    let (mut owner, endpoint, _) = configured(false, false);
    for position in 0..4 {
        let before = owner.inspect(); let usage = owner.decoder_binding_usage();
        let request = raw(&owner, b"OK");
        assert_eq!(owner.propose(1, request, &snapshot()).err(), Some(Error::Incomplete));
        assert_eq!(owner.inspect(), before); assert_eq!(owner.decoder_binding_usage(), usage);
        assert_eq!(owner.input_revision(1), Err(Error::Missing));
        let state = owner.hosted_learned_generation().unwrap();
        assert_eq!(state.position, position);
        owner.advance_hosted_learned(state.actor_revision, position).unwrap();
    }
    for payload in [b"O".as_slice(), b"OK ", b"PP", b"KO", b""] {
        let before = owner.inspect(); let usage = owner.decoder_binding_usage();
        let request = raw(&owner, payload);
        assert_eq!(owner.propose(1, request, &snapshot()).err(), Some(Error::Binding));
        assert_eq!(owner.inspect(), before); assert_eq!(owner.decoder_binding_usage(), usage);
    }
    let request = raw(&owner, b"OK");
    assert_eq!(owner.propose(1, request, &snapshot()).unwrap().action.spec().payload, b"OK");
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn incomplete_or_undecodable_results_have_no_payload_override_path() {
    for case in 0..5 {
        let (mut owner, endpoint, _) = empty_owner(false);
        let chain = match case { 0 => vec![0xc3, END], 1 => vec![OTHER_CONTROL, END],
            2 => vec![END], _ => vec![b'O' as u32, b'K' as u32, END] };
        let model = model(&chain); let mut config = config(&model);
        if case == 3 { config.output.max_bytes = 1; }
        if case == 4 { config.max_new_tokens = 2; }
        owner.own_learned_text_generation(model.clone(), tokenizer(&model), config,
            LearnedDecoderBindingLimits::default()).unwrap(); complete(&mut owner);
        let before = owner.inspect(); let request = raw(&owner, b"replacement");
        assert!(owner.propose(1, request, &snapshot()).is_err());
        let request = destination(&owner);
        assert!(owner.propose_learned_text(1, request, &snapshot()).is_err());
        assert_eq!(owner.inspect(), before); assert_eq!(endpoint.execution_count(), 0);
    }
    let (mut owner, _, _) = empty_owner(false); let model = model(&[b'O' as u32, b'K' as u32, END]);
    let mut config = config(&model); config.max_new_tokens = 2;
    config.output.completion = LearnedTextCompletion::StopOrTokenLimit;
    owner.own_learned_text_generation(model.clone(), tokenizer(&model), config,
        LearnedDecoderBindingLimits::default()).unwrap(); complete(&mut owner);
    let request = destination(&owner);
    assert_eq!(owner.propose_learned_text(1, request, &snapshot()).unwrap().action.spec().payload, b"OK");
}

#[test]
fn a_failed_text_bootstrap_is_atomic_but_a_selected_mode_cannot_be_replaced() {
    let (mut stream, _, _) = endpoint_owner(false, true);
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let before_stream = stream.inspect(); let stream_revision = stream.actor_revision();
    assert_eq!(stream.own_learned_text_generation(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default()), Err(Error::Binding));
    assert_eq!(stream.inspect(), before_stream); assert_eq!(stream.actor_revision(), stream_revision);
    assert!(!stream.decoder_monitoring_required());
    let (mut owner, _, _) = empty_owner(false);
    let mut bad = config(&model);
    bad.tokenization.input_bytes = 1;
    let before = owner.inspect(); let revision = owner.actor_revision();
    assert_eq!(owner.own_learned_text_generation(model.clone(), tokenizer(&model), bad,
        LearnedDecoderBindingLimits::default()), Err(Error::Limit));
    assert_eq!(owner.inspect(), before); assert_eq!(owner.actor_revision(), revision);
    assert!(!owner.learned_text_required()); assert!(!owner.decoder_monitoring_required());
    owner.own_learned_text_generation(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default()).unwrap();
    assert!(owner.learned_text_required());
    assert_eq!(owner.own_learned_text_generation(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default()), Err(Error::Duplicate));
    complete(&mut owner); assert_eq!(owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
}

#[test]
fn stopping_at_review_authorization_or_dispatch_never_promotes_historical_text() {
    for cut in 0..4 {
        let (mut owner, endpoint, _) = configured(false, false); complete(&mut owner);
        let original = owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).unwrap();
        let request = destination(&owner);
        let action = owner.propose_learned_text(1, request, &snapshot()).unwrap().action;
        let inputs = record_input(&mut owner, 1, &action);
        let mut review = if cut >= 1 { Some(review(&mut owner, 1, Verdict::Allow)) } else { None };
        if cut >= 2 { owner.apply_review(review.take().unwrap(), Some(&inputs), &snapshot()).unwrap(); }
        let permit = if cut == 3 { Some(owner.authorize(1, Some(&inputs), &snapshot()).unwrap()) } else { None };
        stop(&mut owner);
        assert_eq!(owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).err(), Some(Error::WrongState));
        match cut {
            0 => { assert!(owner.begin_review(1, 101, [7; 32], ReviewWindow {
                commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, &snapshot()).is_err()); },
            1 => { assert!(owner.apply_review(review.unwrap(), Some(&inputs), &snapshot()).is_err()); },
            2 => { assert!(owner.authorize(1, Some(&inputs), &snapshot()).is_err()); },
            _ => { assert!(owner.dispatch(&permit.unwrap(), &action, Some(&inputs), &snapshot()).is_err()); },
        }
        assert_eq!(original.bytes(), b"OK"); assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn changed_dispatch_bytes_do_not_spend_keys_and_correct_original_bytes_still_publish() {
    let (mut owner, mut endpoint, human) = configured(true, false); complete(&mut owner);
    let request = destination(&owner);
    let action = owner.propose_learned_text(1, request, &snapshot()).unwrap().action;
    let input = record_input(&mut owner, 1, &action); approve(&mut owner, 1, &input);
    let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    let request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    let human = human.unwrap().approve(&request, ElapsedTick(1)).unwrap();
    let mut altered = action.spec().clone(); altered.payload = b"KO".to_vec();
    let altered = FrozenAction::freeze(altered).unwrap();
    let before = owner.inspect();
    assert!(owner.dispatch_with_human(&permit, &human, &altered, Some(&input), &snapshot()).is_err());
    assert_eq!(owner.inspect(), before); assert_eq!(endpoint.execution_count(), 0);
    let message = owner.dispatch_with_human(&permit, &human, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"OK"); assert_eq!(endpoint.execution_count(), 1);
}

#[test]
fn issued_envelopes_reconcile_without_new_text_after_the_original_authority_stops() {
    for executed in [false, true] {
        let (mut owner, mut endpoint, _) = configured(false, false); complete(&mut owner);
        let request = destination(&owner);
        let action = owner.propose_learned_text(1, request, &snapshot()).unwrap().action;
        let input = record_input(&mut owner, 1, &action); approve(&mut owner, 1, &input);
        let permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
        let message = owner.dispatch(&permit, &action, Some(&input), &snapshot()).unwrap();
        if executed { endpoint.deliver(&message).unwrap(); } // acknowledgment intentionally lost
        stop(&mut owner);
        assert_eq!(owner.inspect().ledger.charged, 16);
        assert!(owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).is_err());
        let sweep = owner.progress_stop(&mut endpoint).unwrap();
        assert!(sweep.progress.drained());
        assert_eq!(owner.inspect().ledger.charged, if executed { 16 } else { 0 });
        assert_eq!(endpoint.execution_count(), if executed { 1 } else { 0 });
        assert_eq!(endpoint.payload(), if executed { b"OK".as_slice() } else { b"initial" });
    }
}

#[test]
fn generic_effect_profiles_remain_generic_and_do_not_inherit_a_text_constraint() {
    let (mut owner, mut endpoint, _) = empty_owner(false);
    let model = model(&[b'O' as u32, b'K' as u32, END]); let config = config(&model);
    let encoded = tokenizer(&model).encode(config.prompt.as_bytes(), config.tokenization).unwrap();
    let spec = GenerationSpec::new(encoded.tokens().to_vec(), config.max_new_tokens, config.stop_tokens, config.sampling).unwrap();
    owner.own_learned_generation(model, LearnedSourceConfig { stream: config.stream,
        evaluation_origin: config.evaluation_origin, monitor_generation: config.monitor_generation,
        spec, policy: config.policy, budget: config.budget, telemetry: config.telemetry },
        LearnedDecoderBindingLimits::default()).unwrap();
    assert!(!owner.learned_text_required());
    let state = owner.hosted_learned_generation().unwrap();
    owner.advance_hosted_learned(state.actor_revision, 0).unwrap();
    let request = raw(&owner, b"generic effect");
    let action = owner.propose(1, request, &snapshot()).unwrap().action;
    let inputs = record_input(&mut owner, 1, &action); approve(&mut owner, 1, &inputs);
    let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let envelope = owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"generic effect");
    assert!(owner.hosted_learned_generation().unwrap().status.is_active());
}

#[path = "learned_text_publication/stream.rs"]
mod stream;
