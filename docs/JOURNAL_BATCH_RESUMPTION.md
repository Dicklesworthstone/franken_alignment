# Completing an interrupted original rights-journal group

`FileJournal::resume_batch(anchor, events, limits)` completes the missing records
of a caller's independently retained original accounting group after strict
recovery. Its consumer is the existing `FileJournal::commit_batch` caller (or a
caller of the cooperative new-group writer). It closes the gap between
[exact prefix inspection](JOURNAL_BATCH_RECOVERY.md) and completing the rest of
that same group. It is not an external-effect retry protocol or a second ledger.

## One original history, one final acknowledgment

Capture `batch_anchor()` before the first group attempt and retain that anchor
and the complete ordered events independently. Following an uncertain write,
drop the poisoned owner and reopen under the original identity, limits, reserve
profile and independently retained recovery head. Strict reopening applies and
synchronizes every complete record it finds, including any unacknowledged
prefix. It refuses torn records; resumption does not bypass that requirement.

The new method uses the existing exact inspector, unchanged. Before writing, it
reconstructs the anchored prefix, checks its actual head and extent, validates
ALL original group events and their whole-group limits, compares every stored
group byte, and matches the current owner's acknowledged rights, head and length.
An offset within a record, changed stored order or effect, incompatible profile,
corruption or an unrelated suffix cannot be treated as missing work.

Only the missing records are then prepared from that verified acknowledged
state, using the original batch encoder and reducer. The original writer lock
remains held through verification, end-of-file positioning, writing and one
final `sync_all`. The final length must match the prepared candidate before its
head and accounting replace the acknowledged state. No event format, new batch
marker, stored record, ordinary limit or reserve rule changes.

If the whole group is already acknowledged, exact verification returns its
existing head without writing a byte or invoking synchronization again. This
also works at full capacity. It is an exact retry at the final group head, not
an authorization to ignore unrelated subsequent work.

## Failed continuation is still uncertain

The original inspector latches quarantine before reading; resumption retains
that latch until successful completion. Read/verification failures, preparation
failures after reading, write/sync errors and caught unwinds cannot expose stale
accounting. The former acknowledged head remains usable only as an independent
minimum for a fresh strict recovery. A second uncertain write may leave another
complete prefix, the entire group, or a torn record. It receives the same strict
recovery rules as the first attempt; there is no truncation, rollback or repair.

Reserved recovery records can use only their previously configured capacity.
Whole-group ceilings still include records already stored, and every prior
record still consumes lifetime capacity. Settlement can refund effect units
only through the original reducer's explicitly supplied reconciliation event;
it never refills the journal's ordinary or reserved capacity. An unknown
external outcome cannot be cancelled into nonexecution by this API.

## Trust, scope and work

The caller must retain the original events. Stored bytes cannot authenticate a
previously unwritten intention. Neither the public anchor fields nor the CRC
chain prove authenticity; the existing trusted-directory and cooperating-writer
assumptions remain necessary. This code appends accounting records only. It does
not send requests, run an effect, discover a remote outcome, authenticate a
reconciliation input or grant permission to resend anything externally.

Work remains synchronous: one existing anchored inspection, its bounded group
preparation, preparation of the missing suffix, and file I/O. Inspection does
not donate a reusable unchecked success flag; each retry verifies again.
Original frame, group and lifetime bounds apply. No reduced replay cost,
wall-clock bound, throughput, memory improvement, physical disk reservation or
multi-record disk atomicity is claimed.

## Source and validation status

The implementation is isolated in the existing batch-recovery module. Existing
inspection, single-record, grouped commit, replay and reserve bodies are unchanged.
Eight authored original-file regression functions cover every complete prefix,
a genesis batch, whole-group (not just remaining-group) limits, late invalid
events, changed anchors/order/effects, uncertain write/sync and unwinding,
every byte cut of an interrupted continuation, exact reserved capacity, and
same-length corruption. Successful continuations are compared to the original
single-record encoder and reducer; injected I/O failures are not power-cut tests.

The new tests have NOT executed. The preparation environment lacks the required
remote-only `rch` launcher; compilation, Rust tests, rustfmt and Clippy remain
unverified. Source hash and whitespace checks do not qualify Rust behavior.
No packet or bead is closed, and no old execution receipt qualifies this source.
The independent integrated verification remains:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal batch_recovery
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

