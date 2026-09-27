//! Computed helper votes over actual checked learned K/V, not supplied ballots.
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
    CommitteeContract, HelperContract, ObservedReview, OversightBroker, ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits,
    human::{HumanReviewer, HumanReviewPolicy},
    helper_workers::{HelperLimits, HelperPhase, HelperPort, HelperRound, wire::{self, WorkerInput}},
    learned_host::sidecar::{LearnedSidecar, LearnedSidecarRequest},
    learned_source::LearnedSourceConfig,
    sidecar::{SidecarCongressBudget, SidecarIdentity, SidecarRefinementOutcome,
        probe_helper::{ProbeHelperBudget, ProbeHelperStatus, ProbeHelperWork, SidecarProbeEvaluator}},
};
use fa_reference::action::consequence::activation::probe::{LinearProbe, ProbeOutcome};
use fa_reference::action::consequence::activation::probe::learned::{CheckedLearnedKv, KvRow};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationSpec, GenerationTelemetryBudget,
};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};

fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}
fn owner() -> (OversightBroker, PublicationEndpoint, HumanReviewer) {
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1,
        grade: RestartGrade::AuditOnly }, Vec::new(), vec![0], vec![0], 0).unwrap();
    let contract = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1,
            b"registered numerical probes; uncertain means abstain".to_vec()).unwrap(),
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
    let reviewer = owner.enable_human_review(HumanReviewPolicy {
        reviewer_id: 55, max_validity_ticks: 100, max_requests: 8,
    }).unwrap();
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    let model = fixture::model();
    let config = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
        }).unwrap(), policy: fixture::policy(&model, 0, 1),
        budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
    owner.own_learned_generation(model, config, LearnedDecoderBindingLimits::default()).unwrap();
    owner.enable_learned_sidecar_requirement().unwrap();
    (owner, endpoint, reviewer)
}
fn step(owner: &mut OversightBroker) {
    let state = owner.hosted_learned_generation().unwrap();
    owner.advance_hosted_learned(state.actor_revision, state.position).unwrap();
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn propose(owner: &mut OversightBroker, id: u64) -> (FrozenAction, LearnedSidecar) {
    let spec = ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"visible".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch, deadline: ElapsedTick(100), units: 16 };
    let action = owner.propose(id, spec, &snapshot()).unwrap().action;
    let evidence = owner.learned_decoder_evidence(id).unwrap().unwrap();
    let request = LearnedSidecarRequest {
        identity: SidecarIdentity { object_id: 1000 + id, generation: 1, transform_id: 7 },
        priority: evidence.audit().source().groups().collect(), budget: SidecarCongressBudget::default(),
    };
    let sidecar = owner.begin_learned_sidecar(id, owner.actor_revision(), request).unwrap();
    (action, sidecar)
}
fn probes(source: &CheckedLearnedKv, mode: u8) -> BTreeMap<KvRow, Vec<LinearProbe>> {
    let rows: BTreeSet<_> = source.groups().map(|group| group.row).collect();
    rows.into_iter().map(|row| {
        let (frame, heads, channels) = source.row_shape(row).unwrap();
        let mut weights = vec![0.0; heads * channels];
        let (bias, threshold) = if row.layer == 1 && row.side == KvSide::Value {
            match mode {
                1 => { weights[0] = 1.0; (0.0, 0.5) }
                2 => { weights[1] = 1.0; (0.0, 0.5) }
                3 => (0.0, 0.0),
                4 => (2.0, 1.0),
                _ => (0.0, 1.0),
            }
        } else { (0.0, 1.0) };
        (row, vec![LinearProbe::new(1, 1, frame.profile, &weights, bias, threshold).unwrap()])
    }).collect()
}
fn evaluator(sidecar: &LearnedSidecar, mode: u8, port: &HelperPort) -> SidecarProbeEvaluator {
    SidecarProbeEvaluator::new(port, sidecar.round(), sidecar.source().clone(),
        probes(sidecar.source(), mode), ProbeHelperBudget::default()).unwrap()
}
fn channel(owner: &mut OversightBroker, sidecar: &LearnedSidecar, id: u64) -> (HelperRound, HelperPort, WorkerInput) {
    let session = owner.begin_learned_sidecar_review(sidecar, id, [7; 32],
        ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, &snapshot()).unwrap();
    let (round, mut ports) = HelperRound::new(session, HelperLimits::default()).unwrap();
    let port = ports.remove("reviewer").unwrap();
    let input = wire::decode_request(&wire::encode_request(&port).unwrap()).unwrap();
    (round, port, input)
}
fn complete(mut round: HelperRound, port: HelperPort, verdict: Verdict) -> ObservedReview {
    let salt = [11; 32];
    port.submit_commitment(port.request().commitment(verdict, &salt).unwrap()).unwrap();
    round.advance(ElapsedTick(1)).unwrap();
    assert_eq!(port.phase(), HelperPhase::ReadyReveal);
    port.reveal(verdict, &salt).unwrap();
    round.finish(ElapsedTick(1)).unwrap()
}
fn one_refinement(owner: &mut OversightBroker, sidecar: &mut LearnedSidecar) {
    let (round, port, input) = channel(owner, sidecar, 101);
    let mut worker = evaluator(sidecar, 1, &port);
    let verdict = worker.evaluate(&input).unwrap();
    assert_eq!(verdict, Verdict::Abstain);
    let review = complete(round, port, verdict);
    assert!(matches!(owner.refine_learned_sidecar(sidecar, &review).unwrap(), SidecarRefinementOutcome::Refined { .. }));
}

