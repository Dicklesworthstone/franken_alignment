# Serve original actor receipts without opening new admission

The Linux `supervise_publication` example accepts an explicit restricted mode:

```
supervise_publication serve-open CONFIG ACTOR_PROFILE --receipts-only
supervise_publication serve-open-checked CONFIG ACTOR_PROFILE WITNESS_PROFILE --receipts-only
```

There is no create variant, implicit fallback, reviewer-profile argument, new
credential capsule, numerical recipe, fleet selector, or request sequence. The
whole option set is validated before any configuration read. Existing service
and actor-submit command branches retain their behavior.

This opens the same original live-service deployment through its existing plain
or witness-checked recovery path, including its native recovery fence and exact
profile checks. It does not introduce a weaker replay engine or accept a stored
profile as new authority. Opening is NOT read-only: the original fence withdraws
old approvals and can cancel undispatched reservations. A failed startup after
that acknowledgment cannot undo the fence. Other guarded/learned deployments
must use their existing explicitly matched recovery interfaces, not ignored flags
on this command.

After recovery, the only actor ingress is the selected request's
`FileActorRecoveryPort` inside the original `PeerSession`. The server checks
actual Linux peer credentials before decoding actor traffic, preserves native
connection/exchange limits across reconnections, and applies the selected key to
Submit, Poll and Cancel. No fresh policy evidence, helper executable, reviewer
socket, logical clock, generation or reconciliation is invoked. The configuration
still describes the original deployment, but helper files and live evidence need
not be available. Historical replay is distinct from new source acquisition.

An exact Submit recovers a ticket only when the original server request already
exists. A missing key returns Withheld without creating a request or consuming an
admission snapshot. A retained refusal stays refused, an executed request stays
executed, and an unknown dispatch stays unknown and charged. Foreign keys and
altered proposal fields cannot replace records. No returned observation is an
execution permit. Cancel retains the original law and cannot refund a dispatched
or executed effect. This is non-admitting service, not a completely read-only
capability or a settlement service.

A durable actor whose earlier submission received no reply can explicitly use
its existing document-free `resume MIN_REV` against this operator-provisioned
restricted endpoint. The client sends the exact saved proposal to reacquire its
ticket, but that receiver cannot create it when absent. The unchanged client
records the original response with its write-ahead semantics. Pending and
OutcomeUnknown polling remains bounded by the client's original budgets; this
server does not make either state terminal. Ordinary cancellation against an
admitting endpoint still requires the prior acknowledgment enforced by its
existing command. An arbitrary server cannot be made non-admitting merely by a
client's intent; provision the service and its identity independently.

The finite monotonic service window starts before config ingestion. It is checked
before binding/opening, after recovery, and around each original socket drive.
No peer arrival, rejection or reconnect restarts it. Candidate refusals remain
charged to the existing per-invocation quota. Expiry closes the session and releases
the original owner; pending replies can be lost without inventing outcomes or
resending effects. The stderr summary describes transport work, never success of
an effect. Synchronous config I/O, recovery and socket operations can exceed a
checkpoint; this is not preemptive cancellation, an independent watchdog, or a
bound on kernel/disk latency. Unknown liabilities require separate native
reconciliation after this exclusive owner has been released.

Six added regression functions cover actual Linux receipt/cancel exchanges for
reserved, sent, published and confirmed effects; original open with an unavailable
live source and retained refusal; missing selected requests after recovery;
closed flags/scope/path collisions; deadlines before open versus after its fence;
and kernel credential rejection without request disclosure. They reuse original
file owners, full-input congress and separate human approval. The native gateway
increment also has six tests and one compile-fail boundary. These are authored
assertions, not passing executions. RCH is unavailable here; compilation, tests,
rustfmt and Clippy remain unverified. No existing assertions, dependencies, wire
commands, journal formats, Beads states or qualification gates change.
