//! The optional FA-062 gate is consumed by real review/permit/endpoint paths.
//! Fixtures exercise a bounded reference endpoint, not provider authentication.
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{EndpointOutcome, PublicationEndpoint};
use fa_reference::action::consequence::delivery::publication_gate::{PublicationInputs, PublicationLimits};
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{
    CommitteeContract, CommitteeInput, HelperContract, OversightBroker, ReviewWindow, action_frame,
};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewer, HumanReviewPolicy};
use fa_reference::action::consequence::oversight::publication::{PublicationJudgment, PublicationOutcome};
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, OpaqueJudgment, PartKind, SubmittedPart};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, QueryRole, SnapshotEntry, WitnessJudgment, WitnessRequest, WitnessSnapshot};
use fa_reference::witness::refinement::RefinementBudget;
use fa_reference::{Error, Snapshot};
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
fn spec() -> ActionSpec {
    ActionSpec { version: VERSION, scope: scope(), target: Some(target()), payload: b"visible".to_vec(),
        required_witnesses: vec![], policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }
}
fn limits() -> PublicationLimits {
    PublicationLimits { bindings: 16, validation: RefinementBudget { steps: 4096, value_bytes: 1_048_576 } }
}
fn structured(revision: u64, epoch: u64, keys: &[u64]) -> (WitnessSnapshot, ProductFrontiers) {
    let key = ProjectionKey { source: 1, branch: 4, projection: 3, source_epoch: 4 };
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    frontiers.record_close(marker).unwrap();
    let domain = AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker));
    (WitnessSnapshot::new(revision, revision * 2, epoch, domain, keys.iter().map(|key| {
        SnapshotEntry::new(*key, 1, b"abc".to_vec()).unwrap()
    }).collect()).unwrap(), frontiers)
}

struct Fixture {
    broker: OversightBroker,
    endpoint: PublicationEndpoint,
    action: FrozenAction,
    committee: CommitteeInput,
    evidence: PublicationInputs,
    reviewer: Option<HumanReviewer>,
}
impl Fixture {
    fn new(limits: Option<PublicationLimits>, human: bool) -> Self {
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
        if let Some(limits) = limits { broker.enable_publication_validation(limits).unwrap(); }
        let reviewer = human.then(|| broker.enable_human_review(HumanReviewPolicy {
            reviewer_id: 99, max_validity_ticks: 50, max_requests: 16,
        }).unwrap());
        broker.observe_time(ElapsedTick(1)).unwrap();
        endpoint.observe_time(ElapsedTick(1)).unwrap();
        let ack = endpoint.install_fence(broker.fence_request()).unwrap();
        broker.confirm_fence(ack).unwrap();
        let action = broker.propose(1, spec(), &snapshot()).unwrap().action;
        let helper = &contracts.members()["helper"];
        let mut bytes = action_frame(&action); let boundary = bytes.len(); bytes.extend_from_slice(helper.question());
        let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
            SubmittedPart { span: ByteSpan { start: boundary, end: bytes.len() }, kind: PartKind::Question },
        ], vec![]).unwrap();
        let manifest = EvidenceViewManifest::new(actual.clone(), AuthorizationProjection { projection_id: 9,
            policy_epoch: action.spec().policy_epoch, projected_originals: vec![] }, vec![]).unwrap();
        let committee = CommitteeInput::capture(&action, &contracts,
            BTreeMap::from([("helper".to_owned(), manifest)])).unwrap();
        broker.record_inputs(1, 0, committee.clone()).unwrap();
        let evidence = PublicationInputs { structured: Some(structured(10, 30, &[0, 2, 4])), opaque: Some(actual) };
        Self { broker, endpoint, action, committee, evidence, reviewer }
    }

    fn judgment(&self, action: FrozenAction) -> PublicationJudgment {
        let (snapshot, frontiers) = self.evidence.structured.as_ref().unwrap();
        let structured = WitnessJudgment::capture(snapshot, frontiers, vec![
            WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
            WitnessRequest::AbsentKey { key: 1 },
            WitnessRequest::EmptyRange { start: 6, end: 9 },
            WitnessRequest::RangeMembers { start: 2, end: 6 },
        ]).unwrap();
        PublicationJudgment::bind(action, Some(structured),
            Some(OpaqueJudgment::capture(self.evidence.opaque.as_ref().unwrap(), b"only one byte matters"))).unwrap()
    }

    fn bind(&mut self) {
        let judgment = self.judgment(self.action.clone());
        self.broker.bind_publication_judgment(1, judgment).unwrap();
        self.broker.record_publication_inputs(1, 0, Some(self.evidence.clone())).unwrap();
    }

    fn review(&mut self) {
        let now = ElapsedTick(1);
        let mut review = self.broker.begin_review(1, 101, [9; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, &snapshot()).unwrap();
        let commitment = review.commitment("helper", Verdict::Allow, b"salt").unwrap();
        review.commit("helper", commitment, now).unwrap(); review.open_reveals(now).unwrap();
        review.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
        self.broker.apply_review(review.finish(now).unwrap(), Some(&self.committee), &snapshot()).unwrap();
    }

    fn authorize(&mut self) -> Permit {
        self.broker.authorize(1, Some(&self.committee), &snapshot()).unwrap()
    }
}

