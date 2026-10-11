# Cooperative rights-journal recovery

The existing fa-journal reference profile can reconstruct its original accounting
without completing the entire stream in one poll. Journal::begin_replay binds
one reader, independently supplied Identity/Head, and immutable lifetime Limits.
Replay::advance subdivides reads and record checks; finish consumes only a fully
verified reconstruction. This does not add a journal format, dependency, runtime,
receipt authenticator, authority key, external effect, or repair operation.

The original synchronous Journal::replay remains the independent framing oracle.
Both paths use the existing canonical event decoder, checksum function, and
Rights reducer. Matching anchor bytes do not end recovery: every complete suffix
is checked and applied, including an unacknowledged dispatch/unknown outcome.
Only clean EOF between complete records plus anchor coverage permits finish.
Truncated or corrupt tails never return a usable prefix or get silently removed.

## Work and ownership

ReplayBudget bounds calls to the supplied Read, returned bytes, and attempted
complete-record checks independently per poll. Interrupted reads spend calls,
so a permanently interrupted source cannot busy-loop beyond its poll allowance.
EOF also costs a read attempt. A buffered record can be checked with zero new
reads, and at most one unchecked record is buffered. Header/framing completion
is included with its last read; a record check includes checksum, canonical
decoding and the original reducer. The latter may inspect already retained
rights: these limits are logical work, NOT CPU-instruction or elapsed-time bounds.
A supplied reader may block and may have its own buffering/read-ahead.

The cursor retains one frame buffer bounded by max_frame_bytes + 4 (or the fixed
84-byte header), plus original decoded event and rights storage. All original
record and total-byte limits apply across polls. No changing target, imported
prefix, reset offset, mutable reader, or intermediate ledger accessor exists.
Reported work survives cancellation and failure; cancellation is not a refund.
An error or unwind seals the cursor, even if a private reducer partially changed.
WouldBlock is an I/O error, not a temporary end-of-stream or an async readiness
protocol. Only Interrupted is retried, within the read-call budget.

## Source validation status

Eight public regression functions compare the complete state, head, encoded
length and error boundary with original synchronous replay. They cover all
transition kinds, unacknowledged suffixes, four poll schedules, truncation and
single-byte damage at every fixture offset, lifetime-limit neighbors, unknown
liabilities, fragmented/Interrupted readers, cancellation, hostile lengths,
WouldBlock and reader unwinds. Two compile-fail examples prohibit prefix reads
and cursor cloning. Existing wire/replay tests are preserved.

The preparation environment has no RCH or Rust toolchain. Authored Rust and
compile-fail tests, compilation, rustfmt and Clippy are unexecuted. The required
remote-only gate must run on the integrated revision; source/hash checks are not
runtime, crash, performance or production qualification. No bead is closed.

## Locked file recovery and its existing synchronous consumer

FileJournal::begin_open opens the existing file under its original exclusive
writer lock and returns FileRecovery. The same lock stays held during all polls,
after Ready, and through final file-plus-parent-directory synchronization.
Only consuming finish returns the original FileJournal. Incomplete finish,
failed replay, changed file extent, sync failure or unwind returns no writer.
Cancellation/drop releases the lock and discards only private reconstructed state;
it never creates, truncates, rewrites or repairs journal bytes.

The existing FileJournal::open now drives this same recovery implementation to
completion. Both APIs retain the original identity/limit admission before open,
regular-file/extent checks, independently supplied minimum, strict suffix replay,
and sync-before-return. Complete unknown liabilities are retained, not refunded;
recovered accounting is still not an external dispatch or authenticated receipt.
No new event or epoch is invented, and append/commit semantics are unchanged.

The file adapter uses an explicitly bounded 8 KiB BufReader. Replay read counters
charge that interface's delivered bytes and calls, not underlying physical I/O
or read-ahead. Opening/locking, metadata checks and final synchronization remain
separate operations. The lock is advisory and assumes cooperating writers in a
trusted directory; extent rechecking is not malicious-writer authentication.

Seven public file regressions cover tiny polls, unknown suffix recovery and the
next append, locks through recovery, cancellation at framing boundaries and after
Ready, premature finish, damaged files, context/anchor and lifetime limits, and
an extent change after EOF. Three injected-barrier tests cover failure before
file sync and at directory sync, premature sync prevention, and a sync unwind.
They retain original bytes and require the lock to release on unsuccessful
finalization. These ten additional Rust tests and the file compile-fail example
are authored but unexecuted under the same RCH limitation stated above.
