//! Bounded declarative inputs. No verified cut or topology verdict is decoded.
use super::{FileMediationUpdate, MediationEvent};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::mediation::{
    AuthorityGraph, Channel, Completeness, Edge, Enforcer, GraphSpec, Node, NodeKind,
    MAX_CHECK_EDGE_VISITS, MAX_CUT_NODES, MAX_EDGES, MAX_NODES,
};
use crate::perimeter::{
    BypassDisposition, CredentialExposure, CredentialHolder, EffectFamilyRecord, Mediation,
    PerimeterScope, RouteRecord, ThreatClass, TrustDomain, MAX_CREDENTIALS_PER_FAMILY,
    MAX_RESIDUAL_NONCLAIMS, MAX_ROUTES_PER_FAMILY, MAX_TEXT_BYTES,
};
use crate::Error;

pub(in super::super) fn write(w: &mut Writer, event: &MediationEvent) -> Result<(), Error> {
    match event {
        MediationEvent::Enable(graph) => { w.u8(0)?; write_graph(w, graph)?; }
        MediationEvent::Check { generation, epoch, gates, reachable, budget } => {
            if gates.len() > MAX_CUT_NODES || reachable.len() > MAX_NODES || *budget > MAX_CHECK_EDGE_VISITS {
                return Err(Error::Limit);
            }
            w.u8(1)?; w.u64(*generation)?; w.u64(*epoch)?;
            w.count(gates.len())?; for id in gates { w.u64(*id)?; }
            w.count(reachable.len())?; for id in reachable { w.u64(*id)?; }
            w.count(*budget)?;
        }
        MediationEvent::Update(update) => {
            if update.operation == 0 { return Err(Error::InvalidInput); }
            w.u8(2)?; w.u64(update.operation)?; w.u64(update.expected_generation)?;
            w.u64(update.expected_authority_epoch)?;
            match &update.next { None => w.u8(0)?, Some(graph) => { w.u8(1)?; write_graph(w, graph)?; } }
        }
    }
    Ok(())
}
pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<MediationEvent, Error> {
    Ok(match r.u8()? {
        0 => MediationEvent::Enable(read_graph(r)?),
        1 => MediationEvent::Check { generation: r.u64()?, epoch: r.u64()?,
            gates: items(r, MAX_CUT_NODES, |r| r.u64())?, reachable: items(r, MAX_NODES, |r| r.u64())?,
            budget: r.count(MAX_CHECK_EDGE_VISITS)? },
        2 => {
            let operation = r.u64()?; if operation == 0 { return Err(Error::InvalidInput); }
            let expected_generation = r.u64()?; let expected_authority_epoch = r.u64()?;
            let next = match r.u8()? { 0 => None, 1 => Some(read_graph(r)?), _ => return Err(Error::InvalidInput) };
            MediationEvent::Update(FileMediationUpdate { operation, expected_generation, expected_authority_epoch, next })
        }
        _ => return Err(Error::InvalidInput),
    })
}
fn text(w: &mut Writer, s: &str) -> Result<(), Error> {
    if s.is_empty() { return Err(Error::InvalidInput); }
    if s.len() > MAX_TEXT_BYTES { return Err(Error::Limit); }
    w.blob(s.as_bytes())
}
fn read_text(r: &mut Reader<'_>) -> Result<String, Error> {
    let s = std::str::from_utf8(r.blob(MAX_TEXT_BYTES)?).map_err(|_| Error::InvalidInput)?;
    if s.is_empty() { return Err(Error::InvalidInput); }
    Ok(s.to_owned())
}
fn items<'a, T>(r: &mut Reader<'a>, max: usize,
    mut read: impl FnMut(&mut Reader<'a>) -> Result<T, Error>) -> Result<Vec<T>, Error>
{
    let n = r.count(max)?; let mut result = Vec::new();
    result.try_reserve_exact(n).map_err(|_| Error::Limit)?;
    for _ in 0..n { result.push(read(r)?); }
    Ok(result)
}
fn write_graph(w: &mut Writer, graph: &AuthorityGraph) -> Result<(), Error> {
    let s = graph.spec();
    w.u64(s.generation)?; w.u64(s.inventory_generation)?; w.scope(s.scope)?; w.target(s.target)?;
    let f = &s.family;
    w.u64(f.scope.tenant)?; w.u64(f.scope.principal)?; w.u64(f.scope.purpose)?; text(w, &f.family)?;
    w.count(f.trust_domains.len())?;
    for d in &f.trust_domains { w.u8(match d { TrustDomain::Actor => 0, TrustDomain::ObservationAndAnalysis => 1,
        TrustDomain::Enforcement => 2, TrustDomain::GovernanceAndInvestigation => 3 })?; }
    w.count(f.credentials.len())?;
    for c in &f.credentials { text(w, &c.credential)?; w.u8(match c.holder { CredentialHolder::ActorDirect => 0, CredentialHolder::Broker => 1 })?; }
    w.count(f.routes.len())?;
    for route in &f.routes {
        text(w, &route.route)?;
        w.u8(match route.threat { None => 0, Some(ThreatClass::GuardrailRemoval) => 1, Some(ThreatClass::PromptInjection) => 2,
            Some(ThreatClass::PayloadSubstitution) => 3, Some(ThreatClass::DirectCredentialOrEgress) => 4,
            Some(ThreatClass::UnknownRemoteOutcome) => 5 })?;
        w.u8(match route.mediation { Mediation::ObserveOnly => 0, Mediation::CooperativeGate => 1, Mediation::BrokeredEffects => 2 })?;
        w.u8(match route.bypass { BypassDisposition::Blocked => 0, BypassDisposition::ResidualUncovered => 1, BypassDisposition::Unmodeled => 2 })?;
    }
    w.count(f.residual_nonclaims.len())?; for s in &f.residual_nonclaims { text(w, s)?; }
    match s.completeness { Completeness::Unknown => w.u8(0)?, Completeness::DeclaredComplete { inventory_generation } => {
        w.u8(1)?; w.u64(inventory_generation)?;
    } }
    w.count(s.nodes.len())?;
    for node in &s.nodes { w.u64(node.id)?; w.u8(match node.kind { NodeKind::Actor => 0, NodeKind::Process => 1,
        NodeKind::Credential => 2, NodeKind::Enforcer => 3, NodeKind::Sink => 4 })?; }
    w.count(s.edges.len())?;
    for edge in &s.edges {
        w.u64(edge.id)?; w.u64(edge.from)?; w.u64(edge.to)?;
        w.u8(match edge.channel { Channel::Delegation => 0, Channel::Execution => 1, Channel::Ipc => 2,
            Channel::CredentialAccess => 3, Channel::Dispatch => 4 })?;
        text(w, &edge.route)?; w.u64(edge.provenance)?;
    }
    w.count(s.enforcers.len())?;
    for gate in &s.enforcers {
        for n in [gate.node, gate.adapter, gate.contract_version, gate.generation, gate.provenance] { w.u64(n)?; }
    }
    Ok(())
}
fn read_graph(r: &mut Reader<'_>) -> Result<AuthorityGraph, Error> {
    let generation = r.u64()?; let inventory_generation = r.u64()?;
    let scope = r.scope()?; let target = r.target()?;
    let perimeter_scope = PerimeterScope { tenant: r.u64()?, principal: r.u64()?, purpose: r.u64()? };
    let family = read_text(r)?;
    let trust_domains = items(r, 4, |r| Ok(match r.u8()? { 0 => TrustDomain::Actor, 1 => TrustDomain::ObservationAndAnalysis,
        2 => TrustDomain::Enforcement, 3 => TrustDomain::GovernanceAndInvestigation, _ => return Err(Error::InvalidInput) }))?;
    let credentials = items(r, MAX_CREDENTIALS_PER_FAMILY, |r| Ok(CredentialExposure {
        credential: read_text(r)?, holder: match r.u8()? { 0 => CredentialHolder::ActorDirect,
            1 => CredentialHolder::Broker, _ => return Err(Error::InvalidInput) },
    }))?;
    let routes = items(r, MAX_ROUTES_PER_FAMILY, |r| Ok(RouteRecord {
        route: read_text(r)?,
        threat: match r.u8()? { 0 => None, 1 => Some(ThreatClass::GuardrailRemoval), 2 => Some(ThreatClass::PromptInjection),
            3 => Some(ThreatClass::PayloadSubstitution), 4 => Some(ThreatClass::DirectCredentialOrEgress),
            5 => Some(ThreatClass::UnknownRemoteOutcome), _ => return Err(Error::InvalidInput) },
        mediation: match r.u8()? { 0 => Mediation::ObserveOnly, 1 => Mediation::CooperativeGate,
            2 => Mediation::BrokeredEffects, _ => return Err(Error::InvalidInput) },
        bypass: match r.u8()? { 0 => BypassDisposition::Blocked, 1 => BypassDisposition::ResidualUncovered,
            2 => BypassDisposition::Unmodeled, _ => return Err(Error::InvalidInput) },
    }))?;
    let residual_nonclaims = items(r, MAX_RESIDUAL_NONCLAIMS, read_text)?;
    let family = EffectFamilyRecord { scope: perimeter_scope, family, trust_domains, credentials, routes, residual_nonclaims };
    let completeness = match r.u8()? { 0 => Completeness::Unknown,
        1 => Completeness::DeclaredComplete { inventory_generation: r.u64()? }, _ => return Err(Error::InvalidInput) };
    let nodes = items(r, MAX_NODES, |r| Ok(Node { id: r.u64()?, kind: match r.u8()? { 0 => NodeKind::Actor,
        1 => NodeKind::Process, 2 => NodeKind::Credential, 3 => NodeKind::Enforcer, 4 => NodeKind::Sink,
        _ => return Err(Error::InvalidInput) } }))?;
    let edges = items(r, MAX_EDGES, |r| Ok(Edge { id: r.u64()?, from: r.u64()?, to: r.u64()?,
        channel: match r.u8()? { 0 => Channel::Delegation, 1 => Channel::Execution, 2 => Channel::Ipc,
            3 => Channel::CredentialAccess, 4 => Channel::Dispatch, _ => return Err(Error::InvalidInput) },
        route: read_text(r)?, provenance: r.u64()?,
    }))?;
    let enforcers = items(r, MAX_CUT_NODES, |r| Ok(Enforcer { node: r.u64()?, adapter: r.u64()?,
        contract_version: r.u64()?, generation: r.u64()?, provenance: r.u64()? }))?;
    // Reuse the ORIGINAL full graph and perimeter validation; no imported cut.
    AuthorityGraph::new(GraphSpec { generation, inventory_generation, scope, target, family, completeness,
        nodes, edges, enforcers })
}