#[test]
fn quiet_original_scores_produce_one_complete_vote_without_touching_the_generator() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let numerical = owner.hosted_learned_generation().unwrap();
    let (_round, port, input) = channel(&mut owner, &sidecar, 101);
    let mut worker = evaluator(&sidecar, 0, &port);
    assert_eq!(worker.evaluate(&input), Ok(Verdict::Allow));
    let report = worker.report().unwrap();
    assert_eq!(report.observations().len(), 4);
    assert!(report.observations().iter().all(|value| value.outcome() == ProbeOutcome::CertifiedQuiet));
    assert_eq!(report.work().refined_groups, 0);
    let before = worker.work();
    assert_eq!(worker.evaluate_port(&port), Err(Error::WrongState));
    assert_eq!(worker.work(), before);
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn retained_but_undisclosed_values_cannot_make_a_coarse_vote_permissive() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    assert!(sidecar.source().report().retained_groups > 0);
    let (_round, port, _) = channel(&mut owner, &sidecar, 101);
    let mut worker = evaluator(&sidecar, 1, &port);
    assert_eq!(worker.evaluate_port(&port), Ok(Verdict::Abstain));
    let report = worker.report().unwrap();
    assert!(report.observations().iter().any(|value| value.outcome() == ProbeOutcome::NeedsRefinement));
    assert!(report.selected_groups().is_empty());
    assert_eq!(report.work().refined_groups, 0);
    assert_eq!(report.work().refinement_bytes, 0);
    assert_eq!(report.work().materialized_values, 0);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn actual_abstention_refines_original_inputs_and_fresh_scores_still_require_two_keys() {
    let (mut owner, mut endpoint, human) = owner(); step(&mut owner); step(&mut owner);
    let (action, mut sidecar) = propose(&mut owner, 1);
    let mut abstentions = 0;
    let mut allowed = None;
    for id in 101..=110 {
        let (round, port, input) = channel(&mut owner, &sidecar, id);
        let mut worker = evaluator(&sidecar, 1, &port);
        let verdict = worker.evaluate(&input).unwrap();
        let selected = worker.report().unwrap().selected_groups();
        assert_eq!(selected, sidecar.round().selected_groups());
        let review = complete(round, port, verdict);
        if verdict == Verdict::Allow { allowed = Some(review); break; }
        assert_eq!(verdict, Verdict::Abstain); abstentions += 1;
        assert!(matches!(owner.refine_learned_sidecar(&mut sidecar, &review).unwrap(), SidecarRefinementOutcome::Refined { .. }));
    }
    assert!(abstentions > 0);
    let input = sidecar.round().input().clone();
    owner.apply_review(allowed.expect("finite retained residuals resolve the quiet probe"), Some(&input), &snapshot()).unwrap();
    let key = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&key, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    assert_eq!(endpoint.execution_count(), 0);
    let request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    let human_key = human.approve(&request, ElapsedTick(1)).unwrap();
    let envelope = owner.dispatch_with_human(&key, &human_key, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1); assert_eq!(endpoint.payload(), b"visible");
    assert_eq!(owner.inspect().ledger.charged, 16);
    assert!(owner.dispatch_with_human(&key, &human_key, &action, Some(&input), &snapshot()).is_err());
}

