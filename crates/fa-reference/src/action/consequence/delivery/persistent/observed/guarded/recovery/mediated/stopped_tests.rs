//! Original full-input/two-key publication followed by native terminal recovery.
//! Synthetic ballots test enforcement, not helper accuracy or host containment.
use super::*;
use super::super::{FileMediatedRequirements, FileGuardedRecoveryStatus as Status};
use super::super::super::{BaseEvent, Event, Machine, Transition, journal, storage, MAX_JOURNAL_EVENTS};
use crate::Error;
use crate::action::consequence::delivery::persistent::{RecoveryReserve, JournalIo};
use crate::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile};
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements, anchored::FileHistoryAnchor};
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::FileTopologyRequirement;
use crate::action::consequence::mediation::{AuthorityGraph, Channel, Completeness,
    CutCheck, Edge, Enforcer, GraphSpec, Node, NodeKind, MAX_CHECK_EDGE_VISITS};
use crate::perimeter::*;
use std::{cell::Cell, fs};
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
fn prepared(root: &Directory, p: FileOversightProfile) -> Ready {
    let (mut host, roles) = FileOversight::create_guarded(root.store(), p, &guards(), None).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
    let observer = host.enable_mediation(host.revision(), graph(1)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = graph(1).propose_cut(&[2]).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(observer.certify(&mut host, revision, epoch,
        &proposal, MAX_CHECK_EDGE_VISITS).unwrap().unwrap(), CutCheck::Verified(_)));
    Ready::from_owner(host, roles.human, 1, 101, ElapsedTick(1))
}
fn expected(host: &FileOversight) -> FileMediatedRequirements {
    FileMediatedRequirements { oversight: requirements(host, guards()),
        topology: FileTopologyRequirement { initial: graph(1), current: graph(1), available: true },
        prediction: None, evaluation: None }
}
fn begin(root: &Directory, p: FileOversightProfile, e: &FileMediatedRequirements) -> FileMediatedRecovery {
    FileOversight::begin_open_mediated_guarded(root.store(), p, e).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
}
fn ready(run: &mut FileMediatedRecovery, quantum: usize) {
    while run.progress().status == Status::Replaying {
        let before = run.progress(); let after = run.advance(before.replayed_events, quantum).unwrap();
        assert_eq!(after.replayed_events - before.replayed_events,
            quantum.min(before.total_events - before.replayed_events));
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    assert_eq!(run.progress().status, Status::Ready);
}
fn request(host: &FileOversight) -> StopRequest {
    let state = host.inspect();
    StopRequest { operation: 700, expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch }
}

#[test]
fn native_terminal_bytes_and_outcomes_match_for_reserved_dispatched_published_and_expired_histories() {
    for mode in 0..4 {
        let root = Directory::new(); let mut original = prepared(&root, profile());
        if mode > 0 { original.dispatch(); }
        if mode >= 2 { original.publish(); }
        let tick = if mode == 3 { ElapsedTick(2000) } else { ElapsedTick(3) };
        let intent = request(&original.host); let e = expected(&original.host);
        let anchor = original.host.history_anchor().unwrap(); let before = root.bytes();
        let mut events = original.host.events.clone();
        events.extend([Event::Core(BaseEvent::Stop(intent)), Event::Core(BaseEvent::Fence),
            Event::Core(BaseEvent::StopProgress(tick))]);
        let mut oracle = Machine::new(&profile()).unwrap(); let mut sweep = None;
        for event in &events {
            if let Transition::StopProgressed(result) = oracle.apply(event).unwrap() { sweep = Some(result); }
        }
        let state = oracle.snapshot(events.len()); let sweep = sweep.unwrap();
        let bytes = journal::encode(&profile(), original.host.store.identity(), &events).unwrap();
        let retained = FileHistoryAnchor::from_acknowledged(events.len(), bytes.clone());
        drop(original);
        for anchored in [false, true] { for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
            fs::write(root.store().join(storage::CANONICAL), &before).unwrap();
            let mut run = if anchored {
                FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &e, &anchor).unwrap()
                    .require_recovery_reserve(RecoveryReserve::terminal()).unwrap()
            } else { begin(&root, profile(), &e) };
            ready(&mut run, quantum);
            assert_eq!(run.stop_request(intent.operation).unwrap(), intent);
            assert_eq!(root.bytes(), before, "selection is not stop acknowledgment");
            let clocks = Cell::new(0);
            let history = run.finish_stopped_with_anchor(intent, || {
                clocks.set(clocks.get() + 1); tick
            }).unwrap();
            assert_eq!(clocks.get(), 1);
            assert_eq!(history.stopped(), &FileStoppedRecovery::Advanced { snapshot: state.clone(), sweep: sweep.clone() });
            assert_eq!(history.history_anchor(), &retained); assert_eq!(root.bytes(), bytes);
            assert_eq!(history.stopped().snapshot().executions, u64::from(mode >= 2));
            assert_eq!(history.stopped().progress().reserved_units, 0);
            if mode >= 2 { assert_eq!(history.stopped().progress().charged_units, 16); }
        } }
    }
}

