//! Recovered observer custody consumed by the original two-key publication path.
#![cfg(unix)]
#[path = "support/file_mediated.rs"] mod support;
use support::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::*;
use fa_reference::action::consequence::mediation::{AuthorityGraph, CutCheck, MAX_CHECK_EDGE_VISITS};
use fa_reference::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use fa_reference::Error;

#[test]
fn fresh_topology_and_human_custody_restore_publication_but_not_old_cuts_or_keys() {
    let root = Directory::new();
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), profile(),
        &guards(), None, graph(1, false), None, None).unwrap();
    assert!(host.publication_guard_required());
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(host.propose(host.revision(), 1, ordinary::spec(&host, b"first"), snapshot()).is_err());
    assert!(matches!(certify(&mut host, &roles.topology_observer), CutCheck::Verified(_)));
    let first = ordinary::ready(&mut host, &roles.oversight.human, 1, b"first");
    ordinary::dispatch(&mut host, &first);
    let published = host.publish_checked(host.revision(), 1, Some(&first.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let expected: FileMediatedRequirements = requirements(&host, None); let cut = host.delivery_mediation(1).unwrap().cloned();
    let revision = host.revision(); drop(host);
    let (mut host, recovered) = FileOversight::open_mediated_guarded(root.store(), profile(), &expected).unwrap();
    assert_eq!(host.revision(), revision + 1); assert!(!host.clock_ready());
    assert!(!host.mediation_snapshot().unwrap().available);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(host.delivery_mediation(1).unwrap(), cut.as_ref());
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(published.outcome));
    let old_proposal = graph(1, false).propose_cut(&[2]).unwrap();
    let rev = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert_eq!(recovered.topology_observer.certify(&mut host, rev, epoch, &old_proposal,
        MAX_CHECK_EDGE_VISITS).unwrap(), Err(Error::Incomplete));
    let request = replacement(&host, 10, Some(graph(2, false))); let rev = host.revision();
    assert_eq!(roles.topology_observer.update(&mut host, rev, &request), Err(JournalError::Contract(Error::Binding)));
    let stale = replacement(&host, 11, Some(graph(1, false))); let rev = host.revision();
    assert!(recovered.topology_observer.update(&mut host, rev, &stale).is_err());
    assert!(host.storage_failure().is_none());
    update(&mut host, &recovered.topology_observer, 12, Some(graph(2, false)));
    assert!(matches!(certify(&mut host, &recovered.topology_observer), CutCheck::Verified(_)));
    let next = ordinary::ready(&mut host, &recovered.oversight.human, 2, b"second");
    assert!(host.dispatch(host.revision(), &first.automatic, &first.human, &first.action, &first.inputs, snapshot()).is_err());
    ordinary::dispatch(&mut host, &next);
    let result = host.publish_checked(host.revision(), 2, Some(&next.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 3 });
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 32);
}

#[test]
fn exact_initial_and_current_graphs_availability_and_external_floors_precede_cleanup() {
    let root = Directory::new();
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), profile(),
        &guards(), None, graph(1, false), None, None).unwrap();
    update(&mut host, &roles.topology_observer, 1, Some(graph(2, false)));
    let expected = requirements(&host, None); drop(host);
    let path = root.store(); let bytes = std::fs::read(path.join("delivery.bin")).unwrap();
    std::fs::write(path.join("delivery.pending"), b"retain on refusal").unwrap();
    let mut candidates = Vec::new();
    let mut bad = expected.clone(); bad.topology.initial = graph(1, true); candidates.push(bad);
    let mut bad = expected.clone(); bad.topology.current = graph(2, true); candidates.push(bad);
    let mut bad = expected.clone(); bad.topology.current = graph(1, false); candidates.push(bad);
    let mut bad = expected.clone(); bad.topology.available = false; candidates.push(bad);
    let mut bad = expected.clone(); bad.oversight.minimum.journal_revision += 1; candidates.push(bad);
    let mut bad = expected.clone(); bad.oversight.minimum.control_sequence += 1; candidates.push(bad);
    let mut bad = expected.clone(); bad.oversight.minimum.authority_epoch += 1; candidates.push(bad);
    for bad in candidates {
        assert!(FileOversight::open_mediated_guarded(&path, profile(), &bad).is_err());
        assert_eq!(std::fs::read(path.join("delivery.bin")).unwrap(), bytes);
        assert_eq!(std::fs::read(path.join("delivery.pending")).unwrap(), b"retain on refusal");
    }
    assert!(matches!(FileOversight::open_guarded(&path, profile(), &expected.oversight),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(path.join("delivery.pending")).unwrap(), b"retain on refusal");
    let (host, _) = FileOversight::open_mediated_guarded(&path, profile(), &expected).unwrap();
    assert!(!host.mediation_snapshot().unwrap().available);
    assert_eq!(host.mediation_snapshot().unwrap().graph, graph(2, false));
}

