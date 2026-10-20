# Clock-independent registered fleet admission stop

FA-014; plan 16.4 and 25.4. Registered offline recovery can now withdraw
admission without requiring a working settlement clock, then drain later through
the same coordinator and original native receipts.

`recover_admission_stop(revision, binding, max_events, checkpoint)` records a
StopAdmission intent before opening the registered domain. The original locked
cursor validates its newest retained history and replays each event once. It
then invokes the original `finish_admission_stop`: two Stop/Fence events for a
new stop, or confirmation of a matching existing stop without another event.
There is no clock callback or time observation. Existing unfenced stops remain
reported with their actual fence state; confirmation never claims a new fence.

`drain_stopped_with_clock(revision, binding, max_events, clock, checkpoint)`
records a distinct DrainStoppedAtCompletion intent. It requires an already
matching stop, acquires time only after verified replay and canonical comparison,
and uses just one original StopProgress event when settlement is needed. Already
drained observations skip the clock and write, including at capacity. A two-event
stop plus a later one-event drain fits the original three-event terminal reserve.
An active domain is refused before clock acquisition rather than silently stopped.

Every original event/byte bound, source-admission check, consistency check and
recovery reserve remains in force. The registered aggregate retained-head bound
is checked before native mutation. Returned observations preserve the real latest
sweep and the exact dispatch list since registration. Admission stopping never
manufactures a sweep or refunds sent/unknown/executed effects.

## Recovery and acknowledgment

Both operations retain zero/quantum/Ready checkpoints and the original exclusive
domain lock. Cancellation records a negative coordinator result without changing
the domain's canonical or staged bytes. Callback unwinds leave Entered and the
coordinator unavailable until reopening. A failed domain write may still be
visible; successful native settlement can precede a failed coordinator completion.
Neither outcome licenses effect resend or an inferred refund.

The existing passive resolver recognizes these intents. Saved observations must
still be reread at their exact cut; an unstopped domain is not automatically
retried. A resolved drain intent reports the actual stop progress, not a claim
that its requested drain ran. The old combined APIs remain explicitly available.

Coordinator encoding version four adds tags five and six for these two different
operations. Histories without them retain their exact version-one/two/three
encoding. Older readers reject the new version instead of treating admission
withdrawal as a completed sweep. Native domain journal formats do not change.
Independently retained coordinator anchors preserve intent identity and finalized
results through the existing continuity check, including across the new kinds.

## Verification boundary

Eight new regression functions use original two-key effects, native Machine
oracles, real files and the existing five Store failure barriers. They cover
reserved/sent/executed/expired liabilities, every replay cancellation point,
exact reserve use and read-only retries, active/foreign/capacity refusals, clocks
that fail or unwind, canonical replacement around time acquisition, independent
anchors and legacy/new encoding boundaries. Existing assertions are unchanged.

These tests are authored, not executed. The editing environment has no RCH or
Rust toolchain. Required targeted and full xtask execution remains pending on
the exact integrated revision; source comparisons and successful commits are
not compilation or qualification. No Bead is closed or production gate activated.
The APIs authenticate neither clocks nor custodians, do not establish real-world
fleet completeness, and cannot make synchronous I/O hard-real-time.

## Full-roster passes and runnable selection

The existing `shutdown_fleet` executable selects the phases independently:

```sh
cargo run --locked -p fa-reference --example shutdown_fleet -- create /operator/fleet.json --admission-only
cargo run --locked -p fa-reference --example shutdown_fleet -- recover /operator/fleet.json --drain-only
cargo run --locked -p fa-reference --example shutdown_fleet -- resume /operator/fleet.json --admission-only
```

Both roster formats and the existing coordinator-anchor input/output options are
supported. The phase flags are mutually exclusive and cannot be combined with
`--terminal-clocks`. Unknown/duplicate flags and flags in a custody-value slot
refuse before profile intake. No-flag and `--terminal-clocks` behavior retains its
original contract and output schema. No automatic stronger-to-weaker fallback
or new clock, source, helper, review or publication route is introduced.

`stop_registered_admission` and `resume_registered_admission` accept the complete
registered bindings but no logical clock provider. New work invokes the native
admission stop. `drain_registered_stopped_with_clocks` and
`resume_registered_stopped_with_clocks` restrict new work to already stopped
members and acquire independent time only for needed settlement. All four share
the existing full-roster admission, latest-visit selector and checkpoint loop.
The original immutable clocked-pass result is reused, with absent acquired ticks
rather than invented times on the admission path.

Resume remains evidence-first, NOT a command retry. Eligible retained visits
are inspected, including undrained ones; missing or unstopped evidence does not
fall through. Explicit `recover --drain-only` is needed to request new settlement
after an acknowledged admission pass. The original visit/session allowances
apply to both phases, so an operator must provision enough visits for its chosen
operations; this does not replenish budgets or make fleet settlement atomic.

The new `fa.shutdown-fleet-result/3` report names the selected phase and whether
settlement was requested, while retaining every original per-member outcome,
charge, acquired tick and explicit fence state. Admission-only success requires
all registered members observed stopped, not drained; drain-only success still
requires all drained. An unknown effect stays charged and unresolved even when
its domain's admission stop succeeds. Busy/offline/cancelled members remain in
the denominator. Coordinator write failures end the pass with an unvisited tail.

Both execution phases share the original cumulative deadline, independent
custody admission, archive retention and output-failure handling. Failure to
retain/report cannot undo acknowledged native work. Admission stopping acquires
no Unix-time observation, but cooperative deadlines still use monotonic elapsed
time. Synchronous acquisition, one replay event and storage work can exceed that
budget; this is not a hard-real-time or simultaneous fleet halt guarantee.

Six further native regressions cover complete two-key fleet stop/drain, active
drain controls, mixed interrupted resume at the visit ceiling, cancellation,
full-roster admission and failed intent writes. Five operator regressions cover
actual phase routing and clock-free resume, active-drain refusal, closed options,
every elapsed checkpoint, and independent custody surviving lost stdout. The
combined increment contains nineteen authored regression functions. None has
executed in this environment. Targeted native/example and unchanged xtask RCH
launches still require exact-revision execution; FA-014 remains open.
