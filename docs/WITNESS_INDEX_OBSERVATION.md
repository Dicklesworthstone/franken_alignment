# Cooperative complete-tail index observation

This source increment advances the bounded FA-061 change-index path and the
work-budget contract of plan §7.7. The existing `WitnessChangeIndex::observe`
is the consumer: it now drives the same update exposed by `begin_observe`,
without changing its synchronous return type or its admission/error precedence.
This is an L2 reference capability, not a new authority or deployment profile.
It serves the existing epistemic-MVCC engineering addition (FI-A13/FI-A17),
without changing the founding semantics, registry or native dependency universe.

## Observable behavior

A caller can advance the actual comparison of two consecutive immutable
snapshots with independent step and compared-byte limits. An exhausted value
comparison resumes at its original byte offset. A returned minimum describes
only the next atomic step. Zero work never constitutes a complete observation.

`Ready` means that the complete delta has been built privately. Only consuming
`commit()` publishes it, advances the indexed revision and evicts the oldest
complete delta when capacity requires it. Dropping or explicitly cancelling at
any point, including after `Ready`, leaves all three unchanged. Cancellation
returns completed logical costs and does not mask a prior admission refusal.
Calling `commit()` on an unfinished update refuses without modifying the index.

The update exclusively borrows the index and immutably borrows its source,
target and frontier evidence. It cannot be moved to another target or reused
for another issuer. The public index cannot expose a partial tail while the
update lives. No resume token, caller-supplied changed-key list or new candidate
mask setter exists. Existing exact fallback, closing-frontier revalidation,
semantic checks and issuer/target identity checks are unchanged.

## Work and compatibility

A step is one bounded admission check, map lookup/iterator advance, exhausted
iterator check, or comparison chunk. Payload bytes are charged before each
chunk is compared. A chunk containing a mismatch is fully charged; later bytes
of that value need not be examined. Thus chunked mismatch costs may be lower
than the whole-value synchronous algorithm. With an unlimited call budget,
the original `ChangeCost` is preserved exactly. Candidate membership and exact
validation results must be independent of the polling schedule.

There are at most 512 retained changed keys and no copied payload values. The
256-entry and 8 KiB-per-value source bounds are unchanged. The report accounts
for logical work, not physical comparisons, CPU time, allocations, or peak RSS.
Final publication may evict one bounded key-only delta and perform queue
housekeeping; it does not rescan either snapshot. Frontiers remain asserted by
the caller's adapter contract, not authenticated by this reference module.

## Validation status

The patch contains unit regressions for budget exhaustion, exact offsets,
terminal idempotence, cancellation/drop at every small-fixture step, delayed
publication/eviction, refusal precedence, revision overflow, source bounds, and
differential comparison against the retained original eager delta algorithm.
Five public-API integration tests additionally cover mutation scenarios under
multiple ingestion schedules and planning budgets, cancelled phantom insertion,
committed ABA and history eviction, replacement/missing closure evidence, and
refused unknown targets. Near-identical permitted cases retain valid historical
and exact-value reuse. Two compile-fail examples cover exclusive history access
and double publication.

These Rust tests, examples, formatting and Clippy are **UNEXECUTED** in the
preparation environment: `rch`, `cargo`, `rustc` and `rustfmt` are unavailable.
No packet or bead is closed, no historical receipt qualifies this change, and
no production capability is claimed. The required independent gate remains:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
