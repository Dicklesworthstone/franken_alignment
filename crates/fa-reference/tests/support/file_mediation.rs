#![allow(dead_code)]
use fa_reference::action::{ResolvedTarget, Scope};
use fa_reference::action::consequence::mediation::*;
use fa_reference::perimeter::*;

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
