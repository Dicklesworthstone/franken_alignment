# Durable pending-effect reconciliation

This extends the narrow Unix `FileDelivery` profile described in
`FILE_DELIVERY_RECOVERY.md`. It does not add an alternative rights ledger or turn
reference ballots into authenticated helper observations.

## Connected operation

`FileDelivery::reconcile_pending(revision)` now drives the ORIGINAL
`DeliveryBroker::reconcile_pending` against the same reconstructed memory-only
endpoint. Every retained in-flight obligation is visited. A recorded execution
is acknowledged without reexecution; a missing live request stays Unknown and
charged; an expired request is refunded only after the original endpoint records
its terminal nonexecution receipt. Retention expiry is not that receipt.

The typed result is a map from attempt ID to `Result<Reconciliation, Error>`.
An individual unsuccessful resolution does not hide successfully resolved
siblings. No proposal, review, human approval, permit, clock observation or
sendable envelope is created by a sweep. Fresh time must have been explicitly
admitted, including after reopening; stale revision and unavailable-clock calls
leave the canonical history unchanged.

The existing journal transaction first applies the entire sweep to its private
RAM projection. It then commits the original transition inputs as one canonical
replacement before returning any outcomes. Successful sibling settlements commit
together, while unresolved and failed entries retain their original liabilities.
No per-attempt refund is exposed to the caller before persistence succeeds.
The event tag is 10 in the existing version-one format; unknown event tags still
refuse rather than being skipped. The existing 4,096-event/16-MiB ceilings apply.
An empty or idempotent sweep still consumes an event slot in this baseline.

## Failure and process-death scenarios

The storage owner has test-only one-shot barriers before staging-file creation,
write, file synchronization and rename, and after rename but before directory
synchronization. There is no public fault-injection interface. A directory-sync
failure therefore exercises a real canonical replacement that can already be
visible while the failed owner retains its last acknowledged in-memory state.
The owner refuses further mutation; its inspection is not fresh authority.

Three internal test functions exercise publication failures at all five barriers,
mixed execution/nonexecution sweeps at all five barriers, and framed-but-illegal
history plus every truncation of a small selected journal. The tests distinguish
pre-rename history from post-rename history and require recovery to use the actual
canonical cut without issuing a speculative refund or replaying a publication.

Three public integration tests cover recovered mixed batches, retention expiry
alongside a younger resolvable obligation, stale revision/fresh-clock refusal,
and a late delivery after a persisted expiry seal. They construct real policy
reviews and original permits through the public reference profile.

A separate subprocess test launches the same test executable as an owner, waits
for a real selected operation to return, confirms lock contention, forcibly kills
and waits for that child, and reopens the same store in the parent. Its three modes
stop after authorization, dispatch, or publication. The parent checks released
kernel ownership, cancellation of unspent reservations, retained uncertain charges,
and execution receipts without resend. The child entry point is also a test
function and is inert unless its explicit environment is supplied. These are
process-death scenarios, NOT hardware power-cut or hostile-filesystem tests.

## Execution status

The required RCH command was attempted during editing and failed with
`rch: command not found`. There is no local Cargo/rustc fallback. The added Rust
source, subprocess tests, fault campaigns, doctests, formatting and Clippy have
NOT been compiled or executed in this session. Test source is not proof that
any scenario has passed. No Beads task, production claim or revision-bound
qualification gate is closed.

Commands for the repository's required remote runner:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --lib action::consequence::delivery::persistent::tests
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_delivery --test file_delivery_sweep --test file_delivery_processes
```

This is reference progress toward the existing FA-006, FA-012 and FA-014 contracts.
All plaintext, trusted-clock, cooperative-lock, bounded rewrite/replay cost and
missing cryptographic/independent anti-rollback qualifications from the base
profile still apply. The original reducers and their assertions are unchanged.
