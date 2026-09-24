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

## Original wire and Unix transport integration

The source-only port now implements the existing sealed `ActorRequestPort`.
`ActorWire`, `ActorChannel` and `UnixActorConnection` consume it without changes
to their parser, response projection, connection-local ticket inventory, bounded
framing, backpressure, EOF rules, socket-work budgets or reconnect machinery.
The operator chooses this port for a mandatory-native stream; no wire bytes can
change the backend or downgrade it to a caller-text stream. The peer receives
only the opposite socket, not the trusted connection or its supervisor.

The existing version-one `Submit` carries a fixed 32-byte `FAGREF\0\x01` intent:
8 domain bytes followed by three big-endian u64 values: request ID, generation
ID and generation revision. The embedded request must equal the outer command's
request ID. A nonzero generation names an original text request. Generation zero
AND revision zero encode explicit finish; no other zero-generation form is valid.
Exact length, domain, request binding and `units == 32` are checked before gateway
lookup or snapshot consumption. Truncation, suffix bytes and ordinary text are
refused. The original strict JSON codec still checks duplicate/unknown fields,
outer target/deadline structure, decimal identifiers and transport limits.

`FileGeneratedTextActorPort::encode_message` and `encode_finish` construct those
ordinary `ActorProposal` values for a client without requiring access to a live
port. They encode intent, not proof that the generation exists or is complete.
For example, a caller with an independently supplied `FileTextMessageRequest`
constructs `Command::Submit { request: source.request, proposal:
FileGeneratedTextActorPort::encode_message(&source)? }`, then calls the original
`encode_command` and adds its transport newline. Existing `Poll` and `Cancel`
commands are unchanged; there is no new top-level operation or response variant.

The outer 32-unit value describes only this fixed input representation. It is
NEVER copied into effect accounting. The original source-linked reducer derives
the actual complete cumulative message frame and charges its full encoded size.
Finish similarly uses the original complete-prefix frame and two-key policy.
Neither the binary marker nor a successful JSON response is a publication permit.

A complete JSON document without its newline is not admitted by the channel.
A prior response must be written AND flushed before another command can enter.
Dropping a socket with an unsent reply does not cancel its admitted request. A
fresh wire session cannot poll that request by guessing its ID, but its exact
source submission reacquires a ticket without another observation or journal
write. Dispatched/published-but-unreconciled requests remain `Unknown`; socket
acceptance never becomes a claim that a remote audience received an effect.

Three further codec tests check independently specified source/finish bytes,
all truncation offsets, suffixes, conflicting request IDs, noncanonical finish
forms and full-width generation bounds. Five integration tests exercise original
JSON refusals, snapshot preservation, ticket reconnection, exact/one-under frame
limits, flush backpressure, truncated EOF, and actual Unix socket pairs. The
lost-reply case proceeds through the ORIGINAL reference congress/manual ballot,
separate human approval, dispatch, publication and reconciliation, comparing
numerical state and full-frame charge. A separate socket case publishes an
explicit finish without publishing the retained but unsubmitted model output.
The tests do not claim authenticated external helpers or remote delivery.

All SIXTEEN `generated_actor_` tests (eight gateway, three codec and five wire/
socket integration tests), compilation, rustfmt, Clippy and the complete gate
remain UNEXECUTED. The second targeted RCH command, with the same filter shown
above, failed before compilation (`rch: command not found`, exit 127). Source
inspection and blob/whitespace verification are not runtime qualification.
The new input payload profile adds no dependency, journal tag, actor verb,
listener, source-acquisition callback, second executor or peer authentication.
