//! Real public native broker path shared by reference and file-adapter tests.
use fa_reference::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{TargetCeiling, containment::{ActorState, RestartGrade, RestartProfile}};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate, controller::ControllerConfig};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, ObservedReview, OversightBroker, ReviewWindow};
use fa_reference::action::consequence::oversight::credibility::{Assessment, EvaluationProtocol, Fraction, GroundTruth, IndependentEvaluator, SealedAssessment};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, AndonLedger, LedgerIdentity};
use fa_reference::Snapshot;
use std::collections::BTreeMap;

pub fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
pub fn identity() -> LedgerIdentity { LedgerIdentity { deployment: 101, evaluator: 202 } }
pub fn budget() -> AndonBudget {
    AndonBudget { window: 1, start: 0, end: 20, false_stops_allowed: 1, cost_per_stop: 7 }
}
pub fn accounting() -> AndonLedger {
    let mut ledger = AndonLedger::new(identity()).unwrap();
    ledger.register_window(budget()).unwrap();
    ledger
}
pub fn setup(hold_maximum: u64) -> (OversightBroker, IndependentEvaluator, PublicationEndpoint) {
    setup_with_period(hold_maximum, 3)
}
pub fn setup_with_period(hold_maximum: u64, period: u64) -> (OversightBroker, IndependentEvaluator, PublicationEndpoint) {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    let actor = ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
        model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
        grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap();
    let members = BTreeMap::from([("a".into(), MemberPolicy { cohort: "a".into(), weight: 10 }),
        ("b".into(), MemberPolicy { cohort: "b".into(), weight: 10 })]);
    let contracts = CommitteeContract::new([("a", 11), ("b", 22)].into_iter().map(|(name, generation)| (
        name.to_owned(), HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: vec![],
            tokenizer_epoch: 1, policy_epoch: 0, model_epoch: generation }, 1, b"approve?".to_vec()).unwrap(),
    )).collect()).unwrap();
    let mut endpoint = PublicationEndpoint::new(target, b"initial".to_vec(), 1000, 8).unwrap();
    let mut host = OversightBroker::new(ControllerConfig { scope: scope(), total: 100, max_attempts: 8,
        actor, suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members, caps: Caps { per_member: 10, per_cohort: 10 },
            continue_minimum: 5, continue_hold_maximum: hold_maximum, narrow_at: 20,
            suspend_at: 30, minimum_members: 2, minimum_cohorts: 2 },
        narrowed_targets: TargetCeiling::new(&[target]).unwrap() }, &mut endpoint, contracts).unwrap();
    host.confirm_fence(endpoint.install_fence(host.fence_request()).unwrap()).unwrap();
    let evaluator = host.enable_credibility(EvaluationProtocol { domain: 1, stratum: 2, period,
        minimum_violation_origins: 2, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 100 }).unwrap();
    (host, evaluator, endpoint)
}
pub fn prepare(host: &mut OversightBroker, id: u64, tick: u64, verdicts: [Verdict; 2])
    -> (ObservedReview, fa_reference::action::consequence::oversight::CommitteeInput, EvidenceSnapshot)
{
    let now = ElapsedTick(tick);
    host.observe_time(now).unwrap();
    let source = EvidenceSnapshot::new(EvidenceIdentity { source: 1, generation: id, scope: scope() },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() },
        BTreeMap::from([("a".into(), b"context".to_vec()), ("b".into(), b"context".to_vec())])).unwrap();
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    let action = host.propose(id, ActionSpec { version: VERSION, scope: scope(), target: Some(target),
        payload: b"output".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().ledger.epoch,
        deadline: ElapsedTick(1000), units: 16 }, source.snapshot()).unwrap().action;
    let inputs = source.inputs_for(&action, host.contracts()).unwrap();
    host.record_inputs(id, 0, inputs.clone()).unwrap();
    let mut session = host.begin_review(id, id + 100, source.reference_root(), ReviewWindow {
        commit_by: ElapsedTick(tick + 1), reveal_by: ElapsedTick(tick + 2) }, source.snapshot()).unwrap();
    for (name, verdict) in ["a", "b"].into_iter().zip(verdicts) {
        let digest = session.commitment(name, verdict, b"native-andon-salt").unwrap();
        session.commit(name, digest, now).unwrap();
    }
    session.open_reveals(now).unwrap();
    for (name, verdict) in ["a", "b"].into_iter().zip(verdicts) {
        session.reveal(name, verdict, b"native-andon-salt", now).unwrap();
    }
    (session.finish(now).unwrap(), inputs, source)
}
pub fn apply(host: &mut OversightBroker, id: u64, tick: u64, verdicts: [Verdict; 2]) {
    let (review, inputs, source) = prepare(host, id, tick, verdicts);
    host.apply_review(review, Some(&inputs), source.snapshot()).unwrap();
}
pub fn label(host: &OversightBroker, evaluator: &IndependentEvaluator, id: u64, truth: GroundTruth) -> SealedAssessment {
    let ticket = host.evaluation_ticket(id + 100).unwrap();
    evaluator.assess(&ticket, Assessment { origin: id, evidence_id: [2; 32], truth }).unwrap()
}
