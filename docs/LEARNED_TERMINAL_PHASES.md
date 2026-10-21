# Stop learned admission without a settlement clock

FA-014; plan 16.4/16.5/25.4; FI-A02. A learned numerical history must not have to
obtain a settlement timestamp merely to withdraw new admission. The original
`FileLearnedRecovery` now exposes `finish_admission_stop` and
`finish_stopped_drain`, returning the existing immutable stopped-history types.

Complete original learned replay precedes a private handoff of the SAME machine,
canonical bytes, events and exclusive Store to the existing terminal consumer.
There is no second numerical replay, ordinary recovery fence, owner/role escape,
resampling, replacement forecast or completion of a pending numerical intent.
A new admission stop uses exactly Stop/Fence; a needed later drain uses exactly
one StopProgress event. The original three-event terminal reserve suffices for
both phases; no budget or unknown-effect charge is refunded by the handoff.

Drain refuses an active domain before acquiring time. Its fallible clock is
called only after replay and canonical/intent checks. A clock error or unwind
leaves the earlier admission stop unchanged. Already-stopped admission retries
and already-drained retries confirm the original cut without a write; drained
retries need no clock even at capacity. Changed canonical bytes and wrong stop
identities refuse under the original checks, before staged-file cleanup.

`try_finish_stopped_with_anchor` also exposes the original combined transaction
with a fallible clock. Callers can refuse missing or late time without supplying
a synthetic timestamp or panicking for control flow. Existing infallible methods
keep their implementation and signatures. Native acknowledgment, externally
retaining its returned history anchor, and reporting are separate operations.
Failed storage can leave a visible replacement; only explicit verified reopening
can resolve it. A failure is never proof of nonexecution.

This first increment exposes the native learned cursor. Specialised guarded,
predictive and topology wrappers and the runnable learned operator require their
own full independent checks before using it; this document does not claim those
consumers have been connected by this increment. The original base and specialised
recovery entry points still require exact independently supplied configurations.

Five new regression functions use original sampled numerical state, pending
numerical intents, original two-key publication, Machine transition/encoding
oracles and all five Store barriers. They cover reserved/sent/executed/expired
liabilities, exact terminal reserve, clock-free full-capacity retries, incomplete
and corrupt replay, active-drain refusal, failed/panicking clocks, both phased
write ambiguities and retained numerical/token/cache/sampler state. Existing
tests and assertions are unchanged. These tests are authored, not executed.

Required targeted and full xtask execution must use RCH against the exact
integrated revision. The editing environment has no rch/cargo/rustc/rustfmt;
source/hash/diff checks are not compilation, test or production qualification.
FA-014 remains open. Clock authenticity, storage isolation and independent
anti-rollback custody remain assumptions; synchronous callbacks and storage
are not hard-real-time cancellation boundaries.
