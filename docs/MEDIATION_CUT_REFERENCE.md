# Typed authority projection and independent mediation cuts

Reference implementation of plan sections 15.2 and 15.6, FA-072 and FA-073, serving FI-A02 and FA-INV-024. This is conditional finite-graph evidence, not deployment containment or production qualification. Source is in `action::consequence::mediation`.

## Executable contract

The immutable projection binds the complete action scope, adapter/resource identity, graph and inventory generations, the existing validated effect-family inventory, directed channel types, parallel edge IDs and boundary provenance. All declared Actor nodes are roots and all declared Sink nodes are protected exits to the one registered resource. The checker does not accept caller-selected subsets. Enforcer declarations must bind the resource's adapter contract and generation; an actor, credential, process or sink cannot be selected as a convenient cut.

Every family route must be declared brokered and blocked and represented in the projection. An observe-only or residual alternate route cannot be hidden by selecting only the brokered route. Unknown closure and a closing inventory generation mismatch cannot qualify. These are checks on supplied declarations: completeness, provenance authenticity and physical enforcement still require the real host inventory/bypass campaign.

Sparse BFS proposes a removed-gate partition. A separate bounded edge-scan fixed point verifies it without calling that BFS. It first checks original connectivity, then removes the proposed enforcers and checks all roots and sinks. Results distinguish VerifiedCut, Bypass with original node/edge identities, and Unreachable. The latter is not credited as a functioning gate. The candidate partition is untrusted and must match the independently computed partition before a VerifiedCut is issued.

The scope is bounded to 256 nodes, 4,096 edges and 64 enforcers. Both verification traversals share an explicit edge-examination budget. Exhaustion returns Limit, never a certificate. The retained counter counts actual edge examinations, not elapsed time, allocations or a production performance claim. There is no dense residual matrix, max-flow solver, outside dependency, runtime or cryptographic primitive.

Resource content versions may advance without changing the topology. Adapter, resource object, contract and resource generation remain binding; the existing endpoint must independently enforce the frozen action's expected content version. Equal graph generation numbers do not substitute different graph bytes, scope or provenance.

## Verification status

Public test source covers a mediated positive case, parallel edges, concrete bypasses, disconnected sinks, all-root/all-sink coverage, missing/stale closure, residual/unprojected routes, forged partitions, trivial cuts, foreign scope and provenance, exact/one-over graph sizes, and exact/one-short verification work budgets. A 256-case directed-graph corpus uses an independent public DFS oracle and checks each returned bypass edge. A compile-fail example forbids conversion from a cut into an effect permit.

This increment has not been compiled, run, formatted by rustfmt or qualified through RCH in this environment. No Rust or RCH toolchain is available here. Historical receipts do not validate these additions. FA-072, FA-073 and production gates remain open; the br-managed tracker is not edited by hand. Dispatch integration is separate from this pure checker.