#[test]
fn ordinary_exhaustion_still_allows_terminal_stop_and_zero_event_drained_retry() {
    let root = Directory::new(); let mut p = profile(); p.delivery.limits.events = 40;
    let mut original = prepared(&root, p.clone()); original.dispatch(); original.publish();
    while original.host.journal_capacity().unwrap().ordinary_remaining().events > 0 {
        let tick = ElapsedTick(original.host.revision() + 3);
        original.host.observe_time(original.host.revision(), tick).unwrap();
    }
    let intent = request(&original.host); let mut e = expected(&original.host);
    let before = root.bytes(); drop(original);
    let mut live = begin(&root, p.clone(), &e); ready(&mut live, 1);
    assert!(matches!(live.finish_reconciled(|| ElapsedTick(50)), Err(JournalError::Contract(Error::Limit))));
    assert_eq!(root.bytes(), before);
    let mut stopped = begin(&root, p.clone(), &e); ready(&mut stopped, 1);
    let history = stopped.finish_stopped_with_anchor(intent, || ElapsedTick(50)).unwrap();
    assert_eq!(history.stopped().snapshot().revision, p.delivery.limits.events as u64);
    assert!(history.stopped().progress().drained());
    assert_eq!(history.stopped().progress().charged_units, 16);
    e.topology.available = false;
    let before = root.bytes();
    let mut retry = FileOversight::begin_open_mediated_guarded_anchored(root.store(), p.clone(), &e,
        history.history_anchor()).unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    ready(&mut retry, 1);
    assert_eq!(retry.stop_request(intent.operation).unwrap(), intent);
    assert_eq!(retry.stop_request(intent.operation + 1), Err(JournalError::Contract(Error::Duplicate)));
    let retried = retry.finish_stopped_with_anchor(intent, || panic!("drained retry must not sample time")).unwrap();
    assert!(matches!(retried.stopped(), FileStoppedRecovery::AlreadyDrained { .. }));
    assert_eq!(retried.stopped().snapshot(), history.stopped().snapshot());
    assert_eq!(retried.history_anchor(), history.history_anchor()); assert_eq!(root.bytes(), before);
    let mut wrong = e.clone(); wrong.topology.available = true;
    let mut run = begin(&root, p, &wrong);
    assert!(run.advance(0, MAX_JOURNAL_EVENTS).is_err());
    assert!(run.finish_stopped(intent, || panic!("wrong availability clock")).is_err());
    assert_eq!(root.bytes(), before);
}

