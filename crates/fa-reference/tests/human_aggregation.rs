//! Public-path regressions for budgeted human review aggregation.
//! These use the actual broker, congress, helper capture and permit ledgers;
//! they do not simulate a deployed authenticated reviewer or durable co-signing.

use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanRequest,
    HumanReviewPolicy, HumanReviewWorkQueue, HumanWorkBudget, HumanWorkUsage, MAX_HUMAN_REQUESTS};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::{Error, ReadWitness, Snapshot};
use std::collections::BTreeMap;

fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}

fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }
}

fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}

fn budget() -> HumanWorkBudget {
    HumanWorkBudget { max_work_items: 16, max_members_per_work: 16, max_units_per_work: 100, max_total_units: 100 }
}

fn spec() -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"visible".to_vec(),
        required_witnesses: Vec::new(), policy_epoch: 0, deadline: ElapsedTick(100), units: 10 }
}

struct Prepared {
    id: u64,
    action: FrozenAction,
    inputs: CommitteeInput,
}

struct Fixture {
    broker: OversightBroker,
    _endpoint: PublicationEndpoint,
    contracts: CommitteeContract,
    work: HumanReviewWorkQueue,
}

impl Fixture {
    fn new(budget: HumanWorkBudget) -> Self {
        let contracts = CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"profile-v1".to_vec(), model_epoch: 1,
                tokenizer_epoch: 1, policy_epoch: 0 }, 9, b"Review".to_vec(),
        ).unwrap())])).unwrap();
        let config = ControllerConfig {
            scope: scope(), total: 100, max_attempts: 16,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("helper".to_owned(),
                MemberPolicy { cohort: "a".to_owned(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
                minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
        };
        let mut endpoint = PublicationEndpoint::new(target(), b"old".to_vec(), 200, 16).unwrap();
        let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
        let reviewer = broker.enable_human_review(HumanReviewPolicy {
            reviewer_id: 99, max_validity_ticks: 100, max_requests: 16,
        }).unwrap();
        let work = reviewer.into_work_queue(budget).unwrap();
        broker.observe_time(ElapsedTick(1)).unwrap();
        endpoint.observe_time(ElapsedTick(1)).unwrap();
        let acknowledgment = endpoint.install_fence(broker.fence_request()).unwrap();
        broker.confirm_fence(acknowledgment).unwrap();
        Self { broker, _endpoint: endpoint, contracts, work }
    }

    fn prepare(&mut self, id: u64, spec: ActionSpec) -> Prepared {
        let action = self.broker.propose(id, spec, &snapshot()).unwrap().action;
        let helper = &self.contracts.members()["helper"];
        let mut bytes = action_frame(&action);
        let boundary = bytes.len();
        bytes.extend_from_slice(helper.question());
        let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
            SubmittedPart { span: ByteSpan { start: boundary, end }, kind: PartKind::Question },
        ], Vec::new()).unwrap();
        let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
            projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new(),
        }, Vec::new()).unwrap();
        let inputs = CommitteeInput::capture(&action, &self.contracts,
            BTreeMap::from([("helper".to_owned(), manifest)])).unwrap();
        self.broker.record_inputs(id, 0, inputs.clone()).unwrap();
        let prepared = Prepared { id, action, inputs };
        self.review(&prepared, id + 100);
        prepared
    }

    fn review(&mut self, prepared: &Prepared, round: u64) {
        let now = self.broker.inspect().ledger.elapsed.unwrap();
        let mut review = self.broker.begin_review(prepared.id, round, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, &snapshot()).unwrap();
        let commitment = review.commitment("helper", Verdict::Allow, b"salt").unwrap();
        review.commit("helper", commitment, now).unwrap();
        review.open_reveals(now).unwrap();
        review.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
        self.broker.apply_review(review.finish(now).unwrap(), Some(&prepared.inputs), &snapshot()).unwrap();
    }

    fn request(&mut self, prepared: &Prepared) -> HumanRequest {
        self.broker.request_human_approval(prepared.id + 1000, prepared.id,
            Some(&prepared.inputs), ElapsedTick(90)).unwrap()
    }
}

