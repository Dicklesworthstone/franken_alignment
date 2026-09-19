# Live publication invalidation (FA-061 / FA-062)

The original DeliveryBroker now consumes reverse dependency notifications before
its existing exact final-cut checks. OversightBroker forwards the same operations;
FileOversight journals the profile and notices in original event 30 (new subtags
5 and 6). No notification is a judgment, effect permission or refinement mask.
The existing complete-snapshot change index remains the only indexed skipping
oracle. This notification profile NEVER skips final exact validation.

## Original dependencies, bounded routing

Enable publication validation and then `enable_publication_changes` before any
proposal. Pin a nonzero source identity, independent initial `after` sequence and
a fixed lookup budget. There is no disable, source reset, late upgrade or budget
widening. Every subsequently bound original judgment registers automatically.
No API accepts a caller's allegedly complete footprint in place of the judgment.

Exact values and absent keys use sorted point postings. Empty ranges and complete
range membership use sorted interval starts with per-domain maximum-end prefixes.
Ranges are half-open, and a key at u64::MAX does not require computing key + 1.
Semantic or closing-frontier changes must use Domain or All. New source/domain
epochs reach old registrations under the same stable identity. Opaque judgments
are conservatively selected by EVERY notice, even when they also carry structured
requirements; structured precision cannot narrow a whole-helper dependency.

The standalone reverse index holds at most 128 judgments and 8,192 dependency
postings. The delivery consumer uses its existing lifetime maximum of 16 bindings
and at most 64 dependencies per binding. It retains no value payloads. Admission
can temporarily hold old and replacement footprint arrays; notification lookup
has a fixed-size candidate mask. Every inspected search/registration record and
output ID is charged before use. Limits cover logical record steps/bytes, not CPU,
wall time, total RSS or journal encoding/replay. A bounded 16-slot union/withdrawal
is additional consumer work. Worst-case overlapping ranges may visit all postings;
lookup exhaustion returns NO partial candidate set.

## Change sequence and live behavior

`record_publication_change` accepts one change at a source sequence. A multi-key
record can use a covering range, domain or All; never omit keys to fit a record.
The native API takes a notice; the durable API additionally takes the exact journal
predecessor. Wrong sources and already-covered sequence numbers refuse unchanged.

A contiguous notice uses the index to withdraw only affected observations. It
increments their original input revisions and clears stale diagnostics, but keeps
reviewed bindings, snapshot/producer high-water marks, automatic/human keys and
all rights accounting. A source-bound attempt loses its one-use capture freshness.
Existing final validation and capture rules still decide whether it may proceed.
Unbound slots are conservatively withdrawn, not assumed independent.

A future sequence beyond the next expected one commits a MissingTail report,
records the observed high-water mark and withdraws all slots. It does NOT advance
the complete prefix. Every authorization, dispatch and first-publication check
refuses while complete-through differs from observed-through. Repairs must arrive
from the next complete sequence. Repair events withdraw all slots again, including
on the final repair, so captures made during the gap cannot become usable merely
because coverage was restored. Repair is not reauthorization or a refund.

Lookup-budget exhaustion or a malformed decoded range withdraws all observations
and records Conservative(reason), rather than retaining a partial selective result.
Full exact validation of newly supplied/captured inputs is still required. Reports
record affected IDs, routing outcome, actual lookup work and both prefix frontiers.
A report is historical diagnostic data and no authority API accepts it as a key.

## Durability, recovery and nonclaims

A recognized current-source notice makes the durable owner unavailable BEFORE
encoding, allocation, replay or storage can fail. No acknowledged report escapes
a failed canonical replacement; the last acknowledged image remains historical,
not permission to publish. Recovery uses the original exclusive owner/fence and
withdraws old sendable keys. Missing-tail state, source, budget and dependency
registrations replay from the original journal. Generic reopen cannot disable it.

Notices may be recorded during an interrupted native policy source. They can only
withdraw inputs/change completeness, cannot clear the independent interruption
latch, and cannot resume or authorize an effect. Existing receipt reconciliation,
stop and cancellation remain separate. Previously resolved endpoint outcomes and
expiry precede new evidence checks; missing notifications cannot turn an executed
effect into a refund. After dispatch, failed witness checks use the existing seal
and receipt-based settlement path rather than inventing nonexecution evidence.

Notification contents, coverage and source identity remain trusted host assertions.
A contiguous counter is NOT cryptographic proof of a complete change feed. The
caller must report every relevant change and supply genuinely current inputs.
This feature adds no remote stream adapter, authentication, clock lease, global
producer ordering, anti-rollback storage guarantee or production qualification.

## Verification

Routing tests compare every bounded key/range case against an independent scan
and compare real eager-validator invalidations to routing coverage. Consumer tests
exercise the original file/committee/two-key/receipt path, selective versus opaque
withdrawal, gap repairs, restart, limits and failed durable notices. The new Rust
code and tests remain UNEXECUTED here: `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run
--locked -p xtask -- check` cannot start because rch is absent. Compiler, formatting,
Clippy and runtime verification must run on the exact integrated revision. FA-061
and FA-062 remain open; authored tests are not execution or alignment evidence.