#[test]
fn incomplete_replay_zero_operations_and_stale_predecessors_do_not_sample_time_or_change_files() {
    let root = Directory::new(); let original = prepared(&root, profile());
    let e = expected(&original.host); let intent = request(&original.host); drop(original);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap(); let before = root.files();
    let run = begin(&root, profile(), &e);
    assert_eq!(run.stop_request(intent.operation), Err(JournalError::Contract(Error::Incomplete)));
    assert!(matches!(run.finish_stopped(intent, || panic!("incomplete clock")), Err(JournalError::Contract(Error::Incomplete))));
    for mode in 0..3 {
        let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
        assert_eq!(run.stop_request(0), Err(JournalError::Contract(Error::InvalidInput)));
        let mut invalid = intent;
        match mode { 0 => invalid.operation = 0, 1 => invalid.expected_control_sequence += 1,
            _ => invalid.expected_authority_epoch += 1 }
        assert!(run.finish_stopped(invalid, || panic!("invalid intent clock")).is_err());
        assert_eq!(root.files(), before);
    }
    let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
    assert!(run.finish_stopped(intent, || ElapsedTick(3)).unwrap().progress().drained());
}

#[test]
fn full_mediation_requirements_are_rechecked_at_selection_and_terminal_completion() {
    let root = Directory::new(); let original = prepared(&root, profile());
    let e = expected(&original.host); let intent = request(&original.host); drop(original); let before = root.files();
    for mode in 0..4 {
        let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
        let selected = run.stop_request(intent.operation).unwrap();
        // Internal fault controls, not public setters or caller-selected checks.
        match mode { 0 => run.expected.topology.initial = graph(2),
            1 => run.expected.topology.current = graph(2), 2 => run.expected.topology.available = false,
            _ => run.expected.oversight.minimum.authority_epoch = u64::MAX }
        assert!(run.stop_request(intent.operation).is_err());
        assert!(run.finish_stopped_with_anchor(selected, || panic!("mismatching profile clock")).is_err());
        assert_eq!(root.files(), before);
    }
    let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
    assert!(run.finish_stopped(intent, || ElapsedTick(3)).is_ok());
}

#[test]
fn stale_or_unwinding_clock_preserves_dispatched_liability_and_staging() {
    let root = Directory::new(); let mut original = prepared(&root, profile()); original.dispatch();
    let e = expected(&original.host); let intent = request(&original.host); drop(original);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap(); let before = root.files();
    let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
    assert!(matches!(run.finish_stopped(intent, || ElapsedTick(0)), Err(JournalError::Contract(Error::Stale))));
    assert_eq!(root.files(), before);
    let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run.finish_stopped(intent, || panic!("clock unavailable"))
    })).is_err());
    assert_eq!(root.files(), before);
    let disk = FileOversight::read_mediation(root.store(), &profile(), &e).unwrap();
    assert_eq!(disk.journal.control.ledger.charged, 16); assert_eq!(disk.journal.executions, 0);
    let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
    assert!(run.finish_stopped(intent, || ElapsedTick(3)).unwrap().progress().drained());
}

#[test]
fn canonical_changes_before_or_during_clock_cannot_be_overwritten_by_terminal_recovery() {
    for during in [false, true] {
        let root = Directory::new(); let original = prepared(&root, profile());
        let e = expected(&original.host); let intent = request(&original.host); drop(original);
        let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
        fs::write(root.store().join("delivery.pending"), b"preserve").unwrap();
        if !during { fs::write(root.store().join(storage::CANONICAL), b"changed").unwrap(); }
        let clocks = Cell::new(0);
        assert!(matches!(run.finish_stopped(intent, || {
            clocks.set(clocks.get() + 1); fs::write(root.store().join(storage::CANONICAL), b"changed").unwrap();
            ElapsedTick(3)
        }), Err(JournalError::Contract(Error::Binding))));
        assert_eq!(clocks.get(), usize::from(during)); assert_eq!(root.bytes(), b"changed");
        assert_eq!(fs::read(root.store().join("delivery.pending")).unwrap(), b"preserve");
    }
}

