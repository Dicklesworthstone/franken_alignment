//! Native graph decisions consumed by actual durable two-key publication.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_mediation.rs"] mod topology;
use ordinary::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::mediation::*;
use fa_reference::action::consequence::mediation::{AuthorityGraph, Completeness, CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::Error;

fn graph(generation: u64, bypass: bool) -> AuthorityGraph {
    topology::graph(profile().delivery.scope, profile().delivery.target, generation, bypass)
}
fn start(root: &Directory) -> (FileOversight, FileHumanReviewer, FileMediationObserver) {
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    let observer = host.enable_mediation(host.revision(), graph(1, false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, human, observer)
}
fn certify(host: &mut FileOversight, observer: &FileMediationObserver) -> CutCheck {
    let proposal = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    observer.certify(host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS).unwrap().unwrap()
}
fn change(host: &FileOversight, operation: u64, next: Option<AuthorityGraph>) -> FileMediationUpdate {
    FileMediationUpdate { operation, expected_generation: host.mediation_snapshot().unwrap().graph.spec().generation,
        expected_authority_epoch: host.inspect().control.ledger.epoch, next }
}
fn update(host: &mut FileOversight, role: &FileMediationObserver, operation: u64, next: Option<AuthorityGraph>) {
    let request = change(host, operation, next); let revision = host.revision();
    role.update(host, revision, &request).unwrap();
}

#[test]
fn graph_certificate_is_required_but_still_needs_original_congress_and_both_keys() {
    let root = Directory::new(); let (mut host, human, observer) = start(&root);
    let before = host.inspect();
    assert!(host.propose(host.revision(), 1, spec(&host, b"publish"), snapshot()).is_err());
    assert_eq!(host.inspect(), before); assert!(host.publication_guard_required());
    assert!(matches!(certify(&mut host, &observer), CutCheck::Verified(_)));
    let action = host.propose(host.revision(), 1, spec(&host, b"publish"), snapshot()).unwrap();
    let input = inputs(&action, b"whole evidence");
    assert!(host.authorize(host.revision(), 1, &input, snapshot()).is_err());
    review_existing(&mut host, 1, 101, &input);
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).is_err());
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(20)).unwrap();
    let revision = host.revision(); let key = human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &key, &action, &input, snapshot()).unwrap();
    assert_eq!(host.delivery_mediation(1).unwrap().unwrap().graph(), &graph(1, false));
    let outcome = host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(outcome.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome.outcome));
}

#[test]
fn bypass_unknown_and_disconnected_graphs_never_become_certified_or_admit_actions() {
    for variant in 0..3 {
        let root = Directory::new(); let (mut host, _, observer) = start(&root);
        certify(&mut host, &observer);
        let mut spec = graph(2, variant == 0).spec().clone();
        if variant == 1 { spec.completeness = Completeness::Unknown; }
        if variant == 2 { spec.edges.retain(|edge| edge.to != 3); }
        let replacement = AuthorityGraph::new(spec).unwrap();
        update(&mut host, &observer, 10, Some(replacement));
        let proposal = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
        let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
        let result = observer.certify(&mut host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS).unwrap();
        match variant {
            0 => assert!(matches!(result, Ok(CutCheck::Bypass(ref path)) if path.nodes == vec![1, 3])),
            1 => assert_eq!(result, Err(Error::Incomplete)),
            _ => assert!(matches!(result, Ok(CutCheck::Unreachable { .. }))),
        }
        assert_eq!(host.mediation_snapshot().unwrap().last_check, Some(result));
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert!(host.propose(host.revision(), 1, ordinary::spec(&host, b"blocked"), snapshot()).is_err());
        update(&mut host, &observer, 11, Some(graph(3, false)));
        assert!(matches!(certify(&mut host, &observer), CutCheck::Verified(_)));
        assert!(host.propose(host.revision(), 1, ordinary::spec(&host, b"permitted"), snapshot()).is_ok());
    }
}

#[test]
fn a_postdispatch_topology_change_seals_instead_of_publishing_and_refunds_only_on_reconciliation() {
    for changed in [false, true] {
        let root = Directory::new(); let (mut host, human, observer) = start(&root); certify(&mut host, &observer);
        let keys = ready(&mut host, &human, 1, b"exact publication"); dispatch(&mut host, &keys);
        let original = host.delivery_mediation(1).unwrap().cloned();
        if changed { update(&mut host, &observer, 10, Some(graph(2, true))); }
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.delivery_mediation(1).unwrap(), original.as_ref());
        let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
        if changed { assert_ne!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 }); }
        else { assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 }); }
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
        assert_eq!(host.inspect().control.ledger.charged, if changed { 0 } else { 16 });
        assert_eq!(host.inspect().executions, u64::from(!changed));
    }
}

