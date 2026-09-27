//! Original inference and computed probe votes; no synthetic worker ballots.
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
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, OversightBroker,
    ReviewWindow, helper_workers::HelperLimits, decoder_monitoring::LearnedDecoderBindingLimits,
    human::{HumanReviewer, HumanReviewPolicy}, learned_source::LearnedSourceConfig,
    learned_host::sidecar::{LearnedSidecar, LearnedSidecarRequest,
        workers::{LearnedWorkerSchedule, LearnedWorkerRound, LearnedWorkerStatus, LearnedWorkerStop,
            probes::{LearnedProbeReview, ProbeReviewMember, ProbeReviewLimits}}},
    sidecar::{SidecarCongressBudget, SidecarIdentity,
        probe_helper::{ProbeHelperBudget, ProbeHelperStatus}}};
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::activation::probe::learned::{CheckedLearnedKv, KvRow};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::monitored::{
    GenerationBudget, GenerationSpec, GenerationTelemetryBudget};
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
    let contract = CommitteeContract::new(["alpha", "beta"].into_iter().map(|member| {
        (member.to_owned(), HelperContract::new(InputProfileBinding { profile_id: 1,
            profile_bytes: Vec::new(), tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 },
            1, b"registered numerical probes; uncertain means abstain".to_vec()).unwrap())
    }).collect()).unwrap();
    let mut endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap();
    let mut owner = OversightBroker::new(ControllerConfig {
        scope: scope(), total: 100, max_attempts: 8, actor, suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: ["alpha", "beta"].into_iter().map(|member| (member.to_owned(),
                MemberPolicy { cohort: member.to_owned(), weight: 1 })).collect(),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 2,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 2, minimum_cohorts: 2,
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
fn schedule() -> LearnedWorkerSchedule {
    LearnedWorkerSchedule { rounds: (0..5).map(|i| LearnedWorkerRound {
        round: 101 + i, evidence_root: [7; 32], window: ReviewWindow {
            commit_by: ElapsedTick(10 + 15 * i), reveal_by: ElapsedTick(15 + 15 * i) },
    }).collect(), helpers: HelperLimits::default(), polls: 64 }
}
fn members(sidecar: &LearnedSidecar, mode: u8) -> BTreeMap<String, ProbeReviewMember> {
    ["alpha", "beta"].into_iter().enumerate().map(|(i, member)| {
        (member.to_owned(), ProbeReviewMember { probes: probes(sidecar.source(), mode),
            salts: (0..5).map(|round| vec![16 + (i * 5 + round) as u8; 32]).collect() })
    }).collect()
}
fn limits() -> ProbeReviewLimits {
    ProbeReviewLimits { evaluations: 10, per_evaluation: ProbeHelperBudget::default() }
}
fn drive(owner: &mut OversightBroker, run: &mut LearnedProbeReview) -> LearnedWorkerStatus {
    for _ in 0..64 {
        if run.status() != LearnedWorkerStatus::Running { return run.status(); }
        let revision = run.revision();
        owner.advance_learned_probe_review(run, revision, ElapsedTick(1), &snapshot()).unwrap();
    }
    panic!("fixed computed review did not terminate");
}

#[test]
fn computed_refinement_completes_original_rounds_then_requires_both_effect_keys() {
    let (mut owner, mut endpoint, human) = owner(); step(&mut owner); step(&mut owner);
    let (action, sidecar) = propose(&mut owner, 1);
    let definitions = members(&sidecar, 1);
    let numerical = owner.hosted_learned_generation().unwrap();
    let mut run = owner.begin_learned_probe_review(sidecar, schedule(), definitions, limits(), &snapshot()).unwrap();
    let reserved = run.reservation();
    assert_eq!(reserved.evaluations, 10);
    assert_eq!(drive(&mut owner, &mut run), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Decided));
    assert!(run.history().len() > 1);
    assert_eq!(run.evaluations(), run.history().len() * 2);
    assert!(run.records().values().any(|r| r.status == Some(ProbeHelperStatus::Judged(Verdict::Abstain))));
    assert!(run.records().values().any(|r| r.work.refined_groups > 0));
    assert!(run.records().values().all(|r| r.commitment_queued && r.reveal_queued && r.failure.is_none()));
    assert_eq!(run.reservation(), reserved);
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
    let input = run.input().clone(); let review = run.take_review().unwrap();
    assert!(review.missing().is_empty()); assert!(review.abstained().is_empty());
    assert!(run.take_review().is_err());
    owner.apply_review(review, Some(&input), &snapshot()).unwrap();
    let key = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    assert_eq!(owner.dispatch(&key, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
    let request = owner.request_human_approval(10, 1, Some(&input), ElapsedTick(80)).unwrap();
    let human = human.approve(&request, ElapsedTick(1)).unwrap();
    let envelope = owner.dispatch_with_human(&key, &human, &action, Some(&input), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"visible"); assert_eq!(endpoint.execution_count(), 1);
    assert_eq!(owner.inspect().ledger.charged, 16);
    assert!(owner.dispatch_with_human(&key, &human, &action, Some(&input), &snapshot()).is_err());
}

#[test]
fn one_actual_alarm_holds_the_effect_while_its_quiet_sibling_still_completes() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1);
    let mut definitions = members(&sidecar, 0);
    definitions.get_mut("alpha").unwrap().probes = probes(sidecar.source(), 2);
    let mut run = owner.begin_learned_probe_review(sidecar, schedule(), definitions, limits(), &snapshot()).unwrap();
    assert_eq!(drive(&mut owner, &mut run), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Decided));
    assert!(run.records().iter().any(|((_, member), r)| member == "alpha"
        && r.status == Some(ProbeHelperStatus::Judged(Verdict::Hold))));
    assert!(run.records().iter().filter(|((_, member), _)| member == "beta")
        .all(|(_, r)| r.status == Some(ProbeHelperStatus::Judged(Verdict::Allow))));
    let input = run.input().clone();
    owner.apply_review(run.take_review().unwrap(), Some(&input), &snapshot()).unwrap();
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn roster_salts_and_full_schedule_budget_are_checked_before_starting_a_round() {
    for defect in 0..5 {
        let (mut owner, endpoint, _) = owner(); step(&mut owner);
        let (_, sidecar) = propose(&mut owner, 1);
        let mut definitions = members(&sidecar, 0); let mut bound = limits();
        let expected = match defect {
            0 => { definitions.remove("beta"); Error::Binding }
            1 => { definitions.get_mut("alpha").unwrap().salts.pop(); Error::Incomplete }
            2 => { definitions.get_mut("beta").unwrap().salts[0] = vec![1; 15]; Error::Limit }
            3 => { bound.evaluations = 9; Error::Limit }
            _ => { bound.per_evaluation.probes = 3; Error::Limit }
        };
        let numerical = owner.hosted_learned_generation().unwrap();
        assert_eq!(owner.begin_learned_probe_review(sidecar, schedule(), definitions, bound, &snapshot()).err(), Some(expected));
        assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
        // Same original round IDs remain unused after these preflight refusals.
        let (_, fresh) = propose(&mut owner, 2); let definitions = members(&fresh, 0);
        let mut good = owner.begin_learned_probe_review(fresh, schedule(), definitions, limits(), &snapshot()).unwrap();
        assert_eq!(drive(&mut owner, &mut good), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Decided));
        assert_eq!(good.evaluations(), 2); assert_eq!(good.reservation().evaluations, 10);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn stale_foreign_and_cancel_calls_never_repeat_numerical_evaluation() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1); let definitions = members(&sidecar, 0);
    let mut run = owner.begin_learned_probe_review(sidecar, schedule(), definitions, limits(), &snapshot()).unwrap();
    let before = run.records().clone();
    assert_eq!(owner.advance_learned_probe_review(&mut run, 1, ElapsedTick(1), &snapshot()), Err(Error::Stale));
    assert_eq!(owner.advance_learned_probe_review(&mut run, 0, ElapsedTick(0), &snapshot()), Err(Error::Stale));
    let (mut foreign, _, _) = self::owner();
    assert_eq!(foreign.advance_learned_probe_review(&mut run, 0, ElapsedTick(1), &snapshot()), Err(Error::Binding));
    assert_eq!(run.records(), &before); assert_eq!(run.evaluations(), 0);
    assert_eq!(owner.advance_learned_probe_review(&mut run, 0, ElapsedTick(1), &snapshot()), Ok(LearnedWorkerStatus::Running));
    assert_eq!(run.evaluations(), 2);
    assert!(run.records().values().all(|r| r.commitment_queued && !r.reveal_queued));
    assert_eq!(run.cancel(0), Err(Error::Stale));
    let work = run.records().clone(); let reserved = run.reservation();
    run.cancel(1).unwrap();
    assert_eq!(run.status(), LearnedWorkerStatus::Cancelled);
    let revision = run.revision();
    assert_eq!(owner.advance_learned_probe_review(&mut run, revision, ElapsedTick(1), &snapshot()), Err(Error::WrongState));
    assert_eq!(run.records(), &work); assert_eq!(run.reservation(), reserved); assert_eq!(run.evaluations(), 2);
    assert!(run.take_review().is_err()); assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn source_loss_before_scoring_or_before_reveal_stops_the_same_original_review() {
    for after_scoring in [false, true] {
        let (mut owner, endpoint, _) = owner(); step(&mut owner);
        let (_, sidecar) = propose(&mut owner, 1); let definitions = members(&sidecar, 0);
        let mut run = owner.begin_learned_probe_review(sidecar, schedule(), definitions, limits(), &snapshot()).unwrap();
        if after_scoring {
            owner.advance_learned_probe_review(&mut run, 0, ElapsedTick(1), &snapshot()).unwrap();
        }
        let before = run.records().clone(); let evaluations = run.evaluations();
        step(&mut owner);
        let revision = run.revision();
        assert!(owner.advance_learned_probe_review(&mut run, revision, ElapsedTick(1), &snapshot()).is_err());
        assert!(matches!(run.status(), LearnedWorkerStatus::Failed(_)));
        assert_eq!(run.records(), &before); assert_eq!(run.evaluations(), evaluations);
        assert_eq!(evaluations, if after_scoring { 2 } else { 0 });
        assert!(run.records().values().all(|r| !r.reveal_queued));
        assert!(run.take_review().is_err()); assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn expired_unstarted_workers_do_no_scoring_and_remain_missing_not_abstaining() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1); let definitions = members(&sidecar, 0);
    let mut run = owner.begin_learned_probe_review(sidecar, schedule(), definitions, limits(), &snapshot()).unwrap();
    owner.advance_learned_probe_review(&mut run, 0, ElapsedTick(10), &snapshot()).unwrap();
    if run.status() == LearnedWorkerStatus::Running {
        let revision = run.revision();
        owner.advance_learned_probe_review(&mut run, revision, ElapsedTick(15), &snapshot()).unwrap();
    }
    assert_eq!(run.status(), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Missing));
    assert_eq!(run.evaluations(), 0);
    assert!(run.records().values().all(|r| r.status.is_none() && !r.commitment_queued));
    let input = run.input().clone(); let review = run.take_review().unwrap();
    assert_eq!(review.missing(), &["alpha".to_owned(), "beta".to_owned()]);
    assert!(review.abstained().is_empty());
    owner.apply_review(review, Some(&input), &snapshot()).unwrap();
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn poll_exhaustion_preserves_computation_but_never_finishes_a_partial_vote() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1); let definitions = members(&sidecar, 0);
    let mut plan = schedule(); plan.polls = 1;
    let mut run = owner.begin_learned_probe_review(sidecar, plan, definitions, limits(), &snapshot()).unwrap();
    owner.advance_learned_probe_review(&mut run, 0, ElapsedTick(1), &snapshot()).unwrap();
    let before = run.records().clone();
    assert_eq!(owner.advance_learned_probe_review(&mut run, 1, ElapsedTick(1), &snapshot()), Err(Error::Limit));
    assert_eq!(run.status(), LearnedWorkerStatus::Failed(Error::Limit));
    assert_eq!(run.records(), &before); assert_eq!(run.evaluations(), 2);
    assert!(run.history().is_empty()); assert!(run.take_review().is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn refinement_cannot_refill_a_frozen_evaluation_disclosure_budget() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1); let definitions = members(&sidecar, 1);
    let mut bound = limits(); bound.per_evaluation.refinement_bytes = 0;
    let mut run = owner.begin_learned_probe_review(sidecar, schedule(), definitions, bound, &snapshot()).unwrap();
    owner.advance_learned_probe_review(&mut run, 0, ElapsedTick(1), &snapshot()).unwrap();
    assert_eq!(owner.advance_learned_probe_review(&mut run, 1, ElapsedTick(1), &snapshot()), Err(Error::Limit));
    assert_eq!(run.status(), LearnedWorkerStatus::Failed(Error::Limit));
    assert_eq!(run.history().len(), 1); assert_eq!(run.evaluations(), 2);
    assert!(run.records().values().all(|r| r.status == Some(ProbeHelperStatus::Judged(Verdict::Abstain))));
    assert_eq!(run.reservation().per_evaluation.refinement_bytes, 0);
    assert!(run.take_review().is_err()); assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn a_completed_quiet_driver_result_is_not_permission_after_source_advance() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1); let definitions = members(&sidecar, 0);
    let mut run = owner.begin_learned_probe_review(sidecar, schedule(), definitions, limits(), &snapshot()).unwrap();
    assert_eq!(drive(&mut owner, &mut run), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Decided));
    let records = run.records().clone(); let input = run.input().clone();
    step(&mut owner);
    assert!(owner.apply_review(run.take_review().unwrap(), Some(&input), &snapshot()).is_err());
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
    assert_eq!(run.records(), &records); assert_eq!(run.evaluations(), 2);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn a_finite_round_horizon_preserves_abstentions_instead_of_defaulting_to_allow() {
    let (mut owner, endpoint, _) = owner(); step(&mut owner); step(&mut owner);
    let (_, sidecar) = propose(&mut owner, 1); let mut definitions = members(&sidecar, 1);
    for definition in definitions.values_mut() { definition.salts.truncate(1); }
    let mut plan = schedule(); plan.rounds.truncate(1);
    let bound = ProbeReviewLimits { evaluations: 2, ..limits() };
    let mut run = owner.begin_learned_probe_review(sidecar, plan, definitions, bound, &snapshot()).unwrap();
    assert_eq!(drive(&mut owner, &mut run), LearnedWorkerStatus::Stopped(LearnedWorkerStop::RoundLimit));
    assert_eq!(run.evaluations(), 2); assert_eq!(run.history().len(), 1);
    let input = run.input().clone(); let review = run.take_review().unwrap();
    assert_eq!(review.abstained(), &["alpha".to_owned(), "beta".to_owned()]);
    assert!(review.missing().is_empty());
    owner.apply_review(review, Some(&input), &snapshot()).unwrap();
    assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}