#[test]
fn all_five_native_storage_barriers_preserve_visibility_and_exact_stop_retry() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut original = prepared(&root, profile()); original.dispatch(); original.publish();
        let mut e = expected(&original.host); let intent = request(&original.host);
        let revision = original.host.revision(); drop(original); let before = root.bytes();
        let mut run = begin(&root, profile(), &e); ready(&mut run, 1); run.inner.store.fail_once(barrier);
        assert!(matches!(run.finish_stopped_with_anchor(intent, || ElapsedTick(3)),
            Err(JournalError::Io(failure)) if failure.operation == barrier));
        let visible = barrier == JournalIo::DirectorySync; e.topology.available = !visible;
        let disk = FileOversight::read_mediation(root.store(), &profile(), &e).unwrap();
        assert_eq!(disk.journal.revision, revision + if visible { 3 } else { 0 });
        assert_eq!(disk.journal.stop.is_some(), visible); assert_eq!(disk.journal.executions, 1);
        assert_eq!(disk.journal.control.ledger.charged, 16);
        if !visible { assert_eq!(root.bytes(), before); }
        let mut retry = begin(&root, profile(), &e); ready(&mut retry, 1);
        assert_eq!(retry.stop_request(intent.operation).unwrap(), intent);
        let clocks = Cell::new(0);
        let result = retry.finish_stopped(intent, || { clocks.set(clocks.get() + 1); ElapsedTick(3) }).unwrap();
        assert_eq!(clocks.get(), usize::from(!visible)); assert!(result.progress().drained());
        assert_eq!(result.snapshot().revision, revision + 3); assert_eq!(result.progress().charged_units, 16);
    }
}

#[test]
fn interrupted_or_invalid_replay_never_gains_a_terminal_completion_escape_hatch() {
    let root = Directory::new(); let original = prepared(&root, profile());
    let e = expected(&original.host); let intent = request(&original.host);
    let anchor = original.host.history_anchor().unwrap(); let mut invalid = original.host.events.clone();
    invalid.push(Event::Core(BaseEvent::Time(ElapsedTick(0))));
    let invalid_bytes = journal::encode(&profile(), original.host.store.identity(), &invalid).unwrap();
    drop(original); let before = root.files();
    let mut run = begin(&root, profile(), &e);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run.inner.advance_checked_with(0, 1, |machine, event| {
            let _ = machine.apply(event)?; panic!("original transition interrupted")
        }, |_| Ok(()))
    })).is_err());
    assert_eq!(run.stop_request(intent.operation), Err(JournalError::Unavailable));
    assert!(matches!(run.finish_stopped(intent, || panic!("interrupted clock")), Err(JournalError::Unavailable)));
    assert_eq!(root.files(), before);
    fs::write(root.store().join(storage::CANONICAL), &invalid_bytes).unwrap();
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(root.store(), profile(), &e, &anchor).unwrap();
    assert!(run.advance(0, MAX_JOURNAL_EVENTS).is_err());
    assert!(matches!(run.progress().status, Status::Failed(_)));
    assert!(run.stop_request(intent.operation).is_err());
    assert!(run.finish_stopped(intent, || panic!("failed replay clock")).is_err());
    assert_eq!(root.bytes(), invalid_bytes);
}

#[test]
fn exact_drained_retry_cannot_accept_a_substituted_stop_predecessor_or_another_operation() {
    let root = Directory::new(); let original = prepared(&root, profile());
    let mut e = expected(&original.host); let intent = request(&original.host); drop(original);
    let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
    let history = run.finish_stopped_with_anchor(intent, || ElapsedTick(3)).unwrap();
    e.topology.available = false; let before = root.files();
    for mode in 0..3 {
        let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
        let mut wrong = intent;
        match mode { 0 => wrong.expected_authority_epoch += 1, 1 => wrong.expected_control_sequence += 1,
            _ => wrong.operation += 1 }
        let error = if mode == 2 { Error::Duplicate } else { Error::Binding };
        assert!(matches!(run.finish_stopped(wrong, || panic!("wrong retry clock")),
            Err(JournalError::Contract(actual)) if actual == error));
        assert_eq!(root.files(), before);
    }
    let mut run = begin(&root, profile(), &e); ready(&mut run, 1);
    let retried = run.finish_stopped_with_anchor(intent, || panic!("exact retry clock")).unwrap();
    assert_eq!(retried.history_anchor(), history.history_anchor()); assert_eq!(root.files(), before);
}