## Original cooperative-writer integration

Four additional public-API regression functions compose the already published
`begin_commit_batch`, cancellation, strict reopen, `begin_inspect_batch`, and
`resume_batch`. They compare all six boundaries of a five-event group under
three write quanta against the original single-record encoding and rights.
No private persistence hook supplies these integration results. Other controls
cancel inside each record, add a legal but conflicting record AFTER successful
inspection, and finish a cancelled reserved tail at both exact lifetime limits.
A former inspection result is not a reusable authorization to skip verification.

These four functions are also authored but unexecuted under the same RCH
limitation. The full set comprises twelve new regression functions, with no
changes to existing test assertions or wire-format expectations.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal --test file_batch_resume
```

## Cooperative resumption without a second verification pass

`FileJournal::begin_resume_batch` freezes the same independently retained anchor
and entire original batch in the existing `FileBatchInspection`. Advance with
`BatchInspectionBudget` until `Ready`, retain its work report, then consume it
through `FileBatchInspection::resume`. That handoff returns the existing
`FileBatchCommit`; advance it with `BatchCommitBudget` and consume `finish` only
after all missing bytes are written. A nonempty suffix requires the original
file synchronization before a usable final head and accounting are returned.

This is an explicit choice BEFORE inspection. Ordinary `begin_inspect_batch`
discards its private final candidate as before and cannot later upgrade to a
writer. The resumption path retains that candidate privately through stored-byte
comparison and clean EOF. No partial candidate, writable owner, raw descriptor,
import callback or asserted progress token is exposed. The same advisory lock
survives both stages, including the handoff and final synchronization.

The original replay, per-record preparation and byte comparison are unchanged.
Resumption transfers their actual final candidate and already encoded missing
suffix rather than replaying the prefix or re-encoding that suffix. The handoff
checks the actual file extent and candidate identity, profile, head and byte
length. Removing the stored prefix can compact the bounded 8 MiB group buffer;
it is not a zero-copy or memory-reduction claim. The original inspection still
clones its private ledger for each prepared record. Only explicit resumption
retains one final candidate longer; ordinary inspection retention is unchanged.

The inspection report accounts for all original group preparation, including
already stored records. The writer then reports only missing records and its
own writes; its new preparation-attempt count is zero. Keep both reports rather
than treating one as the total cost. Initial setup, metadata, seeking, buffer
compaction, a reducer step and final synchronization remain synchronous.
Budgets bound logical operations, not syscall latency or physical-media traffic.

If the entire matching group was already acknowledged, the returned writer is
immediately `Ready`, with zero missing records and no new preparation or write
work. Its `finish` checks extent and returns the original acknowledged owner
without another synchronization. It does not replace the original live ledger
with the replayed copy. New-group commits and nonempty suffixes still synchronize.

Cancellation before handoff leaves bytes unchanged; after writing starts it may
leave complete records or a torn record. Neither stage truncates, repairs,
refunds or invents an external outcome. Failed reads, semantic checks, writes,
synchronization and caught unwinds return no new usable accounting. Extent is
checked at handoff, every write and final completion; malicious same-size edits
remain outside the cooperating-writer contract. `finish` on the inspector still
supports explicitly stopping after verification with only the previously
acknowledged owner and exact status, discarding unused private preparation.

Six additional public regression functions cover every surviving record boundary
under three quanta, zero allowances, explicit selection, incomplete/failed
handoff, cancellation before/after handoff, complete and torn suffix boundaries,
whole-group limits, conflicting records, exact reserved capacity and changes in
extent at each ownership boundary. Four new native fault regressions exercise
the SAME original writer: EINTR and short writes, errors after actual writes,
synchronization failures and unwinds, one sync for a new suffix, and no sync for
an already acknowledged group. One compile-fail example checks one-use handoff.
Together with the eight synchronous and four previously prepared public tests,
22 new regression functions are authored across these two source increments.

These tests remain UNEXECUTED. The targeted public suite, private resumption
suite and full gate again stopped before compilation because `rch` is absent
(exit 127). No compiler, formatter, Clippy, performance or deployment pass is
claimed. Existing tests and the independent single-record encoder are unchanged.

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal --test file_batch_resume
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal resumption
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```