#[test]
fn grouped_keys_use_original_dispatch_checks_and_cannot_be_swapped_or_reused() {
    let mut f = Fixture::new(budget());
    let first = f.prepare(1, spec());
    let second = f.prepare(2, spec());
    let r1 = f.request(&first);
    let r2 = f.request(&second);
    let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    assert_eq!(work.requests().iter().map(HumanRequest::id).collect::<Vec<_>>(), vec![r1.id(), r2.id()]);
    assert_eq!(work.total_units(), 20);
    let p1 = f.broker.authorize(first.id, Some(&first.inputs), &snapshot()).unwrap();
    let p2 = f.broker.authorize(second.id, Some(&second.inputs), &snapshot()).unwrap();
    assert!(matches!(f.broker.dispatch(&p1, &first.action, Some(&first.inputs), &snapshot()), Err(Error::Incomplete)));
    let keys = f.work.approve(&work, ElapsedTick(1)).unwrap();
    assert!(matches!(f.broker.dispatch_with_human(&p1, &keys[&r2.id()], &first.action,
        Some(&first.inputs), &snapshot()), Err(Error::Binding)));
    assert_eq!(f.broker.human_status(r2.id()).unwrap().disposition, HumanDisposition::Approved);
    f.broker.dispatch_with_human(&p1, &keys[&r1.id()], &first.action, Some(&first.inputs), &snapshot()).unwrap();
    assert_eq!(f.broker.human_status(r1.id()).unwrap().disposition, HumanDisposition::Consumed);
    assert!(f.broker.dispatch_with_human(&p1, &keys[&r1.id()], &first.action,
        Some(&first.inputs), &snapshot()).is_err());
    let reserved = f.broker.inspect().ledger.reserved;
    assert_eq!(reserved, 20);
    assert_eq!(f.work.revoke(&work, ElapsedTick(1)).unwrap().requests, vec![r2.id()]);
    assert_eq!(f.broker.human_status(r1.id()).unwrap().disposition, HumanDisposition::Consumed);
    assert!(matches!(f.broker.dispatch_with_human(&p2, &keys[&r2.id()], &second.action,
        Some(&second.inputs), &snapshot()), Err(Error::WrongState)));
    assert_eq!(f.broker.inspect().ledger.reserved, reserved);
    assert_eq!(f.broker.inspect().ledger.stages[&first.id], ActionState::Dispatching);
}

#[test]
fn late_arrivals_never_inherit_a_presented_or_approved_member_set() {
    let mut f = Fixture::new(budget());
    let first = f.prepare(1, spec());
    let second = f.prepare(2, spec());
    let late = f.prepare(3, spec());
    let r1 = f.request(&first);
    let r2 = f.request(&second);
    let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    let r3 = f.request(&late);
    let keys = f.work.approve(&work.clone(), ElapsedTick(1)).unwrap();
    assert_eq!(keys.keys().copied().collect::<Vec<_>>(), vec![r1.id(), r2.id()]);
    assert_eq!(f.broker.human_status(r3.id()).unwrap().disposition, HumanDisposition::Pending);
    let next = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    assert_eq!(next.requests().iter().map(HumanRequest::id).collect::<Vec<_>>(), vec![r3.id()]);
    assert!(matches!(f.work.approve(&work, ElapsedTick(1)), Err(Error::WrongState)));
}

#[test]
fn different_payloads_and_private_witnesses_do_not_share_review_work() {
    let mut f = Fixture::new(budget());
    let first = f.prepare(1, spec());
    let changed = f.prepare(2, ActionSpec { payload: b"other-effect".to_vec(), ..spec() });
    let witnessed = f.prepare(3, ActionSpec {
        required_witnesses: vec![ReadWitness::Exact { key: 7, value: Some(b"ok".to_vec()) }], ..spec()
    });
    let r1 = f.request(&first);
    let r2 = f.request(&changed);
    let r3 = f.request(&witnessed);
    for id in [r1.id(), r2.id(), r3.id()] {
        let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
        assert_eq!(work.requests().iter().map(HumanRequest::id).collect::<Vec<_>>(), vec![id]);
        assert_eq!(work.total_units(), 10);
    }
    assert!(f.work.next_work(ElapsedTick(1)).unwrap().is_none());
}

