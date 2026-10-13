# Acquire fleet settlement time at the terminal boundary

FA-014; plan 16.4, 16.5 and 25.4. A time sampled before replay of an entire
fleet is not a new observation at a later domain's terminal boundary. Conversely,
resolving an already completed native stop should not require a working clock.
The new opt-in native path separates clock identity from clock acquisition.

`recover_and_drain_with_clock(revision, binding, max_events, clock, checkpoint)`
checks the independently registered domain and clock identity before persisting
intent or calling either callback. It writes an original coordinator intent,
retains the original domain lock, and replays each original event once. The
checkpoint receives zero, each event quantum and Ready; false records a negative
Unavailable completion without a domain write or staging cleanup.

Only after complete replay, exact stop-identity admission and a canonical-byte
comparison does the terminal path call the bound, fallible clock. A matching
already-drained stop does not call it, including at domain journal capacity.
Acquisition failure, invalid history, cancellation and insufficient event space
also cannot create a substitute time observation. A clock error is an acknowledged
native refusal. Callback unwind leaves Entered and the coordinator unavailable;
reopening and passive resolution do not silently retry an unstopped domain.

The acquired tick goes into the ORIGINAL Stop/Fence/StopProgress transaction.
Every encoded prefix, aggregate retained-head limit and recovery reserve still
passes before mutation. The existing finalizer checks canonical bytes again after
time acquisition and before cleanup/replacement. The domain and coordinator
remain separate acknowledgment boundaries; a failed coordinator write may follow
a completed stop. Neither failure nor missing time proves nonexecution. Expired
unknown effects retain charges; exact receipt-only completion never refreshes
expiry using consumer read time or an earlier saved timestamp.

A new `RecoverStoppedAtCompletion` intent contains no asserted tick. Coordinator
format `FASHCOO` version three encodes that intent as tag four; original versions
one and two keep their exact representation and meaning. Original domain journal
formats and native reducers are unchanged. Time, when successfully consumed, is
retained in the domain's original terminal event. Native resolution and fleet
resume recognize this intent as eligible evidence-only work, never an implicit
new command. Independently retained coordinator anchors preserve the unchanged
intent kind and final result through the existing continuity checker.

Clocks remain trusted observations in the registered domain. A callback can lie,
block, fail, or return a regressing tick; native time checks still apply, but this
API does not authenticate it or promise hard-real-time completion. It guarantees
where acquisition is requested, not that time stops during preflight/fsync.
Existing supplied-time APIs remain available with their explicit older contract.

Eight new real-file/native regression functions cover late retention cutoffs,
clock-free full-capacity completion and passive resume, binding/quantum admission,
failed clocks with positive retries, every replay cancellation cut, canonical
changes before/during acquisition, legacy/v3 framing and anchor continuity, all
five barriers in both stores, and caught clock unwinds. Original two-key effects
are used for the charge tests. Fixtures are not authenticated time or hardware
fault evidence. Rust compilation, tests, formatting and Clippy remain unexecuted
in the editing environment; exact-revision RCH verification is required. FA-014
and its prerequisites remain open. No dependency or runtime is introduced.

## Full-roster clocked passes

`recover_registered_pass_with_clocks` and `resume_registered_pass_with_clocks`
accept the complete independent roster of `FileShutdownClockBinding` identities,
not a list of early timestamps. They share the original full-roster, latest-visit,
clock-domain, visit/attempt-capacity and revision-growth admission with the
supplied-time APIs. Admission precedes callbacks and does not create a dummy tick.

Only new recovery selected by the original plan invokes the clocked single-domain
operation. The callback runs independently after each domain's verified replay;
there is no cached time shared by later domains. Eligible retained visits use
passive resolution, including alongside live owners, with no time acquisition.
An evidence failure never falls through into a new command. Cancellation preserves
the original distinction between a session-only evidence refusal and an
acknowledged negative new visit, leaving later members NotVisited.

`FileShutdownClockedRecoveryPass` retains the original per-member result types
plus an optional acquired tick. None means no successful acquisition; it is not
zero or the previous member's time. A stale returned tick remains visible as an
actual observation alongside native refusal. A successful acquisition is not a
successful native write. Native failures may permit the remaining roster to
proceed; a failed coordinator acknowledgment ends the pass. Unwinds propagate
and retain entered quarantine. No result predicate imports earlier session success.

Seven further native regression functions cover per-domain acquisition ordering
and original supplied-time oracle parity, mixed ambiguous/exact/new resume at the
visit ceiling, clock failure versus stale acquisition, every checkpoint, complete
admission before callbacks, all intent-write barriers, and a second-clock unwind
following an acknowledged first stop. Existing tests are unchanged. These tests
are source only and await the same RCH qualification; no executed result is claimed.

## Runnable operator selection

The existing command explicitly selects this path with `--terminal-clocks`:

```sh
cargo run --locked -p fa-reference --example shutdown_fleet -- create /operator/fleet.json --terminal-clocks
cargo run --locked -p fa-reference --example shutdown_fleet -- resume /operator/fleet.json --terminal-clocks --coordinator-anchor /operator/custody/previous.anchor --retain-coordinator-anchor /operator/custody/next.anchor
```

Both existing fleet profile formats and independent coordinator-custody options
are supported. Without the flag, the original supplied-time invocation and result
schema remain unchanged. A duplicate flag refuses; selecting terminal clocks never
falls back to early timestamps after an error. Plan/configuration/custody admission
and original anchor retention/reporting are shared, not reimplemented here.

The Unix clock is sampled only when a verified member requires native settlement.
The original cumulative elapsed budget is checked at each replay checkpoint and
immediately before and after that clock call. A slow or failed clock supplies no
usable time and leaves the domain unchanged; the negative coordinator completion
may still require storage. A later member can proceed after a clock refusal, but
elapsed expiry ends the remaining pass. Evidence-only resume can succeed without
a working Unix clock; its elapsed deadline still requires monotonic observations.

The selected path emits `fa.shutdown-fleet-result/2` using the original result
fields plus `acquired_tick` per member. A string is the exact callback result,
not confirmation of storage. Null means no tick was returned to native settlement:
resolution, already-drained rereads, pre-acquisition refusal/cancellation, or a
failed clock all qualify. A clock that blocks past the elapsed deadline is also
rejected without reporting its result as an admitted observation. No zero timestamp
is invented for these states. Earlier successful members and every unvisited tail
remain explicit. Only original per-member receipts determine stopped/drained.

Seven operator tests cover independently acquired ticks and prior-member settlement,
clock-free exact resume at the visit ceiling, clock refusal with a reachable tail,
expiry during acquisition, caught clock unwind without retry, actual flag routing
with a legacy control, and anchored retention before lost stdout. These augment
seven fleet-pass tests and the eight single-domain tests (22 authored functions).
Existing test assertions are unchanged. Targeted native/example and full xtask
commands remain required through RCH; absent tooling does not qualify this code.