#[test]
fn one_recovery_returns_independent_evaluator_and_keeps_unresolved_effect_accounting() {
    let root = Directory::new(); let evaluation = protocol();
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), profile(),
        &guards(), None, graph(1, false), None, Some(evaluation.clone())).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); certify(&mut host, &roles.topology_observer);
    let keys = ordinary::ready(&mut host, &roles.oversight.human, 1, b"unresolved"); ordinary::dispatch(&mut host, &keys);
    let old_ticket = host.evaluation_ticket(101).unwrap();
    let report = host.credibility_report().unwrap(); assert_eq!(report.pending_cases, 1);
    let expected = requirements(&host, Some(evaluation.clone())); drop(host);
    assert!(matches!(FileOversight::open_evaluated_guarded(root.store(), profile(), &expected.oversight, &evaluation),
        Err(JournalError::Contract(Error::Binding))));
    let (mut host, fresh) = FileOversight::open_mediated_guarded(root.store(), profile(), &expected).unwrap();
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let assessment = Assessment { origin: 7, evidence_id: [4; 32], truth: GroundTruth::Benign };
    let rev = host.revision();
    assert!(roles.evaluator.as_ref().unwrap().assess(&mut host, rev, &old_ticket, assessment).is_err());
    let ticket = host.evaluation_ticket(101).unwrap(); let rev = host.revision();
    assert!(fresh.evaluator.as_ref().unwrap().assess(&mut host, rev, &ticket, assessment).unwrap());
    assert!(!host.clock_ready()); assert!(!host.mediation_snapshot().unwrap().available);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    host.seal_unexecuted(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 0);
    update(&mut host, &fresh.topology_observer, 10, Some(graph(2, false))); certify(&mut host, &fresh.topology_observer);
    let keys = ordinary::ready(&mut host, &fresh.oversight.human, 2, b"fresh"); ordinary::dispatch(&mut host, &keys);
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn original_native_bootstrap_validation_precedes_creating_a_directory() {
    let root = Directory::new(); let mut s = graph(1, false).spec().clone(); s.scope.tenant += 1;
    s.family.scope.tenant += 1;
    let foreign = AuthorityGraph::new(s).unwrap();
    assert!(FileOversight::create_mediated_guarded(root.store(), profile(), &guards(), None,
        foreign, None, None).is_err());
    assert!(!root.store().exists());
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), profile(), &guards(), None,
        graph(1, false), None, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(matches!(certify(&mut host, &roles.topology_observer), CutCheck::Verified(_)));
    assert!(host.propose(host.revision(), 1, ordinary::spec(&host, b"allowed"), snapshot()).is_ok());
}

#[test]
fn recovered_topology_custody_does_not_undo_terminal_stop() {
    let root = Directory::new();
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), profile(), &guards(), None,
        graph(1, false), None, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); certify(&mut host, &roles.topology_observer);
    let state = host.inspect();
    let receipt = host.request_stop(host.revision(), StopRequest { operation: 55,
        expected_control_sequence: state.control.sequence, expected_authority_epoch: state.control.ledger.epoch }).unwrap();
    let expected = requirements(&host, None); drop(host);
    let (mut host, roles) = FileOversight::open_mediated_guarded(root.store(), profile(), &expected).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let req = replacement(&host, 10, Some(graph(2, false))); let rev = host.revision();
    roles.topology_observer.update(&mut host, rev, &req).unwrap();
    assert!(matches!(certify(&mut host, &roles.topology_observer), CutCheck::Verified(_)));
    assert_eq!(host.inspect().stop, Some(receipt)); assert!(host.inspect().control.suspended);
    assert!(host.propose(host.revision(), 1, ordinary::spec(&host, b"blocked"), snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
}
