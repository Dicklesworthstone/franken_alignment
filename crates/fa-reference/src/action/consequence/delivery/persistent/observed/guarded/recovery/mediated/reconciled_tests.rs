//! Same original two-key fixtures, reducer oracle and canonical replacement.
//! Fixed manual helper votes are controls, not learned/calibration evidence.
use super::*;
use super::super::{FileMediatedRequirements, FileGuardedRecoveryStatus as Status};
use super::super::super::{BaseEvent, Event, Machine, Transition, journal, storage, MAX_JOURNAL_EVENTS};
use crate::Error;
use crate::action::consequence::delivery::persistent::{RecoveryReserve, JournalIo};
use crate::action::consequence::delivery::persistent::observed::FileOversightProfile;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements};
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::FileTopologyRequirement;
use crate::action::consequence::mediation::{AuthorityGraph, Channel, Completeness,
    CutCheck, Edge, Enforcer, GraphSpec, Node, NodeKind, MAX_CHECK_EDGE_VISITS};
use crate::perimeter::*;
use std::{cell::Cell, fs};
// Reuse the original native test fixture; only part of its helper API is needed.
#[allow(dead_code)]
#[path = "../reconciled/fixture.rs"]
mod fixture;
use fixture::*;

fn graph() -> AuthorityGraph {
    let p = profile().delivery;
    AuthorityGraph::new(GraphSpec { generation: 1, inventory_generation: 1, scope: p.scope, target: p.target,
        family: EffectFamilyRecord { scope: PerimeterScope { tenant: 1, principal: 2, purpose: 1 },
            family: "publication".into(), trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared topology only".into()] },
        completeness: Completeness::DeclaredComplete { inventory_generation: 1 },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer }, Node { id: 3, kind: NodeKind::Sink }],
        edges: vec![Edge { id: 1, from: 1, to: 2, channel: Channel::Dispatch, route: "publication".into(), provenance: 1 },
            Edge { id: 2, from: 2, to: 3, channel: Channel::Dispatch, route: "publication".into(), provenance: 2 }],
        enforcers: vec![Enforcer { node: 2, adapter: p.target.adapter, contract_version: 1, generation: 1, provenance: 9 }],
    }).unwrap()
}
fn prepared(root: &Directory) -> Ready {
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), profile(),
        &guards(), None, graph(), None, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = graph().propose_cut(&[2]).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(roles.topology_observer.certify(&mut host, revision, epoch,
        &proposal, MAX_CHECK_EDGE_VISITS).unwrap().unwrap(), CutCheck::Verified(_)));
    Ready::from_owner(host, roles.oversight.human, 1, 101, ElapsedTick(1))
}
fn expected(host: &FileOversight) -> FileMediatedRequirements {
    FileMediatedRequirements { oversight: requirements(host, guards()),
        topology: FileTopologyRequirement { initial: graph(), current: graph(), available: true },
        prediction: None, evaluation: None }
}
fn begin(root: &Directory, expected: &FileMediatedRequirements) -> FileMediatedRecovery {
    FileOversight::begin_open_mediated_guarded(root.store(), profile(), expected).unwrap()
}
fn ready(run: &mut FileMediatedRecovery, quantum: usize) {
    while run.progress().status == Status::Replaying {
        let before = run.progress(); let after = run.advance(before.replayed_events, quantum).unwrap();
        assert!(after.replayed_events - before.replayed_events <= quantum);
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    assert_eq!(run.progress().status, Status::Ready);
}

#[test]
fn all_native_dispositions_match_original_replay_bytes_and_outcomes_without_resending() {
    for mode in 0..3 {
        let root = Directory::new(); let mut original = prepared(&root);
        if mode > 0 { original.dispatch(); }
        if mode == 2 { original.publish(); }
        let e = expected(&original.host); let anchor = original.host.history_anchor().unwrap();
        let before = root.bytes(); let revision = original.host.revision();
        let mut events = original.host.events.clone();
        events.extend([Event::Core(BaseEvent::Fence), Event::Core(BaseEvent::Time(ElapsedTick(3))),
            Event::Core(BaseEvent::Sweep)]);
        let mut oracle = Machine::new(&profile()).unwrap(); let mut outcomes = None;
        for event in &events {
            if let Transition::Swept(result) = oracle.apply(event).unwrap() { outcomes = Some(result); }
        }
        let state = oracle.snapshot(events.len()); let outcomes = outcomes.unwrap();
        let bytes = journal::encode(&profile(), original.host.store.identity(), &events).unwrap(); drop(original);
        for anchored in [false, true] { for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
            // Controlled independent runs from the same acknowledged original cut.
            fs::write(root.store().join(storage::CANONICAL), &before).unwrap();
            let mut run = if anchored {
                FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &e, &anchor).unwrap()
            } else { begin(&root, &e) };
            ready(&mut run, quantum); let clocks = Cell::new(0);
            let recovered = run.finish_reconciled(|| { clocks.set(clocks.get() + 1); ElapsedTick(3) }).unwrap();
            assert_eq!(clocks.get(), 1); assert_eq!(recovered.owner.revision(), revision + 3);
            assert_eq!(recovered.owner.inspect(), state); assert_eq!(recovered.outcomes, outcomes);
            assert_eq!(root.bytes(), bytes); assert_eq!(recovered.owner.inspect().executions, u64::from(mode == 2));
            assert_eq!(recovered.owner.inspect().control.ledger.charged, if mode == 2 { 16 } else { 0 });
            assert!(!recovered.owner.mediation_snapshot().unwrap().available);
            assert!(recovered.roles.consistency_observer.is_none()); assert!(recovered.roles.evaluator.is_none());
        } }
    }
}

