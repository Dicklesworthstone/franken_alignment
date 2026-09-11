//! Topology admission through real reference congress, keys and endpoint state.
//! These tests exercise the in-memory product path, not OS containment.
#[path = "support/mediation.rs"]
mod topology;
#[path = "support/two_key_delivery.rs"]
mod support;

use support::Fixture;
use fa_reference::action::consequence::mediation::*;
use fa_reference::action::consequence::oversight::{CommitteeInput, OversightBroker, ReviewWindow, action_frame};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::ResetRequest;
use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Permit, ResolvedTarget, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use fa_reference::round::Verdict;
use fa_reference::{Error, Snapshot};
use std::collections::BTreeMap;

fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}
fn spec(epoch: u64, target: ResolvedTarget) -> ActionSpec {
    ActionSpec { version: VERSION, scope: topology::scope(), target: Some(target), payload: b"publish".to_vec(),
        required_witnesses: vec![], policy_epoch: epoch, deadline: ElapsedTick(100), units: 16 }
}
fn activate(broker: &mut OversightBroker, graph: &AuthorityGraph) {
    let result = broker.certify_mediation(graph.spec().generation, broker.inspect().ledger.epoch,
        &graph.propose_cut(&[3]).unwrap(), MAX_CHECK_EDGE_VISITS).unwrap();
    assert!(matches!(result, CutCheck::Verified(_)));
}
fn pending(broker: &mut OversightBroker, id: u64, target: ResolvedTarget) -> (FrozenAction, CommitteeInput, Permit) {
    let action = broker.propose(id, spec(broker.inspect().ledger.epoch, target), &snapshot()).unwrap().action;
    let helper = &broker.contracts().members()["helper"];
    let mut bytes = action_frame(&action); let end = bytes.len(); bytes.extend_from_slice(helper.question());
    let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { span: ByteSpan { start: 0, end }, kind: PartKind::Other },
        SubmittedPart { span: ByteSpan { start: end, end: bytes.len() }, kind: PartKind::Question },
    ], vec![]).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
    }, vec![]).unwrap();
    let inputs = CommitteeInput::capture(&action, broker.contracts(), BTreeMap::from([("helper".into(), view)])).unwrap();
    broker.record_inputs(id, 0, inputs.clone()).unwrap();
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, id + 1_000, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let committed = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", committed, now).unwrap(); session.open_reveals(now).unwrap();
    session.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
    broker.apply_review(session.finish(now).unwrap(), Some(&inputs), &snapshot()).unwrap();
    let permit = broker.authorize(id, Some(&inputs), &snapshot()).unwrap();
    (action, inputs, permit)
}

#[test]
fn both_one_key_and_two_key_dispatch_require_and_retain_the_verified_cut() {
    for two_key in [false, true] {
        let mut fixture = Fixture::new(two_key); let graph = topology::graph(1);
        fixture.broker.enable_mediation(graph.clone()).unwrap();
        let before = fixture.broker.inspect();
        assert_eq!(fixture.broker.propose(1, spec(0, topology::target()), &snapshot()).unwrap_err(), Error::Incomplete);
        assert_eq!(fixture.broker.inspect(), before);
        assert_eq!(fixture.broker.certify_mediation(1, 0, &graph.propose_cut(&[3]).unwrap(), 1), Err(Error::Limit));
        assert!(fixture.broker.mediation_cut().is_none());
        activate(&mut fixture.broker, &graph);
        let message = fixture.dispatch(1, two_key.then_some(50));
        assert_eq!(fixture.endpoint.execution_count(), 0);
        let cut = fixture.broker.delivery_mediation(1).unwrap().unwrap();
        assert_eq!(cut.graph(), &graph); assert_eq!(cut.gates(), &[3]);
        fixture.broker.accept_receipt(fixture.endpoint.deliver(&message).unwrap()).unwrap();
        assert_eq!(fixture.endpoint.execution_count(), 1);
        assert_eq!(fixture.endpoint.payload(), b"publish");
        assert_eq!(fixture.broker.inspect().ledger.charged, 16);
    }
}

