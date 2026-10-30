# Bounded witness-sensitive exact prechecks

This increment serves FA-142's cache requirement, plan §17.8 and the existing
exact-witness/one-directional-control contracts. It memoizes only the original
`Policy::evaluate` result. It is not a cached congress judgment, admission,
reservation, source observation, response or permit.

## Explicit native selection

The supervisor creates a `FilePrecheckCache` with `precheck_cache(limits)`, then
passes it to `disclose_exact_precheck_cached(revision, disclosure, queries, &cache)`.
The types and limits are exported through `precheck_wire::cache`. The original
`disclose_exact_precheck` remains uncached. Creating a cache does not disclose it
or enable it on any existing grant. An actor cannot obtain the owner or grant
itself this capability through an ordinary actor port.

A cache is bound by weak owner identity to one live authority incarnation.
Equal-looking owners, including reopened journals, cannot import it. The same
supervisor can retain it for a later explicitly issued grant, but the original
source/revision checks still run and every old port remains invalid. Cached
witnesses do not keep the authority lock alive. Dropping all selected grants and
the supervisor's cache handle drops its retained data; it has no disk format.

## Exact reuse boundary

Entries bind the complete frozen action, exact policy contents and generation,
policy epoch, scope, source policy/identity/generation and snapshot semantic
epoch. No hash, query label or approximate equality substitutes for those keys.
Every positive, absent-key and empty-range witness is revalidated, including
reads in unselected Boolean arms. Invalid entries are removed, not retained for
possible resurrection. A context/generation change removes the whole old cache.

A nonempty range also binds the exact first member selected by the original
evaluator. A newly earlier member can change its retained-byte preflight even
when an older nonemptiness witness remains true. Such a change forces original
evaluation, preserving its `Limit` refusal rather than hiding it behind a hit.
Incomplete and Unknown evaluations are never cached. Both known exact passes
and known policy denials can be reused.

Source protocol rules are unchanged. A producer update requiring a new semantic
epoch invalidates the cache even if the particular predicate values did not
change. The private evaluator tests exercise finer witness changes within a
fixed semantic context; they do not bypass the file producer's version rules.

The active grant's current target, ceiling, stop, units and deadline screens run
before lookup. Original owner/source eligibility and revocation run before that.
Every entered query, including a hit, spends its original shared allowance; wire
frames and connections remain separately charged. A result receives the current
grant's query/disclosure/epoch/observation basis, never an old cached envelope.
Normal Submit still acquires its own fresh evidence and ordinary review.

## Bounded retention and disclosure

Limits are 1–64 entries and 1–1,048,576 charged retained bytes; the optional native
default is 16 entries and 262,144 bytes. Accounting includes policy node/edge and
literal storage, full action payload/witness storage, cached witness storage,
range-head records and entry/context structures. These are structural retained
bytes, not an allocator-overhead or RSS guarantee. The entry/node limits bound
additional container metadata. The full source snapshot is never retained by
the cache. FIFO eviction is deterministic and does not reset query allowances.

A context or entry too large for the selected cache is evaluated normally and
not retained. Cache-borrow unavailability also runs the original evaluator, not
a cached pass or a permissive substitute. Supervisor-only aggregate counters
report cached-path evaluations, hits, invalidations, evictions and skipped
retention; they are not latency measurements or endpoint-wide work accounting.
Debug output and actor responses do not expose entries, policy/source values,
cache keys, counters or helper material. Timing remains an observable channel.

Capability discovery reports `witness_cache: "exact_witnesses"` only for a
selected cached grant; ordinary grants retain `"unavailable"` and their original
bytes. The strict decoder accepts only those two spellings, not full-policy or
congress-replay claims. Older decoders fail closed on the new spelling. Exact
precheck responses and their `review_required` semantics do not change.

## Verification status

Ten differential regression functions cover positive/negative/range witnesses,
unselected arms, all action fields, policy/source generations, incomplete data,
exact/one-short byte limits, FIFO eviction and the earlier-range-member limit
case. Eight real-owner/wire functions cover reuse across explicit new grants,
unchanged durable/source state, ordinary review, deadline screens, exhaustion,
revocation, source expiry/withdrawal, owner identity/lifetime, disabled defaults
and bounded strict capability responses. One compile-fail example denies a cache
handle the ability to issue a grant.

The eighteen functions are authored, not executed. Targeted tests and the
canonical check could not start because `rch` is absent (exit 127); formatting
could not start because `cargo` is absent (exit 127). Git whitespace checks and
original-blob preservation checks passed. A separately constructed maximum-width
cached-discovery JSON fixture is 714 bytes under the 1,024-byte ceiling; that is
not execution of the Rust codec. No performance, compilation, runtime pass,
production qualification, signed E27 profile or complete FA-142 acceptance is
claimed. Beads remain unchanged because `br` is unavailable.
