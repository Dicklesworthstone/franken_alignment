//! Public input-to-original-checker controls. Fixtures do not attest a live host.
use fa_reference::action::consequence::mediation::{
    AuthorityGraph, Channel, Completeness, CutCheck, Edge, Enforcer, GraphSpec, MAX_CUT_NODES,
    MAX_EDGES, MAX_NODES, Node, NodeKind,
};
use fa_reference::action::{Purpose, ResolvedTarget, Scope};
use fa_reference::perimeter::*;
use fa_reference::perimeter_inventory::{InventoryLoadError, MAX_INVENTORY_BYTES};
use fa_reference::perimeter_inventory::topology::{
    LoadedAuthorityTopology, MAX_TOPOLOGY_BYTES, MAX_TOPOLOGY_SOURCE_CHECKS,
    MAX_TOPOLOGY_SOURCE_READ_BYTES,
};
use fa_reference::Error;

const TOPOLOGY: &str = include_str!("fixtures/authority_topology.json");
const INVENTORY: &str = include_str!("fixtures/authority_topology_inventory.json");
fn load(topology: &str, inventory: &str) -> Result<LoadedAuthorityTopology, InventoryLoadError> {
    LoadedAuthorityTopology::from_json_bytes(topology.as_bytes(), inventory.as_bytes())
}
fn fixture() -> LoadedAuthorityTopology { load(TOPOLOGY, INVENTORY).unwrap() }
fn original_graph() -> AuthorityGraph {
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    AuthorityGraph::new(GraphSpec {
        generation: 1, inventory_generation: 1, scope, target,
        family: EffectFamilyRecord {
            scope: PerimeterScope { tenant: 1, principal: 2, purpose: 1 },
            family: "file-publication".into(),
            trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "file-owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared topology is not authenticated deployment coverage".into()],
        },
        completeness: Completeness::DeclaredComplete { inventory_generation: 1 },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer },
            Node { id: 3, kind: NodeKind::Sink }],
        edges: vec![Edge { id: 1, from: 1, to: 2, channel: Channel::Dispatch, route: "publication".into(), provenance: 101 },
            Edge { id: 2, from: 2, to: 3, channel: Channel::Dispatch, route: "publication".into(), provenance: 102 }],
        enforcers: vec![Enforcer { node: 2, adapter: 10, contract_version: 1, generation: 1, provenance: 90 }],
    }).unwrap()
}
fn add_edge(topology: &str, edge: &str) -> String {
    topology.replacen("\"edges\": [", &format!("\"edges\": [{edge},"), 1)
}
// Fixed test fixture arrays contain no nested arrays or bracket-bearing strings.
fn replace_array(topology: &str, field: &str, body: &str) -> String {
    let marker = format!("\"{field}\": [");
    let start = topology.find(&marker).unwrap() + marker.len();
    let end = start + topology[start..].find(']').unwrap();
    format!("{}{}{}", &topology[..start], body, &topology[end..])
}

#[test]
fn complete_input_builds_the_original_graph_and_independent_cut_result() {
    let loaded = fixture();
    let expected = original_graph();
    assert_eq!(loaded.graph(), &expected);
    assert_eq!(loaded.proposal(), &expected.propose_cut(&[2]).unwrap());
    assert_eq!(loaded.check(), expected.verify_cut(loaded.proposal(), 4096));
    let CutCheck::Verified(cut) = loaded.check().unwrap() else { panic!("original verified cut"); };
    assert_eq!(cut.gates(), &[2]);
    assert_eq!(cut.reachable(), &[1]);
    assert_eq!(loaded.inventory().family_count(), 1);
    let route = loaded.inventory().route_for(PerimeterScope { tenant: 1, principal: 2, purpose: 1 },
        "file-publication", "publication").unwrap();
    assert_eq!(route.metadata().profile().id(), "reference-publication");
    assert_eq!(loaded.source_limits().checks, 4096);
}