#[test]
fn identical_evidence_at_different_input_revisions_remains_separate() {
    let mut f = Fixture::new(budget());
    let first = f.prepare(1, spec());
    let second = f.prepare(2, spec());
    assert_eq!(first.inputs, second.inputs);
    let revision = f.broker.inputs_unavailable(second.id, 1).unwrap();
    assert_eq!(f.broker.record_inputs(second.id, revision, second.inputs.clone()).unwrap(), 3);
    f.review(&second, 999);
    let r1 = f.request(&first);
    let r2 = f.request(&second);
    assert_ne!(r1.input_revision(), r2.input_revision());
    let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    assert_eq!(work.requests().len(), 1);
    let next = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    assert_eq!(next.requests().iter().map(HumanRequest::id).collect::<Vec<_>>(), vec![r2.id()]);
}

#[test]
fn per_work_and_lifetime_budgets_count_each_effect_and_never_refund_rejections() {
    let mut f = Fixture::new(HumanWorkBudget { max_units_per_work: 10, max_total_units: 20, ..budget() });
    let first = f.prepare(1, spec());
    let second = f.prepare(2, spec());
    let third = f.prepare(3, spec());
    f.request(&first);
    f.request(&second);
    let r3 = f.request(&third);
    for _ in 0..2 {
        let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
        assert_eq!(work.requests().len(), 1);
        f.work.reject(&work, ElapsedTick(1)).unwrap();
        assert!(matches!(f.work.approve(&work, ElapsedTick(1)), Err(Error::WrongState)));
    }
    assert_eq!(f.work.usage(), HumanWorkUsage { work_items: 2, requests: 2, total_units: 20 });
    assert!(matches!(f.work.next_work(ElapsedTick(1)), Err(Error::Limit)));
    assert_eq!(f.broker.human_status(r3.id()).unwrap().disposition, HumanDisposition::Pending);
    assert_eq!(f.work.revoke_all(ElapsedTick(1)).unwrap().requests, vec![r3.id()]);
    assert_eq!(f.work.usage().total_units, 20);
}

#[test]
fn work_item_and_member_limits_apply_even_with_excess_unit_budget() {
    let mut f = Fixture::new(HumanWorkBudget { max_work_items: 1, max_members_per_work: 1, ..budget() });
    let first = f.prepare(1, spec());
    let second = f.prepare(2, spec());
    f.request(&first);
    let r2 = f.request(&second);
    let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    assert_eq!(work.requests().len(), 1);
    f.work.reject(&work, ElapsedTick(1)).unwrap();
    assert!(matches!(f.work.next_work(ElapsedTick(1)), Err(Error::Limit)));
    assert_eq!(f.work.revoke_all(ElapsedTick(1)).unwrap().requests, vec![r2.id()]);
}

#[test]
fn an_oversized_request_fails_closed_without_spending_presentation_budget() {
    let mut f = Fixture::new(HumanWorkBudget { max_units_per_work: 9, ..budget() });
    let first = f.prepare(1, spec());
    let request = f.request(&first);
    assert!(matches!(f.work.next_work(ElapsedTick(1)), Err(Error::Limit)));
    assert_eq!(f.work.usage(), HumanWorkUsage { work_items: 0, requests: 0, total_units: 0 });
    assert_eq!(f.broker.human_status(request.id()).unwrap().disposition, HumanDisposition::Pending);
    assert_eq!(f.work.revoke_all(ElapsedTick(1)).unwrap().requests, vec![request.id()]);
}

#[test]
fn foreign_work_with_identical_ids_and_evidence_cannot_issue_any_keys() {
    let mut owner = Fixture::new(budget());
    let mut foreign = Fixture::new(budget());
    let first = owner.prepare(1, spec());
    let second = foreign.prepare(1, spec());
    owner.request(&first);
    foreign.request(&second);
    let own_work = owner.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    let foreign_work = foreign.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    assert_eq!(own_work.id(), foreign_work.id());
    assert!(matches!(owner.work.approve(&foreign_work, ElapsedTick(1)), Err(Error::Binding)));
    assert_eq!(owner.work.reject(&foreign_work, ElapsedTick(1)), Err(Error::Binding));
    assert_eq!(owner.work.revoke(&foreign_work, ElapsedTick(1)), Err(Error::Binding));
    assert_eq!(owner.work.statuses(&own_work).unwrap()[0].disposition, HumanDisposition::Pending);
}

