# Source-only actor intake for native text streams

## Implemented boundary

`FileGeneratedTextActorPort` connects an independently bootstrapped mandatory
native-text stream to the original actor gateway. The actor can submit a
`FileTextMessageRequest` or request an explicit stream finish, then poll or cancel
its original gateway ticket. It cannot supply output bytes, a partial prefix,
model state, an observation snapshot, a clock, a verdict, or either effect key.
This serves the existing actor intake consumer (FA-107, plan 8.7, 9.11 and 17.1).
It is not an additional inference service or a production conformance claim.

`into_generated_text_actor_gateway` consumes the already locked owner and returns
this weak port with the original `FileActorSupervisor<FileOversight>`.
`into_generated_text_supervised_driver` instead adopts that same supervisor into
`FileSupervisedDriver`; its original helper, approval, dispatch and reconciliation
state machine is unchanged. Legacy streams refuse this conversion rather than
acquiring a stronger provenance claim. All separately held roles stay separate.
The supervisor decides which generation identities it discloses to the actor.
This remains a single scoped owner, not a cross-tenant generation registry.

## Admission and retry semantics

The original source-linked reducer derives bytes from the immutable tokenizer
and acknowledged generation. It still requires the complete reviewed prompt,
monitored control stop, exact numerical predecessor, nonempty UTF-8, current
source/clock and original stream bounds. It builds the cumulative frame with
ALL confirmed messages and charges the complete frame, never the source ID size.
Successful intake alone neither reviews nor publishes that message.

A new submission consumes exactly one independently supplied gateway snapshot.
No snapshot is taken from actor fields. Once source admission is attempted that
snapshot remains consumed on failure; the caller needs a new observation for a
new attempt. Malformed fixed identities, exact recorded retries, and conflicting
recorded identities do not consume a snapshot waiting for a different request.
Matching retries return the original request's CURRENT disposition. A stored
policy refusal returns a redacted `NotAdmitted` ticket, not private diagnostics.
An ordinary request cannot acquire a source link through this port.

Explicit finish uses the original stream builder with no new message. Its exact
confirmed prefix and target still require the normal review, human key and
publication checks. It neither terminates numerical generation nor promotes an
unfinished output to a complete message. Message and finish requests share the
same original idempotency namespace; they cannot replace each other under one ID.

Polling uses the existing `Knowledge<ActorOutcome>` projection. Publication may
be visible before its receipt is reconciled; that interval remains `Unknown`,
not `Executed`. Cancellation can release only an undispatched reservation. It
cannot refund a dispatched charge, cancel the source generation or resend output.

## Ownership, failure and recovery

Ports and tickets retain only the original weak gateway ownership. Dropping the
supervisor releases its owner even if an actor retains a port. A new recovered
gateway refuses old tickets; exact source retry reacquires a new ticket from the
original durable request book. Recovery still withdraws old approvals, pauses
inference and requires fresh observations before new work. Exact historical
retry alone does not resume inference or restore clock readiness.

The port adds no storage transition or journal tag. Source submission uses the
existing tag 31 and original checked replacement. Failed storage returns no
ticket; the original owner is unavailable until exclusive recovery. Old versus
visible-but-unacknowledged request cuts retain their existing meaning. Actor
isolation, authenticated channels, storage durability and snapshot authenticity
remain deployment assumptions, not consequences of this Rust wrapper.

## Qualification status

Eight `generated_actor_` regressions are authored. They compare actor intake to
the original reducer, exercise positive/negative snapshot consumption, incomplete
and held output, policy refusal, the original two-key lifecycle through the
retained supervisor, explicit finish, anchored reopen/weak ownership, and every
existing Store replacement barrier. The numerical fixtures are synthetic weights
run through the original implementation; the Store seam is not physical power
loss. Compile-fail examples reject host access and arbitrary-message submission.

The targeted attempt failed before compilation (`rch: command not found`, exit
127):

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference generated_actor_
```

Compilation, Rust tests, rustfmt, Clippy and the complete gate are UNEXECUTED.
Source hash and whitespace checks do not qualify runtime behavior. No dependency,
actor wire verb, production gate or broader bead is added or closed by this work.
