# Reference address resolution and graph traversal

The `address_resolver` module implements the reference `get` and `related`
operations for FA-134 and plan §§17.2 and 17.6. Its caller supplies a
`CallerScope`; this reference API does not authenticate that declaration.

## Source admission

Both operations check tenant, backing plane and required authority before
looking up a source. Unauthorized existing and nonexistent sources return the
same `Knowledge::Withheld` value. Public address fields are revalidated against
the canonical tenant/id grammar, so direct struct construction cannot bypass
the parser's component checks.

For an authorized source, the requested generation is compared with the stored
object generation:

| Request | Result |
|---|---|
| Current generation or no generation | Known, carrying the stored basis |
| Older generation | Stale, naming the current generation |
| Future generation | Unknown(IncompatibleGeneration) |
| Missing or malformed source | Unknown(MissingExactDomain) |

A non-Known source does not expand adjacency or resolve targets. Related preserves
the same source outcome and basis as get.

## Relationship generations

An edge retains the source generation established at registration:

| Source supplied to insert_edge | Association |
|---|---|
| Explicit generation | Exactly that generation, including zero |
| Unqualified, with a stored source | The source's current generation at insertion |
| Unqualified, without a stored source | Unbound; later insertion does not attach it |

Traversal expands only edges bound to the admitted source's current generation.
An explicit future edge can become visible when that exact source generation is
registered. An old edge cannot be relabeled as a newer source's relationship.
Every returned RelatedEdge.from carries the exact generation traversed, including
when the request used an unqualified address.

The handoff/latest resolver therefore discovers the current generation's
provenance while older concrete handoff objects remain separately addressable.
This does not make object insertion immutable or prevent version rollback.

## Visible graph projection

Related applies the optional relation filter and target authorization before
resolving or disclosing each target. A foreign tenant, denied plane, denied
authority, or malformed target contributes no address, relation, result entry or
target-resolution audit entry. Adding hidden edges does not change the visible
edge vector or its audit output. Authorized missing, stale and future targets
retain their distinct Knowledge states.

This is an authorization-filtered reference graph. It is not proof that no
hidden or unobserved edge exists. The backing adjacency remains indexed by
tenant/kind/id, with a source-generation binding on each edge. It does not supply
an immutable object history, timing noninterference or a production datastore
authentication boundary.

## Regression scope

`crates/fa-reference/tests/address_resolver_boundaries.rs` contains ten public
regressions covering useful traversal and its causal negatives. The in-module
related test also rejects disclosure of a secret target address. The fifteen
additional cases in `crates/fa-reference/tests/agent_projection_boundaries.rs`
cover relationship generations, real handoff discovery and scoped journal
projections, including permitted updates and identity-bound hold retractions.

The 25 resolver/projection regressions passed with the qualified
nightly-2026-09-08 compiler as part of a combined 74-test run. The eight changed
agent-interface Rust files also passed their targeted formatting check. See
[the scoped execution receipt](../artifacts/execution/2026-11-05-unknown-abandonment-receipt.json)
for exact source hashes, commands and retained logs.

The canonical repository gate still stops at the pre-existing snapshot-policy
binding mismatch before the later workspace gates. Full workspace formatting,
Clippy, tests, complete dispatcher and production qualification remain open.
