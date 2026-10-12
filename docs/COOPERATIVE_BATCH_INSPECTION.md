# Cooperative inspection of interrupted journal batches

## Consumer and boundary

The synchronous `FileJournal::inspect_batch` verifies an uncertain group by
replaying its complete anchored history in one call. An operator cannot yield
or cancel that read through this API while a large journal is being checked.
`FileJournal::begin_inspect_batch(anchor, events, limits)` consumes the same
usable locked owner and returns a `FileBatchInspection` instead. The caller must
still strictly reopen a poisoned writer using an independently retained minimum.

This is an evidence-only alternative for the original batch-recovery consumer.
It never appends a missing record, repeats an accounting transition on the live
ledger, repairs a torn tail, refunds an unknown effect or issues an external send.
The original batch and anchor must be independently retained. Stored-prefix
matching cannot authenticate intentions that were never written.

## Three bounded stages

`advance(BatchInspectionBudget)` runs only the current stage and yields before
the next one. The caller controls six logical dimensions: original replay read
calls, returned bytes and checked records; original batch preparation attempts;
and suffix-comparison read calls and returned bytes. Zero allowances perform no
reads or reducer attempts. Interrupted reads consume calls, so repeated EINTR
cannot turn a bounded poll into an infinite loop. EOF still requires read budget.

The first stage runs the original cooperative `Replay` over a privately bounded
view ending at the independently supplied anchor. The exact head and extent must
match there. The already configured owner's original v1 or v2 reserve profile is
used; there is no header autodetection or wider recovery allowance.

The second stage validates every event, including the unpersisted remainder,
through the original single-record `prepare_batch` primitive. The complete batch
count/byte ceilings and original lifetime reserve law remain binding. At the
surviving record count, its privately reconstructed rights, head and extent must
match the owner's already acknowledged state. No temporary ledger is exported.

The third stage compares every stored suffix byte in bounded reads and verifies
clean EOF and unchanged extent. A different ordering, corrupt record, conflicting
history or misaligned anchor refuses. Costs retain failed attempts. A read or
reducer error, or an unwind, permanently latches the session as Failed.

## Ownership, cancellation and costs

The session owns the original file descriptor and lock without a clone or
handoff gap. `progress()` contains only stage and non-authoritative work counts;
`acknowledged_head()` is the preexisting recovery minimum, not a new prefix claim.
`cancel()` returns those work counts and drops the locked session. Dropping or
consuming an incomplete/failed session cannot return a usable journal.

After Ready, `finish()` checks the same file extent once more and returns the
original `FileJournal` plus exact `BatchStatus`. It installs no reconstructed
ledger and performs no write or synchronization: that owner was synchronized
before inspection began. Repeated Ready polls do no work. Existing synchronous
inspection and all original commit/replay implementations remain unchanged.

Budgets count logical operations, not elapsed time or CPU instructions. A file
read can block; one native reducer can inspect retained rights. Per-record
preparation clones a private original ledger, so this yielding path does not
claim better throughput or a smaller process-memory footprint. It retains the
original frame, bounded batch encodings, private ledgers and an 8 KiB comparison
buffer, plus the caller-supplied events. Metadata, seeks and final extraction are
not part of the poll read counters. Trusted directories, advisory locks, exact
independent profiles and anchor trust remain required. Noncooperating edits,
malicious CRC recomputation, remote receipts and power-loss qualification remain
outside these claims.

## Source and verification status

2026-10-12: added the sealed session, staged budgets, retained work and eight
original-file regression functions plus two compile-fail capability examples.
Positive tests compare every surviving batch prefix with synchronous inspection
under three schedules, including byte-at-a-time replay/comparison. Controls cover
zero budgets, explicit EOF admission, cancellation and incomplete finish at stage
boundaries, original lock retention, invalid order and late events, whole-batch
limits and anchor substitution, corruption and extent changes, v2 reserved-tail
inspection followed by original settlement, EINTR, read failures and unwinds.
All existing assertions and original event/record formats remain unchanged.

These tests are authored, not executed. The required commands were attempted:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Both stopped before compilation with exit 127 because `rch` is absent. Cargo,
rustc and rustfmt are also absent. Source/patch/hash checks are not Rust execution.
No compilation, formatting, Clippy, test, runtime or performance qualification is
claimed, and no bead or production gate is closed.