#[test]
fn a_certified_alarm_is_a_hold_recommendation_not_an_exact_disqualifier() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (round, port, input) = channel(&mut owner, &sidecar, 101);
    let mut worker = evaluator(&sidecar, 4, &port);
    let verdict = worker.evaluate(&input).unwrap();
    assert_eq!(verdict, Verdict::Hold);
    assert!(worker.report().unwrap().observations().iter().any(|value| value.outcome() == ProbeOutcome::CertifiedAlarm));
    owner.apply_review(complete(round, port, verdict), Some(sidecar.round().input()), &snapshot()).unwrap();
    assert!(owner.authorize(1, Some(sidecar.round().input()), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn exact_equality_abstains_instead_of_turning_a_zero_margin_into_allow() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (_round, port, input) = channel(&mut owner, &sidecar, 101);
    let mut worker = evaluator(&sidecar, 3, &port);
    assert_eq!(worker.evaluate(&input), Ok(Verdict::Abstain));
    assert!(worker.report().unwrap().observations().iter().any(|value| value.outcome() == ProbeOutcome::AtThreshold));
}

#[test]
fn empty_missing_duplicate_and_foreign_probe_rosters_do_not_claim_complete_coverage() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (_round, port, _) = channel(&mut owner, &sidecar, 101);
    let source = sidecar.source(); let good = probes(source, 0);
    assert!(evaluator(&sidecar, 0, &port).report().is_none());
    for mode in 0..5 {
        let mut changed = good.clone();
        let row = *changed.keys().next().unwrap();
        let expected = match mode {
            0 => { changed.clear(); Error::Incomplete }
            1 => { changed.remove(&row); Error::Incomplete }
            2 => { changed.get_mut(&row).unwrap().push(good[&row][0].clone()); Error::Duplicate }
            4 => { changed.get_mut(&row).unwrap().clear(); Error::Incomplete }
            _ => {
                let (mut frame, heads, channels) = source.row_shape(row).unwrap();
                frame.profile.model_generation += 1;
                changed.insert(row, vec![LinearProbe::new(1, 1, frame.profile, &vec![0.0; heads * channels], 0.0, 1.0).unwrap()]);
                Error::Binding
            }
        };
        assert_eq!(SidecarProbeEvaluator::new(&port, sidecar.round(), source.clone(), changed,
            ProbeHelperBudget::default()).err(), Some(expected));
    }
}

#[test]
fn source_labels_and_shapes_cannot_replace_exact_checked_source_bytes() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, first) = propose(&mut owner, 1);
    let (_first_round, first_port, _) = channel(&mut owner, &first, 101);
    step(&mut owner);
    let (_, second) = propose(&mut owner, 2);
    let (_second_round, second_port, _) = channel(&mut owner, &second, 102);
    assert_eq!(SidecarProbeEvaluator::new(&first_port, first.round(), second.source().clone(),
        probes(second.source(), 0), ProbeHelperBudget::default()).err(), Some(Error::Binding));
    assert!(evaluator(&second, 0, &second_port).report().is_none());
}

#[test]
fn exact_aggregate_budgets_pass_and_every_one_less_component_refuses() {
    let (mut owner, _, _) = owner(); step(&mut owner); step(&mut owner);
    let (_, mut sidecar) = propose(&mut owner, 1); one_refinement(&mut owner, &mut sidecar);
    let roster = probes(sidecar.source(), 1);
    let exact = SidecarProbeEvaluator::required_budget(sidecar.source(), sidecar.round(), "reviewer", &roster).unwrap();
    let (_round, port, input) = channel(&mut owner, &sidecar, 102);
    let mut good = SidecarProbeEvaluator::new(&port, sidecar.round(), sidecar.source().clone(), roster.clone(), exact).unwrap();
    assert!(good.evaluate(&input).is_ok());
    for field in 0..6 {
        let mut small = exact;
        match field {
            0 => { assert!(small.input_bytes > 0); small.input_bytes -= 1; }
            1 => { assert!(small.probes > 0); small.probes -= 1; }
            2 => { assert!(small.probe_coordinates > 0); small.probe_coordinates -= 1; }
            3 => { assert!(small.refinement_bytes > 0); small.refinement_bytes -= 1; }
            4 => { assert!(small.materialized_values > 0); small.materialized_values -= 1; }
            _ => { assert!(small.reconstruction_products > 0); small.reconstruction_products -= 1; }
        }
        assert_eq!(SidecarProbeEvaluator::new(&port, sidecar.round(), sidecar.source().clone(), roster.clone(), small).err(),
            Some(Error::Limit), "field {field}");
    }
}

