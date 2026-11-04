//! Complete mediated requirements stay attached to both terminal phases.
use super::*;
use super::super::{FileMediatedRequirements, FileGuardedRecoveryStatus as Status};
use super::super::super::{BaseEvent, Event, FileOversightProfile, Machine, Transition, journal};
use crate::Error;
use crate::action::consequence::delivery::persistent::{RecoveryReserve, MAX_JOURNAL_EVENTS};
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements, anchored::FileHistoryAnchor};
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::FileTopologyRequirement;
use crate::action::consequence::mediation::{AuthorityGraph, Channel, Completeness,
    CutCheck, Edge, Enforcer, GraphSpec, Node, NodeKind, MAX_CHECK_EDGE_VISITS};
use crate::perimeter::*;
use std::fs;
#[allow(dead_code)]
#[path = "../reconciled/fixture.rs"]
mod fixture;
use fixture::*;

fn graph(generation: u64) -> AuthorityGraph {
    let p = profile().delivery;
    AuthorityGraph::new(GraphSpec { generation, inventory_generation: generation, scope: p.scope, target: p.target,
        family: EffectFamilyRecord { scope: PerimeterScope { tenant: 1, principal: 2, purpose: 1 },
            family: "publication".into(), trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared topology only".into()] },
        completeness: Completeness::DeclaredComplete { inventory_generation: generation },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer }, Node { id: 3, kind: NodeKind::Sink }],
        edges: vec![Edge { id: 1, from: 1, to: 2, channel: Channel::Dispatch, route: "publication".into(), provenance: 1 },
            Edge { id: 2, from: 2, to: 3, channel: Channel::Dispatch, route: "publication".into(), provenance: 2 }],
        enforcers: vec![Enforcer { node: 2, adapter: p.target.adapter, contract_version: 1, generation: 1, provenance: 9 }],
    }).unwrap()
}
fn prepared(root: &Directory) -> Ready {
    let (mut host, roles) = FileOversight::create_guarded(root.store(), profile(), &guards(), None).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let observer = host.enable_mediation(host.revision(), graph(1)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = graph(1).propose_cut(&[2]).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(observer.certify(&mut host, revision, epoch, &proposal,
        MAX_CHECK_EDGE_VISITS).unwrap().unwrap(), CutCheck::Verified(_)));
    Ready::from_owner(host, roles.human, 1, 101, ElapsedTick(1))
}
fn expected(host: &FileOversight) -> FileMediatedRequirements {
    FileMediatedRequirements { oversight: requirements(host, guards()),
        topology: FileTopologyRequirement { initial: graph(1), current: graph(1), available: true },
        prediction: None, evaluation: None }
}
fn begin(root: &Directory, e: &FileMediatedRequirements, anchor: &FileHistoryAnchor) -> FileMediatedRecovery {
    FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), e, anchor).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
}
fn ready(run: &mut FileMediatedRecovery) {
    while run.progress().status == Status::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(run.progress().status, Status::Ready);
}

#[test]
fn mediated_phases_preserve_original_native_outcomes_and_require_stopped_availability() {
    for mode in 0..4 {
        let root = Directory::new(); let mut live = prepared(&root);
        if mode > 0 { live.dispatch(); }
        if mode == 2 { live.publish(); }
        let mut e = expected(&live.host); let anchor = live.host.history_anchor().unwrap();
        let state = live.host.inspect();
        let request = StopRequest { operation: 900, expected_control_sequence: state.control.sequence,
            expected_authority_epoch: state.control.ledger.epoch };
        let tick = ElapsedTick(if mode == 3 { 2000 } else { 3 });
        let mut events = live.host.events.clone();
        events.extend([Event::Core(BaseEvent::Stop(request)), Event::Core(BaseEvent::Fence),
            Event::Core(BaseEvent::StopProgress(tick))]);
        let mut oracle = Machine::new(&profile()).unwrap(); let mut sweep = None;
        for event in &events {
            if let Transition::StopProgressed(value) = oracle.apply(event).unwrap() { sweep = Some(value); }
        }
        let expected_state = oracle.snapshot(events.len());
        let bytes = journal::encode(&profile(), live.host.store.identity(), &events).unwrap(); drop(live);
        let mut run = begin(&root, &e, &anchor); ready(&mut run);
        let admission = run.finish_admission_stop(request).unwrap();
        assert_eq!(admission.appended_events(), 2);
        let stopped_bytes = root.bytes();
        let mut wrong = begin(&root, &e, admission.history_anchor());
        assert!(wrong.advance(0, MAX_JOURNAL_EVENTS).is_err());
        assert!(wrong.finish_stopped_drain(request, || panic!("stale availability")).is_err());
        assert_eq!(root.bytes(), stopped_bytes);
        e.topology.available = false;
        let mut run = begin(&root, &e, admission.history_anchor()); ready(&mut run);
        let drained = run.finish_stopped_drain(request, || Ok(tick)).unwrap();
        assert_eq!(drained.stopped(), &FileStoppedRecovery::Advanced { snapshot: expected_state, sweep: sweep.unwrap() });
        assert_eq!(root.bytes(), bytes);
        assert_eq!(drained.stopped().progress().drained(), mode != 3);
        assert_eq!(drained.stopped().progress().charged_units, if mode >= 2 { 16 } else { 0 });
    }
}

#[test]
fn neither_phase_downgrades_the_final_topology_or_floor_checker() {
    for drain in [false, true] { for mismatch in 0..4 {
        let root = Directory::new(); let live = prepared(&root);
        let mut e = expected(&live.host); let anchor = live.host.history_anchor().unwrap(); drop(live);
        let mut run = begin(&root, &e, &anchor); ready(&mut run);
        let request = run.stop_request(900).unwrap();
        if drain {
            let admission = run.finish_admission_stop(request).unwrap(); e.topology.available = false;
            run = begin(&root, &e, admission.history_anchor()); ready(&mut run);
        }
        fs::write(root.store().join("delivery.pending"), b"retain").unwrap(); let before = root.files();
        match mismatch {
            0 => run.expected.topology.initial = graph(2),
            1 => run.expected.topology.current = graph(2),
            2 => run.expected.topology.available = !run.expected.topology.available,
            _ => run.expected.oversight.minimum.authority_epoch = u64::MAX,
        }
        if drain { assert!(run.finish_stopped_drain(request, || panic!("changed full profile")).is_err()); }
        else { assert!(run.finish_admission_stop(request).is_err()); }
        assert_eq!(root.files(), before);
    } }
}

#[test]
fn base_open_cannot_substitute_for_mediation_and_active_drain_cannot_stop() {
    let root = Directory::new(); let live = prepared(&root);
    let e = expected(&live.host); let anchor = live.host.history_anchor().unwrap(); drop(live);
    let before = root.files();
    assert!(FileOversight::begin_open_guarded_anchored(root.store(), profile(), &e.oversight, &anchor).is_err());
    let mut run = begin(&root, &e, &anchor); ready(&mut run);
    let request = run.stop_request(900).unwrap();
    assert_eq!(run.finish_stopped_drain(request, || panic!("active mediated domain")), Err(Error::WrongState.into()));
    assert_eq!(root.files(), before);
    let mut run = begin(&root, &e, &anchor); ready(&mut run);
    assert!(run.finish_admission_stop(request).is_ok());
}
