# Authenticated actor intake with journal-backed work scheduling

`FileActorInbox` connects the existing Linux `PeerSession` and source-aware actor
admission to a bounded supervisor work queue. It is an integration of FA-107's
actor boundary and the original durable review driver, not an alternate executor.

Construct it from the `FileActorPort<FileOversight>` returned by the original
owner's `into_supervised_driver()`, wrapped in `ActorWire`, plus an independently
selected `PeerPolicy`, `ChannelLimits` and lifetime connection ceiling. The
supervisor supplies a connected socket to `attach`; the native kernel-credential
check runs before any protocol input. Socket naming, permissions, scheduling and
which peer identities are trusted remain the embedding host's responsibilities.

`drive` takes the SAME driver, its registered evidence source, a current-clock
provider and a native `DriveBudget`. Complete Submit frames pass through the
original admission and request ledger. Partial frames, poll/cancel, exact retries
and conflicting retries retain their original behavior. Each read/flush/frame
budget is enforced by the existing transport. Reconnects retain the same tickets
and connection counter. A refused attachment cannot replace an existing peer.

The queue stores only external request IDs. After a drive, an encountered Submit
is checked against the actual native request status before it can be queued.
Encountering an ID is not proof that this particular Submit succeeded: a
conflicting retry may name already admitted work, which is still the ORIGINAL
request. Duplicate queued IDs collapse. No proposal, verdict or receipt is copied
into a new ledger, and no socket write is mistaken for effect acknowledgment.

`next_request(&driver)` rechecks the original owner and status, then returns the
next actionable request in observed order. Reviewing work may enter the normal
trusted helper workflow. Dispatching or Unknown work may ONLY enter original
reconciliation; it must never be restarted as a review or resent. Cancelled,
denied, executed and irrecoverably unknown work is skipped, not resurrected.
Dequeuing is not admission, acknowledgment or permission. A lost reply does not
erase work already committed to the original journal.

The queue is bounded by `MAX_FILE_REQUESTS` and is not persistent. Following a
process restart, the native recovery fence applies. An actor can recover its
original request visibility through an exact submit retry. Dropping, disconnecting
or revoking an inbox never cancels or refunds accepted work: the supervisor still
owns stop, recovery and settlement. Authentication identifies a local connecting
process, not its executable, a human or the current holder of a transferred file
descriptor. This adds neither a sandbox nor a production runtime.

Seven integration regressions exercise real socket pairs and native canonical
files: FIFO pending work, partial-frame and cancellation behavior, missing evidence
and recovery, retained tickets/conflicting retries, foreign-owner refusal, lost
output with ingress revocation, and bounded drive admission. They do not run
models. Compilation and all Rust tests remain unexecuted in the editing environment;
RCH and the Rust tools are unavailable. A fresh operator RCH gate is required.
