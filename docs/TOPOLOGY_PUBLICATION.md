# Operator topology inputs for publication

## October 12, 2026: original graph input bridge (unqualified source)

`perimeter_inventory::topology::LoadedAuthorityTopology` reads the explicit
`fa.authority-topology/1` document and a separate original version-one perimeter
inventory. It selects the entire exact scope/family, preserves all loaded route
metadata and unselected families, and constructs the original
`action::consequence::mediation::AuthorityGraph`. No second graph algorithm,
perimeter parser, executor, authority ledger or dependency is introduced.

The topology names its full action scope, resolved target, graph and inventory
generations, separately declared complete-inventory generation, nodes, labeled
parallel edges with provenance, registered enforcers and proposed gate IDs.
Reachability is computed by the original proposer; a caller cannot supply a
certificate or a supposedly complete reachable partition in this input format.
Missing, duplicate, unknown and wrongly typed fields refuse. Original node,
edge, cut and checker bounds remain unchanged. Each input is at most 1 MiB.

`check()` is an optional offline invocation of the original independent cut
checker. It distinguishes verified cuts, actual bypass paths, unreachable sinks
and errors. Parsing alone does not certify coverage. Missing routes, unknown
coverage, stale completeness generations and insufficient checker work cannot
become permission. The live consumer must obtain its own cut through the
original durable mediation observer under the current authority epoch.

The source-limits object fixes lifetime check and conservatively reserved read
capacity allowances for the operator consumer. Checks are capped at 65,536 and
reserved read capacity at 1 GiB. These are read-admission units, not wall-clock,
filesystem traffic, process-memory or deployment-performance claims.

## Format

The complete minimal **test fixtures**, not deployable host attestations, are:

- `crates/fa-reference/tests/fixtures/authority_topology.json`
- `crates/fa-reference/tests/fixtures/authority_topology_inventory.json`

Every field shown is required. No path, program, credential secret, alternate
schema or certificate is inferred. The inventory's original route profiles,
trust paths, effect categories, credential dispositions and residual nonclaims
remain available on `inventory()`. A metadata-only substitution changes the
complete loaded object even when the selected logical graph is identical.
The inventory scope's numeric purpose is explicit and distinct from the action
scope's closed `effect` purpose.

## Verification boundary

Twelve public integration tests are authored for original-graph/result parity,
bypass versus disconnected sinks, missing family routes and a repaired positive,
metadata-only substitutions, strict malformed inputs, original binding and
provenance checks, parallel channels, exact/one-short checker work, full byte
ceilings, collection limits, lifetime source limits and nonbrokered declarations.
One compile-fail example forbids conversion of the loaded declaration to a permit.

The targeted test and unchanged complete project gate were attempted through
`RCH_REQUIRE_REMOTE=1 rch exec`. Both stop before compilation because `rch` is
absent (exit 127). Rust parsing, compilation, rustfmt, Clippy, tests and workspace
qualification remain unexecuted. Local checks confirmed only fixture JSON syntax,
selected-source whitespace and the exact unchanged parent-file preimage.
No bead is closed and no production or operating-system containment is claimed.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test authority_topology_input
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

This serves FA-072/FA-073 and plan sections 4.4, 7.1, 8.3, 15.6 and 19.4: the
external-control founding commitment FI-A02 and evidence discipline FI-A06.
Original production acceptance, perimeter completeness, enforcer functioning,
credential isolation and independent source authenticity remain separate gates.