#[test]
fn newer_unrelated_state_publishes_once_through_the_original_endpoint() {
    let mut f = Fixture::new(Some(limits()), false); f.bind(); f.review();
    let permit = f.authorize();
    let mut current = f.evidence.clone(); current.structured = Some(structured(11, 30, &[0, 2, 4, 99]));
    f.broker.record_publication_inputs(1, 1, Some(current)).unwrap();
    let envelope = f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap();
    assert_eq!(f.broker.publication_validation(1).unwrap().unwrap().outcome, PublicationOutcome::StillValid);
    let receipt = f.endpoint.deliver(&envelope).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(f.endpoint.deliver(&envelope).unwrap(), receipt);
    f.broker.accept_receipt(receipt).unwrap();
    assert_eq!(f.endpoint.payload(), b"visible"); assert_eq!(f.endpoint.execution_count(), 1);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert!(f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).is_err());
}

#[test]
fn final_cut_changes_cannot_spend_a_previously_valid_permit_or_refund_it() {
    for change in 0..7 {
        let mut f = Fixture::new(Some(limits()), false); f.bind(); f.review(); let permit = f.authorize();
        assert_eq!(f.broker.publication_validation(1).unwrap().unwrap().outcome, PublicationOutcome::StillValid);
        let mut current = f.evidence.clone();
        let expected = match change {
            0 => { current.structured = Some(structured(11, 30, &[2, 4])); Error::Stale }
            1 => { current.structured = Some(structured(11, 30, &[0, 1, 2, 4])); Error::Stale }
            2 => { current.structured = Some(structured(11, 30, &[0, 2, 4, 7])); Error::Stale }
            3 => { current.structured = Some(structured(11, 30, &[0, 2, 3, 4])); Error::Stale }
            4 => {
                current.structured.as_mut().unwrap().1 = ProductFrontiers::new(1, 8).unwrap();
                Error::Incomplete
            }
            5 => {
                let actual = current.opaque.take().unwrap();
                let mut profile = actual.input_profile().clone(); profile.tokenizer_epoch += 1;
                current.opaque = Some(ActualHelperInput::new(actual.submitted_bytes().to_vec(), profile,
                    actual.ordered_parts().to_vec(), actual.omissions().to_vec()).unwrap());
                Error::Stale
            }
            6 => { current.opaque = None; Error::Incomplete }
            _ => unreachable!(),
        };
        f.broker.record_publication_inputs(1, 1, Some(current)).unwrap();
        let before = f.broker.inspect();
        assert_eq!(f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap_err(), expected);
        assert_eq!(f.broker.inspect(), before);
        assert_eq!(before.ledger.reserved, 16); assert_eq!(before.ledger.charged, 0);
        assert_eq!(f.endpoint.payload(), b"old"); assert_eq!(f.endpoint.execution_count(), 0);
        assert!(f.broker.status_query(1).is_err());
    }
}