#[test]
fn bypass_and_unreachable_remain_distinct_from_certification() {
    assert!(matches!(fixture().check(), Ok(CutCheck::Verified(_))));
    let bypass = add_edge(TOPOLOGY,
        r#"{"id":3,"from":1,"to":3,"channel":"ipc","route":"publication","provenance":103}"#);
    let CutCheck::Bypass(path) = load(&bypass, INVENTORY).unwrap().check().unwrap()
        else { panic!("original bypass result"); };
    assert_eq!(path.nodes, [1, 3]);
    assert_eq!(path.edges, [3]);
    let disconnected = replace_array(TOPOLOGY, "edges",
        r#"{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}"#);
    assert_eq!(load(&disconnected, INVENTORY).unwrap().check(), Ok(CutCheck::Unreachable { sinks: vec![3] }));
}

#[test]
fn entire_selected_family_is_required_not_only_routes_named_in_the_graph() {
    let extra = r#"{"route":"other","effect":"file_write",
        "profile":{"id":"other-profile","generation":1},"trust_path":["actor","enforcement"],
        "threat":"direct_credential_or_egress","actor_credential":{"kind":"broker_mediated"},
        "mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["fixture only"]}"#;
    let inventory = INVENTORY.replacen("\"routes\": [", &format!("\"routes\": [{extra},"), 1);
    let incomplete = load(TOPOLOGY, &inventory).unwrap();
    assert_eq!(incomplete.graph().spec().family.routes.len(), 2);
    assert_eq!(incomplete.check(), Err(Error::Incomplete));
    let complete = add_edge(TOPOLOGY,
        r#"{"id":3,"from":1,"to":2,"channel":"dispatch","route":"other","provenance":103}"#);
    assert!(matches!(load(&complete, &inventory).unwrap().check(), Ok(CutCheck::Verified(_))));
}

#[test]
fn metadata_only_substitution_changes_loaded_binding_without_rewriting_graph_semantics() {
    let original = fixture();
    for (before, after) in [
        ("reference-publication", "another-profile"),
        ("Controlled fixture, not evidence of host isolation", "Different residual claim"),
        ("\"generation\": 1", "\"generation\": 2"),
        ("[\"actor\", \"enforcement\"]", "[\"actor\", \"observation_and_analysis\"]"),
    ] {
        assert!(INVENTORY.contains(before));
        let changed = load(TOPOLOGY, &INVENTORY.replace(before, after)).unwrap();
        assert_eq!(original.graph(), changed.graph());
        assert_ne!(original, changed, "metadata must remain bound: {before}");
    }
}

#[test]
fn strict_schema_refuses_missing_unknown_duplicate_and_wrongly_typed_fields() {
    for (before, after) in [
        ("fa.authority-topology/1", "fa.authority-topology/2"),
        ("\"generation\": 1,", "\"generation\": 1, \"unknown\": 9,"),
        ("\"generation\": 1,", "\"generation\": 1, \"generation\": 1,"),
        ("\"gates\": [2],", ""),
        ("\"id\": 1", "\"id\": \"1\""),
        ("\"provenance\": 101", "\"provenance\": null"),
        ("\"kind\": \"actor\"", "\"kind\": \"unknown\""),
        ("\"channel\": \"dispatch\"", "\"channel\": \"socket_guess\""),
        ("\"purpose\": \"effect\"", "\"purpose\": \"experiment\""),
    ] {
        assert!(TOPOLOGY.contains(before));
        assert!(load(&TOPOLOGY.replacen(before, after, 1), INVENTORY).is_err(), "{before} -> {after}");
    }
    assert!(load(TOPOLOGY, &INVENTORY.replacen("\"version\": 1", "\"version\": 1,\"extra\": true", 1)).is_err());
    assert!(LoadedAuthorityTopology::from_json_bytes(&[255], INVENTORY.as_bytes()).is_err());
    assert!(matches!(fixture().check(), Ok(CutCheck::Verified(_))));
}

#[test]
fn duplicates_missing_nodes_and_non_enforcer_cut_sites_use_original_refusals() {
    let duplicate = add_edge(TOPOLOGY,
        r#"{"id":1,"from":1,"to":2,"channel":"ipc","route":"publication","provenance":999}"#);
    assert_eq!(load(&duplicate, INVENTORY), Err(InventoryLoadError::Schema(Error::Duplicate)));
    let missing = TOPOLOGY.replace("\"to\": 3", "\"to\": 99");
    assert_eq!(load(&missing, INVENTORY), Err(InventoryLoadError::Schema(Error::Missing)));
    for gates in ["1", "3", "99", "2,2", ""] {
        assert!(load(&replace_array(TOPOLOGY, "gates", gates), INVENTORY).is_err());
    }
    let no_provenance = TOPOLOGY.replace("\"provenance\": 101", "\"provenance\": 0");
    assert_eq!(load(&no_provenance, INVENTORY), Err(InventoryLoadError::Schema(Error::InvalidInput)));
}

#[test]
fn independent_scope_target_generation_and_completeness_are_not_inferred() {
    let wrong_family = TOPOLOGY.replace("\"purpose\": 1", "\"purpose\": 2");
    assert_eq!(load(&wrong_family, INVENTORY), Err(InventoryLoadError::Schema(Error::Missing)));
    let wrong_tenant = TOPOLOGY.replacen("\"tenant\": 1", "\"tenant\": 9", 1);
    assert_eq!(load(&wrong_tenant, INVENTORY), Err(InventoryLoadError::Schema(Error::Binding)));
    let wrong_adapter = TOPOLOGY.replacen("\"adapter\": 10", "\"adapter\": 9", 1);
    assert_eq!(load(&wrong_adapter, INVENTORY), Err(InventoryLoadError::Schema(Error::Binding)));
    let stale = TOPOLOGY.replace("\"complete_inventory_generation\": 1", "\"complete_inventory_generation\": 2");
    assert_eq!(load(&stale, INVENTORY).unwrap().check(), Err(Error::Stale));
    let other_run = load(&TOPOLOGY.replace("\"run\": 3", "\"run\": 6"), INVENTORY).unwrap();
    assert!(!other_run.graph().binds(original_graph().spec().scope, original_graph().spec().target));
}

#[test]
fn parallel_channels_and_provenance_are_not_collapsed() {
    for (name, channel) in [("delegation", Channel::Delegation), ("execution", Channel::Execution),
        ("ipc", Channel::Ipc), ("credential_access", Channel::CredentialAccess), ("dispatch", Channel::Dispatch)] {
        let topology = add_edge(TOPOLOGY, &format!(r#"{{"id":3,"from":1,"to":2,"channel":"{name}","route":"publication","provenance":333}}"#));
        let loaded = load(&topology, INVENTORY).unwrap();
        assert_eq!(loaded.graph().spec().edges.len(), 3);
        assert_eq!(loaded.graph().spec().edges[2].channel, channel);
        assert_eq!(loaded.graph().spec().edges[2].provenance, 333);
        assert!(matches!(loaded.check(), Ok(CutCheck::Verified(_))));
    }
}

#[test]
fn original_checker_consumes_exact_and_one_short_edge_budgets() {
    let CutCheck::Verified(cut) = fixture().check().unwrap() else { panic!("verified"); };
    let work = cut.edge_visits();
    assert!(work > 1);
    let exact = TOPOLOGY.replace("\"check_edge_budget\": 4096", &format!("\"check_edge_budget\": {work}"));
    assert!(matches!(load(&exact, INVENTORY).unwrap().check(), Ok(CutCheck::Verified(_))));
    let short = TOPOLOGY.replace("\"check_edge_budget\": 4096", &format!("\"check_edge_budget\": {}", work - 1));
    assert_eq!(load(&short, INVENTORY).unwrap().check(), Err(Error::Limit));
    assert!(load(&TOPOLOGY.replace("\"check_edge_budget\": 4096", "\"check_edge_budget\": 0"), INVENTORY).is_err());
}

#[test]
fn full_byte_limits_admit_exact_inputs_and_refuse_one_more_before_parsing() {
    let mut topology = TOPOLOGY.as_bytes().to_vec(); topology.resize(MAX_TOPOLOGY_BYTES, b' ');
    let mut inventory = INVENTORY.as_bytes().to_vec(); inventory.resize(MAX_INVENTORY_BYTES, b' ');
    assert!(LoadedAuthorityTopology::from_json_bytes(&topology, &inventory).is_ok());
    topology.push(b' ');
    assert_eq!(LoadedAuthorityTopology::from_json_bytes(&topology, &inventory), Err(InventoryLoadError::Schema(Error::Limit)));
    topology.pop(); inventory.push(b' ');
    assert_eq!(LoadedAuthorityTopology::from_json_bytes(&topology, &inventory), Err(InventoryLoadError::Schema(Error::Limit)));
}

#[test]
fn collection_and_lifetime_source_limits_refuse_without_enlarging_original_caps() {
    for (field, count, item) in [
        ("nodes", MAX_NODES + 1, r#"{"id":1,"kind":"actor"}"#),
        ("edges", MAX_EDGES + 1, r#"{"id":1,"from":1,"to":2,"channel":"ipc","route":"publication","provenance":1}"#),
        ("enforcers", MAX_CUT_NODES + 1, r#"{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":1}"#),
        ("gates", MAX_CUT_NODES + 1, "2"),
    ] {
        let body = vec![item; count].join(",");
        assert_eq!(load(&replace_array(TOPOLOGY, field, &body), INVENTORY), Err(InventoryLoadError::Schema(Error::Limit)));
    }
    for (before, after) in [
        ("\"checks\": 4096", format!("\"checks\": {}", MAX_TOPOLOGY_SOURCE_CHECKS + 1)),
        ("\"read_bytes\": 16777216", format!("\"read_bytes\": {}", MAX_TOPOLOGY_SOURCE_READ_BYTES + 1)),
    ] {
        assert_eq!(load(&TOPOLOGY.replace(before, &after), INVENTORY), Err(InventoryLoadError::Schema(Error::Limit)));
    }
    assert!(load(&TOPOLOGY.replace("\"checks\": 4096", "\"checks\": 0"), INVENTORY).is_err());
}

#[test]
fn nonbrokered_inventory_is_valid_data_but_not_a_mediation_certificate() {
    let cooperative = INVENTORY.replace("brokered_effects", "cooperative_gate");
    assert_eq!(load(TOPOLOGY, &cooperative).unwrap().check(), Err(Error::Binding));
    let unknown = INVENTORY.replace("\"bypass\": \"blocked\"", "\"bypass\": \"unmodeled\"");
    assert_eq!(load(TOPOLOGY, &unknown), Err(InventoryLoadError::Schema(Error::Binding)));
    assert!(matches!(fixture().check(), Ok(CutCheck::Verified(_))));
}
