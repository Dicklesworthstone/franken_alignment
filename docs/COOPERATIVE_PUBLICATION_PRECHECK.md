# Cooperative complete-view publication comparison

## Capability and status

This source addition advances the FA-062 publication conjunction and the bounded
work contract of plan 7.6-7.7. It serves the existing epistemic-MVCC engineering
addition (FI-A13/FI-A17). It does not change the plan's authority semantics or add
a dependency, runtime, journal format, CLI verb or deployment profile.

The original PublicationJudgment now compares BOTH structured witnesses and the
opaque helper's entire actual input across bounded polls. Its existing live
validate method consumes the same opaque comparator. The independent eager
OpaqueJudgment::valid_at remains a differential oracle, not a replacement input.

Source and tests are authored, not qualified. The required remote RCH launcher
and Rust toolchain are absent in this editing environment. Rust compilation,
tests, rustfmt and Clippy remain UNEXECUTED. No Bead is closed and historical
receipts do not qualify this change.

## Frozen evidence, not permission

PublicationJudgment::begin_refinement borrows its own immutable reviewed action
and the exact structured snapshot/frontiers and opaque view supplied for its
retained lanes. It accepts no substitute action, reduced dependency list, success
bit or explanation. Missing and extra lanes retain their distinct original
refusal meanings. Construction does not compare or clone payloads.

The structured lane uses the original WitnessRefinement cursor. The opaque lane
compares profile identity and model/policy/tokenizer epochs, every submitted and
profile byte, all ordered parts including spans/provenance, and every ordered
omission record. Matching one lane is insufficient. A mismatch/refusal is sticky;
there is no partial valid result or cross-cut resume token. A borrow prevents
input mutation during a run, not changes to the external world.

A terminal report describes this evidence comparison only. No dispatch or
publication operation accepts it as a key. Normal authority, human approval,
source capture, feed freshness, policy, deadline and endpoint checks remain
independent and mandatory. This is not asynchronous authorization or a cached
permission fast path.

## Work and compatibility

Each cooperative poll has a step and compared-byte allowance. Both lanes share
that allowance and one fixed lifetime ceiling. A new poll cannot refill spent
lifetime work. Insufficient per-poll work yields NeedsRefinement with the minimum
for the NEXT step; insufficient remaining lifetime work latches Refused(Limit).
Cancellation consumes the cursor and returns completed work, not a refund, a
source refresh or a successful validation. Terminal polling does no new work.

A cooperative opaque step is one bounded header comparison, one byte chunk, or
one fixed-size part/omission comparison. Compared chunks are charged before
inspection, even when a mismatch short-circuits. Empty phases do not scan data.
Matching runs inspect each payload byte once regardless of polling boundaries.
Small chunks can spend more steps; short-circuiting mismatches can spend fewer
bytes. These are logical costs, not elapsed time or allocator/CPU counts.

The synchronous live gate RETAINS its legacy cost profile: it admits and charges
the whole opaque comparison before calling the shared comparator. It does not
spend a partial opaque allowance or substitute cooperative chunk counts into
existing reports/journal comparisons. Action binding, lane-refusal precedence,
structured-first ordering and exhaustion behavior remain unchanged. This avoids
reinterpreting retained durable histories while exposing an explicit cooperative
profile to callers that need bounded comparison polls.

## Regression contract

Eight new unit regression functions cover 30 independent opaque mutations plus
the unchanged control under three poll schedules, both-lane progression, stopped
byte offsets, cancellation, exact/one-under lifetime ceilings, every structured
invalidation kind, incomplete closure, missing/extra lanes and maximum-size
opaque input. The legacy-cost matrix retains full charging for early opaque
mismatches and refuses one-under allowances before any opaque comparison. Two
independent oracles remain: original structured refinement and full derived
opaque equality. One compile-fail example protects the immutable opaque view.

The existing publication tests and journal encodings are not changed to obtain
passing results. All Rust tests and compile-fail examples still require execution
against the integrated revision, including the full original gate:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Change record

Added resumable composite validation and shared complete-view opaque comparison.
Preserved the original synchronous public report and final-effect behavior.
