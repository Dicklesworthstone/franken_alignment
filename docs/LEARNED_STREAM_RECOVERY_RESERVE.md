# Learned stream recovery reserve

The publication operator needs enough logical journal space to fence a recovered
owner, stop it and finish the original stop sweep after ordinary work is full.
Learned Enable already counts as work under the existing admission contract.
Installing a reserve after it is invalid even if no token has yet been sampled.

## Atomic startup

`FileOversight::create_with_learned_text_stream_with_reserve` accepts the original
profile, exact `FileLearnedConfig` and explicit `RecoveryReserve`. It constructs
one first image in this order:

1. Original StreamBootstrap.
2. Original ReserveRecovery.
3. Original learned Enable containing the complete learned text recipe.

The original replay and encoder validate the sequence. The existing store makes
one canonical replacement before returning the owner and separate human role.
There is no acknowledged partial stream or unreserved learned owner. Invalid or
unfunded reserves return no owner and publish no canonical image. A failed create
may leave the original empty store/lock directory; it is not a completed journal.

The original `create_with_learned_text_stream` still writes the same two events
without a reserve. Its bytes and subsequent late-installation refusal are
unchanged. The journal class of learned Enable is unchanged as well.

## Recovery from the same locked history

Begin the original guarded learned recovery with independently retained recipe,
guard inventory, effective policy and counter floors. Before the first replay
step, call `require_recovery_reserve` on that sealed recovery object with the
expected reserve. It compares the one original ReserveRecovery event from the
already decoded canonical bytes under the same exclusive owner lock.

A missing or different reserve, or a call made after replay has begun, releases
no writable owner and performs no cleanup or fence. The original decoder has
already checked uniqueness, pre-work ordering and both capacity dimensions.
The immutable recovered history cannot later replace that reserve. Advance and
finish remain the original cooperative replay and atomic final fence: all guard
requirements must match, and finish verifies the retained canonical cut before
cleanup and replacement. Old approvals stay withdrawn and numerical state stays
paused until original fresh-time and resume admissions succeed.

`supervise_publication create-learned-generated` now uses this constructor and
pins `RecoveryReserve::terminal()` before replay on both continuation and receipt
recovery. It no longer checks the reserve after an opener has already fenced.

## Post-read source admission

The command checks `capture_file_policy_state` immediately after acknowledging
post-read time at every generation and review-start refresh. This read-only API
checks the original registered source's complete frontier and conservative
read-start lease, then the command compares the resulting snapshot with the
actual captured file. Acknowledging time alone does not validate source freshness.
The exact-expiry boundary now refuses with Stale before admitting numerical work
or a pending step. The immediately preceding tick remains valid. Old journal
replay and the original half-open lease interval are unchanged.

## Finite original allowance

The terminal reserve is three events and fifty encoded bytes. Ordinary work,
including time, review, generation and new effects, cannot consume it. Only the
original Fence, Stop and StopProgress events may use the tail. Prior recovery
work still counts against total capacity; another restart does not refill it.
This is logical admission, not physical disk allocation or unlimited recovery.

The public integration harness exhausts ordinary space with an actual original
learned prompt step, verifies that another ordinary event refuses without a
write, then reopens with the exact reserve and executes fence, stop and drain.
Numerical work is retained, no output is published and the terminal event limit
is reached exactly. Separate cases verify zero startup inference, unchanged
legacy behavior, malformed limits and preservation of canonical plus staged
bytes on absent/different/late reserve requirements and an unmet external floor.

## Execution evidence and limits

All fifteen tests in `learned_text_stream_durable` pass with the qualified
`nightly-2026-09-08` compiler, including all four new reserve tests. The exact
policy-derived read witness remains part of the frozen action oracle. All six
`native_worker_lora` tests also pass through their actual helper processes.
All eleven direct/admission learned-command tests pass, including complete native
review, publication with both keys, pending-step recovery, cancellation work and
receipt-only reopen. The nonzero actor fixture now trains its V codec on a
constant population so its original coarse certificate is quiet; K still rotates
and remains compressed. Native input/work and publication assertions are intact.

The original `cargo run --locked -p xtask -- check` passes lock, source-inventory
and qualified-compiler checks, then rejects the reviewed source snapshot because
of unlisted files and stale existing digests. The gate has not reached formatting,
Clippy or complete workspace tests, and its frozen records have not been changed.
The full peer-listener command test remains subject to the observed environment
EPERM at Unix listener creation; direct owner tests do not establish that transport.
