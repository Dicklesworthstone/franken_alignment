# Bounded interval invalidation without broad-range interference

FA-061 / plan 7.7: a broad range must not make every narrow, disjoint range
expensive to route. The original maximum-end prefix index is sound, but one
long early interval can keep that prefix maximum high for every later entry.
A change beyond all narrow ranges then scans the entire prefix. Under a frozen
lookup budget, this can force conservative withdrawal of unrelated attempts.

`InvalidationIndex::new_with_strategy(limits, RoutingStrategy::SubtreeV2)` adds
an explicit alternative. It uses the same sorted interval postings and an
implicit balanced tree. Each subtree retains its exact maximum endpoint and
its 128-bit set of owning registration slots. A subtree is skipped only when
all endpoints are disjoint or all its owners have already been selected. Domain
and half-open-start ordering prune the other branches. Owner bits represent
registration slots, not caller-chosen IDs; results keep registration order.
Opaque and changed-semantic requirements still select their owners first.

Both summaries and interval records are charged BEFORE inspection. Summaries
cost one logical step and 24 logical bytes; interval records retain their
64-byte charge. Point routing, registration passes and output charging are
unchanged. Exhaustion returns no partial candidate list. These counters do not
claim to measure physical cache traffic, allocations, CPU time, or subsequent
exact validation. Dense overlaps with many different owners can still require
substantial work; this is not a universal logarithmic-output guarantee.

The tree contains at most one summary per admitted interval. Construction uses
checked/fallible allocation and finishes before the current index is replaced.
Registration rebuilds summaries after sorting; duplicate, capacity or allocation
failure cannot leave old summaries attached to new postings. The dependency cap
bounds recursive depth (8192 entries: at most 14 nonempty frames). Lookups
allocate no traversal stack and never read witness values.

## Replay compatibility

`InvalidationIndex::new` still selects `PrefixV1`. Its traversal and logical
charges remain unchanged. Selecting a faster algorithm without recording the
choice could change a historical budget-exhaustion decision, its all-attempt
withdrawal, and later expected input revisions. Therefore durable consumers must
explicitly record selection before proposals; there is no automatic migration.
The first increment provides the versioned index. The next increment connects
that selection to the original publication owner and journal.

## Verification scope

Eight additional Rust tests cover independent full-scan equivalence across
keys/ranges/domains/epochs, a broad interval alongside 1023 narrow ranges, all
128 owners with 8192 overlapping dependencies, exact budget neighbors, atomic
registration, u64 boundary cases, eager-validator invalidations, and opaque
requirements. Existing tests and assertions are unchanged. The legacy index is
a differential control, not the only correctness oracle.

Rust compilation, formatting, Clippy and tests are unexecuted in this editing
environment; RCH and the Rust toolchain are unavailable. This adds a reference
routing strategy, not authenticated notifications, proof of capture completeness,
production activation or FA-061/062 closure. Full exact witness validation and
all original effect authority requirements remain independent.
