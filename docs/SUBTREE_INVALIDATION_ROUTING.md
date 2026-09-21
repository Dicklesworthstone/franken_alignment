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
The original publication owner now records that selection as described below.

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

## Original publication consumer and durable selection

`DeliveryBroker` and `OversightBroker` expose
`enable_publication_subtree_routing()`. The choice requires an already configured
change feed and must precede proposals or observed changes. There is no live
switch, registration reset, increased dependency capacity, or lookup-budget
increase. The native gate continues deriving postings from immutable original
judgments. It still withdraws every observation on budget exhaustion, malformed
ranges, a missing feed tail, or tail repair. Empty and opaque dependencies retain
their original conservative behavior. Final validation still runs from scratch.

`FileOversight::create_with_publication_subtree_routing` records validation,
change-feed policy and strategy together in the FIRST canonical image. Existing
owners can explicitly select it during bootstrap with the revision-checked
`enable_publication_subtree_routing`. The selection is subtag 10 in the existing
publication-witness journal event. All older bytes/tags are unchanged. Old
readers refuse the new subtag; journals without it reconstruct PrefixV1, including
its original budget-exhaustion withdrawals and subsequent expected revisions.
The event remains bootstrap-class work, preserving reserved recovery capacity.

`open_with_publication_subtree_routing` pins validation limits, the complete
change policy and exactly one strategy selection before cleanup or recovery
writes. It refuses a legacy journal or mismatched cost policy. Other existing
openers replay the actual stored strategy, never silently migrate it. They are
not extra strategy pins. A recovered owner's original fence still cancels old
undispatched work and withdraws old keys; the index is not a permit. The query
`publication_routing_strategy` refuses on an unavailable durable owner.

The same notification consumer is used by file-feed ingestion, coherent producer
acquisition and the existing supervised/atomic publication paths. There is no
second invalidation reducer or alternate effect route. This increment selects
routing through host APIs, not a silent change to existing CLI profile schemas.
Source authentication, freshness and capture coverage remain separate contracts.

Eight real-journal tests exercise 512 ranges across eight attempts: selective
reuse and successful original two-key publication, a relevant post-dispatch
change versus an unrelated control, mandatory exact validation despite an index
nonhit, both historical strategy replays, incomplete-feed/zero-budget behavior,
configuration pins before cleanup, immutable bootstrap and failed-write
quarantine. One additional wire test fixes the new tag and unchanged big-endian
legacy fields. Together with eight index tests, 17 Rust tests are authored but
UNEXECUTED; neither a passing build nor a runtime/production qualification is
claimed. Existing tests and assertions are unchanged.

An independent Python interval model matched 9450 generated cases against an
unsorted exhaustive scan and the legacy traversal. For the modeled 1024-range
broad-plus-disjoint case, logical cost was 1068 steps / 68816 bytes for PrefixV1
versus 65 / 3824 for SubtreeV2. For 8192 overlapping intervals across 128 owners,
it was 8590 / 546696 versus 770 / 35936. These are modeled logical counters,
NOT executed Rust results, throughput measurements, or end-to-end speedups.
The new Rust tests separately assert finite-budget boundaries and eager-validator
coverage; their exact-revision RCH verification remains outstanding.
