//! Actual learned inference and original worker ports; deterministic ballots
//! are protocol controls, not authenticated model inference or safety evidence.
#[path = "support/restart_model.rs"]
pub mod fixture;
use fa_reference::{Error, Snapshot};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::{Consequence, congress::{CongressPolicy, MemberPolicy}};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{TargetCeiling, containment::{ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate, controller::ControllerConfig}}};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, OversightBroker, ReviewWindow,
    decoder_monitoring::LearnedDecoderBindingLimits,
    helper_workers::{HelperLimits, HelperPhase, HelperPort},
    human::{HumanReviewer, HumanReviewPolicy},
    learned_host::sidecar::{LearnedSidecar, LearnedSidecarRequest,
        workers::{LearnedWorkerReview, LearnedWorkerRound, LearnedWorkerSchedule, LearnedWorkerStatus,
            LearnedWorkerStop, LearnedWorkerUpdate}},
    learned_source::LearnedSourceConfig,
    sidecar::{SidecarCongressBudget, SidecarIdentity}};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart,
    monitored::{GenerationBudget, GenerationSpec, GenerationTelemetryBudget}};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use std::collections::{BTreeMap, BTreeSet};

fn scope() -> Scope { Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect } }
fn target() -> ResolvedTarget { ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 } }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn setup(human: bool, disclosure: SidecarCongressBudget)
    -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>, FrozenAction, LearnedSidecar)
{
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 3, tokenizer_generation: 4, state_schema_generation: 1, grade: RestartGrade::AuditOnly },
        Vec::new(), vec![0], vec![0], 0).unwrap();
    let names = ["alice", "bob"];
    let contract = CommitteeContract::new(names.into_iter().map(|name| (name.to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(), tokenizer_epoch: 4,
            policy_epoch: 0, model_epoch: 3 }, 1, format!("{name}: approve?").into_bytes()).unwrap())).collect()).unwrap();
    let mut endpoint = PublicationEndpoint::new(target(), b"initial".to_vec(), 1000, 8).unwrap();
    let mut owner = OversightBroker::new(ControllerConfig { scope: scope(), total: 100, max_attempts: 8,
        actor, suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: names.into_iter().map(|name| (name.to_owned(),
            MemberPolicy { cohort: name.to_owned(), weight: 1 })).collect(),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 2, continue_hold_maximum: 0,
            narrow_at: 3, suspend_at: 4, minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    }, &mut endpoint, contract).unwrap();
    let human = human.then(|| owner.enable_human_review(HumanReviewPolicy {
        reviewer_id: 55, max_validity_ticks: 100, max_requests: 8 }).unwrap());
    let model = fixture::model();
    let config = LearnedSourceConfig { stream: 21, evaluation_origin: 201, monitor_generation: 31,
        spec: GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
            policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173 }).unwrap(),
        policy: fixture::policy(&model, 0, 1), budget: GenerationBudget::default(), telemetry: GenerationTelemetryBudget::default() };
    owner.own_learned_generation(model, config, LearnedDecoderBindingLimits::default()).unwrap();
    owner.enable_learned_sidecar_requirement().unwrap();
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    advance_model(&mut owner);
    let action = owner.propose(1, ActionSpec { version: VERSION, scope: scope(), target: Some(target()),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(), policy_epoch: owner.inspect().ledger.epoch,
        deadline: ElapsedTick(100), units: 16 }, &snapshot()).unwrap().action;
    let priority = owner.learned_decoder_evidence(1).unwrap().unwrap().audit().source().groups().collect();
    let sidecar = owner.begin_learned_sidecar(1, owner.actor_revision(), LearnedSidecarRequest {
        identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 }, priority, budget: disclosure }).unwrap();
    (owner, endpoint, human, action, sidecar)
}
fn advance_model(owner: &mut OversightBroker) {
    let state = owner.hosted_learned_generation().unwrap();
    owner.advance_hosted_learned(state.actor_revision, state.position).unwrap();
}
fn schedule(count: usize) -> LearnedWorkerSchedule {
    LearnedWorkerSchedule { rounds: (0..count).map(|i| LearnedWorkerRound { round: 101 + i as u64,
        evidence_root: [i as u8 + 7; 32], window: ReviewWindow {
            commit_by: ElapsedTick(10 + i as u64 * 20), reveal_by: ElapsedTick(20 + i as u64 * 20) } }).collect(),
        helpers: HelperLimits::default(), polls: 64 }
}
fn poll(owner: &mut OversightBroker, run: &mut LearnedWorkerReview, now: u64) -> Result<LearnedWorkerUpdate, Error> {
    let revision = run.revision(); owner.advance_learned_worker_review(run, revision, ElapsedTick(now), &snapshot())
}
fn commit(ports: &BTreeMap<String, HelperPort>, verdict: Verdict) {
    for port in ports.values() {
        let digest = port.request().commitment(verdict, b"independent-reference-salt").unwrap();
        port.submit_commitment(digest).unwrap();
    }
}
fn reveal(ports: &BTreeMap<String, HelperPort>, verdict: Verdict) {
    for port in ports.values() { port.reveal(verdict, b"independent-reference-salt").unwrap(); }
}
fn vote(owner: &mut OversightBroker, run: &mut LearnedWorkerReview, ports: &BTreeMap<String, HelperPort>, verdict: Verdict)
    -> LearnedWorkerUpdate
{
    commit(ports, verdict);
    assert!(matches!(poll(owner, run, 1).unwrap(), LearnedWorkerUpdate::Waiting));
    assert!(ports.values().all(|port| port.phase() == HelperPhase::ReadyReveal));
    reveal(ports, verdict);
    poll(owner, run, 1).unwrap()
}