#[test]
fn expiry_and_clock_regression_do_not_partially_approve_work() {
    let mut f = Fixture::new(budget());
    let first = f.prepare(1, spec());
    let second = f.prepare(2, spec());
    f.request(&first);
    f.request(&second);
    let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    assert!(matches!(f.work.approve(&work, ElapsedTick(0)), Err(Error::Stale)));
    assert!(matches!(f.work.approve(&work, ElapsedTick(90)), Err(Error::Stale)));
    assert!(f.work.statuses(&work).unwrap().iter().all(|status| status.disposition == HumanDisposition::Pending));
    assert!(matches!(f.work.approve(&work, ElapsedTick(89)), Err(Error::Stale)));
    assert_eq!(f.work.revoke_all(ElapsedTick(90)).unwrap().requests.len(), 2);
}

#[test]
fn expired_unpresented_requests_are_not_eligible_work() {
    let mut f = Fixture::new(budget());
    let first = f.prepare(1, spec());
    f.request(&first);
    assert!(f.work.next_work(ElapsedTick(90)).unwrap().is_none());
    assert_eq!(f.work.usage().work_items, 0);
}

#[test]
fn evidence_loss_after_group_approval_blocks_dispatch_without_spending_the_key() {
    let mut f = Fixture::new(budget());
    let first = f.prepare(1, spec());
    let request = f.request(&first);
    let permit = f.broker.authorize(first.id, Some(&first.inputs), &snapshot()).unwrap();
    let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    let keys = f.work.approve(&work, ElapsedTick(1)).unwrap();
    let reserved = f.broker.inspect().ledger.reserved;
    f.broker.inputs_unavailable(first.id, 1).unwrap();
    assert!(f.broker.dispatch_with_human(&permit, &keys[&request.id()], &first.action,
        Some(&first.inputs), &snapshot()).is_err());
    assert_eq!(f.broker.human_status(request.id()).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(f.broker.inspect().ledger.reserved, reserved);
    assert_eq!(f.work.revoke(&work, ElapsedTick(1)).unwrap().requests, vec![request.id()]);
}

#[test]
fn epoch_revocation_is_not_bypassed_by_group_approval() {
    let mut f = Fixture::new(budget());
    let first = f.prepare(1, spec());
    let request = f.request(&first);
    let permit = f.broker.authorize(first.id, Some(&first.inputs), &snapshot()).unwrap();
    let work = f.work.next_work(ElapsedTick(1)).unwrap().unwrap();
    let keys = f.work.approve(&work, ElapsedTick(1)).unwrap();
    f.broker.revoke_epoch().unwrap();
    assert!(f.broker.dispatch_with_human(&permit, &keys[&request.id()], &first.action,
        Some(&first.inputs), &snapshot()).is_err());
    assert_eq!(f.broker.human_status(request.id()).unwrap().disposition, HumanDisposition::Approved);
}

#[test]
fn work_budget_validation_rejects_unbounded_and_zero_profiles() {
    assert_eq!(budget().validate(), Ok(()));
    for invalid in [
        HumanWorkBudget { max_work_items: 0, ..budget() },
        HumanWorkBudget { max_members_per_work: 0, ..budget() },
        HumanWorkBudget { max_units_per_work: 0, ..budget() },
        HumanWorkBudget { max_total_units: 0, ..budget() },
    ] {
        assert_eq!(invalid.validate(), Err(Error::InvalidInput));
    }
    assert_eq!(HumanWorkBudget { max_work_items: MAX_HUMAN_REQUESTS + 1, ..budget() }.validate(), Err(Error::Limit));
    assert_eq!(HumanWorkBudget { max_members_per_work: MAX_HUMAN_REQUESTS + 1, ..budget() }.validate(), Err(Error::Limit));
}
