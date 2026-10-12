//! Explicit operator topology inputs for the ORIGINAL authority graph and cut checker.
//! Parsing never certifies a cut or proves the declared inventory complete.

use super::{
    InventoryLoadError, LoadedPerimeterInventory, MAX_INVENTORY_BYTES, MAX_INVENTORY_JSON_ITEMS,
    MAX_TEXT_BYTES, array, exact_fields, object, parse_scope, required, schema, text, unsigned,
};
use crate::action::consequence::mediation::{
    AuthorityGraph, Channel, Completeness, CutCheck, CutProposal, Edge, Enforcer, GraphSpec,
    MAX_CHECK_EDGE_VISITS, MAX_CUT_NODES, MAX_EDGES, MAX_NODES, Node, NodeKind,
};
use crate::action::{Purpose, ResolvedTarget, Scope};
use crate::strict_json::{self, Json, Limits};
use crate::Error;

pub const TOPOLOGY_SCHEMA: &str = "fa.authority-topology/1";
/// Separate from the unchanged original inventory byte allowance.
pub const MAX_TOPOLOGY_BYTES: usize = 1024 * 1024;
pub const MAX_TOPOLOGY_SOURCE_CHECKS: u64 = 65_536;
pub const MAX_TOPOLOGY_SOURCE_READ_BYTES: u64 = 1024 * 1024 * 1024;

/// Lifetime allowances for an operator source custodian. Read bytes count
/// conservatively reserved read capacity, not filesystem/device traffic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopologySourceLimits {
    pub checks: u64,
    pub read_bytes: u64,
}

/// Complete loaded inventory metadata is retained alongside the selected graph.
/// Equality therefore notices a changed route profile, trust path, residual or
/// unselected family even when the selected graph's classification is unchanged.
/// This object contains declarations and an UNTRUSTED candidate, not authority.
///
/// ```compile_fail,E0308
/// use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
/// use fa_reference::action::Permit;
/// fn grant(config: LoadedAuthorityTopology) -> Permit { config }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedAuthorityTopology {
    inventory: LoadedPerimeterInventory,
    proposal: CutProposal,
    check_edge_budget: usize,
    source_limits: TopologySourceLimits,
}

impl LoadedAuthorityTopology {
    /// Both inputs use closed, independently bounded schemas. The inventory is
    /// parsed by its original loader, not a second permissive family decoder.
    /// No path, credential, certificate or precomputed reachable set is accepted.
    pub fn from_json_bytes(topology: &[u8], inventory: &[u8]) -> Result<Self, InventoryLoadError> {
        if topology.len() > MAX_TOPOLOGY_BYTES || inventory.len() > MAX_INVENTORY_BYTES {
            return Err(schema(Error::Limit));
        }
        let value = strict_json::parse(topology, Limits {
            max_bytes: MAX_TOPOLOGY_BYTES,
            max_depth: 8,
            max_items: MAX_INVENTORY_JSON_ITEMS,
            max_string_bytes: MAX_TEXT_BYTES,
        }).map_err(InventoryLoadError::Parse)?;
        let root = object(&value)?;
        exact_fields(root, &[
            "schema", "generation", "inventory_generation", "complete_inventory_generation",
            "scope", "target", "inventory_scope", "family", "nodes", "edges", "enforcers",
            "gates", "check_edge_budget", "source_limits",
        ])?;
        if text(required(root, "schema")?)? != TOPOLOGY_SCHEMA {
            return Err(schema(Error::InvalidInput));
        }
        // Bound each collection BEFORE allocating its typed copy.
        let node_values = bounded(required(root, "nodes")?, MAX_NODES)?;
        let edge_values = bounded(required(root, "edges")?, MAX_EDGES)?;
        let enforcer_values = bounded(required(root, "enforcers")?, MAX_CUT_NODES)?;
        let gate_values = bounded(required(root, "gates")?, MAX_CUT_NODES)?;
        let check_edge_budget = usize::try_from(unsigned(required(root, "check_edge_budget")?)?)
            .map_err(|_| schema(Error::Limit))?;
        if check_edge_budget == 0 || check_edge_budget > MAX_CHECK_EDGE_VISITS {
            return Err(schema(Error::Limit));
        }
        let source = object(required(root, "source_limits")?)?;
        exact_fields(source, &["checks", "read_bytes"])?;
        let source_limits = TopologySourceLimits {
            checks: nonzero(required(source, "checks")?)?,
            read_bytes: nonzero(required(source, "read_bytes")?)?,
        };
        if source_limits.checks > MAX_TOPOLOGY_SOURCE_CHECKS
            || source_limits.read_bytes > MAX_TOPOLOGY_SOURCE_READ_BYTES
        { return Err(schema(Error::Limit)); }
        let inventory = LoadedPerimeterInventory::from_json_bytes(inventory)?;
        let selected_scope = parse_scope(required(root, "inventory_scope")?)?;
        let selected_name = text(required(root, "family")?)?;
        let family = inventory.inventory.records().iter()
            .find(|record| record.scope == selected_scope && record.family == selected_name)
            .ok_or_else(|| schema(Error::Missing))?.clone();
        // Preserve every route of the selected family. No manifest-supplied
        // subset can remove a route before the original completeness check.
        let graph = AuthorityGraph::new(GraphSpec {
            generation: nonzero(required(root, "generation")?)?,
            inventory_generation: nonzero(required(root, "inventory_generation")?)?,
            completeness: Completeness::DeclaredComplete {
                inventory_generation: nonzero(required(root, "complete_inventory_generation")?)?,
            },
            scope: action_scope(required(root, "scope")?)?,
            target: target(required(root, "target")?)?,
            family,
            nodes: node_values.iter().map(node).collect::<Result<_, _>>()?,
            edges: edge_values.iter().map(edge).collect::<Result<_, _>>()?,
            enforcers: enforcer_values.iter().map(enforcer).collect::<Result<_, _>>()?,
        }).map_err(schema)?;
        let gates = gate_values.iter().map(nonzero).collect::<Result<Vec<_>, _>>()?;
        let proposal = graph.propose_cut(&gates).map_err(schema)?;
        Ok(Self { inventory, proposal, check_edge_budget, source_limits })
    }