#[test]
fn independent_commits_refine_once_then_fresh_workers_and_two_keys_publish() {
    let (mut owner, mut endpoint, human, action, sidecar) = setup(true, SidecarCongressBudget::default());
    let input = sidecar.round().input().clone();
    let numerical = owner.hosted_learned_generation().unwrap();
    let (mut run, first) = owner.begin_learned_worker_review(sidecar, schedule(2), &snapshot()).unwrap();
    for (name, port) in &first {
        assert_eq!(port.request().member(), name);
        assert_eq!(port.request().view(), &input.views()[name]);
        assert_eq!(port.reveal(Verdict::Allow, b"early"), Err(Error::WrongState));
    }
    // Alice's vote cannot release Bob or leak another worker's verdict.
    first["alice"].submit_commitment(first["alice"].request().commitment(Verdict::Abstain,
        b"independent-reference-salt").unwrap()).unwrap();
    poll(&mut owner, &mut run, 1).unwrap();
    assert_eq!(first["alice"].phase(), HelperPhase::AwaitReveal);
    assert_eq!(first["bob"].phase(), HelperPhase::AwaitCommit);
    first["bob"].submit_commitment(first["bob"].request().commitment(Verdict::Abstain,
        b"independent-reference-salt").unwrap()).unwrap();
    poll(&mut owner, &mut run, 1).unwrap(); reveal(&first, Verdict::Abstain);
    let next = match poll(&mut owner, &mut run, 1).unwrap() { LearnedWorkerUpdate::NextRound(ports) => ports, other => panic!("{other:?}") };
    assert_eq!(run.history().len(), 1); assert_eq!(run.input_revision(), 2);
    assert_ne!(run.input(), &input);
    assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
    assert_eq!(first["alice"].submit_commitment(0), Err(Error::WrongState));
    assert_eq!(next["alice"].request().round(), 102);
    assert_eq!(next["alice"].request().view(), &run.input().views()["alice"]);
    assert!(owner.authorize(1, Some(run.input()), &snapshot()).is_err());
    assert!(matches!(vote(&mut owner, &mut run, &next, Verdict::Allow), LearnedWorkerUpdate::Stopped));
    assert_eq!(run.status(), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Decided));
    assert_eq!(run.history().len(), 2);
    let current = run.input().clone(); let review = run.take_review().unwrap();
    assert_eq!(review.decision().consequence, Consequence::Continue);
    assert!(run.take_review().is_err());
    assert!(owner.authorize(1, Some(&current), &snapshot()).is_err());
    owner.apply_review(review, Some(&current), &snapshot()).unwrap();
    let permit = owner.authorize(1, Some(&current), &snapshot()).unwrap();
    assert!(owner.dispatch(&permit, &action, Some(&current), &snapshot()).is_err());
    let request = owner.request_human_approval(501, 1, Some(&current), ElapsedTick(80)).unwrap();
    let key = human.unwrap().approve(&request, ElapsedTick(1)).unwrap();
    let message = owner.dispatch_with_human(&permit, &key, &action, Some(&current), &snapshot()).unwrap();
    owner.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"visible"); assert_eq!(endpoint.execution_count(), 1);
    assert!(owner.dispatch_with_human(&permit, &key, &action, Some(&current), &snapshot()).is_err());
}