#[test]
fn enabling_the_gate_makes_missing_judgments_mandatory_not_optional() {
    let mut f = Fixture::new(Some(limits()), false); f.review();
    let before = f.broker.inspect();
    assert_eq!(f.broker.authorize(1, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(f.broker.inspect(), before);
    assert_eq!(f.broker.enable_publication_validation(limits()), Err(Error::Duplicate));
    let mut wrong = f.action.spec().clone(); wrong.payload.push(b'!');
    let wrong = f.judgment(FrozenAction::freeze(wrong).unwrap());
    assert_eq!(f.broker.bind_publication_judgment(1, wrong), Err(Error::Binding));
    f.bind();
    let duplicate = f.judgment(f.action.clone());
    assert_eq!(f.broker.bind_publication_judgment(1, duplicate), Err(Error::Duplicate));
    let _permit = f.authorize();
    let replacement = f.judgment(f.action.clone());
    assert_eq!(f.broker.bind_publication_judgment(1, replacement), Err(Error::WrongState));
}

#[test]
fn historical_success_is_not_reused_after_explicit_unavailability() {
    let mut f = Fixture::new(Some(limits()), false); f.bind(); f.review(); let permit = f.authorize();
    let historical = f.broker.publication_validation(1).unwrap().unwrap();
    f.broker.record_publication_inputs(1, 1, None).unwrap();
    assert_eq!(historical.outcome, PublicationOutcome::StillValid);
    assert_eq!(f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(f.broker.inspect().ledger.reserved, 16);
    f.broker.record_publication_inputs(1, 2, Some(f.evidence.clone())).unwrap();
    assert!(f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).is_ok());
}

#[test]
fn budget_exhaustion_keeps_work_visible_and_does_not_reserve_rights() {
    let mut ceiling = limits(); ceiling.validation.value_bytes = 0;
    let mut f = Fixture::new(Some(ceiling), false); f.bind(); f.review();
    let before = f.broker.inspect();
    assert_eq!(f.broker.authorize(1, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    let report = f.broker.publication_validation(1).unwrap().unwrap();
    assert!(matches!(report.outcome, PublicationOutcome::NeedsRefinement { .. }));
    assert!(report.spent.steps > 0); assert_eq!(report.spent.value_bytes, 0);
    assert_eq!(f.broker.inspect(), before); assert_eq!(before.ledger.reserved, 0);
    assert_eq!(f.broker.enable_publication_validation(limits()), Err(Error::Duplicate));
}

#[test]
fn input_cas_and_high_water_marks_survive_unavailability() {
    let mut f = Fixture::new(Some(limits()), false); f.bind();
    let mut later = f.evidence.clone(); later.structured = Some(structured(11, 31, &[0, 2, 4]));
    assert_eq!(f.broker.record_publication_inputs(1, 1, Some(later)), Ok(2));
    assert_eq!(f.broker.record_publication_inputs(1, 1, None), Err(Error::Stale));
    assert_eq!(f.broker.publication_input_revision(1), Ok(2));
    assert_eq!(f.broker.record_publication_inputs(1, 2, None), Ok(3));
    assert_eq!(f.broker.record_publication_inputs(1, 3, Some(f.evidence.clone())), Err(Error::Stale));
    let mut regressed_semantics = f.evidence.clone(); regressed_semantics.structured = Some(structured(12, 30, &[0, 2, 4]));
    assert_eq!(f.broker.record_publication_inputs(1, 3, Some(regressed_semantics)), Err(Error::Stale));
    assert_eq!(f.broker.publication_input_revision(1), Ok(3));
    let mut current = f.evidence.clone(); current.structured = Some(structured(12, 31, &[0, 2, 4]));
    assert_eq!(f.broker.record_publication_inputs(1, 3, Some(current)), Ok(4));
}

#[test]
fn known_and_unknown_dispatched_effects_reconcile_without_input_availability() {
    let mut f = Fixture::new(Some(limits()), false); f.bind(); f.review(); let permit = f.authorize();
    let envelope = f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap();
    let receipt = f.endpoint.deliver(&envelope).unwrap();
    f.broker.acknowledgment_lost(1).unwrap();
    f.broker.record_publication_inputs(1, 1, None).unwrap();
    f.broker.inputs_unavailable(1, 1).unwrap();
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(f.broker.cancel(1), Err(Error::WrongState));
    assert_eq!(f.broker.inspect().ledger.reserved, 0);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert_eq!(f.broker.accept_receipt(receipt), Ok(true));
    assert_eq!(f.broker.inspect().ledger.charged, 16);
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Confirmed);
}

#[test]
fn two_key_dispatch_cannot_bypass_the_gate_and_refusal_preserves_both_keys() {
    let mut f = Fixture::new(Some(limits()), true); f.bind(); f.review(); let permit = f.authorize();
    let request = f.broker.request_human_approval(1, 1, Some(&f.committee), ElapsedTick(40)).unwrap();
    let key = f.reviewer.as_ref().unwrap().approve(&request, ElapsedTick(1)).unwrap();
    f.broker.record_publication_inputs(1, 1, None).unwrap();
    assert_eq!(f.broker.dispatch_with_human(&permit, &key, &f.action, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(f.broker.human_status(1).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Authorized);
    f.broker.record_publication_inputs(1, 2, Some(f.evidence.clone())).unwrap();
    assert_eq!(f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    let envelope = f.broker.dispatch_with_human(&permit, &key, &f.action, Some(&f.committee), &snapshot()).unwrap();
    assert_eq!(f.broker.human_status(1).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(f.endpoint.deliver(&envelope).unwrap().outcome(), EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn capacity_is_checked_before_proposal_and_cannot_be_reopened_by_cancellation() {
    let mut ceiling = limits(); ceiling.bindings = 1;
    let mut f = Fixture::new(Some(ceiling), false);
    let before = f.broker.inspect();
    assert_eq!(f.broker.propose(2, spec(), &snapshot()).unwrap_err(), Error::Limit);
    assert_eq!(f.broker.inspect(), before);
    f.broker.cancel(1).unwrap();
    assert_eq!(f.broker.propose(2, spec(), &snapshot()).unwrap_err(), Error::Limit);
    let mut legacy = Fixture::new(None, false); legacy.review();
    assert_eq!(legacy.broker.enable_publication_validation(limits()), Err(Error::WrongState));
    let permit = legacy.authorize();
    assert!(legacy.broker.dispatch(&permit, &legacy.action, Some(&legacy.committee), &snapshot()).is_ok());
}
