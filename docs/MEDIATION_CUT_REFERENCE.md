# Typed authority projection and independent mediation cuts

Reference implementation of plan sections 15.2 and 15.6, FA-072 and FA-073, serving FI-A02 and FA-INV-024. Dispatch integration also serves sections 8.8 and 21 and FA-INV-003, FA-INV-005 and FA-INV-006. This is conditional finite-graph evidence, not deployment containment or production qualification. Source is in `action::consequence::mediation` and `delivery::mediation_gate`.

## Executable contract

The immutable projection binds the complete action scope, adapter/resource identity, graph and inventory generations, the existing validated effect-family inventory, directed channel types, parallel edge IDs and boundary provenance. All declared Actor nodes are roots and all declared Sink nodes are protected exits to the one registered resource. The checker does not accept caller-selected subsets. Enforcer declarations must bind the resource's adapter contract and generation; an actor, credential, process or sink cannot be selected as a convenient cut.

Every family route must be declared brokered and blocked and represented in the projection. An observe-only or residual alternate route cannot be hidden by selecting only the brokered route. Unknown closure and a closing inventory generation mismatch cannot qualify. These are checks on supplied declarations: completeness, provenance authenticity and physical enforcement still require the real host inventory/bypass campaign.

Sparse BFS proposes a removed-gate partition. A separate bounded edge-scan fixed point verifies it without calling that BFS. It first checks original connectivity, then removes the proposed enforcers and checks all roots and sinks. Results distinguish VerifiedCut, Bypass with original node/edge identities, and Unreachable. The latter is not credited as a functioning gate. The candidate partition is untrusted and must match the independently computed partition before a VerifiedCut is issued.

The scope is bounded to 256 nodes, 4,096 edges and 64 enforcers. Both verification traversals share an explicit edge-examination budget. Exhaustion returns Limit, never a certificate. The retained counter counts actual edge examinations, not elapsed time, allocations or a production performance claim. There is no dense residual matrix, max-flow solver, outside dependency, runtime or cryptographic primitive.

Resource content versions may advance without changing the topology. Adapter, resource object, contract and resource generation remain binding; the existing endpoint independently enforces the frozen action's expected content version. Equal graph generation numbers do not substitute different graph bytes, scope or provenance.

## Topology is a dispatch prerequisite

`DeliveryBroker::enable_mediation` selects the profile before any attempt. Once selected, there is no disable operation or unverified fallback. `certify_mediation` independently verifies an untrusted candidate against the currently owned graph under an expected graph generation and authority epoch. Merely installing a graph, supplying an old certificate, or constructing a candidate never opens admission.

The same lower delivery owner checks the accepted graph at proposal, permissive review application, authorization and dispatch. Both ordinary and human-co-signed dispatch use that owner. The hot check compares the privately retained exact graph allocation and the resource identity; it does not rerun graph traversal on every effect. Each delivery record retains its consumed VerifiedCut with the original graph, while the endpoint projection still contains no helper inputs or graph bytes. The records are bounded by MAX_DELIVERIES; retained graph/cut memory is additional to the payload/witness-only MAX_DELIVERY_ACTION_BYTES counter and has not been heap-profiled.

`replace_mediation` is an explicit trusted governance operation, not an actor proposal. It requires the exact predecessor epoch/generation, a newer graph and inventory generation, and the same scoped effect-family/resource mapping. It accepts a structurally valid new inventory even when its graph has a bypass or unknown completeness: keeping the old permissive graph after such an update would be incorrect. It advances the original control ledger's revocation floor, cancels all undispatched attempts and refunds only their actual reservations, then requires fresh certification and new proposals. No copied ledger or alternative permit pool is introduced.

`withdraw_mediation` handles observation loss without inventing an empty graph. The old topology is historical and cannot be recertified in place; a new inventory must be explicitly installed. Repeated withdrawal at the current predecessor is a no-op. Actor reset, policy changes and dispatcher restart cannot restore a withdrawn graph or disable the requirement. TopologyChange returns exact predecessor/current graphs, the new floor, cancelled IDs and refunded units; retaining a durable control journal remains outside this in-memory profile.

Already admitted envelopes and unknown effects keep their real liabilities. These topology operations are not endpoint cancellation and do not claim to undo effects already admitted for execution. Reconciliation and receipt acceptance remain available without current topology, helper inputs or the reviewer role. The cut retained for an earlier dispatch is never replaced by a later graph.

## Verification status and change history

September 11, 2026: added the typed projection, independent checker, then bootstrap/activation/replacement/withdrawal integrated into both delivery and oversight APIs, with per-dispatch cut retention. These are source additions, not qualified production milestones.

Public checker tests cover a mediated positive case, parallel edges, concrete bypasses, disconnected sinks, all-root/all-sink coverage, missing/stale closure, residual/unprojected routes, forged partitions, trivial cuts, foreign scope and provenance, exact/one-over graph sizes, and exact/one-short verification work budgets. A 256-case directed-graph corpus uses an independent public DFS oracle and checks each returned bypass edge. A compile-fail example forbids conversion from a cut into an effect permit.

Dispatch tests compose actual reference congress/input/key processing with endpoint state. They cover successful one-key and two-key publication; a bypass introduced between authorization and dispatch; cancellation/refund and fresh-proposal recovery after repair; topology loss while another effect is unknown; reconciliation after helper-input loss and reviewer-role drop; stale/foreign update atomicity; reset/restart nonresurrection; and the unchanged explicitly unconfigured profile.

These additions have not been compiled, run, formatted by rustfmt or qualified through RCH in this environment. No Rust or RCH toolchain is available here. Historical receipts do not validate them. FA-072, FA-073 and production gates remain open; the br-managed tracker is not edited by hand. The graph's completeness and enforcer declarations still require independent host evidence before any real containment claim.
