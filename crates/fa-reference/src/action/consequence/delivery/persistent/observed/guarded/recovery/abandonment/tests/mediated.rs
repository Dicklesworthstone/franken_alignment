//! Complete native topology requirements survive the restrictive consumer.
use super::*;
use crate::action::consequence::delivery::persistent::observed::guarded::mediated::{
    FileMediatedRecovery, FileMediatedRequirements, FileTopologyRequirement,
};
use crate::action::consequence::mediation::{
    AuthorityGraph, Channel, Completeness, CutCheck, Edge, Enforcer, GraphSpec,
    MAX_CHECK_EDGE_VISITS, Node, NodeKind,
};
use crate::perimeter::*;

fn graph(generation: u64) -> AuthorityGraph {
    let p = profile().delivery;
    AuthorityGraph::new(GraphSpec {
        generation,
        inventory_generation: generation,
        scope: p.scope,
        target: p.target,
        family: EffectFamilyRecord {
            scope: PerimeterScope {
                tenant: 1,
                principal: 2,
                purpose: 1,
            },
            family: "publication".into(),
            trust_domains: vec![
                TrustDomain::Actor,
                TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement,
                TrustDomain::GovernanceAndInvestigation,
            ],
            credentials: vec![CredentialExposure {
                credential: "owner".into(),
                holder: CredentialHolder::Broker,
            }],
            routes: vec![RouteRecord {
                route: "publication".into(),
                threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects,
                bypass: BypassDisposition::Blocked,
            }],
            residual_nonclaims: vec!["Declared topology only".into()],
        },
        completeness: Completeness::DeclaredComplete {
            inventory_generation: generation,
        },
        nodes: vec![
            Node {
                id: 1,
                kind: NodeKind::Actor,
            },
            Node {
                id: 2,
                kind: NodeKind::Enforcer,
            },
            Node {
                id: 3,
                kind: NodeKind::Sink,
            },
        ],
        edges: vec![
            Edge {
                id: 1,
                from: 1,
                to: 2,
                channel: Channel::Dispatch,
                route: "publication".into(),
                provenance: 1,
            },
            Edge {
                id: 2,
                from: 2,
                to: 3,
                channel: Channel::Dispatch,
                route: "publication".into(),
                provenance: 2,
            },
        ],
        enforcers: vec![Enforcer {
            node: 2,
            adapter: p.target.adapter,
            contract_version: 1,
            generation: 1,
            provenance: 9,
        }],
    })
    .unwrap()
}

fn prepared(root: &Directory, mode: u64) -> Ready {
    let (mut host, roles) =
        FileOversight::create_guarded(root.store(), profile(), &guards(), None).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
        .unwrap();
    let observer = host.enable_mediation(host.revision(), graph(1)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = graph(1).propose_cut(&[2]).unwrap();
    let revision = host.revision();
    let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(
        observer
            .certify(&mut host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS)
            .unwrap()
            .unwrap(),
        CutCheck::Verified(_)
    ));
    let mut live = Ready::from_owner(host, roles.human, 1, 101, ElapsedTick(1));
    unknown(&mut live, mode);
    live
}

fn expected(host: &FileOversight, available: bool) -> FileMediatedRequirements {
    FileMediatedRequirements {
        oversight: requirements(host, guards()),
        topology: FileTopologyRequirement {
            initial: graph(1),
            current: graph(1),
            available,
        },
        prediction: None,
        evaluation: None,
    }
}

fn ready(run: &mut FileMediatedRecovery, quantum: usize) {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let old = run.progress();
        let next = run.advance(old.replayed_events, quantum).unwrap();
        assert_eq!(
            next.replayed_events - old.replayed_events,
            quantum.min(old.total_events - old.replayed_events)
        );
        assert_eq!(next.attempted_events, next.replayed_events);
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
}