    pub fn inventory(&self) -> &LoadedPerimeterInventory { &self.inventory }
    pub fn graph(&self) -> &AuthorityGraph { &self.proposal.graph }
    pub fn proposal(&self) -> &CutProposal { &self.proposal }
    pub fn check_edge_budget(&self) -> usize { self.check_edge_budget }
    pub fn source_limits(&self) -> TopologySourceLimits { self.source_limits }

    /// Optional offline preview using the original checker. An effect consumer
    /// must still certify through its OWN current mediation observer; this
    /// preview neither installs a graph nor grants a live cut or permit.
    pub fn check(&self) -> Result<CutCheck, Error> {
        self.graph().verify_cut(&self.proposal, self.check_edge_budget)
    }
}

fn bounded(value: &Json, maximum: usize) -> Result<&[Json], InventoryLoadError> {
    let values = array(value)?;
    if values.len() > maximum { return Err(schema(Error::Limit)); }
    Ok(values)
}
fn nonzero(value: &Json) -> Result<u64, InventoryLoadError> {
    let value = unsigned(value)?;
    if value == 0 { return Err(schema(Error::InvalidInput)); }
    Ok(value)
}
fn action_scope(value: &Json) -> Result<Scope, InventoryLoadError> {
    let fields = object(value)?;
    exact_fields(fields, &["tenant", "principal", "run", "branch", "authority", "purpose"])?;
    if text(required(fields, "purpose")?)? != "effect" { return Err(schema(Error::Binding)); }
    Ok(Scope {
        tenant: nonzero(required(fields, "tenant")?)?,
        principal: nonzero(required(fields, "principal")?)?,
        run: nonzero(required(fields, "run")?)?,
        branch: nonzero(required(fields, "branch")?)?,
        authority: nonzero(required(fields, "authority")?)?,
        purpose: Purpose::Effect,
    })
}
fn target(value: &Json) -> Result<ResolvedTarget, InventoryLoadError> {
    let fields = object(value)?;
    exact_fields(fields, &["adapter", "object", "contract_version", "expected_version", "generation"])?;
    Ok(ResolvedTarget {
        adapter: nonzero(required(fields, "adapter")?)?,
        object: nonzero(required(fields, "object")?)?,
        contract_version: nonzero(required(fields, "contract_version")?)?,
        expected_version: nonzero(required(fields, "expected_version")?)?,
        generation: nonzero(required(fields, "generation")?)?,
    })
}
fn node(value: &Json) -> Result<Node, InventoryLoadError> {
    let fields = object(value)?;
    exact_fields(fields, &["id", "kind"])?;
    let kind = match text(required(fields, "kind")?)? {
        "actor" => NodeKind::Actor,
        "process" => NodeKind::Process,
        "credential" => NodeKind::Credential,
        "enforcer" => NodeKind::Enforcer,
        "sink" => NodeKind::Sink,
        _ => return Err(schema(Error::InvalidInput)),
    };
    Ok(Node { id: nonzero(required(fields, "id")?)?, kind })
}
fn edge(value: &Json) -> Result<Edge, InventoryLoadError> {
    let fields = object(value)?;
    exact_fields(fields, &["id", "from", "to", "channel", "route", "provenance"])?;
    let channel = match text(required(fields, "channel")?)? {
        "delegation" => Channel::Delegation,
        "execution" => Channel::Execution,
        "ipc" => Channel::Ipc,
        "credential_access" => Channel::CredentialAccess,
        "dispatch" => Channel::Dispatch,
        _ => return Err(schema(Error::InvalidInput)),
    };
    Ok(Edge {
        id: nonzero(required(fields, "id")?)?,
        from: nonzero(required(fields, "from")?)?,
        to: nonzero(required(fields, "to")?)?,
        channel,
        route: text(required(fields, "route")?)?.to_owned(),
        provenance: nonzero(required(fields, "provenance")?)?,
    })
}
fn enforcer(value: &Json) -> Result<Enforcer, InventoryLoadError> {
    let fields = object(value)?;
    exact_fields(fields, &["node", "adapter", "contract_version", "generation", "provenance"])?;
    Ok(Enforcer {
        node: nonzero(required(fields, "node")?)?,
        adapter: nonzero(required(fields, "adapter")?)?,
        contract_version: nonzero(required(fields, "contract_version")?)?,
        generation: nonzero(required(fields, "generation")?)?,
        provenance: nonzero(required(fields, "provenance")?)?,
    })
}
