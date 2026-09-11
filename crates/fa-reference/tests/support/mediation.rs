use fa_reference::action::consequence::mediation::*;
use fa_reference::action::{Purpose, ResolvedTarget, Scope};
use fa_reference::perimeter::*;

pub fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
pub fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }
}
pub fn spec(generation: u64) -> GraphSpec {
    GraphSpec {
        generation, inventory_generation: generation, scope: scope(), target: target(),
        completeness: Completeness::DeclaredComplete { inventory_generation: generation },
        family: EffectFamilyRecord {
            scope: PerimeterScope { tenant: 1, principal: 2, purpose: 9 }, family: "publication".into(),
            trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "publisher".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publish".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared graph only; no OS containment or authenticity claim".into()],
        },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Process },
            Node { id: 3, kind: NodeKind::Enforcer }, Node { id: 4, kind: NodeKind::Credential },
            Node { id: 5, kind: NodeKind::Sink }],
        edges: vec![edge(1, 1, 2), edge(2, 2, 3), edge(3, 3, 4), edge(4, 4, 5)],
        enforcers: vec![Enforcer { node: 3, adapter: 1, contract_version: 1, generation: 1, provenance: 10 }],
    }
}
pub fn edge(id: u64, from: u64, to: u64) -> Edge {
    Edge { id, from, to, channel: Channel::Dispatch, route: "publish".into(), provenance: id + 10 }
}
pub fn graph(generation: u64) -> AuthorityGraph { AuthorityGraph::new(spec(generation)).unwrap() }
