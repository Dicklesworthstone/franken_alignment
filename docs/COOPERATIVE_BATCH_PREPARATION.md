# Cooperative preparation of new rights-journal batches

`Journal::begin_append_batch(events, limits)` freezes an owned list and returns a
sealed `BatchPreparation`. `advance(max_records)` attempts at most that many
original encodings and rights-reducer transitions. Zero does no work; Ready polls
are idempotent. Only `finish()` after Ready returns the complete in-memory Journal
and its canonical group bytes. There is no partial ledger, mutable event list or
partial encoded-record accessor. No disk write or external-effect authority is
created by preparation.

The consumer is the existing journal append/persistence boundary: large admitted
groups can be validated in bounded event steps before a file writer begins I/O.
Synchronous `append_batch` and cooperative preparation share the same private
encoder. The original per-event framing, checksums, ordering, capacity admission
and rights reducer are retained. Existing single-record append remains an
independent parity oracle. Neither route copies the retained ledger per event;
the initial source clone occurs once, after the original cheap whole-group bounds.

Every event, including the last, must succeed. A returned error or unwind leaves
the cursor Failed; only work counters remain inspectable. `attempted_records`
includes the failed attempt, while `prepared_records` counts completed encodings
and reductions. Cancellation drops private work without changing the source
journal. The original source, caller-owned input allocation and initial clone are
outside per-poll counts. A single bounded event can inspect retained rights or
allocate its original bounded payload; these are logical operation allowances,
not latency, instruction or process-memory bounds.

Both v1 and independently pinned v2 reserve profiles retain their bytes and
meaning. Ordinary records cannot enter the recovery tail. Yielding, cancellation,
effect reconciliation and checkpoint reset cannot refill journal capacity. The
new cursor does not discover or retry an earlier interrupted batch; its events
are a new explicit in-memory preparation, not recovered external intentions.

## Source and qualification

2026-10-12: added eight public regression functions and two compile-fail examples.
Coverage includes all 64 contiguous polling partitions of a seven-event original
history, zero/terminal polls, cancellation, late reducer and encoder errors, exact
and one-less budgets, malformed limits, reserved-tail settlement, revoked epochs
and exact effect binding. Existing test bodies and wire fixtures are unchanged.

The original per-event batch body was checked for token-preserving extraction;
selected-source whitespace and preimage-hash checks passed. Required execution:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal --test batch_preparation
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Both commands stopped before compilation because `rch` is absent (exit 127).
Rust compilation, tests, rustfmt and Clippy remain UNEXECUTED. These are authored
regressions, not runtime qualification. No dependency, authority semantics, shared
registry, production gate or bead closure is changed.