#[test]
fn executed_unknown_and_undispatched_obligations_keep_their_distinct_native_accounting() {
    let root = Directory::new(); let (mut host, human, observer) = start(&root); certify(&mut host, &observer);
    let first = ready(&mut host, &human, 1, b"already visible"); dispatch(&mut host, &first);
    let outcome = host.publish_checked(host.revision(), 1, Some(&first.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let second = ready(&mut host, &human, 2, b"not yet visible"); dispatch(&mut host, &second);
    let third = ready(&mut host, &human, 3, b"never dispatched");
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    let request = change(&host, 10, Some(graph(2, true))); let revision = host.revision();
    let receipt = observer.update(&mut host, revision, &request).unwrap().unwrap();
    assert_eq!(receipt.cancelled, vec![3]); assert_eq!(receipt.refunded_units, 16);
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert_eq!(host.inspect().control.ledger.stages[&3], ActionState::Cancelled);
    assert!(host.dispatch(host.revision(), &third.automatic, &third.human, &third.action, &third.inputs, snapshot()).is_err());
    let before = host.inspect();
    assert_eq!(observer.update(&mut host, 0, &request).unwrap(), Some(receipt));
    assert_eq!(host.inspect(), before);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome.outcome));
    assert_eq!(host.reconcile(host.revision(), 2).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 32);
    host.seal_unexecuted(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    update(&mut host, &observer, 11, Some(graph(3, false))); certify(&mut host, &observer);
    let fourth = ready(&mut host, &human, 4, b"freshly reviewed"); dispatch(&mut host, &fourth);
    assert_eq!(host.publish_checked(host.revision(), 4, Some(&fourth.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 3 });
}

#[test]
fn loss_retries_and_predecessors_cannot_recertify_an_old_graph_or_cross_observer_owners() {
    let root = Directory::new(); let other_root = Directory::new();
    let (mut host, _, observer) = start(&root); let (other, _, foreign) = start(&other_root); certify(&mut host, &observer);
    let old = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
    let request = change(&host, 10, None); let revision = host.revision();
    assert!(foreign.update(&mut host, revision, &request).is_err());
    assert_eq!(host.revision(), revision);
    observer.update(&mut host, revision, &request).unwrap();
    assert!(!host.mediation_snapshot().unwrap().available);
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert_eq!(observer.certify(&mut host, revision, epoch, &old, MAX_CHECK_EDGE_VISITS).unwrap(), Err(Error::Incomplete));
    let repeated = change(&host, 11, None); let revision = host.revision();
    assert_eq!(observer.update(&mut host, revision, &repeated).unwrap(), None);
    assert_eq!(host.inspect().control.ledger.epoch, epoch);
    let stale = change(&host, 12, Some(graph(1, false))); let revision = host.revision();
    assert!(observer.update(&mut host, revision, &stale).is_err()); assert!(host.storage_failure().is_none());
    update(&mut host, &observer, 13, Some(graph(2, false))); certify(&mut host, &observer);
    let before = host.inspect(); let mut conflict = request; conflict.next = Some(graph(3, false));
    assert!(observer.update(&mut host, 0, &conflict).is_err()); assert_eq!(host.inspect(), before);
    drop(other);
}

#[test]
fn full_journal_or_staged_write_failure_cannot_keep_the_old_certified_owner_usable() {
    for capacity in [3, 4] {
        let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = capacity;
        let (mut host, _) = FileOversight::create(root.store(), p.clone()).unwrap();
        let observer = host.enable_mediation(0, graph(1, false)).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); certify(&mut host, &observer);
        let request = change(&host, 1, None); let revision = host.revision();
        let result = observer.update(&mut host, revision, &request);
        if capacity == 3 {
            assert_eq!(result, Err(JournalError::Contract(Error::Limit)));
            assert!(host.storage_failure().is_some());
            assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
        } else { assert!(result.unwrap().is_some()); assert!(host.storage_failure().is_none()); }
    }
    let root = Directory::new(); let (mut host, human, observer) = start(&root); certify(&mut host, &observer);
    let keys = ready(&mut host, &human, 1, b"pending"); dispatch(&mut host, &keys);
    let bytes = std::fs::read(root.store().join("delivery.bin")).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"retain uncommitted evidence").unwrap();
    let request = change(&host, 10, None); let revision = host.revision();
    assert!(observer.update(&mut host, revision, &request).is_err());
    assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    drop(host);
    let (mut reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(!reopened.mediation_snapshot().unwrap().available);
    assert!(reopened.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(reopened.inspect().control.ledger.charged, 16);
    let request = change(&reopened, 11, Some(graph(2, false))); let revision = reopened.revision();
    assert!(observer.update(&mut reopened, revision, &request).is_err());
}
