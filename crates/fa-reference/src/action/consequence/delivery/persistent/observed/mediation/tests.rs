use crate::action::{ResolvedTarget, Scope};
use crate::action::consequence::mediation::*;
use crate::perimeter::*;

pub fn graph(scope: Scope, target: ResolvedTarget, generation: u64, bypass: bool) -> AuthorityGraph {
    let mut edges = vec![edge(1, 1, 2), edge(2, 2, 3)];
    if bypass { edges.push(edge(3, 1, 3)); }
    AuthorityGraph::new(GraphSpec { generation, inventory_generation: generation, scope, target,
        family: EffectFamilyRecord {
            scope: PerimeterScope { tenant: scope.tenant, principal: scope.principal, purpose: 1 },
            family: "file-publication".into(),
            trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "file-owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared graph only; no operating-system isolation claim".into()],
        },
        completeness: Completeness::DeclaredComplete { inventory_generation: generation },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer }, Node { id: 3, kind: NodeKind::Sink }],
        edges,
        enforcers: vec![Enforcer { node: 2, adapter: target.adapter, contract_version: target.contract_version,
            generation: target.generation, provenance: 90 }],
    }).unwrap()
}
fn edge(id: u64, from: u64, to: u64) -> Edge {
    Edge { id, from, to, channel: Channel::Dispatch, route: "publication".into(), provenance: id + 100 }
}

use super::{codec, MediationEvent};
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::action::Purpose;
use crate::Error;

fn fixture(bypass: bool) -> AuthorityGraph {
    graph(Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 }, 1, bypass)
}
#[test]
fn graph_wire_retains_all_input_fields_and_runs_the_original_checker_after_decoding() {
    for bypass in [false, true] {
        let graph = fixture(bypass);
        let event = MediationEvent::Enable(graph.clone());
        let mut writer = Writer::new(10000); codec::write(&mut writer, &event).unwrap();
        let bytes = writer.finish(); let mut reader = Reader::new(&bytes);
        let MediationEvent::Enable(decoded) = codec::read(&mut reader).unwrap() else { panic!("graph input"); };
        reader.end().unwrap(); assert_eq!(decoded, graph);
        let proposal = decoded.propose_cut(&[2]).unwrap();
        let result = decoded.verify_cut(&proposal, MAX_CHECK_EDGE_VISITS).unwrap();
        assert_eq!(result, graph.verify_cut(&graph.propose_cut(&[2]).unwrap(), MAX_CHECK_EDGE_VISITS).unwrap());
        assert_eq!(matches!(result, CutCheck::Bypass(_)), bypass);
        for end in 0..bytes.len() { assert!(codec::read(&mut Reader::new(&bytes[..end])).is_err(), "truncation {end}"); }
        let mut trailing = bytes; trailing.push(0); let mut reader = Reader::new(&trailing);
        codec::read(&mut reader).unwrap(); assert!(reader.end().is_err());
    }
}
#[test]
fn partition_wire_keeps_claimed_reachability_untrusted_and_checks_collection_limits() {
    let graph = fixture(false);
    let mut proposal = graph.propose_cut(&[2]).unwrap(); proposal.reachable.push(3);
    let event = MediationEvent::Check { generation: 1, epoch: 7, gates: proposal.gates.clone(),
        reachable: proposal.reachable.clone(), budget: MAX_CHECK_EDGE_VISITS };
    let mut writer = Writer::new(10000); codec::write(&mut writer, &event).unwrap(); let bytes = writer.finish();
    let MediationEvent::Check { generation, epoch, gates, reachable, budget } = codec::read(&mut Reader::new(&bytes)).unwrap()
        else { panic!("untrusted partition input"); };
    assert_eq!((generation, epoch), (1, 7)); assert_eq!(reachable, proposal.reachable);
    assert!(graph.verify_cut(&CutProposal { graph: graph.clone(), gates, reachable }, budget).is_err());
    let oversized = MediationEvent::Check { generation: 1, epoch: 0, gates: vec![2; MAX_CUT_NODES + 1],
        reachable: vec![1], budget: MAX_CHECK_EDGE_VISITS };
    assert_eq!(codec::write(&mut Writer::new(10000), &oversized), Err(Error::Limit));
    assert!(matches!(graph.verify_cut(&graph.propose_cut(&[2]).unwrap(), MAX_CHECK_EDGE_VISITS).unwrap(), CutCheck::Verified(_)));
}
