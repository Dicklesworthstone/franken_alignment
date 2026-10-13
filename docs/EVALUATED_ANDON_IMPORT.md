# Resumable durable evaluated-Andon import

Source implementation, 2026-10-13; scoped continuation of the evaluator-to-accounting join in [EVALUATED_ANDON.md](EVALUATED_ANDON.md), plan section 9.9 and franken_alignment-credibility-ledger-dc0.4. No packet or qualification gate is closed.

`fa_journal::evaluated_andon::EvaluatedAndonImport` consumes the new opaque evaluated receipts through the ORIGINAL `FileAndonLedger`. It introduces no alternative encoder, journal, repair rule, storage dependency or refund transition.

## Admission and progress

`begin(&mut journal, &receipts)` borrows the existing locked writer and immutable receipt slice. It first validates every receipt on a private clone of the original accounting state. A wrong final record, wrong domain, missing occurrence window or conflicting historical round therefore refuses before the first append. The group is bounded by the original campaign case limit. This is semantic preflight, not a disk-space reservation: original file limits and real I/O failures can still stop later publication.

`advance(max_records)` makes at most that many original record calls. Zero makes no calls; exact duplicates count as a step but append nothing. Every newly recorded item keeps the original write-and-sync-before-acknowledgment behavior. The returned progress distinguishes pending, complete and failed imports, newly recorded versus already present receipts, and the exact last acknowledged head. It cannot mistake the end of one budget slice for completion. One synchronous record can exceed an elapsed-time budget; this is cooperative record-count control, not interruptible I/O.

`finish()` refuses incomplete or failed imports. A failure latch is set before entering an original record call, including a call that unwinds. The file owner's existing poisoning rules remain in force. There is no mutable inner ledger escape and no cloneable import owner.

## Interruption and recovery

Dropping the cursor stops local iteration; it does not refund acknowledged charges or imply that an uncertain append failed. Keep the original receipt sequence and every returned recovery head independently. After a process or I/O failure, reopen the same original file with the independently retained identity/head and start the same import again. The original replay includes complete unacknowledged suffixes; exact repeated receipts then count as already present without appending or charging again.

Publication is an explicitly acknowledged PREFIX, not all-or-nothing disk batch atomicity. A failed call may have persisted a later record than its returned progress. Never advance a head or infer nonexecution from the cursor's counter. Original replay decides what was committed. Corrupt or partial tails remain diagnostic and are not truncated or repaired by this adapter.

## Verification status

Seven additional Rust tests exercise the actual file adapter: bounded steps and anchored reopen, interruption/resume, same-group no-write duplicates, a conflicting last entry with no first write, incomplete/empty/oversized groups, complete uncertain-suffix replay, and partial-tail refusal. Fault tests copy bytes emitted by an original writer rather than inventing a replacement format. One compile-fail test rejects cursor cloning. These join the nine source-join tests in the reference crate.

The original journal root source was matched by its exact Git blob hash before adding the Unix module registration. The targeted `RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-journal evaluated_andon` and unchanged full `RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` both exited 127 before compilation because rch is absent. Compilation, rustfmt, Clippy and Rust runtime tests remain unexecuted. This does not establish evaluator authentication, statistical calibration, native operator wiring or production qualification.