#[test]
fn disconnected_and_bad_reveals_remain_missing_without_a_retry_round() {
    for malformed in [false, true] {
        let (mut owner, endpoint, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
        let (mut run, ports) = owner.begin_learned_worker_review(sidecar, schedule(2), &snapshot()).unwrap();
        commit(&ports, Verdict::Allow); poll(&mut owner, &mut run, 1).unwrap();
        ports["alice"].reveal(Verdict::Allow, b"independent-reference-salt").unwrap();
        if malformed { ports["bob"].reveal(Verdict::Deny, b"wrong").unwrap(); } else { ports["bob"].disconnect(); }
        assert!(matches!(poll(&mut owner, &mut run, 1).unwrap(), LearnedWorkerUpdate::Waiting));
        assert!(matches!(poll(&mut owner, &mut run, 20).unwrap(), LearnedWorkerUpdate::Stopped));
        assert_eq!(run.status(), LearnedWorkerStatus::Stopped(LearnedWorkerStop::Missing));
        assert_eq!(run.input_revision(), 1); assert_eq!(run.history().len(), 1);
        assert!(run.worker_statuses()["bob"].failure.is_some());
        let review = run.take_review().unwrap(); assert_eq!(review.missing(), &["bob".to_owned()]);
        assert_ne!(review.decision().consequence, Consequence::Continue);
        owner.apply_review(review, Some(run.input()), &snapshot()).unwrap();
        assert!(owner.authorize(1, Some(run.input()), &snapshot()).is_err());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn schedule_and_disclosure_exhaustion_keep_the_original_abstention_nonpermitting() {
    for short_schedule in [false, true] {
        let budget = if short_schedule { SidecarCongressBudget::default() }
            else { SidecarCongressBudget { rounds: 1, ..SidecarCongressBudget::default() } };
        let (mut owner, endpoint, _, _, sidecar) = setup(false, budget);
        let bytes = owner.captured_input_bytes();
        let (mut run, ports) = owner.begin_learned_worker_review(sidecar, schedule(if short_schedule {1} else {2}), &snapshot()).unwrap();
        vote(&mut owner, &mut run, &ports, Verdict::Abstain);
        let stop = if short_schedule { LearnedWorkerStop::RoundLimit } else { LearnedWorkerStop::RefinementBudget };
        assert_eq!(run.status(), LearnedWorkerStatus::Stopped(stop));
        assert_eq!(run.input_revision(), 1); assert_eq!(owner.captured_input_bytes(), bytes);
        let review = run.take_review().unwrap(); assert_eq!(review.abstained().len(), 2);
        assert_ne!(review.decision().consequence, Consequence::Continue);
        assert_eq!(poll(&mut owner, &mut run, 1).err(), Some(Error::WrongState));
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn stale_calls_and_foreign_brokers_are_free_but_source_changes_close_the_round() {
    let (mut owner, _, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
    let (mut other, _, _, _, _) = setup(false, SidecarCongressBudget::default());
    let (mut run, ports) = owner.begin_learned_worker_review(sidecar, schedule(2), &snapshot()).unwrap();
    assert_eq!(owner.advance_learned_worker_review(&mut run, 9, ElapsedTick(1), &snapshot()).err(), Some(Error::Stale));
    assert_eq!(other.advance_learned_worker_review(&mut run, 0, ElapsedTick(1), &snapshot()).err(), Some(Error::Binding));
    assert_eq!(poll(&mut owner, &mut run, 0).err(), Some(Error::Stale));
    assert_eq!(run.revision(), 0); assert_eq!(run.polls(), 0);
    assert!(matches!(poll(&mut owner, &mut run, 1).unwrap(), LearnedWorkerUpdate::Waiting));
    advance_model(&mut owner);
    assert_eq!(poll(&mut owner, &mut run, 1).err(), Some(Error::Stale));
    assert_eq!(run.status(), LearnedWorkerStatus::Failed(Error::Stale));
    assert!(ports.values().all(|p| p.phase() == HelperPhase::Closed));
    assert!(run.take_review().is_err());
    assert_eq!(poll(&mut owner, &mut run, 1).err(), Some(Error::WrongState));
}

#[test]
fn polling_budget_cancel_and_drop_close_original_ports_without_changing_rights() {
    for mode in 0..3 {
        let (mut owner, _, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
        let bytes = owner.captured_input_bytes();
        let mut selection = schedule(2); selection.polls = 1;
        let (mut run, ports) = owner.begin_learned_worker_review(sidecar, selection, &snapshot()).unwrap();
        let state = owner.inspect();
        poll(&mut owner, &mut run, 1).unwrap();
        match mode {
            0 => { assert_eq!(poll(&mut owner, &mut run, 1).err(), Some(Error::Limit)); assert_eq!(run.polls(), 1); }
            1 => { assert_eq!(run.cancel(0), Err(Error::Stale)); run.cancel(1).unwrap(); assert_eq!(run.status(), LearnedWorkerStatus::Cancelled); }
            _ => { drop(run); }
        }
        assert!(ports.values().all(|p| p.phase() == HelperPhase::Closed));
        assert_eq!(owner.inspect().ledger, state.ledger);
        assert_eq!(owner.captured_input_bytes(), bytes);
    }
}

#[test]
fn malformed_schedules_and_helper_admission_fail_before_any_round_is_started() {
    for mode in 0..6 {
        let (mut owner, _, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
        let mut bad = schedule(2);
        match mode {
            0 => bad.rounds[1].round = bad.rounds[0].round,
            1 => bad.rounds[1].window.commit_by = ElapsedTick(20),
            2 => bad.rounds[1].evidence_root = [0; 32],
            3 => bad.polls = 0,
            4 => bad.helpers.members = 1,
            _ => bad.helpers.input_bytes = sidecar.round().input().logical_bytes() - 1,
        }
        assert!(owner.begin_learned_worker_review(sidecar, bad, &snapshot()).is_err());
        // The same round ID is still available to the original session; no
        // private tracker edits or fake reset were needed for the valid control.
        assert!(owner.begin_review(1, 101, [7; 32], schedule(1).rounds[0].window, &snapshot()).is_ok());
    }
}

#[test]
fn completed_worker_allow_cannot_survive_later_input_replacement_or_source_loss() {
    for source_loss in [false, true] {
        let (mut owner, endpoint, _, _, sidecar) = setup(false, SidecarCongressBudget::default());
        let (mut run, ports) = owner.begin_learned_worker_review(sidecar, schedule(1), &snapshot()).unwrap();
        vote(&mut owner, &mut run, &ports, Verdict::Allow);
        let input = run.input().clone(); let review = run.take_review().unwrap();
        if source_loss { advance_model(&mut owner); }
        else { owner.inputs_unavailable(1, run.input_revision()).unwrap(); }
        assert!(owner.apply_review(review, Some(&input), &snapshot()).is_err());
        assert!(owner.authorize(1, Some(&input), &snapshot()).is_err());
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[cfg(unix)]
#[path = "learned_worker_review/transport.rs"]
mod transport;
