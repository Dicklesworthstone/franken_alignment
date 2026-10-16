# Reference action retirement

This implements pre-dispatch deadline and revocation cleanup for the existing
FA-001 action lifecycle and consequence gate (plan §8). It is an in-memory
reference capability, not a durable broker, live-service integration, verified
clock, remote outcome proof, or release qualification.

## Observable behavior

After the caller supplies a monotonic `ElapsedTick`,
`ReferenceAuthority::retire_invalidated()` cancels Proposed, Prepared, Reviewing
and Authorized attempts whose deadline has been reached or whose policy epoch
is older than the current floor. Both causes are retained when both apply.
Future policy epochs do not count as revoked.

Only an exact matching Reserved ledger entry can refund units. The complete
batch is staged against the original accounting before any change becomes
visible. Missing time, malformed accounting or arithmetic failure leaves the
owner unchanged. Successful retirement clears the cancelled judgment, retains
the frozen envelope and consumed attempt ID, and permits the released units to
fund a different action through the ordinary review/authorization path.

Dispatching, Unknown, IrrecoverablyUnknown and terminal states are untouched.
Timeout and revocation never prove nonexecution. Only the existing trusted
outcome reconciliation can refund a post-dispatch liability.

## Consequence-gate integration

`ConsequenceAuthority::retire_invalidated(expected_control_sequence)` uses the
same staged operation without exposing the inner authority. A nonempty batch
advances the existing control sequence once, records a `RetirementReceipt`, and
makes retired attempts non-continuable. Older review predecessors then refuse.
Unexpired holds, target ceilings, suspension, revocation floors and historical
charges remain unchanged. Retirement is not another helper vote and does not
invent a round, evidence root or permission.

Review and retirement records share `MAX_DECISIONS`. A full record budget or
sequence overflow refuses before accounting changes. With a current predecessor,
a no-op returns `None` without consuming a record or sequence, including at those
limits. Receipt inspection permits callers to recover the result of a completed
in-memory operation; there is no persistence or network retry guarantee.

## Bounds and verification status

The scan visits at most the authority's configured `MAX_ATTEMPTS` ceiling and
returns ascending attempt IDs. Retired attempts remain in history, so this does
not reclaim audit slots or make a full authority reusable indefinitely. No new
dependency or executor is introduced.

Twelve added Rust test functions cover budget reuse through original dispatch,
exact deadlines, both invalidation causes, future epochs, mixed liabilities,
staged failure/retry, consumed identities, full attempt/record bounds, maximum
integer accounting, obsolete review predecessors, held effects, narrowed
ceilings and suspended-run reconciliation.

These tests are **unexecuted** in the authoring environment: `rch`, `cargo`,
`rustc` and `rustfmt` are unavailable. Source reconstruction and diff checks are
not compilation, formatting, Clippy or behavioral validation. The required
remote-only `cargo run --locked -p xtask -- check` remains outstanding; no bead
or release gate is closed by this change.