#[test]
fn original_mediated_cursor_preserves_available_and_withdrawn_topology_without_a_new_fence() {
    for mode in 0..3 {
        let root = Directory::new();
        let live = prepared(&root, mode);
        let intent = request(&live.host);
        let expected = expected(&live.host, mode < 2);
        let anchor = live.host.history_anchor().unwrap();
        let before = live.host.inspect();
        let original = root.bytes();
        let mut events = live.host.events.clone();
        events.push(Event::Core(BaseEvent::AbandonUnknown(intent)));
        let machine = Machine::replay(&profile(), &events).unwrap();
        let state = machine.snapshot(events.len());
        let bytes = journal::encode(&profile(), live.host.store.identity(), &events).unwrap();
        drop(live);
        for anchored in [false, true] {
            for quantum in [1, 3, MAX_JOURNAL_EVENTS] {
                fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
                assert!(
                    FileOversight::begin_open_guarded(root.store(), profile(), &expected.oversight)
                        .is_err()
                );
                let mut run = if anchored {
                    FileOversight::begin_open_mediated_guarded_anchored(
                        root.store(),
                        profile(),
                        &expected,
                        &anchor,
                    )
                    .unwrap()
                } else {
                    FileOversight::begin_open_mediated_guarded(root.store(), profile(), &expected)
                        .unwrap()
                };
                ready(&mut run, quantum);
                let history = run.finish_abandoned_with_anchor(intent).unwrap();
                assert_eq!(history.snapshot(), &state);
                assert_eq!(root.bytes(), bytes);
                assert_eq!(history.snapshot().revision, before.revision + 1);
                assert_eq!(history.snapshot().dispatcher_epoch, before.dispatcher_epoch);
                assert_eq!(
                    history.snapshot().control.ledger.epoch,
                    before.control.ledger.epoch
                );
                let actual =
                    FileOversight::read_mediation(root.store(), &profile(), &expected).unwrap();
                assert_eq!(actual.journal, state);
                assert_eq!(actual.topology.available, mode < 2);
                let mut retry = FileOversight::begin_open_mediated_guarded_anchored(
                    root.store(),
                    profile(),
                    &expected,
                    history.history_anchor(),
                )
                .unwrap();
                ready(&mut retry, quantum);
                assert_eq!(retry.finish_abandoned_with_anchor(intent).unwrap(), history);
                assert_eq!(root.bytes(), bytes);
            }
        }
    }
}

#[test]
fn omitted_or_mismatching_native_mediation_requirements_never_dispose_or_clean_staging() {
    let root = Directory::new();
    let live = prepared(&root, 0);
    let intent = request(&live.host);
    let expected = expected(&live.host, true);
    let anchor = live.host.history_anchor().unwrap();
    drop(live);
    fs::write(root.store().join("delivery.pending"), b"retain").unwrap();
    let before = root.files();
    for mismatch in 0..5 {
        let mut wrong = expected.clone();
        match mismatch {
            0 => wrong.topology.initial = graph(2),
            1 => wrong.topology.current = graph(2),
            2 => wrong.topology.available = false,
            3 => wrong.oversight.minimum.authority_epoch = u64::MAX,
            _ => {
                wrong.oversight.guards.stream = Some(
                    crate::action::consequence::delivery::stream::StreamProfile::new(
                        1, 1, 2, 64, 128,
                    )
                    .unwrap(),
                )
            }
        }
        if let Ok(mut run) = FileOversight::begin_open_mediated_guarded_anchored(
            root.store(),
            profile(),
            &wrong,
            &anchor,
        ) {
            assert!(run.advance(0, MAX_JOURNAL_EVENTS).is_err());
            assert!(run.finish_abandoned_with_anchor(intent).is_err());
        }
        assert_eq!(root.files(), before);
    }
    let run = FileOversight::begin_open_mediated_guarded_anchored(
        root.store(),
        profile(),
        &expected,
        &anchor,
    )
    .unwrap();
    assert_eq!(
        run.finish_abandoned_with_anchor(intent),
        Err(Error::Incomplete.into())
    );
    assert_eq!(root.files(), before);
    let mut run = FileOversight::begin_open_mediated_guarded_anchored(
        root.store(),
        profile(),
        &expected,
        &anchor,
    )
    .unwrap();
    ready(&mut run, 1);
    fs::write(root.store().join(storage::CANONICAL), b"changed custody").unwrap();
    let before = root.files();
    assert_eq!(
        run.finish_abandoned_with_anchor(intent),
        Err(Error::Binding.into())
    );
    assert_eq!(root.files(), before);
}
