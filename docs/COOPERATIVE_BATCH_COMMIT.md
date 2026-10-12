# Cooperative new-batch persistence under the original writer lock

`FileJournal::begin_commit_batch(events, limits)` consumes the sole usable file
owner and freezes a NEW ordered group. It returns `FileBatchCommit`, which keeps
the same descriptor and exclusive advisory lock until completion, cancellation or
drop. There is no second writer, ledger importer, raw-file accessor or live rights
view. This extends the existing new-batch commit boundary; it does not inspect or
resume a previously interrupted batch or infer an external operation to retry.

## Preparation, writing, then acknowledgment

`advance(BatchCommitBudget)` runs only its current stage and yields before the
next. Preparation uses the shared original `BatchPreparation` encoder/reducer,
with bounded event attempts and unchanged whole-batch and lifetime admission.
Every event must validate before the FIRST file I/O. A late invalid event cannot
leave an earlier valid record on disk. The original initial clone occurs once,
not once per event. No partial candidate is exported.

Writing starts on a subsequent poll. Its call/byte budgets use `Write::write`,
not an unbounded `write_all` loop. Interrupted calls consume their allowance;
short writes retain their exact returned offset. Zero progress, overreported
lengths and errors other than Interrupted latch Failed. Before every write, the
original expected extent is checked under the same lock. The private owner is
poisoned before metadata, seek or write operations, including caught unwinds.

Ready means ONLY that all canonical group bytes were reported written. It is not
a commit acknowledgment, durable head, effect receipt or usable ledger. Ready
polls do no further work. `finish()` checks the final extent, synchronizes that
same file once and only then installs the complete original candidate and returns
the original `FileJournal`. Its acknowledged head is now the final group head.
Incomplete/failed finish, sync failure or unwind returns no owner or candidate.
The only head visible during the session is its preexisting acknowledged minimum.

## Cancellation and uncertainty

Cancellation returns a progress report and drops the owner/lock. Before writing,
no journal bytes changed. After writing starts, any complete record prefix or torn
record can remain; cancellation performs NO truncation, rollback or refund. The
caller must retain the independent old head and use the original strict recovery
path. Complete persisted records retain their actual liabilities and consume
original capacity. Torn records still refuse, unchanged, without automatic repair.

A write that errors or panics can persist more bytes than it reports. Progress
counts returned successful bytes only, and retains all attempted calls; it cannot
claim that an error means nonexecution. External sends and observations keep their
original separate causal/durability barriers. No new external effect is sent by
this API, and no blind retry, remote-receipt authentication or exactly-once claim
is introduced.

Both original v1 and independently pinned v2 reserve profiles work unchanged.
Ordinary operations cannot consume recovery space. A lawful recovery group can
still finish when ordinary record AND byte allowance is exhausted. Settlement
can release effect units without replenishing encoded journal space. Existing
synchronous commit, batch commit, recovery and inspection bodies are unchanged.

Budgets measure logical event attempts, write calls and returned bytes, not CPU
instructions, physical disk traffic or elapsed time. Metadata/seeks, the initial
ledger clone and final sync are outside poll counters; any individual syscall
can block. Retained storage includes the original/private ledgers, owned input
list and original bounded group encodings. This is not a throughput, smaller
memory, nonblocking filesystem or physical-space reservation claim. Original
trusted-directory, cooperating-lock and filesystem synchronization assumptions
remain necessary; hostile same-length edits are not authenticated here.

## Source and validation status

2026-10-12: added seven public and six private regression functions plus two
compile-fail owner-boundary examples. Positive controls compare original canonical
bytes/rights under three schedules, finish a reserved group at exact byte/record
limits, retain the lock until one final sync, and settle unknown effects after
strict reopening. Negative controls cover all written-byte cancellation points,
late invalid events before I/O, zero budgets, incomplete extraction, EINTR and
short writes, write errors/unwinds, zero/overreported/WouldBlock returns, sync
errors/unwinds and changed extents before writing or sync. The tests use real
canonical files with private deterministic hooks; they are not power-loss proof.

The existing file.rs implementation body was checked byte-for-byte against its
fetched upstream preimage. Only module/type exports and an explicit incomplete-
commit error were added there. Source whitespace and patch checks passed in the
selected-file workspace. The required commands were attempted:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Both stopped before compilation: `rch: command not found`, exit 127. Rust
compilation, tests, rustfmt and Clippy remain UNEXECUTED. There is no full-checkout,
runtime, performance or production qualification claim. No dependency, wire/event
format, authority rule, shared registry or bead closure changed.
