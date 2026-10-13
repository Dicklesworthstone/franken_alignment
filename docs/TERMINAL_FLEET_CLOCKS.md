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