#[test]
fn changed_wire_epochs_round_root_and_salt_limit_fail_before_scoring_without_retry() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (_round, port, input) = channel(&mut owner, &sidecar, 101);
    let wire = wire::encode_request(&port).unwrap();
    let profile_offset = wire::REQUEST_HEADER_BYTES + 8 + 32 + 2 + "reviewer".len() + 2;
    for field in 0..7 {
        let mut changed = wire.clone();
        match field {
            0..=3 => changed[profile_offset + field * 8 + 7] += 1,
            4 => changed[wire::REQUEST_HEADER_BYTES + 7] += 1,
            5 => changed[wire::REQUEST_HEADER_BYTES + 8] ^= 1,
            _ => changed[profile_offset - 2..profile_offset].copy_from_slice(&128_u16.to_be_bytes()),
        }
        let changed = wire::decode_request(&changed).unwrap();
        let mut worker = evaluator(&sidecar, 0, &port);
        assert_eq!(worker.evaluate(&changed), Err(Error::Binding));
        assert_eq!(worker.status(), ProbeHelperStatus::Failed(Error::Binding));
        assert_eq!(worker.work(), ProbeHelperWork::default()); assert!(worker.report().is_none());
        assert_eq!(worker.evaluate(&input), Err(Error::WrongState));
    }
    assert_eq!(evaluator(&sidecar, 0, &port).evaluate(&input), Ok(Verdict::Allow));
}

#[test]
fn another_valid_action_view_is_not_the_registered_input_even_on_the_same_source() {
    let (mut owner, _, _) = owner(); step(&mut owner);
    let (_, first) = propose(&mut owner, 1); let (_, second) = propose(&mut owner, 2);
    let (_first_round, first_port, _) = channel(&mut owner, &first, 101);
    let (_round, port, input) = channel(&mut owner, &second, 102);
    let mut worker = evaluator(&first, 0, &first_port);
    assert_eq!(worker.evaluate_port(&port), Err(Error::Binding));
    assert!(worker.report().is_none()); assert_eq!(worker.work(), ProbeHelperWork::default());
    assert_eq!(evaluator(&second, 0, &port).evaluate(&input), Ok(Verdict::Allow));
}

#[test]
fn an_actual_activation_alarm_after_refinement_holds_the_original_effect() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner); step(&mut owner);
    let (_, mut sidecar) = propose(&mut owner, 1);
    let mut held = None;
    for id in 101..=110 {
        let (round, port, input) = channel(&mut owner, &sidecar, id);
        // Unlike a constant bias control, this observes the actual off-axis
        // value computed by the original decoder for the sampled second token.
        let mut worker = evaluator(&sidecar, 2, &port);
        let verdict = worker.evaluate(&input).unwrap();
        assert!(worker.work().probe_coordinates > 0);
        let alarm = worker.report().unwrap().observations().iter().any(|observation|
            observation.row().layer == 1 && observation.row().side == KvSide::Value
                && observation.outcome() == ProbeOutcome::CertifiedAlarm);
        let review = complete(round, port, verdict);
        if verdict == Verdict::Hold {
            assert!(alarm); held = Some(review); break;
        }
        assert_eq!(verdict, Verdict::Abstain); assert!(!alarm);
        assert!(matches!(owner.refine_learned_sidecar(&mut sidecar, &review).unwrap(),
            SidecarRefinementOutcome::Refined { .. }));
    }
    let input = sidecar.round().input().clone();
    let review = held.expect("retained exact off-axis source must expose the alarm");
    owner.apply_review(review, Some(&input), &snapshot()).unwrap();
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn an_original_source_advance_invalidates_even_a_fully_computed_quiet_vote() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let (round, port, input) = channel(&mut owner, &sidecar, 101);
    let mut worker = evaluator(&sidecar, 0, &port);
    let verdict = worker.evaluate(&input).unwrap(); assert_eq!(verdict, Verdict::Allow);
    let review = complete(round, port, verdict);
    let evidence = sidecar.round().input().clone();
    let work = worker.work();
    step(&mut owner);
    assert!(owner.apply_review(review, Some(&evidence), &snapshot()).is_err());
    assert!(owner.authorize(1, Some(&evidence), &snapshot()).is_err());
    assert_eq!(worker.work(), work); assert_eq!(worker.report().unwrap().verdict(), Verdict::Allow);
    assert_eq!(endpoint.execution_count(), 0);
    // Current source, same quiet registered question, genuine fresh original round.
    let (_, current) = propose(&mut owner, 2);
    let (round, port, input) = channel(&mut owner, &current, 102);
    let mut worker = evaluator(&current, 0, &port);
    let verdict = worker.evaluate(&input).unwrap(); assert_eq!(verdict, Verdict::Allow);
    owner.apply_review(complete(round, port, verdict), Some(current.round().input()), &snapshot()).unwrap();
    assert!(owner.authorize(2, Some(current.round().input()), &snapshot()).is_ok());
    assert_eq!(endpoint.execution_count(), 0);
}
