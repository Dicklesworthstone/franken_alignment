# Bounded groups in the original rights journal

This implements a bounded reference part of plan 18.3 and 18.6: grouping
ordered accounting records without weakening their individual transitions.
It serves the existing durability/recovery contracts, not another authority
ledger, event format, runtime or external-effect executor.

## Canonical grouped preparation

`Journal::append_batch(events, BatchLimits)` validates a nonempty ordered group
against the original `Rights` reducer. A returned buffer is byte-for-byte the
concatenation of original single-record encodings, including each sequence,
predecessor checksum and checksum. Every record keeps its original ordering and
meaning. The existing `append` and synchronous replay remain independent framing
oracles; grouped encoding uses their original event encoder and checksum.

No head, encoded length or rights change is published until the whole group
succeeds. An invalid final event cannot spend a valid prefix. Preparation owns
one clone of retained accounting, not another clone for every record; an error
or unwind discards that candidate. No public prepared-state import is exposed.
The additional group ceilings are at most 1,024 records and 8 MiB of complete
encoded records. Callers can tighten them, not widen the original cumulative
journal/frame limits. Empty groups refuse rather than creating fake commits.

The original encoder is given the smaller remaining group, frame and lifetime
byte allowance before copying a payload. Retained memory includes the existing
journal, one candidate clone, the bounded output buffer, and at most one bounded
event encoding; caller input and allocator overhead are separate. Repeated
capacity growth/copies and the original reducer's retained-history scans are
not a constant-time or RSS claim. No latency or throughput result is claimed.

## Durability boundary

In-memory all-or-nothing admission is NOT an atomic multi-record disk write.
If storage fails, a group may leave no bytes, a complete prefix or a partial
record. Strict original recovery must inspect it under the independently retained
acknowledged head. A complete dispatch/unknown record remains a liability even
when its containing group was never acknowledged. Truncated tails refuse; there
is no implicit truncation, group rollback, inferred negative receipt or refund.

These are original reference accounting inputs. Batching cannot erase a causal
barrier: a real send must follow its acknowledged dispatch record, and an outcome
record must follow its independently observed result. No group API authenticates
a supplied outcome or permits an external effect. Callers must not manufacture
future observations to place both sides of an external operation in one group.

## Validation status

Eight authored integration regressions cover all 2,048 contiguous partitions of
one 12-transition history against single-record encoding, both replay paths,
failure at every group position, revoked/substituted dispatch, unknown outcomes,
exact/one-under byte and record limits, nonrefillable lifetime/frame limits,
empty and hostile groups, and oversized late payloads. Existing tests are intact.

RCH and the Rust toolchain are absent in the editing environment. The prescribed
remote-only test launch stops before compilation. Compilation, these Rust tests,
rustfmt and Clippy remain UNEXECUTED; source and hash checks are not qualification.
No bead is closed, no production claim is made and no prior execution receipt is
reused to qualify this increment. Run the original gate on the integrated source:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