#[test]
fn old_original_approvals_remain_unusable_after_mediated_reconciliation() {
    let root = Directory::new(); let original = prepared(&root); let e = expected(&original.host);
    let Ready { host, automatic, human, action, inputs } = original; drop(host);
    let mut run = begin(&root, &e); ready(&mut run, 1);
    let mut recovered = run.finish_reconciled(|| ElapsedTick(3)).unwrap();
    let before = root.bytes(); let revision = recovered.owner.revision();
    assert!(recovered.owner.dispatch(revision, &automatic, &human, &action, &inputs, snapshot()).is_err());
    assert_eq!(root.bytes(), before); assert_eq!(recovered.owner.inspect().executions, 0);
}

#[test]
fn incomplete_profiles_and_insufficient_three_event_capacity_cannot_reach_the_clock() {
    let root = Directory::new(); let original = prepared(&root); let e = expected(&original.host); drop(original);
    let before = root.files();
    assert!(matches!(begin(&root, &e).finish_reconciled(|| panic!("incomplete clock")),
        Err(JournalError::Contract(Error::Incomplete))));
    let mut run = begin(&root, &e); ready(&mut run, 1);
    // Internal fault controls: neither requirement mutation nor quota mutation
    // is available to a holder of the public sealed cursor.
    run.expected.topology.available = false;
    assert!(matches!(run.finish_reconciled(|| panic!("mismatching topology clock")),
        Err(JournalError::Contract(Error::Binding))));
    let mut run = begin(&root, &e); ready(&mut run, 1);
    run.inner.profile.delivery.limits.events = run.inner.events.len() + 2;
    assert!(matches!(run.finish_reconciled(|| panic!("capacity clock")), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(root.files(), before);
}

#[test]
fn stale_or_unwinding_clock_preserves_the_original_dispatched_liability_and_staging() {
    let root = Directory::new(); let mut original = prepared(&root); original.dispatch();
    let e = expected(&original.host); drop(original);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap(); let before = root.files();
    let mut run = begin(&root, &e); ready(&mut run, 1);
    assert!(matches!(run.finish_reconciled(|| ElapsedTick(0)), Err(JournalError::Contract(Error::Stale))));
    assert_eq!(root.files(), before);
    let mut run = begin(&root, &e); ready(&mut run, 1);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run.finish_reconciled(|| panic!("trusted clock unavailable"))
    })).is_err());
    assert_eq!(root.files(), before);
    let disk = FileOversight::read_mediation(root.store(), &profile(), &e).unwrap();
    assert_eq!(disk.journal.control.ledger.charged, 16); assert_eq!(disk.journal.executions, 0);
}

#[test]
fn canonical_change_during_final_clock_never_overwrites_the_changed_image() {
    let root = Directory::new(); let original = prepared(&root); let e = expected(&original.host); drop(original);
    let mut run = begin(&root, &e); ready(&mut run, 1);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap();
    assert!(matches!(run.finish_reconciled(|| {
        fs::write(root.store().join(storage::CANONICAL), b"changed image").unwrap(); ElapsedTick(3)
    }), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(root.bytes(), b"changed image");
    assert_eq!(fs::read(root.store().join("delivery.pending")).unwrap(), b"preserve");
}

#[test]
fn every_reconciliation_storage_barrier_exposes_only_original_canonical_outcomes() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut original = prepared(&root); original.dispatch();
        let mut e = expected(&original.host); let revision = original.host.revision(); drop(original); let before = root.bytes();
        let mut run = begin(&root, &e); ready(&mut run, 1); run.inner.store.fail_once(barrier);
        assert!(matches!(run.finish_reconciled(|| ElapsedTick(3)), Err(JournalError::Io(failure)) if failure.operation == barrier));
        let visible = barrier == JournalIo::DirectorySync; e.topology.available = !visible;
        let disk = FileOversight::read_mediation(root.store(), &profile(), &e).unwrap();
        assert_eq!(disk.journal.revision, revision + if visible { 3 } else { 0 });
        assert_eq!(disk.journal.executions, 0);
        assert_eq!(disk.journal.control.ledger.charged, if visible { 0 } else { 16 });
        if !visible { assert_eq!(root.bytes(), before); }
        let mut retry = begin(&root, &e); ready(&mut retry, 1);
        let recovered = retry.finish_reconciled(|| ElapsedTick(3)).unwrap();
        assert_eq!(recovered.owner.inspect().executions, 0);
        assert_eq!(recovered.owner.inspect().control.ledger.charged, 0);
    }
}
