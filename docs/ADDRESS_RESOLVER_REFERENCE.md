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

## Visible graph projection

Related applies the optional relation filter and target authorization before
resolving or disclosing each target. A foreign tenant, denied plane, denied
authority, or malformed target contributes no address, relation, result entry or
target-resolution audit entry. Adding hidden edges does not change the visible
edge vector or its audit output. Authorized missing, stale and future targets
retain their distinct Knowledge states.

This is an authorization-filtered reference graph. It is not proof that no
hidden or unobserved edge exists. The backing adjacency remains indexed by
tenant/kind/id, without an immutable history for each source generation. The
implementation does not claim timing noninterference or a production datastore
authentication boundary.

## Regression scope

`crates/fa-reference/tests/address_resolver_boundaries.rs` contains ten public
regressions covering useful traversal and its causal negatives. The in-module
related test also rejects disclosure of a secret target address.

Compilation, test execution, formatting and Clippy for this change remain
pending: the execution environment became unavailable. These source tests are
not execution evidence and do not close the unified dispatcher or production
qualification gates.