#[test]
fn bypass_update_between_authorization_and_dispatch_cancels_the_old_permit() {
    let mut fixture = Fixture::new(false); let original = topology::graph(1);
    fixture.broker.enable_mediation(original.clone()).unwrap(); activate(&mut fixture.broker, &original);
    let (action, inputs, permit) = pending(&mut fixture.broker, 1, topology::target());
    assert_eq!(fixture.broker.inspect().ledger.reserved, 16);
    let mut changed = topology::spec(2); changed.edges.push(topology::edge(9, 2, 5));
    let bypass = AuthorityGraph::new(changed).unwrap();
    let change = fixture.broker.replace_mediation(1, 0, bypass.clone()).unwrap();
    assert_eq!(change.cancelled, vec![1]); assert_eq!(change.refunded_units, 16); assert_eq!(change.revocation_floor, 1);
    assert_eq!(change.previous, original); assert_eq!(change.current, Some(bypass.clone()));
    assert_eq!(fixture.broker.inspect().ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(fixture.broker.inspect().ledger.available, 100);
    assert!(fixture.broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    assert!(matches!(fixture.broker.certify_mediation(2, 1, &bypass.propose_cut(&[3]).unwrap(), MAX_CHECK_EDGE_VISITS), Ok(CutCheck::Bypass(_))));
    assert!(fixture.broker.mediation_cut().is_none());
    assert_eq!(fixture.broker.propose(2, spec(1, topology::target()), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(fixture.endpoint.execution_count(), 0);
    let repaired = topology::graph(3);
    fixture.broker.replace_mediation(2, 1, repaired.clone()).unwrap();
    assert_eq!(fixture.broker.certify_mediation(3, 2, &original.propose_cut(&[3]).unwrap(), MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
    activate(&mut fixture.broker, &repaired);
    assert!(fixture.broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).is_err());
    let message = fixture.dispatch(2, None);
    fixture.broker.accept_receipt(fixture.endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(fixture.endpoint.execution_count(), 1);
    assert_eq!(fixture.broker.delivery_mediation(2).unwrap().unwrap().graph(), &repaired);
}

#[test]
fn topology_loss_preserves_unknown_effects_and_recovers_without_helpers_or_reviewer() {
    let (mut broker, mut endpoint, original) = {
        let mut fixture = Fixture::new(true); let original = topology::graph(1);
        fixture.broker.enable_mediation(original.clone()).unwrap(); activate(&mut fixture.broker, &original);
        let message = fixture.dispatch(1, Some(50));
        fixture.endpoint.deliver(&message).unwrap(); fixture.broker.acknowledgment_lost(1).unwrap();
        let (_, _, _) = pending(&mut fixture.broker, 2, fixture.endpoint.target());
        let change = fixture.broker.withdraw_mediation(1, 0).unwrap().unwrap();
        assert_eq!(change.cancelled, vec![2]); assert_eq!(change.refunded_units, 16);
        assert_eq!(fixture.broker.inspect().ledger.charged, 16);
        assert_eq!(fixture.broker.inspect().ledger.available, 84);
        assert!(fixture.broker.mediation_graph().is_none()); assert!(fixture.broker.mediation_cut().is_none());
        assert_eq!(fixture.broker.certify_mediation(1, 1, &original.propose_cut(&[3]).unwrap(), MAX_CHECK_EDGE_VISITS), Err(Error::Incomplete));
        let before = fixture.broker.inspect();
        assert_eq!(fixture.broker.withdraw_mediation(1, 1).unwrap(), None);
        assert_eq!(fixture.broker.inspect(), before);
        fixture.broker.inputs_unavailable(1, 1).unwrap();
        (fixture.broker, fixture.endpoint, original)
    };
    let results = broker.reconcile_pending(&mut endpoint).unwrap();
    assert!(matches!(results[&1], Ok(fa_reference::action::consequence::delivery::EndpointStatus::Resolved(_))));
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(broker.inspect().ledger.charged, 16);
    assert_eq!(broker.delivery_mediation(1).unwrap().unwrap().graph(), &original);
    assert_eq!(broker.enable_mediation(topology::graph(2)), Err(Error::Duplicate));
}

#[test]
fn stale_or_foreign_governance_requests_do_not_mutate_a_valid_current_path() {
    let mut fixture = Fixture::new(false); let graph = topology::graph(1);
    fixture.broker.enable_mediation(graph.clone()).unwrap(); activate(&mut fixture.broker, &graph);
    let (action, inputs, permit) = pending(&mut fixture.broker, 1, topology::target());
    let before = fixture.broker.inspect();
    let mut foreign = topology::spec(2); foreign.scope.tenant = 8; foreign.family.scope.tenant = 8;
    assert_eq!(fixture.broker.replace_mediation(1, 0, AuthorityGraph::new(foreign).unwrap()), Err(Error::Binding));
    assert_eq!(fixture.broker.replace_mediation(1, 9, topology::graph(2)), Err(Error::Stale));
    assert_eq!(fixture.broker.replace_mediation(1, 0, graph.clone()), Err(Error::Stale));
    let mut forged = graph.propose_cut(&[3]).unwrap(); forged.reachable.clear();
    assert_eq!(fixture.broker.certify_mediation(1, 0, &forged, MAX_CHECK_EDGE_VISITS), Err(Error::Binding));
    assert_eq!(fixture.broker.inspect(), before);
    assert_eq!(fixture.broker.mediation_graph(), Some(&graph));
    let message = fixture.broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    fixture.broker.accept_receipt(fixture.endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(fixture.endpoint.execution_count(), 1);
}

#[test]
fn reset_and_dispatcher_restart_do_not_restore_withdrawn_topology() {
    let mut fixture = Fixture::new(false); let original = topology::graph(1);
    fixture.broker.enable_mediation(original.clone()).unwrap(); activate(&mut fixture.broker, &original);
    let checkpoint = fixture.broker.capture_checkpoint(1, 0).unwrap();
    fixture.broker.withdraw_mediation(1, 0).unwrap();
    let before = fixture.broker.inspect();
    fixture.broker.reset(ResetRequest {
        checkpoint, expected_control_sequence: before.sequence, expected_actor_revision: fixture.broker.actor_revision(),
        binding: ReviewBinding { round: 99, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[topology::target()]).unwrap(),
    }).unwrap();
    let fence = fixture.broker.restart_dispatcher().unwrap();
    fixture.broker.confirm_fence(fixture.endpoint.install_fence(fence).unwrap()).unwrap();
    let epoch = fixture.broker.inspect().ledger.epoch;
    assert_eq!(fixture.broker.certify_mediation(1, epoch, &original.propose_cut(&[3]).unwrap(), MAX_CHECK_EDGE_VISITS), Err(Error::Incomplete));
    assert_eq!(fixture.broker.propose(1, spec(epoch, topology::target()), &snapshot()).unwrap_err(), Error::Incomplete);
    let repaired = topology::graph(2);
    fixture.broker.replace_mediation(1, epoch, repaired.clone()).unwrap(); activate(&mut fixture.broker, &repaired);
    let message = fixture.dispatch(1, None);
    fixture.broker.accept_receipt(fixture.endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(fixture.endpoint.execution_count(), 1); assert_eq!(fixture.broker.incident_count(), 1);
}

#[test]
fn new_topology_does_not_claim_to_revoke_an_already_admitted_envelope() {
    let mut fixture = Fixture::new(false); let original = topology::graph(1);
    fixture.broker.enable_mediation(original.clone()).unwrap(); activate(&mut fixture.broker, &original);
    let message = fixture.dispatch(1, None); fixture.broker.acknowledgment_lost(1).unwrap();
    let mut unknown = topology::spec(2); unknown.completeness = Completeness::Unknown;
    let change = fixture.broker.replace_mediation(1, 0, AuthorityGraph::new(unknown).unwrap()).unwrap();
    assert!(change.cancelled.is_empty()); assert_eq!(change.refunded_units, 0);
    assert_eq!(fixture.broker.inspect().ledger.charged, 16);
    fixture.broker.accept_receipt(fixture.endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(fixture.endpoint.execution_count(), 1);
    assert_eq!(fixture.broker.delivery_mediation(1).unwrap().unwrap().graph(), &original);
    assert!(fixture.broker.mediation_cut().is_none());
}

#[test]
fn unconfigured_profile_keeps_its_original_non_graph_contract_and_cannot_enable_late() {
    let (mut broker, mut endpoint, message) = support::dispatched(None);
    assert_eq!(broker.enable_mediation(topology::graph(1)), Err(Error::WrongState));
    assert!(broker.delivery_mediation(1).unwrap().is_none());
    broker.accept_receipt(endpoint.deliver(&message).unwrap()).unwrap();
    assert_eq!(endpoint.execution_count(), 1);
}
