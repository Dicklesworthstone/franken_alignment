//! Cooperative screening uses the original native oversight owner and endpoint.
//! The final publication gate, source captures and both keys remain independent.
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
use fa_reference::action::consequence::oversight::publication::{PublicationJudgment, PublicationOutcome, PublicationProgress};
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
fn structured(revision: u64, keys: &[u64]) -> (WitnessSnapshot, ProductFrontiers) {
    let key = ProjectionKey { source: 1, branch: 4, projection: 3, source_epoch: 4 };
    let mut frontiers = ProductFrontiers::new(1, 8).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    frontiers.record_close(marker).unwrap();
    let domain = AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker));
    (WitnessSnapshot::new(revision, revision * 2, 30, domain, keys.iter().map(|key| {
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
        let evidence = PublicationInputs { structured: Some(structured(10, &[0, 2, 4])), opaque: Some(actual) };
        Self { broker, endpoint, action, committee, evidence, reviewer }
    }
    fn bind(&mut self) {
        let (snapshot, frontiers) = self.evidence.structured.as_ref().unwrap();
        let structured = WitnessJudgment::capture(snapshot, frontiers, vec![
            WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
            WitnessRequest::AbsentKey { key: 1 },
            WitnessRequest::EmptyRange { start: 6, end: 9 },
            WitnessRequest::RangeMembers { start: 2, end: 6 },
        ]).unwrap();
        let judgment = PublicationJudgment::bind(self.action.clone(), Some(structured),
            Some(OpaqueJudgment::capture(self.evidence.opaque.as_ref().unwrap(), b"only one byte matters"))).unwrap();
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
    fn precheck(&self) -> PublicationProgress {
        let revision = self.broker.publication_input_revision(1).unwrap();
        let mut cursor = self.broker.begin_publication_precheck(1, revision).unwrap();
        assert_eq!(cursor.action(), &self.action);
        for _ in 0..10_000 {
            let before = cursor.total_work();
            let report = cursor.advance(RefinementBudget { steps: 1, value_bytes: 1 });
            assert!(report.spent.steps <= 1 && report.spent.value_bytes <= 1);
            assert_eq!(report.total.steps, before.steps + report.spent.steps);
            assert_eq!(report.total.value_bytes, before.value_bytes + report.spent.value_bytes);
            if !matches!(report.outcome, PublicationOutcome::NeedsRefinement { .. }) { return report; }
            assert!(report.spent.steps > 0);
        }
        panic!("native precheck did not terminate");
    }
}

#[test]
fn native_precheck_does_not_grant_either_key_but_valid_review_still_publishes() {
    let mut f = Fixture::new(Some(limits()), true); f.bind();
    let before = f.broker.inspect();
    assert_eq!(f.precheck().outcome, PublicationOutcome::StillValid);
    assert_eq!(f.broker.inspect(), before);
    assert_eq!(f.broker.publication_validation(1).unwrap(), None);
    assert_eq!(f.broker.authorize(1, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    f.review(); let permit = f.authorize();
    assert_eq!(f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    let request = f.broker.request_human_approval(1, 1, Some(&f.committee), ElapsedTick(40)).unwrap();
    let key = f.reviewer.as_ref().unwrap().approve(&request, ElapsedTick(1)).unwrap();
    let last = f.broker.publication_validation(1).unwrap();
    assert_eq!(f.precheck().outcome, PublicationOutcome::StillValid);
    assert_eq!(f.broker.publication_validation(1).unwrap(), last);
    assert_eq!(f.broker.human_status(1).unwrap().disposition, HumanDisposition::Approved);
    let envelope = f.broker.dispatch_with_human(&permit, &key, &f.action, Some(&f.committee), &snapshot()).unwrap();
    let receipt = f.endpoint.deliver(&envelope).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    f.broker.accept_receipt(receipt).unwrap();
    assert_eq!(f.endpoint.payload(), b"visible"); assert_eq!(f.endpoint.execution_count(), 1);
    assert_eq!(f.broker.human_status(1).unwrap().disposition, HumanDisposition::Consumed);
    assert_eq!(f.broker.inspect().ledger.charged, 16);
}

#[test]
fn a_successful_precheck_cannot_hide_late_changes_from_original_two_key_dispatch() {
    for mutation in 0..6 {
        let mut f = Fixture::new(Some(limits()), true); f.bind(); f.review();
        let permit = f.authorize();
        let request = f.broker.request_human_approval(1, 1, Some(&f.committee), ElapsedTick(40)).unwrap();
        let key = f.reviewer.as_ref().unwrap().approve(&request, ElapsedTick(1)).unwrap();
        let successful = f.precheck(); assert_eq!(successful.outcome, PublicationOutcome::StillValid);
        let mut changed = f.evidence.clone();
        match mutation {
            0 => changed.structured = Some(structured(11, &[0, 1, 2, 4])),
            1 => changed.structured = Some(structured(11, &[0, 2, 4, 7])),
            2 => changed.structured = Some(structured(11, &[0, 2, 3, 4])),
            3 => changed.structured.as_mut().unwrap().1 = ProductFrontiers::new(1, 8).unwrap(),
            4 => {
                let old = changed.opaque.take().unwrap(); let mut bytes = old.submitted_bytes().to_vec();
                *bytes.last_mut().unwrap() ^= 1;
                changed.opaque = Some(ActualHelperInput::new(bytes, old.input_profile().clone(),
                    old.ordered_parts().to_vec(), old.omissions().to_vec()).unwrap());
            }
            5 => changed.opaque = None,
            _ => unreachable!(),
        }
        f.broker.record_publication_inputs(1, 1, Some(changed)).unwrap();
        let before = f.broker.inspect();
        let expected = if matches!(mutation, 3 | 5) { Error::Incomplete } else { Error::Stale };
        assert_eq!(f.broker.dispatch_with_human(&permit, &key, &f.action, Some(&f.committee), &snapshot()).unwrap_err(), expected);
        assert_ne!(f.precheck().outcome, PublicationOutcome::StillValid);
        assert_eq!(f.broker.inspect(), before);
        assert_eq!(f.broker.human_status(1).unwrap().disposition, HumanDisposition::Approved);
        assert_eq!(f.broker.inspect().ledger.stages[&1], ActionState::Authorized);
        assert_eq!(f.endpoint.execution_count(), 0);
        // Restore original facts at a NEW cut; neither unspent key was lost.
        let mut restored = f.evidence.clone(); restored.structured = Some(structured(12, &[0, 2, 4, 99]));
        f.broker.record_publication_inputs(1, 2, Some(restored)).unwrap();
        assert_eq!(f.precheck().outcome, PublicationOutcome::StillValid);
        let envelope = f.broker.dispatch_with_human(&permit, &key, &f.action, Some(&f.committee), &snapshot()).unwrap();
        assert_eq!(f.endpoint.deliver(&envelope).unwrap().outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    }
}

#[test]
fn readonly_screening_neither_consumes_nor_refreshes_single_boundary_source_captures() {
    let mut f = Fixture::new(Some(limits()), false); f.bind();
    f.broker.bind_publication_source(1, 70, 1, f.evidence.clone()).unwrap();
    assert_eq!(f.precheck().outcome, PublicationOutcome::Refused(Error::Incomplete));
    f.broker.record_publication_inputs(1, 1, None).unwrap();
    f.broker.record_captured_publication_inputs(1, 2, 70, 1, f.evidence.clone()).unwrap();
    let fresh = f.broker.publication_source(1).unwrap();
    assert!(fresh.unwrap().fresh);
    assert_eq!(f.precheck().outcome, PublicationOutcome::StillValid);
    assert_eq!(f.broker.publication_source(1).unwrap(), fresh);
    f.review(); let permit = f.authorize();
    let consumed = f.broker.publication_source(1).unwrap(); assert!(!consumed.unwrap().fresh);
    // Matching retained bytes do not purport to be a fresh capture.
    assert_eq!(f.precheck().outcome, PublicationOutcome::StillValid);
    assert_eq!(f.broker.publication_source(1).unwrap(), consumed);
    assert_eq!(f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(f.broker.inspect().ledger.reserved, 16);
    assert_eq!(f.endpoint.execution_count(), 0);
    f.broker.record_publication_inputs(1, 3, None).unwrap();
    f.broker.record_captured_publication_inputs(1, 4, 70, 1, f.evidence.clone()).unwrap();
    let envelope = f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap();
    assert_eq!(f.endpoint.deliver(&envelope).unwrap().outcome(), EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn exact_predecessors_required_bindings_and_fixed_ceilings_are_not_caller_overrides() {
    let legacy = Fixture::new(None, false);
    assert!(matches!(legacy.broker.begin_publication_precheck(1, 0), Err(Error::Incomplete)));
    let mut f = Fixture::new(Some(limits()), false);
    assert!(matches!(f.broker.begin_publication_precheck(1, 0), Err(Error::Incomplete)));
    f.bind();
    assert!(matches!(f.broker.begin_publication_precheck(1, 0), Err(Error::Stale)));
    assert!(matches!(f.broker.begin_publication_precheck(2, 1), Err(Error::Missing)));
    let mut ceiling = limits(); ceiling.validation.value_bytes = 0;
    let mut f = Fixture::new(Some(ceiling), false); f.bind();
    let before = f.broker.inspect();
    let mut cursor = f.broker.begin_publication_precheck(1, 1).unwrap();
    assert_eq!(cursor.ceiling(), ceiling.validation);
    let unlimited = RefinementBudget { steps: u64::MAX, value_bytes: u64::MAX };
    let report = cursor.advance(unlimited);
    assert_eq!(report.outcome, PublicationOutcome::Refused(Error::Limit));
    assert_eq!(report.total.value_bytes, 0);
    let repeated = cursor.advance(unlimited);
    assert_eq!(repeated.total, report.total);
    assert_eq!(repeated.spent, RefinementBudget::default());
    assert_eq!(cursor.cancel(), report.total);
    assert_eq!(f.broker.inspect(), before);
    assert_eq!(f.broker.publication_validation(1).unwrap(), None);
    f.review();
    assert_eq!(f.broker.authorize(1, Some(&f.committee), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(f.endpoint.execution_count(), 0);
}

#[test]
fn cancelling_opaque_work_leaves_the_original_owner_and_permit_path_unchanged() {
    let mut f = Fixture::new(Some(limits()), false); f.bind();
    let before = f.broker.inspect();
    let mut cursor = f.broker.begin_publication_precheck(1, 1).unwrap();
    let report = cursor.advance(RefinementBudget { steps: 1000, value_bytes: 12 });
    assert!(matches!(report.outcome, PublicationOutcome::NeedsRefinement { .. }));
    assert_eq!(report.total.value_bytes, 12); // nine structured bytes, then three opaque bytes
    assert_eq!(cursor.cancel(), report.total);
    assert_eq!(f.broker.inspect(), before);
    assert_eq!(f.broker.publication_input_revision(1), Ok(1));
    assert_eq!(f.broker.publication_validation(1).unwrap(), None);
    f.review(); let permit = f.authorize();
    let envelope = f.broker.dispatch(&permit, &f.action, Some(&f.committee), &snapshot()).unwrap();
    let receipt = f.endpoint.deliver(&envelope).unwrap();
    assert_eq!(receipt.outcome(), EndpointOutcome::Executed { resulting_version: 2 });
    f.broker.accept_receipt(receipt).unwrap();
    assert_eq!(f.broker.inspect().ledger.charged, 16);
}
